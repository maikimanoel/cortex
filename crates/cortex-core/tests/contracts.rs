//! Behavioral checks for task contracts and independent source identities.

use cortex_core::{
    CandidateId, ContractError, MAX_CONTRACT_TEXT_BYTES, MAX_TASK_ID_BYTES, TaskContract, TaskId,
};

fn task_id() -> TaskId {
    TaskId::new("M001.cli-help").unwrap()
}

#[test]
fn task_identifiers_preserve_valid_text_and_accept_the_size_boundary() {
    let id = TaskId::new("M001.cli_help-1").unwrap();
    assert_eq!(id.as_str(), "M001.cli_help-1");
    assert!(TaskId::new(&"a".repeat(MAX_TASK_ID_BYTES)).is_ok());
    assert_eq!(
        TaskId::new(&"a".repeat(MAX_TASK_ID_BYTES + 1)),
        Err(ContractError::TooLong {
            field: "task_id",
            max_bytes: MAX_TASK_ID_BYTES,
        })
    );
}

#[test]
fn task_identifiers_reject_blank_and_ambiguous_inputs() {
    for input in ["", " ", "\n"] {
        assert_eq!(TaskId::new(input), Err(ContractError::Blank("task_id")));
    }
    for input in ["-task", ".task", "_task", "a b", "a/b", "a\n", "tâche"] {
        assert_eq!(
            TaskId::new(input),
            Err(ContractError::InvalidCharacter("task_id"))
        );
    }
}

#[test]
fn candidate_identifiers_accept_full_git_object_id_formats() {
    for revision in ["a1".repeat(20), "b2".repeat(32)] {
        assert_eq!(
            CandidateId::from_git_commit(&revision).unwrap().as_str(),
            revision
        );
    }
}

#[test]
fn candidate_identifiers_reject_abbreviations_and_malformed_revisions() {
    for revision in [
        String::new(),
        "abc1234".into(),
        "a".repeat(39),
        "a".repeat(41),
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(40),
        "g".repeat(40),
        format!("{} ", "a".repeat(39)),
        "é".repeat(20),
    ] {
        assert_eq!(
            CandidateId::from_git_commit(&revision),
            Err(ContractError::InvalidGitRevision)
        );
    }
}

#[test]
fn contracts_preserve_identity_objective_and_observable_criterion() {
    let contract = TaskContract::new(
        task_id(),
        "Explain the command interface\nKeep terminology clear.",
        "Running cortex --help prints Usage and exits with code 0. ✓",
    )
    .unwrap();
    assert_eq!(contract.id(), &task_id());
    assert_eq!(
        contract.objective(),
        "Explain the command interface\nKeep terminology clear."
    );
    assert_eq!(
        contract.acceptance(),
        "Running cortex --help prints Usage and exits with code 0. ✓"
    );
}

#[test]
fn contracts_require_both_objective_and_acceptance_text() {
    for blank in ["", "  ", "\n"] {
        assert_eq!(
            TaskContract::new(task_id(), blank, "observable result"),
            Err(ContractError::Blank("objective"))
        );
        assert_eq!(
            TaskContract::new(task_id(), "desired change", blank),
            Err(ContractError::Blank("acceptance"))
        );
    }
}

#[test]
fn contract_limits_count_bytes_and_validate_each_field() {
    let boundary = "é".repeat(MAX_CONTRACT_TEXT_BYTES / 2);
    let too_long = format!("{boundary}x");
    assert!(TaskContract::new(task_id(), &boundary, &boundary).is_ok());
    assert_eq!(
        TaskContract::new(task_id(), &too_long, "criterion"),
        Err(ContractError::TooLong {
            field: "objective",
            max_bytes: MAX_CONTRACT_TEXT_BYTES,
        })
    );
    assert_eq!(
        TaskContract::new(task_id(), "objective", &too_long),
        Err(ContractError::TooLong {
            field: "acceptance",
            max_bytes: MAX_CONTRACT_TEXT_BYTES,
        })
    );
}

#[test]
fn contracts_reject_terminal_controls_without_echoing_the_payload() {
    for invalid in [
        "private-token\0",
        "private-token\u{1b}[31m",
        "private-token\r",
    ] {
        let error = TaskContract::new(task_id(), invalid, "criterion").unwrap_err();
        assert_eq!(error, ContractError::InvalidCharacter("objective"));
        assert!(!error.to_string().contains("private-token"));
        assert_eq!(
            TaskContract::new(task_id(), "objective", invalid),
            Err(ContractError::InvalidCharacter("acceptance"))
        );
    }
}
