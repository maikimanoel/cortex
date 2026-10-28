//! Actual Git -> bounded committed source -> typed knowledge -> scoped validity.
#![cfg(target_os = "linux")]
mod support;
use cortex_cli::{inspect, inspect_candidate, knowledge::*};
use cortex_core::{CandidateId, TaskContract, TaskId, binding::*, knowledge::*};
use cortex_evidence::unified::Validity;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use support::Repo;
const SOURCE: &str =
    r##"{"$defs":{"Id":{"type":"string"}},"properties":{"id":{"$ref":"#/$defs/Id"}}}"##;
fn id(n: u64) -> ObservationId {
    ObservationId::new("json-context", n).unwrap()
}
fn contract(r: &Repo) -> CandidateBindingContract {
    let commit = String::from_utf8(r.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("knowledge.integration").unwrap(),
            "Inspect explicit schema references",
            "Retain source and local relations",
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
    path: &[u8],
) -> Result<ReferenceIndex, ExtractionError> {
    extract_json_references(
        &r.path,
        c,
        id(1),
        SourcePath::new(path).unwrap(),
        UNIX_EPOCH,
    )
}
fn snapshot(r: &Repo) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime, u64, bool)> {
    let mut pending = vec![r.path.clone()];
    let mut result = BTreeMap::new();
    while let Some(path) = pending.pop() {
        let m = fs::symlink_metadata(&path).unwrap();
        assert!(!m.is_symlink());
        let bytes = if m.is_dir() {
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
fn useful_incoming_query_retains_real_source_spans_and_read_only_git_state() {
    let r = Repo::new();
    r.commit_file("schema.json", SOURCE);
    let c = contract(&r);
    let before = snapshot(&r);
    let graph = extract(&r, &c, b"schema.json").unwrap();
    let again = extract(&r, &c, b"schema.json").unwrap();
    assert_eq!(graph, again);
    assert_eq!(graph.bytes(), SOURCE.as_bytes());
    let refs: Vec<_> = graph.incoming("/$defs/Id").unwrap().collect();
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].from(), "/properties/id/$ref");
    let span = graph.node(refs[0].from()).unwrap().span();
    assert_eq!(
        &graph.bytes()[span.start()..span.end()],
        br##""#/$defs/Id""##
    );
    let view = reinspect(&r.path, &graph, id(4)).unwrap();
    assert_eq!(view.validity(), Some(Validity::Current));
    assert_eq!(refs[0].origin(), RelationOrigin::Extracted);
    assert_eq!(refs[0].disposition(), ValidationDisposition::Unverified);
    assert_eq!(graph.provenance().version(), PROFILE);
    assert_eq!(snapshot(&r), before);
}
#[test]
fn changed_candidate_or_dirty_same_head_never_relabels_historical_facts_current() {
    let r = Repo::new();
    r.commit_file("schema.json", SOURCE);
    let c = contract(&r);
    let old = extract(&r, &c, b"schema.json").unwrap();
    fs::write(r.path.join("schema.json"), "{\"changed\":true}").unwrap();
    assert_eq!(
        reinspect(&r.path, &old, id(4)).unwrap().validity(),
        Some(Validity::Stale)
    );
    assert!(matches!(
        extract(&r, &c, b"schema.json"),
        Err(ExtractionError::Candidate(_))
    ));
    r.git(&["add", "schema.json"]);
    r.git(&["commit", "-m", "change schema"]);
    assert_eq!(
        reinspect(&r.path, &old, id(5)).unwrap().validity(),
        Some(Validity::Stale)
    );
    assert_eq!(old.incoming("/$defs/Id").unwrap().count(), 1);
    let next_contract = CandidateBindingContract::new(
        c.task().clone(),
        ContractRevision::new(2).unwrap(),
        contract(&r).candidate().clone(),
    );
    let next = extract_json_references(
        &r.path,
        &next_contract,
        id(10),
        SourcePath::new(b"schema.json").unwrap(),
        UNIX_EPOCH,
    )
    .unwrap();
    assert!(next.references().is_empty());
    assert_eq!(
        reinspect(&r.path, &next, id(13)).unwrap().validity(),
        Some(Validity::Current)
    );
}
#[test]
fn unresolved_external_missing_and_unsupported_fragments_keep_validity_unestablished() {
    let r = Repo::new();
    r.commit_file(
        "schema.json",
        r##"[{"$ref":"other.json#/x"},{"$ref":"#/absent"},{"$ref":"#anchor"},{"$ref":"#/%20"}]"##,
    );
    let c = contract(&r);
    let g = extract(&r, &c, b"schema.json").unwrap();
    assert_eq!(g.references().len(), 4);
    assert!(!g.complete());
    assert_eq!(g.nodes().len(), 9);
    assert_eq!(reinspect(&r.path, &g, id(4)).unwrap().validity(), None);
    assert_eq!(qualify(&g, None).unwrap().validity(), None);
    assert!(
        g.references()
            .iter()
            .all(|r| r.disposition() == ValidationDisposition::Unverified)
    );
}
#[test]
fn detached_special_literal_paths_and_subdirectory_invocation_are_supported() {
    let r = Repo::new();
    let filename = "[literal]* ação.json";
    r.commit_file(filename, SOURCE);
    fs::create_dir(r.path.join("sub")).unwrap();
    r.git(&["checkout", "--detach"]);
    let c = contract(&r);
    let g = extract_json_references(
        &r.path.join("sub"),
        &c,
        id(1),
        SourcePath::new(filename.as_bytes()).unwrap(),
        UNIX_EPOCH,
    )
    .unwrap();
    assert_eq!(g.incoming("/$defs/Id").unwrap().count(), 1);
    assert_eq!(g.path().bytes(), filename.as_bytes());
    assert!(matches!(
        g.candidate().observation().state().head(),
        Head::Detached { .. }
    ));
    use std::os::unix::ffi::OsStringExt;
    let raw = std::ffi::OsString::from_vec(b"bytes-\xff.json".to_vec());
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
    let raw_index = extract(&r, &contract(&r), b"bytes-\xff.json").unwrap();
    assert_eq!(raw_index.path().bytes(), b"bytes-\xff.json");
    assert_eq!(raw_index.incoming("/$defs/Id").unwrap().count(), 1);
}
#[test]
fn malformed_duplicate_oversized_and_missing_sources_are_explicit_failures() {
    let r = Repo::new();
    r.commit_file("schema.json", "{\"same\":1,\"same\":2}");
    let c = contract(&r);
    assert!(matches!(
        extract(&r, &c, b"schema.json"),
        Err(ExtractionError::InvalidJson)
    ));
    assert!(matches!(
        extract(&r, &c, b"missing"),
        Err(ExtractionError::Inspection(_))
    ));
    r.commit_file("schema.json", &" ".repeat(MAX_SOURCE_BYTES + 1));
    assert!(matches!(
        extract(&r, &contract(&r), b"schema.json"),
        Err(ExtractionError::Inspection(_))
    ));
}
#[test]
fn wrong_repository_and_unborn_state_cannot_create_a_bound_index() {
    let r = Repo::new();
    r.commit_file("schema.json", SOURCE);
    let c = contract(&r);
    let other = Repo::new();
    other.commit_file("schema.json", SOURCE);
    assert!(matches!(
        extract(&other, &c, b"schema.json"),
        Err(ExtractionError::Candidate(_))
    ));
    let unborn = Repo::new();
    assert!(matches!(
        extract(&unborn, &c, b"schema.json"),
        Err(ExtractionError::MissingCommit)
    ));
    let graph = extract(&r, &c, b"schema.json").unwrap();
    let mut latest = inspect_candidate(&r.path, &c, id(4)).unwrap();
    latest = CandidateObservation::new(
        latest.id().clone(),
        latest.contract(),
        RepositoryState::new(
            None,
            Head::Unknown,
            vec![],
            ObservationCoverage::Incomplete,
            ObservationConsistency::Indeterminate,
        )
        .unwrap(),
        None,
    );
    assert_eq!(qualify(&graph, Some(&latest)).unwrap().validity(), None);
}

#[test]
fn changed_extractor_profile_cannot_reuse_contextual_current_validity() {
    let r = Repo::new();
    r.commit_file("schema.json", SOURCE);
    let c = contract(&r);
    let original = extract(&r, &c, b"schema.json").unwrap();
    let older = ReferenceIndex::new(
        original.candidate().clone(),
        original.path().clone(),
        original.bytes().to_vec(),
        ExtractionRecord::new(
            ObservationId::new("older-extractor", 1).unwrap(),
            ADAPTER,
            "old-rules",
            UNIX_EPOCH,
        )
        .unwrap(),
        original.nodes().to_vec(),
        original.references().to_vec(),
    )
    .unwrap();
    let view = reinspect(&r.path, &older, id(4)).unwrap();
    assert!(!view.profile_matches());
    assert_eq!(view.validity(), Some(Validity::Stale));
    assert_eq!(view.index().incoming("/$defs/Id").unwrap().count(), 1);
}
#[test]
fn symlink_sources_and_missing_local_objects_are_rejected_without_fallback() {
    let r = Repo::new();
    r.commit_file("schema.json", SOURCE);
    let c = contract(&r);
    std::os::unix::fs::symlink("schema.json", r.path.join("link.json")).unwrap();
    r.git(&["add", "link.json"]);
    r.git(&["commit", "-m", "link"]);
    assert!(matches!(
        extract(&r, &contract(&r), b"link.json"),
        Err(ExtractionError::Inspection(_))
    ));
    let oid = String::from_utf8(r.git(&["rev-parse", "HEAD:schema.json"]).stdout).unwrap();
    let oid = oid.trim();
    fs::remove_file(r.path.join(".git/objects").join(&oid[..2]).join(&oid[2..])).unwrap();
    assert!(matches!(
        extract(&r, &c, b"schema.json"),
        Err(ExtractionError::Inspection(_))
    ));
}
