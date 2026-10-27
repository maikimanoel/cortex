//! CAP-MEM-001 temporal semantics, atomic transitions, bounds and privacy.

use cortex_core::{CandidateId, ContractError, TaskContract, TaskId, binding::*, temporal::*};
use std::{
    error::Error,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn context() -> CandidateObservation {
    // Synthetic domain fixture; this is not an assertion that a Git object exists.
    let root = std::env::temp_dir().join("cortex-temporal-synthetic");
    let repository =
        RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git")).unwrap();
    let commit = CandidateId::from_git_commit(&"a".repeat(40)).unwrap();
    let contract = CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("temporal.fixture").unwrap(),
            "Preserve records",
            "Retain historical perspectives",
        )
        .unwrap(),
        ContractRevision::new(1).unwrap(),
        CandidateRequirement::new(
            repository.clone(),
            commit.clone(),
            WorktreeRequirement::AllowChanges,
        ),
    );
    CandidateObservation::new(
        capture(),
        &contract,
        RepositoryState::new(
            Some(repository),
            Head::Detached { commit },
            vec![],
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
        )
        .unwrap(),
        None,
    )
}

fn capture() -> ObservationId {
    ObservationId::new("cap", 1).unwrap()
}

fn entity(value: &str) -> RecordId {
    RecordId::new(value).unwrap()
}

fn version(value: &str) -> RecordVersionId {
    RecordVersionId::new(value).unwrap()
}

fn content(body: &str) -> RecordContent {
    RecordContent::new(
        body,
        vec![SourceReference::operator_reported("o", "s", "1").unwrap()],
    )
    .unwrap()
}

fn time(from: u64, until: Option<u64>, recorded: u64) -> RecordTime {
    RecordTime::new(
        DomainInterval::new(at(from), until.map(at)).unwrap(),
        at(recorded),
    )
    .unwrap()
}

fn initial(name: &str, id: &str, kind: RecordKind, body: &str, t: RecordTime) -> CanonicalRecord {
    CanonicalRecord::initial(entity(name), version(id), kind, capture(), content(body), t)
}

fn correction(id: &str, previous: &str, body: &str, t: RecordTime) -> CanonicalRecord {
    CanonicalRecord::correction(
        entity("e"),
        version(id),
        RecordKind::Requirement,
        capture(),
        version(previous),
        content(body),
        t,
    )
}

fn selected(history: &CanonicalHistory, domain: u64, cutoff: u64) -> Option<&str> {
    history
        .query(
            &entity("e"),
            &TemporalQuery::new(capture(), at(domain), at(cutoff)).unwrap(),
        )
        .unwrap()
        .record()
        .map(|r| r.id().as_str())
}

fn assert_rejected(
    history: &mut CanonicalHistory,
    entry: CanonicalRecord,
    expected: TemporalError,
) {
    let before = history.clone();
    assert_eq!(history.append(entry), Err(expected));
    // Includes context, retained originals, provenance, order and byte accounting.
    assert_eq!(*history, before);
}

#[test]
fn requirement_and_decision_retain_distinct_identity_provenance_and_context() {
    let c = context();
    let mut history = CanonicalHistory::new(c.clone());
    history
        .append(initial(
            "e",
            "r1",
            RecordKind::Requirement,
            "Keep the contract",
            time(0, None, 10),
        ))
        .unwrap();
    let observed_source = SourceReference::observation_reference(
        ObservationId::new("source", 7).unwrap(),
        "bounded inspection",
        "v2",
    )
    .unwrap();
    let decision = CanonicalRecord::initial(
        entity("decision"),
        version("d1"),
        RecordKind::Decision,
        capture(),
        RecordContent::new("Choose a bounded scope", vec![observed_source.clone()]).unwrap(),
        time(20, None, 1),
    );
    history.append(decision.clone()).unwrap();
    assert_eq!(history.context(), &c);
    assert_eq!(
        history
            .records()
            .iter()
            .map(|r| r.id().as_str())
            .collect::<Vec<_>>(),
        ["r1", "d1"]
    );
    assert_eq!(history.version(&version("d1")).unwrap(), &decision);
    let chain = history.history(&entity("decision")).unwrap();
    assert_eq!(chain.entity(), &entity("decision"));
    assert_eq!(chain.context(), &c);
    assert_eq!(chain.records().count(), 1);
    assert_eq!(decision.content().sources(), &[observed_source]);
    assert!(
        matches!(decision.content().sources()[0].origin(), SourceOrigin::ObservationReference(id) if id.sequence() == 7)
    );
    assert!(
        matches!(history.records()[0].content().sources()[0].origin(), SourceOrigin::OperatorReported { author } if author == "o")
    );
    assert_eq!(
        decision.content().sources()[0].source(),
        "bounded inspection"
    );
    assert_eq!(decision.content().sources()[0].version(), "v2");
}

