//! PP01 / CAP-KNW-001 / 003-B: independent full-rebuild transition oracles.
//! Literal graphs and half-open byte offsets below describe deliberately small
//! committed fixtures. No expected target is obtained from a production resolver.
//! Extraction remains lexical, Extracted/Unverified, without assessment or authority.
#![cfg(target_os = "linux")]

mod support;

use cortex_cli::{
    InspectionError, inspect, inspect_candidate,
    knowledge::{ExtractionError, source_set::*},
};
use cortex_core::{
    CandidateId, TaskContract, TaskId,
    binding::*,
    knowledge::{source_set::*, *},
    temporal::SourceReference,
};
use cortex_evidence::unified::Validity;
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    os::unix::fs::MetadataExt,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use support::Repo;

const A_ID: &str = r##"{"link":{"$ref":"b.json#/Id"}}"##;
const A_OTHER: &str = r##"{"link":{"$ref":"b.json#/Other"}}"##;
const A_NONE: &str = r##"{"link":{}}"##;
const A_SPACE: &str = " \n{ \"link\" : { \"$ref\" : \"b.json#/Id\" } }\n";
const B_BOTH: &str = r#"{"Id":0,"Other":1}"#;
const B_ID: &str = r#"{"Id":0}"#;
const COVERAGE: &str = r##"[{"$ref":"b.json#/Id"},{"$ref":"b.json#anchor"},{"$ref":"https://example.invalid/x#/Id"},{"$ref":"absent.json#/Id"}]"##;

