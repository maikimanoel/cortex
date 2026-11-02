//! CAP-KNW-001: real committed Rust source, exact lexical spans and conservative targets.
#![cfg(target_os = "linux")]
mod support;
use cortex_cli::{
    inspect, inspect_candidate,
    knowledge::rust::{self, ExtractionError},
};
use cortex_core::{CandidateId, TaskContract, TaskId, binding::*, knowledge::*};
use cortex_evidence::unified::Validity;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use support::Repo;
fn id(n: u64) -> ObservationId {
    ObservationId::new("rust-context", n).unwrap()
}
fn contract(r: &Repo) -> CandidateBindingContract {
    let head = String::from_utf8(r.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("rust.knowledge").unwrap(),
            "Inspect Rust lexical structure",
            "Retain exact occurrences and refuse unsupported local proof",
        )
        .unwrap(),
        ContractRevision::new(1).unwrap(),
        CandidateRequirement::new(
            inspect(&r.path).unwrap().repository_identity().unwrap(),
            CandidateId::from_git_commit(head.trim()).unwrap(),
            WorktreeRequirement::Clean,
        ),
    )
}
fn extract(r: &Repo, c: &CandidateBindingContract) -> Result<ReferenceIndex, ExtractionError> {
    rust::extract_rust_references(
        &r.path,
        c,
        id(1),
        SourcePath::new(b"source.rs").unwrap(),
        UNIX_EPOCH,
    )
}
fn loc(kind: &str, name: &str, at: usize) -> String {
    format!("{kind}@{at:06}/{name}")
}
fn span(index: &ReferenceIndex, key: &str) -> Vec<u8> {
    let s = index.node(key).unwrap().span();
    index.bytes()[s.start()..s.end()].to_vec()
}
fn snapshot(r: &Repo) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime, u64, u32)> {
    use std::os::unix::fs::PermissionsExt;
    let mut pending = vec![r.path.clone()];
    let mut result = BTreeMap::new();
    while let Some(p) = pending.pop() {
        let m = fs::symlink_metadata(&p).unwrap();
        assert!(!m.is_symlink());
        let bytes = if m.is_dir() {
            pending.extend(fs::read_dir(&p).unwrap().map(|e| e.unwrap().path()));
            vec![]
        } else {
            fs::read(&p).unwrap()
        };
        result.insert(
            p.strip_prefix(&r.path).unwrap().to_owned(),
            (
                bytes,
                m.modified().unwrap(),
                m.len(),
                m.permissions().mode(),
            ),
        );
    }
    result
}
#[test]
fn exact_committed_structure_grouped_aliases_and_byte_spans_are_read_only() {
    let r = Repo::new();
    let source = "// ação\nmod models { pub struct Item; pub fn make() {} }\nuse self::models::{Item as Renamed, make as build};\nmod external;\n";
    r.commit_file("source.rs", source);
    let c = contract(&r);
    let before = snapshot(&r);
    let g = extract(&r, &c).unwrap();
    let module = loc("mod", "models", 14);
    let item = format!("{module}/{}", loc("struct", "Item", 34));
    let make = format!("{module}/{}", loc("fn", "make", 47));
    let use_item = loc("use", "Renamed", 78);
    let use_make = loc("use", "build", 95);
    let external = loc("mod", "external", 115);
    let external_ref = loc("mod-ref", "external", 115);
    let expected = [
        "".to_owned(),
        module.clone(),
        item.clone(),
        make.clone(),
        use_item.clone(),
        use_make.clone(),
        external.clone(),
        external_ref.clone(),
    ];
    let mut expected = expected.to_vec();
    expected.sort();
    assert_eq!(
        g.nodes().iter().map(|n| n.locator()).collect::<Vec<_>>(),
        expected.iter().map(String::as_str).collect::<Vec<_>>()
    );
    for (key, text) in [
        (&module, "models"),
        (&item, "Item"),
        (&make, "make"),
        (&use_item, "Item as Renamed"),
        (&use_make, "make as build"),
        (&external, "external"),
        (&external_ref, "external"),
    ] {
        assert_eq!(span(&g, key), text.as_bytes());
    }
    assert_eq!(g.incoming(&item).unwrap().count(), 1);
    assert_eq!(g.incoming(&make).unwrap().count(), 1);
    assert_eq!(g.references().len(), 3);
    assert_eq!(
        g.references()
            .iter()
            .find(|r| r.from() == use_item)
            .unwrap()
            .spelling(),
        "self::models::Item as Renamed"
    );
    assert!(g.references().iter().any(|r| r.from() == external_ref
        && r.target() == &ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage)));
    assert_eq!(g.bytes(), source.as_bytes());
    assert_eq!(g.provenance().source(), rust::ADAPTER);
    assert_eq!(g.provenance().version(), rust::PROFILE);
    assert_eq!(g.id(), &id(3));
    assert_eq!(g, extract(&r, &c).unwrap());
    let next =
        rust::extract_rust_references(&r.path, &c, id(10), g.path().clone(), UNIX_EPOCH).unwrap();
    assert_ne!(next.id(), g.id());
    assert_eq!(next.nodes(), g.nodes());
    assert_eq!(next.references(), g.references());
    assert_eq!(
        rust::reinspect(&r.path, &g, id(4)).unwrap().validity(),
        None
    );
    assert_eq!(snapshot(&r), before);
}
#[test]
fn comments_literals_attributes_signatures_and_opaque_macros_do_not_invent_items() {
    let r = Repo::new();
    let source = r####"//! use fake::Doc; fn fake_doc() {}
/* outer /* mod fake_nested; */ use fake::Comment; */
#![allow(dead_code)]
#[doc = "fn fake_attr() {} use fake::Attr;"]
pub(crate) struct Real<T> where T: Clone { value: T }
const TEXT: &str = r###"use fake::Raw; mod fake_raw; fn fake() {}"###;
const BYTES: &[u8] = br#"use fake::Bytes;"#;
const C: &core::ffi::CStr = cr#"mod fake_c;"#;
const ESCAPED: &str = "\" use fake::Escaped; ";
const CHAR: char = '\'';
pub async unsafe fn run<'a, T: Clone>(x: &'a T) -> Result<(), ()> where T: Send {
    let _: fn() = || {};
    let _ = b'\x41'; let _ = c"fn fake_cooked() {}";
    fn nested() {}
    use super::Outside as Unknown;
}
trait Trait<T> { type Out; const N: usize; fn method(&self) -> T; }
impl<T: Clone> Trait<T> for Real<T> { type Out = T; const N: usize = 1; fn method(&self) -> T { self.value.clone() } }
macro_rules! invented { () => { fn fake_expansion() {} use fake::Expansion; } }
invented!();
"####;
    r.commit_file("source.rs", source);
    let g = extract(&r, &contract(&r)).unwrap();
    assert!(g.nodes().iter().all(|n| !n.locator().contains("fake")));
    for expected in [
        "Real", "TEXT", "BYTES", "C", "ESCAPED", "CHAR", "run", "nested", "Trait", "Out", "N",
        "method",
    ] {
        assert!(
            g.nodes()
                .iter()
                .any(|n| n.locator().ends_with(&format!("/{expected}"))),
            "missing {expected}"
        );
    }
    assert_eq!(g.references().len(), 3); // one real use and two opaque macro occurrences
    assert!(
        g.references()
            .iter()
            .all(|r| matches!(r.target(), ReferenceTarget::Unresolved(_)))
    );
    assert!(
        g.references()
            .iter()
            .all(|r| r.disposition() == ValidationDisposition::Unverified
                && r.origin() == RelationOrigin::Extracted)
    );
}
#[test]
fn explicit_scope_paths_duplicates_and_unresolved_semantics_are_discriminated() {
    let r = Repo::new();
    let source = "struct Root; mod a { struct Same; mod b { use super::Same as Up; use super::super::Root as Top; } } mod c { struct Same; } use self::{a::{Same as One, b::{self as B}}, c::Same as Two}; use self::a::Same as Again; use self::a::Same as Again2; use crate::Root as Crate; use external::Root as Ext; use self::Missing as MissingAlias;";
    r.commit_file("source.rs", source);
    let g = extract(&r, &contract(&r)).unwrap();
    assert_eq!(g.references().len(), 10);
    let by_alias = |suffix: &str| {
        g.references()
            .iter()
            .find(|r| r.from().ends_with(suffix))
            .unwrap()
    };
    assert_eq!(by_alias("/One").target(), by_alias("/Again").target());
    assert_eq!(by_alias("/Again").target(), by_alias("/Again2").target());
    assert_ne!(by_alias("/Again").from(), by_alias("/Again2").from());
    assert_eq!(by_alias("/Up").target(), by_alias("/One").target());
    assert_ne!(by_alias("/One").target(), by_alias("/Two").target());
    for alias in ["/Top", "/B"] {
        assert!(matches!(
            by_alias(alias).target(),
            ReferenceTarget::Local(_)
        ));
    }
    for alias in ["/Crate", "/Ext"] {
        assert_eq!(
            by_alias(alias).target(),
            &ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage)
        );
    }
    assert_eq!(
        by_alias("/MissingAlias").target(),
        &ReferenceTarget::Unresolved(UnresolvedReference::MissingTarget)
    );
}
#[test]
fn ambiguity_import_chains_globs_attributes_and_macros_never_prove_a_local_target() {
    let r = Repo::new();
    for source in [
        "struct Same; fn Same() {} use self::Same as Alias;",
        "mod m { struct Item; struct Item; } use self::m::Item as Alias;",
        "struct Item; use unknown::Item; use self::Item as Alias;",
        "struct Item; use unknown::*; use self::Item as Alias;",
        "#[cfg(any())] struct Item; use self::Item as Alias;",
        "struct Item; unknown!(); use self::Item as Alias;",
        "mod outside; use self::outside::Item as Alias;",
        "struct Item; use self::Item as Imported; use self::Imported as Alias;",
    ] {
        r.commit_file("source.rs", source);
        let g = extract(&r, &contract(&r)).unwrap();
        let alias = g
            .references()
            .iter()
            .find(|r| r.from().ends_with("/Alias"))
            .unwrap();
        assert!(
            matches!(alias.target(), ReferenceTarget::Unresolved(_)),
            "{source}"
        );
    }
}
#[test]
fn current_stale_missing_observation_and_profile_qualification_retain_original_facts() {
    let r = Repo::new();
    r.commit_file("source.rs", "struct Item; use self::Item as Alias;");
    let c = contract(&r);
    let g = extract(&r, &c).unwrap();
    assert!(g.complete());
    assert_eq!(
        rust::reinspect(&r.path, &g, id(4)).unwrap().validity(),
        Some(Validity::Current)
    );
    assert_eq!(rust::qualify(&g, None).unwrap().validity(), None);
    assert_eq!(
        rust::reinspect(&r.path, &g, id(3)).unwrap().validity(),
        Some(Validity::Stale)
    );
    let old = ReferenceIndex::new(
        g.candidate().clone(),
        g.path().clone(),
        g.bytes().to_vec(),
        ExtractionRecord::new(
            ObservationId::new("old-rust", 1).unwrap(),
            rust::ADAPTER,
            "old-profile",
            UNIX_EPOCH,
        )
        .unwrap(),
        g.nodes().to_vec(),
        g.references().to_vec(),
    )
    .unwrap();
    let view = rust::reinspect(&r.path, &old, id(5)).unwrap();
    assert!(!view.profile_matches());
    assert_eq!(view.validity(), Some(Validity::Stale));
    assert!(
        !cortex_cli::knowledge::qualify(&g, Some(&inspect_candidate(&r.path, &c, id(6)).unwrap()))
            .unwrap()
            .profile_matches()
    );
    fs::write(r.path.join("source.rs"), "struct Changed;").unwrap();
    assert!(matches!(
        extract(&r, &c),
        Err(ExtractionError::Candidate(_))
    ));
    assert_eq!(
        rust::reinspect(&r.path, &g, id(7)).unwrap().validity(),
        Some(Validity::Stale)
    );
    r.git(&["add", "source.rs"]);
    r.git(&["commit", "-m", "changed source"]);
    assert_eq!(
        rust::reinspect(&r.path, &g, id(8)).unwrap().validity(),
        Some(Validity::Stale)
    );
    let new = extract(&r, &contract(&r)).unwrap();
    assert!(new.references().is_empty());
    assert_eq!(g.references().len(), 1);
}
#[test]
fn wrong_candidate_dirty_deleted_replaced_missing_and_symlink_sources_refuse() {
    let r = Repo::new();
    r.commit_file("source.rs", "struct Item;");
    let c = contract(&r);
    let other = Repo::new();
    other.commit_file("source.rs", "struct Item;");
    assert!(matches!(
        extract(&other, &c),
        Err(ExtractionError::Candidate(_))
    ));
    let unborn = Repo::new();
    assert!(matches!(
        extract(&unborn, &c),
        Err(ExtractionError::MissingCommit)
    ));
    fs::write(r.path.join("source.rs"), "struct Replaced;").unwrap();
    assert!(matches!(
        extract(&r, &c),
        Err(ExtractionError::Candidate(_))
    ));
    fs::remove_file(r.path.join("source.rs")).unwrap();
    assert!(matches!(
        extract(&r, &c),
        Err(ExtractionError::Candidate(_))
    ));
    std::os::unix::fs::symlink("missing", r.path.join("source.rs")).unwrap();
    assert!(matches!(
        extract(&r, &c),
        Err(ExtractionError::Candidate(_))
    ));
    r.git(&["add", "source.rs"]);
    r.git(&["commit", "-m", "symlink"]);
    assert!(matches!(
        extract(&r, &contract(&r)),
        Err(ExtractionError::Inspection(_))
    ));
    assert!(matches!(
        rust::extract_rust_references(
            &r.path,
            &contract(&r),
            id(1),
            SourcePath::new(b"missing.rs").unwrap(),
            UNIX_EPOCH
        ),
        Err(ExtractionError::Inspection(_))
    ));
}
#[test]
fn detached_literal_and_non_utf8_paths_work_without_assuming_crate_root() {
    use std::os::unix::ffi::OsStringExt;
    let r = Repo::new();
    r.commit_file("[ação]*.rs", "struct Item; use self::Item as Alias;");
    r.git(&["checkout", "--detach"]);
    fs::create_dir(r.path.join("sub")).unwrap();
    let g = rust::extract_rust_references(
        &r.path.join("sub"),
        &contract(&r),
        id(1),
        SourcePath::new("[ação]*.rs".as_bytes()).unwrap(),
        UNIX_EPOCH,
    )
    .unwrap();
    assert!(matches!(
        g.candidate().observation().state().head(),
        Head::Detached { .. }
    ));
    assert!(g.complete());
    let raw = std::ffi::OsString::from_vec(b"source-\xff.rs".to_vec());
    fs::write(r.path.join(&raw), "struct BytePath;").unwrap();
    assert!(
        r.command()
            .args(["add", "--"])
            .arg(&raw)
            .status()
            .unwrap()
            .success()
    );
    r.git(&["commit", "-m", "byte path"]);
    let g = rust::extract_rust_references(
        &r.path,
        &contract(&r),
        id(1),
        SourcePath::new(b"source-\xff.rs").unwrap(),
        UNIX_EPOCH,
    )
    .unwrap();
    assert_eq!(g.path().bytes(), b"source-\xff.rs");
}
#[test]
fn sequence_overflow_and_invalid_time_are_rejected_before_repository_io() {
    let r = Repo::new();
    r.commit_file("source.rs", "struct Item;");
    let c = contract(&r);
    let nonexistent = r.path.join("absent");
    for n in [u64::MAX, u64::MAX - 1] {
        assert!(matches!(
            rust::extract_rust_references(
                &nonexistent,
                &c,
                id(n),
                SourcePath::new(b"source.rs").unwrap(),
                UNIX_EPOCH
            ),
            Err(ExtractionError::Limit)
        ));
    }
    let max = rust::extract_rust_references(
        &r.path,
        &c,
        id(u64::MAX - 2),
        SourcePath::new(b"source.rs").unwrap(),
        UNIX_EPOCH,
    )
    .unwrap();
    assert_eq!(max.id().sequence(), u64::MAX);
    assert!(matches!(
        rust::extract_rust_references(
            &nonexistent,
            &c,
            id(1),
            SourcePath::new(b"source.rs").unwrap(),
            UNIX_EPOCH - std::time::Duration::from_secs(1)
        ),
        Err(ExtractionError::Domain(KnowledgeError::InvalidProvenance))
    ));
}
#[test]
fn malformed_unsupported_encoding_and_budget_failures_publish_no_index() {
    let r = Repo::new();
    for source in [
        "fn f() { incomplete! }",
        "macro_rules! incomplete",
        "struct A fn invented() {}",
        "mod m {",
        "use self::{Item,",
        "use self::;",
        "use self::{Item as};",
        "fn f(",
        "struct",
        "fn f",
        "/* unfinished",
        "const S: &str = r###\"unfinished;",
        "const S: &str = \"\\q\";",
        "fn f() {} trailing",
        "union",
        "#[cfg(any())]",
    ] {
        r.commit_file("source.rs", source);
        assert!(extract(&r, &contract(&r)).is_err(), "accepted {source}");
    }
    r.commit_file("source.rs", "macro future() {}");
    assert!(matches!(
        extract(&r, &contract(&r)),
        Err(ExtractionError::UnsupportedSyntax)
    ));
    r.commit_file("source.rs", "struct Ação;");
    assert!(matches!(
        extract(&r, &contract(&r)),
        Err(ExtractionError::UnsupportedSyntax)
    ));
    fs::write(r.path.join("source.rs"), b"\xff").unwrap();
    r.git(&["add", "source.rs"]);
    r.git(&["commit", "-m", "invalid encoding"]);
    let error = extract(&r, &contract(&r)).unwrap_err();
    assert!(matches!(error, ExtractionError::Encoding));
    assert!(!format!("{error:?} {error}").contains("source.rs"));
    r.commit_file("source.rs", &" ".repeat(MAX_SOURCE_BYTES + 1));
    assert!(matches!(
        extract(&r, &contract(&r)),
        Err(ExtractionError::Inspection(_))
    ));
}

