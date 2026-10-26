//! Behavioral checks for provenance, judgment, and assessment transitions.

use cortex_core::{CandidateId, TaskContract, TaskId};
use cortex_evidence::{
    Evidence, EvidenceError, ExecutionObservation, MAX_EVIDENCE_TEXT_BYTES, Provenance, Verdict,
};

fn evidence(observation: ExecutionObservation) -> Evidence {
    Evidence::new(
        TaskContract::new(
            TaskId::new("M001.cli-help").unwrap(),
            "Provide command help",
            "cortex --help prints Usage and exits with code 0",
        )
        .unwrap(),
        CandidateId::from_git_commit(&"a1".repeat(20)).unwrap(),
        Provenance::new(
            "CLI integration test; Rust 1.99.1",
            "crates/cortex-cli; Cargo.lock at the stated candidate",
            "cargo test --workspace --locked; local Linux; no network",
        )
        .unwrap(),
        observation,
    )
}

#[test]
fn evidence_preserves_contract_candidate_and_verification_context() {
    let record = evidence(ExecutionObservation::NotRun);
    assert_eq!(record.contract().id().as_str(), "M001.cli-help");
    assert_eq!(
        record.contract().acceptance(),
        "cortex --help prints Usage and exits with code 0"
    );
    assert_eq!(record.candidate().as_str(), "a1".repeat(20));
    assert_eq!(
        record.provenance().mechanism(),
        "CLI integration test; Rust 1.99.1"
    );
    assert_eq!(
        record.provenance().inputs(),
        "crates/cortex-cli; Cargo.lock at the stated candidate"
    );
    assert_eq!(
        record.provenance().configuration(),
        "cargo test --workspace --locked; local Linux; no network"
    );
    assert_eq!(record.observation(), ExecutionObservation::NotRun);
    assert!(record.assessment().is_none());
}

#[test]
fn successful_process_exit_does_not_infer_a_pass() {
    let mut record = evidence(ExecutionObservation::Exited { code: 0 });
    assert!(record.assessment().is_none());
    record
        .assess(Verdict::Fail, "Process exited zero but help omitted Usage.")
        .unwrap();
    assert_eq!(record.assessment().unwrap().verdict(), Verdict::Fail);
}

#[test]
fn explicit_pass_requires_and_preserves_reasoning() {
    let mut record = evidence(ExecutionObservation::Exited { code: 0 });
    let original = record.clone();
    record
        .assess(Verdict::Pass, "Exit code was 0 and stdout included Usage.")
        .unwrap();
    assert_eq!(record.assessment().unwrap().verdict(), Verdict::Pass);
    assert_eq!(
        record.assessment().unwrap().rationale(),
        "Exit code was 0 and stdout included Usage."
    );
    assert_eq!(record.contract(), original.contract());
    assert_eq!(record.candidate(), original.candidate());
    assert_eq!(record.provenance(), original.provenance());
    assert_eq!(record.observation(), original.observation());
}

#[test]
fn every_observation_verdict_pair_enforces_execution_preconditions() {
    use ExecutionObservation::{Cancelled, Exited, NotRun, TimedOut, Unavailable};
    for observation in [
        NotRun,
        Exited { code: 0 },
        Exited { code: 1 },
        TimedOut,
        Cancelled,
        Unavailable,
    ] {
        for verdict in [Verdict::Pass, Verdict::Fail, Verdict::Inconclusive] {
            let mut record = evidence(observation);
            let expected = match (observation, verdict) {
                (_, Verdict::Inconclusive)
                | (Exited { code: 0 }, _)
                | (Exited { .. }, Verdict::Fail) => Ok(()),
                (Exited { .. }, Verdict::Pass) => Err(EvidenceError::NonzeroExitCannotPass),
                _ => Err(EvidenceError::ExecutionNotCompleted),
            };
            assert_eq!(
                record.assess(verdict, "Scoped assessment rationale"),
                expected,
                "{observation:?} / {verdict:?}"
            );
            match expected {
                Ok(()) => assert_eq!(record.assessment().unwrap().verdict(), verdict),
                Err(_) => assert!(record.assessment().is_none()),
            }
        }
    }
}

#[test]
fn rejected_pass_keeps_the_report_available_for_a_valid_assessment() {
    let mut record = evidence(ExecutionObservation::Exited { code: 2 });
    let original = record.clone();
    assert_eq!(
        record.assess(Verdict::Pass, "Unsupported"),
        Err(EvidenceError::NonzeroExitCannotPass)
    );
    assert_eq!(record, original);
    record
        .assess(
            Verdict::Fail,
            "The command rejected the required interface.",
        )
        .unwrap();
    assert_eq!(record.assessment().unwrap().verdict(), Verdict::Fail);
}

#[test]
fn all_assessments_are_terminal_and_cannot_be_overwritten() {
    for initial in [Verdict::Pass, Verdict::Fail, Verdict::Inconclusive] {
        let mut record = evidence(ExecutionObservation::Exited { code: 0 });
        record.assess(initial, "Initial reasoning").unwrap();
        let original = record.clone();
        for replacement in [Verdict::Pass, Verdict::Fail, Verdict::Inconclusive] {
            assert_eq!(
                record.assess(replacement, "Replacement reasoning"),
                Err(EvidenceError::AlreadyAssessed)
            );
            assert_eq!(record, original);
        }
    }
}

#[test]
fn unsupported_rationale_leaves_evidence_unassessed() {
    for invalid in [
        String::new(),
        " \n".into(),
        "x".repeat(MAX_EVIDENCE_TEXT_BYTES + 1),
        "private-token\u{1b}".into(),
    ] {
        let mut record = evidence(ExecutionObservation::Exited { code: 0 });
        let original = record.clone();
        let error = record.assess(Verdict::Pass, &invalid).unwrap_err();
        assert!(!error.to_string().contains("private-token"));
        assert_eq!(record, original);
    }
}

#[test]
fn provenance_rejects_missing_oversized_and_control_bearing_fields() {
    for (invalid, kind) in [
        (String::new(), "blank"),
        (" \n".into(), "blank"),
        ("x".repeat(MAX_EVIDENCE_TEXT_BYTES + 1), "long"),
        ("private-token\0".into(), "control"),
    ] {
        for (position, field) in ["mechanism", "inputs", "configuration"]
            .into_iter()
            .enumerate()
        {
            let mut values = ["mechanism", "inputs", "configuration"];
            values[position] = &invalid;
            let expected = match kind {
                "blank" => EvidenceError::Blank(field),
                "long" => EvidenceError::TooLong(field),
                _ => EvidenceError::InvalidCharacter(field),
            };
            assert_eq!(
                Provenance::new(values[0], values[1], values[2]),
                Err(expected)
            );
        }
    }
}

#[test]
fn provenance_and_rationale_accept_the_exact_utf8_byte_limit() {
    let boundary = "é".repeat(MAX_EVIDENCE_TEXT_BYTES / 2);
    let provenance = Provenance::new(&boundary, &boundary, &boundary).unwrap();
    assert_eq!(provenance.inputs(), boundary);
    let mut record = evidence(ExecutionObservation::NotRun);
    record.assess(Verdict::Inconclusive, &boundary).unwrap();
    assert_eq!(record.assessment().unwrap().rationale(), boundary);
    assert_eq!(
        Provenance::new(&format!("{boundary}x"), "inputs", "config"),
        Err(EvidenceError::TooLong("mechanism"))
    );
}
