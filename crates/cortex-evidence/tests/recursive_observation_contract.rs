//! Recursive observation ordering, bounds and retained uncertainty.
#[path = "support/program.rs"]
mod program_fixture;
#[path = "support/domain.rs"]
mod support;
use cortex_core::program::{domain::*, scheduling::*};
use cortex_evidence::program::*;
use support::*;

fn immutable_holds(record: &DomainRecord) {
    assert_eq!(record.outcome, HistoricalOutcome::Unknown);
    assert!(record.cancelled);
    assert!(record.closure.is_none());
    assert!(record.settlements.is_empty());
    for demand in &record.intent.effect.demand {
        if matches!(
            demand.resource,
            Resource::WorkerSlots
                | Resource::BuildJobs
                | Resource::TestProcesses
                | Resource::CpuMillicores
                | Resource::RamBytes
                | Resource::DiskBytes
        ) {
            assert!(record.holds(demand.resource));
        }
    }
    assert!(record.unresolved_consumption(Consumption::CpuExecutionMillis));
}

#[test]
fn review_later_empty_direct_preserves_recursive_descendants_and_all_holds() {
    let (mut k, intent) = coordinator();
    observed_cancelled(&mut k, &intent);
    let original = closure_tree_observation(&intent, 5);
    event(
        &mut k,
        "review-tree",
        DomainEvent::ClosureTreeObserved {
            domain: intent.id.clone(),
            observation: original.clone(),
            at: 5,
        },
    );
    let direct = closure_observation(&intent, 6);
    event(
        &mut k,
        "review-direct",
        DomainEvent::ClosureObserved {
            domain: intent.id.clone(),
            observation: direct.clone(),
            at: 6,
        },
    );
    let record = k.domains().unwrap().domain(&intent.id).unwrap();
    let inspection = record.closure_inspection();
    assert_eq!(inspection.retained_observations, 2);
    assert!(!inspection.receipt_recorded);
    assert_eq!(
        inspection.missing,
        vec![
            ClosureProofGap::PopulatedDomain,
            ClosureProofGap::RemainingMembers,
            ClosureProofGap::WholeDomainCoverage,
            ClosureProofGap::HelperCoverage,
            ClosureProofGap::StreamCoverage,
            ClosureProofGap::SnapshotExclusion,
            ClosureProofGap::FinalConsumption,
        ]
    );
    assert!(record.closure_observations == vec![direct.clone()]);
    assert!(record.closure_tree_observations == vec![original.clone()]);
    immutable_holds(record);

    // A later empty recursive tree removes only those latest-tree population
    // gaps. It cannot alter the original capture or settle any capacity/debt.
    let mut empty_tree = closure_tree_observation(&intent, 7);
    empty_tree.nodes.truncate(1);
    empty_tree.nodes[0].populated = false;
    empty_tree.sample.populated = false;
    empty_tree.sample.cpu_micros = Some(0);
    event(
        &mut k,
        "review-empty-tree",
        DomainEvent::ClosureTreeObserved {
            domain: intent.id.clone(),
            observation: empty_tree.clone(),
            at: 7,
        },
    );
    let record = k.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(record.closure_inspection().retained_observations, 3);
    assert_eq!(
        record.closure_inspection().missing,
        vec![
            ClosureProofGap::WholeDomainCoverage,
            ClosureProofGap::HelperCoverage,
            ClosureProofGap::StreamCoverage,
            ClosureProofGap::SnapshotExclusion,
            ClosureProofGap::FinalConsumption,
        ]
    );
    assert!(record.closure_observations == vec![direct]);
    assert!(record.closure_tree_observations == vec![original, empty_tree]);
    immutable_holds(record);
}

#[test]
fn review_direct_recursive_timestamp_order_rejects_equal_and_older_without_mutation() {
    for tree_first in [true, false] {
        for rejected_at in [4, 5] {
            let (mut k, intent) = coordinator();
            observed_cancelled(&mut k, &intent);
            let first = if tree_first {
                DomainEvent::ClosureTreeObserved {
                    domain: intent.id.clone(),
                    observation: closure_tree_observation(&intent, 5),
                    at: 5,
                }
            } else {
                DomainEvent::ClosureObserved {
                    domain: intent.id.clone(),
                    observation: closure_observation(&intent, 5),
                    at: 5,
                }
            };
            event(&mut k, "review-first", first);
            let stale = if tree_first {
                DomainEvent::ClosureObserved {
                    domain: intent.id.clone(),
                    observation: closure_observation(&intent, rejected_at),
                    at: rejected_at,
                }
            } else {
                DomainEvent::ClosureTreeObserved {
                    domain: intent.id.clone(),
                    observation: closure_tree_observation(&intent, rejected_at),
                    at: rejected_at,
                }
            };
            let before = k.clone();
            let operation = domain_operation(&k, "review-stale", Command::Domain(Box::new(stale)));
            assert!(k.admit(operation).is_err());
            assert_eq!(k.revision(), before.revision());
            assert!(k.domains() == before.domains());
            immutable_holds(k.domains().unwrap().domain(&intent.id).unwrap());
        }
    }
}

#[test]
fn review_recursive_contract_accepts_exact_bounds_and_rejects_nearest_neighbors() {
    let (_, intent) = coordinator();
    let mut tree = closure_tree_observation(&intent, 5);
    tree.nodes[1].members = (0..64)
        .map(|offset| DomainProcessObservation {
            pid: 910_000 + offset,
            start_ticks: 920_000 + u64::from(offset),
        })
        .collect();
    assert!(tree.validate().is_ok());
    let mut too_many_members = tree.clone();
    too_many_members.nodes[1]
        .members
        .push(DomainProcessObservation {
            pid: 910_064,
            start_ticks: 920_064,
        });
    assert!(too_many_members.validate().is_err());

    tree.nodes[1].members.clear();
    while tree.nodes.len() < 32 {
        let index = tree.nodes.len() as u64;
        tree.nodes.push(DomainCgroupObservation {
            relative_path: format!("child-{index}"),
            device: tree.nodes[0].device,
            inode: tree.nodes[0].inode + index,
            populated: false,
            members: vec![],
        });
    }
    assert!(tree.validate().is_ok());
    let mut too_many_nodes = tree.clone();
    too_many_nodes.nodes.push(DomainCgroupObservation {
        relative_path: "child-32".into(),
        device: tree.nodes[0].device,
        inode: tree.nodes[0].inode + 32,
        populated: false,
        members: vec![],
    });
    assert!(too_many_nodes.validate().is_err());

    tree.nodes.truncate(1);
    let mut path = String::new();
    for depth in 1..=8 {
        if !path.is_empty() {
            path.push('/');
        }
        path.push('a');
        tree.nodes.push(DomainCgroupObservation {
            relative_path: path.clone(),
            device: tree.nodes[0].device,
            inode: tree.nodes[0].inode + depth,
            populated: true,
            members: vec![],
        });
    }
    assert!(tree.validate().is_ok());
    let mut too_deep = tree.clone();
    too_deep.nodes.push(DomainCgroupObservation {
        relative_path: format!("{path}/a"),
        device: tree.nodes[0].device,
        inode: tree.nodes[0].inode + 9,
        populated: true,
        members: vec![],
    });
    assert!(too_deep.validate().is_err());
}
