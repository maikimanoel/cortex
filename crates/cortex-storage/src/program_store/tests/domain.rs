//! Encrypted replay and concurrent admission for the shared pool coordinator.
use super::fixture as program_fixture;
use super::*;
#[path = "../../../../cortex-evidence/tests/support/domain.rs"]
mod domain_fixture;
use cortex_core::binding::WorktreeRequirement;
use cortex_core::program::domain::*;
use cortex_core::program::scheduling::Resource;
use cortex_evidence::program::Command as DomainInput;
use std::sync::{Arc, Barrier};
fn stored(k: &CoordinationKernel, f: &Temp) -> ProgramStore {
    let s = ProgramStore::create_domain_fixture(&f.store(), &key(), k.initial().clone(), limits())
        .unwrap();
    for o in k.operations() {
        s.admit(&key(), o.clone()).unwrap();
    }
    s
}
fn op(k: &CoordinationKernel, id: &str, e: DomainEvent) -> Operation {
    domain_fixture::domain_operation(k, id, DomainInput::Domain(Box::new(e)))
}
#[test]
fn domain_profile_roundtrip_preserves_history_debt_and_legacy_refusal() {
    let f = Temp::new();
    let (mut k, i) = domain_fixture::coordinator();
    domain_fixture::bound(&mut k, &i);
    domain_fixture::event(
        &mut k,
        "closed",
        DomainEvent::Closed {
            receipt: domain_fixture::closure(&i, None),
            at: 4,
        },
    );
    let store = stored(&k, &f);
    let open = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    assert_eq!(open.profile(), ProgramProfile::RecoverableDomainsV4);
    let actual = open.load(&key(), Access::ReadOnly).unwrap();
    assert!(actual.domains() == k.domains());
    let r = actual.domains().unwrap().domain(&i.id).unwrap();
    assert_eq!(r.outcome, HistoricalOutcome::Unknown);
    assert!(!r.holds(Resource::WorkerSlots));
    assert!(r.holds(Resource::DiskBytes));
    assert!(r.unresolved_consumption(Consumption::CpuExecutionMillis));
    let bytes = fs::read(f.store().join("program.db")).unwrap();
    assert!(
        !bytes
            .windows(b"domain-fixture".len())
            .any(|x| x == b"domain-fixture")
    );
    let old = Temp::new();
    let v3 = ProgramStore::create_supervised_fixture(
        &old.store(),
        &key(),
        k.initial().clone(),
        limits(),
    )
    .unwrap();
    let before = snapshot(&old);
    let failure = v3.admit(&key(), k.operations()[0].clone()).unwrap_err();
    assert!(matches!(failure.error, ProgramStoreError::Unsupported));
    assert_eq!(snapshot(&old), before);
    let bad = op(
        &actual,
        "duplicate-close",
        DomainEvent::Closed {
            receipt: domain_fixture::closure(&i, None),
            at: 5,
        },
    );
    assert!(store.admit(&key(), bad).is_err());
    assert_eq!(
        store.load(&key(), Access::ReadOnly).unwrap().revision(),
        actual.revision()
    );
}
#[test]
fn pool_admission_competing_clients_cannot_double_allocate() {
    let f = Temp::new();
    let (mut k, i) = domain_fixture::coordinator();
    let j = domain_fixture::add_member(&mut k, "other-program");
    let s = stored(&k, &f);
    let one = op(&k, "one", DomainEvent::Reserve { intent: i, at: 1 });
    let two = op(&k, "two", DomainEvent::Reserve { intent: j, at: 1 });
    let b = Arc::new(Barrier::new(3));
    let handles = [one, two]
        .into_iter()
        .map(|o| {
            let b = b.clone();
            let path = f.store();
            std::thread::spawn(move || {
                let store = ProgramStore::open(&path, &key(), Access::ReadOnly, limits()).unwrap();
                b.wait();
                store.admit(&key(), o).is_ok()
            })
        })
        .collect::<Vec<_>>();
    b.wait();
    let winners = handles
        .into_iter()
        .map(|h| h.join().unwrap())
        .filter(|x| *x)
        .count();
    assert_eq!(winners, 1);
    let k = s.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(k.domains().unwrap().held(Resource::WorkerSlots), Ok(1));
    assert_eq!(k.domains().unwrap().domains().len(), 1);
}
#[test]
fn lost_manager_reply_and_replay_never_repeat_creation_callback() {
    let f = Temp::new();
    let (mut k, i) = domain_fixture::coordinator();
    domain_fixture::event(
        &mut k,
        "reserve",
        DomainEvent::Reserve {
            intent: i.clone(),
            at: 1,
        },
    );
    let s = stored(&k, &f);
    let creation = op(
        &k,
        "create",
        DomainEvent::CreateRequested {
            domain: i.id.clone(),
            at: 2,
        },
    );
    let lost = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        s.dispatch_domain(&key(), creation.clone(), |_| {
            panic!("controlled lost manager reply")
        })
    }));
    assert!(lost.is_err());
    let reopen = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    let k = reopen.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(
        k.domains().unwrap().domain(&i.id).unwrap().phase,
        DomainPhase::CreateUncertain
    );
    let (r, called) = reopen
        .dispatch_domain(&key(), creation, |_| panic!("duplicate callback"))
        .unwrap();
    assert!(r.duplicate);
    assert!(called.is_none());
}
#[test]
fn closure_before_after_commit_faults_reconstruct_without_reexecution() {
    for after in [false, true] {
        let f = Temp::new();
        let (mut k, i) = domain_fixture::coordinator();
        domain_fixture::bound(&mut k, &i);
        let s = stored(&k, &f);
        let input = op(
            &k,
            "closure",
            DomainEvent::Closed {
                receipt: domain_fixture::closure(&i, Some(7)),
                at: 4,
            },
        );
        let cut = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            s.admit_observed(&key(), input.clone(), |stage, _| {
                if matches!(
                    (after, stage),
                    (false, Stage::BeforeCommit) | (true, Stage::AfterCommit)
                ) {
                    panic!("controlled publication cut")
                }
            })
        }));
        assert!(cut.is_err());
        let current = s.load(&key(), Access::ReadOnly).unwrap();
        assert_eq!(
            current.domains().unwrap().domain(&i.id).unwrap().phase,
            if after {
                DomainPhase::Closed
            } else {
                DomainPhase::Bound
            }
        );
        let receipt = s.admit(&key(), input.clone()).unwrap();
        assert_eq!(receipt.duplicate, after);
        let current = s.load(&key(), Access::ReadOnly).unwrap();
        let record = current.domains().unwrap().domain(&i.id).unwrap();
        assert_eq!(record.outcome, HistoricalOutcome::Unknown);
        assert_eq!(
            current.domains().unwrap().held(Resource::WorkerSlots),
            Ok(0)
        );
        assert_eq!(
            current
                .domains()
                .unwrap()
                .consumed_or_reserved(Consumption::CpuExecutionMillis),
            Ok(7)
        );
        assert!(s.admit(&key(), input).unwrap().duplicate);
    }
}
#[test]
fn member_source_roots_cannot_contain_private_journal_and_nested_inputs_refuse_early() {
    let f = Temp::new();
    let (k, _) = domain_fixture::coordinator();
    let s = stored(&k, &f);
    let before = snapshot(&f);
    let unsafe_definition =
        with_contracts(&domain_fixture::named_definition("unsafe-member"), |c| {
            let root = f.0.clone();
            CandidateBindingContract::new(
                c.task().clone(),
                c.revision(),
                CandidateRequirement::new(
                    RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git"))
                        .unwrap(),
                    c.candidate().commit().clone(),
                    WorktreeRequirement::Clean,
                ),
            )
        });
    let input = domain_fixture::domain_operation(
        &k,
        "register-private",
        DomainInput::RegisterDomainMember(Box::new(unsafe_definition)),
    );
    assert!(s.admit(&key(), input).is_err());
    assert_eq!(snapshot(&f), before);
    let nested = domain_fixture::domain_operation(
        &k,
        "nested",
        DomainInput::RegisterDomainMember(Box::new(domain_fixture::named_definition("deep"))),
    );
    let outer = domain_fixture::domain_operation(
        &k,
        "outer",
        DomainInput::DomainMember {
            program: k.initial().id().clone(),
            operation: Box::new(nested),
        },
    );
    assert!(matches!(
        s.admit(&key(), outer).unwrap_err().error,
        ProgramStoreError::Unsupported
    ));
    assert_eq!(snapshot(&f), before);
}
#[test]
fn nested_member_amendment_checks_private_source_roots_and_allows_disjoint_neighbor() {
    let f = Temp::new();
    let (mut k, _) = domain_fixture::coordinator();
    let i = domain_fixture::add_member(&mut k, "member");
    let s = stored(&k, &f);
    let before = snapshot(&f);
    for contained in [true, false] {
        let revision = definition_revision(2, false);
        let revision = ProgramDefinition::new(
            i.effect.scope.program.clone(),
            revision.outcome().clone(),
            revision.revision(),
            revision.predecessor(),
            revision.approval().clone(),
            revision.graph().clone(),
            revision.workstreams().to_vec(),
            revision.increments().to_vec(),
            revision.limits(),
        )
        .unwrap();
        let root = if contained {
            f.0.clone()
        } else {
            f.0.join("separate-source")
        };
        let next = with_contracts(&revision, |c| {
            CandidateBindingContract::new(
                c.task().clone(),
                rev(2),
                CandidateRequirement::new(
                    RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git"))
                        .unwrap(),
                    c.candidate().commit().clone(),
                    c.candidate().worktree(),
                ),
            )
        });
        let member = k.domain_member(&i.effect.scope.program).unwrap();
        let input = domain_fixture::domain_operation(
            member,
            "amend-member",
            DomainInput::Amend {
                next: Box::new(next),
                non_impact: cortex_evidence::program::NonImpact {
                    tasks: vec![],
                    basis: source("all-contracts-changed"),
                },
            },
        );
        let outer = domain_fixture::domain_operation(
            &k,
            "outer-amend",
            DomainInput::DomainMember {
                program: i.effect.scope.program.clone(),
                operation: Box::new(input),
            },
        );
        if contained {
            let failure = s.admit(&key(), outer).unwrap_err();
            assert!(
                matches!(failure.error,ProgramStoreError::Storage(e)if e.kind()==ErrorKind::InvalidLocation)
            );
            assert_eq!(snapshot(&f), before);
        } else {
            s.admit(&key(), outer).unwrap();
        }
    }
    assert_eq!(
        s.load(&key(), Access::ReadOnly)
            .unwrap()
            .domain_member(&i.effect.scope.program)
            .unwrap()
            .state()
            .definition()
            .revision(),
        rev(2)
    );
}

