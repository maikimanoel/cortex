//! CAP-CLI-003: actual executable -> Git -> Core -> unified evidence -> output.

mod support;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::SystemTime,
};
use support::Repo;

fn head(repo: &Repo) -> String {
    String::from_utf8(repo.git(&["rev-parse", "--verify", "HEAD"]).stdout)
        .unwrap()
        .trim()
        .to_owned()
}
fn command(repo: &Repo, candidate: &str, expected: &str, json: bool) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_cortex"));
    c.arg("check");
    if json {
        c.arg("--json");
    }
    c.args([
        "--task",
        "M002D.revision",
        "--revision",
        "1",
        "--candidate",
        candidate,
        "--expect-head",
        expected,
        "--repo",
    ])
    .arg(&repo.path);
    c
}
fn parsed(output: &Output, exit: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(exit),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["exit_code"], exit);
    assert_eq!(value["authority"], "NONE");
    value
}
#[test]
fn real_pass_and_fail_distinguish_proposition_from_binding_and_exit_zero() {
    let repo = Repo::new();
    repo.commit_file("file", "first");
    let old = head(&repo);
    repo.commit_file("file", "second");
    let current = head(&repo);
    for (expected, result, exit, oracle) in [
        (&current, "PASS", 0, "SATISFIED"),
        (&old, "FAIL", 8, "VIOLATED"),
    ] {
        let output = command(&repo, &current, expected, true).output().unwrap();
        assert!(output.stderr.is_empty());
        let json = parsed(&output, exit);
        assert_eq!(json["type"], "assessment");
        assert!(json["error"].is_null());
        assert_eq!(json["task"]["id"], "M002D.revision");
        assert_eq!(json["task"]["revision"], "1");
        assert_eq!(json["criterion"]["id"], "git.head-equals.v1");
        assert_eq!(json["criterion"]["expected_head"], *expected);
        assert_eq!(json["required_candidate"], current);
        assert_eq!(json["source"]["head"]["commit"], current);
        assert_eq!(json["binding"]["status"], "COMPATIBLE");
        assert_eq!(json["assessment"]["result"], result);
        assert_eq!(json["assessment"]["applicability"], "APPLICABLE");
        assert_eq!(json["assessment"]["validity"], "CURRENT");
        assert_eq!(json["assessment"]["current_pass"], result == "PASS");
        assert_eq!(json["observation"]["outcome"]["execution"]["exit_code"], 0);
        assert_eq!(json["observation"]["outcome"]["relevant_checks"], 1);
        assert_eq!(
            json["observation"]["outcome"]["oracle"]["conclusion"],
            oracle
        );
        assert_eq!(json["source"]["observation_id"]["sequence"], 1);
        assert_eq!(json["reobserved"]["observation_id"]["sequence"], 2);
        assert_eq!(json["observation"]["id"]["sequence"], 3);
        assert_eq!(json["observation"]["authenticated"], false);
        assert_eq!(json["observation"]["source"], "CALLER_REPORTED");
        assert!(
            json["observation"]["mechanism"]["components"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["name"] == "git"
                    && c["version"].as_str().unwrap().starts_with("git version "))
        );
        for field in ["observed_at", "recorded_at"] {
            assert!(json["observation"][field]["unix_seconds"].as_u64().unwrap() > 0);
        }
        let human = command(&repo, &current, expected, false).output().unwrap();
        assert_eq!(human.status.code(), Some(exit));
        assert!(human.stderr.is_empty());
        let text = String::from_utf8(human.stdout).unwrap();
        for required in [
            format!("Result: {result}"),
            "Applicability: APPLICABLE".into(),
            "Validity: CURRENT".into(),
            "Authority: NONE".into(),
            "not an atomic snapshot".into(),
            "no source-code correctness".into(),
            format!("Observed HEAD: {current}"),
            format!("Expected HEAD: {expected}"),
        ] {
            assert!(text.contains(&required), "{text}");
        }
    }
}
#[test]
fn wrong_candidate_pin_is_unknown_stale_rather_than_a_criterion_failure_or_pass() {
    let repo = Repo::new();
    repo.commit_file("file", "first");
    let old = head(&repo);
    repo.commit_file("file", "second");
    let current = head(&repo);
    let output = command(&repo, &old, &current, true).output().unwrap();
    let json = parsed(&output, 9);
    assert_eq!(json["binding"]["status"], "INCOMPATIBLE");
    assert_eq!(json["binding"]["reason"], "COMMIT");
    assert_eq!(json["assessment"]["result"], "UNKNOWN");
    assert_eq!(json["assessment"]["validity"], "STALE");
    assert_eq!(json["assessment"]["current_pass"], false);
    assert_eq!(json["source"]["head"]["commit"], current);
}
#[test]
fn mixed_dirty_status_cannot_be_promoted_by_an_equal_head_or_successful_git_exit() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let oid = head(&repo);
    fs::write(repo.path.join("file"), "staged").unwrap();
    repo.git(&["add", "file"]);
    fs::write(repo.path.join("file"), "unstaged").unwrap();
    fs::write(repo.path.join("private-filename"), "untracked").unwrap();
    let output = command(&repo, &oid, &oid, true).output().unwrap();
    let json = parsed(&output, 9);
    assert_eq!(json["assessment"]["result"], "UNKNOWN");
    assert_eq!(json["assessment"]["validity"], "UNESTABLISHED");
    assert_eq!(json["binding"]["reason"], "UNCOMMITTED_IDENTITY");
    assert_eq!(
        json["source"]["working_tree"],
        serde_json::json!({"state":"DIRTY","staged":1,"unstaged":1,"untracked":1})
    );
    assert_eq!(json["source"]["head"]["commit"], oid);
    assert_eq!(json["assessment"]["current_pass"], false);
    assert!(
        !String::from_utf8(output.stdout)
            .unwrap()
            .contains("private-filename")
    );
}
#[test]
fn detached_commit_passes_and_unborn_state_never_fabricates_a_commit() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let oid = head(&repo);
    repo.git(&["checkout", "--detach"]);
    let json = parsed(&command(&repo, &oid, &oid, true).output().unwrap(), 0);
    assert_eq!(json["source"]["head"]["kind"], "DETACHED");
    assert!(json["source"]["head"]["branch"].is_null());
    repo.git(&["symbolic-ref", "HEAD", "refs/heads/unborn"]);
    let json = parsed(&command(&repo, &oid, &oid, true).output().unwrap(), 9);
    assert_eq!(json["source"]["head"]["kind"], "UNBORN");
    assert!(json["source"]["head"]["commit"].is_null());
    assert_eq!(json["binding"]["reason"], "COMMIT_ABSENT");
    assert_eq!(json["assessment"]["result"], "UNKNOWN");
    assert_eq!(
        json["observation"]["outcome"]["oracle"]["conclusion"],
        "INSUFFICIENT"
    );
}
#[test]
fn invalid_and_incomplete_inputs_have_parseable_errors_without_private_payloads() {
    let oid = "a".repeat(40);
    let base = vec![
        "--task".to_owned(),
        "valid".into(),
        "--revision".into(),
        "1".into(),
        "--candidate".into(),
        oid.clone(),
        "--expect-head".into(),
        oid,
    ];
    let mut cases = vec![
        vec![],
        vec!["--private-token=secret".into()],
        vec!["--repo".into()],
        vec!["--help".into(), "extra".into()],
    ];
    for (index, invalid) in [
        (1, "private/token"),
        (3, "0"),
        (3, "18446744073709551616"),
        (3, "+1"),
        (5, "DEADBEEF"),
        (7, "short"),
        (1, ""),
    ] {
        let mut args = base.clone();
        args[index] = invalid.into();
        cases.push(args);
    }
    let mut duplicate = base.clone();
    duplicate.extend(["--task".into(), "secret".into()]);
    cases.push(duplicate);
    let mut oversized = base.clone();
    oversized[1] = "x".repeat(129);
    cases.push(oversized);
    let mut oversized = base.clone();
    oversized.extend(["--repo".into(), "z".repeat(16_385)]);
    cases.push(oversized);
    let mut count = base;
    count.extend(vec!["x".into(); 5]);
    cases.push(count);
    for args in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
            .args(["check", "--json"])
            .args(args)
            .output()
            .unwrap();
        let json = parsed(&output, 2);
        assert_eq!(json["type"], "error");
        assert!(json["assessment"].is_null());
        assert!(!output.stderr.is_empty());
        let all = format!(
            "{}{}",
            String::from_utf8(output.stdout).unwrap(),
            String::from_utf8(output.stderr).unwrap()
        );
        assert!(!all.contains("private/token"));
        assert!(!all.contains("--private-token=secret"));
        assert!(!all.contains("DEADBEEF"));
    }
}
#[test]
fn revision_and_identifier_upper_boundaries_remain_exact() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let oid = head(&repo);
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args([
            "check",
            "--json",
            "--task",
            &"x".repeat(128),
            "--revision",
            &u64::MAX.to_string(),
            "--candidate",
            &oid,
            "--expect-head",
            &oid,
            "--repo",
        ])
        .arg(&repo.path)
        .output()
        .unwrap();
    let json = parsed(&output, 0);
    assert_eq!(json["task"]["revision"], u64::MAX.to_string());
    assert_eq!(json["task"]["id"], "x".repeat(128));
}
#[test]
fn path_nonrepository_bare_and_git_unavailable_errors_never_invent_an_assessment() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let oid = head(&repo);
    let mut missing = command(&repo, &oid, &oid, true);
    // Override the existing value, without adding a duplicate option.
    let args: Vec<OsString> = missing.get_args().map(OsString::from).collect();
    let mut invalid = args.clone();
    *invalid.last_mut().unwrap() = repo.path.join("absent").into_os_string();
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(invalid)
        .output()
        .unwrap();
    assert!(parsed(&output, 2)["assessment"].is_null());
    let plain = Repo::new();
    fs::remove_dir_all(plain.path.join(".git")).unwrap();
    let mut nonrepo = args.clone();
    *nonrepo.last_mut().unwrap() = plain.path.clone().into_os_string();
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(nonrepo)
        .output()
        .unwrap();
    assert!(parsed(&output, 4)["assessment"].is_null());
    repo.git(&["init", "--bare", "bare"]);
    let mut bare = args;
    *bare.last_mut().unwrap() = repo.path.join("bare").into_os_string();
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(bare)
        .output()
        .unwrap();
    assert_eq!(parsed(&output, 5)["error"]["category"], "UNSUPPORTED");
    missing.env("PATH", &repo.path);
    let output = missing.output().unwrap();
    assert_eq!(parsed(&output, 3)["error"]["category"], "GIT_UNAVAILABLE");
}
#[test]
fn unicode_quotes_controls_and_non_utf8_paths_survive_without_output_injection() {
    let mut repo = Repo::new();
    let name =
        repo.path.file_name().unwrap().to_string_lossy().to_string() + "-雪-quote\"-slash\\-line\n";
    let path = repo.path.with_file_name(name);
    fs::rename(&repo.path, &path).unwrap();
    repo.path = path;
    repo.commit_file("file", "initial");
    let oid = head(&repo);
    repo.git(&["branch", "-m", "topic/雪\"quoted"]);
    let output = command(&repo, &oid, &oid, true).output().unwrap();
    let json = parsed(&output, 0);
    assert_eq!(
        json["source"]["repository_root"]["value"],
        repo.path.canonicalize().unwrap().to_str().unwrap()
    );
    assert_eq!(
        json["source"]["head"]["branch"]["value"],
        "topic/雪\"quoted"
    );
    let human = command(&repo, &oid, &oid, false).output().unwrap();
    let text = String::from_utf8(human.stdout).unwrap();
    assert!(!text.contains("slash\\-line\n"));
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let path = repo.path.join(OsString::from_vec(b"nonutf8-\xff".to_vec()));
        fs::create_dir(&path).unwrap();
        repo.git(&["commit", "--allow-empty", "-m", "stable"]);
        let oid = head(&repo);
        // Select a subdirectory with non-UTF-8 bytes. Root still has UTF-8 bytes.
        let output = command(&repo, &oid, &oid, true)
            .current_dir(&path)
            .output()
            .unwrap();
        assert_eq!(parsed(&output, 0)["assessment"]["result"], "PASS");
        let mut bad = command(&repo, &oid, &oid, true);
        bad.arg(OsString::from_vec(vec![255]));
        let output = bad.output().unwrap();
        assert_eq!(parsed(&output, 2)["type"], "error");
        let mut bad = Command::new(env!("CARGO_BIN_EXE_cortex"));
        bad.args(["check", "--json", "--task"])
            .arg(OsString::from_vec(vec![255]))
            .args([
                "--revision",
                "1",
                "--candidate",
                &oid,
                "--expect-head",
                &oid,
            ]);
        let output = bad.output().unwrap();
        assert_eq!(parsed(&output, 2)["type"], "error");
        fs::remove_dir(&path).unwrap();
        assert!(
            repo.command()
                .args(["branch", "-m"])
                .arg(OsString::from_vec(b"topic-\xff".to_vec()))
                .output()
                .unwrap()
                .status
                .success()
        );
        let branch = parsed(&command(&repo, &oid, &oid, true).output().unwrap(), 0);
        assert_eq!(branch["source"]["head"]["branch"]["encoding"], "hex");
        assert_eq!(
            branch["source"]["head"]["branch"]["value"],
            "746f7069632dff"
        );
        let mut name = repo.path.file_name().unwrap().as_encoded_bytes().to_vec();
        name.push(255);
        let moved = repo.path.with_file_name(OsString::from_vec(name));
        fs::rename(&repo.path, &moved).unwrap();
        repo.path = moved;
        let json = parsed(&command(&repo, &oid, &oid, true).output().unwrap(), 0);
        assert_eq!(json["source"]["repository_root"]["encoding"], "hex");
    }
}
#[test]
fn subdirectory_default_and_linked_worktree_select_actual_canonical_root() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let oid = head(&repo);
    let sub = repo.path.join("sub");
    fs::create_dir(&sub).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .current_dir(&sub)
        .args([
            "check",
            "--json",
            "--task",
            "default",
            "--revision",
            "1",
            "--candidate",
            &oid,
            "--expect-head",
            &oid,
        ])
        .output()
        .unwrap();
    let json = parsed(&output, 0);
    assert_eq!(
        json["source"]["repository_root"]["value"],
        repo.path.canonicalize().unwrap().to_str().unwrap()
    );
    let linked = repo.path.join("linked");
    repo.git(&["worktree", "add", "--detach", linked.to_str().unwrap()]);
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args([
            "check",
            "--json",
            "--task",
            "linked",
            "--revision",
            "1",
            "--candidate",
            &oid,
            "--expect-head",
            &oid,
            "--repo",
        ])
        .arg(&linked)
        .output()
        .unwrap();
    let json = parsed(&output, 0);
    assert_eq!(
        json["source"]["repository_root"]["value"],
        linked.canonicalize().unwrap().to_str().unwrap()
    );
}
#[test]
fn sha256_revisions_use_actual_full_commit_identity() {
    let repo = Repo::new();
    fs::remove_dir_all(repo.path.join(".git")).unwrap();
    repo.git(&["init", "--object-format=sha256", "--initial-branch=main"]);
    repo.git(&["config", "user.name", "Cortex Test"]);
    repo.git(&["config", "user.email", "cortex-test@example.invalid"]);
    repo.git(&["config", "commit.gpgSign", "false"]);
    repo.commit_file("file", "initial");
    let oid = head(&repo);
    assert_eq!(oid.len(), 64);
    let json = parsed(&command(&repo, &oid, &oid, true).output().unwrap(), 0);
    assert_eq!(json["source"]["head"]["commit"], oid);
}
#[derive(Debug, PartialEq, Eq)]
struct State {
    bytes: Vec<u8>,
    target: Option<PathBuf>,
    modified: SystemTime,
    readonly: bool,
    #[cfg(unix)]
    stat: (u64, u64, u32, u32, u32, i64, i64),
}
fn snapshot(root: &Path) -> BTreeMap<PathBuf, State> {
    fn visit(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, State>) {
        let m = fs::symlink_metadata(path).unwrap();
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        result.insert(
            path.strip_prefix(root).unwrap().to_owned(),
            State {
                bytes: if m.is_file() {
                    fs::read(path).unwrap()
                } else {
                    vec![]
                },
                target: if m.is_symlink() {
                    Some(fs::read_link(path).unwrap())
                } else {
                    None
                },
                modified: m.modified().unwrap(),
                readonly: m.permissions().readonly(),
                #[cfg(unix)]
                stat: (
                    m.dev(),
                    m.ino(),
                    m.mode(),
                    m.uid(),
                    m.gid(),
                    m.ctime(),
                    m.ctime_nsec(),
                ),
            },
        );
        if m.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                visit(root, &entry.unwrap().path(), result);
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}
#[test]
fn repeated_clean_dirty_and_linked_checks_preserve_all_protected_fixture_state() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let oid = head(&repo);
    repo.git(&["config", "private.token", "private-config-sentinel"]);
    let linked = repo.path.join("linked");
    repo.git(&["worktree", "add", "--detach", linked.to_str().unwrap()]);
    // Modified mtime but equal content can cause an ordinary Git status index refresh.
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(linked.join("file"), "initial").unwrap();
    let before = snapshot(&repo.path);
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
            .args([
                "check",
                "--json",
                "--task",
                "readonly",
                "--revision",
                "1",
                "--candidate",
                &oid,
                "--expect-head",
                &oid,
                "--repo",
            ])
            .arg(&linked)
            .env("GIT_TRACE", linked.join("trace-sentinel"))
            .output()
            .unwrap();
        let json = parsed(&output, 0);
        assert_eq!(json["assessment"]["result"], "PASS");
        assert!(
            !String::from_utf8(output.stdout)
                .unwrap()
                .contains("private-config-sentinel")
        );
    }
    assert_eq!(snapshot(&repo.path), before);
    fs::write(linked.join("file"), "staged").unwrap();
    assert!(
        repo.command()
            .current_dir(&linked)
            .args(["add", "--", "file"])
            .output()
            .unwrap()
            .status
            .success()
    );
    fs::write(linked.join("file"), "dirty unstaged").unwrap();
    fs::write(linked.join("new-private-file"), "private-content-sentinel").unwrap();
    let before = snapshot(&repo.path);
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
            .args([
                "check",
                "--json",
                "--task",
                "readonly",
                "--revision",
                "1",
                "--candidate",
                &oid,
                "--expect-head",
                &oid,
                "--repo",
            ])
            .arg(&linked)
            .output()
            .unwrap();
        assert_eq!(
            parsed(&output, 9)["assessment"]["validity"],
            "UNESTABLISHED"
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(!text.contains("new-private-file"));
        assert!(!text.contains("private-content-sentinel"));
    }
    assert_eq!(snapshot(&repo.path), before);
}
#[test]
fn check_help_and_json_option_position_are_discoverable_and_unambiguous() {
    let out = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(["check", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(out.stderr.is_empty());
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("--json must immediately follow check"));
    assert!(text.contains("0 current PASS; 8 current FAIL; 9 unresolved"));
    let out = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(["check", "--task", "invalid", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(!out.stderr.is_empty());
}
