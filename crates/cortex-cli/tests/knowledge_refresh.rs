//! PP02 / CAP-KNW-001 / 003-B: producer-owned refresh against real local Git.
#![cfg(target_os = "linux")]
mod support;

use cortex_cli::{
    inspect, inspect_candidate,
    knowledge::{ExtractionError, refresh::*, source_set},
};
use cortex_core::{
    CandidateId, TaskContract, TaskId,
    binding::*,
    knowledge::{source_set::*, *},
};
use cortex_evidence::unified::Validity;
use std::{
    collections::BTreeMap,
    error::Error,
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

const A: &str = r##"{"Id":0,"use":{"$ref":"b.json#/Id"}}"##;
const B: &str = r##"{"Id":1}"##;
type RefreshError = source_set::SourceSetExtractionError;

fn id(n: u64) -> ObservationId {
    ObservationId::new("refresh", n).unwrap()
}
fn path(p: &[u8]) -> SourcePath {
    SourcePath::new(p).unwrap()
}
fn repo() -> Repo {
    let r = Repo::new();
    r.commit_file("a.json", A);
    r.commit_file("b.json", B);
    r
}
fn contract(r: &Repo) -> CandidateBindingContract {
    let commit = String::from_utf8(r.git(&["rev-parse", "HEAD"]).stdout).unwrap();
    let revision = String::from_utf8(r.git(&["rev-list", "--count", "HEAD"]).stdout).unwrap();
    CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("knowledge.refresh").unwrap(),
            "Refresh selected JSON",
            "Bind fresh bytes and rebuild selected resolution",
        )
        .unwrap(),
        ContractRevision::new(revision.trim().parse().unwrap()).unwrap(),
        CandidateRequirement::new(
            inspect(&r.path).unwrap().repository_identity().unwrap(),
            CandidateId::from_git_commit(commit.trim()).unwrap(),
            WorktreeRequirement::Clean,
        ),
    )
}
fn refresh<'a>(
    r: &Repo,
    c: &CandidateBindingContract,
    n: u64,
    names: &[&[u8]],
    cache: &'a mut RefreshCache,
) -> Result<&'a RefreshGeneration, RefreshError> {
    refresh_json_source_set(
        &r.path,
        c,
        id(n),
        names.iter().map(|p| path(p)).collect(),
        UNIX_EPOCH + Duration::from_secs(n),
        cache,
    )
}
fn both<'a>(r: &Repo, n: u64, cache: &'a mut RefreshCache) -> &'a RefreshGeneration {
    refresh(r, &contract(r), n, &[b"a.json", b"b.json"], cache).unwrap()
}

