//! CAP-KNW-001 source-set invariants. All captures here are synthetic producer
//! assertions; actual Git acquisition/quoted-token spans are tested by CLI.
use cortex_core::{
    CandidateId, TaskContract, TaskId,
    binding::*,
    knowledge::{source_set::*, *},
};
use std::time::{Duration, UNIX_EPOCH};

fn id(n: u64) -> ObservationId {
    ObservationId::new("set-domain", n).unwrap()
}
fn path(p: &[u8]) -> SourcePath {
    SourcePath::new(p).unwrap()
}
fn observation(n: u64, revision: u64, root: &str, commit: char) -> CandidateObservation {
    let root = std::env::temp_dir().join(root);
    let repo = RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git")).unwrap();
    let commit = CandidateId::from_git_commit(&commit.to_string().repeat(40)).unwrap();
    let contract = CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("source-set").unwrap(),
            "Inspect explicit sources",
            "Retain exact endpoints",
        )
        .unwrap(),
        ContractRevision::new(revision).unwrap(),
        CandidateRequirement::new(repo.clone(), commit.clone(), WorktreeRequirement::Clean),
    );
    CandidateObservation::new(
        id(n),
        &contract,
        RepositoryState::new(
            Some(repo),
            Head::Detached { commit },
            vec![],
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
        )
        .unwrap(),
        Some(ComparisonBasis::new("fixture", vec![1]).unwrap()),
    )
}
fn observed(n: u64) -> CandidateObservation {
    observation(n, 1, "cortex-set-domain", 'a')
}
fn bound(revision: u64, root: &str, commit: char) -> BoundCandidate {
    let a = observation(1, revision, root, commit);
    let b = observation(2, revision, root, commit);
    match bind_candidate(a.contract(), Some(&a), Some(&b)).unwrap() {
        BindingDecision::Compatible(c) => *c,
        _ => panic!("synthetic fixture must bind"),
    }
}
fn candidate() -> BoundCandidate {
    bound(1, "cortex-set-domain", 'a')
}
fn record(n: u64) -> ExtractionRecord {
    ExtractionRecord::new(id(n), "fixture", "v1", UNIX_EPOCH).unwrap()
}
fn node(key: &str) -> KnowledgeNode {
    KnowledgeNode::new(key, SourceSpan::new(0, 1).unwrap()).unwrap()
}
fn source(
    p: &[u8],
    n: u64,
    nodes: Vec<KnowledgeNode>,
    refs: Vec<ExtractedReference>,
) -> ReferenceIndex {
    ReferenceIndex::new(candidate(), path(p), b"0".to_vec(), record(n), nodes, refs).unwrap()
}
fn blank(p: &[u8], n: u64) -> ReferenceIndex {
    source(p, n, vec![node("")], vec![])
}
fn collection(sources: Vec<ReferenceIndex>) -> Result<ReferenceSet, SourceSetError> {
    let n = sources.len() as u64 + 3;
    ReferenceSet::new(sources, record(n))
}
fn external(from: &str, spelling: &str) -> ExtractedReference {
    ExtractedReference::new(
        from,
        spelling,
        ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
    )
    .unwrap()
}

#[test]
fn exact_borrowed_endpoints_cycle_order_and_provenance_are_additive() {
    let a = source(
        b"a.json",
        3,
        vec![node(""), node("/Id"), node("/$ref")],
        vec![external("/$ref", "b.json#/Id")],
    );
    let b = source(
        b"b.json",
        4,
        vec![node(""), node("/Id"), node("/$ref")],
        vec![external("/$ref", "a.json#/Id")],
    );
    let set = collection(vec![b.clone(), a.clone()]).unwrap();
    assert_eq!(set, collection(vec![a.clone(), b.clone()]).unwrap());
    assert_eq!(set.sources(), &[a, b]);
    assert!(set.complete());
    assert!(set.sources().iter().all(|s| !s.complete()));
    assert_eq!(set.references().len(), 2);
    let incoming: Vec<_> = set.incoming(&path(b"a.json"), "/Id").unwrap().collect();
    assert_eq!(incoming.len(), 1);
    let occurrence = incoming[0];
    assert_eq!(occurrence.from().path().bytes(), b"b.json");
    assert_eq!(occurrence.from().node().locator(), "/$ref");
    assert!(std::ptr::eq(
        occurrence.reference(),
        &set.sources()[1].references()[0]
    ));
    assert!(std::ptr::eq(occurrence.from().source(), &set.sources()[1]));
    assert_eq!(
        occurrence.from().source().provenance(),
        set.sources()[1].provenance()
    );
    assert_eq!(occurrence.reference().origin(), RelationOrigin::Extracted);
    assert_eq!(
        occurrence.reference().disposition(),
        ValidationDisposition::Unverified
    );
    let Resolution::Resolved(endpoint) = occurrence.target() else {
        panic!("resolved")
    };
    assert_eq!(endpoint.path().bytes(), b"a.json");
    assert_eq!(endpoint.node().locator(), "/Id");
    assert_eq!(set.incoming(&path(b"b.json"), "/Id").unwrap().count(), 1);
    assert_eq!(set.incoming(&path(b"a.json"), "").unwrap().count(), 0);
    assert_eq!(
        set.incoming(&path(b"a.json"), "/missing").err(),
        Some(SourceSetError::MissingNode)
    );
    assert_eq!(
        set.incoming(&path(b"missing.json"), "").err(),
        Some(SourceSetError::MissingSource)
    );
    assert_eq!(set.candidate(), &candidate());
    assert_eq!(set.id(), &id(5));
    assert_eq!(set.recorded_at(), UNIX_EPOCH);
    assert_eq!(set.provenance().source(), "fixture");
    assert!(!format!("{set:?} {occurrence:?} {endpoint:?}").contains("b.json"));
}