type Node<'a> = (&'a str, usize, usize);
const ID_NODES: &[Node<'_>] = &[("", 0, 30), ("/link", 8, 29), ("/link/$ref", 16, 28)];
const OTHER_NODES: &[Node<'_>] = &[("", 0, 33), ("/link", 8, 32), ("/link/$ref", 16, 31)];
const NONE_NODES: &[Node<'_>] = &[("", 0, 11), ("/link", 8, 10)];
const SPACE_NODES: &[Node<'_>] = &[("", 2, 40), ("/link", 13, 38), ("/link/$ref", 24, 36)];
const BOTH_NODES: &[Node<'_>] = &[("", 0, 18), ("/Id", 6, 7), ("/Other", 16, 17)];
const B_ID_NODES: &[Node<'_>] = &[("", 0, 8), ("/Id", 6, 7)];
const COVERAGE_NODES: &[Node<'_>] = &[
    ("", 0, 116),
    ("/0", 1, 22),
    ("/0/$ref", 9, 21),
    ("/1", 23, 47),
    ("/1/$ref", 31, 46),
    ("/2", 48, 88),
    ("/2/$ref", 56, 87),
    ("/3", 89, 115),
    ("/3/$ref", 97, 114),
];

#[derive(Debug, PartialEq, Eq)]
enum Target<'a> {
    Node(&'a str, &'a str),
    Unresolved(UnresolvedReference),
}
type Edge<'a> = (&'a str, &'a str, &'a str, Target<'a>);
const TO_ID: &[Edge<'_>] = &[(
    "a.json",
    "/link/$ref",
    "b.json#/Id",
    Target::Node("b.json", "/Id"),
)];
const TO_OTHER: &[Edge<'_>] = &[(
    "a.json",
    "/link/$ref",
    "b.json#/Other",
    Target::Node("b.json", "/Other"),
)];
const MISSING_OTHER: &[Edge<'_>] = &[(
    "a.json",
    "/link/$ref",
    "b.json#/Other",
    Target::Unresolved(UnresolvedReference::MissingTarget),
)];

fn id(sequence: u64) -> ObservationId {
    // Each test owns one isolated repository and one deterministic capture series.
    let current = std::thread::current();
    ObservationId::new(current.name().unwrap(), sequence).unwrap()
}

fn path(name: &str) -> SourcePath {
    SourcePath::new(name.as_bytes()).unwrap()
}

fn time(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn commit(repo: &Repo, files: &[(&str, &str)]) {
    for (name, bytes) in files {
        fs::write(repo.path.join(name), bytes).unwrap();
    }
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "explicit rebuild transition"]);
}

#[derive(Debug, PartialEq, Eq)]
struct FileState {
    bytes: Vec<u8>,
    modified: SystemTime,
    len: u64,
    mode: u32,
    directory: bool,
}

fn snapshot(repo: &Repo) -> BTreeMap<PathBuf, FileState> {
    // Includes .git/index, objects, refs, reflogs, config and directories.
    // Access time and inode/change/birth metadata are outside this oracle.
    // No status command runs here that might itself refresh the index.
    let mut pending = vec![repo.path.clone()];
    let mut result = BTreeMap::new();
    while let Some(p) = pending.pop() {
        let m = fs::symlink_metadata(&p).unwrap();
        assert!(!m.is_symlink());
        let directory = m.is_dir();
        let bytes = if directory {
            pending.extend(fs::read_dir(&p).unwrap().map(|entry| entry.unwrap().path()));
            vec![]
        } else {
            fs::read(&p).unwrap()
        };
        result.insert(
            p.strip_prefix(&repo.path).unwrap().to_owned(),
            FileState {
                bytes,
                modified: m.modified().unwrap(),
                len: m.len(),
                mode: m.mode(),
                directory,
            },
        );
    }
    result
}

fn read_only<T>(repo: &Repo, operation: impl FnOnce() -> T) -> T {
    let before = snapshot(repo);
    let result = operation();
    // Also compare returned failures before checking their categories.
    assert_eq!(snapshot(repo), before);
    result
}

fn contract(repo: &Repo, revision: u64) -> CandidateBindingContract {
    read_only(repo, || {
        let head = String::from_utf8(repo.git(&["rev-parse", "HEAD"]).stdout).unwrap();
        CandidateBindingContract::new(
            TaskContract::new(
                TaskId::new("knowledge.rebuild").unwrap(),
                "Rebuild explicitly selected committed JSON sources",
                "Retain literal endpoints, spans, coverage and actual capture provenance",
            )
            .unwrap(),
            ContractRevision::new(revision).unwrap(),
            CandidateRequirement::new(
                inspect(&repo.path).unwrap().repository_identity().unwrap(),
                CandidateId::from_git_commit(head.trim()).unwrap(),
                WorktreeRequirement::Clean,
            ),
        )
    })
}

fn extract(
    repo: &Repo,
    c: &CandidateBindingContract,
    first: u64,
    names: &[&str],
) -> Result<ReferenceSet, SourceSetExtractionError> {
    read_only(repo, || {
        extract_json_source_set(
            &repo.path,
            c,
            id(first),
            names.iter().map(|name| path(name)).collect(),
            time(first),
        )
    })
}

fn assert_capture(set: &ReferenceSet, c: &CandidateBindingContract, first: u64) {
    let n = set.sources().len() as u64;
    assert_eq!(set.id(), &id(first + n + 2));
    assert_eq!(set.recorded_at(), time(first));
    assert_eq!(set.provenance(), &SourceReference::observation_reference(
        id(first + n + 2), "cortex.json-source-set-references",
        "v1;relative-selected-documents;exact-bytes;single-fragment;paths=no-uri-no-dot;limits=8,1048576,8192,2048;source=cortex.json-local-references;source-profile=v1;serde_json=1.0.154;raw_value;local-pointer-fragments;depth=32",
    ).unwrap());
    let candidate = set.candidate();
    assert_eq!(candidate.baseline(), &id(first));
    let observed = candidate.observation();
    assert_eq!(observed.id(), &id(first + 1));
    assert_eq!(observed.contract(), c);
    assert_eq!(
        observed.state().repository(),
        Some(c.candidate().repository())
    );
    assert_eq!(
        observed.state().head(),
        &Head::Attached {
            branch: b"main".to_vec(),
            commit: c.candidate().commit().clone(),
        }
    );
    assert!(observed.state().entries().is_empty());
    assert_eq!(observed.state().coverage(), ObservationCoverage::GitStatus);
    assert_eq!(
        observed.state().consistency(),
        ObservationConsistency::TwoMatchingSamples
    );
    assert!(observed.basis().is_some());
    for (offset, source) in set.sources().iter().enumerate() {
        assert_eq!(source.candidate(), candidate);
        assert_eq!(source.id(), &id(first + offset as u64 + 2));
        assert_eq!(source.recorded_at(), time(first));
        assert_eq!(
            source.provenance(),
            &SourceReference::observation_reference(
                id(first + offset as u64 + 2),
                "cortex.json-local-references",
                "v1;serde_json=1.0.154;raw_value;local-pointer-fragments;depth=32",
            )
            .unwrap()
        );
    }
}

fn edges(set: &ReferenceSet) -> Vec<Edge<'_>> {
    set.references()
        .map(|o| {
            let target = match o.target() {
                Resolution::Resolved(t) => Target::Node(
                    std::str::from_utf8(t.path().bytes()).unwrap(),
                    t.node().locator(),
                ),
                Resolution::Unresolved(reason) => Target::Unresolved(reason),
            };
            (
                std::str::from_utf8(o.from().path().bytes()).unwrap(),
                o.from().node().locator(),
                o.reference().spelling(),
                target,
            )
        })
        .collect()
}

