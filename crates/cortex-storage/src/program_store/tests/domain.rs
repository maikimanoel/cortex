//! Encrypted replay and concurrent admission for the shared pool coordinator.
use super::fixture as program_fixture;
use super::*;
#[path = "../../../../cortex-evidence/tests/support/domain.rs"]
mod domain_fixture;
use cortex_core::binding::WorktreeRequirement;
use cortex_core::program::domain::*;
use cortex_core::program::scheduling::Resource;
use cortex_evidence::program::Command as DomainInput;
use std::sync::{Arc, Barrier};
fn stored(k: &CoordinationKernel, f: &Temp) -> ProgramStore {
    let s = ProgramStore::create_domain_fixture(&f.store(), &key(), k.initial().clone(), limits())
        .unwrap();
    for o in k.operations() {
        s.admit(&key(), o.clone()).unwrap();
    }
    s
}
fn op(k: &CoordinationKernel, id: &str, e: DomainEvent) -> Operation {
    domain_fixture::domain_operation(k, id, DomainInput::Domain(Box::new(e)))
}
#[test]
fn domain_profile_roundtrip_preserves_history_debt_and_legacy_refusal() {
    let f = Temp::new();
    let (mut k, i) = domain_fixture::coordinator();
    domain_fixture::bound(&mut k, &i);
    domain_fixture::event(
        &mut k,
        "closed",
        DomainEvent::Closed {
            receipt: domain_fixture::closure(&i, None),
            at: 4,
        },
    );
    let store = stored(&k, &f);
    let open = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    assert_eq!(open.profile(), ProgramProfile::RecoverableDomainsV4);
    let actual = open.load(&key(), Access::ReadOnly).unwrap();
    assert!(actual.domains() == k.domains());
    let r = actual.domains().unwrap().domain(&i.id).unwrap();
    assert_eq!(r.outcome, HistoricalOutcome::Unknown);
    assert!(!r.holds(Resource::Workers));
    assert!(r.holds(Resource::DiskBytes));
    assert!(r.unresolved_consumption(Consumption::CpuExecutionMillis));
    let bytes = fs::read(f.store().join("program.db")).unwrap();
    assert!(
        !bytes
            .windows(b"domain-fixture".len())
            .any(|x| x == b"domain-fixture")
    );
    let old = Temp::new();
    let v3 = ProgramStore::create_supervised_fixture(
        &old.store(),
        &key(),
        k.initial().clone(),
        limits(),
    )
    .unwrap();
    let before = snapshot(&old);
    let failure = v3.admit(&key(), k.operations()[0].clone()).unwrap_err();
    assert!(matches!(failure.error, ProgramStoreError::Unsupported));
    assert_eq!(snapshot(&old), before);
    let bad = op(
        &actual,
        "duplicate-close",
        DomainEvent::Closed {
            receipt: domain_fixture::closure(&i, None),
            at: 5,
        },
    );
    assert!(store.admit(&key(), bad).is_err());
    assert_eq!(
        store.load(&key(), Access::ReadOnly).unwrap().revision(),
        actual.revision()
    );
}
#[test]
fn pool_admission_competing_clients_cannot_double_allocate() {
    let f = Temp::new();
    let (mut k, i) = domain_fixture::coordinator();
    let j = domain_fixture::add_member(&mut k, "other-program");
    let s = stored(&k, &f);
    let one = op(&k, "one", DomainEvent::Reserve { intent: i, at: 1 });
    let two = op(&k, "two", DomainEvent::Reserve { intent: j, at: 1 });
    let b = Arc::new(Barrier::new(3));
    let handles = [one, two]
        .into_iter()
        .map(|o| {
            let b = b.clone();
            let path = f.store();
            std::thread::spawn(move || {
                let store = ProgramStore::open(&path, &key(), Access::ReadOnly, limits()).unwrap();
                b.wait();
                store.admit(&key(), o).is_ok()
            })
        })
        .collect::<Vec<_>>();
    b.wait();
    let winners = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .filter(|x| *x)
        .count();
    assert_eq!(winners, 1);
    let k = s.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(k.domains().unwrap().held(Resource::Workers), Ok(1));
    assert_eq!(k.domains().unwrap().domains().len(), 1);
}
#[test]
fn lost_manager_reply_and_replay_never_repeat_creation_callback() {
    let f = Temp::new();
    let (mut k, i) = domain_fixture::coordinator();
    domain_fixture::event(
        &mut k,
        "reserve",
        DomainEvent::Reserve {
            intent: i.clone(),
            at: 1,
        },
    );
    let s = stored(&k, &f);
    let creation = op(
        &k,
        "create",
        DomainEvent::CreateRequested {
            domain: i.id.clone(),
            at: 2,
        },
    );
    let lost = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        s.dispatch_domain(&key(), creation.clone(), |_| {
            panic!("controlled lost manager reply")
        })
    }));
    assert!(lost.is_err());
    let reopen = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    let k = reopen.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(
        k.domains().unwrap().domain(&i.id).unwrap().phase,
        DomainPhase::CreateUncertain
    );
    let (r, called) = reopen
        .dispatch_domain(&key(), creation, |_| panic!("duplicate callback"))
        .unwrap();
    assert!(r.duplicate);
    assert!(called.is_none());
}
#[test]
fn closure_before_after_commit_faults_reconstruct_without_reexecution() {
    for after in [false, true] {
        let f = Temp::new();
        let (mut k, i) = domain_fixture::coordinator();
        domain_fixture::bound(&mut k, &i);
        let s = stored(&k, &f);
        let input = op(
            &k,
            "closure",
            DomainEvent::Closed {
                receipt: domain_fixture::closure(&i, Some(7)),
                at: 4,
            },
        );
        let cut = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            s.admit_observed(&key(), input.clone(), |stage, _| {
                if matches!(
                    (after, stage),
                    (false, Stage::BeforeCommit) | (true, Stage::AfterCommit)
                ) {
                    panic!("controlled publication cut")
                }
            })
        }));
        assert!(cut.is_err());
        let current = s.load(&key(), Access::ReadOnly).unwrap();
        assert_eq!(
            current.domains().unwrap().domain(&i.id).unwrap().phase,
            if after {
                DomainPhase::Closed
            } else {
                DomainPhase::Bound
            }
        );
        let receipt = s.admit(&key(), input.clone()).unwrap();
        assert_eq!(receipt.duplicate, after);
        let current = s.load(&key(), Access::ReadOnly).unwrap();
        let record = current.domains().unwrap().domain(&i.id).unwrap();
        assert_eq!(record.outcome, HistoricalOutcome::Unknown);
        assert_eq!(current.domains().unwrap().held(Resource::Workers), Ok(0));
        assert_eq!(
            current
                .domains()
                .unwrap()
                .consumed_or_reserved(Consumption::CpuExecutionMillis),
            Ok(7)
        );
        assert!(s.admit(&key(), input).unwrap().duplicate);
    }
}
#[test]
fn member_source_roots_cannot_contain_private_journal_and_nested_inputs_refuse_early() {
    let f = Temp::new();
    let (k, _) = domain_fixture::coordinator();
    let s = stored(&k, &f);
    let before = snapshot(&f);
    let unsafe_definition =
        with_contracts(&domain_fixture::named_definition("unsafe-member"), |c| {
            let root = f.0.clone();
            CandidateBindingContract::new(
                c.task().clone(),
                c.revision(),
                CandidateRequirement::new(
                    RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git"))
                        .unwrap(),
                    c.candidate().commit().clone(),
                    WorktreeRequirement::Clean,
                ),
            )
        });
    let input = domain_fixture::domain_operation(
        &k,
        "register-private",
        DomainInput::RegisterDomainMember(Box::new(unsafe_definition)),
    );
    assert!(s.admit(&key(), input).is_err());
    assert_eq!(snapshot(&f), before);
    let nested = domain_fixture::domain_operation(
        &k,
        "nested",
        DomainInput::RegisterDomainMember(Box::new(domain_fixture::named_definition("deep"))),
    );
    let outer = domain_fixture::domain_operation(
        &k,
        "outer",
        DomainInput::DomainMember {
            program: k.initial().id().clone(),
            operation: Box::new(nested),
        },
    );
    assert!(matches!(
        s.admit(&key(), outer).unwrap_err().error,
        ProgramStoreError::Unsupported
    ));
    assert_eq!(snapshot(&f), before);
}
#[test]
fn nested_member_amendment_checks_private_source_roots_and_allows_disjoint_neighbor() {
    let f = Temp::new();
    let (mut k, _) = domain_fixture::coordinator();
    let i = domain_fixture::add_member(&mut k, "member");
    let s = stored(&k, &f);
    let before = snapshot(&f);
    for contained in [true, false] {
        let revision = definition_revision(2, false);
        let revision = ProgramDefinition::new(
            i.effect.scope.program.clone(),
            revision.outcome().clone(),
            revision.revision(),
            revision.predecessor(),
            revision.approval().clone(),
            revision.graph().clone(),
            revision.workstreams().to_vec(),
            revision.increments().to_vec(),
            revision.limits(),
        )
        .unwrap();
        let root = if contained {
            f.0.clone()
        } else {
            f.0.join("separate-source")
        };
        let next = with_contracts(&revision, |c| {
            CandidateBindingContract::new(
                c.task().clone(),
                rev(2),
                CandidateRequirement::new(
                    RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git"))
                        .unwrap(),
                    c.candidate().commit().clone(),
                    c.candidate().worktree(),
                ),
            )
        });
        let member = k.domain_member(&i.effect.scope.program).unwrap();
        let input = domain_fixture::domain_operation(
            member,
            "amend-member",
            DomainInput::Amend {
                next: Box::new(next),
                non_impact: cortex_evidence::program::NonImpact {
                    missions: vec![],
                    basis: source("all-contracts-changed"),
                },
            },
        );
        let outer = domain_fixture::domain_operation(
            &k,
            "outer-amend",
            DomainInput::DomainMember {
                program: i.effect.scope.program.clone(),
                operation: Box::new(input),
            },
        );
        if contained {
            let failure = s.admit(&key(), outer).unwrap_err();
            assert!(
                matches!(failure.error,ProgramStoreError::Storage(e)if e.kind()==ErrorKind::InvalidLocation)
            );
            assert_eq!(snapshot(&f), before);
        } else {
            s.admit(&key(), outer).unwrap();
        }
    }
    assert_eq!(
        s.load(&key(), Access::ReadOnly)
            .unwrap()
            .domain_member(&i.effect.scope.program)
            .unwrap()
            .state()
            .definition()
            .revision(),
        rev(2)
    );
}
