use super::*;
use std::{
    os::unix::fs::{PermissionsExt, symlink},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    proc: PathBuf,
    cgroup: PathBuf,
    bwrap: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "cortex-preflight-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let proc = root.join("proc");
        let cgroup = root.join("cgroup");
        fs::create_dir_all(proc.join("self")).unwrap();
        fs::create_dir_all(proc.join("sys/user")).unwrap();
        fs::create_dir_all(proc.join("sys/kernel")).unwrap();
        fs::create_dir_all(cgroup.join("owned.scope")).unwrap();
        fs::write(proc.join("self/cgroup"), "0::/owned.scope\n").unwrap();
        fs::write(
            proc.join("self/mountinfo"),
            "1 0 0:1 / /sys/fs/cgroup rw,nosuid,nodev,noexec - cgroup2 cgroup rw\n",
        )
        .unwrap();
        fs::write(proc.join("sys/user/max_user_namespaces"), "1024\n").unwrap();
        fs::write(proc.join("sys/kernel/unprivileged_userns_clone"), "1\n").unwrap();
        for (name, value) in [
            ("cgroup.controllers", "cpu memory pids\n"),
            ("cgroup.subtree_control", "\n"),
            ("cpu.max", "100000 100000\n"),
            ("memory.max", "536870912\n"),
            ("memory.swap.max", "0\n"),
            ("pids.max", "8\n"),
            ("cgroup.events", "populated 1\nfrozen 0\n"),
        ] {
            fs::write(cgroup.join("owned.scope").join(name), value).unwrap();
        }
        let bwrap = root.join("bwrap");
        fs::write(&bwrap, "not an executable; metadata fixture only").unwrap();
        fs::set_permissions(&bwrap, fs::Permissions::from_mode(0o700)).unwrap();
        Self {
            root,
            proc,
            cgroup,
            bwrap,
        }
    }

    fn inspect(&self) -> HostPreflight {
        inspect_at(&self.proc, &self.cgroup, &self.bwrap, || {})
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn finite_metadata_never_becomes_delegation_or_qualification() {
    let fixture = Fixture::new();
    let before = fs::read(fixture.cgroup.join("owned.scope/cpu.max")).unwrap();
    let report = fixture.inspect();
    assert_eq!(
        report.cgroup.cpu_max,
        Observed::Value(CpuMax {
            quota: Ceiling::Limited(100000),
            period_micros: 100000
        })
    );
    assert_eq!(
        report.cgroup.memory_max,
        Observed::Value(Ceiling::Limited(536870912))
    );
    assert_eq!(report.cgroup.swap_max, Observed::Value(Ceiling::Limited(0)));
    assert_eq!(report.cgroup.pids_max, Observed::Value(Ceiling::Limited(8)));
    assert!(report.cgroup.membership_consistent);
    assert_eq!(report.cgroup.depth, Observed::Value(1));
    assert!(report.control_gaps().iter().all(|gap| {
        gap.reasons.contains(&GapReason::DelegationUnestablished)
            && gap.reasons.contains(&GapReason::BehaviorNotRun)
    }));
    assert_eq!(
        fs::read(fixture.cgroup.join("owned.scope/cpu.max")).unwrap(),
        before
    );
    assert!(!format!("{report:?}").contains("owned.scope"));
    assert!(report.reads <= READS);
    assert!(report.bytes_read <= TOTAL_BYTES);
}

#[test]
fn absent_controller_is_distinct_from_unlimited_configuration() {
    let fixture = Fixture::new();
    fs::write(
        fixture.cgroup.join("owned.scope/cgroup.controllers"),
        "memory pids\n",
    )
    .unwrap();
    fs::remove_file(fixture.cgroup.join("owned.scope/cpu.max")).unwrap();
    fs::write(fixture.cgroup.join("owned.scope/memory.max"), "max\n").unwrap();
    let report = fixture.inspect();
    assert_eq!(
        report.cgroup.cpu_max,
        Observed::Unavailable(ReadIssue::Missing)
    );
    assert_eq!(
        report.cgroup.memory_max,
        Observed::Value(Ceiling::Unlimited)
    );
    let gaps = report.control_gaps();
    assert!(
        gaps.iter()
            .find(|g| g.control == HostControl::CpuRate)
            .unwrap()
            .reasons
            .contains(&GapReason::ControllerMissing)
    );
    assert!(
        !gaps
            .iter()
            .find(|g| g.control == HostControl::MemorySwap)
            .unwrap()
            .reasons
            .contains(&GapReason::ControllerMissing)
    );
}

#[test]
fn changed_membership_invalidates_all_cgroup_values() {
    let fixture = Fixture::new();
    let report = inspect_at(&fixture.proc, &fixture.cgroup, &fixture.bwrap, || {
        fs::write(fixture.proc.join("self/cgroup"), "0::/other.scope\n").unwrap();
    });
    assert_eq!(report.cgroup, missing_cgroup(ReadIssue::Changed));
    assert_eq!(report.namespaces.max_user_namespaces, Observed::Value(1024));
}

#[test]
fn traversal_ambiguous_and_noncanonical_memberships_refuse() {
    for raw in [
        "0::/../outside\n",
        "0::/a/../../outside\n",
        "0::/a/./b\n",
        "0:://outside\n",
        "0::relative\n",
        "0::/trailing/\n",
        "0::/a\n0::/b\n",
        "1:cpu:/a\n0::/a\n",
        "0::/nul\0x\n",
    ] {
        assert_eq!(membership(raw), Err(ReadIssue::Malformed), "{raw:?}");
    }
    assert_eq!(membership("0::/\n"), Ok(PathBuf::new()));
    assert_eq!(
        membership("0::/normal.scope/sub-group\n"),
        Ok(PathBuf::from("normal.scope/sub-group"))
    );
}

#[test]
fn invalid_membership_does_not_read_an_outside_canary() {
    let fixture = Fixture::new();
    fs::write(fixture.proc.join("self/cgroup"), "0::/../secret\n").unwrap();
    let report = fixture.inspect();
    assert_eq!(
        report.cgroup.depth,
        Observed::Unavailable(ReadIssue::Malformed)
    );
    assert_eq!(
        report.cgroup.cpu_max,
        Observed::Unavailable(ReadIssue::Malformed)
    );
    assert!(report.reads <= 5);
}

#[test]
fn symlink_and_nonregular_metadata_are_refused_with_valid_neighbor() {
    let fixture = Fixture::new();
    let cpu = fixture.cgroup.join("owned.scope/cpu.max");
    fs::remove_file(&cpu).unwrap();
    let canary = fixture.root.join("protected-canary");
    fs::write(&canary, "99999 100000\n").unwrap();
    symlink(&canary, &cpu).unwrap();
    let report = fixture.inspect();
    assert_eq!(
        report.cgroup.cpu_max,
        Observed::Unavailable(ReadIssue::UnsupportedFile)
    );
    assert_eq!(report.cgroup.pids_max, Observed::Value(Ceiling::Limited(8)));
    fs::remove_file(&cpu).unwrap();
    fs::create_dir(&cpu).unwrap();
    assert_eq!(
        fixture.inspect().cgroup.cpu_max,
        Observed::Unavailable(ReadIssue::UnsupportedFile)
    );
}

#[test]
fn symlink_cgroup_component_never_follows_into_owned_canary() {
    let fixture = Fixture::new();
    fs::rename(
        fixture.cgroup.join("owned.scope"),
        fixture.root.join("protected"),
    )
    .unwrap();
    symlink(
        fixture.root.join("protected"),
        fixture.cgroup.join("owned.scope"),
    )
    .unwrap();
    let report = fixture.inspect();
    assert_eq!(
        report.cgroup.depth,
        Observed::Unavailable(ReadIssue::UnsupportedFile)
    );
    assert_eq!(
        report.cgroup.pids_max,
        Observed::Unavailable(ReadIssue::UnsupportedFile)
    );
}

#[test]
fn metadata_budgets_preserve_failure_instead_of_partial_values() {
    let fixture = Fixture::new();
    let path = fixture.root.join("bounded");
    fs::write(&path, vec![b'x'; FILE_BYTES]).unwrap();
    let mut reader = Reader::new();
    assert_eq!(reader.read(&path).unwrap().len(), FILE_BYTES);
    fs::write(&path, vec![b'x'; FILE_BYTES + 1]).unwrap();
    assert_eq!(Reader::new().read(&path), Err(ReadIssue::Capacity));
    fs::write(&path, vec![b'x'; FILE_BYTES]).unwrap();
    let mut reader = Reader::new();
    for _ in 0..4 {
        assert!(reader.read(&path).is_ok());
    }
    assert_eq!(reader.read(&path), Err(ReadIssue::Capacity));
    assert_eq!(reader.bytes, TOTAL_BYTES);
    let mut reader = Reader::new();
    reader.reads = READS;
    assert_eq!(reader.read(&path), Err(ReadIssue::Capacity));
    let mut reader = Reader::new();
    reader.start = Instant::now() - DEADLINE;
    assert_eq!(reader.read(&path), Err(ReadIssue::Deadline));
}

#[test]
fn numeric_controller_and_event_parsers_reject_ambiguity() {
    for input in ["", "+1", "-1", "1 2", "18446744073709551616", "1x"] {
        assert_eq!(number(input), Err(ReadIssue::Malformed));
    }
    assert_eq!(number("0\n"), Ok(0));
    assert_eq!(
        cpu_max("max 100000\n"),
        Ok(CpuMax {
            quota: Ceiling::Unlimited,
            period_micros: 100000
        })
    );
    for input in ["0 100000", "100 0", "max", "max 100 10"] {
        assert_eq!(cpu_max(input), Err(ReadIssue::Malformed));
    }
    assert_eq!(controllers("cpu cpu memory"), Err(ReadIssue::Malformed));
    assert_eq!(controllers("cpu +memory"), Err(ReadIssue::Malformed));
    assert_eq!(
        controllers("\n"),
        Ok(Controllers {
            cpu: false,
            memory: false,
            pids: false
        })
    );
    assert_eq!(
        populated("populated 1\npopulated 0\n"),
        Err(ReadIssue::Malformed)
    );
    assert_eq!(populated("frozen 0\n"), Err(ReadIssue::Malformed));
    assert_eq!(populated("populated 0\nfrozen 0\n"), Ok(false));
}

#[test]
fn namespace_disabled_and_backend_missing_remain_causal_gaps() {
    let fixture = Fixture::new();
    fs::write(fixture.proc.join("sys/user/max_user_namespaces"), "0\n").unwrap();
    fs::remove_file(&fixture.bwrap).unwrap();
    let report = fixture.inspect();
    let gaps = report.control_gaps();
    let fs_gap = gaps
        .iter()
        .find(|g| g.control == HostControl::Filesystem)
        .unwrap();
    assert!(fs_gap.reasons.contains(&GapReason::NamespaceDisabled));
    assert!(fs_gap.reasons.contains(&GapReason::BackendUnavailable));
    assert!(fs_gap.reasons.contains(&GapReason::BehaviorNotRun));
}

#[test]
fn unsupported_or_duplicate_mounts_block_cgroup_observation() {
    let good = "1 0 0:1 / /sys/fs/cgroup rw - cgroup2 cgroup rw\n";
    assert_eq!(mount(good), Ok(true));
    assert_eq!(
        mount(&format!("{good}{good}")),
        Err(ReadIssue::UnsupportedFile)
    );
    assert_eq!(
        mount(&good.replace("cgroup2", "cgroup")),
        Err(ReadIssue::UnsupportedFile)
    );
    assert_eq!(
        mount(&good.replace("0:1 / ", "0:1 /subtree ")),
        Err(ReadIssue::UnsupportedFile)
    );
    assert_eq!(mount("broken\n"), Err(ReadIssue::Malformed));
}
