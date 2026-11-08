//! C4: independent lexical inspection axes over actual bounded adapter consumers.
#![cfg(target_os = "linux")]
mod support;

use cortex_cli::{
    inspect, inspect_candidate,
    knowledge::{self, cargo_manifest, markdown, refresh, rust, source_set},
};
use cortex_core::{
    CandidateId, TaskContract, TaskId,
    binding::*,
    knowledge::{
        inspection::*,
        source_set::{ReferenceSet, SourceSetError},
        *,
    },
};
use cortex_evidence::unified::Validity;
use std::{fs, time::UNIX_EPOCH};
use support::Repo;

fn id(n: u64) -> ObservationId {
    ObservationId::new("dimensions", n).unwrap()
}
fn path(s: &str) -> SourcePath {
    SourcePath::new(s.as_bytes()).unwrap()
}
fn fixture(files: &[(&str, &str)]) -> (Repo, CandidateBindingContract) {
    let repo = Repo::new();
    for (name, text) in files {
        repo.commit_file(name, text);
    }
    let commit = String::from_utf8(repo.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    let contract = CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("knowledge.dimensions").unwrap(),
            "Inspect lexical facts",
            "Keep independent dimensions",
        )
        .unwrap(),
        ContractRevision::new(1).unwrap(),
        CandidateRequirement::new(
            inspect(&repo.path).unwrap().repository_identity().unwrap(),
            CandidateId::from_git_commit(commit.trim()).unwrap(),
            WorktreeRequirement::Clean,
        ),
    );
    (repo, contract)
}
fn claim<'a>(
    kind: ReferenceClaimKind,
    source: &'a SourcePath,
    locator: &'a str,
) -> ReferenceClaim<'a> {
    ReferenceClaim::new(kind, source, locator).unwrap()
}
fn changed_profile(index: &ReferenceIndex) -> ReferenceIndex {
    ReferenceIndex::new(
        index.candidate().clone(),
        index.path().clone(),
        index.bytes().to_vec(),
        ExtractionRecord::new(
            index.id().clone(),
            index.provenance().source(),
            "unsupported-profile",
            index.recorded_at(),
        )
        .unwrap(),
        index.nodes().to_vec(),
        index.references().to_vec(),
    )
    .unwrap()
}

#[test]
fn unresolved_reference_supports_exact_lexical_occurrence_not_target_or_semantics() {
    let text = r##"[{"$ref":"other.json#/Id"},{"$ref":"#/missing"},{"$ref":"#anchor"}]"##;
    let (repo, contract) = fixture(&[("source.json", text)]);
    let source = path("source.json");
    let index = knowledge::extract_json_references(
        &repo.path,
        &contract,
        id(1),
        source.clone(),
        UNIX_EPOCH,
    )
    .unwrap();
    let current = inspect_candidate(&repo.path, &contract, id(4)).unwrap();
    let view = knowledge::inspect_validity(&index, Some(&current)).unwrap();
    assert!(matches!(
        view.correspondence(),
        BindingDecision::Compatible(_)
    ));
    assert!(view.profile_matches());
    assert_eq!(view.resolved_projection_validity(), None);
    assert_eq!(
        view.extraction_coverage(),
        ExtractionCoverage::Bounded(ExtractionProfile::JsonReferences)
    );
    assert!(
        !ExtractionProfile::JsonReferences
            .omitted_constructs()
            .is_empty()
    );
    let resolution = view.reference_resolution();
    assert_eq!(
        (
            resolution.emitted(),
            resolution.resolved(),
            resolution.missing_target(),
            resolution.outside_coverage(),
            resolution.unsupported_fragment()
        ),
        (3, 0, 1, 1, 1)
    );
    let occurrence = claim(ReferenceClaimKind::LexicalOccurrence, &source, "/0/$ref");
    let ClaimInspection::Supported(support) = view.inspect_claim(occurrence) else {
        panic!("lexical occurrence not supported")
    };
    assert_eq!(support.source().id(), index.id());
    assert_eq!(support.source().candidate(), index.candidate());
    assert_eq!(
        &support.source().bytes()[support.span().start()..support.span().end()],
        br#""other.json#/Id""#
    );
    assert_eq!(
        support.occurrence().disposition(),
        ValidationDisposition::Unverified
    );
    assert!(support.target().is_none());
    assert!(matches!(
        view.inspect_claim(claim(
            ReferenceClaimKind::LexicalTargetResolution,
            &source,
            "/0/$ref"
        )),
        ClaimInspection::NotEstablished(ClaimGap::UnresolvedTarget(
            UnresolvedReference::OutsideCoverage
        ))
    ));
    assert!(matches!(
        view.inspect_claim(claim(
            ReferenceClaimKind::LexicalTargetResolution,
            &source,
            "/1/$ref"
        )),
        ClaimInspection::NotEstablished(ClaimGap::UnresolvedTarget(
            UnresolvedReference::MissingTarget
        ))
    ));
    for kind in [
        ReferenceClaimKind::SemanticRelationship,
        ReferenceClaimKind::CompleteExtraction,
    ] {
        assert!(
            matches!(view.inspect_claim(claim(kind, &source, "/0/$ref")), ClaimInspection::Unsupported(k) if k == kind)
        );
    }
    assert!(matches!(
        view.inspect_claim(claim(
            ReferenceClaimKind::LexicalOccurrence,
            &source,
            "/absent"
        )),
        ClaimInspection::NotEstablished(ClaimGap::OccurrenceNotRetained)
    ));
    assert!(matches!(
        view.inspect_claim(claim(
            ReferenceClaimKind::LexicalOccurrence,
            &path("unselected.json"),
            "/0/$ref"
        )),
        ClaimInspection::NotEstablished(ClaimGap::SourceNotSelected)
    ));
}

