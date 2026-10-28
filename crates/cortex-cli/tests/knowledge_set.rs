//! CAP-KNW-001 / 003-B: actual committed Git source sets, immutable legacy
//! extraction, exact-byte endpoints, resource/failure boundaries and contextual validity.
#![cfg(target_os = "linux")]
mod support;
use cortex_cli::{
    inspect, inspect_candidate,
    knowledge::{self, ExtractionError, source_set::*},
};
use cortex_core::{
    CandidateId, TaskContract, TaskId,
    binding::*,
    knowledge::{source_set::*, *},
};
use cortex_evidence::unified::Validity;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::MetadataExt,
    },
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use support::Repo;

const A: &str = r##" {"Id":0,"a":{"$ref":"b.json#/Id"},"b":{"$ref":"b.json"},"c":{"$ref":"b.json#"},"d":{"$ref":"#"},"e":{"$ref":"#/Id"}} "##;
const B: &str = r##"{"Id":1,"$ref":"a.json#/Id"}"##;
fn id(n: u64) -> ObservationId {
    ObservationId::new("source-set", n).unwrap()
}
fn path(p: &[u8]) -> SourcePath {
    SourcePath::new(p).unwrap()
}
fn repo(files: &[(&str, &str)]) -> Repo {
    let r = Repo::new();
    for (name, content) in files {
        let p = r.path.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }
    r.git(&["add", "."]);
    r.git(&["commit", "-m", "synthetic sources"]);
    r
}
fn contract(r: &Repo) -> CandidateBindingContract {
    let commit = String::from_utf8(r.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("knowledge.set").unwrap(),
            "Inspect explicit source set",
            "Retain cross-document endpoints and provenance",
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
fn extract(
    r: &Repo,
    c: &CandidateBindingContract,
    names: &[&[u8]],
) -> Result<ReferenceSet, SourceSetExtractionError> {
    extract_json_source_set(
        &r.path,
        c,
        id(1),
        names.iter().map(|p| path(p)).collect(),
        UNIX_EPOCH,
    )
}
#[derive(Debug, PartialEq, Eq)]
struct Fingerprint {
    bytes: Vec<u8>,
    modified: SystemTime,
    len: u64,
    mode: u32,
}
fn snapshot(r: &Repo) -> BTreeMap<PathBuf, Fingerprint> {
    let mut pending = vec![r.path.clone()];
    let mut result = BTreeMap::new();
    while let Some(p) = pending.pop() {
        let m = fs::symlink_metadata(&p).unwrap();
        let bytes = if m.is_symlink() {
            fs::read_link(&p).unwrap().as_os_str().as_bytes().to_vec()
        } else if m.is_dir() {
            pending.extend(fs::read_dir(&p).unwrap().map(|e| e.unwrap().path()));
            vec![]
        } else {
            fs::read(&p).unwrap()
        };
        result.insert(
            p.strip_prefix(&r.path).unwrap().to_owned(),
            Fingerprint {
                bytes,
                modified: m.modified().unwrap(),
                len: m.len(),
                mode: m.mode(),
            },
        );
    }
    result
}
fn assert_collection_limit(result: Result<ReferenceSet, SourceSetExtractionError>) {
    assert!(
        matches!(
            result,
            Err(SourceSetExtractionError::Collection(SourceSetError::Limit))
        ),
        "{result:?}"
    );
}

#[test]
fn exact_endpoints_spans_cycles_and_order_preserve_the_single_source_contract_read_only() {
    let r = repo(&[("a.json", A), ("b.json", B)]);
    let c = contract(&r);
    let before = snapshot(&r);
    let set = extract(&r, &c, &[b"b.json", b"a.json"]).unwrap();
    let ordered = extract(&r, &c, &[b"a.json", b"b.json"]).unwrap();
    assert_eq!(set, ordered);
    assert_eq!(
        set.sources()
            .iter()
            .map(|s| s.path().bytes())
            .collect::<Vec<_>>(),
        vec![b"a.json".as_slice(), b"b.json"]
    );
    assert_eq!(set.candidate().baseline(), &id(1));
    assert_eq!(set.candidate().observation().id(), &id(2));
    assert_eq!(set.sources()[0].id(), &id(3));
    assert_eq!(set.sources()[1].id(), &id(4));
    assert_eq!(set.id(), &id(5));
    assert_eq!(set.references().len(), 6);
    assert!(set.complete());
    let incoming: Vec<_> = set.incoming(&path(b"b.json"), "/Id").unwrap().collect();
    assert_eq!(incoming.len(), 1);
    let occurrence = incoming[0];
    assert_eq!(occurrence.from().path().bytes(), b"a.json");
    assert_eq!(occurrence.from().node().locator(), "/a/$ref");
    let span = occurrence.from().node().span();
    assert_eq!(span, SourceSpan::new(21, 33).unwrap());
    assert_eq!(
        &occurrence.from().source().bytes()[span.start()..span.end()],
        br##""b.json#/Id""##
    );
    let Resolution::Resolved(target) = occurrence.target() else {
        panic!("resolved endpoint")
    };
    assert_eq!(target.path().bytes(), b"b.json");
    assert_eq!(target.node().locator(), "/Id");
    assert_eq!(
        set.incoming(&path(b"a.json"), "/Id")
            .unwrap()
            .map(|o| o.from().node().locator())
            .collect::<Vec<_>>(),
        vec!["/e/$ref", "/$ref"]
    );
    // Source byte ordering precedes locator ordering: a.json occurs before b.json.
    assert_eq!(
        set.incoming(&path(b"a.json"), "/Id")
            .unwrap()
            .map(|o| o.from().path().bytes())
            .collect::<Vec<_>>(),
        vec![b"a.json".as_slice(), b"b.json"]
    );
    assert_eq!(
        set.incoming(&path(b"b.json"), "")
            .unwrap()
            .map(|o| o.from().node().locator())
            .collect::<Vec<_>>(),
        vec!["/b/$ref", "/c/$ref"]
    );
    assert_eq!(set.incoming(&path(b"b.json"), "/$ref").unwrap().count(), 0);
    assert_eq!(
        set.incoming(&path(b"a.json"), "/absent").err(),
        Some(SourceSetError::MissingNode)
    );
    assert_eq!(
        set.incoming(&path(b"unselected.json"), "").err(),
        Some(SourceSetError::MissingSource)
    );
    let legacy =
        knowledge::extract_json_references(&r.path, &c, id(1), path(b"a.json"), UNIX_EPOCH)
            .unwrap();
    assert_eq!(legacy, set.sources()[0]);
    assert_eq!(knowledge::ADAPTER, "cortex.json-local-references");
    assert_eq!(
        knowledge::PROFILE,
        "v1;serde_json=1.0.154;raw_value;local-pointer-fragments;depth=32"
    );
    assert!(!legacy.complete());
    assert_eq!(
        legacy.references()[0].target(),
        &ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage)
    );
    assert_eq!(
        knowledge::reinspect(&r.path, &legacy, id(6))
            .unwrap()
            .validity(),
        None
    );
    assert_eq!(
        reinspect(&r.path, &set, id(6)).unwrap().validity(),
        Some(Validity::Current)
    );
    for o in set.references() {
        assert_eq!(o.reference().origin(), RelationOrigin::Extracted);
        assert_eq!(
            o.reference().disposition(),
            ValidationDisposition::Unverified
        );
        assert_eq!(o.from().source().provenance().source(), knowledge::ADAPTER);
        assert_eq!(o.from().source().provenance().version(), knowledge::PROFILE);
    }
    assert_eq!(snapshot(&r), before);
}

#[test]
fn nested_relative_paths_and_non_utf8_parent_identities_resolve_exactly() {
    let r = repo(&[
        ("dir/schema.json", r##"{"$ref":"models/user.json#/Id"}"##),
        ("dir/models/user.json", "{\"Id\":0}"),
    ]);
    let c = contract(&r);
    let set = extract(&r, &c, &[b"dir/schema.json", b"dir/models/user.json"]).unwrap();
    let o = set
        .incoming(&path(b"dir/models/user.json"), "/Id")
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(o.from().path().bytes(), b"dir/schema.json");
    let raw_dir = r.path.join(OsString::from_vec(b"raw-\xff".to_vec()));
    fs::create_dir(&raw_dir).unwrap();
    fs::write(raw_dir.join("a.json"), r##"{"$ref":"b.json#/Id"}"##).unwrap();
    fs::write(raw_dir.join("b.json"), "{\"Id\":1}").unwrap();
    r.git(&["add", "."]);
    r.git(&["commit", "-m", "raw parent"]);
    let c = contract(&r);
    let before = snapshot(&r);
    let set = extract(&r, &c, &[b"raw-\xff/b.json", b"raw-\xff/a.json"]).unwrap();
    assert_eq!(
        set.incoming(&path(b"raw-\xff/b.json"), "/Id")
            .unwrap()
            .next()
            .unwrap()
            .from()
            .path()
            .bytes(),
        b"raw-\xff/a.json"
    );
    assert_eq!(
        set.endpoint(&path(b"raw-\xef\xbf\xbd/b.json"), "/Id")
            .unwrap_err(),
        SourceSetError::MissingSource
    );
    assert_eq!(snapshot(&r), before);
}

#[test]
fn unsupported_forms_unselected_documents_and_missing_locators_stay_explicit() {
    use UnresolvedReference::*;
    let cases = [
        ("unselected.json#/Id", OutsideCoverage),
        ("common.json#/missing", MissingTarget),
        ("http://host/common.json#", OutsideCoverage),
        ("//host/common.json#", OutsideCoverage),
        ("/common.json", OutsideCoverage),
        ("common\\file.json", OutsideCoverage),
        ("common.json?query#/Id", OutsideCoverage),
        ("common%2ejson", OutsideCoverage),
        ("models//user.json", OutsideCoverage),
        ("./common.json", OutsideCoverage),
        ("models/../common.json", OutsideCoverage),
        ("common.json/", OutsideCoverage),
        ("common.json#anchor", UnsupportedFragment),
        ("common.json#/~2", UnsupportedFragment),
        ("common.json#/~", UnsupportedFragment),
        ("common.json#/%49d", UnsupportedFragment),
        ("common.json##", UnsupportedFragment),
        ("#/a#b", UnsupportedFragment),
        ("#anchor", UnsupportedFragment),
        ("#/%49d", UnsupportedFragment),
        ("", OutsideCoverage),
    ];
    let values: Vec<_> = cases
        .iter()
        .map(|(s, _)| serde_json::json!({"$ref":s}))
        .collect();
    let source = serde_json::to_string(&serde_json::json!({"items":values,"a#b":0})).unwrap();
    let r = repo(&[
        ("a.json", &source),
        ("common.json", "{\"Id\":0}"),
        ("unselected.json", "{\"Id\":1}"),
    ]);
    let c = contract(&r);
    let before = snapshot(&r);
    let set = extract(&r, &c, &[b"a.json", b"common.json"]).unwrap();
    assert_eq!(set.references().len(), cases.len());
    assert!(!set.complete());
    for o in set.references() {
        let (_, reason) = cases
            .iter()
            .find(|(s, _)| *s == o.reference().spelling())
            .unwrap();
        assert_eq!(o.target(), Resolution::Unresolved(*reason));
    }
    assert_eq!(reinspect(&r.path, &set, id(6)).unwrap().validity(), None);
    assert_eq!(
        set.incoming(&path(b"common.json"), "/Id").unwrap().count(),
        0
    );
    // The legacy extractor can resolve a literal '#' in a JSON key; projection
    // enforces the newly declared single-separator subset without changing it.
    let o = set
        .references()
        .find(|o| o.reference().spelling() == "#/a#b")
        .unwrap();
    assert_eq!(
        o.reference().target(),
        &ReferenceTarget::Local("/a#b".into())
    );
    assert_eq!(snapshot(&r), before);
}

#[test]
fn local_pointer_escapes_empty_keys_arrays_and_relative_names_have_positive_neighbours() {
    let a = r##"{"refs":[{"$ref":"common.json#/a~1b/~0"},{"$ref":"common.json#/"},{"$ref":"common.json#/list/0"},{"$ref":"#"},{"$ref":"#/refs/0/$ref"},{"$ref":"common.json#/list/01"}]}"##;
    let r = repo(&[
        ("a.json", a),
        ("common.json", r##"{"a/b":{"~":0},"":1,"list":[2]}"##),
    ]);
    let c = contract(&r);
    let set = extract(&r, &c, &[b"a.json", b"common.json"]).unwrap();
    for locator in ["/a~1b/~0", "/", "/list/0"] {
        assert_eq!(
            set.incoming(&path(b"common.json"), locator)
                .unwrap()
                .count(),
            1
        );
    }
    assert_eq!(set.incoming(&path(b"a.json"), "").unwrap().count(), 1);
    assert_eq!(
        set.incoming(&path(b"a.json"), "/refs/0/$ref")
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        set.references().last().unwrap().target(),
        Resolution::Unresolved(UnresolvedReference::MissingTarget)
    );
    let span = set.references().next().unwrap().from().node().span();
    assert_eq!(
        &set.sources()[0].bytes()[span.start()..span.end()],
        br##""common.json#/a~1b/~0""##
    );
}

#[test]
fn admission_fails_before_external_acquisition_for_bad_sets_time_and_overflow() {
    let r = repo(&[("a.json", "0")]);
    let c = contract(&r);
    let absent = r.path.join("absent-directory");
    let attempt = |first, paths, time| extract_json_source_set(&absent, &c, first, paths, time);
    assert!(matches!(
        attempt(id(1), vec![], UNIX_EPOCH),
        Err(SourceSetExtractionError::Collection(SourceSetError::Empty))
    ));
    assert!(matches!(
        attempt(id(1), vec![path(b"a"), path(b"a")], UNIX_EPOCH),
        Err(SourceSetExtractionError::Collection(
            SourceSetError::DuplicateSource
        ))
    ));
    assert_collection_limit(attempt(
        id(1),
        (0..9).map(|i| path(format!("{i}").as_bytes())).collect(),
        UNIX_EPOCH,
    ));
    assert!(matches!(
        attempt(id(u64::MAX - 2), vec![path(b"a")], UNIX_EPOCH),
        Err(SourceSetExtractionError::Collection(
            SourceSetError::InvalidProvenance
        ))
    ));
    assert!(matches!(
        attempt(id(1), vec![path(b"a")], UNIX_EPOCH - Duration::from_secs(1)),
        Err(SourceSetExtractionError::Extraction(
            ExtractionError::Domain(KnowledgeError::InvalidProvenance)
        ))
    ));
    assert!(ObservationId::new("invalid scope", 1).is_err());
    assert!(ObservationId::new("source-set", 0).is_err());
    assert!(matches!(
        attempt(id(1), vec![path(b"a")], UNIX_EPOCH),
        Err(SourceSetExtractionError::Extraction(
            ExtractionError::Inspection(_)
        ))
    ));
    // The last representable collection ID is accepted; only overflow rejects.
    let at_end = extract_json_source_set(
        &r.path,
        &c,
        id(u64::MAX - 3),
        vec![path(b"a.json")],
        UNIX_EPOCH,
    )
    .unwrap();
    assert_eq!(at_end.id(), &id(u64::MAX));
}

#[test]
fn eight_sources_and_aggregate_byte_boundary_are_read_only_with_real_git() {
    let padded = format!("0{}", " ".repeat(MAX_SOURCE_BYTES - 1));
    let r = repo(&[
        ("0", &padded),
        ("1", &padded),
        ("2", &padded),
        ("3", &padded),
        ("4", "0"),
        ("5", "0"),
        ("6", "0"),
        ("7", "0"),
    ]);
    let c = contract(&r);
    let before = snapshot(&r);
    let set = extract(&r, &c, &[b"0", b"1", b"2", b"3"]).unwrap();
    assert_eq!(
        set.sources().iter().map(|s| s.bytes().len()).sum::<usize>(),
        1_048_576
    );
    assert_collection_limit(extract(&r, &c, &[b"0", b"1", b"2", b"3", b"4"]));
    assert_eq!(snapshot(&r), before);
    for i in 0..4 {
        fs::write(r.path.join(format!("{i}")), "0").unwrap();
    }
    r.git(&["add", "."]);
    r.git(&["commit", "-m", "small eight"]);
    let c = contract(&r);
    let before = snapshot(&r);
    assert_eq!(
        extract(&r, &c, &[b"7", b"6", b"5", b"4", b"3", b"2", b"1", b"0"])
            .unwrap()
            .sources()
            .len(),
        8
    );
    assert_eq!(snapshot(&r), before);
}

#[test]
fn aggregate_node_and_reference_boundaries_are_not_conflated() {
    let nodes = format!("[{}]", vec!["0"; 4095].join(","));
    let r = repo(&[("0", &nodes), ("1", &nodes), ("2", "0")]);
    let c = contract(&r);
    let before = snapshot(&r);
    let set = extract(&r, &c, &[b"0", b"1"]).unwrap();
    assert_eq!(
        set.sources().iter().map(|s| s.nodes().len()).sum::<usize>(),
        8192
    );
    assert_collection_limit(extract(&r, &c, &[b"0", b"1", b"2"]));
    assert_eq!(snapshot(&r), before);
    let refs = format!("[{}]", vec![r##"{"$ref":"#"}"##; 1024].join(","));
    r.commit_file("0", &refs);
    r.commit_file("1", &refs);
    r.commit_file("2", r##"{"$ref":"#"}"##);
    let c = contract(&r);
    let before = snapshot(&r);
    let set = extract(&r, &c, &[b"0", b"1"]).unwrap();
    assert_eq!(set.references().len(), 2048);
    assert_eq!(
        set.sources().iter().map(|s| s.nodes().len()).sum::<usize>(),
        4098
    );
    assert_collection_limit(extract(&r, &c, &[b"0", b"1", b"2"]));
    assert_eq!(snapshot(&r), before);
}

#[test]
fn existing_per_source_limits_remain_enforced_inside_a_collection() {
    let r = repo(&[("a", "0"), ("b", "0")]);
    let cases = [
        format!("0{}", " ".repeat(MAX_SOURCE_BYTES)),
        format!("[{}]", vec!["0"; 4096].join(",")),
        format!("[{}]", vec![r##"{"$ref":"#"}"##; 1025].join(",")),
        format!("{}0{}", "[".repeat(33), "]".repeat(33)),
        format!("{{\"{}\":0}}", "k".repeat(4096)),
    ];
    for text in cases {
        r.commit_file("b", &text);
        let c = contract(&r);
        let before = snapshot(&r);
        let result = extract(&r, &c, &[b"a", b"b"]);
        assert!(
            matches!(
                result,
                Err(SourceSetExtractionError::Extraction(ExtractionError::Limit))
                    | Err(SourceSetExtractionError::Extraction(
                        ExtractionError::Inspection(cortex_cli::InspectionError::Limit(_))
                    ))
            ),
            "{result:?}"
        );
        assert_eq!(snapshot(&r), before);
    }
    let depth = format!("{}0{}", "[".repeat(32), "]".repeat(32));
    r.commit_file("b", &depth);
    assert!(extract(&r, &contract(&r), &[b"a", b"b"]).is_ok());
    r.commit_file("b", &format!("{{\"{}\":0}}", "k".repeat(4095)));
    assert!(extract(&r, &contract(&r), &[b"a", b"b"]).is_ok());

    // Independent locator-byte oracle from the common contract: the long-key
    // object contributes 2044, 126 children 2048 each and one child 2052.
    // Exactly 262144 locator bytes is accepted; lengthening one key adds one.
    for overflow in [false, true] {
        let mut members: Vec<_> = (0..126).map(|i| format!("\"{i:03}\":0")).collect();
        members.push(format!(
            "\"{}\":0",
            if overflow { "long-key" } else { "longkey" }
        ));
        let text = format!("{{\"{}\":{{{}}}}}", "p".repeat(2043), members.join(","));
        r.commit_file("b", &text);
        let result = extract(&r, &contract(&r), &[b"a", b"b"]);
        if overflow {
            assert!(matches!(
                result,
                Err(SourceSetExtractionError::Extraction(ExtractionError::Limit))
            ));
        } else {
            let set = result.unwrap();
            assert_eq!(
                set.sources()[1]
                    .nodes()
                    .iter()
                    .map(|n| n.locator().len())
                    .sum::<usize>(),
                262144
            );
        }
    }
}

#[test]
fn malformed_json_and_missing_sources_never_publish_partial_collections_or_private_diagnostics() {
    let r = repo(&[("a", "0"), ("b", "0")]);
    for (text, unsupported) in [
        (
            "{\"private-sentinel-924\":1,\"private-sentinel-924\":2}",
            false,
        ),
        ("{} trailing", false),
        ("{\"$ref\":42}", true),
    ] {
        r.commit_file("b", text);
        let c = contract(&r);
        let before = snapshot(&r);
        let error = extract(&r, &c, &[b"a", b"b"]).unwrap_err();
        if unsupported {
            assert!(matches!(
                error,
                SourceSetExtractionError::Extraction(ExtractionError::Unsupported)
            ));
        } else {
            assert!(matches!(
                error,
                SourceSetExtractionError::Extraction(ExtractionError::InvalidJson)
            ));
        }
        assert!(!format!("{error:?} {error}").contains("private-sentinel-924"));
        assert_eq!(snapshot(&r), before);
    }
    r.commit_file("b", "0");
    let c = contract(&r);
    assert!(matches!(
        extract(&r, &c, &[b"a", b"absent"]),
        Err(SourceSetExtractionError::Extraction(
            ExtractionError::Inspection(_)
        ))
    ));
    fs::write(r.path.join("b"), b"\xff").unwrap();
    r.git(&["add", "b"]);
    r.git(&["commit", "-m", "invalid utf8"]);
    assert!(matches!(
        extract(&r, &contract(&r), &[b"a", b"b"]),
        Err(SourceSetExtractionError::Extraction(
            ExtractionError::InvalidJson
        ))
    ));
}

#[test]
fn dirty_unborn_wrong_repository_and_candidate_mismatch_cannot_publish() {
    let r = repo(&[("a", "0"), ("b", "0")]);
    let c = contract(&r);
    r.commit_file("b", "1");
    assert!(matches!(
        extract(&r, &c, &[b"a", b"b"]),
        Err(SourceSetExtractionError::Extraction(
            ExtractionError::Candidate(_)
        ))
    ));
    let c = contract(&r);
    fs::write(r.path.join("b"), "2").unwrap();
    assert!(matches!(
        extract(&r, &c, &[b"a", b"b"]),
        Err(SourceSetExtractionError::Extraction(
            ExtractionError::Candidate(_)
        ))
    ));
    let unborn = Repo::new();
    assert!(matches!(
        extract(&unborn, &c, &[b"a"]),
        Err(SourceSetExtractionError::Extraction(
            ExtractionError::MissingCommit
        ))
    ));
    let other = repo(&[("a", "0"), ("b", "1")]);
    assert!(matches!(
        extract(&other, &c, &[b"a", b"b"]),
        Err(SourceSetExtractionError::Extraction(
            ExtractionError::Candidate(_)
        ))
    ));
}

#[test]
fn symlinks_and_missing_local_blobs_reject_without_fallback_or_mutation() {
    let r = repo(&[("a", "0"), ("b", "1")]);
    std::os::unix::fs::symlink("b", r.path.join("link")).unwrap();
    r.git(&["add", "link"]);
    r.git(&["commit", "-m", "symlink"]);
    let c = contract(&r);
    let before = snapshot(&r);
    assert!(matches!(
        extract(&r, &c, &[b"a", b"link"]),
        Err(SourceSetExtractionError::Extraction(
            ExtractionError::Inspection(cortex_cli::InspectionError::Unsupported(_))
        ))
    ));
    assert_eq!(snapshot(&r), before);
    let oid = String::from_utf8(r.git(&["rev-parse", "HEAD:b"]).stdout).unwrap();
    let oid = oid.trim();
    fs::remove_file(r.path.join(".git/objects").join(&oid[..2]).join(&oid[2..])).unwrap();
    let before = snapshot(&r);
    assert!(matches!(
        extract(&r, &c, &[b"a", b"b"]),
        Err(SourceSetExtractionError::Extraction(
            ExtractionError::Inspection(_)
        ))
    ));
    assert_eq!(snapshot(&r), before);
}

#[test]
fn actual_reinspection_is_later_and_distinguishes_incomplete_stale_and_current() {
    let r = repo(&[("a.json", A), ("b.json", B)]);
    let c = contract(&r);
    let set = extract(&r, &c, &[b"a.json", b"b.json"]).unwrap();
    let original = set.clone();
    assert_eq!(qualify(&set, None).unwrap().validity(), None);
    for n in 3..=5 {
        let view = reinspect(&r.path, &set, id(n)).unwrap();
        assert_eq!(view.validity(), Some(Validity::Stale));
        assert_eq!(
            view.correspondence(),
            &BindingDecision::Stale(BindingStaleness::ObservationOrder)
        );
    }
    let latest = inspect_candidate(&r.path, &c, id(6)).unwrap();
    assert_eq!(
        qualify(&set, Some(&latest)).unwrap().validity(),
        Some(Validity::Current)
    );
    let incomplete = CandidateObservation::new(
        id(7),
        &c,
        RepositoryState::new(
            latest.state().repository().cloned(),
            latest.state().head().clone(),
            vec![],
            ObservationCoverage::Incomplete,
            ObservationConsistency::Indeterminate,
        )
        .unwrap(),
        latest.basis().cloned(),
    );
    let view = qualify(&set, Some(&incomplete)).unwrap();
    assert_eq!(view.validity(), None);
    assert_eq!(
        view.correspondence(),
        &BindingDecision::Insufficient(BindingInsufficiency::IncompleteCoverage)
    );
    fs::write(r.path.join("b.json"), "0").unwrap();
    assert_eq!(
        reinspect(&r.path, &set, id(8)).unwrap().validity(),
        Some(Validity::Stale)
    );
    r.git(&["add", "b.json"]);
    r.git(&["commit", "-m", "new candidate"]);
    assert_eq!(
        reinspect(&r.path, &set, id(9)).unwrap().validity(),
        Some(Validity::Stale)
    );
    assert_eq!(set, original);
    assert_eq!(set.incoming(&path(b"b.json"), "/Id").unwrap().count(), 1);
}

#[test]
fn changed_collection_or_source_profiles_are_stale_and_provenance_is_retained() {
    let r = repo(&[("a.json", A), ("b.json", B)]);
    let c = contract(&r);
    let set = extract(&r, &c, &[b"a.json", b"b.json"]).unwrap();
    for source_profile_changed in [false, true] {
        let sources = set
            .sources()
            .iter()
            .map(|s| {
                ReferenceIndex::new(
                    s.candidate().clone(),
                    s.path().clone(),
                    s.bytes().to_vec(),
                    ExtractionRecord::new(
                        s.id().clone(),
                        knowledge::ADAPTER,
                        if source_profile_changed {
                            "old-source-rules"
                        } else {
                            knowledge::PROFILE
                        },
                        s.recorded_at(),
                    )
                    .unwrap(),
                    s.nodes().to_vec(),
                    s.references().to_vec(),
                )
                .unwrap()
            })
            .collect();
        let changed = ReferenceSet::new(
            sources,
            ExtractionRecord::new(
                set.id().clone(),
                ADAPTER,
                if source_profile_changed {
                    PROFILE
                } else {
                    "old-collection-rules"
                },
                UNIX_EPOCH,
            )
            .unwrap(),
        )
        .unwrap();
        let view = reinspect(&r.path, &changed, id(6)).unwrap();
        assert!(!view.profile_matches());
        assert_eq!(view.validity(), Some(Validity::Stale));
        assert_eq!(
            view.set()
                .incoming(&path(b"b.json"), "/Id")
                .unwrap()
                .count(),
            1
        );
        assert_eq!(view.set().provenance(), changed.provenance());
    }
    assert!(!format!("{set:?} {:?}", set.references().next().unwrap()).contains("b.json"));
}

#[test]
fn detached_committed_candidate_is_supported_without_repository_mutation() {
    let r = repo(&[("a.json", A), ("b.json", B)]);
    r.git(&["checkout", "--detach"]);
    let c = contract(&r);
    let before = snapshot(&r);
    let set = extract(&r, &c, &[b"a.json", b"b.json"]).unwrap();
    assert!(matches!(
        set.candidate().observation().state().head(),
        Head::Detached { .. }
    ));
    assert_eq!(
        reinspect(&r.path, &set, id(6)).unwrap().validity(),
        Some(Validity::Current)
    );
    assert_eq!(snapshot(&r), before);
}
