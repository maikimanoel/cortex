//! Controlled state-machine tests, never actual host qualification.
#[path = "support/program.rs"]
mod program_fixture;
#[path = "support/domain.rs"]
mod support;
use cortex_core::program::{domain::*, operational::*, scheduling::*, *};
use cortex_evidence::program::*;
use support::*;

#[test]
fn unknown_closure_releases_covered_capacity_once_but_keeps_debt_and_disk() {
    let (mut k, i) = coordinator();
    bound(&mut k, &i);
    event(
        &mut k,
        "closure",
        DomainEvent::Closed {
            receipt: closure(&i, None),
            at: 4,
        },
    );
    let p = k.domains().unwrap();
    let r = p.domain(&i.id).unwrap();
    assert_eq!(r.outcome, HistoricalOutcome::Unknown);
    assert!(!r.holds(Resource::Workers));
    assert!(r.holds(Resource::DiskBytes));
    assert!(r.unresolved_consumption(Consumption::CpuExecutionMillis));
    let j = setup(&mut k, "D");
    let before = k.revision();
    let op = domain_operation(
        &k,
        "held-next",
        Command::Domain(Box::new(DomainEvent::Reserve { intent: j, at: 5 })),
    );
    assert!(matches!(
        k.admit(op),
        Err(KernelError::Domain(DomainError::Debt(
            Consumption::CpuExecutionMillis
        )))
    ));
    assert_eq!(k.revision(), before);
    let mut later = closure(&i, Some(7));
    later.released = vec![];
    event(
        &mut k,
        "settle-cpu",
        DomainEvent::Settle {
            receipt: later,
            at: 5,
        },
    );
    let mut duplicate = closure(&i, Some(7));
    duplicate.released = vec![];
    let op = domain_operation(
        &k,
        "duplicate-measurement",
        Command::Domain(Box::new(DomainEvent::Settle {
            receipt: duplicate,
            at: 6,
        })),
    );
    assert!(k.admit(op).is_err());
    assert_eq!(
        k.domains()
            .unwrap()
            .consumed_or_reserved(Consumption::CpuExecutionMillis),
        Ok(7)
    );
}
#[test]
fn measured_consumption_and_closed_capacity_allow_independent_next_work() {
    let (mut k, i) = coordinator();
    bound(&mut k, &i);
    event(
        &mut k,
        "closed",
        DomainEvent::Closed {
            receipt: closure(&i, Some(7)),
            at: 4,
        },
    );
    let j = setup(&mut k, "D");
    event(
        &mut k,
        "next",
        DomainEvent::Reserve {
            intent: j.clone(),
            at: 5,
        },
    );
    assert_eq!(k.domains().unwrap().held(Resource::Workers), Ok(1));
    assert_eq!(k.domains().unwrap().held(Resource::DiskBytes), Ok(64));
    assert_eq!(
        k.domains()
            .unwrap()
            .consumed_or_reserved(Consumption::CpuExecutionMillis),
        Ok(37)
    );
    assert_eq!(
        k.domains().unwrap().domain(&i.id).unwrap().outcome,
        HistoricalOutcome::Unknown
    );
    assert_eq!(
        k.domains().unwrap().domain(&j.id).unwrap().phase,
        DomainPhase::Reserved
    );
}
#[test]
fn global_pool_refuses_two_programs_double_allocating() {
    let (mut k, i) = coordinator();
    let j = add_member(&mut k, "other-program");
    event(
        &mut k,
        "reserve-one",
        DomainEvent::Reserve { intent: i, at: 1 },
    );
    let op = domain_operation(
        &k,
        "reserve-two",
        Command::Domain(Box::new(DomainEvent::Reserve { intent: j, at: 1 })),
    );
    assert!(matches!(
        k.admit(op),
        Err(KernelError::Domain(DomainError::Capacity(
            Resource::Workers
        )))
    ));
    assert_eq!(k.domains().unwrap().domains().len(), 1);
}
#[test]
fn missing_creation_reply_reconstruction_never_returns_to_reserved() {
    let (mut k, i) = coordinator();
    event(
        &mut k,
        "reserve",
        DomainEvent::Reserve {
            intent: i.clone(),
            at: 1,
        },
    );
    event(
        &mut k,
        "create",
        DomainEvent::CreateRequested {
            domain: i.id.clone(),
            at: 2,
        },
    );
    let mut replay = CoordinationKernel::new_domain_fixture(k.initial().clone()).unwrap();
    for o in k.operations() {
        replay.admit(o.clone()).unwrap();
    }
    assert_eq!(
        replay.domains().unwrap().domain(&i.id).unwrap().phase,
        DomainPhase::CreateUncertain
    );
    let op = domain_operation(
        &replay,
        "recreate",
        Command::Domain(Box::new(DomainEvent::CreateRequested {
            domain: i.id,
            at: 3,
        })),
    );
    assert!(replay.admit(op).is_err());
}
#[test]
fn stale_receipts_missing_helpers_and_wrong_epochs_never_release() {
    let (mut k, i) = coordinator();
    bound(&mut k, &i);
    for which in 0..7 {
        let mut r = closure(&i, Some(0));
        match which {
            0 => r.identity.boot = "other".into(),
            1 => r.identity.invocation = "reused-name".into(),
            2 => r.identity.cgroup_inode += 1,
            3 => r.whole_domain = false,
            4 => r.restart_prevented = false,
            5 => r.snapshot_safe = false,
            _ => r.quiescent = false,
        };
        let op = domain_operation(
            &k,
            &format!("bad-{which}"),
            Command::Domain(Box::new(DomainEvent::Closed { receipt: r, at: 4 })),
        );
        assert!(k.admit(op).is_err());
    }
    assert_eq!(k.domains().unwrap().held(Resource::Workers), Ok(1));
}
#[test]
fn admin_disposition_and_cancel_never_claim_cleanup() {
    let (mut k, i) = coordinator();
    bound(&mut k, &i);
    event(
        &mut k,
        "cancel",
        DomainEvent::Cancel {
            domain: i.id.clone(),
            at: 4,
            basis: source("cancel"),
        },
    );
    event(
        &mut k,
        "archive",
        DomainEvent::Disposition {
            domain: i.id.clone(),
            disposition: AdministrativeDisposition::Abandoned,
            at: 5,
            basis: source("archive"),
        },
    );
    let r = k.domains().unwrap().domain(&i.id).unwrap();
    assert_eq!(r.outcome, HistoricalOutcome::Unknown);
    assert_eq!(r.phase, DomainPhase::Bound);
    assert!(r.holds(Resource::Workers));
    let op = domain_operation(
        &k,
        "late",
        Command::Domain(Box::new(DomainEvent::ReleaseRequested {
            domain: i.id.clone(),
            identity: identity(&i),
            at: 6,
        })),
    );
    assert!(k.admit(op).is_err());
}
#[test]
fn cancellation_before_creation_cannot_be_restarted_and_refunds_only_future_allocation() {
    let (mut k, i) = coordinator();
    event(
        &mut k,
        "reserve",
        DomainEvent::Reserve {
            intent: i.clone(),
            at: 1,
        },
    );
    event(
        &mut k,
        "cancel",
        DomainEvent::Cancel {
            domain: i.id.clone(),
            at: 2,
            basis: source("cancel"),
        },
    );
    let r = k.domains().unwrap().domain(&i.id).unwrap();
    assert_eq!(r.phase, DomainPhase::CancelledBeforeCreate);
    assert_eq!(r.outcome, HistoricalOutcome::NotDispatched);
    assert!(!r.holds(Resource::Workers));
    assert_eq!(
        r.measured_consumption(Consumption::CpuExecutionMillis),
        None
    );
    let op = domain_operation(
        &k,
        "create",
        Command::Domain(Box::new(DomainEvent::CreateRequested {
            domain: i.id,
            at: 3,
        })),
    );
    assert!(k.admit(op).is_err());
}
#[test]
fn old_effect_path_and_member_recursion_are_unavailable() {
    let (mut k, i) = coordinator();
    let op = domain_operation(
        &k,
        "legacy",
        Command::SupervisedEffect(Box::new(EffectEvent::Reserve {
            intent: i.effect,
            at: 1,
        })),
    );
    assert!(k.admit(op).is_err());
    let j = add_member(&mut k, "member");
    let member = k.domain_member(&j.effect.scope.program).unwrap();
    let nested = domain_operation(
        member,
        "nested",
        Command::RegisterDomainMember(Box::new(named_definition("nested"))),
    );
    let op = domain_operation(
        &k,
        "outer",
        Command::DomainMember {
            program: j.effect.scope.program,
            operation: Box::new(nested),
        },
    );
    assert!(k.admit(op).is_err());
}
#[test]
fn pending_and_closed_unknown_domains_keep_metadata_ownership() {
    for closed in [false, true] {
        let (mut k, i) = coordinator();
        bound(&mut k, &i);
        if closed {
            event(
                &mut k,
                "close",
                DomainEvent::Closed {
                    receipt: closure(&i, Some(7)),
                    at: 4,
                },
            );
        }
        let op = domain_operation(
            &k,
            "discard-attempt",
            Command::Attempt {
                id: i.effect.scope.attempt,
                generation: rev(1),
                status: AttemptStatus::Cancelled,
                basis: source("no-proof"),
            },
        );
        assert!(k.admit(op).is_err());
    }
    let (mut k, _) = coordinator();
    let j = add_member(&mut k, "member");
    bound(&mut k, &j);
    let member = k.domain_member(&j.effect.scope.program).unwrap();
    let op = domain_operation(
        member,
        "discard",
        Command::Attempt {
            id: j.effect.scope.attempt.clone(),
            generation: rev(1),
            status: AttemptStatus::Cancelled,
            basis: source("no-proof"),
        },
    );
    let outer = domain_operation(
        &k,
        "outer",
        Command::DomainMember {
            program: j.effect.scope.program,
            operation: Box::new(op),
        },
    );
    assert!(k.admit(outer).is_err());
}
#[test]
fn stale_authority_blocks_binding_and_release_without_resetting_history() {
    let (mut k, i) = coordinator();
    event(
        &mut k,
        "reserve",
        DomainEvent::Reserve {
            intent: i.clone(),
            at: 1,
        },
    );
    event(
        &mut k,
        "create",
        DomainEvent::CreateRequested {
            domain: i.id.clone(),
            at: 2,
        },
    );
    submit(
        &mut k,
        "revoke",
        Command::SupervisedEffect(Box::new(EffectEvent::Revoke {
            grant: i.effect.grant.clone(),
            at: 3,
            basis: source("revoke"),
        })),
    );
    let op = domain_operation(
        &k,
        "bind",
        Command::Domain(Box::new(DomainEvent::Bound {
            domain: i.id.clone(),
            identity: identity(&i),
            at: 4,
        })),
    );
    assert!(k.admit(op).is_err());
    assert_eq!(
        k.domains().unwrap().domain(&i.id).unwrap().phase,
        DomainPhase::CreateUncertain
    );
}
#[test]
fn exact_recovered_closure_after_lost_identity_and_revocation_never_reopens_payload() {
    let (mut k, i) = coordinator();
    event(
        &mut k,
        "reserve",
        DomainEvent::Reserve {
            intent: i.clone(),
            at: 1,
        },
    );
    event(
        &mut k,
        "create",
        DomainEvent::CreateRequested {
            domain: i.id.clone(),
            at: 2,
        },
    );
    submit(
        &mut k,
        "revoke",
        Command::SupervisedEffect(Box::new(EffectEvent::Revoke {
            grant: i.effect.grant.clone(),
            at: 3,
            basis: source("revoke"),
        })),
    );
    event(
        &mut k,
        "cancel",
        DomainEvent::Cancel {
            domain: i.id.clone(),
            at: 4,
            basis: source("cancel"),
        },
    );
    let mut forged = closure(&i, Some(7));
    forged.identity.request = OperationId::new("unrelated-request").unwrap();
    let op = domain_operation(
        &k,
        "wrong-proof",
        Command::Domain(Box::new(DomainEvent::RecoveredClosed {
            receipt: forged,
            at: 5,
        })),
    );
    assert!(k.admit(op).is_err());
    event(
        &mut k,
        "recover-closed",
        DomainEvent::RecoveredClosed {
            receipt: closure(&i, Some(7)),
            at: 5,
        },
    );
    let r = k.domains().unwrap().domain(&i.id).unwrap();
    assert_eq!(r.outcome, HistoricalOutcome::Unknown);
    assert_eq!(r.phase, DomainPhase::Closed);
    assert!(r.cancelled);
    assert_eq!(k.domains().unwrap().held(Resource::Workers), Ok(0));
    assert!(k.check_domain_current(&i.id, 6).is_err());
    let j = setup(&mut k, "D");
    event(
        &mut k,
        "next-independent",
        DomainEvent::Reserve { intent: j, at: 6 },
    );
}
#[test]
fn measured_use_over_declared_reservation_is_retained_not_clamped() {
    let (mut k, i) = coordinator();
    bound(&mut k, &i);
    event(
        &mut k,
        "closed",
        DomainEvent::Closed {
            receipt: closure(&i, Some(100)),
            at: 4,
        },
    );
    assert_eq!(
        k.domains()
            .unwrap()
            .consumed_or_reserved(Consumption::CpuExecutionMillis),
        Ok(100)
    );
    let j = setup(&mut k, "D");
    let input = domain_operation(
        &k,
        "next",
        Command::Domain(Box::new(DomainEvent::Reserve { intent: j, at: 5 })),
    );
    assert!(matches!(
        k.admit(input),
        Err(KernelError::Domain(DomainError::Budget(
            Consumption::CpuExecutionMillis
        )))
    ));
}
#[test]
fn member_aggregate_capacity_is_not_reinterpreted_as_a_per_attempt_limit() {
    let mut k = CoordinationKernel::new_domain_fixture(definition()).unwrap();
    let mut p = pool();
    p.capacities[0].available = Some(4);
    event(&mut k, "pool", DomainEvent::ConfigurePool(p));
    let i = setup(&mut k, "A");
    let j = setup(&mut k, "D");
    // Member's existing policy has one available worker after reserve, whereas
    // this physical pool has three. Same-program allocations still cannot exceed one.
    event(&mut k, "one", DomainEvent::Reserve { intent: i, at: 1 });
    let op = domain_operation(
        &k,
        "two",
        Command::Domain(Box::new(DomainEvent::Reserve { intent: j, at: 1 })),
    );
    assert!(matches!(
        k.admit(op),
        Err(KernelError::Operational(OperationalError::ResourceLimit(
            Resource::Workers
        )))
    ));
    let j = add_member(&mut k, "independent-member");
    event(
        &mut k,
        "independent",
        DomainEvent::Reserve { intent: j, at: 1 },
    );
    assert_eq!(k.domains().unwrap().held(Resource::Workers), Ok(2));
}
#[test]
fn cumulative_member_output_requires_exact_shared_consumption_budget() {
    for shared in [false, true] {
        let (base, mut i) = coordinator();
        let mut k = CoordinationKernel::new_domain_fixture(base.initial().clone()).unwrap();
        for input in base.operations() {
            let mut input = input.clone();
            match &mut input.command {
                Command::SupervisedEffect(e) => match e.as_mut() {
                    EffectEvent::Policy(p) => {
                        p.capacities.push(Capacity {
                            resource: Resource::OutputBytes,
                            available: Some(20),
                            verification_reserve: 0,
                            declared_enforceable: true,
                        });
                        p.cumulative = vec![Resource::OutputBytes];
                    }
                    EffectEvent::Grant(g) => g.ceilings.push(Demand {
                        resource: Resource::OutputBytes,
                        amount: Some(10),
                    }),
                    _ => {}
                },
                Command::Domain(e) => {
                    if let DomainEvent::ConfigurePool(p) = e.as_mut()
                        && shared
                    {
                        p.consumption.push(ConsumptionBudget {
                            dimension: Consumption::OutputBytes,
                            total: 100,
                            reserve: 20,
                            per_operation: 10,
                        });
                    }
                }
                _ => {}
            }
            k.admit(input).unwrap();
        }
        i.effect.demand.push(Demand {
            resource: Resource::OutputBytes,
            amount: Some(10),
        });
        i.consumption.push(ConsumptionReservation {
            dimension: Consumption::OutputBytes,
            amount: 10,
        });
        let op = domain_operation(
            &k,
            "shared-output",
            Command::Domain(Box::new(DomainEvent::Reserve { intent: i, at: 1 })),
        );
        assert_eq!(k.admit(op).is_ok(), shared);
    }
}