#[test]
fn extern_crates_and_macro_names_keep_real_spans_and_never_expand() {
    let r = Repo::new();
    let source = "extern crate external as Renamed; macro_rules! actual { () => { fn invented() {} } } fn f() { let _ = external::make!(); }";
    r.commit_file("source.rs", source);
    let g = extract(&r, &contract(&r)).unwrap();
    assert_eq!(g.references().len(), 3);
    let mut observed: Vec<_> = g
        .references()
        .iter()
        .map(|r| (r.spelling(), span(&g, r.from())))
        .collect();
    observed.sort();
    assert_eq!(
        observed,
        [
            ("actual", b"actual".to_vec()),
            ("external as Renamed", b"external as Renamed".to_vec()),
            ("external::make", b"external::make".to_vec())
        ]
    );
    assert!(
        g.references()
            .iter()
            .all(|r| matches!(r.target(), ReferenceTarget::Unresolved(_)))
    );
    assert!(!g.nodes().iter().any(|n| n.locator().contains("invented")));
}
#[test]
fn invalid_reserved_identifiers_and_malformed_path_anchors_never_prove_local_targets() {
    let r = Repo::new();
    for source in [
        "struct fn;",
        "struct r#self;",
        "use self::Item as fn;",
        "struct Item; mod m { use self::super::Item as Alias; }",
        "mod m {} use self::m::self as Alias;",
    ] {
        r.commit_file("source.rs", source);
        match extract(&r, &contract(&r)) {
            Err(ExtractionError::InvalidRust | ExtractionError::UnsupportedSyntax) => {}
            Ok(index) => assert!(
                index
                    .references()
                    .iter()
                    .all(|r| matches!(r.target(), ReferenceTarget::Unresolved(_)))
            ),
            _ => panic!("unexpected refusal category"),
        }
    }
}

