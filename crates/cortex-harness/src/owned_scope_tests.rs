use super::*;
use cortex_core::{program::OperationId, temporal::SourceReference};
use std::{
    os::unix::fs::{PermissionsExt, symlink},
    sync::atomic::{AtomicU64, Ordering},
};

fn source(id: &str) -> SourceReference {
    SourceReference::operator_reported("scope-test", id, "1").unwrap()
}
fn process() -> ProcessSpec {
    ProcessSpec {
        reference: source("process"),
        executable: "/owned/fixture".into(),
        executable_identity: source("fixture"),
        cwd: "/owned".into(),
        args: vec!["scope".into(), "literal;$(not-a-shell)".into()],
        prepared_input: OperationId::new("input").unwrap(),
        input_len: 1,
        host: source("host"),
        profile: source("profile"),
        privacy: source("privacy"),
        approval: source("approval"),
        limits: ProcessLimits {
            wall_ms: 5000,
            cleanup_ms: 1000,
            input_bytes: 16384,
            output_bytes: 81920,
            frame_bytes: 4096,
            authority_poll_ms: 10,
            executable_bytes: 16000000,
            arguments: 2,
            argument_bytes: 100,
            transport: None,
        },
        identity_checkpoint: None,
        containment: Some(OwnedScopeSpec {
            reference: source("scope"),
            unit: "cortex-qualification-test.scope".into(),
            delegated_root: "/sys/fs/cgroup/user.slice/user-42.slice/user@42.service".into(),
            runtime_dir: "/run/user/42".into(),
            launcher: "/usr/bin/systemd-run".into(),
            launcher_identity: source("launcher"),
            manager: "/usr/bin/systemctl".into(),
            manager_identity: source("manager"),
            controls: ScopeControls {
                cpu_quota_us: 100000,
                cpu_period_us: 100000,
                memory_bytes: 536870912,
                swap_bytes: 0,
                tasks_max: 8,
            },
            cpu_time_ms: 2000,
            cancellation_grace_ms: 500,
            escalation_ms: 500,
            isolation: ScopeIsolation::ScopeOnlyInertFixture,
        }),
    }
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "cortex-scope-metadata-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn exact_scope_command_carries_limits_without_shell_or_implicit_profile() {
    let p = process();
    let args = scope_arguments(&p).unwrap();
    for expected in [
        "--scope",
        "--slice=-.slice",
        "--expand-environment=no",
        "--property=CPUQuota=100.00%",
        "--property=CPUQuotaPeriodSec=100000us",
        "--property=MemoryMax=536870912",
        "--property=MemorySwapMax=0",
        "--property=TasksMax=8",
        "--property=RuntimeMaxSec=5000ms",
        "literal;$(not-a-shell)",
    ] {
        assert!(
            args.iter().any(|s| s == expected),
            "missing explicit argument: {expected}"
        );
    }
    assert_eq!(
        &args[args.len() - 3..],
        ["/owned/fixture", "scope", "literal;$(not-a-shell)"]
    );
    let s = p.containment.unwrap();
    let command = manager_command(&s.launcher, &s.runtime_dir);
    let env: Vec<_> = command.get_envs().collect();
    assert_eq!(env.len(), 2);
    assert!(env.iter().all(|(key, value)| (*key == "XDG_RUNTIME_DIR"
        || *key == "DBUS_SESSION_BUS_ADDRESS")
        && value.is_some()));
}

#[test]
fn quota_serialization_preserves_exact_supported_precision_and_multicore_ratios() {
    for (quota, period, expected) in [
        (100_000, 100_000, "100.00%"),
        (17_500, 100_000, "17.50%"),
        (100_010, 100_000, "100.01%"),
        (250_000, 100_000, "250.00%"),
        (1, 10_000, "0.01%"),
        (i32::MAX as u64, 10_000, "21474836.47%"),
        (u64::MAX, u64::MAX, "100.00%"),
    ] {
        assert_eq!(quota_percent(quota, period).unwrap(), expected);
    }
}

#[test]
fn quota_serialization_refuses_rounding_zero_and_interface_overflow() {
    for (quota, period) in [
        (0, 100_000),
        (1, 0),
        (100_001, 100_000),
        (1, 3),
        (1, 100_001),
        (i32::MAX as u64 + 1, 10_000),
        (u64::MAX, 1),
    ] {
        assert!(quota_percent(quota, period).is_err());
    }
    let mut p = process();
    p.containment.as_mut().unwrap().controls.cpu_quota_us = 100_001;
    assert!(scope_arguments(&p).is_err());
}