#[test]
fn observed_binding_retains_facts_without_release_or_resource_refund() {
    let (mut k, i) = coordinator();
    event(
        &mut k,
        "reserve",
        DomainEvent::Reserve {
            intent: i.clone(),
            at: 1,
        },
    );
    event(
        &mut k,
        "create",
        DomainEvent::CreateRequested {
            domain: i.id.clone(),
            at: 2,
        },
    );
    let observation = binding(&i);
    event(
        &mut k,
        "observed-bind",
        DomainEvent::BoundObserved {
            domain: i.id.clone(),
            observation: observation.clone(),
            at: 3,
        },
    );
    let r = k.domains().unwrap().domain(&i.id).unwrap();
    assert!(r.binding_observation.as_ref() == Some(&observation));
    assert_eq!(r.phase, DomainPhase::Bound);
    assert_eq!(r.outcome, HistoricalOutcome::Unknown);
    assert!(r.holds(Resource::Workers));
    assert!(r.unresolved_consumption(Consumption::CpuExecutionMillis));
    assert!(r.closure.is_none());
    let duplicate = domain_operation(
        &k,
        "another-bind",
        Command::Domain(Box::new(DomainEvent::BoundObserved {
            domain: i.id.clone(),
            observation,
            at: 4,
        })),
    );
    assert!(k.admit(duplicate).is_err());
}