#[test]
fn observed_binding_encoding_roundtrip_and_tampering_refuse_downgrade() {
    let f = Temp::new();
    let (mut k, i) = domain_fixture::coordinator();
    domain_fixture::event(
        &mut k,
        "reserve",
        DomainEvent::Reserve {
            intent: i.clone(),
            at: 1,
        },
    );
    domain_fixture::event(
        &mut k,
        "create",
        DomainEvent::CreateRequested {
            domain: i.id.clone(),
            at: 2,
        },
    );
    let s = stored(&k, &f);
    let input = op(
        &k,
        "observed",
        DomainEvent::BoundObserved {
            domain: i.id.clone(),
            observation: domain_fixture::binding(&i),
            at: 3,
        },
    );
    let encoded = codec::operation(&input, k.initial().limits().bytes).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(value[3][0], "domain-observation-v1");
    assert_eq!(value[3][1][0], "bound-observed-v1");
    assert!(codec::decode_operation(&encoded, &k).unwrap() == input);
    for case in 0..5 {
        let mut bad = value.clone();
        match case {
            0 => bad[3][0] = serde_json::json!("domain-v1"),
            1 => bad[3][1][2][2][0] = serde_json::json!(0),
            2 => bad[3][1][2][3] = serde_json::json!([]),
            3 => bad[3][1][2][4][5] = serde_json::json!(0),
            _ => bad[3][1][2][5] = serde_json::json!(false),
        }
        assert!(
            codec::decode_operation(&serde_json::to_vec(&bad).unwrap(), &k).is_err(),
            "case {case}"
        );
    }
    let old = op(
        &k,
        "legacy",
        DomainEvent::Bound {
            domain: i.id.clone(),
            identity: domain_fixture::identity(&i),
            at: 3,
        },
    );
    let old_bytes = codec::operation(&old, k.initial().limits().bytes).unwrap();
    let mut old_value: serde_json::Value = serde_json::from_slice(&old_bytes).unwrap();
    assert_eq!(old_value[3][0], "domain-v1");
    assert_eq!(old_value[3][1][0], "bound-v1");
    assert!(codec::decode_operation(&old_bytes, &k).unwrap() == old);
    old_value[3][0] = serde_json::json!("domain-observation-v1");
    assert!(codec::decode_operation(&serde_json::to_vec(&old_value).unwrap(), &k).is_err());
    s.admit(&key(), input.clone()).unwrap();
    let reopened = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    let restored = reopened.load(&key(), Access::ReadOnly).unwrap();
    let r = restored.domains().unwrap().domain(&i.id).unwrap();
    assert!(r.binding_observation.as_ref() == Some(&domain_fixture::binding(&i)));
    assert_eq!(r.phase, DomainPhase::Bound);
    assert!(r.holds(Resource::WorkerSlots));
    assert!(s.admit(&key(), input).unwrap().duplicate);
}

