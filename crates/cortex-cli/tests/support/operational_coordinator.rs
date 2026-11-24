use super::*;
use cortex_cli::program_coordinator::{
    FixtureHold, FixtureOperationIds, FixtureProgress, FixtureTurn, FixtureTurnError, FixtureWork,
    FixtureWorkspaceSelection, run_fixture_turn,
};
use std::time::Instant;

fn work(
    c: &Campaign,
    k: &CoordinationKernel,
    name: &str,
    consumed: Option<&CapturedArtifact>,
) -> FixtureWork {
    let task = task(name);
    let current = k.state().readiness().scope(&task).unwrap();
    let candidate = current.context().contract().candidate().clone();
    let id = |step: &str| op(&format!("coordinator-{name}-{step}"));
    let owner = ActorId::new(&fixture_id("operator")).unwrap();
    let scope = EffectScope {
        program: k.state().definition().id().clone(),
        baseline: rev(1),
        task: task.clone(),
        candidate: candidate.clone(),
        attempt: AttemptId::new(id("attempt").as_str()).unwrap(),
        generation: current.generation(),
        package: PackageId::new(id("package").as_str()).unwrap(),
        package_revision: rev(1),
    };
    let package = PackageRequest {
        id: scope.package.clone(),
        revision: scope.package_revision,
        task: task.clone(),
        baseline: scope.baseline,
        generation: scope.generation,
        sources: consumed
            .into_iter()
            .map(|artifact| SourceInput {
                source: SourceReference::operator_reported(
                    "gpe-coordinator-fixture",
                    "accepted-input",
                    artifact.sha256(),
                )
                .unwrap(),
                candidate: candidate.clone(),
                text: String::from_utf8(artifact.bytes().to_vec()).unwrap(),
            })
            .collect(),
        methods: vec![],
        omissions: "Selected cooperative sources only".into(),
        deliverable: "Bounded exact work product".into(),
        stop_rules: "Stop on any unestablished outcome".into(),
    };
    let approval = source("selected-cooperative-coordinator");
    let previous = current.context().capture().unwrap().id();
    let producer_output = c
        .output
        .join(fixture_id("coordinator-A-effect"))
        .join("output");
    FixtureWork {
        proposal: Proposal {
            task,
            band: u32::from(name == "A"),
            waiting_rounds: 0,
            risk_rank: 0,
            cost_rank: 0,
            demand: demand(),
        },
        attempt: Attempt {
            id: scope.attempt.clone(),
            increment: IncrementId::new(&fixture_id(name)).unwrap(),
            owner: owner.clone(),
            baseline: scope.baseline,
            generation: scope.generation,
            status: AttemptStatus::Claimed,
            basis: source("explicit-fixture-responsibility"),
        },
        input: fixture_executor_request(&package),
        package,
        authority: grant(k),
        grant: OperationalGrant {
            id: GrantId::new(id("grant").as_str()).unwrap(),
            principal: owner,
            policy: policy().reference,
            scope,
            not_before: 0,
            expires_before: 240_000,
            ceilings: demand(),
            basis: approval.clone(),
        },
        effect: EffectId::new(id("effect").as_str()).unwrap(),
        process: {
            let mut spec = fixture_process(c, name, name, 0, approval);
            spec.prepared_input = id("input");
            if name == "A" {
                spec.args[2] = producer_output.to_str().unwrap().into();
            } else {
                spec.args[3] = producer_output.to_str().unwrap().into();
            }
            spec
        },
        workspace: (name == "A").then(|| FixtureWorkspaceSelection {
            parent: c.output.clone(),
            additional_sources: vec![c.cortex.clone()],
            output_argument: 2,
            source_observation: ObservationId::new(previous.scope(), previous.sequence() + 1)
                .unwrap(),
        }),
        operations: FixtureOperationIds {
            claim: id("claim"),
            package: id("package"),
            input: id("input"),
            grant: id("grant"),
            reserve: id("reserve"),
            process: id("process"),
            dispatch: id("dispatch"),
        },
        captures: capture_ids(&format!("coordinator-{name}")),
        workspace_observation: ObservationId::new(previous.scope(), previous.sequence() + 2)
            .unwrap(),
        dispatch_basis: source("finite-coordinator-turn"),
    }
}