#[test]
fn malformed_or_mismatched_observed_bindings_refuse_atomically() {
    let (mut k, i) = coordinator();
    event(
        &mut k,
        "reserve",
        DomainEvent::Reserve {
            intent: i.clone(),
            at: 1,
        },
    );
    event(
        &mut k,
        "create",
        DomainEvent::CreateRequested {
            domain: i.id.clone(),
            at: 2,
        },
    );
    let before = k.domains().unwrap().clone();
    for case in 0..16 {
        let mut observation = binding(&i);
        match case {
            0 => observation.main.pid = 0,
            1 => observation.main.start_ticks = 0,
            2 => observation.direct_members.clear(),
            3 => observation.direct_members.push(observation.main.clone()),
            4 => observation.direct_members[0].start_ticks += 1,
            5 => observation.identity.boot = "wrong-boot".into(),
            6 => observation.configuration = source("wrong-config"),
            7 => observation.controls.tasks = 0,
            8 => observation.controls.cpu_period_us = 0,
            9 => observation.controls.swap_bytes = 1,
            10 => observation.controls.restart_prevented = false,
            11 => observation.controls.kill_control_group = false,
            12 => observation.controls.no_new_privileges = false,
            13 => observation.controls.send_sigkill = false,
            14 => observation.controls.runtime_max_micros = 0,
            _ => observation.populated = false,
        }
        let input = domain_operation(
            &k,
            &format!("bad-observation-{case}"),
            Command::Domain(Box::new(DomainEvent::BoundObserved {
                domain: i.id.clone(),
                observation,
                at: 3,
            })),
        );
        assert!(k.admit(input).is_err(), "case {case}");
        assert!(k.domains().unwrap() == &before);
    }
    event(
        &mut k,
        "allowed-observation",
        DomainEvent::BoundObserved {
            domain: i.id.clone(),
            observation: binding(&i),
            at: 3,
        },
    );
}

