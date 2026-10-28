//! CAP-KNW-001 domain invariants; these synthetic commits are not Git observations.
use cortex_core::{CandidateId, TaskContract, TaskId, binding::*, knowledge::*};
use std::time::{Duration, UNIX_EPOCH};
fn id(n: u64) -> ObservationId {
    ObservationId::new("knowledge", n).unwrap()
}
fn observed(n: u64) -> CandidateObservation {
    let root = std::env::temp_dir().join("cortex-knowledge-domain");
    let repo = RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git")).unwrap();
    let commit = CandidateId::from_git_commit(&"a".repeat(40)).unwrap();
    let contract = CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("knowledge").unwrap(),
            "Index explicit source",
            "Return exact local refs",
        )
        .unwrap(),
        ContractRevision::new(1).unwrap(),
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
fn bound() -> BoundCandidate {
    let a = observed(1);
    let b = observed(2);
    match bind_candidate(a.contract(), Some(&a), Some(&b)).unwrap() {
        BindingDecision::Compatible(b) => *b,
        _ => panic!("fixture must bind"),
    }
}
fn node(key: &str) -> KnowledgeNode {
    KnowledgeNode::new(key, SourceSpan::new(0, 2).unwrap()).unwrap()
}
fn index(
    nodes: Vec<KnowledgeNode>,
    refs: Vec<ExtractedReference>,
) -> Result<ReferenceIndex, KnowledgeError> {
    ReferenceIndex::new(
        bound(),
        SourcePath::new(b"fixture.json").unwrap(),
        b"{}".to_vec(),
        ExtractionRecord::new(id(3), "fixture-adapter", "v1", UNIX_EPOCH).unwrap(),
        nodes,
        refs,
    )
}
#[test]
fn borrowed_queries_preserve_identity_provenance_and_unverified_origin() {
    let reference =
        ExtractedReference::new("/ref", "#/target", ReferenceTarget::Local("/target".into()))
            .unwrap();
    let value = index(
        vec![node("/target"), node(""), node("/ref")],
        vec![reference.clone()],
    )
    .unwrap();
    assert_eq!(
        value.incoming("/target").unwrap().collect::<Vec<_>>(),
        vec![&reference]
    );
    assert_eq!(value.incoming("").unwrap().count(), 0);
    assert_eq!(
        value.node("absent").unwrap_err(),
        KnowledgeError::MissingNode
    );
    assert_eq!(value.provenance().version(), "v1");
    assert_eq!(value.recorded_at(), UNIX_EPOCH);
    assert_eq!(value.id(), &id(3));
    assert_eq!(value.candidate().observation(), &observed(2));
    assert_eq!(reference.origin(), RelationOrigin::Extracted);
    assert_eq!(reference.disposition(), ValidationDisposition::Unverified);
    assert!(matches!(
        value.correspondence(None).unwrap(),
        BindingDecision::Insufficient(_)
    ));
    assert!(matches!(
        value.correspondence(Some(&observed(4))).unwrap(),
        BindingDecision::Compatible(_)
    ));
    assert!(matches!(
        value.correspondence(Some(&observed(3))).unwrap(),
        BindingDecision::Stale(BindingStaleness::ObservationOrder)
    ));
    let again = index(
        vec![node(""), node("/ref"), node("/target")],
        vec![reference],
    )
    .unwrap();
    assert_eq!(value, again);
    assert!(!format!("{value:?}").contains("fixture.json"));
}
#[test]
fn contradictions_and_out_of_source_spans_are_rejected() {
    assert_eq!(
        index(vec![node(""), node("")], vec![]).unwrap_err(),
        KnowledgeError::DuplicateNode
    );
    let r = ExtractedReference::new("", "x", ReferenceTarget::Local("absent".into())).unwrap();
    assert_eq!(
        index(vec![node("")], vec![r]).unwrap_err(),
        KnowledgeError::MissingNode
    );
    let bad = KnowledgeNode::new("", SourceSpan::new(0, 3).unwrap()).unwrap();
    assert_eq!(
        index(vec![bad], vec![]).unwrap_err(),
        KnowledgeError::InvalidSpan
    );
    assert!(SourceSpan::new(2, 2).is_err());
    assert_eq!(
        index(vec![], vec![]).unwrap_err(),
        KnowledgeError::MissingNode
    );
}
#[test]
fn bounded_paths_and_provenance_never_silently_normalize_or_alias() {
    for p in [b"".as_slice(), b"/x", b"../x", b"a/./x", b"a//x", b"x\0"] {
        assert!(SourcePath::new(p).is_err());
    }
    assert_eq!(
        SourcePath::new(&vec![b'x'; MAX_LOCATION_BYTES + 1]).unwrap_err(),
        KnowledgeError::Limit
    );
    let path = SourcePath::new(b"space \xff.json").unwrap();
    assert_eq!(path.bytes(), b"space \xff.json");
    assert!(ExtractionRecord::new(id(3), "x", "v", UNIX_EPOCH - Duration::from_secs(1)).is_err());
    let record = ExtractionRecord::new(id(2), "x", "v", UNIX_EPOCH).unwrap();
    assert_eq!(
        ReferenceIndex::new(
            bound(),
            path,
            b"{}".to_vec(),
            record,
            vec![node("")],
            vec![]
        )
        .unwrap_err(),
        KnowledgeError::InvalidProvenance
    );
    assert_eq!(
        index(vec![node(""); MAX_NODES + 1], vec![]).unwrap_err(),
        KnowledgeError::Limit
    );
}
#[test]
fn unresolved_structure_is_not_invented_or_validated() {
    let r = ExtractedReference::new(
        "",
        "https://untrusted.invalid",
        ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
    )
    .unwrap();
    let value = index(vec![node("")], vec![r]).unwrap();
    assert!(!value.complete());
    assert_eq!(value.nodes().len(), 1);
    assert_eq!(
        value.references()[0].disposition(),
        ValidationDisposition::Unverified
    );
}