#[test]
fn observed_binding_write_and_commit_crash_cuts_preserve_unreleased_state() {
    for cut in [Stage::BeforeWrite, Stage::BeforeCommit, Stage::AfterCommit] {
        let f = Temp::new();
        let (mut k, i) = domain_fixture::coordinator();
        domain_fixture::event(
            &mut k,
            "reserve",
            DomainEvent::Reserve {
                intent: i.clone(),
                at: 1,
            },
        );
        domain_fixture::event(
            &mut k,
            "create",
            DomainEvent::CreateRequested {
                domain: i.id.clone(),
                at: 2,
            },
        );
        let s = stored(&k, &f);
        let input = op(
            &k,
            "bind",
            DomainEvent::BoundObserved {
                domain: i.id.clone(),
                observation: domain_fixture::binding(&i),
                at: 3,
            },
        );
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            s.admit_observed(&key(), input, |stage, _| {
                if std::mem::discriminant(&stage) == std::mem::discriminant(&cut) {
                    panic!("controlled binding publication cut");
                }
            })
        }));
        assert!(failed.is_err());
        let restored = s.load(&key(), Access::ReadOnly).unwrap();
        let r = restored.domains().unwrap().domain(&i.id).unwrap();
        let committed = matches!(cut, Stage::AfterCommit);
        assert_eq!(r.binding_observation.is_some(), committed);
        assert_eq!(
            r.phase,
            if committed {
                DomainPhase::Bound
            } else {
                DomainPhase::CreateUncertain
            }
        );
        assert_eq!(r.outcome, HistoricalOutcome::Unknown);
        assert!(r.holds(Resource::WorkerSlots));
        assert!(r.unresolved_consumption(Consumption::CpuExecutionMillis));
    }
}