#[test]
fn cancellation_and_revocation_before_observed_binding_hold_identity_and_release() {
    for cancel in [true, false] {
        let (mut k, i) = coordinator();
        event(
            &mut k,
            "reserve",
            DomainEvent::Reserve {
                intent: i.clone(),
                at: 1,
            },
        );
        event(
            &mut k,
            "create",
            DomainEvent::CreateRequested {
                domain: i.id.clone(),
                at: 2,
            },
        );
        if cancel {
            event(
                &mut k,
                "cancel",
                DomainEvent::Cancel {
                    domain: i.id.clone(),
                    at: 3,
                    basis: source("cancel"),
                },
            );
        } else {
            submit(
                &mut k,
                "revoke",
                Command::SupervisedEffect(Box::new(EffectEvent::Revoke {
                    grant: i.effect.grant.clone(),
                    at: 3,
                    basis: source("revoke"),
                })),
            );
        }
        let input = domain_operation(
            &k,
            "late-observed-bind",
            Command::Domain(Box::new(DomainEvent::BoundObserved {
                domain: i.id.clone(),
                observation: binding(&i),
                at: 4,
            })),
        );
        assert!(k.admit(input).is_err());
        let r = k.domains().unwrap().domain(&i.id).unwrap();
        assert_eq!(r.phase, DomainPhase::CreateUncertain);
        assert!(r.binding_observation.is_none() && r.identity.is_none());
        assert!(r.holds(Resource::Workers));
    }
}

