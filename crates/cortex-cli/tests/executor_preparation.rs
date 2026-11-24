//! CLI preparation composes canonical guards; these tests open no store or process.
#[path = "../../cortex-evidence/tests/support/program.rs"]
mod fixture;
#[path = "../../cortex-evidence/tests/support/operational.rs"]
mod operational_fixture;

use cortex_cli::program_preparation::*;
use cortex_core::{
    binding::CandidateRequirement,
    parallel::Event,
    program::{context::*, executor::*, operational::*, scheduling::*, *},
};
use cortex_evidence::program::{
    Command, CoordinationKernel, ExecutorInputRequest, KernelError, Operation,
};
use fixture::*;

fn setup() -> (CoordinationKernel, EffectScope) {
    let mut kernel = initialized();
    let scope = operational_fixture::fixture_scope(&mut kernel, "A");
    (kernel, scope)
}

fn input(kernel: &CoordinationKernel, scope: &EffectScope, id: &str) -> Operation {
    operation(
        kernel,
        id,
        Command::PrepareExecutorInput(Box::new(operational_fixture::preparation(scope))),
    )
}

fn prepare_input(
    kernel: &CoordinationKernel,
    scope: &EffectScope,
    operation: Operation,
) -> Result<ExecutorPreparation, PreparationError> {
    prepare_executor_input(
        kernel,
        kernel.revision(),
        &scope.task,
        &scope.candidate,
        operation,
    )
}

fn request(operation: &mut Operation) -> &mut ExecutorInputRequest {
    match &mut operation.command {
        Command::PrepareExecutorInput(request) => request,
        _ => panic!("test operation must prepare executor input"),
    }
}

fn descriptor_with(observations: Vec<CapabilityObservation>) -> ExecutorDescriptor {
    let descriptor = operational_fixture::descriptor();
    ExecutorDescriptor::new(
        descriptor.reference().clone(),
        descriptor.identity().clone(),
        descriptor.transport(),
        observations,
        8,
    )
    .unwrap()
}

fn unchanged(before: &CoordinationKernel, after: &CoordinationKernel) {
    assert_eq!(after.revision(), before.revision());
    assert!(after.operations() == before.operations());
    assert!(after.state().definition() == before.state().definition());
    assert!(after.state().attempts() == before.state().attempts());
    assert!(after.state().ordering_plan() == before.state().ordering_plan());
    assert!(after.packages() == before.packages());
    assert!(after.prepared_inputs() == before.prepared_inputs());
    assert!(after.records() == before.records());
    assert!(after.decisions() == before.decisions());
    assert!(after.fixture_effects() == before.fixture_effects());
    for id in ["A", "B", "C", "D", "E", "F"] {
        assert!(
            after.state().readiness().scope(&task(id)).unwrap()
                == before.state().readiness().scope(&task(id)).unwrap()
        );
    }
}

#[test]
fn exact_package_bytes_and_control_basis_are_prepared_without_mutation() {
    let (kernel, scope) = setup();
    let before = kernel.clone();
    let operation = input(&kernel, &scope, "input-A");
    let result = prepare_input(&kernel, &scope, operation.clone()).unwrap();
    assert_eq!(result.revision(), kernel.revision());
    assert!(result.operation() == &operation);
    assert_eq!(result.input().bytes(), kernel.packages()[0].bytes());
    assert!(result.input().package() == &kernel.packages()[0]);
    assert!(result.input().descriptor() == &operational_fixture::descriptor());
    assert!(result.input().requirements() == &operational_fixture::requirements());
    result
        .input()
        .check(
            kernel.state(),
            &operational_fixture::descriptor(),
            &operational_fixture::requirements(),
            kernel.packages()[0].bytes(),
        )
        .unwrap();
    assert!(kernel.prepared_inputs().is_empty());
    unchanged(&before, &kernel);
}

