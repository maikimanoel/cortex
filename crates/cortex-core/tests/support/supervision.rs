#![allow(dead_code)]
pub use super::fixture::*;
use cortex_core::{
    binding::ObservationId,
    program::{operational::*, scheduling::*, supervision::*, *},
};

pub fn demand() -> Vec<Demand> {
    [
        (Resource::Workers, 1),
        (Resource::TimeMillis, 1200),
        (Resource::OutputBytes, 1000),
    ]
    .into_iter()
    .map(|(resource, amount)| Demand {
        resource,
        amount: Some(amount),
    })
    .collect()
}
pub fn policy() -> Policy {
    Policy {
        reference: source("supervised-policy"),
        program: definition().id().clone(),
        baseline: rev(1),
        mode: EffectMode::SupervisedFixture,
        host: source("trusted-fixture-host"),
        privacy: source("private-fixture"),
        capacities: vec![
            Capacity {
                resource: Resource::Workers,
                available: Some(2),
                verification_reserve: 1,
                declared_enforceable: true,
            },
            Capacity {
                resource: Resource::TimeMillis,
                available: Some(5000),
                verification_reserve: 1000,
                declared_enforceable: true,
            },
            Capacity {
                resource: Resource::OutputBytes,
                available: Some(5000),
                verification_reserve: 1000,
                declared_enforceable: true,
            },
        ],
        cumulative: vec![Resource::TimeMillis, Resource::OutputBytes],
        max_effects: 8,
        max_retries: 1,
    }
}
pub fn scope(state: &ProgramState) -> EffectScope {
    EffectScope {
        program: state.definition().id().clone(),
        baseline: rev(1),
        mission: task("A"),
        candidate: state
            .readiness()
            .scope(&task("A"))
            .unwrap()
            .context()
            .contract()
            .candidate()
            .clone(),
        attempt: AttemptId::new("attempt-A").unwrap(),
        generation: rev(1),
        package: PackageId::new("package-A").unwrap(),
        package_revision: rev(1),
    }
}
pub fn proposed_grant(scope: &EffectScope) -> OperationalGrant {
    OperationalGrant {
        id: GrantId::new("supervised-grant").unwrap(),
        principal: ActorId::new("operator").unwrap(),
        policy: policy().reference,
        scope: scope.clone(),
        not_before: 1,
        expires_before: 5000,
        ceilings: demand(),
        basis: source("explicit-fixture-approval"),
    }
}
pub fn intent(scope: &EffectScope) -> EffectIntent {
    EffectIntent {
        id: EffectId::new("effect").unwrap(),
        grant: proposed_grant(scope).id,
        scope: scope.clone(),
        demand: demand(),
        retry_of: None,
    }
}
pub fn spec(scope: &EffectScope) -> ProcessSpec {
    ProcessSpec {
        reference: source("launch-spec"),
        executable: "/explicit/native-fixture".into(),
        executable_identity: source("fixture-binary"),
        cwd: scope.candidate.repository().root().into(),
        args: vec!["--fixture".into()],
        prepared_input: OperationId::new("prepared").unwrap(),
        input_len: 10,
        host: policy().host,
        profile: source("fixture-profile"),
        privacy: policy().privacy,
        approval: proposed_grant(scope).basis,
        limits: ProcessLimits {
            wall_ms: 1000,
            cleanup_ms: 200,
            input_bytes: 10000,
            output_bytes: 1000,
            frame_bytes: 100,
            authority_poll_ms: 10,
            executable_bytes: 1_000_000,
            arguments: 2,
            argument_bytes: 50,
            transport: None,
        },
        containment: None,
    }
}
/// Explicit synthetic test contract; these paths are not a discovered host grant.
pub fn owned_scope() -> OwnedScopeSpec {
    OwnedScopeSpec {
        reference: source("owned-scope-spec"),
        unit: "cortex-qualification-fixture.scope".into(),
        delegated_root: "/owned/cgroup".into(),
        runtime_dir: "/owned/runtime".into(),
        launcher: "/usr/bin/systemd-run".into(),
        launcher_identity: source("scope-launcher"),
        manager: "/usr/bin/systemctl".into(),
        manager_identity: source("scope-manager"),
        controls: ScopeControls {
            cpu_quota_us: 100_000,
            cpu_period_us: 100_000,
            memory_bytes: 536_870_912,
            swap_bytes: 0,
            tasks_max: 8,
        },
        cpu_time_ms: 2000,
        cancellation_grace_ms: 500,
        escalation_ms: 500,
        isolation: ScopeIsolation::ScopeOnlyInertFixture,
    }
}
pub fn scope_demand() -> Vec<Demand> {
    [
        (Resource::Workers, 1),
        (Resource::TimeMillis, 7000),
        (Resource::OutputBytes, 81920),
        (Resource::CpuMillis, 1000),
        (Resource::RamBytes, 536_870_912),
    ]
    .into_iter()
    .map(|(resource, amount)| Demand {
        resource,
        amount: Some(amount),
    })
    .collect()
}
pub fn scope_policy() -> Policy {
    let mut p = policy();
    p.reference = source("scope-policy");
    p.capacities = [
        (Resource::Workers, 2, 1),
        (Resource::TimeMillis, 21_000, 7000),
        (Resource::OutputBytes, 245_760, 81_920),
        (Resource::CpuMillis, 3000, 2000),
        (Resource::RamBytes, 4_831_838_208, 4_294_967_296),
    ]
    .into_iter()
    .map(|(resource, available, verification_reserve)| Capacity {
        resource,
        available: Some(available),
        verification_reserve,
        declared_enforceable: true,
    })
    .collect();
    p.max_retries = 0;
    p
}
pub fn scope_grant(s: &EffectScope) -> OperationalGrant {
    let mut g = proposed_grant(s);
    g.policy = scope_policy().reference;
    g.ceilings = scope_demand();
    g.expires_before = 10_000;
    g
}
pub fn scope_intent(s: &EffectScope) -> EffectIntent {
    let mut i = intent(s);
    i.demand = scope_demand();
    i
}
pub fn contained_spec(s: &EffectScope) -> ProcessSpec {
    let mut p = spec(s);
    p.limits.wall_ms = 5000;
    p.limits.cleanup_ms = 1000;
    p.limits.input_bytes = 16_384;
    p.limits.output_bytes = 81_920;
    p.limits.frame_bytes = 4096;
    p.limits.transport = Some(TransportLimits {
        startup_ms: 1000,
        idle_ms: 1000,
        stdout_bytes: 65_536,
        stderr_bytes: 16_384,
        records: 128,
        buffer_bytes: 131_072,
    });
    p.containment = Some(owned_scope());
    p
}
pub fn scope_observation(s: &OwnedScopeSpec, populated: Option<bool>) -> ScopeObservation {
    ScopeObservation {
        identity: ScopeIdentity {
            spec: s.reference.clone(),
            unit: s.unit.clone(),
            invocation_id: "123456789abcdef0123456789abcdef0".into(),
            cgroup: s.delegated_root.join(&s.unit),
            device: 1,
            inode: 2,
        },
        controls: Some(s.controls.clone()),
        membership_verified: true,
        helper_quiescent: true,
        lifecycle_verified: true,
        populated,
        cpu_usage_us: Some(10_000),
        memory_peak_bytes: Some(1_048_576),
        tasks_peak: Some(1),
    }
}
pub fn setup() -> (ProgramState, OperationalProjection, EffectScope) {
    let mut state = ProgramState::new(definition()).unwrap();
    initialize(&mut state);
    state
        .claim(&grant(1, "operator"), attempt("attempt-A", "A", "operator"))
        .unwrap();
    let scope = scope(&state);
    let mut p = OperationalProjection::new_supervised_fixture();
    for e in [
        EffectEvent::Policy(policy()),
        EffectEvent::Grant(proposed_grant(&scope)),
        EffectEvent::Reserve {
            intent: intent(&scope),
            at: 1,
        },
    ] {
        p.apply(&state, e).unwrap();
    }
    (state, p, scope)
}
pub fn prepare(state: &ProgramState, p: &mut OperationalProjection, scope: &EffectScope) {
    p.apply(
        state,
        EffectEvent::PrepareProcess {
            effect: intent(scope).id,
            scope: scope.clone(),
            at: 2,
            spec: Box::new(spec(scope)),
        },
    )
    .unwrap();
}
pub fn dispatch(state: &ProgramState, p: &mut OperationalProjection, scope: &EffectScope) {
    p.apply(
        state,
        EffectEvent::Dispatch {
            effect: intent(scope).id,
            scope: scope.clone(),
            at: 3,
            basis: source("durable-prelaunch"),
        },
    )
    .unwrap();
}
pub fn observation(
    scope: &EffectScope,
    n: u64,
    completion: ProcessCompletion,
    quiescence: Quiescence,
) -> ProcessObservation {
    let process = if completion == ProcessCompletion::StartFailed {
        None
    } else {
        Some(ProcessIdentity {
            launch: spec(scope).reference.clone(),
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
        spec: Box::new(spec(scope)).reference,
        process,
        completion,
        quiescence,
        input_bytes: if terminal { 10 } else { 0 },
        stdout_bytes: if terminal { 3 } else { 0 },
        stderr_bytes: 0,
        structured_records: if terminal { 1 } else { 0 },
        malformed_output: false,
        elapsed_ms: n,
        basis: source("captured-fixture-observation"),
        containment: None,
        unresolved_scope_helpers: vec![],
    }
}
pub fn observed(
    state: &ProgramState,
    p: &mut OperationalProjection,
    scope: &EffectScope,
    o: ProcessObservation,
    at: u64,
) {
    p.apply(
        state,
        EffectEvent::ProcessObserved {
            effect: intent(scope).id,
            scope: scope.clone(),
            at,
            observation: o,
        },
    )
    .unwrap();
}
pub fn usage(o: &ProcessObservation) -> Vec<Demand> {
    [
        (Resource::Workers, 1),
        (Resource::TimeMillis, o.elapsed_ms),
        (Resource::OutputBytes, o.stdout_bytes + o.stderr_bytes),
    ]
    .into_iter()
    .map(|(resource, amount)| Demand {
        resource,
        amount: Some(amount),
    })
    .collect()
}