#[test]
fn partial_closure_readback_preserves_unknown_holds_and_final_debt() {
    let (mut k, i) = coordinator();
    observed_cancelled(&mut k, &i);
    event(
        &mut k,
        "post-cancel",
        DomainEvent::ClosureObserved {
            domain: i.id.clone(),
            observation: closure_observation(&i, 5),
            at: 5,
        },
    );
    let r = k.domains().unwrap().domain(&i.id).unwrap();
    assert_eq!(r.outcome, HistoricalOutcome::Unknown);
    assert_eq!(r.cancelled_at, Some(4));
    assert!(r.holds(Resource::Workers));
    assert!(r.holds(Resource::RamBytes));
    assert!(r.holds(Resource::DiskBytes));
    assert!(r.unresolved_consumption(Consumption::CpuExecutionMillis));
    assert!(r.closure.is_none());
    let inspection = r.closure_inspection();
    assert_eq!(inspection.retained_observations, 1);
    assert!(!inspection.receipt_recorded);
    for gap in [
        ClosureProofGap::WholeDomainCoverage,
        ClosureProofGap::HelperCoverage,
        ClosureProofGap::StreamCoverage,
        ClosureProofGap::SnapshotExclusion,
        ClosureProofGap::FinalConsumption,
    ] {
        assert!(inspection.missing.contains(&gap));
    }
    assert!(
        !inspection
            .missing
            .contains(&ClosureProofGap::PopulatedDomain)
    );
    let next = setup(&mut k, "D");
    let op = domain_operation(
        &k,
        "next-refused",
        Command::Domain(Box::new(DomainEvent::Reserve {
            intent: next,
            at: 6,
        })),
    );
    assert!(k.admit(op).is_err());
}

