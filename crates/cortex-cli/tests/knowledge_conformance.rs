//! CAP-KNW-001 / 003-B: conformance of the frozen single-source public API.
//! Expected endpoints, byte offsets and limits come from the reference contract,
//! independently of parser traversal. All repositories and payloads are synthetic.
#![cfg(target_os = "linux")]

mod support;

use cortex_cli::{
    inspect, inspect_candidate,
    knowledge::{ExtractionError, extract_json_references, qualify, reinspect},
};
use cortex_core::{CandidateId, TaskContract, TaskId, binding::*, knowledge::*};
use cortex_evidence::{ExecutionObservation, Provenance, unified::*};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use support::Repo;

fn id(sequence: u64) -> ObservationId {
    ObservationId::new("single-source-conformance", sequence).unwrap()
}

struct Fixture {
    repo: Repo,
    contract: CandidateBindingContract,
}

impl Fixture {
    fn new(files: &[(&str, &str)]) -> Self {
        let repo = Repo::new();
        for (name, source) in files {
            repo.commit_file(name, source);
        }
        let commit =
            String::from_utf8(repo.git(&["rev-parse", "--verify", "HEAD"]).stdout).unwrap();
        let contract = CandidateBindingContract::new(
            TaskContract::new(
                TaskId::new("pilot.003b.single-source-conformance").unwrap(),
                "Inspect one committed JSON source",
                "Preserve exact local references, bounds, privacy and uncertainty",
            )
            .unwrap(),
            ContractRevision::new(1).unwrap(),
            CandidateRequirement::new(
                inspect(&repo.path).unwrap().repository_identity().unwrap(),
                CandidateId::from_git_commit(commit.trim()).unwrap(),
                WorktreeRequirement::Clean,
            ),
        );
        Self { repo, contract }
    }

    fn extract(&self, path: &str) -> Result<ReferenceIndex, ExtractionError> {
        let before = snapshot(&self.repo);
        let result = extract_json_references(
            &self.repo.path,
            &self.contract,
            id(1),
            SourcePath::new(path.as_bytes()).unwrap(),
            UNIX_EPOCH,
        );
        // Compare even on rejection, before assertions about the returned result.
        assert_eq!(snapshot(&self.repo), before);
        result
    }
}

#[derive(Debug, PartialEq, Eq)]
struct FileState {
    bytes: Vec<u8>,
    modified: SystemTime,
    size: u64,
    mode: u32,
    directory: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    files: BTreeMap<PathBuf, FileState>,
    git_identity: Vec<u8>,
}

