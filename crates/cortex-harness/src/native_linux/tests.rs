use super::*;
use std::{
    os::unix::fs::{PermissionsExt, symlink},
    sync::atomic::{AtomicU64, Ordering},
};
static SEQ: AtomicU64 = AtomicU64::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "cxn-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
    p
}
#[test]
fn native_policy_refuses_missing_budgets_symlink_nonempty_and_changed_root() {
    let p = root();
    assert!(NativeIsolation::inspect(&p, 0, 10).is_err());
    assert!(NativeIsolation::inspect(&p, 4096, 0).is_err());
    let policy = NativeIsolation::inspect(&p, 4096, 16).unwrap();
    fs::write(p.join("unexpected"), b"owned fixture").unwrap();
    assert!(policy.check().is_err());
    fs::remove_file(p.join("unexpected")).unwrap();
    let link = p.with_extension("link");
    symlink(&p, &link).unwrap();
    assert!(NativeIsolation::inspect(&link, 4096, 16).is_err());
    fs::remove_file(link).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(policy.check().is_err());
    fs::remove_dir(p).unwrap();
}
#[test]
fn every_policy_input_changes_the_digest_and_metadata_is_not_enforcement() {
    let a = root();
    let b = root();
    let p = NativeIsolation::inspect(&a, 4096, 16).unwrap();
    for other in [
        NativeIsolation::inspect(&a, 8192, 16).unwrap(),
        NativeIsolation::inspect(&a, 4096, 32).unwrap(),
        NativeIsolation::inspect(&b, 4096, 16).unwrap(),
    ] {
        assert_ne!(p.digest(), other.digest());
    }
    assert_eq!(
        NativeIsolation::planned_support(&Control::Filesystem),
        Support::Unknown
    );
    assert_eq!(
        NativeIsolation::planned_support(&Control::Network),
        Support::Unknown
    );
    assert_eq!(
        NativeIsolation::planned_support(&Control::SessionContinuation),
        Support::Unsupported
    );
    fs::remove_dir(a).unwrap();
    fs::remove_dir(b).unwrap();
}
#[test]
fn stable_process_refuses_wrong_start_identity_and_keeps_real_handle() {
    let pid = std::process::id();
    let ticks = crate::domain_bootstrap::ticks(pid).unwrap();
    assert!(StableProcess::acquire(pid, ticks + 1).is_err());
    let live = StableProcess::acquire(pid, ticks).unwrap();
    live.require_live().unwrap();
    assert!(StableProcess::acquire(u32::MAX, ticks).is_err());
}
#[test]
fn namespace_setup_is_not_callable_in_multithreaded_test_controller() {
    let p = root();
    let policy = NativeIsolation::inspect(&p, 4096, 16).unwrap();
    let failure = install(&policy, -1).unwrap_err();
    assert_eq!(failure.phase, IsolationPhase::Policy);
    // No namespace operation was reached; the owned root is still empty.
    assert!(fs::read_dir(&p).unwrap().next().is_none());
    fs::remove_dir(p).unwrap();
}

#[test]
fn readiness_projects_only_installed_controls_and_host_or_policy_changes_invalidate_it() {
    use cortex_core::{program::executor::*, temporal::SourceReference};
    let p = root();
    let policy = NativeIsolation::inspect(&p, 4096, 16).unwrap();
    let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap();
    let ready = NativeReadiness::observed(&policy, boot.trim());
    let basis =
        SourceReference::operator_reported("fixture", "1", "native-controlled-test").unwrap();
    let identity =
        ExecutorIdentity::new("native-fixed-fixture", "1", basis.clone(), basis.clone()).unwrap();
    let descriptor = ExecutorDescriptor::new(
        basis.clone(),
        identity,
        Transport::StandardInputJsonLines,
        ready.observations(&basis).unwrap(),
        8,
    )
    .unwrap();
    let requested = |control, kind| {
        Requirements::new(basis.clone(), vec![Requirement { control, kind }], 8).unwrap()
    };
    assert!(
        negotiate(
            &descriptor,
            &requested(Control::Filesystem, ObservationKind::ControlledFixture)
        )[0]
        .held
        .is_empty()
    );
    assert!(
        !negotiate(
            &descriptor,
            &requested(
                Control::Filesystem,
                ObservationKind::HostQualificationReport
            )
        )[0]
        .held
        .is_empty()
    );
    assert_eq!(
        negotiate(
            &descriptor,
            &requested(Control::Cancellation, ObservationKind::ControlledFixture)
        )[0]
        .held,
        vec![HoldReason::Missing]
    );
    assert!(
        NativeReadiness::observed(&policy, "different-boot")
            .check_context()
            .is_err()
    );
    fs::write(p.join("changed"), b"owned").unwrap();
    assert!(ready.observations(&basis).is_err());
    fs::remove_file(p.join("changed")).unwrap();
    fs::remove_dir(p).unwrap();
}
