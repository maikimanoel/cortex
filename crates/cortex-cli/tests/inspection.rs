//! CAP-CLI-002 acceptance: real local Git fixtures and the actual Cortex binary.

mod support;
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use support::Repo;

fn inspect(path: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args([
            OsStr::new("inspect"),
            OsStr::new("--repo"),
            path.as_os_str(),
        ])
        .output()
        .unwrap()
}
fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}
fn failure(output: Output, code: i32, message: &str) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr).unwrap().contains(message));
}
fn counts(text: &str, staged: usize, unstaged: usize, untracked: usize) {
    assert!(
        text.contains(&format!(
            "Staged: {staged}\nUnstaged: {unstaged}\nUntracked: {untracked}\n"
        )),
        "{text}"
    );
    assert!(text.contains(if staged + unstaged + untracked == 0 {
        "Working tree: CLEAN (Git status coverage)"
    } else {
        "Working tree: DIRTY"
    }));
    assert!(text.contains("no working snapshot identity"));
    assert!(text.contains("Assessment: none"));
}

#[test]
fn clean_commit_and_default_directory_are_observed() {
    let repo = Repo::new();
    repo.commit_file("tracked", "initial");
    let head = String::from_utf8(repo.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    let text = success(
        Command::new(env!("CARGO_BIN_EXE_cortex"))
            .current_dir(&repo.path)
            .arg("inspect")
            .output()
            .unwrap(),
    );
    assert!(text.contains(&format!("HEAD: {head}")));
    assert!(text.contains("Branch: \"main\"\n"));
    counts(&text, 0, 0, 0);
    assert_eq!(text, success(inspect(&repo.path)));
}

#[test]
fn staged_unstaged_untracked_and_mixed_remain_distinct() {
    for (staged, unstaged, untracked) in [
        (true, false, false),
        (false, true, false),
        (false, false, true),
        (true, true, true),
    ] {
        let repo = Repo::new();
        repo.commit_file("tracked", "initial");
        if staged {
            fs::write(repo.path.join("tracked"), "staged").unwrap();
            repo.git(&["add", "tracked"]);
        }
        if unstaged {
            fs::write(repo.path.join("tracked"), "unstaged longer").unwrap();
        }
        if untracked {
            fs::write(repo.path.join("untracked"), "new").unwrap();
        }
        counts(
            &success(inspect(&repo.path)),
            usize::from(staged),
            usize::from(unstaged),
            usize::from(untracked),
        );
    }
}

#[test]
fn detached_head_retains_commit_and_unborn_does_not_invent_one() {
    let repo = Repo::new();
    let text = success(inspect(&repo.path));
    assert!(text.contains("HEAD: NONE (unborn)\nBranch: \"main\" (unborn)"));
    counts(&text, 0, 0, 0);
    fs::write(repo.path.join("first"), "new").unwrap();
    repo.git(&["add", "first"]);
    counts(&success(inspect(&repo.path)), 1, 0, 0);
    repo.git(&["commit", "-m", "first"]);
    let head = String::from_utf8(repo.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    repo.git(&["checkout", "--detach"]);
    let text = success(inspect(&repo.path));
    assert!(text.contains(&format!("HEAD: {head}Branch: DETACHED\n")));
}

#[test]
fn subdirectory_observes_the_whole_worktree() {
    let repo = Repo::new();
    repo.commit_file("outside", "initial");
    fs::create_dir(repo.path.join("inside")).unwrap();
    fs::write(repo.path.join("outside"), "changed").unwrap();
    let text = success(inspect(&repo.path.join("inside")));
    assert!(text.contains(&format!(
        "Repository: {:?}\n",
        repo.path.canonicalize().unwrap()
    )));
    counts(&text, 0, 1, 0);
    repo.git(&["update-index", "--assume-unchanged", "outside"]);
    failure(inspect(&repo.path.join("inside")), 5, "assume-unchanged");
}

#[test]
fn invalid_non_repository_file_and_bare_paths_fail_explicitly() {
    let repo = Repo::new();
    failure(
        inspect(&repo.path.join("absent-private-name")),
        2,
        "cannot access repository path",
    );
    fs::write(repo.path.join("file"), "x").unwrap();
    failure(inspect(&repo.path.join("file")), 2, "must be a directory");
    let plain = repo.path.join("plain");
    fs::create_dir(&plain).unwrap();
    // Outside this fixture's repository discovery chain.
    let empty = Repo::new();
    fs::remove_dir_all(empty.path.join(".git")).unwrap();
    failure(inspect(&empty.path), 4, "local Git worktree is required");
    repo.git(&["init", "--bare", "bare"]);
    failure(inspect(&repo.path.join("bare")), 5, "bare repository");
    failure(
        inspect(&repo.path.join(".git")),
        5,
        "outside the working tree",
    );
}

#[test]
fn spaces_unicode_and_shell_syntax_are_literal_paths() {
    let repo = Repo::new();
    let nested = repo.path.join("répo space $(touch SENTINEL)");
    repo.git(&["init", "--initial-branch=main", nested.to_str().unwrap()]);
    for name in [
        "space file",
        "日本語",
        "$(touch SENTINEL)",
        "-dash",
        "quote'\"",
        "line\nbreak\t",
    ] {
        fs::write(nested.join(name), "new").unwrap();
    }
    let text = success(inspect(&nested));
    counts(&text, 0, 0, 6);
    assert!(!repo.path.join("SENTINEL").exists());
    assert!(!nested.join("SENTINEL").exists());
}

#[cfg(unix)]
#[test]
fn unix_non_utf8_paths_and_control_characters_are_not_lossily_split() {
    use std::os::unix::ffi::OsStringExt;
    let repo = Repo::new();
    let name = std::ffi::OsString::from_vec(b"repo-\xff\n".to_vec());
    let nested = repo.path.join(name);
    assert!(
        repo.command()
            .args([OsStr::new("init"), nested.as_os_str()])
            .output()
            .unwrap()
            .status
            .success()
    );
    fs::write(
        nested.join(std::ffi::OsString::from_vec(b"file-\xff\n".to_vec())),
        "x",
    )
    .unwrap();
    let text = success(inspect(&nested));
    counts(&text, 0, 0, 1);
    assert!(text.lines().next().unwrap().contains("\\xFF\\n"));
}

#[test]
fn linked_worktree_has_its_own_root_index_and_head() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let linked = repo.path.join("linked worktree");
    repo.git(&["worktree", "add", "-b", "linked", linked.to_str().unwrap()]);
    fs::write(linked.join("file"), "changed").unwrap();
    let text = success(inspect(&linked));
    assert!(text.contains(&format!(
        "Repository: {:?}\n",
        linked.canonicalize().unwrap()
    )));
    assert!(text.contains("Branch: \"linked\""));
    assert!(text.contains(&format!(
        "Common Git directory: {:?}\n",
        repo.path.join(".git").canonicalize().unwrap()
    )));
    counts(&text, 0, 1, 0);
}

#[test]
fn missing_commit_and_wrong_object_type_cannot_succeed() {
    let repo = Repo::new();
    repo.commit_file("file", "contents");
    let head = String::from_utf8(repo.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    let head = head.trim();
    fs::remove_file(
        repo.path
            .join(".git/objects")
            .join(&head[..2])
            .join(&head[2..]),
    )
    .unwrap();
    failure(inspect(&repo.path), 4, "Git could not complete");
    let blob = String::from_utf8(repo.git(&["hash-object", "file"]).stdout).unwrap();
    fs::write(repo.path.join(".git/HEAD"), blob).unwrap();
    let output = inspect(&repo.path);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

#[test]
fn special_index_states_are_rejected() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    for flag in ["--assume-unchanged", "--skip-worktree"] {
        repo.git(&["update-index", flag, "file"]);
        failure(inspect(&repo.path), 5, "index entries");
        repo.git(&["update-index", &flag.replacen("--", "--no-", 1), "file"]);
    }
    let head = String::from_utf8(repo.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    repo.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{},sub", head.trim()),
    ]);
    failure(inspect(&repo.path), 5, "submodules");
}

#[test]
fn unresolved_merge_is_explicitly_unsupported() {
    let repo = Repo::new();
    repo.commit_file("file", "base\n");
    repo.git(&["checkout", "-b", "other"]);
    repo.commit_file("file", "other\n");
    repo.git(&["checkout", "main"]);
    repo.commit_file("file", "main\n");
    assert!(
        !repo
            .command()
            .args(["merge", "other"])
            .output()
            .unwrap()
            .status
            .success()
    );
    failure(inspect(&repo.path), 5, "unmerged");
}

#[test]
fn sparse_checkout_and_partial_clone_configurations_are_unsupported() {
    let repo = Repo::new();
    repo.git(&["config", "core.sparseCheckout", "true"]);
    failure(inspect(&repo.path), 5, "sparse checkout");
    repo.git(&["config", "core.sparseCheckout", "false"]);
    repo.git(&["config", "remote.origin.promisor", "true"]);
    failure(inspect(&repo.path), 5, "partial clones");
}

#[test]
fn renames_are_counted_as_add_and_delete_without_similarity_inference() {
    let repo = Repo::new();
    repo.commit_file("before", "contents");
    repo.git(&["mv", "before", "after"]);
    counts(&success(inspect(&repo.path)), 2, 0, 0);
}

#[test]
fn ignored_files_are_explicitly_outside_coverage() {
    let repo = Repo::new();
    repo.commit_file(".gitignore", "ignored\n");
    fs::write(repo.path.join("ignored"), "private").unwrap();
    let text = success(inspect(&repo.path));
    counts(&text, 0, 0, 0);
    assert!(text.contains("ignored paths excluded"));
}

#[test]
fn missing_git_and_invalid_inspect_arguments_have_distinct_exits() {
    let repo = Repo::new();
    let empty_path = repo.path.join("empty-bin");
    fs::create_dir(&empty_path).unwrap();
    failure(
        Command::new(env!("CARGO_BIN_EXE_cortex"))
            .current_dir(&repo.path)
            .arg("inspect")
            .env("PATH", empty_path)
            .output()
            .unwrap(),
        3,
        "Git is unavailable",
    );
    for args in [
        vec!["inspect", "--repo"],
        vec!["inspect", "--repo", ""],
        vec!["inspect", "--token=private"],
        vec!["inspect", "--repo", ".", "extra"],
    ] {
        failure(
            Command::new(env!("CARGO_BIN_EXE_cortex"))
                .args(args)
                .output()
                .unwrap(),
            2,
            "unsupported arguments",
        );
    }
}

#[test]
fn inherited_git_overrides_cannot_redirect_or_trace_inspection() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let trace = repo.path.join("trace-secret");
    let text = success(
        Command::new(env!("CARGO_BIN_EXE_cortex"))
            .current_dir(&repo.path)
            .arg("inspect")
            .env("GIT_DIR", "/missing-private-path")
            .env("GIT_INDEX_FILE", "/missing-index")
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "core.bare")
            .env("GIT_CONFIG_VALUE_0", "true")
            .env("GIT_TRACE", &trace)
            .output()
            .unwrap(),
    );
    counts(&text, 0, 0, 0);
    assert!(!trace.exists());
}

