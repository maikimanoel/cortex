#![allow(dead_code)]
pub use super::program_fixture::*;
use cortex_core::parallel::*;
use cortex_core::program::{context::*, domain::*, operational::*, scheduling::*, *};
use cortex_evidence::program::*;

pub fn pool_identity() -> PoolIdentity {
    PoolIdentity {
        physical: "synthetic-pool".into(),
        incarnation: "allocation-1".into(),
        host: "synthetic-host".into(),
        boot: "boot-1".into(),
    }
}
pub fn pool() -> PoolContract {
    PoolContract {
        identity: pool_identity(),
        basis: source("pool-journal-association"),
        capacities: vec![
            Capacity {
                resource: Resource::WorkerSlots,
                available: Some(2),
                verification_reserve: 1,
                declared_enforceable: true,
            },
            Capacity {
                resource: Resource::RamBytes,
                available: Some(1024),
                verification_reserve: 512,
                declared_enforceable: true,
            },
            Capacity {
                resource: Resource::DiskBytes,
                available: Some(1024),
                verification_reserve: 0,
                declared_enforceable: true,
            },
        ],
        consumption: vec![ConsumptionBudget {
            dimension: Consumption::CpuExecutionMillis,
            total: 100,
            reserve: 20,
            per_operation: 30,
        }],
        max_domains: 16,
    }
}
pub fn domain_operation(k: &CoordinationKernel, id: &str, command: Command) -> Operation {
    let mut g = grant(k.state().definition().revision().get(), "operator");
    g.id = GrantId::new(&format!("coord-{}", k.state().definition().id().as_str())).unwrap();
    g.program = k.state().definition().id().clone();
    Operation {
        id: OperationId::new(id).unwrap(),
        expected: k.revision(),
        grant: g,
        command,
    }
}
pub fn submit(k: &mut CoordinationKernel, id: &str, command: Command) {
    let op = domain_operation(k, id, command);
    k.admit(op).unwrap();
}
pub fn event(k: &mut CoordinationKernel, id: &str, event: DomainEvent) {
    submit(k, id, Command::Domain(Box::new(event)));
}
pub fn named_definition(name: &str) -> ProgramDefinition {
    let d = definition();
    ProgramDefinition::new(
        ProgramId::new(name).unwrap(),
        d.outcome().clone(),
        d.revision(),
        d.predecessor(),
        d.approval().clone(),
        d.graph().clone(),
        d.workstreams().to_vec(),
        d.increments().to_vec(),
        d.limits(),
    )
    .unwrap()
}
pub fn setup(k: &mut CoordinationKernel, task: &str) -> DomainIntent {
    let scope = k
        .state()
        .readiness()
        .scope(&cortex_core::TaskId::new(task).unwrap())
        .unwrap();
    let notification = notify(
        k.state(),
        &format!("ready-{task}"),
        Event::Preconditions {
            scope,
            ready: true,
            source: source("controlled-precondition"),
        },
    );
    submit(k, &format!("ready-{task}"), Command::Notify(notification));
    if k.fixture_effects().policies().is_empty() {
        let p = Policy {
            reference: source("domain-member-policy"),
            program: k.state().definition().id().clone(),
            baseline: rev(1),
            mode: EffectMode::SupervisedFixture,
            host: source("controlled-host"),
            privacy: source("synthetic-only"),
            capacities: pool().capacities,
            cumulative: vec![],
            max_effects: 16,
            max_retries: 0,
        };
        submit(
            k,
            "member-policy",
            Command::SupervisedEffect(Box::new(EffectEvent::Policy(p))),
        );
    }
    let attempt = attempt(&format!("attempt-{task}"), task, "operator");
    submit(k, &format!("claim-{task}"), Command::Claim(attempt.clone()));
    let request = PackageRequest {
        id: PackageId::new(&format!("package-{task}")).unwrap(),
        revision: rev(1),
        task: cortex_core::TaskId::new(task).unwrap(),
        baseline: rev(1),
        generation: rev(1),
        sources: vec![],
        methods: vec![],
        omissions: "Synthetic owned fixture".into(),
        deliverable: "Provider-free controlled test".into(),
        stop_rules: "Hold on uncertainty".into(),
    };
    submit(
        k,
        &format!("package-{task}"),
        Command::Package(request.clone()),
    );
    let scope = EffectScope {
        program: k.state().definition().id().clone(),
        baseline: rev(1),
        task: cortex_core::TaskId::new(task).unwrap(),
        candidate: k
            .state()
            .readiness()
            .scope(&cortex_core::TaskId::new(task).unwrap())
            .unwrap()
            .context()
            .contract()
            .candidate()
            .clone(),
        attempt: attempt.id,
        generation: rev(1),
        package: request.id,
        package_revision: rev(1),
    };
    let demand = vec![
        Demand {
            resource: Resource::WorkerSlots,
            amount: Some(1),
        },
        Demand {
            resource: Resource::RamBytes,
            amount: Some(256),
        },
        Demand {
            resource: Resource::DiskBytes,
            amount: Some(32),
        },
    ];
    let g = OperationalGrant {
        id: GrantId::new(&format!("operational-{task}")).unwrap(),
        principal: ActorId::new("operator").unwrap(),
        policy: source("domain-member-policy"),
        scope: scope.clone(),
        not_before: 1,
        expires_before: 1000,
        ceilings: demand.clone(),
        basis: source("explicit-fixture-grant"),
    };
    submit(
        k,
        &format!("grant-{task}"),
        Command::SupervisedEffect(Box::new(EffectEvent::Grant(g.clone()))),
    );
    DomainIntent {
        id: DomainId::new(&format!("domain-{}-{task}", scope.program.as_str())).unwrap(),
        effect: EffectIntent {
            id: EffectId::new(&format!("effect-{task}")).unwrap(),
            grant: g.id,
            scope,
            demand,
            retry_of: None,
        },
        pool: pool_identity(),
        request: OperationId::new(&format!(
            "request-{}-{task}",
            k.state().definition().id().as_str()
        ))
        .unwrap(),
        configuration: source("fixed-bootstrap-profile"),
        consumption: vec![ConsumptionReservation {
            dimension: Consumption::CpuExecutionMillis,
            amount: 30,
        }],
    }
}
pub fn coordinator() -> (CoordinationKernel, DomainIntent) {
    let mut k = CoordinationKernel::new_domain_fixture(definition()).unwrap();
    event(&mut k, "pool", DomainEvent::ConfigurePool(pool()));
    let i = setup(&mut k, "A");
    (k, i)
}
pub fn identity(i: &DomainIntent) -> DomainIdentity {
    DomainIdentity {
        pool: i.pool.clone(),
        request: i.request.clone(),
        manager: ":1.42".into(),
        invocation: format!("invocation-{}", i.id.as_str()),
        control_group: format!("/owned/{}", i.id.as_str()),
        cgroup_device: 1,
        cgroup_inode: 42,
        boot: i.pool.boot.clone(),
    }
}
pub fn closure(i: &DomainIntent, measured: Option<u64>) -> ClosureReceipt {
    ClosureReceipt {
        domain: i.id.clone(),
        identity: identity(i),
        configuration: i.configuration.clone(),
        scope: i.effect.scope.clone(),
        whole_domain: true,
        restart_prevented: true,
        snapshot_safe: true,
        quiescent: true,
        released: vec![Resource::WorkerSlots, Resource::RamBytes],
        consumption: vec![ConsumptionObservation {
            dimension: Consumption::CpuExecutionMillis,
            measured,
        }],
        evidence: source("independent-controlled-closure"),
    }
}
pub fn bound(k: &mut CoordinationKernel, i: &DomainIntent) {
    event(
        k,
        "reserve",
        DomainEvent::Reserve {
            intent: i.clone(),
            at: 1,
        },
    );
    event(
        k,
        "create",
        DomainEvent::CreateRequested {
            domain: i.id.clone(),
            at: 2,
        },
    );
    event(
        k,
        "bind",
        DomainEvent::Bound {
            domain: i.id.clone(),
            identity: identity(i),
            at: 3,
        },
    );
}
pub fn add_member(k: &mut CoordinationKernel, name: &str) -> DomainIntent {
    let mut member = CoordinationKernel::new_supervised_fixture(named_definition(name)).unwrap();
    let i = setup(&mut member, "A");
    submit(
        k,
        &format!("register-{name}"),
        Command::RegisterDomainMember(Box::new(member.initial().clone())),
    );
    for o in member.operations() {
        submit(
            k,
            &format!("member-{name}-{}", o.id.as_str()),
            Command::DomainMember {
                program: member.initial().id().clone(),
                operation: Box::new(o.clone()),
            },
        );
    }
    i
}

