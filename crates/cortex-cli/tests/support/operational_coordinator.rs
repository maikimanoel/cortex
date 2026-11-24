use super::*;
use cortex_cli::program_coordinator::{
    FixtureHold, FixtureOperationIds, FixtureProgress, FixtureTurn, FixtureTurnError, FixtureWork,
    run_fixture_turn,
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
            spec
        },
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
        workspace_observation: ObservationId::new(id("workspace").as_str(), 1).unwrap(),
        dispatch_basis: source("finite-coordinator-turn"),
    }
}

fn observed(turn: &FixtureTurn, job: &FixtureWork) {
    assert_eq!(turn.selected.as_ref(), Some(&job.proposal.task));
    assert_eq!(turn.receipts.len(), 6);
    assert!(turn.receipts.iter().all(|receipt| !receipt.duplicate));
    let run = match &turn.progress {
        Some(FixtureProgress::Observed(run)) => run,
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

#[test]
fn canonical_turn_selects_and_dispatches_after_a_durable_edge_on_reopen() {
    let c = Campaign::new();
    let initial = definition(&c);
    let artifact = initial.graph().edges()[0].requirement().artifact().clone();
    let store =
        ProgramStore::create_supervised_fixture(&c.state, &c.key, initial, read_limits()).unwrap();
    for name in ["A", "B"] {
        let scope = load(&c, &store)
            .state()
            .readiness()
            .scope(&task(name))
            .unwrap();
        notify(
            &c,
            &store,
            &format!("ready-{name}"),
            Event::Preconditions {
                scope,
                ready: true,
                source: source("cooperative-test-prerequisites"),
            },
        );
    }
    notify(
        &c,
        &store,
        "capacity",
        Event::Capacity {
            available: 2,
            source: source("bounded-fixture-slots"),
        },
    );
    effect(&c, &store, "policy", EffectEvent::Policy(policy()));
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
    c.launch_slot();
    let first = run_fixture_turn(&store, &c.key, &work_set, None, deadline, &cancel).unwrap();
    observed(&first, &work_set[0]);
    assert!(first.held.iter().any(|hold| matches!(hold,
        FixtureHold::Waiting(blocked, causes) if blocked == &task("B") && causes.contains(&WaitReason::Preconditions)
    )));
    assert!(!c.output.join("launched-B").exists());
    let retained = load(&c, &store);
    let captured = capture_artifact(
        &retained,
        &work_set[0].effect,
        &artifact,
        &c.output,
        &c.output.join("A"),
        std::slice::from_ref(&c.cortex),
        256,
    )
    .unwrap();
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