#[test]
fn relative_paths_preserve_raw_parent_bytes_and_downward_identity() {
    let a = source(
        b"raw-\xff/schema.json",
        4,
        vec![node(""), node("/$ref")],
        vec![external("/$ref", "models/user.json#/Id")],
    );
    let b = source(
        b"raw-\xff/models/user.json",
        3,
        vec![node(""), node("/Id")],
        vec![],
    );
    let set = collection(vec![a, b]).unwrap();
    let incoming = set
        .incoming(&path(b"raw-\xff/models/user.json"), "/Id")
        .unwrap()
        .next()
        .unwrap();
    assert_eq!(incoming.from().path().bytes(), b"raw-\xff/schema.json");
    assert!(set.complete());
    assert_eq!(
        set.endpoint(&path(b"raw-\xef\xbf\xbd/models/user.json"), "")
            .unwrap_err(),
        SourceSetError::MissingSource
    );
}

#[test]
fn unsupported_unselected_and_missing_targets_are_not_invented() {
    use UnresolvedReference::*;
    let cases = [
        ("", OutsideCoverage),
        ("unselected.json", OutsideCoverage),
        ("common.json#/missing", MissingTarget),
        ("https://host/x", OutsideCoverage),
        ("//host/x", OutsideCoverage),
        ("/common.json", OutsideCoverage),
        ("common\\file.json", OutsideCoverage),
        ("common.json?x=1", OutsideCoverage),
        ("common%2ejson", OutsideCoverage),
        ("models//x", OutsideCoverage),
        ("./common.json", OutsideCoverage),
        ("models/../common.json", OutsideCoverage),
        ("common.json#anchor", UnsupportedFragment),
        ("common.json#/~2", UnsupportedFragment),
        ("common.json#/%41", UnsupportedFragment),
        ("common.json##", UnsupportedFragment),
    ];
    let mut nodes = vec![node("")];
    let mut refs = vec![];
    for (i, (spelling, _)) in cases.iter().enumerate() {
        let from = format!("/{i}/$ref");
        nodes.push(node(&from));
        refs.push(external(&from, spelling));
    }
    let a = source(b"a.json", 3, nodes, refs);
    let b = blank(b"common.json", 4);
    let set = collection(vec![a, b]).unwrap();
    assert!(!set.complete());
    for occurrence in set.references() {
        let (_, expected) = cases
            .iter()
            .find(|(s, _)| *s == occurrence.reference().spelling())
            .unwrap();
        assert_eq!(occurrence.target(), Resolution::Unresolved(*expected));
    }
    assert_eq!(set.incoming(&path(b"common.json"), "").unwrap().count(), 0);
}

#[test]
fn lexical_positive_neighbours_and_stricter_fragment_projection_preserve_old_values() {
    let local =
        ExtractedReference::new("/local", "#/a#b", ReferenceTarget::Local("/a#b".into())).unwrap();
    let a = source(
        b"a.json",
        3,
        vec![
            node(""),
            node("/local"),
            node("/a#b"),
            node("/root"),
            node("/empty"),
            node("/escape"),
        ],
        vec![
            local,
            external("/root", "common.json"),
            external("/empty", "common.json#"),
            external("/escape", "common.json#/a~1b/~0"),
        ],
    );
    let b = source(b"common.json", 4, vec![node(""), node("/a~1b/~0")], vec![]);
    let set = collection(vec![a, b]).unwrap();
    assert_eq!(set.incoming(&path(b"common.json"), "").unwrap().count(), 2);
    assert_eq!(
        set.incoming(&path(b"common.json"), "/a~1b/~0")
            .unwrap()
            .count(),
        1
    );
    let local = set
        .references()
        .find(|r| r.reference().from() == "/local")
        .unwrap();
    assert_eq!(
        local.target(),
        Resolution::Unresolved(UnresolvedReference::UnsupportedFragment)
    );
    assert_eq!(
        local.reference().target(),
        &ReferenceTarget::Local("/a#b".into())
    );
}