fn observed(turn: &FixtureTurn, job: &FixtureWork) {
    assert_eq!(turn.selected.as_ref(), Some(&job.proposal.task));
    assert_eq!(turn.receipts.len(), 6);
    assert!(turn.receipts.iter().all(|receipt| !receipt.duplicate));
    let run = match &turn.progress {
        Some(FixtureProgress::Observed(run)) => run.as_ref(),
        Some(FixtureProgress::WorkspaceObserved(run)) => run.fixture(),
        Some(FixtureProgress::Workspace(error)) => panic!("workspace refused: {error:?}"),
        Some(FixtureProgress::Dispatch { error, .. }) => panic!("dispatch refused: {error:?}"),
        Some(FixtureProgress::Admission { failure, .. }) => {
            panic!("admission stopped: {failure:?}")
        }
        Some(FixtureProgress::Budget) => panic!("coordinator exhausted its cut"),
        Some(FixtureProgress::ConcurrentPrefix) => panic!("unexpected concurrent prefix"),
        None => panic!("no selection: {:?}", turn.held),
    };
    assert!(run.run.recording_failure.is_none());
    assert!(run.run.started.is_some() && run.run.recorded.is_some());
    assert!(!run.run.dispatch.duplicate);
    assert_eq!(
        run.workspace.observation().contract().candidate(),
        &job.grant.scope.candidate
    );
    let observation = run.run.observation.as_ref().unwrap();
    let mut spec = job.process.clone();
    spec.input_len = observation.input_bytes;
    assert_eq!(observation.outcome(&spec), Some(EffectOutcome::Succeeded));
    assert!(observation.quiescent(&spec));
    let process = observation.process.as_ref().unwrap();
    assert!(!Path::new(&format!("/proc/{}", process.pid)).exists());
}

fn prepare_turn(c: &Campaign, store: &ProgramStore) {
    for name in ["A", "B"] {
        let scope = load(c, store)
            .state()
            .readiness()
            .scope(&task(name))
            .unwrap();
        notify(
            c,
            store,
            &format!("ready-{name}"),
            Event::Preconditions {
                scope,
                ready: true,
                source: source("cooperative-test-prerequisites"),
            },
        );
    }
    notify(
        c,
        store,
        "capacity",
        Event::Capacity {
            available: 2,
            source: source("bounded-fixture-slots"),
        },
    );
    effect(c, store, "policy", EffectEvent::Policy(policy()));
}

