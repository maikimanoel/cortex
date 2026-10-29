//! PP03 / CAP-KNW-001 / 003-B literal delta oracles over synthetic Core reports.
//! These fixtures assert typed structure, not real parsing, Git or authenticated
//! captures. No refresh API or comparator-derived expected-value oracle is used.

use cortex_core::{
    CandidateId, TaskContract, TaskId,
    binding::*,
    knowledge::{delta::*, source_set::*, *},
};
use std::{path::PathBuf, time::UNIX_EPOCH};

struct Snapshot {
    repository: RepositoryIdentity,
    scope: &'static str,
    first: u64,
    revision: u64,
    commit: char,
    capture_profile: &'static str,
    mechanism: &'static str,
    profile: &'static str,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            repository: repository("/synthetic-cortex-delta", ".git", ".git"),
            scope: "delta-fixture",
            first: 1,
            revision: 1,
            commit: 'a',
            capture_profile: "synthetic-capture-v1",
            mechanism: "synthetic-set",
            profile: "opaque-v1;source-profile=opaque-parser-v1;rules=lexical-v1",
        }
    }
}
fn repository(root: &str, git: &str, common: &str) -> RepositoryIdentity {
    let root = PathBuf::from(root);
    RepositoryIdentity::new(root.clone(), root.join(git), root.join(common)).unwrap()
}
struct Source {
    path: Vec<u8>,
    bytes: Vec<u8>,
    nodes: Vec<KnowledgeNode>,
    references: Vec<ExtractedReference>,
    mechanism: &'static str,
    profile: &'static str,
}
fn source(path: &[u8], locators: &[&str], references: Vec<ExtractedReference>) -> Source {
    Source {
        path: path.to_vec(),
        bytes: vec![b' '; 32],
        nodes: locators.iter().map(|key| node(key, 0, 1)).collect(),
        references,
        mechanism: "synthetic-source",
        profile: "opaque-parser-v1;rules=local-pointer-v1",
    }
}
fn node(key: &str, start: usize, end: usize) -> KnowledgeNode {
    KnowledgeNode::new(key, SourceSpan::new(start, end).unwrap()).unwrap()
}
fn local(from: &str, spelling: &str, target: &str) -> ExtractedReference {
    ExtractedReference::new(from, spelling, ReferenceTarget::Local(target.to_owned())).unwrap()
}
fn unresolved(from: &str, spelling: &str, reason: UnresolvedReference) -> ExtractedReference {
    ExtractedReference::new(from, spelling, ReferenceTarget::Unresolved(reason)).unwrap()
}
fn outside(from: &str, spelling: &str) -> ExtractedReference {
    unresolved(from, spelling, UnresolvedReference::OutsideCoverage)
}
impl Snapshot {
    fn id(&self, n: u64) -> ObservationId {
        ObservationId::new(self.scope, n).unwrap()
    }
    fn record(&self, n: u64, mechanism: &str, profile: &str) -> ExtractionRecord {
        ExtractionRecord::new(
            self.id(n),
            mechanism,
            profile,
            UNIX_EPOCH + std::time::Duration::from_secs(self.first),
        )
        .unwrap()
    }
    fn candidate(&self) -> BoundCandidate {
        let commit = CandidateId::from_git_commit(&self.commit.to_string().repeat(40)).unwrap();
        let contract = CandidateBindingContract::new(
            TaskContract::new(
                TaskId::new("delta-task").unwrap(),
                "Compare selected source reports",
                "Retain structural differences and original context",
            )
            .unwrap(),
            ContractRevision::new(self.revision).unwrap(),
            CandidateRequirement::new(
                self.repository.clone(),
                commit.clone(),
                WorktreeRequirement::Clean,
            ),
        );
        let observed = |n| {
            CandidateObservation::new(
                self.id(n),
                &contract,
                RepositoryState::new(
                    Some(self.repository.clone()),
                    Head::Detached {
                        commit: commit.clone(),
                    },
                    vec![],
                    ObservationCoverage::GitStatus,
                    ObservationConsistency::TwoMatchingSamples,
                )
                .unwrap(),
                Some(ComparisonBasis::new(self.capture_profile, vec![self.commit as u8]).unwrap()),
            )
        };
        let (before, after) = (observed(self.first), observed(self.first + 1));
        match bind_candidate(&contract, Some(&before), Some(&after)).unwrap() {
            BindingDecision::Compatible(candidate) => *candidate,
            _ => panic!("synthetic candidate must bind"),
        }
    }
    fn build(&self, mut sources: Vec<Source>, reverse: bool) -> ReferenceSet {
        sources.sort_by(|a, b| a.path.cmp(&b.path));
        let count = sources.len() as u64;
        let mut indexes: Vec<_> = sources
            .into_iter()
            .enumerate()
            .map(|(i, source)| {
                ReferenceIndex::new(
                    self.candidate(),
                    SourcePath::new(&source.path).unwrap(),
                    source.bytes,
                    self.record(self.first + i as u64 + 2, source.mechanism, source.profile),
                    source.nodes,
                    source.references,
                )
                .unwrap()
            })
            .collect();
        if reverse {
            indexes.reverse();
        }
        ReferenceSet::new(
            indexes,
            self.record(self.first + count + 2, self.mechanism, self.profile),
        )
        .unwrap()
    }
}
fn snapshot(sources: Vec<Source>) -> ReferenceSet {
    Snapshot::default().build(sources, false)
}
fn dimensions(
    changes: ReferenceChanges,
) -> (Option<ResolutionChange>, bool, bool, bool, bool, bool) {
    (
        changes.resolution(),
        changes.spelling(),
        changes.span(),
        changes.original_target(),
        changes.origin(),
        changes.disposition(),
    )
}