fn snapshot(repo: &Repo) -> Snapshot {
    // Include .git: index, objects, refs, logs, config and directory metadata.
    // Do not run status/diff here: they could refresh the index under test.
    // Access time and inode/change/birth metadata are outside this oracle.
    let mut pending = vec![repo.path.clone()];
    let mut files = BTreeMap::new();
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(!metadata.is_symlink());
        let directory = metadata.is_dir();
        let bytes = if directory {
            pending.extend(
                fs::read_dir(&path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
            vec![]
        } else {
            fs::read(&path).unwrap()
        };
        files.insert(
            path.strip_prefix(&repo.path).unwrap().to_owned(),
            FileState {
                bytes,
                modified: metadata.modified().unwrap(),
                size: metadata.len(),
                mode: metadata.permissions().mode(),
                directory,
            },
        );
    }
    let git_identity = repo
        .git(&[
            "--no-optional-locks",
            "rev-parse",
            "--show-toplevel",
            "--absolute-git-dir",
            "--git-common-dir",
            "HEAD",
            "HEAD^{tree}",
        ])
        .stdout;
    Snapshot {
        files,
        git_identity,
    }
}

fn assert_unverified(index: &ReferenceIndex) {
    assert!(!index.references().is_empty());
    for reference in index.references() {
        assert_eq!(reference.origin(), RelationOrigin::Extracted);
        assert_eq!(reference.disposition(), ValidationDisposition::Unverified);
    }
}

#[test]
fn pointer_identity_distinguishes_object_keys_from_array_positions() {
    let source = r##"{
        "":0,"a/b":{"~":1},"numeric":{"0":2,"01":3,"-":4,"-1":5},"array":[6],
        "refs":[{"$ref":"#"},{"$ref":"#/"},{"$ref":"#/a~1b/~0"},
        {"$ref":"#/numeric/0"},{"$ref":"#/numeric/01"},{"$ref":"#/numeric/-"},
        {"$ref":"#/numeric/-1"},{"$ref":"#/array/0"},{"$ref":"#/array/01"},
        {"$ref":"#/array/-"},{"$ref":"#/array/-1"},{"$ref":"#/array/1"}]
    }"##;
    let fixture = Fixture::new(&[("pointers.json", source)]);
    let index = fixture.extract("pointers.json").unwrap();
    let before = snapshot(&fixture.repo);
    let expected = [
        ("/refs/0/$ref", "#", Some("")),
        ("/refs/1/$ref", "#/", Some("/")),
        ("/refs/2/$ref", "#/a~1b/~0", Some("/a~1b/~0")),
        ("/refs/3/$ref", "#/numeric/0", Some("/numeric/0")),
        ("/refs/4/$ref", "#/numeric/01", Some("/numeric/01")),
        ("/refs/5/$ref", "#/numeric/-", Some("/numeric/-")),
        ("/refs/6/$ref", "#/numeric/-1", Some("/numeric/-1")),
        ("/refs/7/$ref", "#/array/0", Some("/array/0")),
        ("/refs/8/$ref", "#/array/01", None),
        ("/refs/9/$ref", "#/array/-", None),
        ("/refs/10/$ref", "#/array/-1", None),
        ("/refs/11/$ref", "#/array/1", None),
    ];
    assert_eq!(index.references().len(), expected.len());
    for (from, spelling, target) in expected {
        let reference = index
            .references()
            .iter()
            .find(|r| r.from() == from)
            .unwrap();
        assert_eq!(reference.spelling(), spelling);
        let expected_target = match target {
            Some(locator) => {
                assert_eq!(index.node(locator).unwrap().locator(), locator);
                let incoming: Vec<_> = index.incoming(locator).unwrap().map(|r| r.from()).collect();
                assert_eq!(incoming, [from]);
                ReferenceTarget::Local(locator.to_owned())
            }
            None => {
                assert_eq!(index.node(&spelling[1..]), Err(KnowledgeError::MissingNode));
                assert!(matches!(
                    index.incoming(&spelling[1..]),
                    Err(KnowledgeError::MissingNode)
                ));
                ReferenceTarget::Unresolved(UnresolvedReference::MissingTarget)
            }
        };
        assert_eq!(reference.target(), &expected_target, "{from}");
    }
    assert_eq!(index.incoming("/array").unwrap().count(), 0);
    assert!(!index.complete());
    assert_unverified(&index);
    assert_eq!(snapshot(&fixture.repo), before);
}

#[test]
fn lexical_classification_preserves_reasons_without_loading_committed_external_documents() {
    let source = r##"{"local":0,"%20":1,"refs":[
        {"$ref":"#/local"},{"$ref":"#/missing"},{"$ref":"#/bad~"},
        {"$ref":"#/bad~2"},{"$ref":"#/%20"},{"$ref":"#anchor"},
        {"$ref":"other.json"},{"$ref":"other.json#"},{"$ref":"other.json#/x"},
        {"$ref":"https://example.invalid/other.json#/x"},{"$ref":"malformed.json#/x"}]}"##;
    // A real external endpoint still lies outside this operation's coverage.
    // Parsing the other committed document would fail instead of preserving it.
    let fixture = Fixture::new(&[
        ("other.json", r#"{"x":0}"#),
        ("malformed.json", "not JSON"),
        ("lexical.json", source),
    ]);
    let index = fixture.extract("lexical.json").unwrap();
    let expected = [
        (
            "/refs/0/$ref",
            "#/local",
            ReferenceTarget::Local("/local".into()),
        ),
        (
            "/refs/1/$ref",
            "#/missing",
            ReferenceTarget::Unresolved(UnresolvedReference::MissingTarget),
        ),
        (
            "/refs/2/$ref",
            "#/bad~",
            ReferenceTarget::Unresolved(UnresolvedReference::UnsupportedFragment),
        ),
        (
            "/refs/3/$ref",
            "#/bad~2",
            ReferenceTarget::Unresolved(UnresolvedReference::UnsupportedFragment),
        ),
        (
            "/refs/4/$ref",
            "#/%20",
            ReferenceTarget::Unresolved(UnresolvedReference::UnsupportedFragment),
        ),
        (
            "/refs/5/$ref",
            "#anchor",
            ReferenceTarget::Unresolved(UnresolvedReference::UnsupportedFragment),
        ),
        (
            "/refs/6/$ref",
            "other.json",
            ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
        ),
        (
            "/refs/7/$ref",
            "other.json#",
            ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
        ),
        (
            "/refs/8/$ref",
            "other.json#/x",
            ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
        ),
        (
            "/refs/9/$ref",
            "https://example.invalid/other.json#/x",
            ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
        ),
        (
            "/refs/10/$ref",
            "malformed.json#/x",
            ReferenceTarget::Unresolved(UnresolvedReference::OutsideCoverage),
        ),
    ];
    assert_eq!(index.references().len(), expected.len());
    for (from, spelling, target) in expected {
        let reference = index
            .references()
            .iter()
            .find(|r| r.from() == from)
            .unwrap();
        assert_eq!(reference.spelling(), spelling);
        assert_eq!(reference.target(), &target);
    }
    assert_eq!(index.incoming("/%20").unwrap().count(), 0);
    assert_eq!(index.node("/x"), Err(KnowledgeError::MissingNode));
    let before = snapshot(&fixture.repo);
    let view = reinspect(&fixture.repo.path, &index, id(4)).unwrap();
    assert!(matches!(
        view.correspondence(),
        BindingDecision::Compatible(_)
    ));
    assert_eq!(view.validity(), None);
    assert_unverified(view.index());
    assert_eq!(snapshot(&fixture.repo), before);
}