#[test]
fn canonical_turn_selects_and_dispatches_after_a_durable_edge_on_reopen() {
    if std::env::var_os("CORTEX_GPE_CAMPAIGN_ROOT").is_some() {
        // The retained execution mode selects only the original exact fixture
        // population and must not spend its launch slots on this new scenario.
        return;
    }
    let c = Campaign::new();
    let initial = definition(&c);
    let artifact = initial.graph().edges()[0].requirement().artifact().clone();
    let store =
        ProgramStore::create_supervised_fixture(&c.state, &c.key, initial, read_limits()).unwrap();
    prepare_turn(&c, &store);
    let kernel = load(&c, &store);
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(180);
    let work_set = [work(&c, &kernel, "A", None), work(&c, &kernel, "B", None)];
    let before = snapshot(&c.state);
    assert!(matches!(
        run_fixture_turn(&store, &c.key, &work_set, None, Instant::now(), &cancel),
        Err(FixtureTurnError::Budget)
    ));
    assert_eq!(snapshot(&c.state), before);
    let wrong_key = StoreKey::new(&[0x78; 32]).unwrap();
    assert!(matches!(
        run_fixture_turn(&store, &wrong_key, &work_set, None, deadline, &cancel),
        Err(FixtureTurnError::Store(_))
    ));
    assert_eq!(snapshot(&c.state), before);
    c.launch_slot();
    let gate = std::sync::Barrier::new(2);
    let (left, right) = std::thread::scope(|threads| {
        let offer = || {
            gate.wait();
            run_fixture_turn(&store, &c.key, &work_set, None, deadline, &cancel).unwrap()
        };
        let left = threads.spawn(offer);
        let right = threads.spawn(offer);
        (left.join().unwrap(), right.join().unwrap())
    });
    let (first, competitor) =
        if matches!(left.progress, Some(FixtureProgress::WorkspaceObserved(_))) {
            (left, right)
        } else {
            (right, left)
        };
    observed(&first, &work_set[0]);
    assert!(matches!(
        competitor.progress,
        None | Some(FixtureProgress::ConcurrentPrefix)
    ));
    assert_eq!(
        load(&c, &store)
            .operations()
            .iter()
            .filter(|operation| operation.id == work_set[0].operations.dispatch)
            .count(),
        1,
    );
    assert!(first.held.iter().any(|hold| matches!(hold,
        FixtureHold::Waiting(blocked, causes) if blocked == &task("B") && causes.contains(&WaitReason::Preconditions)
    )));
    assert!(!c.output.join("launched-B").exists());
    let retained = load(&c, &store);
    let Some(FixtureProgress::WorkspaceObserved(producer)) = &first.progress else {
        panic!("producer was not observed in its fresh output slot");
    };
    assert!(!c.output.join("A").exists());
    let captured = producer
        .capture(
            &store,
            &c.key,
            &artifact,
            "a1a410bd6438bb73e09ee3220cf8a87470d1d6401757f68be666fd8399e78df0",
            256,
            ObservationId::new(
                work_set[0].workspace_observation.scope(),
                work_set[0].workspace_observation.sequence() + 1,
            )
            .unwrap(),
        )
        .unwrap()
        .content;
    assert_eq!(captured.bytes(), ARTIFACT);
    settle(
        &c,
        &store,
        &work_set[0].grant.scope,
        &work_set[0].effect,
        "A",
    );
    let waiting = run_fixture_turn(
        &store,
        &c.key,
        &work_set,
        Some(&first.cursor),
        deadline,
        &cancel,
    )
    .unwrap();
    assert!(waiting.progress.is_none());
    assert!(
        waiting
            .held
            .iter()
            .any(|hold| matches!(hold, FixtureHold::Retained(id) if id == &task("A")))
    );
    assert!(!c.output.join("launched-B").exists());

    let independent = observe(&c, &store, "A", captured.bytes(), ARTIFACT, Some(&captured));
    assess(&c, &store, &independent, "A");
    let report = NotificationId::new(&fixture_id("artifact-A-v1")).unwrap();
    notify(
        &c,
        &store,
        "artifact-A-v1",
        Event::Artifact(ArtifactReport::new(
            retained.state().readiness().scope(&task("A")).unwrap(),
            artifact,
            DependencyKind::Artifact,
            ReadinessLevel::ContractStable,
            source("bounded-producer-capture"),
        )),
    );
    let release_input = release(&load(&c, &store), &independent, &report);
    let before_release =
        run_fixture_turn(&store, &c.key, &work_set, None, deadline, &cancel).unwrap();
    assert!(before_release.selected.is_none());
    let receipt = admit(
        &c,
        &store,
        "release-AB",
        Input::Release(Box::new(release_input)),
    );
    drop(store);
    let store = ProgramStore::open(&c.state, &c.key, Access::Recover, read_limits()).unwrap();
    let kernel = load(&c, &store);
    assert_eq!(kernel.revision(), receipt.ordinal);
    let work_set = [
        work(&c, &kernel, "A", None),
        work(&c, &kernel, "B", Some(&captured)),
    ];
    c.launch_slot();
    let second = run_fixture_turn(
        &store,
        &c.key,
        &work_set,
        Some(&before_release.cursor),
        deadline,
        &cancel,
    )
    .unwrap();
    observed(&second, &work_set[1]);
    assert!(
        second.held.is_empty(),
        "only the released edge's consumer should wake"
    );
    assert_eq!(fs::read(c.output.join("B.consumed")).unwrap(), ARTIFACT);
    let product = fs::read(c.output.join("B")).unwrap();
    assert_eq!(product, CONSUMED);
    let independent_b = observe(&c, &store, "B", &product, CONSUMED, None);
    assess(&c, &store, &independent_b, "B");
    let count = load(&c, &store).revision();
    let repeated = run_fixture_turn(
        &store,
        &c.key,
        &work_set,
        Some(&second.cursor),
        deadline,
        &cancel,
    )
    .unwrap();
    assert!(repeated.progress.is_none());
    assert_eq!(load(&c, &store).revision(), count);
    let idle = run_fixture_turn(
        &store,
        &c.key,
        &work_set,
        Some(&repeated.cursor),
        deadline,
        &cancel,
    )
    .unwrap();
    assert!(idle.progress.is_none() && idle.held.is_empty());
}