#[test]
fn selected_sources_nodes_and_occurrences_have_literal_add_remove_results() {
    let before = snapshot(vec![
        source(b"a", &["", "/r"], vec![local("/r", "#", "")]),
        source(b"b", &["", "/old"], vec![local("/old", "#", "")]),
        source(b"d", &[""], vec![]),
    ]);
    let after = snapshot(vec![
        source(b"b", &["", "/new"], vec![local("/new", "#", "")]),
        source(b"c", &["", "/r"], vec![local("/r", "#", "")]),
        source(b"d", &[""], vec![]),
    ]);
    let delta = compare(&before, &after).unwrap();
    let (a, b, new_b, c) = (
        &before.sources()[0],
        &before.sources()[1],
        &after.sources()[0],
        &after.sources()[1],
    );
    assert_eq!(
        delta.source_changes().collect::<Vec<_>>(),
        vec![SourceChange::Removed(a), SourceChange::Added(c)]
    );
    assert_eq!(
        delta.node_changes().collect::<Vec<_>>(),
        vec![
            NodeChange::Removed {
                source: a,
                node: a.node("").unwrap()
            },
            NodeChange::Removed {
                source: a,
                node: a.node("/r").unwrap()
            },
            NodeChange::Added {
                source: new_b,
                node: new_b.node("/new").unwrap()
            },
            NodeChange::Removed {
                source: b,
                node: b.node("/old").unwrap()
            },
            NodeChange::Added {
                source: c,
                node: c.node("").unwrap()
            },
            NodeChange::Added {
                source: c,
                node: c.node("/r").unwrap()
            },
        ]
    );
    let old_refs: Vec<_> = before.references().collect();
    let new_refs: Vec<_> = after.references().collect();
    assert_eq!(
        delta.reference_changes().collect::<Vec<_>>(),
        vec![
            ReferenceChange::Removed(old_refs[0]),
            ReferenceChange::Added(new_refs[0]),
            ReferenceChange::Removed(old_refs[1]),
            ReferenceChange::Added(new_refs[1]),
        ]
    );
}

fn scalar_source(bytes: &[u8]) -> Source {
    let mut value = source(b"scalar.json", &[], vec![local("/$ref", "#", "")]);
    value.bytes = bytes.to_vec();
    value.nodes = vec![node("", 0, 18), node("/x", 5, 6), node("/$ref", 14, 17)];
    value
}

