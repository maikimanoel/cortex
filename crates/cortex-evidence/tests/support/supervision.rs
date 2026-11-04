#![allow(dead_code)]
use super::fixture;
pub use super::fixture::*;
#[path = "../../../cortex-core/tests/support/supervision.rs"]
mod core_supervision;
// Shared consumers exercise different fixture operations.
#[allow(unused_imports)]
pub use core_supervision::{demand, intent, policy, proposed_grant, usage};
use cortex_core::{
    binding::*,
    parallel::*,
    program::{context::*, executor::*, operational::*, supervision::*, *},
};
use cortex_evidence::program::*;
use std::path::Path;

pub fn definition_at(root: Option<&Path>) -> ProgramDefinition {
    let original = definition();
    let Some(root) = root else { return original };
    with_contracts(&original, |contract| {
        if contract.task().id() != &task("A") {
            return contract.clone();
        }
        CandidateBindingContract::new(
            contract.task().clone(),
            contract.revision(),
            CandidateRequirement::new(
                RepositoryIdentity::new(root.into(), root.join(".git"), root.join(".git")).unwrap(),
                contract.candidate().commit().clone(),
                contract.candidate().worktree(),
            ),
        )
    })
}
pub fn descriptor() -> ExecutorDescriptor {
    ExecutorDescriptor::new(
        source("supervised-descriptor"),
        ExecutorIdentity::new(
            "trusted-native-fixture",
            "1",
            source("fixture-binary"),
            policy().host,
        )
        .unwrap(),
        Transport::StandardInputJsonLines,
        [Control::StandardInput, Control::StructuredOutput]
            .into_iter()
            .map(|control| CapabilityObservation {
                control,
                kind: ObservationKind::ControlledFixture,
                support: Support::Supported,
                basis: source("explicit-fixture-capture"),
            })
            .collect(),
        2,
    )
    .unwrap()
}
pub fn requirements() -> Requirements {
    Requirements::new(
        source("fixture-profile"),
        [Control::StandardInput, Control::StructuredOutput]
            .into_iter()
            .map(|control| Requirement {
                control,
                kind: ObservationKind::ControlledFixture,
            })
            .collect(),
        2,
    )
    .unwrap()
}
pub fn effect(k: &mut CoordinationKernel, id: &str, event: EffectEvent) {
    admit(k, id, Command::SupervisedEffect(Box::new(event)));
}
/// A current explicit trusted-fixture program and prepared stdin; no effect has yet
/// been reserved or dispatched. The optional root supports real temporary fixtures.
pub fn configured_kernel(root: Option<&Path>) -> (CoordinationKernel, EffectScope, ProcessSpec) {
    let mut k = CoordinationKernel::new_supervised_fixture(definition_at(root)).unwrap();
    for m in ["A", "B", "C", "D", "E", "F"] {
        let n = notify(
            k.state(),
            &format!("ready-{m}"),
            Event::Preconditions {
                scope: k.state().readiness().scope(&task(m)).unwrap(),
                ready: true,
                source: source("fixture-prerequisites"),
            },
        );
        admit(&mut k, &format!("ready-{m}"), Command::Notify(n));
    }
    let n = notify(
        k.state(),
        "capacity",
        Event::Capacity {
            available: 2,
            source: source("metadata-capacity"),
        },
    );
    admit(&mut k, "capacity", Command::Notify(n));
    let a = attempt("attempt-A", "A", "operator");
    admit(&mut k, "claim-A", Command::Claim(a.clone()));
    let request=PackageRequest{id:PackageId::new("package-A").unwrap(),revision:rev(1),mission:task("A"),baseline:rev(1),generation:rev(1),sources:vec![],methods:vec![],
        omissions:"Provider-free deterministic development fixture; no acquired private project content".into(),
        deliverable:"Bounded JSON-lines fixture output, with no engineering acceptance inference".into(),
        stop_rules:"Only the explicitly authorized trusted local development fixture; no provider, shell, network, sandbox claim or protected effect. Stop on timeout, cancellation, revoked or uncertain authority.".into()};
    admit(&mut k, "package-A", Command::Package(request.clone()));
    let scope = EffectScope {
        program: k.state().definition().id().clone(),
        baseline: rev(1),
        mission: task("A"),
        candidate: k
            .state()
            .readiness()
            .scope(&task("A"))
            .unwrap()
            .context()
            .contract()
            .candidate()
            .clone(),
        attempt: a.id,
        generation: rev(1),
        package: request.id,
        package_revision: request.revision,
    };
    admit(
        &mut k,
        "prepared",
        Command::PrepareExecutorInput(Box::new(ExecutorInputRequest {
            package: scope.package.clone(),
            revision: scope.package_revision,
            descriptor: descriptor(),
            requirements: requirements(),
            max_bytes: 1_000_000,
        })),
    );
    effect(&mut k, "supervised-policy", EffectEvent::Policy(policy()));
    effect(
        &mut k,
        "supervised-grant",
        EffectEvent::Grant(proposed_grant(&scope)),
    );
    let mut spec = core_supervision::spec(&scope);
    spec.input_len = k.prepared_inputs()[0].1.bytes().len() as u64;
    (k, scope, spec)
}
pub fn prepare(k: &mut CoordinationKernel, scope: &EffectScope, spec: &ProcessSpec) {
    effect(
        k,
        "reserve",
        EffectEvent::Reserve {
            intent: intent(scope),
            at: 1,
        },
    );
    effect(
        k,
        "process-spec",
        EffectEvent::PrepareProcess {
            effect: intent(scope).id,
            scope: scope.clone(),
            at: 2,
            spec: Box::new(spec.clone()),
        },
    );
}
pub fn prepared_kernel(root: Option<&Path>) -> (CoordinationKernel, EffectScope, ProcessSpec) {
    let (mut k, scope, spec) = configured_kernel(root);
    prepare(&mut k, &scope, &spec);
    (k, scope, spec)
}
pub fn dispatch_operation(k: &CoordinationKernel, scope: &EffectScope) -> Operation {
    operation(
        k,
        "dispatch",
        Command::SupervisedEffect(Box::new(EffectEvent::Dispatch {
            effect: intent(scope).id,
            scope: scope.clone(),
            at: 3,
            basis: source("durable-prelaunch"),
        })),
    )
}
pub fn observation(
    spec: &ProcessSpec,
    n: u64,
    completion: ProcessCompletion,
    quiescence: Quiescence,
) -> ProcessObservation {
    let process = if completion == ProcessCompletion::StartFailed {
        None
    } else {
        Some(ProcessIdentity {
            launch: spec.reference.clone(),
            pid: 123,
            start_ticks: 456,
        })
    };
    let terminal = !matches!(
        completion,
        ProcessCompletion::Spawned | ProcessCompletion::StartFailed | ProcessCompletion::Unknown
    );
    ProcessObservation {
        id: ObservationId::new("process-fixture", n).unwrap(),
        spec: spec.reference.clone(),
        process,
        completion,
        quiescence,
        input_bytes: if terminal { spec.input_len } else { 0 },
        stdout_bytes: if terminal { 3 } else { 0 },
        stderr_bytes: 0,
        structured_records: if terminal { 1 } else { 0 },
        malformed_output: false,
        elapsed_ms: n,
        basis: source("captured-fixture-observation"),
    }
}