#[cfg(unix)]
#[test]
fn repository_helpers_are_disabled_or_rejected_before_execution() {
    use std::os::unix::fs::PermissionsExt;
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let helper = repo.path.join(".git/helper");
    fs::write(&helper, "#!/bin/sh\ntouch SENTINEL\nexit 1\n").unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).unwrap();
    repo.git(&["config", "core.fsmonitor", helper.to_str().unwrap()]);
    repo.git(&["config", "core.hooksPath", ".git"]);
    fs::copy(&helper, repo.path.join(".git/post-index-change")).unwrap();
    fs::write(repo.path.join("file"), "modified to require status").unwrap();
    counts(&success(inspect(&repo.path)), 0, 1, 0);
    assert!(!repo.path.join("SENTINEL").exists());
    repo.git(&["config", "filter.danger.clean", helper.to_str().unwrap()]);
    fs::write(repo.path.join(".gitattributes"), "file filter=danger\n").unwrap();
    failure(inspect(&repo.path), 5, "content filters");
    assert!(!repo.path.join("SENTINEL").exists());
}

#[test]
fn actual_entry_limit_returns_no_partial_report() {
    let repo = Repo::new();
    for n in 0..10_001 {
        fs::write(repo.path.join(format!("f{n}")), "").unwrap();
    }
    failure(inspect(&repo.path), 6, "changed entries");
    repo.git(&["add", "."]);
    failure(inspect(&repo.path), 6, "tracked entries");
}