#[test]
fn coordinator_retains_pre_dispatch_prefix_and_reopen_never_launches_it() {
    if std::env::var_os("CORTEX_GPE_CAMPAIGN_ROOT").is_some() {
        return;
    }
    let c = Campaign::new();
    let store =
        ProgramStore::create_supervised_fixture(&c.state, &c.key, definition(&c), read_limits())
            .unwrap();
    prepare_turn(&c, &store);
    let before = load(&c, &store);
    let jobs = [work(&c, &before, "A", None), work(&c, &before, "B", None)];
    let cancel = AtomicBool::new(false);
    // This envelope admits fixture metadata but can never include the 10 s
    // worker wall budget. No payload is started before the durable cut.
    let cut = run_fixture_turn(
        &store,
        &c.key,
        &jobs,
        None,
        Instant::now() + Duration::from_secs(9),
        &cancel,
    )
    .unwrap();
    assert_eq!(cut.selected, Some(task("A")));
    assert!(matches!(cut.progress, Some(FixtureProgress::Budget)));
    assert_eq!(cut.receipts.len(), 6);
    assert!(
        cut.receipts
            .iter()
            .all(|r| r.store == store.id() && !r.duplicate)
    );
    let ordinal = cut.receipts.last().unwrap().ordinal;
    assert_eq!(ordinal, before.revision() + 6);
    drop(store);
    let reopened = ProgramStore::open(&c.state, &c.key, Access::Recover, read_limits()).unwrap();
    let saved = load(&c, &reopened);
    assert_eq!(saved.revision(), ordinal);
    let record = &saved.fixture_effects().effects()[0];
    assert_eq!(record.intent.id, jobs[0].effect);
    assert_eq!(record.status, EffectStatus::Reserved);
    assert!(record.process_spec.is_some() && record.dispatch_at.is_none());
    assert!(record.process_observations.is_empty());
    assert!(
        saved
            .operations()
            .iter()
            .all(|op| op.id != jobs[0].operations.dispatch)
    );
    let current = snapshot(&c.state);
    let again = run_fixture_turn(
        &reopened,
        &c.key,
        &jobs,
        None,
        Instant::now() + Duration::from_secs(180),
        &cancel,
    )
    .unwrap();
    assert!(again.selected.is_none() && again.receipts.is_empty());
    assert!(again.progress.is_none());
    assert!(
        again
            .held
            .iter()
            .any(|h| matches!(h, FixtureHold::Retained(id) if *id == task("A")))
    );
    assert_eq!(snapshot(&c.state), current);
    assert!(!c.output.join("launched-A").exists());
    let slot = c.output.join(jobs[0].effect.as_str());
    assert_eq!(fs::read_dir(slot).unwrap().count(), 0);
}