// Expected source order, node tables and edge tables are literal fixture facts.
// Incoming expectations are filtered from those literal edges, never resolved.
type Source<'a> = (&'a str, &'a str, &'a [Node<'a>], bool);
fn assert_graph(set: &ReferenceSet, sources: &[Source<'_>], expected: &[Edge<'_>], complete: bool) {
    assert_eq!(set.sources().len(), sources.len());
    assert_eq!(edges(set), expected);
    assert_eq!(set.complete(), complete);
    for (source, (name, bytes, nodes, legacy_complete)) in set.sources().iter().zip(sources) {
        assert_eq!(source.path(), &path(name));
        assert_eq!(source.bytes(), bytes.as_bytes());
        assert_eq!(source.complete(), *legacy_complete);
        assert_eq!(
            source
                .nodes()
                .iter()
                .map(|n| (n.locator(), n.span().start(), n.span().end(),))
                .collect::<Vec<_>>(),
            *nodes
        );
        for (locator, start, end) in *nodes {
            let endpoint = set.endpoint(&path(name), locator).unwrap();
            assert_eq!(
                endpoint.node().span(),
                SourceSpan::new(*start, *end).unwrap()
            );
            assert!(std::ptr::eq(endpoint.source(), source));
            let incoming = set
                .incoming(&path(name), locator)
                .unwrap()
                .map(|o| {
                    (
                        std::str::from_utf8(o.from().path().bytes()).unwrap(),
                        o.reference().from(),
                    )
                })
                .collect::<Vec<_>>();
            let expected_incoming = expected
                .iter()
                .filter_map(|(from, at, _, target)| {
                    (target == &Target::Node(name, locator)).then_some((*from, *at))
                })
                .collect::<Vec<_>>();
            assert_eq!(incoming, expected_incoming, "incoming {name}:{locator}");
        }
    }
    for o in set.references() {
        assert_eq!(o.reference().origin(), RelationOrigin::Extracted);
        assert_eq!(
            o.reference().disposition(),
            ValidationDisposition::Unverified
        );
        // Every fixture spelling is an unescaped ASCII JSON string.
        let span = o.from().node().span();
        assert_eq!(
            &o.from().source().bytes()[span.start()..span.end()],
            format!("\"{}\"", o.reference().spelling()).as_bytes()
        );
        assert_eq!(
            o.reference().target(),
            &ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage)
        );
    }
}

fn assert_validity(repo: &Repo, set: &ReferenceSet, sequence: u64, expected: Option<Validity>) {
    read_only(repo, || {
        let retained = set.clone();
        let view = reinspect(&repo.path, set, id(sequence)).unwrap();
        assert_eq!(view.validity(), expected);
        assert!(view.profile_matches());
        assert!(std::ptr::eq(view.set(), set));
        if let BindingDecision::Compatible(bound) = view.correspondence() {
            assert_eq!(bound.baseline(), set.candidate().observation().id());
            assert_eq!(bound.observation().id(), &id(sequence));
            assert_eq!(
                bound.observation().contract(),
                set.candidate().observation().contract()
            );
            assert_eq!(
                bound.observation().state(),
                set.candidate().observation().state()
            );
            assert_eq!(
                bound.observation().basis(),
                set.candidate().observation().basis()
            );
        }
        assert_eq!(set, &retained);
    });
}