#[test]
fn closure_readback_roundtrip_and_version_guards_preserve_debt_and_legacy_bytes() {
    let f = Temp::new();
    let (mut k, i) = domain_fixture::coordinator();
    domain_fixture::observed_cancelled(&mut k, &i);
    let s = stored(&k, &f);
    let input = op(
        &k,
        "closure-readback",
        DomainEvent::ClosureObserved {
            domain: i.id.clone(),
            observation: domain_fixture::closure_observation(&i, 5),
            at: 5,
        },
    );
    let bytes = codec::operation(&input, k.initial().limits().bytes).unwrap();
    let mut wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(wire[3][0], "domain-closure-observation-v1");
    assert_eq!(wire[3][1][0], "closure-observed-v1");
    assert!(codec::decode_operation(&bytes, &k).unwrap() == input);
    for tag in [
        "domain-v1",
        "domain-observation-v1",
        "domain-closure-observation-v2",
    ] {
        wire[3][0] = serde_json::json!(tag);
        assert!(codec::decode_operation(&serde_json::to_vec(&wire).unwrap(), &k).is_err());
    }
    wire = serde_json::from_slice(&bytes).unwrap();
    wire[3][1][2][3][1] = serde_json::json!(0); // Invalid bound process birth tick.
    assert!(codec::decode_operation(&serde_json::to_vec(&wire).unwrap(), &k).is_err());
    let old = op(
        &k,
        "legacy-cancel",
        DomainEvent::Cancel {
            domain: i.id.clone(),
            at: 5,
            basis: domain_fixture::source("legacy"),
        },
    );
    let old_bytes = codec::operation(&old, k.initial().limits().bytes).unwrap();
    let old_wire: serde_json::Value = serde_json::from_slice(&old_bytes).unwrap();
    assert_eq!(old_wire[3][0], "domain-v1");
    assert_eq!(old_wire[3][1][0], "cancel-v1");
    assert!(codec::decode_operation(&old_bytes, &k).unwrap() == old);
    s.admit(&key(), input.clone()).unwrap();
    assert!(s.admit(&key(), input).unwrap().duplicate);
    let reopened = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    let restored = reopened.load(&key(), Access::ReadOnly).unwrap();
    let r = restored.domains().unwrap().domain(&i.id).unwrap();
    assert_eq!(r.closure_observations.len(), 1);
    assert_eq!(r.cancelled_at, Some(4));
    assert_eq!(r.outcome, HistoricalOutcome::Unknown);
    assert!(r.holds(Resource::WorkerSlots));
    assert!(r.unresolved_consumption(Consumption::CpuExecutionMillis));
}

