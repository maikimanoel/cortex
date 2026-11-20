//! Source-level reservation and runtime dispatch tests. No systemd or mount effects.
use super::*;
use crate::native_execution::{ExecutionError, managed_backend::check_scratch_reservation};
use cortex_core::program::scheduling::Resource;

fn reservation(disk_bytes: u64, capacity: u64) -> (DomainProjection, DomainIntent) {
    let (_, mut intent) = fixture::coordinator();
    let mut pool = fixture::pool();
    pool.capacities
        .iter_mut()
        .find(|item| item.resource == Resource::DiskBytes)
        .unwrap()
        .available = Some(capacity);
    intent
        .effect
        .demand
        .iter_mut()
        .find(|item| item.resource == Resource::DiskBytes)
        .unwrap()
        .amount = Some(disk_bytes);
    let mut domains = DomainProjection::default();
    domains.apply(DomainEvent::ConfigurePool(pool)).unwrap();
    domains
        .apply(DomainEvent::Reserve {
            intent: intent.clone(),
            at: 1,
        })
        .unwrap();
    (domains, intent)
}

#[test]
fn scratch_quota_cannot_exceed_committed_disk_reservation() {
    let (domains, intent) = reservation(4096, 16384);
    assert_eq!(domains.held(Resource::DiskBytes).unwrap(), 4096);
    assert_eq!(
        check_scratch_reservation(&domains, &intent, 8192),
        Err(ExecutionError::Policy),
    );
}

#[test]
fn scratch_guard_uses_inclusive_byte_bound_without_overflow() {
    let highest_aligned = u64::MAX - 4095;
    for (quota, held, expected) in [
        (8192, 8191, Err(ExecutionError::Policy)),
        (8192, 8192, Ok(())),
        (8192, 12288, Ok(())),
        (
            highest_aligned,
            highest_aligned - 1,
            Err(ExecutionError::Policy),
        ),
        (highest_aligned, highest_aligned, Ok(())),
        (highest_aligned, u64::MAX, Ok(())),
        (0, 8192, Err(ExecutionError::Policy)),
    ] {
        let (domains, intent) = reservation(held, u64::MAX);
        assert_eq!(
            check_scratch_reservation(&domains, &intent, quota),
            expected,
            "quota {quota}, hold {held}",
        );
    }
}

#[test]
fn scratch_guard_refuses_missing_disk_and_core_refuses_invalid_amount_or_pool() {
    let (_, mut intent) = fixture::coordinator();
    let mut pool = fixture::pool();
    pool.capacities
        .retain(|item| item.resource != Resource::DiskBytes);
    let mut without_disk = DomainProjection::default();
    without_disk
        .apply(DomainEvent::ConfigurePool(pool))
        .unwrap();
    let reserve = |intent| DomainEvent::Reserve { intent, at: 1 };
    assert_eq!(
        without_disk.apply(reserve(intent.clone())),
        Err(DomainError::Structure),
    );

    let disk = intent
        .effect
        .demand
        .iter_mut()
        .find(|item| item.resource == Resource::DiskBytes)
        .unwrap();
    disk.amount = None;
    let mut with_disk = DomainProjection::default();
    with_disk
        .apply(DomainEvent::ConfigurePool(fixture::pool()))
        .unwrap();
    assert_eq!(
        with_disk.apply(reserve(intent.clone())),
        Err(DomainError::Structure)
    );
    intent
        .effect
        .demand
        .iter_mut()
        .find(|item| item.resource == Resource::DiskBytes)
        .unwrap()
        .amount = Some(0);
    assert_eq!(
        with_disk.apply(reserve(intent.clone())),
        Err(DomainError::Structure)
    );

    intent
        .effect
        .demand
        .retain(|item| item.resource != Resource::DiskBytes);
    assert_eq!(
        with_disk.apply(reserve(intent.clone())),
        Err(DomainError::Capacity(Resource::DiskBytes)),
    );
    without_disk.apply(reserve(intent.clone())).unwrap();
    assert_eq!(
        check_scratch_reservation(&without_disk, &intent, 8192),
        Err(ExecutionError::Policy)
    );
}

#[test]
fn scratch_guard_refuses_uncommitted_intent_and_released_hold() {
    let (mut domains, intent) = reservation(8192, 16384);
    let mut other = intent.clone();
    other.configuration = fixture::source("changed-executable-policy");
    assert_eq!(
        check_scratch_reservation(&domains, &other, 8192),
        Err(ExecutionError::Authority),
    );
    other = intent.clone();
    other.id = DomainId::new("another-domain").unwrap();
    assert_eq!(
        check_scratch_reservation(&domains, &other, 8192),
        Err(ExecutionError::Authority),
    );

    domains
        .apply(DomainEvent::Cancel {
            domain: intent.id.clone(),
            at: 2,
            basis: fixture::source("cancelled-before-create"),
        })
        .unwrap();
    assert_eq!(domains.held(Resource::DiskBytes).unwrap(), 0);
    assert_eq!(
        check_scratch_reservation(&domains, &intent, 8192),
        Err(ExecutionError::Policy),
    );
}