#[test]
fn retarget_remove_target_remove_occurrence_and_restore_bytes_in_later_commits() {
    let repo = Repo::new();
    commit(&repo, &[("a.json", A_ID), ("b.json", B_BOTH)]);
    let mut retained = Vec::new();
    type Phase<'a> = (
        &'a str,
        &'a str,
        &'a [Node<'a>],
        &'a [Node<'a>],
        &'a [Edge<'a>],
        bool,
    );
    let phases: &[Phase<'_>] = &[
        (A_ID, B_BOTH, ID_NODES, BOTH_NODES, TO_ID, true),
        (A_OTHER, B_BOTH, OTHER_NODES, BOTH_NODES, TO_OTHER, true),
        (A_OTHER, B_ID, OTHER_NODES, B_ID_NODES, MISSING_OTHER, false),
        (A_NONE, B_ID, NONE_NODES, B_ID_NODES, &[], true),
        (A_ID, B_BOTH, ID_NODES, BOTH_NODES, TO_ID, true),
    ];
    for (phase, (a, b, a_nodes, b_nodes, expected, complete)) in phases.iter().enumerate() {
        if phase > 0 {
            commit(&repo, &[("a.json", a), ("b.json", b)]);
        }
        let c = contract(&repo, phase as u64 + 1);
        let first = phase as u64 * 10 + 1;
        let set = extract(&repo, &c, first, &["b.json", "a.json"]).unwrap();
        assert_capture(&set, &c, first);
        assert_graph(
            &set,
            &[
                ("a.json", a, a_nodes, phase == 3),
                ("b.json", b, b_nodes, true),
            ],
            expected,
            *complete,
        );
        assert_validity(
            &repo,
            &set,
            first + 5,
            complete.then_some(Validity::Current),
        );
        if phase == 2 || phase == 3 {
            assert_eq!(
                set.endpoint(&path("b.json"), "/Other").unwrap_err(),
                SourceSetError::MissingNode
            );
            assert_eq!(
                set.incoming(&path("b.json"), "/Other").err(),
                Some(SourceSetError::MissingNode)
            );
        }
        for (offset, old) in retained.iter().enumerate() {
            assert_validity(&repo, old, first + 6 + offset as u64, Some(Validity::Stale));
        }
        retained.push(set);
    }
    let old = &retained[0];
    let restored = &retained[4];
    assert_eq!(edges(old), edges(restored));
    for (original, rebuilt) in old.sources().iter().zip(restored.sources()) {
        assert_eq!(original.bytes(), rebuilt.bytes());
        assert_eq!(original.nodes(), rebuilt.nodes());
        assert_eq!(original.references(), rebuilt.references());
        assert_ne!(original.provenance(), rebuilt.provenance());
        assert_ne!(original.recorded_at(), rebuilt.recorded_at());
    }
    assert_ne!(
        old.candidate().observation().state().head(),
        restored.candidate().observation().state().head()
    );
    assert_ne!(
        old.candidate().observation().contract(),
        restored.candidate().observation().contract()
    );
    assert_ne!(old.id(), restored.id());
    assert_ne!(old, restored);
    // Historical queries still return the original target after all transitions.
    assert_graph(
        old,
        &[
            ("a.json", A_ID, ID_NODES, false),
            ("b.json", B_BOTH, BOTH_NODES, true),
        ],
        TO_ID,
        true,
    );
    assert_graph(
        &retained[1],
        &[
            ("a.json", A_OTHER, OTHER_NODES, false),
            ("b.json", B_BOTH, BOTH_NODES, true),
        ],
        TO_OTHER,
        true,
    );
}

