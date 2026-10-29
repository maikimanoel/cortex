//! PP04 / CAP-MEM-001 / bounded 003-A interval selection and immutable views.

use cortex_core::{
    CandidateId, TaskContract, TaskId,
    binding::*,
    temporal::{timeline::*, *},
};
use std::{
    error::Error,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH
        .checked_add(Duration::from_secs(seconds))
        .unwrap()
}

fn nano(seconds: u64, nanos: u32) -> SystemTime {
    UNIX_EPOCH
        .checked_add(Duration::new(seconds, nanos))
        .unwrap()
}

fn capture() -> ObservationId {
    ObservationId::new("timeline.capture", 1).unwrap()
}

fn context() -> CandidateObservation {
    // Synthetic values only: no files are created and no Git object is asserted.
    let root = std::env::temp_dir().join("cortex-timeline-synthetic");
    let repository =
        RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git")).unwrap();
    let commit = CandidateId::from_git_commit(&"a".repeat(40)).unwrap();
    let contract = CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new("timeline.fixture").unwrap(),
            "Retain canonical perspectives",
            "Partition a finite interval using existing point-query semantics",
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

fn entity(name: &str) -> RecordId {
    RecordId::new(name).unwrap()
}

fn version(id: &str) -> RecordVersionId {
    RecordVersionId::new(id).unwrap()
}

fn valid(from: u64, until: Option<u64>, recorded: u64) -> RecordTime {
    times(at(from), until.map(at), at(recorded))
}

fn times(from: SystemTime, until: Option<SystemTime>, recorded: SystemTime) -> RecordTime {
    RecordTime::new(DomainInterval::new(from, until).unwrap(), recorded).unwrap()
}

fn append(
    history: &mut CanonicalHistory,
    name: &str,
    id: &str,
    previous: Option<&str>,
    kind: RecordKind,
    time: RecordTime,
) {
    let content = RecordContent::new(
        "private timeline body",
        vec![
            SourceReference::operator_reported("private author", "private source", "v1").unwrap(),
            SourceReference::observation_reference(
                ObservationId::new("source", 7).unwrap(),
                "private observation",
                "v2",
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let record = match previous {
        Some(previous) => CanonicalRecord::correction(
            entity(name),
            version(id),
            kind,
            capture(),
            version(previous),
            content,
            time,
        ),
        None => CanonicalRecord::initial(entity(name), version(id), kind, capture(), content, time),
    };
    history.append(record).unwrap();
}

type Expected<'a> = (SystemTime, SystemTime, Option<&'a str>);

fn assert_timeline(
    history: &CanonicalHistory,
    name: &str,
    from: SystemTime,
    until: SystemTime,
    cutoff: SystemTime,
    expected: &[Expected<'_>],
) {
    let before = history.clone();
    let request = || TimelineQuery::new(capture(), from, until, cutoff).unwrap();
    let timeline = query_timeline(history, &entity(name), request()).unwrap();
    assert_eq!(
        timeline,
        query_timeline(history, &entity(name), request()).unwrap()
    );
    assert_eq!(timeline.entity(), &entity(name));
    assert!(std::ptr::eq(timeline.context(), history.context()));
    assert_eq!(
        timeline.interval(),
        DomainInterval::new(from, Some(until)).unwrap()
    );
    assert_eq!(timeline.recorded_by(), cutoff);
    // Literal expected tuples are the primary oracle; point queries below are
    // additional cross-checks, not a segmentation implementation in the test.
    let actual: Vec<_> = timeline
        .segments()
        .iter()
        .map(|segment| {
            (
                segment.from(),
                segment.until(),
                segment.record().map(|record| record.id().as_str()),
            )
        })
        .collect();
    assert_eq!(actual, expected);
    assert!(!timeline.segments().is_empty());
    assert_eq!(timeline.segments().first().unwrap().from(), from);
    assert_eq!(timeline.segments().last().unwrap().until(), until);
    for segment in timeline.segments() {
        assert!(segment.from() < segment.until());
        if let Some(record) = segment.record() {
            assert!(std::ptr::eq(record, history.version(record.id()).unwrap()));
        }
        let width = segment.until().duration_since(segment.from()).unwrap();
        let midpoint = segment.from().checked_add(width / 2).unwrap();
        let last = segment
            .until()
            .checked_sub(Duration::from_nanos(1))
            .unwrap();
        for position in [segment.from(), midpoint, last] {
            let point = TemporalQuery::new(capture(), position, cutoff).unwrap();
            assert_eq!(
                segment.record(),
                history.query(&entity(name), &point).unwrap().record(),
                "point={position:?} cutoff={cutoff:?}"
            );
        }
    }
    for pair in timeline.segments().windows(2) {
        assert_eq!(pair[0].until(), pair[1].from());
        assert_ne!(
            pair[0].record().map(CanonicalRecord::id),
            pair[1].record().map(CanonicalRecord::id)
        );
    }
    // Includes all bodies, source order/bytes, context, append order and accounting.
    assert_eq!(*history, before);
}

fn assert_seconds(
    history: &CanonicalHistory,
    name: &str,
    window: (u64, u64),
    cutoff: u64,
    expected: &[(u64, u64, Option<&str>)],
) {
    let expected: Vec<_> = expected
        .iter()
        .map(|&(a, b, v)| (at(a), at(b), v))
        .collect();
    assert_timeline(
        history,
        name,
        at(window.0),
        at(window.1),
        at(cutoff),
        &expected,
    );
}

#[test]
fn initial_requirements_and_decisions_cover_exactly_the_requested_window() {
    let mut h = CanonicalHistory::new(context());
    append(
        &mut h,
        "requirement",
        "r",
        None,
        RecordKind::Requirement,
        valid(5, None, 10),
    );
    append(
        &mut h,
        "decision",
        "d",
        None,
        RecordKind::Decision,
        valid(10, Some(20), 1),
    );
    assert_seconds(
        &h,
        "requirement",
        (0, 30),
        10,
        &[(0, 5, None), (5, 30, Some("r"))],
    );
    assert_seconds(
        &h,
        "decision",
        (0, 30),
        10,
        &[(0, 10, None), (10, 20, Some("d")), (20, 30, None)],
    );
    assert_seconds(&h, "requirement", (0, 30), 9, &[(0, 30, None)]);
}

fn corrections() -> CanonicalHistory {
    let mut h = CanonicalHistory::new(context());
    append(
        &mut h,
        "e",
        "z-base",
        None,
        RecordKind::Requirement,
        valid(5, Some(40), 10),
    );
    append(
        &mut h,
        "e",
        "m-narrow",
        Some("z-base"),
        RecordKind::Requirement,
        valid(10, Some(20), 30),
    );
    append(
        &mut h,
        "e",
        "a-overlap",
        Some("m-narrow"),
        RecordKind::Requirement,
        valid(15, Some(25), 40),
    );
    append(
        &mut h,
        "e",
        "0-patch",
        Some("a-overlap"),
        RecordKind::Requirement,
        valid(17, Some(18), 40),
    );
    h
}

#[test]
fn narrow_overlapping_corrections_fall_back_at_their_own_half_open_ends() {
    let h = corrections();
    for cutoff in [40, 41, 100] {
        assert_seconds(
            &h,
            "e",
            (0, 45),
            cutoff,
            &[
                (0, 5, None),
                (5, 10, Some("z-base")),
                (10, 15, Some("m-narrow")),
                (15, 17, Some("a-overlap")),
                (17, 18, Some("0-patch")),
                (18, 25, Some("a-overlap")),
                (25, 40, Some("z-base")),
                (40, 45, None),
            ],
        );
    }
}

#[test]
fn cutoffs_before_at_and_between_recordings_retain_earlier_perspectives() {
    let h = corrections();
    assert_seconds(&h, "e", (0, 45), 9, &[(0, 45, None)]);
    for cutoff in [10, 29] {
        assert_seconds(
            &h,
            "e",
            (0, 45),
            cutoff,
            &[(0, 5, None), (5, 40, Some("z-base")), (40, 45, None)],
        );
    }
    for cutoff in [30, 39] {
        assert_seconds(
            &h,
            "e",
            (0, 45),
            cutoff,
            &[
                (0, 5, None),
                (5, 10, Some("z-base")),
                (10, 20, Some("m-narrow")),
                (20, 40, Some("z-base")),
                (40, 45, None),
            ],
        );
    }
}

#[test]
fn clipped_windows_and_exact_boundaries_do_not_leak_outside_the_request() {
    let h = corrections();
    assert_seconds(
        &h,
        "e",
        (12, 26),
        40,
        &[
            (12, 15, Some("m-narrow")),
            (15, 17, Some("a-overlap")),
            (17, 18, Some("0-patch")),
            (18, 25, Some("a-overlap")),
            (25, 26, Some("z-base")),
        ],
    );
    assert_seconds(&h, "e", (10, 15), 40, &[(10, 15, Some("m-narrow"))]);
    assert_seconds(&h, "e", (18, 25), 40, &[(18, 25, Some("a-overlap"))]);
    assert_seconds(&h, "e", (25, 40), 40, &[(25, 40, Some("z-base"))]);
    assert_seconds(&h, "e", (40, 45), 40, &[(40, 45, None)]);
}

#[test]
fn gaps_and_finite_original_validity_remain_explicit() {
    let mut h = CanonicalHistory::new(context());
    append(
        &mut h,
        "e",
        "v1",
        None,
        RecordKind::Requirement,
        valid(5, Some(10), 10),
    );
    append(
        &mut h,
        "e",
        "v2",
        Some("v1"),
        RecordKind::Requirement,
        valid(15, Some(20), 20),
    );
    append(
        &mut h,
        "e",
        "v3",
        Some("v2"),
        RecordKind::Requirement,
        valid(25, Some(30), 30),
    );
    assert_seconds(
        &h,
        "e",
        (0, 35),
        30,
        &[
            (0, 5, None),
            (5, 10, Some("v1")),
            (10, 15, None),
            (15, 20, Some("v2")),
            (20, 25, None),
            (25, 30, Some("v3")),
            (30, 35, None),
        ],
    );
    assert_seconds(&h, "e", (10, 15), 30, &[(10, 15, None)]);
}

#[test]
fn future_decisions_and_equal_instants_follow_per_entity_chain_not_lexical_ids() {
    let mut h = CanonicalHistory::new(context());
    append(
        &mut h,
        "e",
        "z",
        None,
        RecordKind::Decision,
        valid(100, Some(110), 5),
    );
    // An independent entity has no shared recording-order requirement.
    append(
        &mut h,
        "other",
        "unrelated",
        None,
        RecordKind::Requirement,
        valid(0, None, 999),
    );
    append(
        &mut h,
        "e",
        "a",
        Some("z"),
        RecordKind::Decision,
        valid(100, Some(110), 5),
    );
    append(
        &mut h,
        "e",
        "m",
        Some("a"),
        RecordKind::Decision,
        valid(105, Some(107), 5),
    );
    assert_seconds(&h, "e", (95, 115), 4, &[(95, 115, None)]);
    for cutoff in [5, 6] {
        assert_seconds(
            &h,
            "e",
            (95, 115),
            cutoff,
            &[
                (95, 100, None),
                (100, 105, Some("a")),
                (105, 107, Some("m")),
                (107, 110, Some("a")),
                (110, 115, None),
            ],
        );
    }
}

#[test]
fn dominated_endpoints_coalesce_and_distinct_versions_with_equal_bodies_do_not() {
    let mut h = CanonicalHistory::new(context());
    append(
        &mut h,
        "e",
        "base",
        None,
        RecordKind::Requirement,
        valid(0, Some(40), 1),
    );
    append(
        &mut h,
        "e",
        "narrow",
        Some("base"),
        RecordKind::Requirement,
        valid(5, Some(10), 2),
    );
    append(
        &mut h,
        "e",
        "final",
        Some("narrow"),
        RecordKind::Requirement,
        valid(0, Some(40), 3),
    );
    assert_seconds(&h, "e", (0, 40), 3, &[(0, 40, Some("final"))]);
    assert_seconds(&h, "e", (0, 40), 0, &[(0, 40, None)]);
    assert_seconds(
        &h,
        "e",
        (0, 40),
        2,
        &[
            (0, 5, Some("base")),
            (5, 10, Some("narrow")),
            (10, 40, Some("base")),
        ],
    );
    append(
        &mut h,
        "adjacent",
        "left",
        None,
        RecordKind::Decision,
        valid(0, Some(10), 1),
    );
    append(
        &mut h,
        "adjacent",
        "right",
        Some("left"),
        RecordKind::Decision,
        valid(10, Some(20), 1),
    );
    assert_eq!(h.records()[3].content(), h.records()[4].content());
    assert_seconds(
        &h,
        "adjacent",
        (0, 20),
        1,
        &[(0, 10, Some("left")), (10, 20, Some("right"))],
    );
}

#[test]
fn nanosecond_intervals_and_cutoffs_preserve_epoch_and_second_boundaries() {
    let mut h = CanonicalHistory::new(context());
    let start = nano(0, 999_999_998);
    let edge = nano(0, 999_999_999);
    let second = at(1);
    let next = nano(1, 1);
    let end = nano(1, 2);
    append(
        &mut h,
        "e",
        "base",
        None,
        RecordKind::Requirement,
        times(UNIX_EPOCH, None, UNIX_EPOCH),
    );
    append(
        &mut h,
        "e",
        "nano",
        Some("base"),
        RecordKind::Requirement,
        times(edge, Some(second), edge),
    );
    append(
        &mut h,
        "e",
        "next",
        Some("nano"),
        RecordKind::Requirement,
        times(second, Some(next), second),
    );
    assert_timeline(
        &h,
        "e",
        UNIX_EPOCH,
        nano(0, 1),
        UNIX_EPOCH,
        &[(UNIX_EPOCH, nano(0, 1), Some("base"))],
    );
    assert_timeline(&h, "e", start, end, start, &[(start, end, Some("base"))]);
    assert_timeline(
        &h,
        "e",
        start,
        end,
        edge,
        &[
            (start, edge, Some("base")),
            (edge, second, Some("nano")),
            (second, end, Some("base")),
        ],
    );
    assert_timeline(
        &h,
        "e",
        start,
        end,
        second,
        &[
            (start, edge, Some("base")),
            (edge, second, Some("nano")),
            (second, next, Some("next")),
            (next, end, Some("base")),
        ],
    );
}

#[test]
fn far_future_seconds_and_last_nanosecond_are_not_truncated() {
    let mut h = CanonicalHistory::new(context());
    // Beyond exact floating-point integer seconds, still representable on Linux.
    let seconds = (1_u64 << 53) + 1;
    let start = nano(seconds, 999_999_998);
    let edge = nano(seconds, 999_999_999);
    let end = nano(seconds + 1, 0);
    append(
        &mut h,
        "e",
        "far",
        None,
        RecordKind::Decision,
        times(start, Some(edge), at(0)),
    );
    append(
        &mut h,
        "e",
        "last",
        Some("far"),
        RecordKind::Decision,
        times(edge, Some(end), at(0)),
    );
    assert_timeline(
        &h,
        "e",
        start,
        end,
        at(0),
        &[(start, edge, Some("far")), (edge, end, Some("last"))],
    );
}

#[test]
fn upper_linux_time_boundary_needs_no_successor_or_lossy_conversion() {
    let mut h = CanonicalHistory::new(context());
    let start = nano(i64::MAX as u64, 999_999_997);
    let edge = nano(i64::MAX as u64, 999_999_998);
    let end = nano(i64::MAX as u64, 999_999_999);
    append(
        &mut h,
        "e",
        "base",
        None,
        RecordKind::Decision,
        times(start, Some(end), at(0)),
    );
    append(
        &mut h,
        "e",
        "open",
        Some("base"),
        RecordKind::Decision,
        times(edge, None, end),
    );
    assert_timeline(&h, "e", start, end, edge, &[(start, end, Some("base"))]);
    assert_timeline(
        &h,
        "e",
        start,
        end,
        end,
        &[(start, edge, Some("base")), (edge, end, Some("open"))],
    );
}

#[test]
fn invalid_empty_inverted_and_pre_epoch_requests_keep_typed_causes() {
    let h = corrections();
    let before = h.clone();
    let pre_epoch = UNIX_EPOCH.checked_sub(Duration::from_nanos(1)).unwrap();
    for (from, until, cutoff, cause) in [
        (at(1), at(1), at(2), TemporalError::InvalidInterval),
        (at(2), at(1), at(2), TemporalError::InvalidInterval),
        (pre_epoch, at(1), at(2), TemporalError::InvalidTime),
        (at(0), pre_epoch, at(2), TemporalError::InvalidTime),
        (at(0), at(1), pre_epoch, TemporalError::InvalidTime),
    ] {
        let error = TimelineQuery::new(capture(), from, until, cutoff).unwrap_err();
        assert_eq!(error, TimelineError::Temporal(cause.clone()));
        assert_eq!(
            error.source().unwrap().downcast_ref::<TemporalError>(),
            Some(&cause)
        );
    }
    assert_eq!(h, before);
}

#[test]
fn context_and_unknown_entity_errors_remain_distinct_and_do_not_mutate_history() {
    let h = corrections();
    let before = h.clone();
    let other_capture = || ObservationId::new("timeline.capture", 2).unwrap();
    for (name, context, expected) in [
        ("missing", capture(), TemporalError::MissingEntity),
        ("e", other_capture(), TemporalError::ScopeMismatch),
        ("missing", other_capture(), TemporalError::ScopeMismatch),
    ] {
        let q = TimelineQuery::new(context, at(0), at(1), at(100)).unwrap();
        let error = query_timeline(&h, &entity(name), q).unwrap_err();
        assert_eq!(error, TimelineError::Temporal(expected.clone()));
        assert_eq!(
            error.source().unwrap().downcast_ref::<TemporalError>(),
            Some(&expected)
        );
        assert_eq!(h, before);
    }
    let empty = CanonicalHistory::new(context());
    let q = TimelineQuery::new(capture(), at(0), at(1), at(1)).unwrap();
    assert_eq!(
        query_timeline(&empty, &entity("e"), q),
        Err(TimelineError::Temporal(TemporalError::MissingEntity))
    );
    assert!(empty.records().is_empty());
    assert_eq!(empty.retained_bytes(), 0);
}

#[test]
fn full_retained_version_bound_can_produce_the_exact_maximum_partition() {
    let mut h = CanonicalHistory::new(context());
    for index in 0..MAX_RECORD_VERSIONS {
        let id = format!("v{index}");
        let previous = index.checked_sub(1).map(|n| format!("v{n}"));
        let from = 2 * index as u64 + 1;
        append(
            &mut h,
            "e",
            &id,
            previous.as_deref(),
            RecordKind::Requirement,
            valid(from, Some(from + 1), 0),
        );
    }
    let before = h.clone();
    let end = 2 * MAX_RECORD_VERSIONS as u64 + 1;
    let q = TimelineQuery::new(capture(), at(0), at(end), at(0)).unwrap();
    let timeline = query_timeline(&h, &entity("e"), q).unwrap();
    assert_eq!(timeline.segments().len(), 2 * MAX_RECORD_VERSIONS + 1);
    // Independent fixture definition: alternating absence and one distinct version
    // in every unit interval. No endpoint-discovery algorithm supplies this oracle.
    for (index, segment) in timeline.segments().iter().enumerate() {
        assert_eq!(segment.from(), at(index as u64));
        assert_eq!(segment.until(), at(index as u64 + 1));
        if index % 2 == 0 {
            assert!(segment.record().is_none());
        } else {
            assert_eq!(
                segment.record().unwrap().id().as_str(),
                format!("v{}", index / 2)
            );
        }
        let point = TemporalQuery::new(capture(), segment.from(), at(0)).unwrap();
        assert_eq!(
            segment.record(),
            h.query(&entity("e"), &point).unwrap().record()
        );
    }
    assert_eq!(h, before);
}

#[test]
fn default_diagnostics_omit_private_record_and_context_payloads() {
    let h = corrections();
    let q = TimelineQuery::new(capture(), at(0), at(45), at(40)).unwrap();
    let query_debug = format!("{q:?}");
    let timeline = query_timeline(&h, &entity("e"), q).unwrap();
    let debug = format!("{query_debug} {timeline:?} {:?}", timeline.segments());
    for private in [
        "timeline.capture",
        "private timeline body",
        "private author",
        "private source",
        "private observation",
        "z-base",
        "a-overlap",
    ] {
        assert!(!debug.contains(private));
    }
    let allocation = TimelineError::AllocationFailed;
    assert!(allocation.source().is_none());
    assert_eq!(
        allocation.to_string(),
        "timeline could not reserve space for its view"
    );
}
