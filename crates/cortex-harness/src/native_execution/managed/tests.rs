//! SOURCE_SIMULATION: fixed typed property/status/pipe oracles, no bus/service.
use super::*;
use crate::domain_runtime::tests::fixture;
use std::{
    io::{Read, Write},
    os::fd::AsRawFd,
};
fn limits() -> ServiceLimits {
    ServiceLimits {
        wall_ms: 100,
        cpu_quota_us: 50000,
        cpu_period_us: 100000,
        memory_bytes: 134217728,
        swap_bytes: 0,
        tasks: 8,
    }
}
fn readback() -> ManagerLifecycleReadback {
    let (_, intent) = fixture::coordinator();
    let mut controls = fixture::binding(&intent).controls;
    let l = limits();
    controls.runtime_max_micros = 100000;
    controls.timeout_stop_micros = 500000;
    controls.cpu_quota_us = l.cpu_quota_us;
    controls.cpu_period_us = l.cpu_period_us;
    controls.memory_bytes = l.memory_bytes;
    controls.swap_bytes = l.swap_bytes;
    controls.tasks = l.tasks;
    ManagerLifecycleReadback {
        identity: fixture::identity(&intent),
        service_type: Some("exec".into()),
        restart: Some("no".into()),
        kill_mode: Some("control-group".into()),
        controls: Some(controls),
        active_enter_monotonic_us: Some(1000000),
    }
}
#[test]
fn complete_same_incarnation_manager_readback_passes_only_its_exact_source_contract() {
    let r = readback();
    assert_eq!(r.check(&r.identity, &limits(), Some(1100000)), Ok(()));
    assert_eq!(
        r.check(&r.identity, &limits(), Some(1099999)),
        Err(ManagerObservationError::DeadlineMismatch)
    );
    assert_eq!(
        r.check(&r.identity, &limits(), None),
        Err(ManagerObservationError::MissingReadback)
    );
}
#[test]
fn controller_timer_pid_or_direct_cgroup_cannot_supply_missing_manager_facts() {
    let original = readback();
    for case in 0..5 {
        let mut r = original.clone();
        match case {
            0 => r.controls = None,
            1 => r.service_type = None,
            2 => r.restart = None,
            3 => r.kill_mode = None,
            _ => r.active_enter_monotonic_us = None,
        }
        assert_eq!(
            r.check(&original.identity, &limits(), Some(1100000)),
            Err(ManagerObservationError::MissingReadback)
        );
    }
}
#[test]
fn lifecycle_policy_facts_cannot_be_replaced_with_intended_success_values() {
    let original = readback();
    for case in 0..9 {
        let mut r = original.clone();
        let controls = r.controls.as_mut().unwrap();
        match case {
            0 => controls.runtime_max_micros = 0,
            1 => controls.timeout_stop_micros = 0,
            2 => controls.remain_after_exit = false,
            3 => controls.restart_prevented = false,
            4 => controls.kill_control_group = false,
            5 => controls.no_new_privileges = false,
            6 => controls.send_sigkill = false,
            7 => r.service_type = Some("oneshot".into()),
            _ => r.restart = Some("always".into()),
        }
        assert_eq!(
            r.check(&original.identity, &limits(), Some(1100000)),
            Err(ManagerObservationError::UnsupportedLifetime)
        );
    }
}
#[test]
fn reused_name_with_new_boot_invocation_manager_or_cgroup_refuses_with_unaffected_neighbor() {
    let original = readback();
    let neighbor = readback();
    for case in 0..6 {
        let mut r = original.clone();
        match case {
            0 => r.identity.boot = "different-boot".into(),
            1 => r.identity.invocation = "different-invocation".into(),
            2 => r.identity.manager = "different-manager-owner".into(),
            3 => r.identity.cgroup_inode += 1,
            4 => r.identity.cgroup_device += 1,
            _ => {
                r.identity.request =
                    cortex_core::program::OperationId::new("other-request").unwrap()
            }
        }
        assert_eq!(
            r.check(&original.identity, &limits(), Some(1100000)),
            Err(ManagerObservationError::ChangedIncarnation)
        );
        assert_eq!(
            neighbor.check(&neighbor.identity, &limits(), Some(1100000)),
            Ok(())
        );
    }
}
#[test]
fn activation_delay_or_numeric_overflow_never_extends_original_deadline() {
    let mut r = readback();
    r.active_enter_monotonic_us = Some(1000001);
    assert_eq!(
        r.check(&r.identity, &limits(), Some(1100000)),
        Err(ManagerObservationError::DeadlineMismatch)
    );
    r.active_enter_monotonic_us = Some(u64::MAX);
    assert_eq!(
        r.check(&r.identity, &limits(), Some(u64::MAX)),
        Err(ManagerObservationError::DeadlineMismatch)
    );
    r.active_enter_monotonic_us = Some(0);
    assert_eq!(
        r.check(&r.identity, &limits(), Some(u64::MAX)),
        Err(ManagerObservationError::MissingReadback)
    );
}
#[test]
fn manager_exit_status_is_not_a_payload_reply_or_descendant_closure() {
    assert_eq!(completion(0, 0, 42).unwrap(), None);
    assert_eq!(
        completion(1, 0, 0).unwrap(),
        Some(ProcessCompletion::Exited {
            code: Some(0),
            signal: None
        })
    );
    assert_eq!(
        completion(1, 23, 0).unwrap(),
        Some(ProcessCompletion::Exited {
            code: Some(23),
            signal: None
        })
    );
    assert_eq!(
        completion(2, 9, 0).unwrap(),
        Some(ProcessCompletion::Exited {
            code: None,
            signal: Some(9)
        })
    );
    assert_eq!(
        completion(3, 11, 0).unwrap(),
        Some(ProcessCompletion::Exited {
            code: None,
            signal: Some(11)
        })
    );
    for (code, status) in [(1, 256), (2, 0), (2, 65), (4, 0), (0, 1)] {
        assert!(completion(code, status, 0).is_err());
    }
}
#[test]
fn held_bootstrap_stdio_uses_only_the_opposite_owned_pipe_ends() {
    let (pipes, mut controller) = ServicePipes::create().unwrap();
    let identities = pipes.identities();
    let mut child = Vec::new();
    for (index, fd) in pipes.into_fds().into_iter().enumerate() {
        let stat = rustix::fs::fstat(&fd).unwrap();
        assert_eq!(
            (stat.st_dev, stat.st_ino),
            (identities[index].device, identities[index].inode)
        );
        assert_ne!(fd.as_raw_fd(), controller.stdin.as_raw_fd());
        child.push(File::from(fd));
    }
    controller.stdin.write_all(b"abc").unwrap();
    let mut actual = [0; 3];
    child[0].read_exact(&mut actual).unwrap();
    assert_eq!(&actual, b"abc");
    child[1].write_all(b"out").unwrap();
    controller.stdout.read_exact(&mut actual).unwrap();
    assert_eq!(&actual, b"out");
    child[2].write_all(b"err").unwrap();
    controller.stderr.read_exact(&mut actual).unwrap();
    assert_eq!(&actual, b"err");
}