#[test]
fn late_corrections_use_both_axes_without_erasing_prior_intervals() {
    let mut h = CanonicalHistory::new(context());
    let original = initial(
        "e",
        "a",
        RecordKind::Requirement,
        "original",
        time(5, None, 10),
    );
    h.append(original.clone()).unwrap();
    h.append(correction(
        "b",
        "a",
        "late correction",
        time(10, Some(20), 30),
    ))
    .unwrap();
    h.append(correction(
        "c",
        "b",
        "another correction",
        time(15, Some(17), 40),
    ))
    .unwrap();
    // Independent expected table: (domain instant, inclusive recording cutoff, version).
    for (domain, cutoff, expected) in [
        (4, 100, None),
        (5, 9, None),
        (5, 10, Some("a")),
        (10, 29, Some("a")),
        (10, 30, Some("b")),
        (9, 100, Some("a")),
        (19, 100, Some("b")),
        (20, 100, Some("a")),
        (15, 39, Some("b")),
        (15, 40, Some("c")),
        (16, 100, Some("c")),
        (17, 100, Some("b")),
        (1000, 100, Some("a")),
    ] {
        assert_eq!(
            selected(&h, domain, cutoff),
            expected,
            "domain={domain} cutoff={cutoff}"
        );
    }
    assert_eq!(h.version(&version("a")).unwrap(), &original);
    assert_eq!(
        h.history(&entity("e"))
            .unwrap()
            .records()
            .map(|r| r.id().as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
}

#[test]
fn equal_recording_times_follow_chain_not_id_sort_and_allow_future_validity() {
    let mut h = CanonicalHistory::new(context());
    h.append(initial(
        "e",
        "z",
        RecordKind::Requirement,
        "future",
        time(100, None, 1),
    ))
    .unwrap();
    h.append(correction("a", "z", "corrected future", time(100, None, 1)))
        .unwrap();
    assert_eq!(selected(&h, 100, 1), Some("a"));
    assert_eq!(selected(&h, 99, 1), None);
    assert_eq!(selected(&h, 100, 0), None);
}

#[test]
fn a_decision_correction_preserves_the_original_source_and_category() {
    let mut h = CanonicalHistory::new(context());
    let first = initial(
        "decision",
        "d1",
        RecordKind::Decision,
        "original choice",
        time(0, None, 1),
    );
    h.append(first.clone()).unwrap();
    let revised = CanonicalRecord::correction(
        entity("decision"),
        version("d2"),
        RecordKind::Decision,
        capture(),
        version("d1"),
        RecordContent::new(
            "revised choice",
            vec![
                SourceReference::operator_reported("another author", "explicit correction", "2")
                    .unwrap(),
            ],
        )
        .unwrap(),
        time(0, None, 2),
    );
    h.append(revised.clone()).unwrap();
    let q = TemporalQuery::new(capture(), at(0), at(2)).unwrap();
    assert_eq!(
        h.query(&entity("decision"), &q).unwrap().record(),
        Some(&revised)
    );
    assert_eq!(h.version(&version("d1")).unwrap(), &first);
    assert_ne!(first.content().sources(), revised.content().sources());
    assert_eq!(revised.predecessor(), Some(&version("d1")));
}

#[test]
fn duplicates_aliases_and_competing_initial_records_fail_atomically() {
    let mut h = CanonicalHistory::new(context());
    let a = initial(
        "e",
        "a",
        RecordKind::Requirement,
        "original",
        time(0, None, 1),
    );
    h.append(a.clone()).unwrap();
    assert_rejected(&mut h, a, TemporalError::DuplicateVersion);
    assert_rejected(
        &mut h,
        initial(
            "other",
            "a",
            RecordKind::Decision,
            "aliased identity",
            time(0, None, 1),
        ),
        TemporalError::DuplicateVersion,
    );
    assert_rejected(
        &mut h,
        initial(
            "e",
            "b",
            RecordKind::Requirement,
            "competing initial",
            time(0, None, 2),
        ),
        TemporalError::EntityExists,
    );
    // Reusing an ancestor ID to form a cycle is also impossible.
    assert_rejected(
        &mut h,
        correction("a", "a", "cycle", time(0, None, 2)),
        TemporalError::DuplicateVersion,
    );
}

#[test]
fn predecessor_entity_category_order_and_scope_errors_preserve_state() {
    let mut h = CanonicalHistory::new(context());
    h.append(initial(
        "e",
        "a",
        RecordKind::Requirement,
        "original",
        time(0, None, 10),
    ))
    .unwrap();
    h.append(initial(
        "other",
        "other-a",
        RecordKind::Decision,
        "another entity",
        time(0, None, 1),
    ))
    .unwrap();
    assert_rejected(
        &mut h,
        correction("b", "missing", "missing target", time(0, None, 11)),
        TemporalError::MissingPredecessor,
    );
    assert_rejected(
        &mut h,
        correction("b", "other-a", "wrong entity", time(0, None, 11)),
        TemporalError::EntityMismatch,
    );
    let wrong_category = CanonicalRecord::correction(
        entity("e"),
        version("b"),
        RecordKind::Decision,
        capture(),
        version("a"),
        content("wrong category"),
        time(0, None, 11),
    );
    assert_rejected(&mut h, wrong_category, TemporalError::CategoryMismatch);
    assert_rejected(
        &mut h,
        correction("b", "a", "backwards", time(0, None, 9)),
        TemporalError::RecordingOrder,
    );
    let wrong_scope = CanonicalRecord::correction(
        entity("e"),
        version("b"),
        RecordKind::Requirement,
        ObservationId::new("cap", 2).unwrap(),
        version("a"),
        content("another capture"),
        time(0, None, 11),
    );
    assert_rejected(&mut h, wrong_scope, TemporalError::ScopeMismatch);
    h.append(correction("b", "a", "valid", time(0, None, 11)))
        .unwrap();
    assert_rejected(
        &mut h,
        correction("c", "a", "fork", time(0, None, 12)),
        TemporalError::StalePredecessor,
    );
}

#[test]
fn queries_distinguish_missing_history_wrong_scope_and_no_match_without_mutation() {
    let mut h = CanonicalHistory::new(context());
    h.append(initial(
        "e",
        "a",
        RecordKind::Requirement,
        "future",
        time(20, None, 10),
    ))
    .unwrap();
    let before = h.clone();
    assert_eq!(selected(&h, 19, 10), None);
    assert_eq!(selected(&h, 20, 9), None);
    let q = TemporalQuery::new(capture(), at(20), at(10)).unwrap();
    assert_eq!(
        h.query(&entity("missing"), &q),
        Err(TemporalError::MissingEntity)
    );
    assert_eq!(
        h.history(&entity("missing")).unwrap_err(),
        TemporalError::MissingEntity
    );
    assert_eq!(
        h.version(&version("missing")),
        Err(TemporalError::MissingVersion)
    );
    let wrong = TemporalQuery::new(
        ObservationId::new("another-producer", 1).unwrap(),
        at(20),
        at(10),
    )
    .unwrap();
    assert_eq!(
        h.query(&entity("e"), &wrong),
        Err(TemporalError::ScopeMismatch)
    );
    assert_eq!(h.query(&entity("e"), &q).unwrap().context(), h.context());
    assert_eq!(h.query(&entity("e"), &q), h.query(&entity("e"), &q));
    assert_eq!(h, before);
}

#[test]
fn replaying_accepted_inputs_reconstructs_identical_order_and_views() {
    let entries = [
        initial(
            "e",
            "a",
            RecordKind::Requirement,
            "first",
            time(0, None, 10),
        ),
        initial(
            "other",
            "d",
            RecordKind::Decision,
            "second",
            time(0, None, 1),
        ),
        correction("b", "a", "third", time(1, Some(9), 10)),
    ];
    let mut first = CanonicalHistory::new(context());
    let mut second = CanonicalHistory::new(context());
    for entry in entries {
        first.append(entry.clone()).unwrap();
        second.append(entry).unwrap();
    }
    assert_eq!(first, second);
    for d in [0, 1, 8, 9] {
        for cutoff in [0, 9, 10, 11] {
            assert_eq!(selected(&first, d, cutoff), selected(&second, d, cutoff));
        }
    }
}

#[test]
fn time_boundaries_reject_empty_inverted_and_pre_epoch_inputs() {
    let before_epoch = UNIX_EPOCH - Duration::from_nanos(1);
    assert_eq!(
        DomainInterval::new(at(1), Some(at(1))),
        Err(TemporalError::InvalidInterval)
    );
    assert_eq!(
        DomainInterval::new(at(2), Some(at(1))),
        Err(TemporalError::InvalidInterval)
    );
    assert_eq!(
        DomainInterval::new(before_epoch, None),
        Err(TemporalError::InvalidTime)
    );
    assert_eq!(
        DomainInterval::new(at(0), Some(before_epoch)),
        Err(TemporalError::InvalidTime)
    );
    let interval = DomainInterval::new(at(0), Some(at(1))).unwrap();
    assert_eq!(interval.from(), UNIX_EPOCH);
    assert_eq!(interval.until(), Some(at(1)));
    assert!(interval.contains(UNIX_EPOCH));
    assert!(!interval.contains(at(1)));
    assert_eq!(
        RecordTime::new(interval, before_epoch),
        Err(TemporalError::InvalidTime)
    );
    assert_eq!(
        TemporalQuery::new(capture(), before_epoch, at(0)),
        Err(TemporalError::InvalidTime)
    );
    assert_eq!(
        TemporalQuery::new(capture(), at(0), before_epoch),
        Err(TemporalError::InvalidTime)
    );
}

#[test]
fn identifiers_and_text_enforce_byte_character_and_source_boundaries() {
    for bad in ["", " ", ".bad", "é", "a b", "a\n", "a/../b"] {
        assert!(RecordId::new(bad).is_err());
        assert!(RecordVersionId::new(bad).is_err());
    }
    assert!(RecordId::new(&"a".repeat(128)).is_ok());
    assert!(RecordVersionId::new(&"a".repeat(128)).is_ok());
    assert!(RecordId::new(&"a".repeat(129)).is_err());
    assert!(RecordVersionId::new(&"a".repeat(129)).is_err());
    assert_eq!(RecordId::new("a.B_1-2").unwrap().as_str(), "a.B_1-2");
    let source = SourceReference::operator_reported("author", "source", "version").unwrap();
    assert!(RecordContent::new(&"é".repeat(2048), vec![source.clone()]).is_ok());
    assert!(RecordContent::new(&format!("{}x", "é".repeat(2048)), vec![source.clone()]).is_err());
    for bad in ["", " \n", "body\t", "body\r", "body\0"] {
        assert!(RecordContent::new(bad, vec![source.clone()]).is_err());
        assert!(SourceReference::operator_reported(bad, "source", "1").is_err());
        assert!(SourceReference::operator_reported("a", bad, "1").is_err());
        assert!(SourceReference::operator_reported("a", "source", bad).is_err());
        assert!(SourceReference::observation_reference(capture(), bad, "1").is_err());
        assert!(SourceReference::observation_reference(capture(), "source", bad).is_err());
    }
    let max = "x".repeat(4096);
    let too_long = "x".repeat(4097);
    assert!(SourceReference::operator_reported(&max, &max, &max).is_ok());
    assert!(SourceReference::observation_reference(capture(), &max, &max).is_ok());
    assert!(SourceReference::operator_reported(&too_long, "s", "1").is_err());
    assert!(SourceReference::operator_reported("a", &too_long, "1").is_err());
    assert!(SourceReference::operator_reported("a", "s", &too_long).is_err());
    assert!(SourceReference::observation_reference(capture(), &too_long, "1").is_err());
    assert!(SourceReference::observation_reference(capture(), "s", &too_long).is_err());
    assert_eq!(
        RecordContent::new("body", vec![]),
        Err(TemporalError::MissingSource)
    );
    assert!(RecordContent::new("Unicode é\nretained", vec![source.clone(); 32]).is_ok());
    assert_eq!(
        RecordContent::new("body", vec![source; 33]),
        Err(TemporalError::SourceLimit)
    );
    let err = RecordId::new("").unwrap_err();
    assert_eq!(
        err,
        TemporalError::Contract(ContractError::Blank("record_id"))
    );
    assert!(err.source().is_some());
}

#[test]
fn retained_version_bound_counts_superseded_entries_and_never_evicts() {
    let mut h = CanonicalHistory::new(context());
    h.append(initial(
        "e",
        "v0",
        RecordKind::Requirement,
        "a",
        time(0, None, 0),
    ))
    .unwrap();
    for n in 1..1024 {
        h.append(correction(
            &format!("v{n}"),
            &format!("v{}", n - 1),
            "b",
            time(0, None, 0),
        ))
        .unwrap();
    }
    assert_eq!(h.records().len(), MAX_RECORD_VERSIONS);
    assert_eq!(h.version(&version("v0")).unwrap().content().body(), "a");
    assert_rejected(
        &mut h,
        correction("v1024", "v1023", "c", time(0, None, 0)),
        TemporalError::VersionLimit,
    );
    assert_eq!(selected(&h, 0, 0), Some("v1023"));
}

#[test]
fn byte_budget_counts_ids_references_and_superseded_payloads_at_exact_limit() {
    let mut h = CanonicalHistory::new(context());
    h.append(initial(
        "e",
        "v0",
        RecordKind::Requirement,
        "body",
        time(0, None, 0),
    ))
    .unwrap();
    // e(1), v0(2), cap(3)+u64(8), body(4), o/s/1(3) = 21 bytes.
    assert_eq!(h.retained_bytes(), 21);
    let mut expected = 21;
    for n in 1..1024 {
        let id = format!("v{n}");
        let previous = format!("v{}", n - 1);
        let overhead = 1 + id.len() + 3 + 8 + previous.len() + 3;
        let remaining_body = MAX_RETAINED_RECORD_BYTES - expected - overhead;
        if remaining_body < 4096 {
            assert!(remaining_body > 0);
            let mut exceeds = h.clone();
            assert_rejected(
                &mut exceeds,
                correction(
                    &id,
                    &previous,
                    &"x".repeat(remaining_body + 1),
                    time(0, None, 0),
                ),
                TemporalError::ByteLimit,
            );
            h.append(correction(
                &id,
                &previous,
                &"x".repeat(remaining_body),
                time(0, None, 0),
            ))
            .unwrap();
            assert_eq!(h.retained_bytes(), 4 * 1024 * 1024);
            assert!(h.records().len() < MAX_RECORD_VERSIONS);
            assert_eq!(h.version(&version("v0")).unwrap().content().body(), "body");
            return;
        }
        h.append(correction(
            &id,
            &previous,
            &"x".repeat(4096),
            time(0, None, 0),
        ))
        .unwrap();
        expected += overhead + 4096;
        assert_eq!(h.retained_bytes(), expected);
    }
    panic!("fixture must reach byte budget before version budget");
}

#[test]
fn source_reference_bytes_include_operator_and_observation_origins() {
    let mut h = CanonicalHistory::new(context());
    let sources = vec![
        SourceReference::operator_reported("author", "src", "v2").unwrap(),
        SourceReference::observation_reference(
            ObservationId::new("tool", 1).unwrap(),
            "query",
            "v3",
        )
        .unwrap(),
    ];
    h.append(CanonicalRecord::initial(
        entity("e"),
        version("a"),
        RecordKind::Decision,
        capture(),
        RecordContent::new("body", sources).unwrap(),
        time(0, None, 0),
    ))
    .unwrap();
    // e/a=2, cap+sequence=11, body=4, author/src/v2=11, tool+sequence/query/v3=19.
    assert_eq!(h.retained_bytes(), 47);
}

#[test]
fn incomplete_or_indeterminate_context_remains_information_without_freshness_promotion() {
    let base = context();
    let state = RepositoryState::new(
        None,
        Head::Unknown,
        vec![],
        ObservationCoverage::Incomplete,
        ObservationConsistency::Indeterminate,
    )
    .unwrap();
    let uncertain = CandidateObservation::new(capture(), base.contract(), state, None);
    let mut h = CanonicalHistory::new(uncertain.clone());
    h.append(initial(
        "e",
        "a",
        RecordKind::Decision,
        "reported decision",
        time(0, None, 0),
    ))
    .unwrap();
    let view = h
        .query(
            &entity("e"),
            &TemporalQuery::new(capture(), at(0), at(0)).unwrap(),
        )
        .unwrap();
    assert_eq!(view.context(), &uncertain);
    assert_eq!(
        view.context().state().coverage(),
        ObservationCoverage::Incomplete
    );
    assert_eq!(
        view.context().state().consistency(),
        ObservationConsistency::Indeterminate
    );
    assert!(view.record().is_some()); // A historical assertion only, not a candidate binding or assessment.
}

#[test]
fn default_debug_and_errors_omit_caller_payloads_but_explicit_accessors_retain_them() {
    let secret = "PRIVATE_SENTINEL";
    let c = context();
    let sensitive = CandidateObservation::new(
        ObservationId::new(secret, 1).unwrap(),
        c.contract(),
        c.state().clone(),
        None,
    );
    let source = SourceReference::operator_reported(secret, secret, secret).unwrap();
    let observation_source =
        SourceReference::observation_reference(sensitive.id().clone(), secret, secret).unwrap();
    let value =
        RecordContent::new(secret, vec![source.clone(), observation_source.clone()]).unwrap();
    let record = CanonicalRecord::initial(
        entity(secret),
        version(secret),
        RecordKind::Decision,
        sensitive.id().clone(),
        value.clone(),
        time(0, None, 0),
    );
    let mut h = CanonicalHistory::new(sensitive);
    h.append(record.clone()).unwrap();
    let q = TemporalQuery::new(h.context().id().clone(), at(0), at(0)).unwrap();
    let view = h.query(&entity(secret), &q).unwrap();
    let err = RecordId::new(&format!("{secret}\t")).unwrap_err();
    let debug = format!(
        "{h:?} {record:?} {value:?} {source:?} {observation_source:?} {:?} {:?} {q:?} {view:?} {:?} {err:?} {err}",
        source.origin(),
        record.id(),
        h.history(&entity(secret)).unwrap()
    );
    assert!(!debug.contains(secret));
    assert_eq!(view.record().unwrap().content().body(), secret);
    assert_eq!(
        view.record().unwrap().content().sources()[0].source(),
        secret
    );
}