#[test]
fn occurrence_spans_are_exact_utf8_byte_ranges_with_escaped_names_and_crlf() {
    let source = concat!(
        " \r\n{\r\n",
        "  \"ação\": 0,\r\n",
        r##"  "a\/b": {"\u0024ref": "#/ação"},"##,
        "\r\n",
        r##"  "雪": {"$ref": "#/ação"}"##,
        "\r\n} \t\r\n",
    );
    // Independent layout arithmetic: opening whitespace/brace occupy 6 bytes;
    // subsequent lines occupy 16, 38, 31, 5 bytes.
    // The two reference lines have 24/18-byte prefixes: tokens 46..56 / 78..88.
    // Each quoted token is ten bytes; its Unicode characters occupy fewer chars.
    assert_eq!(source.len(), 96);
    assert!(source.chars().count() < source.len());
    let fixture = Fixture::new(&[("spans.json", source)]);
    let index = fixture.extract("spans.json").unwrap();
    assert_eq!(index.bytes(), source.as_bytes());
    assert_eq!(
        index.node("").unwrap().span(),
        SourceSpan::new(3, 92).unwrap()
    );
    assert_eq!(
        index.node("/ação").unwrap().span(),
        SourceSpan::new(18, 19).unwrap()
    );
    assert_eq!(index.references().len(), 2);
    for (reference, (from, start, end)) in index
        .references()
        .iter()
        .zip([("/a~1b/$ref", 46, 56), ("/雪/$ref", 78, 88)])
    {
        assert_eq!(reference.from(), from);
        assert_eq!(reference.spelling(), "#/ação");
        assert_eq!(reference.target(), &ReferenceTarget::Local("/ação".into()));
        let span = index.node(from).unwrap().span();
        assert_eq!(span, SourceSpan::new(start, end).unwrap());
        assert_eq!(&index.bytes()[start..end], "\"#/ação\"".as_bytes());
    }
    assert_eq!(
        index
            .incoming("/ação")
            .unwrap()
            .map(|r| r.from())
            .collect::<Vec<_>>(),
        ["/a~1b/$ref", "/雪/$ref"]
    );
    assert!(index.complete());
    assert_eq!(fixture.extract("spans.json").unwrap(), index);
}