#[test]
fn closure_observation_publication_crash_cuts_replay_without_refund_or_effects() {
    for cut in [Stage::BeforeWrite, Stage::BeforeCommit, Stage::AfterCommit] {
        let f = Temp::new();
        let (mut k, i) = domain_fixture::coordinator();
        domain_fixture::observed_cancelled(&mut k, &i);
        let s = stored(&k, &f);
        let input = op(
            &k,
            "closure-readback",
            DomainEvent::ClosureObserved {
                domain: i.id.clone(),
                observation: domain_fixture::closure_observation(&i, 5),
                at: 5,
            },
        );
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            s.admit_observed(&key(), input, |stage, _| {
                if std::mem::discriminant(&stage) == std::mem::discriminant(&cut) {
                    panic!("controlled closure readback publication cut");
                }
            })
        }));
        assert!(failed.is_err());
        let restored = s.load(&key(), Access::ReadOnly).unwrap();
        let r = restored.domains().unwrap().domain(&i.id).unwrap();
        assert_eq!(
            r.closure_observations.len(),
            usize::from(matches!(cut, Stage::AfterCommit))
        );
        assert!(r.cancelled && r.closure.is_none());
        assert_eq!(r.outcome, HistoricalOutcome::Unknown);
        assert!(r.holds(Resource::WorkerSlots));
        assert!(r.unresolved_consumption(Consumption::CpuExecutionMillis));
    }
}

#[test]
fn cancel_publication_cut_requires_replayed_durable_cancel_before_readback() {
    for cut in [Stage::BeforeWrite, Stage::BeforeCommit, Stage::AfterCommit] {
        let f = Temp::new();
        let (mut k, i) = domain_fixture::coordinator();
        domain_fixture::event(
            &mut k,
            "reserve",
            DomainEvent::Reserve {
                intent: i.clone(),
                at: 1,
            },
        );
        domain_fixture::event(
            &mut k,
            "create",
            DomainEvent::CreateRequested {
                domain: i.id.clone(),
                at: 2,
            },
        );
        domain_fixture::event(
            &mut k,
            "bind",
            DomainEvent::BoundObserved {
                domain: i.id.clone(),
                observation: domain_fixture::binding(&i),
                at: 3,
            },
        );
        let s = stored(&k, &f);
        let cancel = op(
            &k,
            "cancel-cut",
            DomainEvent::Cancel {
                domain: i.id.clone(),
                at: 4,
                basis: domain_fixture::source("cancel-cut"),
            },
        );
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            s.admit_observed(&key(), cancel, |stage, _| {
                if std::mem::discriminant(&stage) == std::mem::discriminant(&cut) {
                    panic!("controlled cancellation publication cut");
                }
            })
        }));
        assert!(failed.is_err());
        let restored = s.load(&key(), Access::ReadOnly).unwrap();
        let committed = matches!(cut, Stage::AfterCommit);
        let record = restored.domains().unwrap().domain(&i.id).unwrap();
        assert_eq!(record.cancelled, committed);
        assert_eq!(record.cancelled_at, committed.then_some(4));
        let observation = op(
            &restored,
            "after-cancel-cut",
            DomainEvent::ClosureObserved {
                domain: i.id.clone(),
                observation: domain_fixture::closure_observation(&i, 5),
                at: 5,
            },
        );
        assert_eq!(s.admit(&key(), observation).is_ok(), committed);
        let after = s.load(&key(), Access::ReadOnly).unwrap();
        let record = after.domains().unwrap().domain(&i.id).unwrap();
        assert!(record.closure.is_none());
        assert!(record.holds(Resource::WorkerSlots));
        assert_eq!(record.outcome, HistoricalOutcome::Unknown);
    }
}

#[test]
fn recursive_capture_codec_replay_and_crash_cuts_preserve_unknown() {
    for cut in [Stage::BeforeWrite, Stage::BeforeCommit, Stage::AfterCommit] {
        let f = Temp::new();
        let (mut k, i) = domain_fixture::coordinator();
        domain_fixture::observed_cancelled(&mut k, &i);
        let s = stored(&k, &f);
        let input = op(
            &k,
            "tree",
            DomainEvent::ClosureTreeObserved {
                domain: i.id.clone(),
                observation: domain_fixture::closure_tree_observation(&i, 5),
                at: 5,
            },
        );
        let bytes = codec::operation(&input, k.initial().limits().bytes).unwrap();
        let wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(wire[3][0], "domain-closure-tree-v1");
        assert_eq!(wire[3][1][0], "closure-tree-observed-v1");
        assert!(codec::decode_operation(&bytes, &k).unwrap() == input);
        for tag in [
            "domain-v1",
            "domain-observation-v1",
            "domain-closure-observation-v1",
            "domain-closure-tree-v2",
        ] {
            let mut bad = wire.clone();
            bad[3][0] = serde_json::json!(tag);
            assert!(codec::decode_operation(&serde_json::to_vec(&bad).unwrap(), &k).is_err());
        }
        let mut bad = wire.clone();
        bad[3][1][2][1][1][0] = serde_json::json!("../escape");
        assert!(codec::decode_operation(&serde_json::to_vec(&bad).unwrap(), &k).is_err());
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            s.admit_observed(&key(), input.clone(), |stage, _| {
                if std::mem::discriminant(&stage) == std::mem::discriminant(&cut) {
                    panic!("controlled recursive observation publication cut");
                }
            })
        }));
        assert!(failed.is_err());
        let restored = s.load(&key(), Access::ReadOnly).unwrap();
        let record = restored.domains().unwrap().domain(&i.id).unwrap();
        assert_eq!(
            record.closure_tree_observations.len(),
            usize::from(matches!(cut, Stage::AfterCommit))
        );
        assert!(
            record.cancelled && record.closure.is_none() && record.holds(Resource::WorkerSlots)
        );
        assert_eq!(record.outcome, HistoricalOutcome::Unknown);
        assert!(record.unresolved_consumption(Consumption::CpuExecutionMillis));
        if matches!(cut, Stage::AfterCommit) {
            assert!(s.admit(&key(), input).unwrap().duplicate);
        }
    }
}

