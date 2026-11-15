//! Core contract counterexamples and valid bounded neighbors.
#[path = "support/evaluation.rs"]
mod fixture;
use cortex_core::{
    binding::*,
    evaluation::*,
    program::{memory::*, scheduling::*},
};
use fixture::*;

#[test]
fn duplicate_case_trial_arm_and_configuration_aliases_are_rejected() {
    let spec = specification();
    assert_eq!(
        replace(
            &spec,
            vec![case(), case()],
            spec.arms().to_vec(),
            spec.trials().to_vec()
        ),
        Err(EvaluationError::Population)
    );
    let mut trials = spec.trials().to_vec();
    trials.push(trials[0].clone());
    assert_eq!(
        replace(&spec, spec.cases().to_vec(), spec.arms().to_vec(), trials),
        Err(EvaluationError::Population)
    );
    let mut arms = spec.arms().to_vec();
    arms[1].strategy = arms[0].strategy;
    assert_eq!(
        replace(&spec, spec.cases().to_vec(), arms, spec.trials().to_vec()),
        Err(EvaluationError::Population)
    );
    let mut arms = spec.arms().to_vec();
    arms[1].configuration.reference = arms[0].configuration.reference.clone();
    assert_eq!(
        replace(&spec, spec.cases().to_vec(), arms, spec.trials().to_vec()),
        Err(EvaluationError::Population)
    );
    assert!(
        replace(
            &spec,
            spec.cases().to_vec(),
            spec.arms().to_vec(),
            spec.trials().to_vec()
        )
        .is_ok()
    );
}
#[test]
fn exact_case_source_and_membership_cannot_be_replaced_by_matching_names() {
    let spec = specification();
    let mut cases = spec.cases().to_vec();
    cases[0].contract = CandidateBindingContract::new(
        cases[0].contract.task().clone(),
        rev(1),
        CandidateRequirement::new(
            source().candidate().repository().clone(),
            candidate("c"),
            WorktreeRequirement::Clean,
        ),
    );
    assert_eq!(
        replace(&spec, cases, spec.arms().to_vec(), spec.trials().to_vec()),
        Err(EvaluationError::Population)
    );
    let mut trials = spec.trials().to_vec();
    trials[0].case = CaseId::new("absent").unwrap();
    assert_eq!(
        replace(&spec, spec.cases().to_vec(), spec.arms().to_vec(), trials),
        Err(EvaluationError::Population)
    );
    let mut trials = spec.trials().to_vec();
    trials[0].repetition = 0;
    assert!(replace(&spec, spec.cases().to_vec(), spec.arms().to_vec(), trials).is_err());
}
#[test]
fn clean_commit_and_tree_identities_are_distinct_and_hash_width_must_match() {
    assert!(SourceTreeId::new("abc").is_err());
    assert!(SourceTreeId::new(&"A".repeat(40)).is_err());
    assert!(
        SourceIdentity::new(
            source().candidate().clone(),
            SourceTreeId::new(&"b".repeat(64)).unwrap()
        )
        .is_err()
    );
    let dirty = CandidateRequirement::new(
        source().candidate().repository().clone(),
        candidate("a"),
        WorktreeRequirement::AllowChanges,
    );
    assert!(SourceIdentity::new(dirty, source().tree().clone()).is_err());
    assert_ne!(
        source().candidate().commit().as_str(),
        source().tree().as_str()
    );
}
#[test]
fn finite_bounds_and_resource_profiles_reject_invalid_neighbors() {
    assert!(EvaluationLimits::new(0, 1, 1).is_err());
    assert!(EvaluationLimits::new(33, 1, 1).is_err());
    assert!(EvaluationLimits::new(1, 257, 1).is_err());
    assert!(EvaluationLimits::new(1, 1, 257).is_err());
    let spec = specification();
    let mut arms = spec.arms().to_vec();
    arms[0].configuration.envelope.push(Demand {
        resource: Resource::UnspecifiedMemoryBytes,
        amount: Some(1024),
    });
    assert!(replace(&spec, spec.cases().to_vec(), arms, spec.trials().to_vec()).is_err());
    let mut arms = spec.arms().to_vec();
    let duplicate = arms[0].configuration.envelope[0].clone();
    arms[0].configuration.envelope.push(duplicate);
    assert!(replace(&spec, spec.cases().to_vec(), arms, spec.trials().to_vec()).is_err());
    let mut arms = spec.arms().to_vec();
    arms[0].configuration.profile = ExecutionProfile::ProviderFreeFixture;
    arms[0].configuration.model = ModelConfiguration::Configured(reference("unavailable-model"));
    assert!(replace(&spec, spec.cases().to_vec(), arms, spec.trials().to_vec()).is_err());
    let memory = MemoryResource::new(
        MemoryBasis::ProcessResident,
        MemoryScope::Process(MemoryProcessIdentity {
            host: "synthetic-host".into(),
            boot: "synthetic-boot".into(),
            pid: 1,
            start_ticks: 1,
        }),
    )
    .unwrap();
    let mut arms = spec.arms().to_vec();
    arms[0].configuration.envelope.push(Demand {
        resource: Resource::MemoryBytes(Box::new(memory)),
        amount: Some(1024),
    });
    assert!(replace(&spec, spec.cases().to_vec(), arms, spec.trials().to_vec()).is_err());
    assert!(EvaluationLimits::new(32, 256, 256).is_ok());
}