pub fn binding(i: &DomainIntent) -> DomainBindingObservation {
    let main = DomainProcessObservation {
        pid: 7,
        start_ticks: 11,
    };
    DomainBindingObservation {
        identity: identity(i),
        configuration: i.configuration.clone(),
        main: main.clone(),
        direct_members: vec![main],
        controls: DomainControlReadback {
            cpu_quota_us: 100_000,
            cpu_period_us: 100_000,
            memory_bytes: 512,
            swap_bytes: 0,
            tasks: 8,
            runtime_max_micros: 1_000_000,
            timeout_stop_micros: 500_000,
            restart_prevented: true,
            kill_control_group: true,
            no_new_privileges: true,
            send_sigkill: true,
            remain_after_exit: true,
        },
        populated: true,
        basis: source("controlled-binding-observation"),
    }
}

/// Synthetic direct-member readback; it never supplies physical closure proof.
pub fn closure_observation(i: &DomainIntent, at: u64) -> DomainClosureObservation {
    DomainClosureObservation {
        identity: identity(i),
        configuration: i.configuration.clone(),
        scope: i.effect.scope.clone(),
        binding_main: binding(i).main,
        observed_main: None,
        direct_members: vec![],
        populated: false,
        restart_prevented: true,
        cpu_micros: Some(7_000),
        memory_peak: Some(256),
        observed_at: at,
        basis: source("controlled-post-cancellation-readback"),
    }
}
pub fn observed_cancelled(k: &mut CoordinationKernel, i: &DomainIntent) {
    event(
        k,
        "reserve",
        DomainEvent::Reserve {
            intent: i.clone(),
            at: 1,
        },
    );
    event(
        k,
        "create",
        DomainEvent::CreateRequested {
            domain: i.id.clone(),
            at: 2,
        },
    );
    event(
        k,
        "binding-observation",
        DomainEvent::BoundObserved {
            domain: i.id.clone(),
            observation: binding(i),
            at: 3,
        },
    );
    event(
        k,
        "cancel",
        DomainEvent::Cancel {
            domain: i.id.clone(),
            at: 4,
            basis: source("controlled-cancel"),
        },
    );
}