#[derive(Debug, PartialEq, Eq)]
struct State {
    contents: Vec<u8>,
    target: Option<PathBuf>,
    modified: std::time::SystemTime,
    readonly: bool,
    #[cfg(unix)]
    stat: (u64, u64, u32, u32, u32, i64, i64),
}
fn snapshot(root: &Path) -> BTreeMap<PathBuf, State> {
    fn visit(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, State>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        let state = State {
            contents: if metadata.is_file() {
                fs::read(path).unwrap()
            } else {
                Vec::new()
            },
            target: if metadata.is_symlink() {
                Some(fs::read_link(path).unwrap())
            } else {
                None
            },
            modified: metadata.modified().unwrap(),
            readonly: metadata.permissions().readonly(),
            #[cfg(unix)]
            stat: (
                metadata.dev(),
                metadata.ino(),
                metadata.mode(),
                metadata.uid(),
                metadata.gid(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            ),
        };
        result.insert(path.strip_prefix(root).unwrap().to_owned(), state);
        if metadata.is_dir() {
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
fn repeated_actual_inspections_preserve_contents_and_metadata() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    fs::write(repo.path.join("file"), "staged").unwrap();
    repo.git(&["add", "file"]);
    fs::write(repo.path.join("file"), "unstaged bigger").unwrap();
    fs::write(repo.path.join("new"), "untracked").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("file", repo.path.join("link")).unwrap();
    let before = snapshot(&repo.path);
    let first = success(inspect(&repo.path));
    for _ in 0..3 {
        assert_eq!(success(inspect(&repo.path)), first);
    }
    assert_eq!(snapshot(&repo.path), before);
}

#[cfg(unix)]
#[test]
fn inaccessible_input_and_tracked_content_cannot_report_clean() {
    use std::os::unix::fs::PermissionsExt;
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    fs::write(repo.path.join("file"), "changed").unwrap();
    fs::set_permissions(repo.path.join("file"), fs::Permissions::from_mode(0o0)).unwrap();
    let output = inspect(&repo.path);
    fs::set_permissions(repo.path.join("file"), fs::Permissions::from_mode(0o600)).unwrap();
    // Git may identify a modification from stat information without reading the
    // bytes. A truthful dirty observation is valid; clean would be incorrect.
    if output.status.success() {
        counts(&success(output), 0, 1, 0);
    } else {
        failure(output, 4, "Git could not complete");
    }
    let denied = repo.path.join("denied");
    fs::create_dir(&denied).unwrap();
    fs::write(denied.join("hidden"), "untracked").unwrap();
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o0)).unwrap();
    assert_eq!(
        fs::read_dir(&denied).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied,
        "permission tests require a non-root user"
    );
    let root_output = inspect(&repo.path);
    let output = inspect(&denied.join("child"));
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o700)).unwrap();
    failure(output, 2, "PermissionDenied");
    failure(root_output, 4, "working-tree inspection");
}