#[test]
fn namespace_profile_has_no_try_fallback_and_keeps_root_readonly() {
    let mut p = process();
    p.containment.as_mut().unwrap().isolation = ScopeIsolation::RootlessSyntheticCanary {
        bubblewrap: "/usr/bin/bwrap".into(),
        bubblewrap_identity: source("bwrap"),
        scratch_bytes: 16777216,
        mounts: vec![ReadOnlyMount {
            source: "/owned/allowed".into(),
            destination: "/allowed".into(),
        }],
    };
    let args = scope_arguments(&p).unwrap();
    for flag in [
        "--unshare-user",
        "--unshare-pid",
        "--unshare-net",
        "--disable-userns",
        "--clearenv",
        "--die-with-parent",
        "--new-session",
    ] {
        assert!(args.iter().any(|s| s == flag));
    }
    assert!(
        !args
            .iter()
            .any(|s| s.ends_with("-try") || s == "--unshare-all")
    );
    let scratch = args.iter().position(|s| s == "--tmpfs").unwrap();
    let readonly = args.iter().position(|s| s == "--remount-ro").unwrap();
    assert!(readonly > scratch);
    assert_eq!(args[readonly + 1], "/");
}

#[test]
fn foreign_names_ambiguous_paths_and_mount_overlay_conflicts_are_refused() {
    for unit in [
        "user@42.service",
        "cortex-qualification-.scope",
        "cortex-qualification-UPPER.scope",
        "cortex-qualification-a/b.scope",
        "cortex-qualification-a.scope\n",
    ] {
        assert!(validate_unit(unit).is_err());
    }
    assert!(validate_unit("cortex-qualification-case-3.scope").is_ok());
    for group in ["relative", "/", "/x/../y", "/x\ny"] {
        assert!(cgroup_path(group).is_err());
    }
    assert_eq!(
        cgroup_path("/owned/scope").unwrap(),
        Path::new("/sys/fs/cgroup/owned/scope")
    );
    let mount = |destination: &str| ReadOnlyMount {
        source: "/owned/file".into(),
        destination: destination.into(),
    };
    assert!(validate_mounts(&[mount("/lib/a"), mount("/lib/a/b")]).is_err());
    for forbidden in [
        "/",
        "/fixture",
        "/scratch/a",
        "/proc/a",
        "/sys/a",
        "/run/bus",
        "/home/user",
    ] {
        assert!(validate_mounts(&[mount(forbidden)]).is_err());
    }
    assert!(validate_mounts(&[mount("/lib/a"), mount("/lib/b")]).is_ok());
}

#[test]
fn duplicated_or_malformed_control_observations_never_become_success() {
    assert!(properties("LoadState=loaded\nLoadState=not-found\n").is_err());
    assert!(properties("unexpected text\n").is_err());
    assert!(properties("InvocationID=bad\rvalue\n").is_err());
    assert_eq!(
        properties("ControlGroup=/owned\nDisableControllers=\n").unwrap()["DisableControllers"],
        ""
    );
    assert!(key_values("populated 1\npopulated 0\n").is_err());
    assert!(key_values("populated 0 forged\n").is_err());
    assert_eq!(
        key_values("populated 1\nfrozen 0\n").unwrap()["populated"],
        1
    );
    for value in ["max", "-1", "+1", "1 2", "18446744073709551616"] {
        assert!(number(value).is_err());
    }
    assert_eq!(number("0\n").unwrap(), 0);
}

#[test]
fn metadata_reads_are_bounded_regular_and_symlink_refusing() {
    let root = Temp::new();
    let path = root.0.join("metadata");
    fs::write(&path, "finite\n").unwrap();
    assert_eq!(read_metadata(&path).unwrap(), "finite\n");
    let link = root.0.join("link");
    symlink(&path, &link).unwrap();
    assert!(read_metadata(&link).is_err());
    assert!(read_metadata(&root.0).is_err());
    fs::write(&path, vec![b'x'; METADATA_BYTES as usize + 1]).unwrap();
    assert!(matches!(read_metadata(&path), Err(HarnessError::Limits)));
}

#[test]
fn readonly_file_pin_detects_same_length_mutation_and_missing_control_is_not_zero() {
    let root = Temp::new();
    let path = root.0.join("canary");
    fs::write(&path, b"one").unwrap();
    let pin = FilePin::inspect(&path, 3).unwrap();
    pin.check().unwrap();
    fs::write(&path, b"two").unwrap();
    assert!(pin.check().is_err());
    assert_eq!(
        optional_counter(&root.0.join("absent"), Some(42)).unwrap(),
        Some(42)
    );
    assert_eq!(
        optional_counter(&root.0.join("absent"), None).unwrap(),
        None
    );
    fs::write(&path, b"41").unwrap();
    assert!(optional_counter(&path, Some(42)).is_err());
    assert!(read_controls(&root.0).is_err());
    for (name, contents) in [
        ("cpu.max", "100000 100000"),
        ("memory.max", "536870912"),
        ("memory.swap.max", "0"),
        ("pids.max", "8"),
    ] {
        fs::write(root.0.join(name), contents).unwrap();
    }
    assert_eq!(
        read_controls(&root.0).unwrap(),
        process().containment.unwrap().controls
    );
    fs::write(root.0.join("memory.max"), "max").unwrap();
    assert!(read_controls(&root.0).is_err());
}

