//! Exercise the real terminal executable, including invalid invocations.

use std::process::{Command, Output};

fn invoke(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(args)
        .output()
        .expect("cortex executable must start")
}

#[test]
fn help_flags_describe_only_the_implemented_interface() {
    for flag in ["--help", "-h"] {
        let output = invoke(&[flag]);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("Usage: cortex [OPTIONS]"));
        assert!(text.contains("--help"));
        assert!(text.contains("--version"));
        assert!(text.contains("no execution commands are available"));
    }
}

#[test]
fn version_flags_report_the_package_version_exactly() {
    for flag in ["--version", "-V"] {
        let output = invoke(&[flag]);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout,
            format!("cortex {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
        );
    }
}

#[test]
fn no_arguments_prints_help() {
    let output = invoke(&[]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, invoke(&["--help"]).stdout);
}

#[test]
fn unknown_arguments_fail_without_echoing_private_input() {
    for argument in ["--private-token=example", "run", ""] {
        let output = invoke(&[argument]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            b"cortex: unsupported arguments; use --help\n"
        );
    }
}

#[test]
fn extra_arguments_are_rejected_even_after_valid_flags() {
    for args in [["--help", "run"], ["--version", "--help"]] {
        let output = invoke(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            b"cortex: unsupported arguments; use --help\n"
        );
    }
}

#[cfg(unix)]
#[test]
fn non_utf8_arguments_are_rejected_without_panicking() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .arg(OsString::from_vec(vec![0xff]))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"cortex: unsupported arguments; use --help\n"
    );
}
