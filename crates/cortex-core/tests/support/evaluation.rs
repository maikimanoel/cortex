#![allow(dead_code)]
use cortex_core::{
    CandidateId, TaskContract, TaskId, binding::*, evaluation::*, program::scheduling::*,
    temporal::SourceReference,
};
use std::path::PathBuf;
pub fn rev(n: u64) -> ContractRevision {
    ContractRevision::new(n).unwrap()
}
pub fn reference(name: &str) -> SourceReference {
    SourceReference::operator_reported("evaluation-fixture-owner", name, "1").unwrap()
}
pub fn candidate(hex: &str) -> CandidateId {
    CandidateId::from_git_commit(&hex.repeat(40)).unwrap()
}
pub fn source() -> SourceIdentity {
    let root = PathBuf::from("/synthetic/evaluation-source");
    SourceIdentity::new(
        CandidateRequirement::new(
            RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git")).unwrap(),
            candidate("a"),
            WorktreeRequirement::Clean,
        ),
        SourceTreeId::new(&"b".repeat(40)).unwrap(),
    )
    .unwrap()
}
pub fn case() -> EvaluationCase {
    EvaluationCase {
        id: CaseId::new("arithmetic").unwrap(),
        revision: rev(1),
        contract: CandidateBindingContract::new(
            TaskContract::new(
                TaskId::new("engineering.arithmetic").unwrap(),
                "Calculate an independently checked finite result",
                "Exact squared sum is 87 for 2, 3, 5, 7",
            )
            .unwrap(),
            rev(1),
            source().candidate().clone(),
        ),
        source: source(),
        oracle: reference("independent-literal-oracle"),
        family: WorkloadFamily::Independent,
        access: CaseAccess::Development,
    }
}
pub fn configuration(name: &str) -> RunConfiguration {
    RunConfiguration {
        reference: reference(name),
        profile: ExecutionProfile::Synthetic,
        model: ModelConfiguration::Configured(reference("hypothetical-model-settings")),
        tools: vec![reference("rust-toolchain")],
        harness: reference(name),
        environment: reference("synthetic-environment"),
        information: reference("exact-public-fixture-input"),
        envelope: vec![
            Demand {
                resource: Resource::WorkerSlots,
                amount: Some(2),
            },
            Demand {
                resource: Resource::ElapsedMillis,
                amount: Some(1000),
            },
        ],
    }
}
pub fn specification() -> EvaluationSpecification {
    let arms: Vec<_> = [
        (ExecutionStrategy::SequentialExecution, "sequential"),
        (ExecutionStrategy::GovernedParallelEngineering, "governed"),
    ]
    .into_iter()
    .map(|(strategy, name)| ComparisonArm {
        strategy,
        configuration: configuration(name),
    })
    .collect();
    let trials = arms
        .iter()
        .enumerate()
        .map(|(i, a)| TrialDefinition {
            id: TrialId::new(&format!("trial-{i}")).unwrap(),
            case: case().id,
            strategy: a.strategy,
            repetition: 1,
        })
        .collect();
    EvaluationSpecification::new(
        reference("evaluation-specification"),
        vec![case()],
        arms,
        trials,
        EvaluationLimits::new(4, 16, 32).unwrap(),
    )
    .unwrap()
}
pub fn replace(
    spec: &EvaluationSpecification,
    cases: Vec<EvaluationCase>,
    arms: Vec<ComparisonArm>,
    trials: Vec<TrialDefinition>,
) -> Result<EvaluationSpecification, EvaluationError> {
    EvaluationSpecification::new(spec.reference().clone(), cases, arms, trials, spec.limits())
}
pub fn capture(
    contract: &CandidateBindingContract,
    scope: &str,
    sequence: u64,
) -> CandidateObservation {
    CandidateObservation::new(
        ObservationId::new(scope, sequence).unwrap(),
        contract,
        RepositoryState::new(
            Some(contract.candidate().repository().clone()),
            Head::Detached {
                commit: contract.candidate().commit().clone(),
            },
            vec![],
            ObservationCoverage::GitStatus,
            ObservationConsistency::TwoMatchingSamples,
        )
        .unwrap(),
        Some(
            ComparisonBasis::new(
                "synthetic-evaluation-capture",
                b"same explicit synthetic source".to_vec(),
            )
            .unwrap(),
        ),
    )
}