#[test]
fn admission_rejects_empty_duplicate_mixed_and_conflicting_contexts() {
    assert_eq!(collection(vec![]).unwrap_err(), SourceSetError::Empty);
    assert_eq!(
        collection(vec![blank(b"a", 3), blank(b"a", 4)]).unwrap_err(),
        SourceSetError::DuplicateSource
    );
    for different in [
        bound(2, "cortex-set-domain", 'a'),
        bound(1, "another-repository", 'a'),
        bound(1, "cortex-set-domain", 'b'),
    ] {
        let b = ReferenceIndex::new(
            different,
            path(b"b"),
            b"0".to_vec(),
            record(4),
            vec![node("")],
            vec![],
        )
        .unwrap();
        assert_eq!(
            collection(vec![blank(b"a", 3), b]).unwrap_err(),
            SourceSetError::MixedContext
        );
    }
    let conflict =
        ExtractedReference::new("/ref", "#/target", ReferenceTarget::Local("".into())).unwrap();
    let a = source(
        b"a",
        3,
        vec![node(""), node("/ref"), node("/target")],
        vec![conflict],
    );
    assert_eq!(
        collection(vec![a]).unwrap_err(),
        SourceSetError::ConflictingReference
    );
    let false_missing = ExtractedReference::new(
        "",
        "#",
        ReferenceTarget::Unresolved(UnresolvedReference::MissingTarget),
    )
    .unwrap();
    assert_eq!(
        collection(vec![source(b"a", 3, vec![node("")], vec![false_missing])]).unwrap_err(),
        SourceSetError::ConflictingReference
    );
    // Nonexistent and duplicate endpoints fail at the one canonical source constructor.
    assert_eq!(
        ReferenceIndex::new(
            candidate(),
            path(b"a"),
            b"0".to_vec(),
            record(3),
            vec![node("")],
            vec![
                ExtractedReference::new("", "#/absent", ReferenceTarget::Local("/absent".into()))
                    .unwrap()
            ]
        )
        .unwrap_err(),
        KnowledgeError::MissingNode
    );
    assert_eq!(
        ReferenceIndex::new(
            candidate(),
            path(b"a"),
            b"0".to_vec(),
            record(3),
            vec![node(""), node("")],
            vec![]
        )
        .unwrap_err(),
        KnowledgeError::DuplicateNode
    );
}

#[test]
fn provenance_rejects_reused_wrong_series_unsorted_ids_and_different_times() {
    for n in [1, 2, 3, 5, 6] {
        assert_eq!(
            ReferenceSet::new(vec![blank(b"a", 3)], record(n)).unwrap_err(),
            SourceSetError::InvalidProvenance
        );
    }
    let other = ExtractionRecord::new(
        ObservationId::new("other", 4).unwrap(),
        "fixture",
        "v1",
        UNIX_EPOCH,
    )
    .unwrap();
    assert_eq!(
        ReferenceSet::new(vec![blank(b"a", 3)], other).unwrap_err(),
        SourceSetError::InvalidProvenance
    );
    assert_eq!(
        collection(vec![blank(b"a", 4), blank(b"b", 3)]).unwrap_err(),
        SourceSetError::InvalidProvenance
    );
    let time =
        ExtractionRecord::new(id(4), "fixture", "v1", UNIX_EPOCH + Duration::from_secs(1)).unwrap();
    assert_eq!(
        ReferenceSet::new(vec![blank(b"a", 3)], time).unwrap_err(),
        SourceSetError::InvalidProvenance
    );
    // A bound capture pair must itself reserve n/n+1, not merely increasing IDs.
    let a = observed(1);
    let b = observed(3);
    let BindingDecision::Compatible(c) = bind_candidate(a.contract(), Some(&a), Some(&b)).unwrap()
    else {
        panic!("bind")
    };
    let source = ReferenceIndex::new(
        *c,
        path(b"a"),
        b"0".to_vec(),
        record(4),
        vec![node("")],
        vec![],
    )
    .unwrap();
    assert_eq!(
        collection(vec![source]).unwrap_err(),
        SourceSetError::InvalidProvenance
    );
}