// Independent original-PR19 source transcription, not encoder-generated output.
const RECURSIVE_ORIGINAL_GOLDEN: &[u8] = br#"["tree-compat",10,["coord-fixture-program","fixture-program",1,"operator",["A","B","C","D","E","F"],["Amend","Plan","Claim","Review"],[["operator","fixture-operator"],"explicit-local-operator-metadata","1"]],["domain-closure-tree-v1",["closure-tree-observed-v1","domain-fixture-program-A",[[[["synthetic-pool","allocation-1","synthetic-host","boot-1"],"request-fixture-program-A",":1.42","invocation-domain-fixture-program-A","/owned/domain-fixture-program-A",1,42,"boot-1"],[["operator","fixture-operator"],"fixed-bootstrap-profile","1"],["fixture-program",1,"A",[[[47,115,121,110,116,104,101,116,105,99,47,99,111,114,116,101,120,45,112,114,111,103,114,97,109,45,65],[47,115,121,110,116,104,101,116,105,99,47,99,111,114,116,101,120,45,112,114,111,103,114,97,109,45,65,47,46,103,105,116],[47,115,121,110,116,104,101,116,105,99,47,99,111,114,116,101,120,45,112,114,111,103,114,97,109,45,65,47,46,103,105,116]],"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","Clean"],"attempt-A",1,"package-A",1],[7,11],null,[],true,true,7000,256,5,[["operator","fixture-operator"],"controlled-post-cancellation-readback","1"]],[["",1,42,true,[]],["child",1,43,true,[[910001,910002]]]]],5]]]"#;

#[test]
fn recursive_original_literal_preserves_positions_and_rejects_wrong_shapes() {
    let (mut k, i) = domain_fixture::coordinator();
    domain_fixture::observed_cancelled(&mut k, &i);
    assert_eq!(k.revision(), 10);
    let expected = op(
        &k,
        "tree-compat",
        DomainEvent::ClosureTreeObserved {
            domain: i.id.clone(),
            observation: domain_fixture::closure_tree_observation(&i, 5),
            at: 5,
        },
    );
    let decoded = codec::decode_operation(RECURSIVE_ORIGINAL_GOLDEN, &k).unwrap();
    assert!(decoded == expected);
    assert_eq!(
        codec::operation(&expected, k.initial().limits().bytes)
            .unwrap()
            .as_slice(),
        RECURSIVE_ORIGINAL_GOLDEN
    );
    assert_eq!(k.revision(), 10); // Decode does not admit or execute.
    for profile in [
        ProgramProfile::MetadataV1,
        ProgramProfile::FixturesV2,
        ProgramProfile::SupervisedFixturesV3,
    ] {
        assert!(profile.check(&decoded).is_err());
    }
    assert!(ProgramProfile::RecoverableDomainsV4.check(&decoded).is_ok());
    let wire: serde_json::Value = serde_json::from_slice(RECURSIVE_ORIGINAL_GOLDEN).unwrap();
    for case in 0..12 {
        let mut malformed = wire.clone();
        match case {
            0 => malformed[3][0] = serde_json::json!("domain-closure-tree-v2"),
            1 => malformed[3][1][0] = serde_json::json!("closure-tree-observed-v2"),
            2 => malformed[3][0] = serde_json::json!("domain-closure-observation-v1"),
            3 => malformed[3][1][3] = serde_json::json!(6),
            4 => malformed[3][1][2]
                .as_array_mut()
                .unwrap()
                .push(serde_json::Value::Null),
            5 => malformed[3][1][2][1].as_array_mut().unwrap().swap(0, 1),
            6 => malformed[3][1][2][1][1][2] = serde_json::json!(42),
            7 => malformed[3][1][2][1][0][3] = serde_json::json!(false),
            8 => malformed[3][1][2][0][2][2] = serde_json::json!(""),
            9 => malformed[3][1][2][0].as_array_mut().unwrap().swap(3, 4),
            10 => malformed[3][1][2][1][1][4][0]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!(1)),
            _ => malformed[3][1]
                .as_array_mut()
                .unwrap()
                .pop()
                .map(|_| ())
                .unwrap(),
        }
        assert!(
            codec::decode_operation(&serde_json::to_vec(&malformed).unwrap(), &k).is_err(),
            "malformed recursive positional case {case} was accepted"
        );
    }
    let mut whitespace = RECURSIVE_ORIGINAL_GOLDEN.to_vec();
    whitespace.push(b' ');
    assert!(codec::decode_operation(&whitespace, &k).is_err());

    // These same-typed fields may legitimately differ. Their permutation is a
    // different observation, not an invented structural rejection condition.
    let mut permuted = wire;
    permuted[3][1][2][0].as_array_mut().unwrap().swap(8, 9);
    let changed = codec::decode_operation(&serde_json::to_vec(&permuted).unwrap(), &k).unwrap();
    assert!(changed != expected);
    let DomainInput::Domain(event) = changed.command else {
        panic!("domain event")
    };
    let DomainEvent::ClosureTreeObserved { observation, .. } = *event else {
        panic!("tree event")
    };
    assert_eq!(observation.sample.cpu_micros, Some(256));
    assert_eq!(observation.sample.memory_peak, Some(7000));
}