#[test]
fn closure_readback_rejects_mixed_identity_scope_ticks_and_pid_reuse_atomically() {
    let (mut k, i) = coordinator();
    observed_cancelled(&mut k, &i);
    let good = closure_observation(&i, 5);
    let mut inputs = vec![];
    let mut o = good.clone();
    o.identity.manager = "other-manager".into();
    inputs.push(o);
    let mut o = good.clone();
    o.identity.invocation = "other-invocation".into();
    inputs.push(o);
    let mut o = good.clone();
    o.identity.cgroup_inode += 1;
    inputs.push(o);
    let mut o = good.clone();
    o.identity.boot = "other-boot".into();
    inputs.push(o);
    let mut o = good.clone();
    o.configuration = source("other-configuration");
    inputs.push(o);
    let mut o = good.clone();
    o.scope.generation = rev(2);
    inputs.push(o);
    let mut o = good.clone();
    o.scope.package_revision = rev(2);
    inputs.push(o);
    let mut o = good.clone();
    o.binding_main.start_ticks += 1;
    inputs.push(o);
    let mut o = good.clone();
    o.observed_at = 6;
    inputs.push(o);
    let mut o = good.clone();
    o.populated = true;
    o.direct_members.push(DomainProcessObservation {
        pid: o.binding_main.pid,
        start_ticks: o.binding_main.start_ticks + 1,
    });
    inputs.push(o);
    for (n, observation) in inputs.into_iter().enumerate() {
        let before = k.clone();
        let op = domain_operation(
            &k,
            &format!("bad-observation-{n}"),
            Command::Domain(Box::new(DomainEvent::ClosureObserved {
                domain: i.id.clone(),
                observation,
                at: 5,
            })),
        );
        assert!(k.admit(op).is_err());
        assert!(k.domains() == before.domains());
        assert_eq!(k.revision(), before.revision());
    }
    event(
        &mut k,
        "good-observation",
        DomainEvent::ClosureObserved {
            domain: i.id.clone(),
            observation: good.clone(),
            at: 5,
        },
    );
    let duplicate = domain_operation(
        &k,
        "duplicate-observation",
        Command::Domain(Box::new(DomainEvent::ClosureObserved {
            domain: i.id.clone(),
            observation: good,
            at: 5,
        })),
    );
    assert!(k.admit(duplicate).is_err());
    let mut stale = closure_observation(&i, 3);
    stale.basis = source("delayed");
    let op = domain_operation(
        &k,
        "delayed-observation",
        Command::Domain(Box::new(DomainEvent::ClosureObserved {
            domain: i.id,
            observation: stale,
            at: 3,
        })),
    );
    assert!(k.admit(op).is_err());
}