#[test]
fn aggregate_count_bytes_nodes_and_references_have_accept_and_reject_boundaries() {
    let eight = (0..8)
        .map(|i| blank(format!("{i}").as_bytes(), i + 3))
        .collect();
    assert_eq!(collection(eight).unwrap().sources().len(), 8);
    let nine = (0..9)
        .map(|i| blank(format!("{i}").as_bytes(), i + 3))
        .collect();
    assert_eq!(collection(nine).unwrap_err(), SourceSetError::Limit);
    let bytes = |extra: bool| {
        let mut sources: Vec<_> = (0..4)
            .map(|i| {
                ReferenceIndex::new(
                    candidate(),
                    path(format!("{i}").as_bytes()),
                    [b"0".as_slice(), &vec![b' '; MAX_SOURCE_BYTES - 1]].concat(),
                    record(i + 3),
                    vec![node("")],
                    vec![],
                )
                .unwrap()
            })
            .collect();
        if extra {
            sources.push(blank(b"4", 7));
        }
        collection(sources)
    };
    assert_eq!(
        bytes(false)
            .unwrap()
            .sources()
            .iter()
            .map(|s| s.bytes().len())
            .sum::<usize>(),
        1_048_576
    );
    assert_eq!(bytes(true).unwrap_err(), SourceSetError::Limit);
    let nodes = |extra: bool| {
        let mut sources: Vec<_> = (0..2)
            .map(|i| {
                let mut nodes = vec![node("")];
                nodes.extend((0..4095).map(|j| node(&format!("/{j}"))));
                source(format!("{i}").as_bytes(), i + 3, nodes, vec![])
            })
            .collect();
        if extra {
            sources.push(blank(b"2", 5));
        }
        collection(sources)
    };
    assert_eq!(
        nodes(false)
            .unwrap()
            .sources()
            .iter()
            .map(|s| s.nodes().len())
            .sum::<usize>(),
        8192
    );
    assert_eq!(nodes(true).unwrap_err(), SourceSetError::Limit);
    let references = |extra: bool| {
        let mut sources: Vec<_> = (0..2)
            .map(|i| {
                let mut nodes = vec![node("")];
                let refs = (0..1024)
                    .map(|j| {
                        let locator = format!("/{j}");
                        nodes.push(node(&locator));
                        ExtractedReference::new(&locator, "#", ReferenceTarget::Local("".into()))
                            .unwrap()
                    })
                    .collect();
                source(format!("{i}").as_bytes(), i + 3, nodes, refs)
            })
            .collect();
        if extra {
            sources.push(source(
                b"2",
                5,
                vec![node("")],
                vec![ExtractedReference::new("", "#", ReferenceTarget::Local("".into())).unwrap()],
            ));
        }
        collection(sources)
    };
    assert_eq!(references(false).unwrap().references().len(), 2048);
    assert_eq!(references(true).unwrap_err(), SourceSetError::Limit);
}

#[test]
fn correspondence_distinguishes_missing_incomplete_stale_and_later_capture() {
    let set = collection(vec![blank(b"a", 3), blank(b"b", 4)]).unwrap();
    assert_eq!(
        set.correspondence(None).unwrap(),
        BindingDecision::Insufficient(BindingInsufficiency::ReobservationRequired)
    );
    for n in 1..=5 {
        assert_eq!(
            set.correspondence(Some(&observed(n))).unwrap(),
            BindingDecision::Stale(BindingStaleness::ObservationOrder)
        );
    }
    assert!(matches!(
        set.correspondence(Some(&observed(6))).unwrap(),
        BindingDecision::Compatible(_)
    ));
    let later = observed(6);
    let incomplete = CandidateObservation::new(
        id(6),
        later.contract(),
        RepositoryState::new(
            later.state().repository().cloned(),
            later.state().head().clone(),
            vec![],
            ObservationCoverage::Incomplete,
            ObservationConsistency::Indeterminate,
        )
        .unwrap(),
        later.basis().cloned(),
    );
    assert_eq!(
        set.correspondence(Some(&incomplete)).unwrap(),
        BindingDecision::Insufficient(BindingInsufficiency::IncompleteCoverage)
    );
    let changed = observation(6, 2, "cortex-set-domain", 'b');
    assert_eq!(
        set.correspondence(Some(&changed)).unwrap(),
        BindingDecision::Stale(BindingStaleness::ContractRevision)
    );
    let other_scope = CandidateObservation::new(
        ObservationId::new("elsewhere", 6).unwrap(),
        later.contract(),
        later.state().clone(),
        later.basis().cloned(),
    );
    assert_eq!(
        set.correspondence(Some(&other_scope)).unwrap(),
        BindingDecision::Insufficient(BindingInsufficiency::DifferentObservationScope)
    );
    // Querying cannot mutate a saved context or turn extraction into a capture.
    assert_eq!(set.candidate().observation(), &observed(2));
}