#[test]
fn sha256_commit_is_resolved_without_abbreviation() {
    let repo = Repo::new();
    let sha = repo.path.join("sha256");
    repo.git(&[
        "init",
        "--object-format=sha256",
        "--initial-branch=main",
        sha.to_str().unwrap(),
    ]);
    let sha_repo = Repo { path: sha };
    sha_repo.git(&["config", "user.name", "Cortex Test"]);
    sha_repo.git(&["config", "user.email", "cortex-test@example.invalid"]);
    sha_repo.commit_file("file", "initial");
    let head = String::from_utf8(sha_repo.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    assert_eq!(head.trim().len(), 64);
    assert!(success(inspect(&sha_repo.path)).contains(&format!("HEAD: {head}")));
}

#[test]
fn clean_stat_refresh_and_linked_metadata_remain_read_only() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let linked = repo.path.join("linked");
    repo.git(&["worktree", "add", "-b", "linked", linked.to_str().unwrap()]);
    // Same contents, changed mtime: ordinary git status may refresh the index.
    let file = fs::OpenOptions::new()
        .write(true)
        .open(linked.join("file"))
        .unwrap();
    file.set_modified(
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000),
    )
    .unwrap();
    drop(file);
    let before = snapshot(&repo.path);
    for _ in 0..3 {
        counts(&success(inspect(&linked)), 0, 0, 0);
    }
    assert_eq!(snapshot(&repo.path), before);
}