#[test]
fn post_cancel_readbacks_are_bounded_and_hidden_members_do_not_prove_quiescence() {
    let (mut k, i) = coordinator();
    observed_cancelled(&mut k, &i);
    for n in 0..32 {
        let mut observation = closure_observation(&i, 5 + n);
        observation.populated = true;
        observation.restart_prevented = false;
        observation.cpu_micros = None;
        observation.memory_peak = None;
        event(
            &mut k,
            &format!("partial-{n}"),
            DomainEvent::ClosureObserved {
                domain: i.id.clone(),
                observation,
                at: 5 + n,
            },
        );
    }
    let r = k.domains().unwrap().domain(&i.id).unwrap();
    assert_eq!(r.closure_observations.len(), 32);
    let gaps = r.closure_inspection().missing;
    assert!(gaps.contains(&ClosureProofGap::PopulatedDomain));
    assert!(gaps.contains(&ClosureProofGap::RestartPrevention));
    assert!(gaps.contains(&ClosureProofGap::FinalConsumption));
    let extra = domain_operation(
        &k,
        "too-many",
        Command::Domain(Box::new(DomainEvent::ClosureObserved {
            domain: i.id.clone(),
            observation: closure_observation(&i, 37),
            at: 37,
        })),
    );
    assert!(matches!(
        k.admit(extra),
        Err(KernelError::Domain(DomainError::Full))
    ));
}

#[test]
fn readback_requires_cancelled_observed_binding_and_cannot_reopen_closed_domain() {
    let (mut k, i) = coordinator();
    bound(&mut k, &i); // Legacy identity-only binding supplies no birth evidence.
    let input = DomainEvent::ClosureObserved {
        domain: i.id.clone(),
        observation: closure_observation(&i, 4),
        at: 4,
    };
    assert!(
        k.admit(domain_operation(
            &k,
            "no-binding-cancel",
            Command::Domain(Box::new(input))
        ))
        .is_err()
    );
    let (mut k, i) = coordinator();
    observed_cancelled(&mut k, &i);
    event(
        &mut k,
        "fixture-closed",
        DomainEvent::Closed {
            receipt: closure(&i, Some(7)),
            at: 5,
        },
    );
    let input = DomainEvent::ClosureObserved {
        domain: i.id.clone(),
        observation: closure_observation(&i, 6),
        at: 6,
    };
    assert!(
        k.admit(domain_operation(
            &k,
            "after-close",
            Command::Domain(Box::new(input))
        ))
        .is_err()
    );
}

#[test]
fn recursive_capture_retains_hidden_descendant_without_closure_or_final_usage() {
    let (mut k, i) = coordinator();
    observed_cancelled(&mut k, &i);
    event(
        &mut k,
        "tree",
        DomainEvent::ClosureTreeObserved {
            domain: i.id.clone(),
            observation: closure_tree_observation(&i, 5),
            at: 5,
        },
    );
    let record = k.domains().unwrap().domain(&i.id).unwrap();
    assert_eq!(record.closure_tree_observations.len(), 1);
    assert!(
        record.closure_tree_observations[0]
            .sample
            .direct_members
            .is_empty()
    );
    assert_eq!(
        record.closure_tree_observations[0].nodes[1].members.len(),
        1
    );
    assert_eq!(record.closure_inspection().retained_observations, 1);
    for gap in [
        ClosureProofGap::PopulatedDomain,
        ClosureProofGap::RemainingMembers,
        ClosureProofGap::WholeDomainCoverage,
        ClosureProofGap::HelperCoverage,
        ClosureProofGap::StreamCoverage,
        ClosureProofGap::SnapshotExclusion,
        ClosureProofGap::FinalConsumption,
    ] {
        assert!(record.closure_inspection().missing.contains(&gap));
    }
    assert_eq!(record.outcome, HistoricalOutcome::Unknown);
    assert!(record.closure.is_none() && record.holds(Resource::Workers));
    assert!(record.unresolved_consumption(Consumption::CpuExecutionMillis));
    let next = setup(&mut k, "D");
    let op = domain_operation(
        &k,
        "next",
        Command::Domain(Box::new(DomainEvent::Reserve {
            intent: next,
            at: 6,
        })),
    );
    let before = k.revision();
    let result = k.admit(op);
    assert!(
        matches!(
            &result,
            Err(KernelError::Operational(OperationalError::ResourceLimit(
                Resource::Workers
            )))
        ),
        "unexpected refusal: {result:?}"
    );
    assert_eq!(k.revision(), before);
}

