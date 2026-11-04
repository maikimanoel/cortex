#![allow(dead_code)]
pub use super::fixture::*;
use cortex_core::program::{context::*, executor::*, operational::*, scheduling::*, *};
use cortex_evidence::program::*;

pub fn policy() -> Policy {
    Policy {
        reference: source("fixture-policy-v1"),
        program: definition().id().clone(),
        baseline: rev(1),
        mode: EffectMode::ControlledFixture,
        host: source("synthetic-host"),
        privacy: source("synthetic-only"),
        capacities: vec![
            Capacity {
                resource: Resource::Workers,
                available: Some(3),
                verification_reserve: 1,
                declared_enforceable: true,
            },
            Capacity {
                resource: Resource::OutputBytes,
                available: Some(100),
                verification_reserve: 20,
                declared_enforceable: true,
            },
        ],
        cumulative: vec![Resource::OutputBytes],
        max_effects: 8,
        max_retries: 1,
    }
}
pub fn demand() -> Vec<Demand> {
    vec![
        Demand {
            resource: Resource::Workers,
            amount: Some(1),
        },
        Demand {
            resource: Resource::OutputBytes,
            amount: Some(30),
        },
    ]
}
pub fn effect(k: &mut CoordinationKernel, id: &str, e: EffectEvent) {
    admit(k, id, Command::FixtureEffect(Box::new(e)));
}
pub fn fixture_scope(k: &mut CoordinationKernel, mission: &str) -> EffectScope {
    let a = attempt(&format!("attempt-{mission}"), mission, "operator");
    admit(k, &format!("claim-{mission}"), Command::Claim(a.clone()));
    let request = PackageRequest {
        id: PackageId::new(&format!("package-{mission}")).unwrap(),
        revision: rev(1),
        mission: task(mission),
        baseline: rev(1),
        generation: rev(1),
        sources: vec![],
        methods: vec![],
        omissions: "Synthetic fixture, no acquired sources".into(),
        deliverable: "Controlled fixture observation only".into(),
        stop_rules: "No executor; retain uncertainty".into(),
    };
    admit(
        k,
        &format!("package-{mission}"),
        Command::Package(request.clone()),
    );
    EffectScope {
        program: k.state().definition().id().clone(),
        baseline: rev(1),
        mission: task(mission),
        candidate: k
            .state()
            .readiness()
            .scope(&task(mission))
            .unwrap()
            .context()
            .contract()
            .candidate()
            .clone(),
        attempt: a.id,
        generation: rev(1),
        package: request.id,
        package_revision: request.revision,
    }
}
pub fn proposed_grant(scope: &EffectScope) -> OperationalGrant {
    OperationalGrant {
        id: GrantId::new(&format!("fixture-grant-{}", scope.mission.as_str())).unwrap(),
        principal: ActorId::new("operator").unwrap(),
        policy: policy().reference,
        scope: scope.clone(),
        not_before: 1,
        expires_before: 100,
        ceilings: demand(),
        basis: source("fixture-grant-assertion"),
    }
}
pub fn intent(scope: &EffectScope, id: &str) -> EffectIntent {
    EffectIntent {
        id: EffectId::new(id).unwrap(),
        grant: proposed_grant(scope).id,
        scope: scope.clone(),
        demand: demand(),
        retry_of: None,
    }
}
pub fn fixture_kernel() -> (CoordinationKernel, EffectScope) {
    let mut k = initialized();
    let scope = fixture_scope(&mut k, "A");
    effect(&mut k, "fixture-policy", EffectEvent::Policy(policy()));
    effect(
        &mut k,
        "fixture-grant",
        EffectEvent::Grant(proposed_grant(&scope)),
    );
    (k, scope)
}
pub fn descriptor() -> ExecutorDescriptor {
    ExecutorDescriptor::new(
        source("descriptor"),
        ExecutorIdentity::new(
            "fixture-executor",
            "1",
            source("fixture-binary"),
            source("synthetic-host"),
        )
        .unwrap(),
        Transport::StandardInputJsonLines,
        [Control::StandardInput, Control::StructuredOutput]
            .into_iter()
            .map(|control| CapabilityObservation {
                control,
                kind: ObservationKind::ControlledFixture,
                support: Support::Supported,
                basis: source("fixture-observation"),
            })
            .collect(),
        2,
    )
    .unwrap()
}
pub fn requirements() -> Requirements {
    Requirements::new(
        source("fixture-transport-profile"),
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
pub fn preparation(scope: &EffectScope) -> ExecutorInputRequest {
    ExecutorInputRequest {
        package: scope.package.clone(),
        revision: scope.package_revision,
        descriptor: descriptor(),
        requirements: requirements(),
        max_bytes: 1_000_000,
    }
}