#[test]
fn oversized_git_output_fails_without_exposing_configuration() {
    use std::io::Write;
    let repo = Repo::new();
    let mut config = fs::OpenOptions::new()
        .append(true)
        .open(repo.path.join(".git/config"))
        .unwrap();
    writeln!(
        config,
        "[cortex]\nprivate = {}",
        "x".repeat(4 * 1024 * 1024)
    )
    .unwrap();
    drop(config);
    let output = inspect(&repo.path);
    assert!(output.stderr.len() < 512);
    failure(output, 6, "Git output");
}

#[cfg(target_os = "linux")]
#[test]
fn actual_cli_timeout_kills_and_reaps_the_direct_process() {
    use std::{
        os::unix::fs::PermissionsExt,
        time::{Duration, Instant},
    };
    let repo = Repo::new();
    let fake_git = repo.path.join("git");
    let pid_file = repo.path.join("pid");
    fs::write(
        &fake_git,
        "#!/bin/sh\nprintf '%s' \"$$\" > \"$CORTEX_TEST_PID_FILE\"\nwhile :; do :; done\n",
    )
    .unwrap();
    fs::set_permissions(&fake_git, fs::Permissions::from_mode(0o755)).unwrap();
    let start = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .arg("inspect")
        .current_dir(&repo.path)
        .env("PATH", &repo.path)
        .env("CORTEX_TEST_PID_FILE", &pid_file)
        .output()
        .unwrap();
    failure(output, 6, "Git time");
    assert!(start.elapsed() < Duration::from_secs(10));
    let pid = fs::read_to_string(pid_file).unwrap();
    assert!(
        !Path::new("/proc").join(pid).exists(),
        "direct child must be reaped"
    );
}

#[cfg(unix)]
#[test]
fn failing_git_diagnostics_do_not_echo_private_output() {
    use std::os::unix::fs::PermissionsExt;
    let repo = Repo::new();
    let fake_git = repo.path.join("git");
    fs::write(
        &fake_git,
        "#!/bin/sh\nprintf 'private-token-example' >&2\nexit 42\n",
    )
    .unwrap();
    fs::set_permissions(&fake_git, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .arg("inspect")
        .current_dir(&repo.path)
        .env("PATH", &repo.path)
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&output.stderr).contains("private-token-example"));
    failure(output, 4, "Git version discovery");
}

#[test]
fn nested_untracked_repository_cannot_hide_its_contents() {
    let repo = Repo::new();
    repo.git(&["init", "--initial-branch=main", "nested"]);
    fs::write(repo.path.join("nested/file"), "new").unwrap();
    failure(inspect(&repo.path), 5, "nested repository");
}

#[test]
fn local_tracking_headers_work_without_remote_access() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    repo.git(&[
        "remote",
        "add",
        "origin",
        "https://example.invalid/never-accessed",
    ]);
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    repo.git(&["branch", "--set-upstream-to=origin/main", "main"]);
    let text = success(inspect(&repo.path));
    counts(&text, 0, 0, 0);
    assert!(text.contains("Branch: \"main\""));
    repo.commit_file("file", "ahead");
    counts(&success(inspect(&repo.path)), 0, 0, 0);
}
