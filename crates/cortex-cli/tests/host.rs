#![cfg(target_os = "linux")]
//! Actual command output must preserve the difference between metadata and proof.
use std::process::{Command, Output};

fn invoke(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn host_inspection_has_no_authority_or_implicit_qualification() {
    let result = invoke(&["host", "inspect", "--json"]);
    assert!(result.status.success());
    assert!(result.stderr.is_empty());
    let view: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(view["schema"], "cortex.host-preflight.v1");
    assert_eq!(view["status"], "INSPECTED");
    assert_eq!(view["host_qualification"], "BLOCKED");
    assert_eq!(view["effects_performed"], false);
    assert_eq!(view["backend_activated"], false);
    assert_eq!(view["execution_authority"], "INSPECTION_CONFERS_NONE");
    assert_eq!(view["cgroup"]["delegation"], "UNESTABLISHED");
    let controls = view["controls"].as_array().unwrap();
    assert_eq!(controls.len(), 10);
    for control in controls {
        assert_eq!(control["qualification"], "BLOCKED");
        assert_eq!(control["behavioral_test"], "NOT_RUN");
        assert!(
            control["causes"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("BehaviorNotRun"))
        );
    }
    let text = String::from_utf8(result.stdout).unwrap();
    for private_field in ["/home/", "/user.slice/", "owner_uid", "inode", "device"] {
        assert!(!text.contains(private_field));
    }
    let human = invoke(&["host", "inspect"]);
    assert!(human.status.success());
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(text.contains("qualification BLOCKED"));
    assert!(text.contains("behavior NOT_RUN"));
}

#[test]
fn host_help_and_rejected_operation_do_not_accept_effect_flags() {
    for args in [&["host", "--help"][..], &["host", "inspect", "--help"]] {
        let result = invoke(args);
        assert!(result.status.success());
        assert!(
            String::from_utf8(result.stdout)
                .unwrap()
                .contains("without launching a process")
        );
    }
    for args in [
        &["host"][..],
        &["host", "run"],
        &["host", "inspect", "--activate"],
        &["host", "inspect", "--json", "--json"],
        &["host", "inspect", "private-canary=synthetic"],
    ] {
        let result = invoke(args);
        assert_eq!(result.status.code(), Some(2));
        assert!(
            !String::from_utf8(result.stdout)
                .unwrap()
                .contains("private-canary")
        );
    }
}