#[test]
fn scratch_underhold_refuses_before_create_dispatch() {
    let root = Temp::new();
    let (store, intent, ids) = stored(&root);
    let before = store.load(&key(), Access::ReadOnly).unwrap();
    let mut manager = FakeManager::new(&root);
    manager.selected_scratch = Some(|_| 8192);
    let run = prepare_only(&store, &mut manager, &ids).run;
    assert_eq!(run.diagnostic.unwrap().cause, DomainCause::Correspondence);
    assert_eq!(manager.created, 0);
    assert!(!manager.endpoint.exists());
    assert!(run.creation.is_none() && run.release.is_none() && run.report.is_none());
    let after = store.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(after.revision(), before.revision());
    let domain = after.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(domain.phase, DomainPhase::Reserved);
    assert!(domain.holds(Resource::DiskBytes));
    assert!(domain.closure.is_none() && domain.settlements.is_empty());
}

#[test]
fn scratch_refusal_inside_locked_release_sends_no_frame_or_report() {
    let root = Temp::new();
    let (store, intent, ids) = stored(&root);
    let mut manager = FakeManager::new(&root);
    // Use the fixture's 32-byte synthetic hold to allow preparation. Inject a
    // mismatched quota only at the committed release cut. Real managed policies
    // are immutable and additionally checked against the pinned configuration.
    manager.selected_scratch = Some(|phase| match phase {
        DomainPhase::ReleaseUncertain => 8192,
        _ => 32,
    });
    let held = prepare_only(&store, &mut manager, &ids).prepared.unwrap();
    let run = release(&store, &key(), &mut manager, held, &AtomicBool::new(false));
    let bootstrap = manager.worker.take().unwrap().join().unwrap().unwrap_err();
    assert_eq!(bootstrap.kind(), io::ErrorKind::UnexpectedEof);
    assert_eq!(run.diagnostic.unwrap().cause, DomainCause::Correspondence);
    assert!(run.release.is_some() && run.nonce.is_none() && run.report.is_none());

    let after = store.load(&key(), Access::ReadOnly).unwrap();
    let domain = after.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(domain.phase, DomainPhase::ReleaseUncertain);
    assert_eq!(domain.outcome, HistoricalOutcome::Unknown);
    assert!(domain.holds(Resource::DiskBytes));
    assert!(domain.closure.is_none() && domain.settlements.is_empty());
    let replay = prepare_only(&store, &mut manager, &ids);
    assert!(replay.run.recovery_only && replay.prepared.is_none());
    assert_eq!(manager.created, 1);
    assert_eq!(
        store.load(&key(), Access::ReadOnly).unwrap().revision(),
        after.revision()
    );
}

#[test]
fn scratch_refusal_inside_locked_create_never_dispatches_creation() {
    let root = Temp::new();
    let (store, intent, ids) = stored(&root);
    let mut manager = FakeManager::new(&root);
    // Pass the numeric check on the Reserved record, then inject the mismatch
    // only when the callback receives the committed CreateRequested record.
    manager.selected_scratch = Some(|phase| match phase {
        DomainPhase::CreateUncertain => 8192,
        _ => 32,
    });
    let run = prepare_only(&store, &mut manager, &ids).run;
    assert_eq!(run.diagnostic.unwrap().cause, DomainCause::Correspondence);
    assert_eq!(manager.created, 0);
    assert!(!manager.endpoint.exists());
    assert!(run.creation.is_some() && run.release.is_none() && run.report.is_none());

    let after = store.load(&key(), Access::ReadOnly).unwrap();
    let domain = after.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(domain.phase, DomainPhase::CreateUncertain);
    assert_eq!(domain.outcome, HistoricalOutcome::Unknown);
    assert!(domain.holds(Resource::DiskBytes));
    assert!(domain.closure.is_none() && domain.settlements.is_empty());
    let replay = prepare_only(&store, &mut manager, &ids);
    assert!(replay.run.recovery_only && replay.prepared.is_none());
    assert_eq!(manager.created, 0);
    assert_eq!(
        store.load(&key(), Access::ReadOnly).unwrap().revision(),
        after.revision()
    );
}
