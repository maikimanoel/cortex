//! CAP-KNW-001 / 003-B Cargo declaration acquisition and source-grounding acceptance.
#![cfg(target_os = "linux")]
mod support;
use cortex_cli::{inspect, inspect_candidate, knowledge::cargo_manifest::*};
use cortex_core::{CandidateId, TaskContract, TaskId, binding::*, knowledge::*};
use cortex_evidence::unified::Validity;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use support::Repo;

const ROOT: &str = include_str!("../../../Cargo.toml");
const CRATE: &str = include_str!("../Cargo.toml");
const SOURCE: &str = r#"[package]
name = "demo"
version = "0.1.0"
[dependencies]
alias = { package = "actual", version = "^1.2", path = "../actual", optional = true, features = ["remote"] }
[features]
default = ["fast", "fast"]
fast = ["dep:alias", "alias/remote", "missing"]
"#;
fn id(n: u64) -> ObservationId {
    ObservationId::new("cargo-context", n).unwrap()
}
fn contract(r: &Repo) -> CandidateBindingContract {
    let commit = String::from_utf8(r.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("cargo.integration").unwrap(),
            "Extract Cargo declarations",
            "Retain exact source spans and explicit uncertainty",
        )
        .unwrap(),
        ContractRevision::new(1).unwrap(),
        CandidateRequirement::new(
            inspect(&r.path).unwrap().repository_identity().unwrap(),
            CandidateId::from_git_commit(commit.trim()).unwrap(),
            WorktreeRequirement::Clean,
        ),
    )
}
fn extract(r: &Repo, c: &CandidateBindingContract) -> Result<ReferenceIndex, ExtractionError> {
    extract_cargo_manifest_references(
        &r.path,
        c,
        id(1),
        SourcePath::new(b"Cargo.toml").unwrap(),
        UNIX_EPOCH,
    )
}
fn fixture(source: &str) -> (Repo, CandidateBindingContract) {
    let r = Repo::new();
    r.commit_file("Cargo.toml", source);
    let c = contract(&r);
    (r, c)
}
fn span<'a>(g: &'a ReferenceIndex, key: &str) -> &'a [u8] {
    let s = g.node(key).unwrap().span();
    &g.bytes()[s.start()..s.end()]
}
fn target(g: &ReferenceIndex, from: &str) -> ReferenceTarget {
    g.references()
        .iter()
        .find(|r| r.from() == from)
        .unwrap()
        .target()
        .clone()
}
fn outside() -> ReferenceTarget {
    ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage)
}
fn snapshot(r: &Repo) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime, u64, bool)> {
    let mut pending = vec![r.path.clone()];
    let mut result = BTreeMap::new();
    while let Some(path) = pending.pop() {
        let m = fs::symlink_metadata(&path).unwrap();
        let bytes = if m.is_symlink() {
            use std::os::unix::ffi::OsStrExt;
            fs::read_link(&path)
                .unwrap()
                .as_os_str()
                .as_bytes()
                .to_vec()
        } else if m.is_dir() {
            pending.extend(fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
            vec![]
        } else {
            fs::read(&path).unwrap()
        };
        result.insert(
            path.strip_prefix(&r.path).unwrap().to_owned(),
            (
                bytes,
                m.modified().unwrap(),
                m.len(),
                m.permissions().readonly(),
            ),
        );
    }
    result
}
#[test]
fn committed_real_root_and_crate_styles_have_literal_spans_and_external_targets() {
    for source in [
        ROOT,
        CRATE,
        include_str!("../../cortex-core/Cargo.toml"),
        include_str!("../../cortex-evidence/Cargo.toml"),
    ] {
        let (r, c) = fixture(source);
        let before = snapshot(&r);
        let g = extract(&r, &c).unwrap();
        assert_eq!(g.bytes(), source.as_bytes());
        assert_eq!(g, extract(&r, &c).unwrap());
        assert_eq!(snapshot(&r), before);
        if source == ROOT {
            assert_eq!(span(&g, "/k:workspace/k:package/k:version"), br#""0.1.0""#);
            assert_eq!(
                span(&g, "/k:workspace/k:members/i:0"),
                br#""crates/cortex-core""#
            );
            assert_eq!(target(&g, "/k:workspace/k:members/i:0"), outside());
        } else {
            assert_eq!(span(&g, "/k:package/k:version/k:workspace"), b"true");
            assert_eq!(target(&g, "/k:package/k:version/k:workspace"), outside());
        }
        if source == CRATE {
            assert_eq!(
                span(&g, "/k:dependencies/k:cortex-core/k:path"),
                br#""../cortex-core""#
            );
            assert_eq!(
                span(&g, "/k:dependencies/k:serde_json/k:version"),
                br#""1.0.154""#
            );
            assert_eq!(span(&g, "/k:bin/i:0/k:name"), br#""cortex""#);
            assert_eq!(
                target(
                    &g,
                    "/k:target/k:cfg(target_os = \"linux\")/k:dependencies/k:serde/k:version"
                ),
                outside()
            );
        }
    }
}
#[test]
fn explicit_feature_endpoints_are_literal_and_duplicate_occurrences_keep_distinct_spans() {
    let (r, c) = fixture(SOURCE);
    let g = extract(&r, &c).unwrap();
    assert_eq!(span(&g, "/k:dependencies/k:alias/@key"), b"alias");
    assert_eq!(span(&g, "/k:dependencies/k:alias/k:version"), br#""^1.2""#);
    assert_eq!(
        span(&g, "/k:dependencies/k:alias/k:path"),
        br#""../actual""#
    );
    assert_eq!(
        span(&g, "/k:dependencies/k:alias/k:package"),
        br#""actual""#
    );
    assert_eq!(span(&g, "/k:dependencies/k:alias/k:optional"), b"true");
    assert_eq!(
        target(&g, "/k:features/k:default/i:0"),
        ReferenceTarget::Local("/k:features/k:fast".into())
    );
    assert_eq!(
        target(&g, "/k:features/k:fast/i:0"),
        ReferenceTarget::Local("/k:dependencies/k:alias".into())
    );
    assert_eq!(target(&g, "/k:features/k:fast/i:1"), outside());
    assert_eq!(
        target(&g, "/k:features/k:fast/i:2"),
        ReferenceTarget::Unresolved(UnresolvedReference::MissingTarget)
    );
    assert_eq!(g.incoming("/k:features/k:fast").unwrap().count(), 2);
    assert_ne!(
        g.node("/k:features/k:default/i:0").unwrap().span(),
        g.node("/k:features/k:default/i:1").unwrap().span()
    );
    assert_eq!(reinspect(&r.path, &g, id(4)).unwrap().validity(), None);
    assert!(
        g.references()
            .iter()
            .all(|r| r.origin() == RelationOrigin::Extracted
                && r.disposition() == ValidationDisposition::Unverified)
    );
}
#[test]
fn aliases_targets_workspace_and_partial_inheritance_cannot_become_effective_dependencies() {
    let source = r#"[package]
name = "x"
version.workspace = true
[dependencies]
alias = { package = "actual", workspace = true, optional = true }
same = { version = "1", optional = true }
[target.'cfg(unknown())'.dependencies]
same = { version = "2", optional = true }
[features]
all = ["dep:alias", "dep:same", "actual", "alias", "same?/x"]
"#;
    let (r, c) = fixture(source);
    let g = extract(&r, &c).unwrap();
    for i in 0..5 {
        assert!(matches!(
            target(&g, &format!("/k:features/k:all/i:{i}")),
            ReferenceTarget::Unresolved(_)
        ));
    }
    assert_ne!(
        span(&g, "/k:dependencies/k:same/k:version"),
        span(
            &g,
            "/k:target/k:cfg(unknown())/k:dependencies/k:same/k:version"
        )
    );
    assert!(
        g.references()
            .iter()
            .all(|r| matches!(r.target(), ReferenceTarget::Unresolved(_)))
    );
    // A feature table without an explicit package scope never proves local activation.
    let (r, c) = fixture("[features]\na = ['b']\nb = []\n");
    let g = extract(&r, &c).unwrap();
    assert_eq!(target(&g, "/k:features/k:a/i:0"), outside());
}
#[test]
fn quoted_unicode_escaped_keys_values_and_fake_comments_have_exact_bytes() {
    let source = "# [dependencies.fake]\r\n[package]\r\nname = 'ação'\r\ndescription = '[dependencies.fake] bad = 1'\r\n[features]\r\n\"a/b~ç\" = []\r\n\"ação\" = []\r\n\"\\u0061\" = [\"ação\"] # dep:fake\r\n";
    let (r, c) = fixture(source);
    let g = extract(&r, &c).unwrap();
    assert_eq!(g.references().len(), 1);
    assert_eq!(
        span(&g, "/k:features/k:a~1b~0ç/@key"),
        "\"a/b~ç\"".as_bytes()
    );
    assert_eq!(span(&g, "/k:features/k:a/@key"), br#""\u0061""#);
    assert_eq!(
        target(&g, "/k:features/k:a/i:0"),
        ReferenceTarget::Local("/k:features/k:ação".into())
    );
    assert_eq!(
        reinspect(&r.path, &g, id(4)).unwrap().validity(),
        Some(Validity::Current)
    );
}
#[test]
fn ordinary_nested_and_repeated_tables_are_syntax_nodes_not_invented_packages() {
    let source = "[[bin]]\nname='one'\n[[bin]]\nname='two'\n[package]\nname='p'\n[package.metadata]\nmatrix=[[1,2],[3,4]]\n[dependencies.dep]\nversion='1'\noptional=true\n[features]\nf=['dep:dep']\n";
    let (r, c) = fixture(source);
    let g = extract(&r, &c).unwrap();
    assert_eq!(span(&g, "/k:bin/i:1/k:name"), b"'two'");
    assert_eq!(span(&g, "/k:package/k:metadata/k:matrix/i:1/i:0"), b"3");
    assert_eq!(span(&g, "/k:dependencies/k:dep"), b"[dependencies.dep]");
    assert_eq!(
        target(&g, "/k:features/k:f/i:0"),
        ReferenceTarget::Local("/k:dependencies/k:dep".into())
    );
}
#[test]
fn dirty_replaced_deleted_wrong_repository_and_sequence_overflow_refuse() {
    let (r, c) = fixture(SOURCE);
    let old = extract(&r, &c).unwrap();
    for replacement in ["[package]\nname='replaced'", ""] {
        fs::write(r.path.join("Cargo.toml"), replacement).unwrap();
        assert!(matches!(
            extract(&r, &c),
            Err(ExtractionError::Candidate(_))
        ));
        assert_eq!(
            reinspect(&r.path, &old, id(4)).unwrap().validity(),
            Some(Validity::Stale)
        );
    }
    fs::remove_file(r.path.join("Cargo.toml")).unwrap();
    assert!(extract(&r, &c).is_err());
    assert_eq!(old.bytes(), SOURCE.as_bytes());
    let (other, _) = fixture(SOURCE);
    assert!(matches!(
        extract(&other, &c),
        Err(ExtractionError::Candidate(_))
    ));
    assert!(matches!(
        extract_cargo_manifest_references(
            &other.path,
            &c,
            id(u64::MAX - 1),
            SourcePath::new(b"Cargo.toml").unwrap(),
            UNIX_EPOCH
        ),
        Err(ExtractionError::Limit)
    ));
}
#[test]
fn detached_literal_and_non_utf8_paths_are_supported_without_source_or_git_mutation() {
    let (r, _) = fixture(SOURCE);
    r.git(&["checkout", "--detach"]);
    let c = contract(&r);
    let before = snapshot(&r);
    let a = extract(&r, &c).unwrap();
    assert_eq!(a, extract(&r, &c).unwrap());
    assert!(matches!(
        a.candidate().observation().state().head(),
        Head::Detached { .. }
    ));
    assert_eq!(snapshot(&r), before);
    use std::os::unix::ffi::OsStringExt;
    let raw = std::ffi::OsString::from_vec(b"literal-[*]-\xff.toml".to_vec());
    fs::write(r.path.join(&raw), SOURCE).unwrap();
    assert!(
        r.command()
            .args(["add", "--"])
            .arg(&raw)
            .status()
            .unwrap()
            .success()
    );
    r.git(&["commit", "-m", "byte path"]);
    let c = contract(&r);
    let before = snapshot(&r);
    let g = extract_cargo_manifest_references(
        &r.path,
        &c,
        id(1),
        SourcePath::new(b"literal-[*]-\xff.toml").unwrap(),
        UNIX_EPOCH,
    )
    .unwrap();
    assert_eq!(g.bytes(), SOURCE.as_bytes());
    assert_eq!(snapshot(&r), before);
}
#[test]
fn missing_symlink_and_unborn_sources_never_follow_files_or_fallback() {
    let (r, c) = fixture(SOURCE);
    assert!(matches!(
        extract_cargo_manifest_references(
            &r.path,
            &c,
            id(1),
            SourcePath::new(b"missing.toml").unwrap(),
            UNIX_EPOCH
        ),
        Err(ExtractionError::Inspection(_))
    ));
    std::os::unix::fs::symlink("Cargo.toml", r.path.join("link.toml")).unwrap();
    r.git(&["add", "link.toml"]);
    r.git(&["commit", "-m", "link"]);
    let before = snapshot(&r);
    assert!(matches!(
        extract_cargo_manifest_references(
            &r.path,
            &contract(&r),
            id(1),
            SourcePath::new(b"link.toml").unwrap(),
            UNIX_EPOCH
        ),
        Err(ExtractionError::Inspection(_))
    ));
    assert_eq!(snapshot(&r), before);
    let unborn = Repo::new();
    assert!(matches!(
        extract(&unborn, &c),
        Err(ExtractionError::MissingCommit)
    ));
}
#[test]
fn candidate_and_profile_changes_explain_staleness_and_preserve_original_facts() {
    let (r, c) = fixture(SOURCE);
    let original = extract(&r, &c).unwrap();
    let old = ReferenceIndex::new(
        original.candidate().clone(),
        original.path().clone(),
        original.bytes().to_vec(),
        ExtractionRecord::new(
            ObservationId::new("old-cargo", 1).unwrap(),
            ADAPTER,
            "old-profile",
            UNIX_EPOCH,
        )
        .unwrap(),
        original.nodes().to_vec(),
        original.references().to_vec(),
    )
    .unwrap();
    let view = reinspect(&r.path, &old, id(4)).unwrap();
    assert!(!view.profile_matches());
    assert_eq!(view.validity(), Some(Validity::Stale));
    assert_eq!(view.index(), &old);
    assert_eq!(inspect_validity(&original, None).unwrap().validity(), None);
    let latest = inspect_candidate(&r.path, &c, id(4)).unwrap();
    assert!(
        inspect_validity(&original, Some(&latest))
            .unwrap()
            .profile_matches()
    );
    r.commit_file("Cargo.toml", "[package]\nname='changed'\n");
    let view = reinspect(&r.path, &original, id(5)).unwrap();
    assert_eq!(view.validity(), Some(Validity::Stale));
    assert!(!matches!(
        view.correspondence(),
        BindingDecision::Compatible(_)
    ));
    assert_eq!(original.bytes(), SOURCE.as_bytes());
}
#[test]
fn malformed_unsupported_duplicate_and_oversized_sources_refuse_read_only() {
    let (r, _) = fixture(SOURCE);
    for source in [
        "[dependencies]\nx = {version='1', version='2'}",
        "x='truncated",
        "[features]\nf=[false]",
        "x='''unsupported'''",
        "x=[1,2",
        "'x'=1\n\"\\u0078\"=2",
    ] {
        r.commit_file("Cargo.toml", source);
        let c = contract(&r);
        let before = snapshot(&r);
        let error = extract(&r, &c).unwrap_err();
        assert!(!format!("{error:?} {error}").contains("truncated"));
        assert_eq!(snapshot(&r), before);
    }
    r.commit_file(
        "Cargo.toml",
        &format!("#{}", "a".repeat(MAX_SOURCE_BYTES - 1)),
    );
    let c = contract(&r);
    assert!(extract(&r, &c).is_ok());
    r.commit_file("Cargo.toml", &format!("#{}", "a".repeat(MAX_SOURCE_BYTES)));
    assert!(matches!(
        extract(&r, &contract(&r)),
        Err(ExtractionError::Inspection(_))
    ));
}
#[test]
fn changed_commit_order_and_redacted_facts_remain_distinct_from_freshness() {
    let (r, c) = fixture(SOURCE);
    let g = extract(&r, &c).unwrap();
    assert!(!format!("{g:?} {:?} {:?}", g.nodes(), g.references()).contains("actual"));
    let reused = inspect_candidate(&r.path, &c, id(3)).unwrap();
    assert!(matches!(
        inspect_validity(&g, Some(&reused))
            .unwrap()
            .correspondence(),
        BindingDecision::Stale(BindingStaleness::ObservationOrder)
    ));
    let boundary = extract_cargo_manifest_references(
        &r.path,
        &c,
        id(u64::MAX - 2),
        SourcePath::new(b"Cargo.toml").unwrap(),
        UNIX_EPOCH,
    )
    .unwrap();
    assert_eq!(boundary.id().sequence(), u64::MAX);
    r.commit_file("Cargo.toml", "[package]\nname='next'\n");
    assert!(matches!(
        extract(&r, &c),
        Err(ExtractionError::Candidate(_))
    ));
}
#[test]
fn missing_committed_blob_and_worktree_symlink_replacement_have_no_fallback() {
    let (r, c) = fixture(SOURCE);
    fs::remove_file(r.path.join("Cargo.toml")).unwrap();
    std::os::unix::fs::symlink("outside.toml", r.path.join("Cargo.toml")).unwrap();
    let before = snapshot(&r);
    assert!(extract(&r, &c).is_err());
    assert_eq!(snapshot(&r), before);
    let (r, c) = fixture(SOURCE);
    let oid = String::from_utf8(r.git(&["rev-parse", "HEAD:Cargo.toml"]).stdout).unwrap();
    let oid = oid.trim();
    fs::remove_file(r.path.join(".git/objects").join(&oid[..2]).join(&oid[2..])).unwrap();
    let before = snapshot(&r);
    assert!(matches!(
        extract(&r, &c),
        Err(ExtractionError::Inspection(_))
    ));
    assert_eq!(snapshot(&r), before);
}
