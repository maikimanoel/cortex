//! Independently authored counterexamples for memory basis, scope and role contracts.
use cortex_core::program::{ProgramError, domain::PoolIdentity, memory::*, scheduling::Resource};

fn pool(incarnation: &str) -> PoolIdentity {
    PoolIdentity {
        physical: "synthetic-pool".into(),
        incarnation: incarnation.into(),
        host: "synthetic-host".into(),
        boot: "boot-1".into(),
    }
}
fn process(pid: u32) -> MemoryProcessIdentity {
    MemoryProcessIdentity {
        host: "synthetic-host".into(),
        boot: "boot-1".into(),
        pid,
        start_ticks: 100,
    }
}
fn allocation(basis: MemoryBasis, incarnation: &str) -> MemoryResource {
    MemoryResource::new(basis, MemoryScope::AccountingPool(pool(incarnation))).unwrap()
}
fn cgroup(inode: u64) -> MemoryResource {
    MemoryResource::new(
        MemoryBasis::CgroupCharged,
        MemoryScope::Cgroup {
            host: "synthetic-host".into(),
            boot: "boot-1".into(),
            manager: "manager-1".into(),
            invocation: "invocation-1".into(),
            path: "/owned/case".into(),
            device: 7,
            inode,
        },
    )
    .unwrap()
}

#[test]
fn identical_byte_amounts_do_not_make_resident_and_charged_accounting_interchangeable() {
    let resident = allocation(MemoryBasis::ProcessResident, "allocation-1");
    let charged = allocation(MemoryBasis::CgroupCharged, "allocation-1");
    let summed = allocation(MemoryBasis::SummedProcessResident, "allocation-1");
    assert_eq!(charged.contains(1024, &charged, 1024), Ok(true));
    assert_eq!(charged.contains(1024, &charged, 1025), Ok(false));
    assert_eq!(
        charged.contains(1024, &resident, 1024),
        Err(ProgramError::Structure)
    );
    assert_eq!(
        charged.checked_add(512, &summed, 512),
        Err(ProgramError::Structure)
    );
    assert_eq!(resident.checked_add(512, &resident, 512), Ok(1024));
    assert_eq!(
        charged.checked_add(u64::MAX, &charged, 1),
        Err(ProgramError::Capacity)
    );
    assert!(Resource::UnspecifiedMemoryBytes.memory_basis_unestablished());
    assert!(
        !Resource::memory(
            MemoryBasis::CgroupCharged,
            MemoryScope::AccountingPool(pool("allocation-1"))
        )
        .unwrap()
        .memory_basis_unestablished()
    );
}

#[test]
fn changed_pool_boot_or_process_birth_invalidates_correspondence() {
    let old = allocation(MemoryBasis::CgroupCharged, "allocation-1");
    let next = allocation(MemoryBasis::CgroupCharged, "allocation-2");
    assert_eq!(old.contains(1024, &next, 1), Err(ProgramError::Structure));
    let mut changed_boot = pool("allocation-1");
    changed_boot.boot = "boot-2".into();
    let changed = MemoryResource::new(
        MemoryBasis::CgroupCharged,
        MemoryScope::AccountingPool(changed_boot),
    )
    .unwrap();
    assert_eq!(
        old.checked_add(1, &changed, 1),
        Err(ProgramError::Structure)
    );
    let same_pid = process(10);
    let first = MemoryResource::new(
        MemoryBasis::ProcessResident,
        MemoryScope::Process(same_pid.clone()),
    )
    .unwrap();
    let second = MemoryResource::new(
        MemoryBasis::ProcessResident,
        MemoryScope::Process(MemoryProcessIdentity {
            start_ticks: 101,
            ..same_pid
        }),
    )
    .unwrap();
    assert_eq!(
        first.contains(100, &second, 1),
        Err(ProgramError::Structure)
    );
    assert_eq!(
        cgroup(9).contains(100, &cgroup(10), 1),
        Err(ProgramError::Structure)
    );
}