#[test]
fn current_claim_requires_supplied_source_and_profile_correspondence_without_acquisition() {
    let (repo, contract) = fixture(&[("source.json", r##"{"$ref":"#"}"##)]);
    let source = path("source.json");
    let index = knowledge::extract_json_references(
        &repo.path,
        &contract,
        id(1),
        source.clone(),
        UNIX_EPOCH,
    )
    .unwrap();
    let question = claim(ReferenceClaimKind::LexicalOccurrence, &source, "/$ref");
    let missing = knowledge::inspect_validity(&index, None).unwrap();
    assert!(matches!(
        missing.inspect_claim(question),
        ClaimInspection::NotEstablished(ClaimGap::SourceCorrespondence)
    ));
    assert_eq!(missing.reference_resolution().resolved(), 1);
    assert!(matches!(
        inspect_retained_index_claim(&index, question),
        ClaimInspection::Supported(_)
    ));
    let current = inspect_candidate(&repo.path, &contract, id(4)).unwrap();
    let other = changed_profile(&index);
    let view = knowledge::inspect_validity(&other, Some(&current)).unwrap();
    assert_eq!(view.resolved_projection_validity(), Some(Validity::Stale));
    assert_eq!(
        view.extraction_coverage(),
        ExtractionCoverage::Unestablished
    );
    assert!(matches!(
        view.inspect_claim(question),
        ClaimInspection::NotEstablished(ClaimGap::ProfileMismatch)
    ));
    fs::write(repo.path.join("source.json"), "{}").unwrap();
    // Supplied observations are the only input: inspection does not acquire source.
    assert!(matches!(
        knowledge::inspect_validity(&index, Some(&current))
            .unwrap()
            .inspect_claim(question),
        ClaimInspection::Supported(_)
    ));
    let stale = knowledge::reinspect(&repo.path, &index, id(5)).unwrap();
    assert_eq!(stale.resolved_projection_validity(), Some(Validity::Stale));
    assert!(matches!(
        stale.inspect_claim(question),
        ClaimInspection::NotEstablished(ClaimGap::SourceCorrespondence)
    ));
    assert_eq!(index.bytes(), br##"{"$ref":"#"}"##);
}

#[test]
fn single_snapshot_and_source_set_resolution_have_explicit_different_scopes() {
    let (repo, contract) = fixture(&[
        ("a.json", r##"{"$ref":"b.json#/Id"}"##),
        ("b.json", r#"{"Id":0}"#),
    ]);
    let a = path("a.json");
    let set = source_set::extract_json_source_set(
        &repo.path,
        &contract,
        id(1),
        vec![a.clone(), path("b.json")],
        UNIX_EPOCH,
    )
    .unwrap();
    let latest = inspect_candidate(&repo.path, &contract, id(6)).unwrap();
    let single = knowledge::inspect_validity(&set.sources()[0], Some(&latest)).unwrap();
    let together = source_set::inspect_validity(&set, Some(&latest)).unwrap();
    assert_eq!(
        single.reference_resolution().scope(),
        ResolutionScope::SingleSource
    );
    assert_eq!(
        together.reference_resolution().scope(),
        ResolutionScope::SelectedSourceSet
    );
    assert!(!single.reference_resolution().all_references_resolved());
    assert!(together.reference_resolution().all_references_resolved());
    let question = claim(ReferenceClaimKind::LexicalTargetResolution, &a, "/$ref");
    assert!(matches!(
        single.inspect_claim(question),
        ClaimInspection::NotEstablished(ClaimGap::UnresolvedTarget(
            UnresolvedReference::OutsideCoverage
        ))
    ));
    let ClaimInspection::Supported(support) = together.inspect_claim(question) else {
        panic!("selected target missing")
    };
    assert_eq!(support.scope(), ResolutionScope::SelectedSourceSet);
    assert_eq!(support.source().id(), single.index().id());
    let target = support.target().unwrap();
    assert_eq!(target.source().path(), &path("b.json"));
    assert_eq!(target.node().locator(), "/Id");
    assert_eq!(
        &target.source().bytes()[target.node().span().start()..target.node().span().end()],
        b"0"
    );
    // A subset with the same source/locator retains the missing dependency.
    let subset = source_set::extract_json_source_set(
        &repo.path,
        &contract,
        id(10),
        vec![a.clone()],
        UNIX_EPOCH,
    )
    .unwrap();
    let subset_latest = inspect_candidate(&repo.path, &contract, id(14)).unwrap();
    assert!(matches!(
        source_set::inspect_validity(&subset, Some(&subset_latest))
            .unwrap()
            .inspect_claim(question),
        ClaimInspection::NotEstablished(ClaimGap::UnresolvedTarget(
            UnresolvedReference::OutsideCoverage
        ))
    ));
    // A valid collection profile does not hide an incompatible per-source profile.
    let mixed = ReferenceSet::new(
        vec![changed_profile(&set.sources()[0]), set.sources()[1].clone()],
        ExtractionRecord::new(
            set.id().clone(),
            source_set::ADAPTER,
            source_set::PROFILE,
            UNIX_EPOCH,
        )
        .unwrap(),
    )
    .unwrap();
    let view = source_set::inspect_validity(&mixed, Some(&latest)).unwrap();
    assert!(!view.profile_matches());
    assert_eq!(
        view.extraction_coverage(),
        ExtractionCoverage::Unestablished
    );
    assert!(matches!(
        view.inspect_claim(question),
        ClaimInspection::NotEstablished(ClaimGap::ProfileMismatch)
    ));
}

#[test]
fn valid_empty_collections_resolve_vacuously_without_claiming_complete_extraction() {
    let (repo, contract) = fixture(&[("empty.json", "{}")]);
    let source = path("empty.json");
    let set = source_set::extract_json_source_set(
        &repo.path,
        &contract,
        id(1),
        vec![source.clone()],
        UNIX_EPOCH,
    )
    .unwrap();
    let view = source_set::reinspect(&repo.path, &set, id(5)).unwrap();
    assert_eq!(view.reference_resolution().emitted(), 0);
    assert!(view.reference_resolution().all_references_resolved());
    assert_eq!(view.resolved_projection_validity(), Some(Validity::Current));
    assert_eq!(
        view.extraction_coverage(),
        ExtractionCoverage::Bounded(ExtractionProfile::JsonSourceSet)
    );
    assert!(matches!(
        view.inspect_claim(claim(ReferenceClaimKind::CompleteExtraction, &source, "")),
        ClaimInspection::Unsupported(ReferenceClaimKind::CompleteExtraction)
    ));
    assert!(matches!(
        ReferenceSet::new(
            vec![],
            ExtractionRecord::new(id(4), source_set::ADAPTER, source_set::PROFILE, UNIX_EPOCH)
                .unwrap()
        ),
        Err(SourceSetError::Empty)
    ));
    assert!(
        ReferenceClaim::new(
            ReferenceClaimKind::LexicalOccurrence,
            &source,
            "bad\nlocator"
        )
        .is_err()
    );
}

#[test]
fn rust_opaque_regions_remain_explicit_even_when_no_references_are_emitted() {
    let (repo, contract) = fixture(&[("lib.rs", "fn inspect(x: ExternalType) { let y = x; }\n")]);
    let index =
        rust::extract_rust_references(&repo.path, &contract, id(1), path("lib.rs"), UNIX_EPOCH)
            .unwrap();
    let view = rust::reinspect(&repo.path, &index, id(4)).unwrap();
    assert_eq!(view.reference_resolution().emitted(), 0);
    assert!(view.reference_resolution().all_references_resolved());
    assert_eq!(
        view.extraction_coverage(),
        ExtractionCoverage::Bounded(ExtractionProfile::RustLexical)
    );
    assert!(
        ExtractionProfile::RustLexical
            .omitted_constructs()
            .iter()
            .any(|s| s.contains("signature-type"))
    );
    assert!(matches!(
        view.inspect_claim(claim(
            ReferenceClaimKind::CompleteExtraction,
            index.path(),
            ""
        )),
        ClaimInspection::Unsupported(_)
    ));
}

#[test]
fn markdown_and_cargo_consumers_expose_bounded_coverage_and_lexical_support() {
    let (repo, contract) = fixture(&[
        ("doc.md", "# Title\n[external](elsewhere.md)\n"),
        (
            "Cargo.toml",
            "[dependencies]\nexternal = '1'\n[features]\nfeature = ['external/remote']\n",
        ),
    ]);
    let index = markdown::extract_markdown_references(
        &repo.path,
        &contract,
        id(1),
        path("doc.md"),
        UNIX_EPOCH,
    )
    .unwrap();
    let view = markdown::reinspect(&repo.path, &index, id(4)).unwrap();
    assert_eq!(
        view.extraction_coverage(),
        ExtractionCoverage::Bounded(ExtractionProfile::MarkdownLexical)
    );
    assert_eq!(view.reference_resolution().emitted(), 1);
    assert!(matches!(
        view.inspect_claim(claim(
            ReferenceClaimKind::LexicalOccurrence,
            index.path(),
            index.references()[0].from()
        )),
        ClaimInspection::Supported(_)
    ));
    assert!(matches!(
        view.inspect_claim(claim(
            ReferenceClaimKind::LexicalTargetResolution,
            index.path(),
            index.references()[0].from()
        )),
        ClaimInspection::NotEstablished(_)
    ));
    let index = cargo_manifest::extract_cargo_manifest_references(
        &repo.path,
        &contract,
        id(10),
        path("Cargo.toml"),
        UNIX_EPOCH,
    )
    .unwrap();
    let view = cargo_manifest::reinspect(&repo.path, &index, id(13)).unwrap();
    assert_eq!(
        view.extraction_coverage(),
        ExtractionCoverage::Bounded(ExtractionProfile::CargoManifestLexical)
    );
    assert!(!view.reference_resolution().all_references_resolved());
    assert!(matches!(
        view.inspect_claim(claim(
            ReferenceClaimKind::LexicalOccurrence,
            index.path(),
            "/k:features/k:feature/i:0"
        )),
        ClaimInspection::Supported(_)
    ));
    assert!(matches!(
        view.inspect_claim(claim(
            ReferenceClaimKind::SemanticRelationship,
            index.path(),
            "/k:features/k:feature/i:0"
        )),
        ClaimInspection::Unsupported(_)
    ));
}

#[test]
fn refresh_inspection_uses_new_snapshot_resolution_without_implicit_acquisition() {
    let (repo, contract) = fixture(&[
        ("a.json", r##"{"$ref":"b.json#/Id"}"##),
        ("b.json", r#"{"Id":0}"#),
    ]);
    let mut cache = refresh::RefreshCache::new();
    let generation = refresh::refresh_json_source_set(
        &repo.path,
        &contract,
        id(1),
        vec![path("a.json"), path("b.json")],
        UNIX_EPOCH,
        &mut cache,
    )
    .unwrap();
    let latest = inspect_candidate(&repo.path, &contract, id(6)).unwrap();
    let view = refresh::inspect_validity(generation, Some(&latest)).unwrap();
    assert_eq!(
        view.extraction_coverage(),
        ExtractionCoverage::Bounded(ExtractionProfile::JsonSourceSet)
    );
    assert_eq!(
        view.reference_resolution().scope(),
        ResolutionScope::SelectedSourceSet
    );
    assert!(view.reference_resolution().all_references_resolved());
    assert_eq!(view.resolved_projection_validity(), Some(Validity::Current));
    assert!(matches!(
        view.inspect_claim(claim(
            ReferenceClaimKind::LexicalTargetResolution,
            &path("a.json"),
            "/$ref"
        )),
        ClaimInspection::Supported(_)
    ));
    let absent = refresh::inspect_validity(generation, None).unwrap();
    assert!(matches!(
        absent.inspect_claim(claim(
            ReferenceClaimKind::LexicalOccurrence,
            &path("a.json"),
            "/$ref"
        )),
        ClaimInspection::NotEstablished(ClaimGap::SourceCorrespondence)
    ));
    assert!(
        !source_set::inspect_validity(generation.set(), Some(&latest))
            .unwrap()
            .profile_matches()
    );
}