#[test]
fn scheduling_package_and_executor_input_compose_through_explicit_admission() {
    let mut kernel = initialized();
    operational_fixture::effect(
        &mut kernel,
        "policy",
        EffectEvent::Policy(operational_fixture::policy()),
    );
    let revision = kernel.revision();
    let claim = operation(
        &kernel,
        "claim-A",
        Command::Claim(attempt("attempt-A", "A", "operator")),
    );
    let package = PackageRequest {
        id: PackageId::new("package-A").unwrap(),
        revision: rev(1),
        task: task("A"),
        baseline: rev(1),
        generation: rev(1),
        sources: vec![],
        methods: vec![],
        omissions: "Synthetic caller-owned metadata only".into(),
        deliverable: "Explicit package and transport preparation".into(),
        stop_rules: "No process launch or provider".into(),
    };
    let mut package_operation = operation(&kernel, "package-A", Command::Package(package));
    package_operation.expected = revision + 1;
    let candidate = binding("A", "a", 1).candidate().clone();
    let prepared = cortex_cli::program_preparation::prepare(
        &kernel,
        revision,
        &[Proposal {
            task: task("A"),
            band: 0,
            waiting_rounds: 0,
            risk_rank: 0,
            cost_rank: 0,
            demand: operational_fixture::demand(),
        }],
        &[TaskOperations {
            task: task("A"),
            candidate: candidate.clone(),
            operations: Some([claim, package_operation]),
        }],
    )
    .unwrap();
    assert_eq!(kernel.revision(), revision);
    assert_eq!(prepared.operations().len(), 2);
    assert!(kernel.packages().is_empty());
    for operation in prepared.into_operations() {
        kernel.admit(operation).unwrap();
    }
    let operation = operation(
        &kernel,
        "input-A",
        Command::PrepareExecutorInput(Box::new(ExecutorInputRequest {
            package: PackageId::new("package-A").unwrap(),
            revision: rev(1),
            descriptor: operational_fixture::descriptor(),
            requirements: operational_fixture::requirements(),
            max_bytes: 1_000_000,
        })),
    );
    let prepared = prepare_executor_input(
        &kernel,
        revision + 2,
        &task("A"),
        &candidate,
        operation.clone(),
    )
    .unwrap();
    assert!(kernel.prepared_inputs().is_empty());
    let receipt = kernel.admit(prepared.into_operation()).unwrap();
    assert_eq!(receipt.ordinal, revision + 3);
    assert!(!receipt.duplicate);
    assert_eq!(kernel.prepared_inputs().len(), 1);
    assert_eq!(
        kernel.prepared_inputs()[0].1.bytes(),
        kernel.packages()[0].bytes()
    );
    let repeated = kernel.admit(operation).unwrap();
    assert!(repeated.duplicate);
    assert_eq!(repeated.ordinal, receipt.ordinal);
    assert_eq!(kernel.revision(), revision + 3);
    assert_eq!(kernel.prepared_inputs().len(), 1);
    assert!(kernel.fixture_effects().effects().is_empty());
}

#[test]
fn stale_snapshot_or_operation_predecessor_is_not_rebased() {
    let (kernel, scope) = setup();
    let before = kernel.clone();
    let operation = input(&kernel, &scope, "input-A");
    assert_eq!(
        prepare_executor_input(
            &kernel,
            kernel.revision() - 1,
            &scope.task,
            &scope.candidate,
            operation.clone(),
        )
        .unwrap_err(),
        PreparationError::Core(ProgramError::Stale)
    );
    let mut stale = operation;
    stale.expected -= 1;
    assert_eq!(
        prepare_input(&kernel, &scope, stale).unwrap_err(),
        PreparationError::Core(ProgramError::Stale)
    );
    unchanged(&before, &kernel);
}

#[test]
fn wrong_task_and_changed_candidate_cannot_borrow_a_retained_package() {
    let (kernel, scope) = setup();
    let operation = input(&kernel, &scope, "input-A");
    assert_eq!(
        prepare_executor_input(
            &kernel,
            kernel.revision(),
            &task("D"),
            &scope.candidate,
            operation.clone(),
        )
        .unwrap_err(),
        PreparationError::Mapping
    );
    let changed = CandidateRequirement::new(
        scope.candidate.repository().clone(),
        candidate("b"),
        scope.candidate.worktree(),
    );
    assert_eq!(
        prepare_executor_input(
            &kernel,
            kernel.revision(),
            &scope.task,
            &changed,
            operation.clone(),
        )
        .unwrap_err(),
        PreparationError::Core(ProgramError::Stale)
    );
    assert!(prepare_input(&kernel, &scope, operation).is_ok());
}

