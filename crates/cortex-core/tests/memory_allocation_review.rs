//! Author-separated allocation counterexamples: observation scopes are not pools.
#[path = "support/program.rs"]
mod fixture;
#[path = "support/supervision.rs"]
mod supervision;

use cortex_core::program::{
    ProgramState,
    domain::PoolIdentity,
    memory::{MemoryBasis, MemoryProcessIdentity, MemoryScope},
    operational::{EffectEvent, OperationalProjection},
    scheduling::{Capacity, Demand, Proposal, Resource, WaitReason, simulate},
};

fn allocation() -> Resource {
    Resource::memory(
        MemoryBasis::CgroupCharged,
        MemoryScope::AccountingPool(PoolIdentity {
            physical: "synthetic-pool".into(),
            incarnation: "allocation-1".into(),
            host: "host-1".into(),
            boot: "boot-1".into(),
        }),
    )
    .unwrap()
}
fn observation_dimensions() -> Vec<Resource> {
    let process = MemoryProcessIdentity {
        host: "host-1".into(),
        boot: "boot-1".into(),
        pid: 10,
        start_ticks: 20,
    };
    [
        (
            MemoryBasis::ProcessResident,
            MemoryScope::Process(process.clone()),
        ),
        (
            MemoryBasis::SummedProcessResident,
            MemoryScope::ProcessSet(vec![process]),
        ),
        (
            MemoryBasis::CgroupCharged,
            MemoryScope::Cgroup {
                host: "host-1".into(),
                boot: "boot-1".into(),
                manager: "manager-1".into(),
                invocation: "invocation-1".into(),
                path: "/synthetic/owned".into(),
                device: 1,
                inode: 2,
            },
        ),
    ]
    .into_iter()
    .map(|(basis, scope)| Resource::memory(basis, scope).unwrap())
    .collect()
}
fn capacity(resource: Resource) -> Capacity {
    Capacity {
        resource,
        available: Some(100),
        verification_reserve: 10,
        declared_enforceable: true,
    }
}
fn proposal(resource: Resource) -> Proposal {
    Proposal {
        task: fixture::task("A"),
        band: 0,
        waiting_rounds: 0,
        risk_rank: 0,
        cost_rank: 0,
        demand: vec![Demand {
            resource,
            amount: Some(50),
        }],
    }
}

#[test]
fn policies_accept_accounting_pools_and_reject_all_observation_scope_shapes() {
    let state = ProgramState::new(fixture::definition()).unwrap();
    let mut positive = supervision::policy();
    positive.capacities.push(capacity(allocation()));
    let mut projection = OperationalProjection::new_supervised_fixture();
    assert!(
        projection
            .apply(&state, EffectEvent::Policy(positive))
            .is_ok()
    );
    for resource in observation_dimensions() {
        let mut policy = supervision::policy();
        policy.capacities.push(capacity(resource));
        let mut projection = OperationalProjection::new_supervised_fixture();
        assert!(
            projection
                .apply(&state, EffectEvent::Policy(policy))
                .is_err()
        );
        assert!(projection.policies().is_empty(), "refusal must be atomic");
    }
}

#[test]
fn scheduling_rejects_observation_scopes_in_capacity_and_in_demand() {
    let mut state = ProgramState::new(fixture::definition()).unwrap();
    fixture::initialize(&mut state);
    let accepted = simulate(&state, &[proposal(allocation())], &[capacity(allocation())]).unwrap();
    assert!(accepted[0].proposed);
    assert!(accepted[0].held.is_empty());
    for resource in observation_dimensions() {
        assert!(
            simulate(
                &state,
                &[proposal(resource.clone())],
                &[capacity(resource.clone())]
            )
            .is_err()
        );
        assert!(simulate(&state, &[proposal(resource)], &[capacity(allocation())]).is_err());
    }
}

#[test]
fn unspecified_legacy_memory_is_a_hold_even_with_known_positive_byte_capacity() {
    let mut state = ProgramState::new(fixture::definition()).unwrap();
    fixture::initialize(&mut state);
    let result = simulate(
        &state,
        &[proposal(Resource::UnspecifiedMemoryBytes)],
        &[capacity(Resource::UnspecifiedMemoryBytes)],
    )
    .unwrap();
    assert!(!result[0].proposed);
    assert!(
        result[0]
            .held
            .contains(&WaitReason::Unknown(Resource::UnspecifiedMemoryBytes))
    );
}