#[test]
fn observed_peaks_are_not_additive_allocations_even_in_identical_scope() {
    let scope = cgroup(9);
    assert_eq!(scope.contains(2048, &scope, 1024), Ok(true));
    assert_eq!(
        scope.checked_add(1024, &scope, 1024),
        Err(ProgramError::Structure)
    );
    let resident = MemoryResource::new(
        MemoryBasis::ProcessResident,
        MemoryScope::Process(process(10)),
    )
    .unwrap();
    assert_eq!(
        resident.checked_add(20, &resident, 20),
        Err(ProgramError::Structure)
    );
}

#[test]
fn process_sum_scope_rejects_duplicate_lifetimes_cross_boot_and_ambiguous_membership() {
    let valid = MemoryScope::ProcessSet(vec![process(10), process(11)]);
    assert!(MemoryResource::new(MemoryBasis::SummedProcessResident, valid.clone()).is_ok());
    assert!(MemoryResource::new(MemoryBasis::CgroupCharged, valid).is_err());
    for members in [
        vec![],
        vec![process(11), process(10)],
        vec![process(10), process(10)],
        vec![
            process(10),
            MemoryProcessIdentity {
                start_ticks: 101,
                ..process(10)
            },
        ],
        vec![
            process(10),
            MemoryProcessIdentity {
                boot: "boot-2".into(),
                ..process(11)
            },
        ],
    ] {
        assert!(
            MemoryResource::new(
                MemoryBasis::SummedProcessResident,
                MemoryScope::ProcessSet(members)
            )
            .is_err()
        );
    }
    assert!(
        MemoryResource::new(
            MemoryBasis::SummedProcessResident,
            MemoryScope::ProcessSet((1..=256).map(process).collect())
        )
        .is_ok()
    );
    assert!(
        MemoryResource::new(
            MemoryBasis::SummedProcessResident,
            MemoryScope::ProcessSet((1..=257).map(process).collect())
        )
        .is_err()
    );
}

#[test]
fn measurement_unknown_and_zero_are_distinct_from_requested_and_readback_limits() {
    let scope = cgroup(9);
    let unknown = MemoryObservation::new(MemoryCorrespondence::Scoped(scope.clone()), None);
    let zero = MemoryObservation::new(MemoryCorrespondence::Scoped(scope.clone()), Some(0));
    assert_eq!(unknown.bytes(), None);
    assert_eq!(zero.bytes(), Some(0));
    assert_ne!(unknown, zero);
    let missing_scope = MemoryObservation::new(
        MemoryCorrespondence::ScopeUnestablished(MemoryBasis::CgroupCharged),
        Some(0),
    );
    assert_eq!(missing_scope.basis(), MemoryBasis::CgroupCharged);
    assert!(missing_scope.resource().is_none());
    assert_eq!(missing_scope.bytes(), Some(0));
    let requested = MemoryLimit::new(
        MemoryCorrespondence::Scoped(scope.clone()),
        4096,
        MemoryLimitKind::Requested,
    )
    .unwrap();
    let readback = MemoryLimit::new(
        MemoryCorrespondence::Scoped(scope.clone()),
        4096,
        MemoryLimitKind::ReadBack,
    )
    .unwrap();
    assert_eq!(requested.bytes(), readback.bytes());
    assert_ne!(requested.kind(), readback.kind());
    assert_eq!(readback.basis(), scope.basis());
    assert_eq!(readback.resource(), Some(&scope));
    assert!(
        MemoryLimit::new(
            MemoryCorrespondence::Scoped(scope),
            0,
            MemoryLimitKind::Requested
        )
        .is_err()
    );
}

#[test]
fn resource_debug_does_not_disclose_new_identity_payloads() {
    let secret = "synthetic-private-identity-canary";
    let scoped = Resource::memory(
        MemoryBasis::CgroupCharged,
        MemoryScope::Cgroup {
            host: secret.into(),
            boot: secret.into(),
            manager: secret.into(),
            invocation: secret.into(),
            path: format!("/owned/{secret}"),
            device: 7,
            inode: 9,
        },
    )
    .unwrap();
    assert!(!format!("{scoped:?}").contains(secret));
    assert!(format!("{scoped:?}").contains("CgroupCharged"));
}