#[test]
fn missing_package_or_changed_package_revision_is_refused() {
    let (kernel, scope) = setup();
    for changed_revision in [false, true] {
        let mut operation = input(&kernel, &scope, "input-A");
        if changed_revision {
            request(&mut operation).revision = rev(2);
        } else {
            request(&mut operation).package = PackageId::new("absent-package").unwrap();
        }
        assert_eq!(
            prepare_input(&kernel, &scope, operation).unwrap_err(),
            PreparationError::Core(ProgramError::Missing)
        );
    }
}

#[test]
fn only_executor_input_operations_are_prepared() {
    let (kernel, scope) = setup();
    let before = kernel.clone();
    for command in [
        Command::Claim(attempt("foreign-attempt", "D", "operator")),
        Command::FixtureEffect(Box::new(EffectEvent::Policy(operational_fixture::policy()))),
    ] {
        let operation = operation(&kernel, "unsupported", command);
        assert_eq!(
            prepare_input(&kernel, &scope, operation).unwrap_err(),
            PreparationError::UnsupportedCommand
        );
    }
    unchanged(&before, &kernel);
}

#[test]
fn unsupported_unknown_missing_and_wrong_class_controls_retain_exact_holds() {
    let (kernel, scope) = setup();
    let before = kernel.clone();
    for (support, kind, absent, expected) in [
        (
            Support::Unsupported,
            ObservationKind::ControlledFixture,
            false,
            HoldReason::Unsupported,
        ),
        (
            Support::Unknown,
            ObservationKind::ControlledFixture,
            false,
            HoldReason::Unknown,
        ),
        (
            Support::Supported,
            ObservationKind::ControlledFixture,
            true,
            HoldReason::Missing,
        ),
        (
            Support::Supported,
            ObservationKind::Advertised,
            false,
            HoldReason::ObservationKind {
                required: ObservationKind::ControlledFixture,
                actual: ObservationKind::Advertised,
            },
        ),
    ] {
        let mut observations = operational_fixture::descriptor().observations().to_vec();
        if absent {
            observations.remove(1);
        } else {
            observations[1].support = support;
            observations[1].kind = kind;
        }
        let mut operation = input(&kernel, &scope, "input-A");
        request(&mut operation).descriptor = descriptor_with(observations);
        assert_eq!(
            prepare_input(&kernel, &scope, operation).unwrap_err(),
            PreparationError::Kernel(KernelError::Executor(ExecutorError::Held(vec![
                ControlDecision {
                    control: Control::StandardInput,
                    held: vec![]
                },
                ControlDecision {
                    control: Control::StructuredOutput,
                    held: vec![expected]
                },
            ])))
        );
    }
    unchanged(&before, &kernel);
}

#[test]
fn fixture_support_is_not_upgraded_to_host_qualification() {
    let (kernel, scope) = setup();
    let mut operation = input(&kernel, &scope, "input-A");
    request(&mut operation).requirements = Requirements::new(
        source("host-transport-profile"),
        [Control::StandardInput, Control::StructuredOutput]
            .into_iter()
            .map(|control| Requirement {
                control,
                kind: ObservationKind::HostQualificationReport,
            })
            .collect(),
        2,
    )
    .unwrap();
    let PreparationError::Kernel(KernelError::Executor(ExecutorError::Held(holds))) =
        prepare_input(&kernel, &scope, operation).unwrap_err()
    else {
        panic!("fixture evidence must remain held for the host profile");
    };
    assert_eq!(holds.len(), 2);
    for hold in holds {
        assert_eq!(
            hold.held,
            [HoldReason::ObservationKind {
                required: ObservationKind::HostQualificationReport,
                actual: ObservationKind::ControlledFixture,
            }]
        );
    }
}

#[test]
fn plan_grant_and_task_scope_are_required_by_authoritative_guards() {
    let (kernel, scope) = setup();
    for missing_permission in [false, true] {
        let mut operation = input(&kernel, &scope, "input-A");
        operation.grant.id = GrantId::new("limited-grant").unwrap();
        if missing_permission {
            operation.grant.permissions = vec![CoordinationPermission::Claim];
        } else {
            operation.grant.tasks = vec![task("D")];
        }
        assert_eq!(
            prepare_input(&kernel, &scope, operation).unwrap_err(),
            PreparationError::Kernel(KernelError::Core(ProgramError::Authority))
        );
    }
}