#[test]
fn permutation_whitespace_and_unselected_changes_preserve_graph_but_not_capture_identity() {
    let repo = Repo::new();
    commit(
        &repo,
        &[
            ("a.json", A_ID),
            ("b.json", B_BOTH),
            ("unselected.json", "0"),
        ],
    );
    let c = contract(&repo, 1);
    let original = extract(&repo, &c, 1, &["a.json", "b.json"]).unwrap();
    assert_validity(&repo, &original, 6, Some(Validity::Current));
    let permuted = extract(&repo, &c, 11, &["b.json", "a.json"]).unwrap();
    assert_validity(&repo, &permuted, 16, Some(Validity::Current));
    for (set, first) in [(&original, 1), (&permuted, 11)] {
        assert_capture(set, &c, first);
        assert_graph(
            set,
            &[
                ("a.json", A_ID, ID_NODES, false),
                ("b.json", B_BOTH, BOTH_NODES, true),
            ],
            TO_ID,
            true,
        );
    }
    assert_eq!(edges(&original), edges(&permuted));
    assert_eq!(
        original.candidate().observation().state(),
        permuted.candidate().observation().state()
    );
    assert_eq!(
        original.candidate().observation().basis(),
        permuted.candidate().observation().basis()
    );
    assert_ne!(
        original.candidate().observation().id(),
        permuted.candidate().observation().id()
    );
    assert_ne!(original.provenance(), permuted.provenance());
    assert_ne!(original, permuted);

    repo.commit_file("a.json", A_SPACE);
    let c = contract(&repo, 2);
    let whitespace = extract(&repo, &c, 21, &["a.json", "b.json"]).unwrap();
    assert_capture(&whitespace, &c, 21);
    assert_graph(
        &whitespace,
        &[
            ("a.json", A_SPACE, SPACE_NODES, false),
            ("b.json", B_BOTH, BOTH_NODES, true),
        ],
        TO_ID,
        true,
    );
    assert_eq!(edges(&original), edges(&whitespace));
    assert_ne!(
        original.sources()[0].bytes(),
        whitespace.sources()[0].bytes()
    );
    assert_ne!(
        original.sources()[0].nodes(),
        whitespace.sources()[0].nodes()
    );
    assert_validity(&repo, &original, 26, Some(Validity::Stale));
    assert_validity(&repo, &whitespace, 27, Some(Validity::Current));

    repo.commit_file("unselected.json", "1");
    let c = contract(&repo, 3);
    let unrelated = extract(&repo, &c, 31, &["b.json", "a.json"]).unwrap();
    assert_capture(&unrelated, &c, 31);
    assert_graph(
        &unrelated,
        &[
            ("a.json", A_SPACE, SPACE_NODES, false),
            ("b.json", B_BOTH, BOTH_NODES, true),
        ],
        TO_ID,
        true,
    );
    assert_eq!(edges(&whitespace), edges(&unrelated));
    assert_eq!(
        whitespace.sources()[0].bytes(),
        unrelated.sources()[0].bytes()
    );
    assert_ne!(whitespace.candidate(), unrelated.candidate());
    // Whole-candidate invalidation has no non-impact exemption for omitted files.
    assert_validity(&repo, &whitespace, 36, Some(Validity::Stale));
    assert_validity(&repo, &unrelated, 37, Some(Validity::Current));
}