// Compare contracted semantic fields, not whole records with provenance stripped.
fn assert_full_reconstruction(r: &Repo, generation: &RefreshGeneration) {
    let set = generation.set();
    let full = source_set::extract_json_source_set(
        &r.path,
        set.candidate().observation().contract(),
        ObservationId::new("full", 1).unwrap(),
        set.sources().iter().map(|s| s.path().clone()).collect(),
        UNIX_EPOCH,
    )
    .unwrap();
    assert_eq!(set.sources().len(), full.sources().len());
    for (actual, expected) in set.sources().iter().zip(full.sources()) {
        assert_eq!(actual.path().bytes(), expected.path().bytes());
        assert_eq!(actual.bytes(), expected.bytes());
        assert_eq!(actual.complete(), expected.complete());
        assert_eq!(actual.nodes().len(), expected.nodes().len());
        for (a, e) in actual.nodes().iter().zip(expected.nodes()) {
            assert_eq!(a.locator(), e.locator());
            assert_eq!(a.span(), e.span());
            assert_eq!(
                &actual.bytes()[a.span().start()..a.span().end()],
                &expected.bytes()[e.span().start()..e.span().end()]
            );
        }
        assert_eq!(actual.references().len(), expected.references().len());
        for (a, e) in actual.references().iter().zip(expected.references()) {
            assert_eq!(a.from(), e.from());
            assert_eq!(a.spelling(), e.spelling());
            assert_eq!(a.target(), e.target());
            assert_eq!(a.origin(), RelationOrigin::Extracted);
            assert_eq!(a.disposition(), ValidationDisposition::Unverified);
        }
    }
    assert_eq!(set.complete(), full.complete());
    assert_eq!(set.references().len(), full.references().len());
    for (a, e) in set.references().zip(full.references()) {
        assert_eq!(a.from().path(), e.from().path());
        assert_eq!(a.from().node().locator(), e.from().node().locator());
        assert_eq!(a.reference().spelling(), e.reference().spelling());
        match (a.target(), e.target()) {
            (Resolution::Resolved(a), Resolution::Resolved(e)) => {
                assert_eq!(a.path(), e.path());
                assert_eq!(a.node().locator(), e.node().locator());
                assert_eq!(a.node().span(), e.node().span());
            }
            (Resolution::Unresolved(a), Resolution::Unresolved(e)) => assert_eq!(a, e),
            _ => panic!("resolution disagrees with full reconstruction"),
        }
    }
    assert_ne!(set.id(), full.id());
    assert_eq!(set.provenance().source(), ADAPTER);
    assert_eq!(full.provenance().source(), source_set::ADAPTER);
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

#[test]
fn cold_refresh_has_independently_expected_nodes_spans_edges_and_fresh_provenance() {
    let r = repo();
    let c = contract(&r);
    let before = snapshot(&r);
    let mut cache = RefreshCache::new();
    assert!(cache.retained().is_none());
    let generation = refresh(&r, &c, 1, &[b"b.json", b"a.json"], &mut cache).unwrap();
    let set = generation.set();
    assert_eq!(set.candidate().baseline(), &id(1));
    assert_eq!(set.candidate().observation().id(), &id(2));
    assert_eq!(set.sources()[0].id(), &id(3));
    assert_eq!(set.sources()[1].id(), &id(4));
    assert_eq!(set.id(), &id(5));
    assert_eq!(set.sources()[0].bytes(), A.as_bytes());
    let nodes: Vec<_> = set.sources()[0]
        .nodes()
        .iter()
        .map(|n| (n.locator(), n.span().start(), n.span().end()))
        .collect();
    assert_eq!(
        nodes,
        vec![
            ("", 0, 36),
            ("/Id", 6, 7),
            ("/use", 14, 35),
            ("/use/$ref", 22, 34)
        ]
    );
    let incoming: Vec<_> = set.incoming(&path(b"b.json"), "/Id").unwrap().collect();
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].from().path().bytes(), b"a.json");
    assert_eq!(incoming[0].from().node().locator(), "/use/$ref");
    assert_eq!(incoming[0].reference().spelling(), "b.json#/Id");
    assert_eq!(
        set.sources()[0].references()[0].target(),
        &ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage)
    );
    assert!(!set.sources()[0].complete());
    assert!(set.complete());
    for (p, s) in generation.parsing().iter().zip(set.sources()) {
        assert!(p.reused_from().is_none());
        assert_eq!(p.origin(), s.id());
        assert_eq!(p.path(), s.path());
        assert_eq!(p.parser(), cortex_cli::knowledge::ADAPTER);
        assert_eq!(p.parser_profile(), cortex_cli::knowledge::PROFILE);
        assert!(s.provenance().version().contains("mode=parsed"));
    }
    assert_full_reconstruction(&r, generation);
    assert_eq!(snapshot(&r), before);
}

#[test]
fn unchanged_bytes_on_new_candidate_reuse_parsing_but_rebind_every_identity() {
    let r = repo();
    let mut cache = RefreshCache::new();
    let original = both(&r, 1, &mut cache).clone();
    r.commit_file("unselected", "new committed candidate");
    let before = snapshot(&r);
    let next = both(&r, 10, &mut cache);
    assert_ne!(next.set().candidate(), original.set().candidate());
    assert_eq!(next.set().candidate().baseline(), &id(10));
    assert_eq!(next.set().candidate().observation().id(), &id(11));
    assert_eq!(next.set().id(), &id(14));
    for ((p, s), old) in next
        .parsing()
        .iter()
        .zip(next.set().sources())
        .zip(original.set().sources())
    {
        assert_eq!(p.reused_from(), Some(old.id()));
        assert_eq!(p.origin(), old.id());
        assert_eq!(p.recorded_at(), old.recorded_at());
        assert_ne!(s.id(), old.id());
        assert_eq!(s.candidate(), next.set().candidate());
        assert_eq!(s.recorded_at(), UNIX_EPOCH + Duration::from_secs(10));
        assert!(s.provenance().version().contains("mode=reused"));
        assert!(s.provenance().version().contains("origin=refresh:"));
    }
    assert_eq!(
        reinspect(&r.path, &original, id(20)).unwrap().validity(),
        Some(Validity::Stale)
    );
    assert_eq!(
        reinspect(&r.path, next, id(20)).unwrap().validity(),
        Some(Validity::Current)
    );
    assert_full_reconstruction(&r, next);
    assert_eq!(snapshot(&r), before);
    // Reuse of reuse retains original parse, and names the immediate copied source.
    let prior = next.clone();
    let again = both(&r, 30, &mut cache);
    assert_eq!(again.parsing()[0].origin(), original.parsing()[0].origin());
    assert_eq!(
        again.parsing()[0].reused_from(),
        Some(prior.set().sources()[0].id())
    );
}