#[test]
fn admitted_operation_identity_cannot_be_reused_for_new_preparation() {
    let (mut kernel, scope) = setup();
    let operation = input(&kernel, &scope, "input-A");
    kernel.admit(operation).unwrap();
    let before = kernel.clone();
    let reused = input(&kernel, &scope, "input-A");
    assert_eq!(
        prepare_input(&kernel, &scope, reused).unwrap_err(),
        PreparationError::Core(ProgramError::IdentityConflict)
    );
    unchanged(&before, &kernel);
}

#[test]
fn descriptor_and_profile_references_cannot_alias_changed_content() {
    let (mut kernel, scope) = setup();
    kernel.admit(input(&kernel, &scope, "input-A")).unwrap();
    let before = kernel.clone();
    for changed_descriptor in [true, false] {
        let mut operation = input(&kernel, &scope, "input-A-next");
        if changed_descriptor {
            let mut observations = operational_fixture::descriptor().observations().to_vec();
            observations[0].basis = source("changed-observation");
            request(&mut operation).descriptor = descriptor_with(observations);
        } else {
            let requirements = operational_fixture::requirements();
            let mut required = requirements.required().to_vec();
            required[0].kind = ObservationKind::Advertised;
            request(&mut operation).requirements =
                Requirements::new(requirements.reference().clone(), required, 2).unwrap();
        }
        assert_eq!(
            prepare_input(&kernel, &scope, operation).unwrap_err(),
            PreparationError::Kernel(KernelError::Core(ProgramError::IdentityConflict))
        );
    }
    unchanged(&before, &kernel);
}

#[test]
fn package_input_limits_are_not_enlarged_or_truncated() {
    let (kernel, scope) = setup();
    for maximum in [0, kernel.state().definition().limits().bytes + 1] {
        let mut operation = input(&kernel, &scope, "input-A");
        request(&mut operation).max_bytes = maximum;
        assert_eq!(
            prepare_input(&kernel, &scope, operation).unwrap_err(),
            PreparationError::Core(ProgramError::Capacity)
        );
    }
    let mut operation = input(&kernel, &scope, "input-A");
    request(&mut operation).max_bytes = kernel.packages()[0].bytes().len() - 1;
    assert_eq!(
        prepare_input(&kernel, &scope, operation).unwrap_err(),
        PreparationError::Kernel(KernelError::Executor(ExecutorError::Capacity))
    );
    let mut operation = input(&kernel, &scope, "input-A");
    request(&mut operation).max_bytes = kernel.packages()[0].bytes().len();
    let prepared = prepare_input(&kernel, &scope, operation).unwrap();
    assert_eq!(prepared.input().bytes(), kernel.packages()[0].bytes());
}

#[test]
fn changed_package_context_and_later_admission_remain_stale() {
    let (mut kernel, scope) = setup();
    let prepared = prepare_input(&kernel, &scope, input(&kernel, &scope, "input-A")).unwrap();
    let notification = notify(
        kernel.state(),
        "no-capacity",
        Event::Capacity {
            available: 0,
            source: source("new-capacity"),
        },
    );
    admit(&mut kernel, "no-capacity", Command::Notify(notification));
    let before = kernel.clone();
    assert_eq!(
        kernel.admit(prepared.into_operation()).unwrap_err(),
        KernelError::Core(ProgramError::Stale)
    );
    assert_eq!(
        prepare_input(&kernel, &scope, input(&kernel, &scope, "new-input-A")).unwrap_err(),
        PreparationError::Kernel(KernelError::Executor(ExecutorError::Program(
            ProgramError::Stale
        )))
    );
    unchanged(&before, &kernel);
}