#[test]
fn selecting_omitting_and_deleting_a_document_keep_coverage_and_acquisition_distinct() {
    use UnresolvedReference::{OutsideCoverage, UnsupportedFragment};
    let repo = Repo::new();
    commit(&repo, &[("a.json", COVERAGE), ("b.json", B_ID)]);
    let c = contract(&repo, 1);
    let outside: &[Edge<'_>] = &[
        (
            "a.json",
            "/0/$ref",
            "b.json#/Id",
            Target::Unresolved(OutsideCoverage),
        ),
        (
            "a.json",
            "/1/$ref",
            "b.json#anchor",
            Target::Unresolved(UnsupportedFragment),
        ),
        (
            "a.json",
            "/2/$ref",
            "https://example.invalid/x#/Id",
            Target::Unresolved(OutsideCoverage),
        ),
        (
            "a.json",
            "/3/$ref",
            "absent.json#/Id",
            Target::Unresolved(OutsideCoverage),
        ),
    ];
    let selected: &[Edge<'_>] = &[
        (
            "a.json",
            "/0/$ref",
            "b.json#/Id",
            Target::Node("b.json", "/Id"),
        ),
        (
            "a.json",
            "/1/$ref",
            "b.json#anchor",
            Target::Unresolved(UnsupportedFragment),
        ),
        (
            "a.json",
            "/2/$ref",
            "https://example.invalid/x#/Id",
            Target::Unresolved(OutsideCoverage),
        ),
        (
            "a.json",
            "/3/$ref",
            "absent.json#/Id",
            Target::Unresolved(OutsideCoverage),
        ),
    ];
    let omitted = extract(&repo, &c, 1, &["a.json"]).unwrap();
    assert_capture(&omitted, &c, 1);
    assert_graph(
        &omitted,
        &[("a.json", COVERAGE, COVERAGE_NODES, false)],
        outside,
        false,
    );
    assert_eq!(
        omitted.incoming(&path("b.json"), "/Id").err(),
        Some(SourceSetError::MissingSource)
    );
    assert_validity(&repo, &omitted, 5, None);
    assert_eq!(fs::read(repo.path.join("b.json")).unwrap(), B_ID.as_bytes());

    let included = extract(&repo, &c, 11, &["b.json", "a.json"]).unwrap();
    assert_capture(&included, &c, 11);
    assert_graph(
        &included,
        &[
            ("a.json", COVERAGE, COVERAGE_NODES, false),
            ("b.json", B_ID, B_ID_NODES, true),
        ],
        selected,
        false,
    );
    assert_validity(&repo, &included, 16, None);

    // Omission did not imply deletion. This explicit fixture transition does.
    repo.git(&["rm", "--", "b.json"]);
    repo.git(&["commit", "-m", "delete selected document"]);
    let c = contract(&repo, 2);
    let deleted = extract(&repo, &c, 21, &["a.json"]).unwrap();
    assert_capture(&deleted, &c, 21);
    assert_graph(
        &deleted,
        &[("a.json", COVERAGE, COVERAGE_NODES, false)],
        outside,
        false,
    );
    assert_eq!(edges(&omitted), edges(&deleted));
    assert_validity(&repo, &included, 25, Some(Validity::Stale));
    assert_validity(&repo, &deleted, 26, None);
    for (first, names) in [
        (31, &["a.json", "b.json"][..]),
        (41, &["a.json", "absent.json"][..]),
    ] {
        let error = extract(&repo, &c, first, names).unwrap_err();
        assert!(matches!(
            error,
            SourceSetExtractionError::Extraction(ExtractionError::Inspection(
                InspectionError::Unsupported("missing committed source")
            ))
        ));
        assert!(error.source().unwrap().source().is_some());
    }
    assert_graph(
        &included,
        &[
            ("a.json", COVERAGE, COVERAGE_NODES, false),
            ("b.json", B_ID, B_ID_NODES, true),
        ],
        selected,
        false,
    );
}

#[test]
fn malformed_or_unsupported_selected_replacements_fail_without_relabeling_old_sets() {
    let repo = Repo::new();
    commit(&repo, &[("a.json", A_ID), ("b.json", B_ID)]);
    let c = contract(&repo, 1);
    let original = extract(&repo, &c, 1, &["a.json", "b.json"]).unwrap();
    let saved = original.clone();
    let outside: &[Edge<'_>] = &[(
        "a.json",
        "/link/$ref",
        "b.json#/Id",
        Target::Unresolved(UnresolvedReference::OutsideCoverage),
    )];
    for (phase, bad) in ["{\"private-malformed-w01\":", "{\"$ref\":42}"]
        .iter()
        .enumerate()
    {
        commit(&repo, &[("b.json", bad)]);
        let c = contract(&repo, phase as u64 + 2);
        let first = phase as u64 * 20 + 11;
        let error = extract(&repo, &c, first, &["a.json", "b.json"]).unwrap_err();
        match phase {
            0 => assert!(matches!(
                error,
                SourceSetExtractionError::Extraction(ExtractionError::InvalidJson)
            )),
            1 => assert!(matches!(
                error,
                SourceSetExtractionError::Extraction(ExtractionError::Unsupported)
            )),
            _ => unreachable!(),
        }
        assert!(!format!("{error:?} {error}").contains("private-malformed-w01"));
        let omitted = extract(&repo, &c, first + 10, &["a.json"]).unwrap();
        assert_capture(&omitted, &c, first + 10);
        assert_graph(
            &omitted,
            &[("a.json", A_ID, ID_NODES, false)],
            outside,
            false,
        );
        assert_validity(&repo, &omitted, first + 14, None);
        assert_validity(&repo, &original, first + 15, Some(Validity::Stale));
        assert_eq!(original, saved);
    }
    commit(&repo, &[("b.json", B_ID)]);
    let c = contract(&repo, 4);
    let repaired_input = extract(&repo, &c, 51, &["a.json", "b.json"]).unwrap();
    assert_capture(&repaired_input, &c, 51);
    assert_graph(
        &repaired_input,
        &[
            ("a.json", A_ID, ID_NODES, false),
            ("b.json", B_ID, B_ID_NODES, true),
        ],
        TO_ID,
        true,
    );
    assert_validity(&repo, &repaired_input, 56, Some(Validity::Current));
    assert_validity(&repo, &original, 57, Some(Validity::Stale));
    assert_eq!(original, saved);
}