#[test]
fn real_git_producer_refuses_whole_batches_at_node_and_reference_boundaries() {
    let r = Repo::new();
    r.commit_file("source.rs", &"struct A;".repeat(MAX_NODES - 1));
    let g = extract(&r, &contract(&r)).unwrap();
    assert_eq!(g.nodes().len(), MAX_NODES);
    r.commit_file("source.rs", &"struct A;".repeat(MAX_NODES));
    assert!(matches!(
        extract(&r, &contract(&r)),
        Err(ExtractionError::Limit)
    ));
    r.commit_file(
        "source.rs",
        &"use external::Item as Alias;".repeat(MAX_REFERENCES),
    );
    let g = extract(&r, &contract(&r)).unwrap();
    assert_eq!(g.references().len(), MAX_REFERENCES);
    r.commit_file(
        "source.rs",
        &"use external::Item as Alias;".repeat(MAX_REFERENCES + 1),
    );
    assert!(matches!(
        extract(&r, &contract(&r)),
        Err(ExtractionError::Limit)
    ));
    r.commit_file("source.rs", &" ".repeat(MAX_SOURCE_BYTES));
    let g = extract(&r, &contract(&r)).unwrap();
    assert_eq!(g.bytes().len(), MAX_SOURCE_BYTES);
}

#[test]
fn multiline_commented_use_spans_retain_original_utf8_trivia_without_fake_occurrences() {
    let r = Repo::new();
    let source = "mod m { struct Item; }\nuse self /* fn fake() {} */ :: m::{\n Item /* ação use fake::X; */ as Alias\n};\n";
    r.commit_file("source.rs", source);
    let g = extract(&r, &contract(&r)).unwrap();
    assert_eq!(g.references().len(), 1);
    let reference = &g.references()[0];
    assert_eq!(reference.spelling(), "self::m::Item as Alias");
    assert_eq!(
        span(&g, reference.from()),
        "Item /* ação use fake::X; */ as Alias".as_bytes()
    );
    assert!(matches!(reference.target(), ReferenceTarget::Local(_)));
    assert!(g.nodes().iter().all(|n| !n.locator().contains("fake")));
}