#[test]
fn scalar_bytes_change_even_when_every_node_and_reference_is_unchanged() {
    let before = snapshot(vec![scalar_source(br##"{"x":0,"$ref":"#"}"##)]);
    let after = snapshot(vec![scalar_source(br##"{"x":1,"$ref":"#"}"##)]);
    let delta = compare(&before, &after).unwrap();
    assert_eq!(delta.node_changes().count(), 0);
    assert_eq!(delta.reference_changes().count(), 0);
    assert_eq!(
        delta.source_changes().collect::<Vec<_>>(),
        vec![SourceChange::BytesChanged {
            before: &before.sources()[0],
            after: &after.sources()[0],
        }]
    );
    assert_eq!(
        delta.before().sources()[0].bytes(),
        br##"{"x":0,"$ref":"#"}"##
    );
    assert_eq!(
        delta.after().sources()[0].bytes(),
        br##"{"x":1,"$ref":"#"}"##
    );
}

#[test]
fn whitespace_changes_bytes_and_spans_without_retargeting() {
    let before = snapshot(vec![scalar_source(br##"{"x":0,"$ref":"#"}"##)]);
    let mut shifted = scalar_source(b" {\"x\":0,\"$ref\":\"#\"}");
    shifted.nodes = vec![node("", 1, 19), node("/x", 6, 7), node("/$ref", 15, 18)];
    let after = snapshot(vec![shifted]);
    let delta = compare(&before, &after).unwrap();
    assert!(matches!(
        delta.source_changes().next(),
        Some(SourceChange::BytesChanged { .. })
    ));
    let b = &before.sources()[0];
    let a = &after.sources()[0];
    assert_eq!(
        delta.node_changes().collect::<Vec<_>>(),
        ["", "/$ref", "/x"].map(|key| NodeChange::SpanChanged {
            before_source: b,
            before: b.node(key).unwrap(),
            after_source: a,
            after: a.node(key).unwrap(),
        })
    );
    let changes: Vec<_> = delta.reference_changes().collect();
    let [
        ReferenceChange::Changed {
            before,
            after,
            changes,
        },
    ] = changes.as_slice()
    else {
        panic!("one span modification expected")
    };
    assert_eq!(
        dimensions(*changes),
        (None, false, true, false, false, false)
    );
    assert_eq!(
        before.from().node().span(),
        SourceSpan::new(14, 17).unwrap()
    );
    assert_eq!(after.from().node().span(), SourceSpan::new(15, 18).unwrap());
}

#[test]
fn exact_node_span_change_does_not_create_a_reference_change() {
    let before = snapshot(vec![source(
        b"a",
        &["", "/target", "/r"],
        vec![local("/r", "#/target", "/target")],
    )]);
    let mut value = source(b"a", &["", "/r"], vec![local("/r", "#/target", "/target")]);
    value.nodes.push(node("/target", 1, 2));
    let after = snapshot(vec![value]);
    let delta = compare(&before, &after).unwrap();
    assert_eq!(delta.source_changes().count(), 0);
    assert_eq!(delta.reference_changes().count(), 0);
    assert_eq!(
        delta.node_changes().collect::<Vec<_>>(),
        vec![NodeChange::SpanChanged {
            before_source: &before.sources()[0],
            before: before.sources()[0].node("/target").unwrap(),
            after_source: &after.sources()[0],
            after: after.sources()[0].node("/target").unwrap(),
        }]
    );
}

#[test]
fn occurrence_membership_changes_independently_of_node_membership() {
    let before = snapshot(vec![source(b"a", &["", "/r"], vec![local("/r", "#", "")])]);
    let after = snapshot(vec![source(b"a", &["", "/r"], vec![])]);
    let removed = before.references().next().unwrap();
    let delta = compare(&before, &after).unwrap();
    assert_eq!(delta.source_changes().count(), 0);
    assert_eq!(delta.node_changes().count(), 0);
    assert_eq!(
        delta.reference_changes().collect::<Vec<_>>(),
        vec![ReferenceChange::Removed(removed)]
    );
    assert_eq!(
        compare(&after, &before)
            .unwrap()
            .reference_changes()
            .collect::<Vec<_>>(),
        vec![ReferenceChange::Added(removed)]
    );
}

#[test]
fn retargeting_distinguishes_locator_and_cross_source_endpoint_identity() {
    let make = |local_target: &str, remote: &str| {
        snapshot(vec![
            source(b"a", &["", "/same"], vec![]),
            source(b"b", &["", "/same"], vec![]),
            source(
                b"refs",
                &["", "/local", "/remote", "/one", "/two"],
                vec![
                    local(
                        "/local",
                        local_target,
                        local_target.strip_prefix('#').unwrap(),
                    ),
                    outside("/remote", remote),
                ],
            ),
        ])
    };
    let before = make("#/one", "a#/same");
    let after = make("#/two", "b#/same");
    let delta = compare(&before, &after).unwrap();
    let changes: Vec<_> = delta.reference_changes().collect();
    assert_eq!(changes.len(), 2);
    for (change, key, original_target, target_path, target_key) in [
        (&changes[0], "/local", true, b"refs".as_slice(), "/two"),
        (&changes[1], "/remote", false, b"b".as_slice(), "/same"),
    ] {
        let ReferenceChange::Changed {
            before,
            after,
            changes,
        } = change
        else {
            panic!("retarget expected")
        };
        assert_eq!(before.from().node().locator(), key);
        assert_eq!(after.from().node().locator(), key);
        assert_eq!(
            dimensions(*changes),
            (
                Some(ResolutionChange::Retargeted),
                true,
                false,
                original_target,
                false,
                false
            )
        );
        let Resolution::Resolved(target) = after.target() else {
            panic!("resolved")
        };
        assert_eq!(target.path().bytes(), target_path);
        assert_eq!(target.node().locator(), target_key);
    }
    let ReferenceChange::Changed {
        before: old,
        after: new,
        ..
    } = changes[1]
    else {
        panic!("changed")
    };
    assert_eq!(old.reference().spelling(), "a#/same");
    assert_eq!(new.reference().spelling(), "b#/same");
}

#[test]
fn spelling_changes_preserve_equal_targets_and_origin_disposition() {
    let make = |spelling| {
        snapshot(vec![
            source(b"a", &["", "/r"], vec![outside("/r", spelling)]),
            source(b"b", &[""], vec![]),
        ])
    };
    let before = make("b");
    let after = make("b#");
    let changes: Vec<_> = compare(&before, &after)
        .unwrap()
        .reference_changes()
        .collect();
    let [
        ReferenceChange::Changed {
            before,
            after,
            changes,
        },
    ] = changes.as_slice()
    else {
        panic!("spelling change")
    };
    assert_eq!(
        dimensions(*changes),
        (None, true, false, false, false, false)
    );
    assert_eq!(before.reference().spelling(), "b");
    assert_eq!(after.reference().spelling(), "b#");
    for occurrence in [before, after] {
        assert_eq!(occurrence.reference().origin(), RelationOrigin::Extracted);
        assert_eq!(
            occurrence.reference().disposition(),
            ValidationDisposition::Unverified
        );
        assert_eq!(
            occurrence.reference().target(),
            &ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage)
        );
    }
}

#[test]
fn unresolved_reason_transition_tracks_changed_coverage_without_invented_targets() {
    let referring = || source(b"a", &["", "/r"], vec![outside("/r", "b#/absent")]);
    let before = snapshot(vec![referring()]);
    let after = snapshot(vec![referring(), source(b"b", &[""], vec![])]);
    let delta = compare(&before, &after).unwrap();
    let changes: Vec<_> = delta.reference_changes().collect();
    let [
        ReferenceChange::Changed {
            before: b,
            after: a,
            changes,
        },
    ] = changes.as_slice()
    else {
        panic!("reason transition")
    };
    assert_eq!(
        dimensions(*changes),
        (
            Some(ResolutionChange::UnresolvedReasonChanged),
            false,
            false,
            false,
            false,
            false
        )
    );
    assert_eq!(
        b.target(),
        Resolution::Unresolved(UnresolvedReference::OutsideCoverage)
    );
    assert_eq!(
        a.target(),
        Resolution::Unresolved(UnresolvedReference::MissingTarget)
    );
    assert!(!delta.before().complete());
    assert!(!delta.after().complete());
    assert_eq!(
        after
            .incoming(&SourcePath::new(b"b").unwrap(), "")
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn unsupported_and_external_reasons_and_unchanged_unresolved_coverage_remain_explicit() {
    let make = |spelling, reason| {
        snapshot(vec![source(
            b"a",
            &["", "/changed", "/unchanged"],
            vec![
                unresolved("/changed", spelling, reason),
                outside("/unchanged", "https://example.invalid/private"),
            ],
        )])
    };
    let before = make("#anchor", UnresolvedReference::UnsupportedFragment);
    let after = make(
        "https://example.invalid/x",
        UnresolvedReference::OutsideCoverage,
    );
    let delta = compare(&before, &after).unwrap();
    let changes: Vec<_> = delta.reference_changes().collect();
    let [
        ReferenceChange::Changed {
            before,
            after,
            changes,
        },
    ] = changes.as_slice()
    else {
        panic!("one change")
    };
    assert_eq!(
        dimensions(*changes),
        (
            Some(ResolutionChange::UnresolvedReasonChanged),
            true,
            false,
            true,
            false,
            false
        )
    );
    assert_eq!(
        before.target(),
        Resolution::Unresolved(UnresolvedReference::UnsupportedFragment)
    );
    assert_eq!(
        after.target(),
        Resolution::Unresolved(UnresolvedReference::OutsideCoverage)
    );
    assert_eq!(delta.before().references().len(), 2);
    assert_eq!(delta.after().references().len(), 2);
    assert!(!delta.after().complete());
    let equal = compare(delta.after(), delta.after()).unwrap();
    assert_eq!(equal.reference_changes().count(), 0);
    assert!(!equal.after().complete());
}

#[test]
fn selection_resolution_and_its_reverse_do_not_require_chronological_order() {
    let referring = || source(b"a", &["", "/r"], vec![outside("/r", "b")]);
    let before = snapshot(vec![referring()]);
    let after = snapshot(vec![referring(), source(b"b", &[""], vec![])]);
    for (b, a, expected) in [
        (&before, &after, ResolutionChange::BecameResolved),
        (&after, &before, ResolutionChange::BecameUnresolved),
    ] {
        let changes: Vec<_> = compare(b, a).unwrap().reference_changes().collect();
        let [ReferenceChange::Changed { changes, .. }] = changes.as_slice() else {
            panic!("resolution transition")
        };
        assert_eq!(
            dimensions(*changes),
            (Some(expected), false, false, false, false, false)
        );
    }
}

#[test]
fn original_target_representation_can_change_while_collection_stays_unsupported() {
    let before = snapshot(vec![source(
        b"a",
        &["", "/r"],
        vec![unresolved(
            "/r",
            "#/a#b",
            UnresolvedReference::MissingTarget,
        )],
    )]);
    let after = snapshot(vec![source(
        b"a",
        &["", "/r", "/a#b"],
        vec![local("/r", "#/a#b", "/a#b")],
    )]);
    let changes: Vec<_> = compare(&before, &after)
        .unwrap()
        .reference_changes()
        .collect();
    let [
        ReferenceChange::Changed {
            before,
            after,
            changes,
        },
    ] = changes.as_slice()
    else {
        panic!("representation change")
    };
    assert_eq!(
        dimensions(*changes),
        (None, false, false, true, false, false)
    );
    assert_eq!(
        before.target(),
        Resolution::Unresolved(UnresolvedReference::UnsupportedFragment)
    );
    assert_eq!(
        after.target(),
        Resolution::Unresolved(UnresolvedReference::UnsupportedFragment)
    );
    assert_eq!(
        before.reference().target(),
        &ReferenceTarget::Unresolved(UnresolvedReference::MissingTarget)
    );
    assert_eq!(
        after.reference().target(),
        &ReferenceTarget::Local("/a#b".into())
    );
}

#[test]
fn distinct_candidates_captures_provenance_and_times_do_not_become_structural_changes() {
    let sources = || vec![source(b"a", &["", "/r"], vec![local("/r", "#", "")])];
    let before = Snapshot::default().build(sources(), false);
    let after = Snapshot {
        scope: "another-series",
        first: 20,
        revision: 2,
        commit: 'b',
        ..Snapshot::default()
    }
    .build(sources(), true);
    let original_before = before.clone();
    let original_after = after.clone();
    let delta = compare(&before, &after).unwrap();
    assert!(std::ptr::eq(delta.before(), &before));
    assert!(std::ptr::eq(delta.after(), &after));
    assert_ne!(delta.before().candidate(), delta.after().candidate());
    assert_ne!(delta.before().id(), delta.after().id());
    assert_ne!(delta.before().provenance(), delta.after().provenance());
    assert_ne!(delta.before().recorded_at(), delta.after().recorded_at());
    for _ in 0..3 {
        assert_eq!(delta.source_changes().count(), 0);
        assert_eq!(delta.node_changes().count(), 0);
        assert_eq!(delta.reference_changes().count(), 0);
    }
    assert_eq!(before, original_before);
    assert_eq!(after, original_after);
    assert_eq!(
        delta.before().candidate().baseline(),
        &ObservationId::new("delta-fixture", 1).unwrap()
    );
    assert_eq!(
        delta.after().candidate().observation().id(),
        &ObservationId::new("another-series", 21).unwrap()
    );
    assert_eq!(
        delta.after().sources()[0].id(),
        &ObservationId::new("another-series", 22).unwrap()
    );
}

#[test]
fn byte_paths_and_locators_define_literal_deterministic_order_under_permutations() {
    let sources = || {
        [b"raw-\xff/a".as_slice(), b"raw-\xef\xbf\xbd/a", b"A"]
            .map(|path| {
                source(
                    path,
                    &["/2", "", "/10"],
                    vec![local("/2", "#", ""), local("/10", "#", "")],
                )
            })
            .into_iter()
            .collect()
    };
    let before = snapshot(vec![source(b"0", &[""], vec![])]);
    let after = Snapshot::default().build(sources(), false);
    let permuted = Snapshot::default().build(sources(), true);
    let delta = compare(&before, &after).unwrap();
    let again = compare(&before, &permuted).unwrap();
    assert_eq!(
        delta.source_changes().collect::<Vec<_>>(),
        again.source_changes().collect::<Vec<_>>()
    );
    assert_eq!(
        delta.node_changes().collect::<Vec<_>>(),
        again.node_changes().collect::<Vec<_>>()
    );
    assert_eq!(
        delta.reference_changes().collect::<Vec<_>>(),
        again.reference_changes().collect::<Vec<_>>()
    );
    let changes: Vec<_> = delta
        .reference_changes()
        .map(|change| match change {
            ReferenceChange::Added(r) => (r.from().path().bytes(), r.from().node().locator()),
            _ => panic!("added reference"),
        })
        .collect();
    assert_eq!(
        changes,
        vec![
            (b"A".as_slice(), "/10"),
            (b"A", "/2"),
            (b"raw-\xef\xbf\xbd/a", "/10"),
            (b"raw-\xef\xbf\xbd/a", "/2"),
            (b"raw-\xff/a", "/10"),
            (b"raw-\xff/a", "/2"),
        ]
    );
    assert_eq!(
        delta.source_changes().collect::<Vec<_>>(),
        vec![
            SourceChange::Removed(&before.sources()[0]),
            SourceChange::Added(&after.sources()[0]),
            SourceChange::Added(&after.sources()[1]),
            SourceChange::Added(&after.sources()[2]),
        ]
    );
    let old_before = before.clone();
    let old_after = after.clone();
    assert_eq!(delta.reference_changes().take(1).count(), 1);
    assert_eq!(delta.reference_changes().count(), 6);
    assert_eq!(before, old_before);
    assert_eq!(after, old_after);
}

#[test]
fn non_utf8_and_replacement_character_paths_remain_distinct_resolution_endpoints() {
    let make = |spelling: &str| {
        snapshot(vec![
            source(b"raw-\xff/a", &[""], vec![]),
            source(b"raw-\xef\xbf\xbd/a", &[""], vec![]),
            source(b"refs", &["", "/r"], vec![outside("/r", spelling)]),
        ])
    };
    // UTF-8 spelling can target only the replacement-character path. The raw-byte
    // source remains a distinct selected endpoint, not its lossy text alias.
    let before = make("raw-\u{fffd}/a");
    let after = make("raw-\u{fffd}/a#");
    let changes: Vec<_> = compare(&before, &after)
        .unwrap()
        .reference_changes()
        .collect();
    let [
        ReferenceChange::Changed {
            before,
            after,
            changes,
        },
    ] = changes.as_slice()
    else {
        panic!("spelling change")
    };
    assert_eq!(
        dimensions(*changes),
        (None, true, false, false, false, false)
    );
    for occurrence in [before, after] {
        let Resolution::Resolved(target) = occurrence.target() else {
            panic!("resolved")
        };
        assert_eq!(target.path().bytes(), b"raw-\xef\xbf\xbd/a");
        assert_ne!(target.path().bytes(), b"raw-\xff/a");
    }
}

#[test]
fn foreign_repository_and_linked_worktree_declarations_fail_without_payloads() {
    let sources = || vec![source(b"a", &[""], vec![])];
    let before = snapshot(sources());
    for repository in [
        repository("/other-private-root", ".git", ".git"),
        repository("/synthetic-cortex-delta", "other-worktree-git", ".git"),
        repository("/synthetic-cortex-delta", ".git", "other-common-git"),
        repository("/synthetic-cortex-delta/", ".git", ".git"),
    ] {
        let after = Snapshot {
            repository,
            ..Snapshot::default()
        }
        .build(sources(), false);
        assert_eq!(
            compare(&before, &after).unwrap_err(),
            DeltaError::RepositoryMismatch
        );
    }
}

#[test]
fn collection_source_capture_and_rule_profile_mismatches_are_typed_errors() {
    let sources = || vec![source(b"a", &[""], vec![]), source(b"z", &[""], vec![])];
    let before = snapshot(sources());
    for (spec, expected) in [
        (
            Snapshot {
                mechanism: "different-set",
                ..Snapshot::default()
            },
            DeltaError::CollectionMechanismMismatch,
        ),
        (
            Snapshot {
                profile: "opaque-v1;source-profile=opaque-parser-v2;rules=lexical-v1",
                ..Snapshot::default()
            },
            DeltaError::CollectionProfileMismatch,
        ),
        (
            Snapshot {
                profile: "opaque-v1;source-profile=opaque-parser-v1;rules=lexical-v2",
                ..Snapshot::default()
            },
            DeltaError::CollectionProfileMismatch,
        ),
        (
            Snapshot {
                capture_profile: "different-capture",
                ..Snapshot::default()
            },
            DeltaError::CaptureProfileMismatch,
        ),
    ] {
        let after = spec.build(sources(), true);
        assert_eq!(compare(&before, &after).unwrap_err(), expected);
    }
    for (mechanism, profile, expected) in [
        (
            "different-source",
            "opaque-parser-v1;rules=local-pointer-v1",
            DeltaError::SourceMechanismMismatch,
        ),
        (
            "synthetic-source",
            "opaque-parser-v1;rules=local-pointer-v2",
            DeltaError::SourceProfileMismatch,
        ),
        (
            "synthetic-source",
            "opaque-parser-v1;rules=local-pointer-v1 ",
            DeltaError::SourceProfileMismatch,
        ),
    ] {
        let mut values = sources();
        values[1].mechanism = mechanism;
        values[1].profile = profile;
        let after = snapshot(values);
        assert_eq!(compare(&before, &after).unwrap_err(), expected);
    }
}

#[test]
fn opaque_assertions_and_new_removed_selection_profiles_are_retained_without_qualification() {
    let old = source(b"old", &[""], vec![]);
    let common = || source(b"common", &[""], vec![]);
    let mut new = source(b"new", &[""], vec![]);
    new.mechanism = "unqualified-new-producer";
    new.profile = "opaque-unknown-rules";
    let before = snapshot(vec![old, common()]);
    let after = snapshot(vec![new, common()]);
    let delta = compare(&before, &after).unwrap();
    assert_eq!(
        delta.source_changes().collect::<Vec<_>>(),
        vec![
            SourceChange::Added(&after.sources()[1]),
            SourceChange::Removed(&before.sources()[1]),
        ]
    );
    assert_eq!(
        delta.after().sources()[1].provenance().source(),
        "unqualified-new-producer"
    );
    assert_eq!(
        delta.after().sources()[1].provenance().version(),
        "opaque-unknown-rules"
    );
}

#[test]
fn simultaneous_reference_dimensions_and_borrowed_values_survive_without_mutation() {
    let before = snapshot(vec![source(
        b"a",
        &["", "/a", "/b", "/r"],
        vec![local("/r", "#/a", "/a")],
    )]);
    let mut changed = source(b"a", &["", "/a", "/b"], vec![local("/r", "#/b", "/b")]);
    changed.nodes.push(node("/r", 2, 5));
    changed.bytes[0] = b'1';
    let after = snapshot(vec![changed]);
    let old_before = before.clone();
    let old_after = after.clone();
    let delta = compare(&before, &after).unwrap();
    assert_eq!(delta.source_changes().count(), 1);
    assert_eq!(delta.node_changes().count(), 1);
    let changes: Vec<_> = delta.reference_changes().collect();
    let [
        ReferenceChange::Changed {
            before: b,
            after: a,
            changes,
        },
    ] = changes.as_slice()
    else {
        panic!("changed")
    };
    assert_eq!(
        dimensions(*changes),
        (
            Some(ResolutionChange::Retargeted),
            true,
            true,
            true,
            false,
            false
        )
    );
    assert!(std::ptr::eq(
        b.reference(),
        &before.sources()[0].references()[0]
    ));
    assert!(std::ptr::eq(a.from().source(), &after.sources()[0]));
    assert_eq!(b.from().source().candidate(), before.candidate());
    assert_eq!(
        a.from().source().provenance(),
        after.sources()[0].provenance()
    );
    assert_eq!(before, old_before);
    assert_eq!(after, old_after);
}

#[test]
fn diagnostics_omit_source_candidate_locator_spelling_and_profile_payloads() {
    let before = snapshot(vec![source(
        b"private-source",
        &["", "/private-locator"],
        vec![outside("/private-locator", "private-spelling")],
    )]);
    let after = snapshot(vec![source(b"private-other", &[""], vec![])]);
    let delta = compare(&before, &after).unwrap();
    let diagnostics = format!(
        "{delta:?} {:?} {:?} {:?} {} {:?}",
        delta.source_changes().collect::<Vec<_>>(),
        delta.node_changes().collect::<Vec<_>>(),
        delta.reference_changes().collect::<Vec<_>>(),
        DeltaError::SourceProfileMismatch,
        DeltaError::RepositoryMismatch,
    );
    for sentinel in [
        "private-source",
        "private-other",
        "private-locator",
        "private-spelling",
        "delta-task",
        "synthetic-cortex-delta",
        "opaque-parser",
        "delta-fixture",
    ] {
        assert!(
            !diagnostics.contains(sentinel),
            "payload appeared in diagnostics"
        );
    }
    assert!(diagnostics.contains("SourceProfileMismatch"));
    assert!(diagnostics.contains("RepositoryMismatch"));
}

#[test]
fn aggregate_maxima_bound_full_disjoint_reports_and_iterators_terminate() {
    let sources = |prefix: &str| {
        (0..MAX_SOURCES)
            .map(|i| {
                let mut value = source(format!("{prefix}-{i}").as_bytes(), &[""], vec![]);
                value.bytes = vec![b' '; MAX_SET_BYTES / MAX_SOURCES];
                value.nodes.extend(
                    (1..MAX_SET_NODES / MAX_SOURCES).map(|j| node(&format!("/{j:04}"), 0, 1)),
                );
                value.references = (1..=MAX_SET_REFERENCES / MAX_SOURCES)
                    .map(|j| local(&format!("/{j:04}"), "#", ""))
                    .collect();
                value
            })
            .collect()
    };
    let before = snapshot(sources("before"));
    let after = snapshot(sources("after"));
    assert_eq!(
        before
            .sources()
            .iter()
            .map(|s| s.bytes().len())
            .sum::<usize>(),
        1_048_576
    );
    assert_eq!(
        before
            .sources()
            .iter()
            .map(|s| s.nodes().len())
            .sum::<usize>(),
        8192
    );
    assert_eq!(before.references().len(), 2048);
    let delta = compare(&before, &after).unwrap();
    assert_eq!(delta.source_changes().count(), 16);
    assert_eq!(delta.node_changes().count(), 16384);
    assert_eq!(delta.reference_changes().count(), 4096);
    assert!(
        matches!(delta.source_changes().next(), Some(SourceChange::Added(s)) if s.path().bytes() == b"after-0")
    );
    assert!(
        matches!(delta.source_changes().last(), Some(SourceChange::Removed(s)) if s.path().bytes() == b"before-7")
    );
    let mut references = delta.reference_changes();
    assert!(references.nth(4095).is_some());
    assert!(references.next().is_none());
    assert!(references.next().is_none());
}