#[test]
fn whitespace_byte_changes_reparse_and_retain_exact_shifted_spans() {
    let r = repo();
    let mut cache = RefreshCache::new();
    let original = both(&r, 1, &mut cache).clone();
    r.commit_file("a.json", &format!(" \n{A} "));
    let next = both(&r, 10, &mut cache);
    assert!(next.parsing()[0].reused_from().is_none());
    assert_eq!(next.parsing()[0].origin(), &id(12));
    assert_eq!(next.parsing()[1].reused_from(), Some(&id(4)));
    assert_eq!(
        next.set().sources()[0].bytes(),
        format!(" \n{A} ").as_bytes()
    );
    for (a, e) in next.set().sources()[0]
        .nodes()
        .iter()
        .zip(original.set().sources()[0].nodes())
    {
        assert_eq!(a.locator(), e.locator());
        assert_eq!(a.span().start(), e.span().start() + 2);
        assert_eq!(a.span().end(), e.span().end() + 2);
    }
    assert_full_reconstruction(&r, next);
}

#[test]
fn changed_other_document_rebuilds_resolution_for_unchanged_source_and_retarget() {
    let r = repo();
    let mut cache = RefreshCache::new();
    both(&r, 1, &mut cache);
    r.commit_file("b.json", r#"{"Other":1}"#);
    let missing = both(&r, 10, &mut cache);
    assert_eq!(missing.parsing()[0].reused_from(), Some(&id(3)));
    assert!(matches!(
        missing.set().references().next().unwrap().target(),
        Resolution::Unresolved(UnresolvedReference::MissingTarget)
    ));
    assert!(!missing.set().complete());
    assert_full_reconstruction(&r, missing);
    r.commit_file("a.json", r##"{"Id":0,"use":{"$ref":"b.json#/Other"}}"##);
    let retargeted = both(&r, 20, &mut cache);
    assert!(retargeted.parsing()[0].reused_from().is_none());
    assert_eq!(
        retargeted
            .set()
            .incoming(&path(b"b.json"), "/Other")
            .unwrap()
            .count(),
        1
    );
    assert_full_reconstruction(&r, retargeted);
    r.commit_file("b.json", B);
    let missing = both(&r, 30, &mut cache);
    assert!(matches!(
        missing.set().references().next().unwrap().target(),
        Resolution::Unresolved(UnresolvedReference::MissingTarget)
    ));
    assert_full_reconstruction(&r, missing);
}

#[test]
fn selection_remove_readd_and_reorder_do_not_infer_repository_deletion() {
    let r = repo();
    let c = contract(&r);
    let mut cache = RefreshCache::new();
    let old = both(&r, 1, &mut cache).clone();
    let only = refresh(&r, &c, 10, &[b"a.json"], &mut cache).unwrap();
    assert_eq!(only.set().sources().len(), 1);
    assert!(matches!(
        only.set().references().next().unwrap().target(),
        Resolution::Unresolved(UnresolvedReference::OutsideCoverage)
    ));
    assert_eq!(fs::read(r.path.join("b.json")).unwrap(), B.as_bytes());
    assert_full_reconstruction(&r, only);
    let restored = refresh(&r, &c, 20, &[b"b.json", b"a.json"], &mut cache).unwrap();
    assert!(restored.parsing()[0].reused_from().is_some());
    assert!(restored.parsing()[1].reused_from().is_none());
    assert_eq!(
        restored
            .set()
            .incoming(&path(b"b.json"), "/Id")
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        old.set().incoming(&path(b"b.json"), "/Id").unwrap().count(),
        1
    );
    assert_full_reconstruction(&r, restored);
    let mut empty = RefreshCache::new();
    let ordered = refresh(&r, &c, 1, &[b"a.json", b"b.json"], &mut empty)
        .unwrap()
        .clone();
    let mut empty = RefreshCache::new();
    assert_eq!(
        &ordered,
        refresh(&r, &c, 1, &[b"b.json", b"a.json"], &mut empty).unwrap()
    );
}

#[test]
fn removed_reference_and_removed_committed_source_are_fresh_observations() {
    let r = repo();
    let mut cache = RefreshCache::new();
    both(&r, 1, &mut cache);
    r.commit_file("a.json", "{}");
    let next = both(&r, 10, &mut cache);
    assert_eq!(next.set().references().len(), 0);
    assert_full_reconstruction(&r, next);
    let prior = next.clone();
    r.git(&["rm", "b.json"]);
    r.git(&["commit", "-m", "remove source"]);
    assert!(matches!(
        refresh(&r, &contract(&r), 20, &[b"a.json", b"b.json"], &mut cache),
        Err(RefreshError::Extraction(ExtractionError::Inspection(_)))
    ));
    assert_eq!(cache.retained(), Some(&prior));
}

#[test]
fn malformed_changed_inputs_and_unsupported_values_preserve_the_entire_cache() {
    let r = repo();
    let mut cache = RefreshCache::new();
    let prior = both(&r, 1, &mut cache).clone();
    for (text, unsupported) in [
        ("{\"secret-924\":1,\"secret-924\":2}", false),
        ("{} trailing", false),
        ("{\"$ref\":42}", true),
    ] {
        r.commit_file("b.json", text);
        let before = snapshot(&r);
        let error =
            refresh(&r, &contract(&r), 10, &[b"a.json", b"b.json"], &mut cache).unwrap_err();
        assert!(matches!(
            (&error, unsupported),
            (
                RefreshError::Extraction(ExtractionError::InvalidJson),
                false
            ) | (RefreshError::Extraction(ExtractionError::Unsupported), true)
        ));
        assert!(!format!("{error:?} {error}").contains("secret-924"));
        assert_eq!(cache.retained(), Some(&prior));
        assert_eq!(snapshot(&r), before);
    }
    fs::write(r.path.join("b.json"), b"\xff").unwrap();
    r.git(&["add", "b.json"]);
    r.git(&["commit", "-m", "invalid utf8"]);
    assert!(matches!(
        refresh(&r, &contract(&r), 10, &[b"a.json", b"b.json"], &mut cache),
        Err(RefreshError::Extraction(ExtractionError::InvalidJson))
    ));
    assert_eq!(cache.retained(), Some(&prior));
}

#[test]
fn dirty_same_head_wrong_repository_candidate_and_unborn_never_publish() {
    let r = repo();
    let c = contract(&r);
    let mut cache = RefreshCache::new();
    let prior = both(&r, 1, &mut cache).clone();
    for text in ["2", "3"] {
        fs::write(r.path.join("b.json"), text).unwrap();
        let before = snapshot(&r);
        assert!(matches!(
            refresh(&r, &c, 10, &[b"a.json", b"b.json"], &mut cache),
            Err(RefreshError::Extraction(ExtractionError::Candidate(_)))
        ));
        assert_eq!(cache.retained(), Some(&prior));
        assert_eq!(snapshot(&r), before);
        assert_eq!(
            reinspect(&r.path, &prior, id(20)).unwrap().validity(),
            Some(Validity::Stale)
        );
    }
    r.git(&["add", "b.json"]);
    r.git(&["commit", "-m", "different candidate"]);
    assert!(matches!(
        refresh(&r, &c, 10, &[b"a.json", b"b.json"], &mut cache),
        Err(RefreshError::Extraction(ExtractionError::Candidate(_)))
    ));
    let other = repo();
    assert!(matches!(
        refresh(&other, &c, 10, &[b"a.json", b"b.json"], &mut cache),
        Err(RefreshError::Extraction(ExtractionError::Candidate(_)))
    ));
    let unborn = Repo::new();
    assert!(matches!(
        refresh(&unborn, &c, 10, &[b"a.json"], &mut cache),
        Err(RefreshError::Extraction(ExtractionError::MissingCommit))
    ));
    assert_eq!(cache.retained(), Some(&prior));
}

#[test]
fn input_bounds_overflow_and_known_id_collisions_fail_before_repository_io() {
    let r = repo();
    let c = contract(&r);
    let mut cache = RefreshCache::new();
    let prior = both(&r, 1, &mut cache).clone();
    let absent = r.path.join("no-such-repository");
    for (names, n, expected) in [
        (vec![], 10, SourceSetError::Empty),
        (vec![path(b"a.json"); 9], 10, SourceSetError::Limit),
        (
            vec![path(b"a.json"); 2],
            10,
            SourceSetError::DuplicateSource,
        ),
        (
            vec![path(b"a.json")],
            u64::MAX,
            SourceSetError::InvalidProvenance,
        ),
        (
            vec![path(b"a.json")],
            u64::MAX - 2,
            SourceSetError::InvalidProvenance,
        ),
        (vec![path(b"a.json")], 5, SourceSetError::InvalidProvenance),
        (vec![path(b"a.json")], 1, SourceSetError::InvalidProvenance),
    ] {
        let error =
            refresh_json_source_set(&absent, &c, id(n), names, UNIX_EPOCH, &mut cache).unwrap_err();
        assert!(matches!(error,RefreshError::Collection(e) if e==expected));
        assert_eq!(cache.retained(), Some(&prior));
    }
    assert!(matches!(
        refresh_json_source_set(
            &absent,
            &c,
            id(10),
            vec![path(b"a.json")],
            UNIX_EPOCH - Duration::from_secs(1),
            &mut cache
        ),
        Err(RefreshError::Extraction(ExtractionError::Domain(
            KnowledgeError::InvalidProvenance
        )))
    ));
    assert_eq!(cache.retained(), Some(&prior));
    // Exactly fitting allocation at the u64 boundary succeeds in debug and release.
    let last = refresh_json_source_set(
        &r.path,
        &c,
        id(u64::MAX - 3),
        vec![path(b"a.json")],
        UNIX_EPOCH,
        &mut cache,
    )
    .unwrap();
    assert_eq!(last.set().id().sequence(), u64::MAX);
}

#[test]
fn source_and_set_resource_limits_preserve_prior_generation() {
    let r = repo();
    let mut cache = RefreshCache::new();
    let prior = both(&r, 1, &mut cache).clone();
    for (text, inspection_limit) in [
        (format!("\"{}\"", "s".repeat(MAX_SOURCE_BYTES - 1)), true),
        (format!("[{}]", vec!["0"; MAX_NODES].join(",")), false),
        (
            format!(
                "[{}]",
                vec![r##"{"$ref":"#"}"##; MAX_REFERENCES + 1].join(",")
            ),
            false,
        ),
        (format!("{}0{}", "[".repeat(33), "]".repeat(33)), false),
    ] {
        r.commit_file("b.json", &text);
        let error =
            refresh(&r, &contract(&r), 10, &[b"a.json", b"b.json"], &mut cache).unwrap_err();
        assert!(
            matches!(
                (&error, inspection_limit),
                (
                    RefreshError::Extraction(ExtractionError::Inspection(
                        cortex_cli::InspectionError::Limit(_)
                    )),
                    true
                ) | (RefreshError::Extraction(ExtractionError::Limit), false)
            ),
            "{error:?}"
        );
        assert_eq!(cache.retained(), Some(&prior));
    }
    // Each source is valid individually; only the aggregate is excessive.
    let source = format!("\"{}\"", "s".repeat(MAX_SOURCE_BYTES - 2));
    for name in ["a.json", "b.json", "c.json", "d.json", "e.json"] {
        r.commit_file(name, &source);
    }
    assert!(matches!(
        refresh(
            &r,
            &contract(&r),
            10,
            &[b"a.json", b"b.json", b"c.json", b"d.json", b"e.json"],
            &mut cache
        ),
        Err(RefreshError::Collection(SourceSetError::Limit))
    ));
    assert_eq!(cache.retained(), Some(&prior));
    let exact = refresh(
        &r,
        &contract(&r),
        10,
        &[b"a.json", b"b.json", b"c.json", b"d.json"],
        &mut cache,
    )
    .unwrap();
    assert_eq!(
        exact
            .set()
            .sources()
            .iter()
            .map(|s| s.bytes().len())
            .sum::<usize>(),
        MAX_SET_BYTES
    );
}

#[test]
fn aggregate_nodes_and_references_are_bounded_including_reused_inputs() {
    let r = repo();
    let mut cache = RefreshCache::new();
    let prior = both(&r, 1, &mut cache).clone();
    for refs in [false, true] {
        let text = if refs {
            format!("[{}]", vec![r##"{"$ref":"#"}"##; 1024].join(","))
        } else {
            format!("[{}]", vec!["0"; 4095].join(","))
        };
        for name in ["a.json", "b.json", "c.json"] {
            r.commit_file(name, &text);
        }
        assert!(matches!(
            refresh(
                &r,
                &contract(&r),
                10,
                &[b"a.json", b"b.json", b"c.json"],
                &mut cache
            ),
            Err(RefreshError::Collection(SourceSetError::Limit))
        ));
        assert_eq!(cache.retained(), Some(&prior));
    }
    let exact = both(&r, 10, &mut cache).clone();
    assert_eq!(exact.set().references().len(), MAX_SET_REFERENCES);
    assert!(matches!(
        refresh(
            &r,
            &contract(&r),
            20,
            &[b"a.json", b"b.json", b"c.json"],
            &mut cache
        ),
        Err(RefreshError::Collection(SourceSetError::Limit))
    ));
    assert_eq!(cache.retained(), Some(&exact));
}

#[test]
fn every_selected_blob_is_reacquired_even_when_parsing_could_be_reused() {
    let r = repo();
    let c = contract(&r);
    let mut cache = RefreshCache::new();
    let prior = both(&r, 1, &mut cache).clone();
    let oid = String::from_utf8(r.git(&["rev-parse", "HEAD:b.json"]).stdout).unwrap();
    let oid = oid.trim();
    fs::remove_file(r.path.join(".git/objects").join(&oid[..2]).join(&oid[2..])).unwrap();
    let before = snapshot(&r);
    let error = refresh(&r, &c, 10, &[b"a.json", b"b.json"], &mut cache).unwrap_err();
    assert!(matches!(
        error,
        RefreshError::Extraction(ExtractionError::Inspection(_))
    ));
    assert!(error.source().is_some());
    assert_eq!(cache.retained(), Some(&prior));
    assert_eq!(snapshot(&r), before);
}

#[test]
fn acquisition_rejects_symlinks_without_partial_publication() {
    let r = repo();
    let mut cache = RefreshCache::new();
    let prior = both(&r, 1, &mut cache).clone();
    std::os::unix::fs::symlink("b.json", r.path.join("link")).unwrap();
    r.git(&["add", "link"]);
    r.git(&["commit", "-m", "symlink"]);
    let before = snapshot(&r);
    assert!(matches!(
        refresh(&r, &contract(&r), 10, &[b"a.json", b"link"], &mut cache),
        Err(RefreshError::Extraction(ExtractionError::Inspection(
            cortex_cli::InspectionError::Unsupported(_)
        )))
    ));
    assert_eq!(cache.retained(), Some(&prior));
    assert_eq!(snapshot(&r), before);
}

#[test]
fn actual_refresh_consumer_distinguishes_current_stale_unestablished_and_legacy_profile() {
    let r = repo();
    let c = contract(&r);
    let mut cache = RefreshCache::new();
    let g = both(&r, 1, &mut cache);
    assert_eq!(qualify(g, None).unwrap().validity(), None);
    for n in 3..=5 {
        let view = reinspect(&r.path, g, id(n)).unwrap();
        assert_eq!(view.validity(), Some(Validity::Stale));
        assert_eq!(
            view.correspondence(),
            &BindingDecision::Stale(BindingStaleness::ObservationOrder)
        );
    }
    let view = reinspect(&r.path, g, id(6)).unwrap();
    assert!(view.profile_matches());
    assert_eq!(view.validity(), Some(Validity::Current));
    assert_eq!(view.generation(), g);
    let latest = inspect_candidate(&r.path, &c, id(6)).unwrap();
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
    assert_eq!(qualify(g, Some(&incomplete)).unwrap().validity(), None);
    let legacy = source_set::reinspect(&r.path, g.set(), id(8)).unwrap();
    assert!(!legacy.profile_matches());
    assert_eq!(legacy.validity(), Some(Validity::Stale));
    let only = refresh(&r, &c, 10, &[b"a.json"], &mut cache).unwrap();
    assert_eq!(reinspect(&r.path, only, id(20)).unwrap().validity(), None);
}

#[test]
fn literal_private_byte_paths_are_retained_and_debug_errors_do_not_disclose() {
    let r = repo();
    let raw = b"private-sentinel-924-\xff.json";
    let name = OsString::from_vec(raw.to_vec());
    let private = r#"{"private-payload-924":0}"#;
    fs::write(r.path.join(&name), private).unwrap();
    assert!(
        r.command()
            .arg("add")
            .arg("--")
            .arg(&name)
            .status()
            .unwrap()
            .success()
    );
    r.git(&["commit", "-m", "literal bytes"]);
    let mut cache = RefreshCache::new();
    let old = refresh(&r, &contract(&r), 1, &[raw], &mut cache)
        .unwrap()
        .clone();
    let next = refresh(&r, &contract(&r), 10, &[raw], &mut cache).unwrap();
    assert_eq!(next.set().sources()[0].path().bytes(), raw);
    assert_eq!(next.set().sources()[0].bytes(), private.as_bytes());
    assert!(next.parsing()[0].reused_from().is_some());
    assert_eq!(next.parsing()[0].origin(), old.parsing()[0].origin());
    assert_full_reconstruction(&r, next);
    let debug = format!("{cache:?}");
    for secret in ["private-sentinel-924", "private-payload-924"] {
        assert!(!debug.contains(secret));
    }
}

#[test]
fn identical_bytes_at_a_different_path_require_a_new_parse_and_maximum_selection_works() {
    let r = repo();
    let mut cache = RefreshCache::new();
    let old = both(&r, 1, &mut cache).clone();
    r.git(&["mv", "a.json", "renamed.json"]);
    for name in ["c.json", "d.json", "e.json", "f.json", "g.json", "h.json"] {
        fs::write(r.path.join(name), "0").unwrap();
    }
    r.git(&["add", "."]);
    r.git(&["commit", "-m", "move and add selected sources"]);
    let g = refresh(
        &r,
        &contract(&r),
        10,
        &[
            b"renamed.json",
            b"b.json",
            b"c.json",
            b"d.json",
            b"e.json",
            b"f.json",
            b"g.json",
            b"h.json",
        ],
        &mut cache,
    )
    .unwrap();
    assert_eq!(g.set().sources().len(), MAX_SOURCES);
    let renamed = g
        .set()
        .sources()
        .iter()
        .find(|s| s.path().bytes() == b"renamed.json")
        .unwrap();
    assert_eq!(renamed.bytes(), old.set().sources()[0].bytes());
    let parse = g
        .parsing()
        .iter()
        .find(|s| s.path().bytes() == b"renamed.json")
        .unwrap();
    assert!(parse.reused_from().is_none());
    assert_eq!(
        g.set().incoming(&path(b"b.json"), "/Id").unwrap().count(),
        1
    );
    assert_full_reconstruction(&r, g);
}

#[test]
fn delta_preserves_declared_profile_boundaries_for_actual_refresh_generations() {
    use cortex_core::knowledge::delta::{DeltaError, compare};

    let r = repo();
    let c = contract(&r);
    let mut cache = RefreshCache::new();
    let original = both(&r, 1, &mut cache).clone();
    let legacy = source_set::extract_json_source_set(
        &r.path,
        &c,
        ObservationId::new("legacy-comparison", 1).unwrap(),
        vec![path(b"a.json"), path(b"b.json")],
        UNIX_EPOCH,
    )
    .unwrap();
    assert_eq!(
        compare(&legacy, original.set()).unwrap_err(),
        DeltaError::CollectionMechanismMismatch
    );

    let reused = both(&r, 10, &mut cache);
    assert_eq!(
        original.set().sources()[0].bytes(),
        reused.set().sources()[0].bytes()
    );
    assert!(reused.parsing()[0].reused_from().is_some());
    // Parse/reuse provenance is part of the exact source profile. The comparator
    // must not normalize it or infer equivalence from the unchanged bytes.
    assert_eq!(
        compare(original.set(), reused.set()).unwrap_err(),
        DeltaError::SourceProfileMismatch
    );
    let same = compare(reused.set(), reused.set()).unwrap();
    assert_eq!(same.source_changes().count(), 0);
    assert_eq!(same.node_changes().count(), 0);
    assert_eq!(same.reference_changes().count(), 0);
    assert_full_reconstruction(&r, reused);
}