#[test]
fn expiration_refuses_even_helper_spawn_and_group_path_is_not_policy_authority() {
    let mut pending = Vec::new();
    let result = run_helper(
        Command::new("/this-test-must-not-execute"),
        Instant::now(),
        &mut pending,
    );
    assert!(matches!(
        result,
        Err(HarnessError::Io(io::ErrorKind::TimedOut))
    ));
    assert!(pending.is_empty());
    let values = vec![
        "cpu memory pids".into(),
        "memory pids".into(),
        "max 100000".into(),
        "max".into(),
        "max".into(),
        "max".into(),
    ];
    assert!(require_controllers(&values).is_err());
    let mut valid = values;
    valid[1] = "cpu memory pids".into();
    require_controllers(&valid).unwrap();
    assert!(inspect_root(Path::new("/tmp/owned-not-cgroup")).is_err());
}

#[test]
fn finite_manager_lifecycle_must_match_instead_of_inheriting_defaults() {
    let p = process();
    let valid = "RuntimeMaxUSec=5s\nTimeoutStopUSec=500ms\nSendSIGKILL=yes\nKillMode=control-group\nKillSignal=15\nFinalKillSignal=9\n";
    verify_lifecycle(&properties(valid).unwrap(), &p).unwrap();
    for (from, to) in [
        ("5s", "infinity"),
        ("500ms", "1min 30s"),
        ("SendSIGKILL=yes", "SendSIGKILL=no"),
        ("control-group", "process"),
        ("KillSignal=15", "KillSignal=9"),
    ] {
        assert!(verify_lifecycle(&properties(&valid.replace(from, to)).unwrap(), &p).is_err());
    }
    assert!(verify_lifecycle(&BTreeMap::new(), &p).is_err());
    assert_eq!(duration_us("1min 500ms").unwrap(), 60_500_000);
    for invalid in ["", "infinity", "-1s", "5", "1.5s", "18446744073709551615h"] {
        assert!(duration_us(invalid).is_err());
    }
}

#[test]
fn privileged_mode_bits_are_refused_without_executing_the_file() {
    let root = Temp::new();
    let executable = root.0.join("never-executed-metadata-fixture");
    fs::write(&executable, b"synthetic metadata").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    refuse_privilege_bits(&executable).unwrap();
    for mode in [0o4700, 0o2700, 0o6700] {
        fs::set_permissions(&executable, fs::Permissions::from_mode(mode)).unwrap();
        assert!(refuse_privilege_bits(&executable).is_err());
    }
}

// Re-executed by the following test with an explicit single-test filter. This
// owned finite fixture touches no external host control or provider interface.
#[test]
fn owned_auxiliary_fixture() {
    let Ok(mode) = std::env::var("CORTEX_OWNED_HELPER_TEST") else {
        return;
    };
    match mode.as_str() {
        "valid" => println!("owned-auxiliary-finished"),
        "wait" => thread::sleep(Duration::from_millis(150)),
        "output" => {
            use std::io::Write;
            let payload = vec![b'x'; 32768];
            std::io::stdout().write_all(&payload).unwrap();
        }
        _ => panic!("unknown owned fixture mode"),
    }
}

#[test]
fn auxiliary_exit_output_and_deadline_are_bounded_with_shared_cleanup() {
    let fixture = |mode: &str| {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .env_clear()
            .env("CORTEX_OWNED_HELPER_TEST", mode)
            .args([
                "--exact",
                "owned_scope::tests::owned_auxiliary_fixture",
                "--nocapture",
                "--test-threads=1",
            ]);
        command
    };
    let mut pending = Vec::new();
    let result = run_helper(
        fixture("valid"),
        Instant::now() + Duration::from_secs(1),
        &mut pending,
    )
    .unwrap();
    assert!(result.status.success());
    assert!(result.output.contains("owned-auxiliary-finished"));
    assert!(pending.is_empty());
    assert!(matches!(
        run_helper(
            fixture("output"),
            Instant::now() + Duration::from_secs(1),
            &mut pending
        ),
        Err(HarnessError::Limits)
    ));
    assert!(pending.is_empty());
    let result = run_helper(
        fixture("wait"),
        Instant::now() + Duration::from_millis(25),
        &mut pending,
    );
    assert!(matches!(
        result,
        Err(HarnessError::Io(io::ErrorKind::TimedOut))
    ));
    // At the absolute deadline cleanup may legitimately remain unresolved.
    // Preserve and explicitly reap only these test-owned auxiliary children.
    for mut helper in pending {
        assert!(helper.identity.pid > 0);
        let until = Instant::now() + Duration::from_secs(1);
        while helper.child.try_wait().unwrap().is_none() {
            assert!(Instant::now() < until);
            thread::sleep(POLL);
        }
    }
}