#[test]
fn preparation_preserves_unknown_effect_and_retained_resource_accounting() {
    let (mut kernel, scope) = operational_fixture::fixture_kernel();
    let intent = operational_fixture::intent(&scope, "unknown-effect");
    operational_fixture::effect(
        &mut kernel,
        "reserve",
        EffectEvent::Reserve {
            intent: intent.clone(),
            at: 2,
        },
    );
    operational_fixture::effect(
        &mut kernel,
        "dispatch",
        EffectEvent::Dispatch {
            effect: intent.id.clone(),
            scope: scope.clone(),
            at: 3,
            basis: source("synthetic-dispatch"),
        },
    );
    operational_fixture::effect(
        &mut kernel,
        "unknown",
        EffectEvent::Unknown {
            effect: intent.id,
            scope: scope.clone(),
            at: 4,
            basis: source("synthetic-unknown"),
        },
    );
    let before = kernel.clone();
    assert_eq!(
        kernel
            .fixture_effects()
            .accounted(Resource::OutputBytes)
            .unwrap(),
        30
    );
    let prepared = prepare_input(&kernel, &scope, input(&kernel, &scope, "input-A")).unwrap();
    assert_eq!(prepared.input().bytes(), kernel.packages()[0].bytes());
    unchanged(&before, &kernel);
    assert_eq!(kernel.fixture_effects().effects().len(), 1);
    assert_eq!(
        kernel.fixture_effects().effects()[0].status,
        EffectStatus::Unknown
    );
    assert_eq!(
        kernel
            .fixture_effects()
            .accounted(Resource::OutputBytes)
            .unwrap(),
        30
    );
}

#[test]
fn executor_preparation_refuses_a_preparable_prefix_during_reconstruction() {
    let (kernel, scope) = setup();
    let mut operations = kernel.operations().iter().cloned();
    let mut prefixes_with_package = 0;
    let rebuilt = CoordinationKernel::new(kernel.initial().clone())
        .unwrap()
        .reconstruct::<KernelError>(|partial| {
            if !partial.packages().is_empty() {
                prefixes_with_package += 1;
                let command = input(partial, &scope, "replay-input");
                let result = prepare_input(partial, &scope, command);
                assert_eq!(
                    result.unwrap_err(),
                    PreparationError::Kernel(KernelError::Core(ProgramError::EffectUnavailable)),
                );
                assert!(partial.prepared_inputs().is_empty());
            }
            Ok(operations.next())
        })
        .unwrap();
    assert!(prefixes_with_package > 0);
    assert!(rebuilt.operations() == kernel.operations());
    let operation = input(&rebuilt, &scope, "current-input");
    let prepared = prepare_input(&rebuilt, &scope, operation).unwrap();
    assert_eq!(prepared.input().bytes(), rebuilt.packages()[0].bytes());
    assert!(rebuilt.prepared_inputs().is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn codex_cli_lowers_exact_input_but_retains_the_live_and_executable_holds() {
    use cortex_cli::program_execution::{CodexCliHold, inspect_codex_cli_invocation};
    let (kernel, scope) = setup();
    let identity = ExecutorIdentity::new(
        "codex-cli",
        "0.167.0",
        source("selected-codex-elf"),
        source("selected-host"),
    )
    .unwrap();
    let mut operation = input(&kernel, &scope, "codex-shape");
    request(&mut operation).descriptor = ExecutorDescriptor::new(
        source("codex-cli-synthetic-transport-test"),
        identity.clone(),
        Transport::StandardInputJsonLines,
        operational_fixture::descriptor()
            .observations()
            .iter()
            .cloned(),
        8,
    )
    .unwrap();
    let prepared = prepare_input(&kernel, &scope, operation).unwrap();
    let lower =
        |bytes| inspect_codex_cli_invocation(kernel.state(), prepared.input(), &identity, bytes);
    let shape = lower(320_575_976).unwrap();
    assert_eq!(shape.live_hold(), CodexCliHold::LiveProviderUnavailable);
    assert_eq!(
        shape.executable_hold(),
        Some(&CodexCliHold::ExecutableBytes)
    );
    assert_eq!(shape.executable(), &source("selected-codex-elf"));
    assert_eq!(shape.workspace(), scope.candidate.repository().root());
    assert_eq!(shape.stdin(), kernel.packages()[0].bytes());
    assert_eq!(
        shape.args(),
        [
            "exec",
            "--json",
            "--ephemeral",
            "--ignore-user-config",
            "--color",
            "never",
            "-"
        ]
    );

    let shape = lower(900_000).unwrap();
    assert!(shape.executable_hold().is_none());
    assert_eq!(shape.live_hold(), CodexCliHold::LiveProviderUnavailable);
    let changed_identity = ExecutorIdentity::new(
        "codex-cli",
        "0.167.0",
        source("different-executable"),
        identity.host().clone(),
    )
    .unwrap();
    assert!(matches!(
        inspect_codex_cli_invocation(kernel.state(), prepared.input(), &changed_identity, 900_000),
        Err(CodexCliHold::Descriptor)
    ));
}