#[test]
fn recursive_capture_rejects_mixed_incarnations_disconnected_paths_and_reused_births() {
    let (mut k, i) = coordinator();
    observed_cancelled(&mut k, &i);
    let good = closure_tree_observation(&i, 5);
    let mut inputs = Vec::new();
    let mut v = good.clone();
    v.nodes[0].inode += 1;
    inputs.push(v);
    let mut v = good.clone();
    v.nodes[1].relative_path = "../escape".into();
    inputs.push(v);
    let mut v = good.clone();
    v.nodes[1].relative_path = "missing/child".into();
    inputs.push(v);
    let mut v = good.clone();
    v.nodes[1].relative_path = "/absolute".into();
    inputs.push(v);
    let mut v = good.clone();
    v.nodes[1].device += 1;
    inputs.push(v);
    let mut v = good.clone();
    v.nodes[1].inode = v.nodes[0].inode;
    inputs.push(v);
    let mut v = good.clone();
    v.nodes.push(v.nodes[1].clone());
    inputs.push(v);
    let mut v = good.clone();
    let member = v.nodes[1].members[0].clone();
    v.nodes[1].members.push(member);
    inputs.push(v);
    let mut v = good.clone();
    v.nodes[1].members[0] = DomainProcessObservation {
        pid: v.sample.binding_main.pid,
        start_ticks: v.sample.binding_main.start_ticks + 1,
    };
    inputs.push(v);
    let mut v = good.clone();
    v.sample.identity.boot = "other-boot".into();
    inputs.push(v);
    let mut v = good.clone();
    v.sample.scope.generation = rev(2);
    inputs.push(v);
    let mut v = good.clone();
    v.sample.observed_at = 6;
    inputs.push(v);
    let mut v = good.clone();
    v.nodes[0].populated = false;
    v.sample.populated = false;
    inputs.push(v);
    for (n, observation) in inputs.into_iter().enumerate() {
        let before = k.clone();
        let op = domain_operation(
            &k,
            &format!("bad-tree-{n}"),
            Command::Domain(Box::new(DomainEvent::ClosureTreeObserved {
                domain: i.id.clone(),
                observation,
                at: 5,
            })),
        );
        assert!(k.admit(op).is_err());
        assert!(k.domains() == before.domains());
        assert_eq!(k.revision(), before.revision());
    }
    event(
        &mut k,
        "tree",
        DomainEvent::ClosureTreeObserved {
            domain: i.id.clone(),
            observation: good,
            at: 5,
        },
    );
    let duplicate = domain_operation(
        &k,
        "tree-duplicate",
        Command::Domain(Box::new(DomainEvent::ClosureTreeObserved {
            domain: i.id.clone(),
            observation: closure_tree_observation(&i, 5),
            at: 5,
        })),
    );
    assert!(k.admit(duplicate).is_err());
    let old_direct = domain_operation(
        &k,
        "older-direct",
        Command::Domain(Box::new(DomainEvent::ClosureObserved {
            domain: i.id.clone(),
            observation: closure_observation(&i, 5),
            at: 5,
        })),
    );
    assert!(k.admit(old_direct).is_err());
}

#[test]
fn empty_recursive_capture_still_requires_stream_snapshot_and_final_measurement_proof() {
    let (mut k, i) = coordinator();
    observed_cancelled(&mut k, &i);
    let mut capture = closure_tree_observation(&i, 5);
    capture.nodes.truncate(1);
    capture.nodes[0].populated = false;
    capture.sample.populated = false;
    capture.sample.cpu_micros = Some(0);
    event(
        &mut k,
        "empty-tree",
        DomainEvent::ClosureTreeObserved {
            domain: i.id.clone(),
            observation: capture,
            at: 5,
        },
    );
    let record = k.domains().unwrap().domain(&i.id).unwrap();
    assert_eq!(record.closure_inspection().missing.len(), 5);
    assert!(
        record.holds(Resource::Workers)
            && record.unresolved_consumption(Consumption::CpuExecutionMillis)
    );
    assert!(record.closure.is_none());
}