#[test]
fn source_byte_limit_accepts_256_kib_and_rejects_one_extra_valid_whitespace_byte() {
    // Valid scalar JSON plus insignificant whitespace isolates source-byte size.
    let accepted = format!("0{}", " ".repeat(262_143));
    let rejected = format!("{accepted} ");
    assert_eq!(accepted.len(), 262_144);
    assert_eq!(rejected.len(), 262_145);
    let fixture = Fixture::new(&[("at.json", &accepted), ("over.json", &rejected)]);
    let index = fixture.extract("at.json").unwrap();
    assert_eq!(index.bytes(), accepted.as_bytes());
    assert_eq!(index.nodes().len(), 1);
    assert_eq!(
        index.node("").unwrap().span(),
        SourceSpan::new(0, 1).unwrap()
    );
    let before = snapshot(&fixture.repo);
    assert_eq!(index.incoming("").unwrap().count(), 0);
    assert_eq!(
        reinspect(&fixture.repo.path, &index, id(4))
            .unwrap()
            .validity(),
        Some(Validity::Current)
    );
    assert_eq!(snapshot(&fixture.repo), before);
    // Acquisition may reject before parsing. The contract guarantees rejection,
    // without fixing which of those boundaries reports this source overflow.
    assert!(fixture.extract("over.json").is_err());
}

#[test]
fn node_limit_counts_the_root_as_well_as_its_scalar_children() {
    let accepted = format!("[{}]", vec!["0"; 4095].join(","));
    let rejected = format!("[{}]", vec!["0"; 4096].join(","));
    let fixture = Fixture::new(&[("at.json", &accepted), ("over.json", &rejected)]);
    let index = fixture.extract("at.json").unwrap();
    assert_eq!(index.nodes().len(), 4096);
    assert_eq!(
        index.node("/0").unwrap().span(),
        SourceSpan::new(1, 2).unwrap()
    );
    assert_eq!(
        index.node("/4094").unwrap().span(),
        SourceSpan::new(8189, 8190).unwrap()
    );
    assert_eq!(index.node("/4095"), Err(KnowledgeError::MissingNode));
    assert!(index.references().is_empty());
    assert!(matches!(
        fixture.extract("over.json"),
        Err(ExtractionError::Limit)
    ));
}