#[test]
fn recursive_literal_replay_retains_admission_identity_without_closure_or_dispatch() {
    let f = Temp::new();
    let (mut k, i) = domain_fixture::coordinator();
    domain_fixture::observed_cancelled(&mut k, &i);
    let s = stored(&k, &f);
    let input = codec::decode_operation(RECURSIVE_ORIGINAL_GOLDEN, &k).unwrap();
    let held: Vec<_> = i
        .effect
        .demand
        .iter()
        .map(|d| {
            (
                d.resource.clone(),
                k.domains().unwrap().held(d.resource.clone()).unwrap(),
            )
        })
        .collect();
    let mut called = false;
    assert!(
        s.dispatch_domain(&key(), input.clone(), |_| {
            called = true;
        })
        .is_err()
    );
    assert!(!called);
    assert_eq!(s.load(&key(), Access::ReadOnly).unwrap().revision(), 10);
    let receipt = s.admit(&key(), input.clone()).unwrap();
    assert_eq!(receipt.store, s.id());
    assert_eq!(receipt.ordinal, 11);
    assert!(!receipt.duplicate);
    let reopened = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    let recovered = reopened.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(recovered.revision(), 11);
    assert_eq!(recovered.operations().len(), 11);
    let record = recovered.domains().unwrap().domain(&i.id).unwrap();
    assert_eq!(record.closure_tree_observations.len(), 1);
    assert!(record.cancelled && record.closure.is_none() && record.settlements.is_empty());
    assert_eq!(record.outcome, HistoricalOutcome::Unknown);
    for (resource, amount) in held {
        assert_eq!(
            recovered.domains().unwrap().held(resource.clone()),
            Ok(amount)
        );
        assert!(record.holds(resource));
    }
    for reservation in &i.consumption {
        assert!(record.unresolved_consumption(reservation.dimension));
    }
    assert_eq!(
        codec::operation(
            recovered.operations().last().unwrap(),
            k.initial().limits().bytes
        )
        .unwrap()
        .as_slice(),
        RECURSIVE_ORIGINAL_GOLDEN
    );
    let duplicate = reopened.admit(&key(), input.clone()).unwrap();
    assert_eq!(duplicate.store, receipt.store);
    assert_eq!(duplicate.ordinal, receipt.ordinal);
    assert!(duplicate.duplicate);
    assert_eq!(
        reopened
            .reconcile(&key(), k.initial(), &input, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Present(duplicate)
    );
    let mut changed =
        serde_json::from_slice::<serde_json::Value>(RECURSIVE_ORIGINAL_GOLDEN).unwrap();
    changed[3][1][2][0].as_array_mut().unwrap().swap(8, 9);
    let changed = codec::decode_operation(&serde_json::to_vec(&changed).unwrap(), &k).unwrap();
    assert!(reopened.admit(&key(), changed.clone()).is_err());
    assert_eq!(
        reopened
            .reconcile(&key(), k.initial(), &changed, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Conflict
    );
    assert_eq!(
        reopened.load(&key(), Access::ReadOnly).unwrap().revision(),
        11
    );
}

#[test]
fn memory_scope_survives_store_reconstruction_and_wrong_pool_refuses() {
    use cortex_core::program::{memory::*, scheduling::*};
    let f = Temp::new();
    let (k, i) = domain_fixture::coordinator();
    let store = stored(&k, &f);
    let actual = store.load(&key(), Access::ReadOnly).unwrap();
    assert!(actual.has_scoped_memory());
    assert_eq!(
        actual.domains().unwrap().pool().unwrap().capacities,
        k.domains().unwrap().pool().unwrap().capacities
    );
    let mut wrong = i.clone();
    let mut other = wrong.pool.clone();
    other.incarnation = "different".into();
    wrong
        .effect
        .demand
        .iter_mut()
        .find(|d| matches!(d.resource, Resource::MemoryBytes(_)))
        .unwrap()
        .resource = Resource::memory(
        MemoryBasis::CgroupCharged,
        MemoryScope::AccountingPool(other),
    )
    .unwrap();
    let before = snapshot(&f);
    assert!(
        store
            .admit(
                &key(),
                op(
                    &actual,
                    "wrong-memory-pool",
                    DomainEvent::Reserve {
                        intent: wrong,
                        at: 1
                    }
                )
            )
            .is_err()
    );
    assert_eq!(snapshot(&f), before);
    store
        .admit(
            &key(),
            op(
                &actual,
                "correct-memory-pool",
                DomainEvent::Reserve { intent: i, at: 1 },
            ),
        )
        .unwrap();
    let recovered = store.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(
        recovered
            .domains()
            .unwrap()
            .held(domain_fixture::memory_resource()),
        Ok(256)
    );
}

#[test]
fn legacy_memory_replays_without_authorizing_a_new_create_or_erasing_bytes() {
    use serde_json::Value;
    fn legacy(v: &mut Value) {
        if v.get(0).and_then(Value::as_str) == Some("memory-resource-v1") {
            *v = Value::String("RamBytes".into());
            return;
        }
        if let Some(a) = v.as_array_mut() {
            for v in a {
                legacy(v);
            }
        }
    }
    let f = Temp::new();
    let (mut k, i) = domain_fixture::coordinator();
    domain_fixture::event(
        &mut k,
        "historical-reserve",
        DomainEvent::Reserve {
            intent: i.clone(),
            at: 1,
        },
    );
    let store =
        ProgramStore::create_domain_fixture(&f.store(), &key(), k.initial().clone(), limits())
            .unwrap();
    // Synthetic old representation written directly: no real historical store/key.
    // The old resource token and all original positional fields remain intact.
    let conn = history_store::connect(&store.location, &key(), Access::Recover).unwrap();
    let mut originals = Vec::new();
    for (n, o) in k.operations().iter().enumerate() {
        let bytes = codec::operation(o, k.initial().limits().bytes).unwrap();
        let mut value: Value = serde_json::from_slice(&bytes).unwrap();
        legacy(&mut value);
        let bytes = serde_json::to_vec(&value).unwrap();
        conn.execute(
            "INSERT INTO program_inputs VALUES(?1,?2,?3)",
            rusqlite::params![n as i64 + 1, o.id.as_str(), &bytes],
        )
        .unwrap();
        originals.push(bytes);
    }
    drop(conn);
    let recovered = store.load(&key(), Access::ReadOnly).unwrap();
    assert!(!recovered.has_scoped_memory());
    for (o, bytes) in recovered.operations().iter().zip(originals) {
        assert_eq!(
            codec::operation(o, recovered.initial().limits().bytes)
                .unwrap()
                .as_slice(),
            bytes
        );
    }
    assert_eq!(
        recovered
            .domains()
            .unwrap()
            .held(Resource::UnspecifiedMemoryBytes),
        Ok(256)
    );
    let before = snapshot(&f);
    let refused = store.admit(
        &key(),
        op(
            &recovered,
            "new-create",
            DomainEvent::CreateRequested {
                domain: i.id.clone(),
                at: 2,
            },
        ),
    );
    assert!(refused.is_err());
    assert_eq!(snapshot(&f), before);
    // Exact idempotent replay remains a receipt, never a second external effect.
    assert!(
        store
            .admit(&key(), recovered.operations().last().unwrap().clone())
            .unwrap()
            .duplicate
    );
    store
        .admit(
            &key(),
            op(
                &recovered,
                "cancel-retained",
                DomainEvent::Cancel {
                    domain: i.id,
                    at: 2,
                    basis: domain_fixture::source("cancel"),
                },
            ),
        )
        .unwrap();
}