#[test]
fn dirty_and_incomplete_observation_transitions_cannot_establish_current() {
    let repo = Repo::new();
    commit(&repo, &[("a.json", A_ID), ("b.json", B_ID)]);
    let c = contract(&repo, 1);
    let set = extract(&repo, &c, 1, &["a.json", "b.json"]).unwrap();
    let saved = set.clone();
    assert_capture(&set, &c, 1);
    assert_validity(&repo, &set, 6, Some(Validity::Current));
    let latest = read_only(&repo, || inspect_candidate(&repo.path, &c, id(7)).unwrap());
    for (sequence, coverage, consistency, basis, reason) in [
        (
            8,
            ObservationCoverage::Incomplete,
            ObservationConsistency::TwoMatchingSamples,
            latest.basis().cloned(),
            BindingInsufficiency::IncompleteCoverage,
        ),
        (
            9,
            ObservationCoverage::GitStatus,
            ObservationConsistency::Indeterminate,
            latest.basis().cloned(),
            BindingInsufficiency::IndeterminateConsistency,
        ),
        (
            10,
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
            None,
            BindingInsufficiency::MissingComparisonBasis,
        ),
    ] {
        // Explicit caller-supplied weakened reports, derived from a real capture.
        // They are not claims that the actual adapter returned partial success.
        let partial = CandidateObservation::new(
            id(sequence),
            &c,
            RepositoryState::new(
                latest.state().repository().cloned(),
                latest.state().head().clone(),
                vec![],
                coverage,
                consistency,
            )
            .unwrap(),
            basis,
        );
        read_only(&repo, || {
            let view = qualify(&set, Some(&partial)).unwrap();
            assert_eq!(view.validity(), None);
            assert_eq!(
                view.correspondence(),
                &BindingDecision::Insufficient(reason)
            );
        });
    }
    assert_eq!(qualify(&set, None).unwrap().validity(), None);

    fs::write(repo.path.join("a.json"), A_OTHER).unwrap();
    let dirty = read_only(&repo, || inspect_candidate(&repo.path, &c, id(11)).unwrap());
    assert_eq!(dirty.state().head(), latest.state().head());
    assert_eq!(
        dirty.state().entries(),
        &[Entry::Tracked {
            path: b"a.json".to_vec(),
            staged: None,
            unstaged: Some(Change::Modified),
        }]
    );
    assert_eq!(
        qualify(&set, Some(&dirty)).unwrap().validity(),
        Some(Validity::Stale)
    );
    assert_validity(&repo, &set, 12, Some(Validity::Stale));
    let failure = extract(&repo, &c, 21, &["a.json", "b.json"]).unwrap_err();
    let SourceSetExtractionError::Extraction(ExtractionError::Candidate(decision)) = failure else {
        panic!("dirty acquisition must fail candidate binding")
    };
    assert_eq!(
        *decision,
        BindingDecision::Incompatible(BindingMismatch::WorkingTree)
    );
    assert_graph(
        &set,
        &[
            ("a.json", A_ID, ID_NODES, false),
            ("b.json", B_ID, B_ID_NODES, true),
        ],
        TO_ID,
        true,
    );

    // Deliberately restore working bytes without claiming a new committed snapshot.
    fs::write(repo.path.join("a.json"), A_ID).unwrap();
    assert_validity(&repo, &set, 26, Some(Validity::Current));
    assert_eq!(set, saved);
}