#[test]
fn reference_limit_is_independent_of_node_and_source_byte_limits() {
    let accepted = format!("[{}]", vec![r##"{"$ref":"#"}"##; 1024].join(","));
    let rejected = format!("[{}]", vec![r##"{"$ref":"#"}"##; 1025].join(","));
    // Root + object + reference-value per entry: 2049 / 2051 nodes, not 4096.
    assert_eq!(accepted.len(), 13_313);
    assert_eq!(rejected.len(), 13_326);
    let fixture = Fixture::new(&[("at.json", &accepted), ("over.json", &rejected)]);
    let index = fixture.extract("at.json").unwrap();
    assert_eq!(index.nodes().len(), 2049);
    assert_eq!(index.references().len(), 1024);
    assert_eq!(index.incoming("").unwrap().count(), 1024);
    for reference in index.references() {
        assert_eq!(reference.target(), &ReferenceTarget::Local(String::new()));
    }
    assert!(index.node("/1023/$ref").is_ok());
    assert!(index.complete());
    assert_unverified(&index);
    assert!(matches!(
        fixture.extract("over.json"),
        Err(ExtractionError::Limit)
    ));
}

#[test]
fn depth_limit_uses_root_depth_zero() {
    let accepted = format!("{}0{}", "[".repeat(32), "]".repeat(32));
    let rejected = format!("{}0{}", "[".repeat(33), "]".repeat(33));
    let fixture = Fixture::new(&[("at.json", &accepted), ("over.json", &rejected)]);
    let index = fixture.extract("at.json").unwrap();
    assert_eq!(index.nodes().len(), 33);
    let scalar = "/0".repeat(32);
    assert_eq!(
        index.node(&scalar).unwrap().span(),
        SourceSpan::new(32, 33).unwrap()
    );
    assert!(matches!(
        fixture.extract("over.json"),
        Err(ExtractionError::Limit)
    ));
}

#[test]
fn individual_locator_limit_includes_the_leading_slash() {
    let key = "a".repeat(4095);
    let accepted = format!("{{\"{key}\":0}}");
    let rejected = format!("{{\"{key}a\":0}}");
    let fixture = Fixture::new(&[("at.json", &accepted), ("over.json", &rejected)]);
    let index = fixture.extract("at.json").unwrap();
    let locator = format!("/{key}");
    assert_eq!(locator.len(), 4096);
    assert_eq!(index.nodes().len(), 2);
    assert_eq!(
        index.node(&locator).unwrap().span(),
        SourceSpan::new(4099, 4100).unwrap()
    );
    assert!(matches!(
        fixture.extract("over.json"),
        Err(ExtractionError::Limit)
    ));
}

#[test]
fn aggregate_locator_limit_accepts_the_independent_256_kib_construction() {
    let parent = "p".repeat(2043);
    let mut members: Vec<_> = (0..126).map(|n| format!("\"{n:03}\":0")).collect();
    members.push("\"special\":0".into());
    let accepted = format!("{{\"{parent}\":{{{}}}}}", members.join(","));
    // Only the selected seven-byte child key changes, by exactly one ASCII byte.
    *members.last_mut().unwrap() = "\"specialx\":0".into();
    let rejected = format!("{{\"{parent}\":{{{}}}}}", members.join(","));
    // Contract oracle: 2044 + 126 * 2048 + 2052 = 262144; root contributes 0.
    // Do not sum extracted locators to decide whether this fixture should pass.
    assert_eq!(accepted.len(), 3069);
    assert_eq!(rejected.len(), 3070);
    let fixture = Fixture::new(&[("at.json", &accepted), ("over.json", &rejected)]);
    let index = fixture.extract("at.json").unwrap();
    assert_eq!(index.nodes().len(), 129);
    assert!(index.node(&format!("/{parent}")).is_ok());
    assert!(index.node(&format!("/{parent}/000")).is_ok());
    assert!(index.node(&format!("/{parent}/125")).is_ok());
    assert!(index.node(&format!("/{parent}/special")).is_ok());
    assert!(matches!(
        fixture.extract("over.json"),
        Err(ExtractionError::Limit)
    ));
}

#[test]
fn debug_and_parser_errors_omit_sensitive_source_names_spellings_and_paths() {
    const SENTINEL: &str = "SYNTHETIC_SENSITIVE_003B";
    let source = format!("{{\"{SENTINEL}\":0,\"$ref\":\"#/{SENTINEL}\"}}");
    let malformed = format!("{{\"{SENTINEL}\":");
    let unsupported = format!("{{\"{SENTINEL}\":0,\"$ref\":42}}");
    let fixture = Fixture::new(&[
        (SENTINEL, &source),
        ("bad.json", &malformed),
        ("unsupported.json", &unsupported),
    ]);
    let index = fixture.extract(SENTINEL).unwrap();
    let reference = &index.references()[0];
    // First demonstrate that the synthetic private data actually reached the values.
    assert_eq!(index.path().bytes(), SENTINEL.as_bytes());
    assert_eq!(reference.spelling(), format!("#/{SENTINEL}"));
    let node = index.node(&format!("/{SENTINEL}")).unwrap();
    for diagnostic in [
        format!("{index:?}"),
        format!("{:?}", index.path()),
        format!("{node:?}"),
        format!("{reference:?}"),
        format!("{:?}", reference.target()),
    ] {
        assert!(!diagnostic.contains(SENTINEL));
    }
    let invalid = fixture.extract("bad.json").unwrap_err();
    assert!(matches!(invalid, ExtractionError::InvalidJson));
    let unsupported = fixture.extract("unsupported.json").unwrap_err();
    assert!(matches!(unsupported, ExtractionError::Unsupported));
    for error in [invalid, unsupported] {
        let mut cause: Option<&dyn Error> = Some(&error);
        while let Some(current) = cause {
            assert!(!format!("{current}").contains(SENTINEL));
            assert!(!format!("{current:?}").contains(SENTINEL));
            cause = current.source();
        }
    }
}

#[test]
fn missing_partial_and_uncertain_context_never_establish_current_validity() {
    let fixture = Fixture::new(&[("source.json", r##"{"$ref":"#"}"##)]);
    let index = fixture.extract("source.json").unwrap();
    let original = index.clone();
    let before = snapshot(&fixture.repo);
    let missing = qualify(&index, None).unwrap();
    assert_eq!(missing.validity(), None);
    assert_eq!(
        missing.correspondence(),
        &BindingDecision::Insufficient(BindingInsufficiency::ReobservationRequired)
    );
    let actual = inspect_candidate(&fixture.repo.path, &fixture.contract, id(4)).unwrap();
    assert_eq!(
        qualify(&index, Some(&actual)).unwrap().validity(),
        Some(Validity::Current)
    );
    let state = actual.state();
    // These are explicit counterfactual coverage assertions, not fresh Git captures.
    for (repository, head, coverage, consistency, basis, reason) in [
        (
            None,
            state.head().clone(),
            state.coverage(),
            state.consistency(),
            actual.basis().cloned(),
            BindingInsufficiency::MissingRepository,
        ),
        (
            state.repository().cloned(),
            Head::Unknown,
            state.coverage(),
            state.consistency(),
            actual.basis().cloned(),
            BindingInsufficiency::MissingCommit,
        ),
        (
            state.repository().cloned(),
            state.head().clone(),
            ObservationCoverage::Incomplete,
            state.consistency(),
            actual.basis().cloned(),
            BindingInsufficiency::IncompleteCoverage,
        ),
        (
            state.repository().cloned(),
            state.head().clone(),
            state.coverage(),
            ObservationConsistency::Indeterminate,
            actual.basis().cloned(),
            BindingInsufficiency::IndeterminateConsistency,
        ),
        (
            state.repository().cloned(),
            state.head().clone(),
            state.coverage(),
            state.consistency(),
            None,
            BindingInsufficiency::MissingComparisonBasis,
        ),
    ] {
        let uncertain = CandidateObservation::new(
            id(4),
            &fixture.contract,
            RepositoryState::new(repository, head, vec![], coverage, consistency).unwrap(),
            basis,
        );
        let view = qualify(&index, Some(&uncertain)).unwrap();
        assert_eq!(view.validity(), None);
        assert_eq!(
            view.correspondence(),
            &BindingDecision::Insufficient(reason)
        );
        assert_unverified(view.index());
    }
    let reused = inspect_candidate(&fixture.repo.path, &fixture.contract, id(3)).unwrap();
    let view = qualify(&index, Some(&reused)).unwrap();
    assert_eq!(view.validity(), Some(Validity::Stale));
    assert_eq!(
        view.correspondence(),
        &BindingDecision::Stale(BindingStaleness::ObservationOrder)
    );
    let current = reinspect(&fixture.repo.path, &index, id(5)).unwrap();
    assert_eq!(current.validity(), Some(Validity::Current));
    assert_unverified(current.index());
    assert_eq!(index, original);
    assert_eq!(snapshot(&fixture.repo), before);
}

#[test]
fn current_extraction_cannot_supply_missing_verification_or_manufacture_pass() {
    let fixture = Fixture::new(&[("source.json", r##"{"$ref":"#"}"##)]);
    let index = fixture.extract("source.json").unwrap();
    let current = inspect_candidate(&fixture.repo.path, &fixture.contract, id(4)).unwrap();
    assert_eq!(
        qualify(&index, Some(&current)).unwrap().validity(),
        Some(Validity::Current)
    );
    let profile = VerificationProfile::new(
        Provenance::new(
            "Synthetic verification report",
            "No verification evidence supplied",
            "Conformance test only",
        )
        .unwrap(),
        "1",
        "synthetic Linux context",
        &[],
    )
    .unwrap();
    let context = ValidationContext::new(&fixture.contract, Some(&current), Some(&profile));
    for (execution, expected_error) in [
        (
            ExecutionObservation::NotRun,
            SemanticError::UnsupportedResult,
        ),
        (
            ExecutionObservation::Exited { code: 0 },
            SemanticError::MissingOracle,
        ),
    ] {
        // Caller assertions deliberately lack checks/oracle, even with exit zero.
        let mut evidence = EvidenceRecord::new(
            ReportedObservation::new(
                ObservationId::new("synthetic-verification", 1).unwrap(),
                index.candidate().observation().clone(),
                profile.clone(),
                ObservationTimes::new(UNIX_EPOCH, UNIX_EPOCH).unwrap(),
                ObservationKind::Execution(VerificationOutcome::new(execution, 0, None).unwrap()),
                "No executed semantic verification is being claimed",
            )
            .unwrap(),
        );
        let view = evidence.inspect(&context);
        assert_eq!(view.validity(), Some(Validity::Current));
        assert_eq!(view.result(), None);
        assert!(!view.current_pass());
        assert_eq!(
            evidence.assess(
                AssessmentId::new("unsupported-pass").unwrap(),
                UNIX_EPOCH,
                AssessmentResult::Pass,
                Applicability::Applicable,
                "Extraction is not an oracle for this verification report",
                &context,
            ),
            Err(expected_error)
        );
        assert!(evidence.assessment().is_none());
        assert!(!evidence.inspect(&context).current_pass());
    }
    assert_unverified(&index);
    // Index/view/binding/validity return types carry no permission or assessment;
    // that API boundary is also inspected during review, not inferred from CURRENT.
}
