//! Author-separated C4 counterexample: dependency freshness is not occurrence truth.
#![cfg(target_os = "linux")]
mod support;
use cortex_cli::{inspect, knowledge::source_set};
use cortex_core::{
    CandidateId, TaskContract, TaskId,
    binding::*,
    knowledge::{SourcePath, inspection::*},
};
use std::{fs, time::UNIX_EPOCH};
use support::Repo;

#[test]
fn changed_selected_target_blocks_current_claim_without_erasing_retained_lexical_facts() {
    let repo = Repo::new();
    repo.commit_file("a.json", r##"{"$ref":"b.json#/Id"}"##);
    repo.commit_file("b.json", r#"{"Id":0}"#);
    let commit = String::from_utf8(repo.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    let contract = CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("review.c4").unwrap(),
            "Inspect references",
            "Keep distinct dimensions",
        )
        .unwrap(),
        ContractRevision::new(1).unwrap(),
        CandidateRequirement::new(
            inspect(&repo.path).unwrap().repository_identity().unwrap(),
            CandidateId::from_git_commit(commit.trim()).unwrap(),
            WorktreeRequirement::Clean,
        ),
    );
    let a = SourcePath::new(b"a.json").unwrap();
    let b = SourcePath::new(b"b.json").unwrap();
    let set = source_set::extract_json_source_set(
        &repo.path,
        &contract,
        ObservationId::new("review.c4", 1).unwrap(),
        vec![a.clone(), b.clone()],
        UNIX_EPOCH,
    )
    .unwrap();
    let lexical = ReferenceClaim::new(ReferenceClaimKind::LexicalOccurrence, &a, "/$ref").unwrap();
    let target =
        ReferenceClaim::new(ReferenceClaimKind::LexicalTargetResolution, &a, "/$ref").unwrap();
    // Extraction n=1 with two sources reserves collection ID 5. Reusing
    // that ID is not a later observation and cannot establish current claims.
    let reused = source_set::reinspect(
        &repo.path,
        &set,
        ObservationId::new("review.c4", 5).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        reused.inspect_claim(lexical),
        ClaimInspection::NotEstablished(ClaimGap::SourceCorrespondence)
    ));
    let before = source_set::reinspect(
        &repo.path,
        &set,
        ObservationId::new("review.c4", 6).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        before.inspect_claim(lexical),
        ClaimInspection::Supported(_)
    ));
    assert!(matches!(
        before.inspect_claim(target),
        ClaimInspection::Supported(_)
    ));
    assert!(before.reference_resolution().all_references_resolved());
    // A remains unchanged. Its retained target exists in the old selected set,
    // but current correspondence depends on B as well as A.
    fs::write(repo.path.join("b.json"), "{}").unwrap();
    let after = source_set::reinspect(
        &repo.path,
        &set,
        ObservationId::new("review.c4", 7).unwrap(),
    )
    .unwrap();
    for claim in [lexical, target] {
        assert!(matches!(
            after.inspect_claim(claim),
            ClaimInspection::NotEstablished(ClaimGap::SourceCorrespondence)
        ));
    }
    assert!(after.reference_resolution().all_references_resolved());
    assert_eq!(
        after.extraction_coverage(),
        ExtractionCoverage::Bounded(ExtractionProfile::JsonSourceSet)
    );
    let ClaimInspection::Supported(historical) = inspect_retained_set_claim(&set, target) else {
        panic!("retained lexical fact was overwritten")
    };
    assert_eq!(historical.source().path(), &a);
    assert_eq!(historical.target().unwrap().source().path(), &b);
    assert_eq!(
        historical.target().unwrap().source().bytes(),
        br#"{"Id":0}"#
    );
    assert_eq!(historical.scope(), ResolutionScope::SelectedSourceSet);
    assert!(matches!(
        after.inspect_claim(
            ReferenceClaim::new(ReferenceClaimKind::SemanticRelationship, &a, "/$ref").unwrap()
        ),
        ClaimInspection::Unsupported(_)
    ));
}
