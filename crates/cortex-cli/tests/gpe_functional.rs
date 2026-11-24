#![cfg(target_os = "linux")]
//! GPE-FUNC-2T Q0-Q7: real trusted artifacts through the existing production owners.
#[path = "support/gpe_admission.rs"]
mod gpe_admission;
#[path = "support/gpe_evaluation.rs"]
mod gpe_evaluation;
#[path = "support/gpe_identity.rs"]
mod gpe_identity;
#[path = "support/program_composition.rs"]
mod program_composition;
fn fixture_namespace() -> Option<&'static gpe_identity::FixtureNamespace> {
    static NAMESPACE: std::sync::OnceLock<Option<gpe_identity::FixtureNamespace>> =
        std::sync::OnceLock::new();
    NAMESPACE
        .get_or_init(|| {
            std::env::var_os("CORTEX_GPE_FIXTURE_NAMESPACE").map(|value| {
                gpe_identity::FixtureNamespace::from_os(&value)
                    .expect("invalid fixture namespace before effects")
            })
        })
        .as_ref()
}
fn fixture_id(label: &str) -> String {
    fixture_namespace().map_or_else(
        || label.to_owned(),
        |namespace| namespace.qualify(label).expect("bounded fixture identity"),
    )
}
use cortex_cli::{
    inspect_candidate,
    program_execution::{
        ExecutionError, FixtureArtifactRequest, PreparedFixtureRun, capture_fixture_output,
        run_prepared_fixture,
    },
    program_preparation::{ExecutorPreparation, prepare_executor_input},
};
use cortex_core::{
    CandidateId, TaskContract, TaskId,
    binding::*,
    parallel::*,
    program::{context::*, executor::*, operational::*, scheduling::*, supervision::*, *},
    temporal::SourceReference,
};
use cortex_evidence::{
    ExecutionObservation, Provenance,
    program::{Command as Input, *},
    readiness::*,
    unified::*,
};
use cortex_harness::{
    CaptureIds, PinnedFixture,
    artifact::{ArtifactError, CapturedArtifact, capture_artifact},
    execute_fixture, inspect_fixture,
};
use cortex_storage as admission_storage;
use cortex_storage::{
    history_store::{Access, Completion, StoreKey},
    program_store::{AdmissionReceipt, ProgramStore, ReadLimits},
};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Duration, SystemTime},
};
use zeroize::Zeroize;

// Established independently of the fixture's prime generation and reduction code.
const ARTIFACT: &[u8] = b"cortex-artifact-v1\n2\n3\n5\n7\n";
const CONSUMED: &[u8] = b"cortex-consumer-v1\ncount=4\nsum=17\nsquares=87\n";
const MAX_BYTES: usize = 8_000_000;
fn rev(n: u64) -> ContractRevision {
    ContractRevision::new(n).unwrap()
}
fn task(id: &str) -> TaskId {
    TaskId::new(&fixture_id(id)).unwrap()
}
fn op(id: &str) -> OperationId {
    OperationId::new(&fixture_id(id)).unwrap()
}
fn source(id: &str) -> SourceReference {
    SourceReference::operator_reported("gpe-fixture-controller", &fixture_id(id), "1").unwrap()
}
fn read_limits() -> ReadLimits {
    ReadLimits::new(MAX_BYTES, Duration::from_millis(500)).unwrap()
}
fn private_dir(path: &Path) {
    fs::create_dir(path).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
fn private_file(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}
fn git(path: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    let output = command
        .current_dir(path)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_DATE", "2001-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2001-01-01T00:00:00Z")
        .output()
        .unwrap();
    assert!(output.status.success(), "fixture Git operation refused");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
fn repository(root: &Path) -> RepositoryIdentity {
    RepositoryIdentity::new(
        root.canonicalize().unwrap(),
        PathBuf::from(git(root, &["rev-parse", "--absolute-git-dir"])),
        PathBuf::from(git(
            root,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )),
    )
    .unwrap()
}
struct Campaign {
    root: PathBuf,
    retained: bool,
    fixture: PathBuf,
    output: PathBuf,
    state: PathBuf,
    binary: PathBuf,
    evidence: PathBuf,
    cortex: RepositoryIdentity,
    key_bytes: [u8; 32],
    key: StoreKey,
}
impl Campaign {
    fn new() -> Self {
        let namespace = fixture_namespace();
        let configured = std::env::var_os("CORTEX_GPE_CAMPAIGN_ROOT").map(PathBuf::from);
        let retained = configured.is_some();
        let root = match configured {
            Some(root) => {
                if !root.exists() {
                    private_dir(&root);
                }
                root
            }
            None => {
                static NEXT: AtomicU64 = AtomicU64::new(0);
                let root = std::env::temp_dir().join(format!(
                    "cortex-gpe-func-2t-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                private_dir(&root);
                root
            }
        };
        let root = root.canonicalize().unwrap();
        if let Some(namespace) = namespace {
            namespace
                .claim(&root)
                .expect("fresh namespace required; no retry");
        }
        for name in [
            "fixture-source",
            "fixture-output",
            "state",
            "controller",
            "build",
            "evidence",
            "quarantine",
        ] {
            if !root.join(name).exists() {
                private_dir(&root.join(name));
            }
        }
        // Slots survive failures and concurrent callers; an invocation is never refunded.
        let slot = (1..=16)
            .find(|number| {
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(root.join(format!("evidence/invocation-{number:02}")))
                    .is_ok()
            })
            .expect("scenario allowance exhausted");
        let id = format!("run-{slot:02}");
        let fixture = root.join("fixture-source").join(&id);
        let output = root.join("fixture-output").join(&id);
        let state = root.join("state").join(&id);
        let build = root.join("build").join(&id);
        let evidence = root.join("evidence").join(&id);
        for path in [&fixture, &output, &build, &evidence] {
            private_dir(path);
        }
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gpe.rs"),
            fixture.join("main.rs"),
        )
        .unwrap();
        git(&fixture, &["init", "--initial-branch=main"]);
        git(&fixture, &["config", "user.name", "Cortex Fixture"]);
        git(
            &fixture,
            &["config", "user.email", "fixture@example.invalid"],
        );
        git(&fixture, &["config", "commit.gpgSign", "false"]);
        git(&fixture, &["config", "gc.autoDetach", "false"]);
        git(&fixture, &["add", "main.rs"]);
        git(
            &fixture,
            &["commit", "-m", "Finite trusted two-task fixture"],
        );
        let binary = build.join("gpe-fixture");
        let compilation = Command::new("rustc")
            .current_dir(&fixture)
            .args([
                "--edition=2024",
                "-C",
                "opt-level=1",
                "-C",
                "debuginfo=0",
                "-C",
                "strip=symbols",
                "-o",
            ])
            .arg(&binary)
            .arg(fixture.join("main.rs"))
            .output()
            .unwrap();
        assert!(
            compilation.status.success(),
            "trusted fixture compilation failed"
        );
        assert!(
            fs::metadata(&binary).unwrap().len() < 1_000_000,
            "fixture package exceeds accepted executable profile"
        );
        let cortex = repository(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(2)
                .unwrap(),
        );
        let mut key_bytes = [0; 32];
        fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut key_bytes)
            .unwrap();
        let key = StoreKey::new(&key_bytes).unwrap();
        Self {
            root,
            retained,
            fixture,
            output,
            state,
            binary,
            evidence,
            cortex,
            key_bytes,
            key,
        }
    }
    fn record(&self, name: &str, detail: &str) {
        private_file(&self.evidence.join(name), detail.as_bytes());
    }
    fn mark(&self, q: &str) {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.evidence.join("observations.txt"))
            .unwrap()
            .write_all(format!("{q}: IMPLEMENTED_AND_VERIFIED\n").as_bytes())
            .unwrap();
    }
    fn launch_slot(&self) {
        (1..=if fixture_namespace().is_some() { 2 } else { 32 })
            .find(|number| {
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(
                        self.root
                            .join(format!("evidence/fixture-dispatch-{number:02}")),
                    )
                    .is_ok()
            })
            .expect("fixture launch allowance exhausted");
    }
    fn cli(&self, schema: Option<&str>, machine: bool) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cortex"));
        command
            .args(["program", "inspect", "--store"])
            .arg(&self.state)
            .args([
                "--program",
                &fixture_id("gpe-func-2t"),
                "--key-fd",
                "0",
                "--max-bytes",
                "8000000",
                "--read-timeout-ms",
                "10000",
            ]);
        if machine {
            command.arg("--json");
        }
        if let Some(schema) = schema {
            command.args(["--schema-version", schema]);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&self.key_bytes)
            .unwrap();
        child.wait_with_output().unwrap()
    }
}
impl Drop for Campaign {
    fn drop(&mut self) {
        // StoreKey separately zeroizes its retained key. These synthetic transfer
        // bytes never reach files, arguments, worker input or diagnostic output.
        self.key_bytes.zeroize();
        if !self.retained {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
fn definition(c: &Campaign) -> ProgramDefinition {
    let repository = repository(&c.fixture);
    let candidate = CandidateId::from_git_commit(&git(&c.fixture, &["rev-parse", "HEAD"])).unwrap();
    let tasks: Vec<_> = ["A", "B"].into_iter().map(|name| {
        let contract = CandidateBindingContract::new(TaskContract::new(task(name),
            if name == "A" { "Produce the version-one prime artifact increment" } else { "Consume the exact released artifact in a separately admitted trusted fixture" },
            if name == "A" { "Exact artifact bytes cortex-artifact-v1 followed by 2, 3, 5, 7 lines" } else { "Exact count 4, sum 17 and squared sum 87 from the released version-one artifact" }).unwrap(),
            rev(1), CandidateRequirement::new(repository.clone(), candidate.clone(), WorktreeRequirement::Clean));
        let capture = inspect_candidate(&c.fixture, &contract, ObservationId::new(&fixture_id(name), 1).unwrap()).unwrap();
        Task::new(TaskContext::new(contract, Some(capture)).unwrap(), ConsumerKind::Preview, DependencyCoverage::DeclaredComplete)
    }).collect();
    let artifact = ArtifactReference::new(
        ArtifactId::new(&fixture_id("prime-list")).unwrap(),
        rev(1),
        candidate,
    );
    let edge = DependencyEdge::new(
        EdgeId::new(&fixture_id("AB")).unwrap(),
        task("A"),
        task("B"),
        ArtifactRequirement::new(
            artifact.clone(),
            DependencyKind::Artifact,
            ReadinessLevel::ContractStable,
            vec![PredicateId::new(&fixture_id("exact-prime-bytes-v1")).unwrap()],
        )
        .unwrap(),
    );
    ProgramDefinition::new(ProgramId::new(&fixture_id("gpe-func-2t")).unwrap(),
        ProgramOutcome::new("Demonstrate trusted two-task functional integration", "Produce, independently assess, release, reconstruct and consume one exact artifact; preserve invalidation and uncertainty").unwrap(),
        rev(1), None, source("owner-approved-fixture-baseline"),
        DependencyGraph::new(GraphReference::new(GraphId::new(&fixture_id("gpe-func-2t-graph")).unwrap(), rev(1)), tasks, vec![edge],
            GraphLimits::new(2, 1, 256, 1024, MAX_BYTES).unwrap()).unwrap(),
        vec![Workstream { id: WorkstreamId::new(&fixture_id("functional-integration")).unwrap(), responsibility: "Trusted fixture production and consumption".into(), tasks: vec![task("A"), task("B")] }],
        ["A", "B"].into_iter().map(|name| Increment { id: IncrementId::new(&fixture_id(name)).unwrap(), task: task(name),
            artifacts: if name == "A" { vec![artifact.clone()] } else { vec![] }, claim_scopes: vec![ClaimScopeId::new(&fixture_id(name)).unwrap()] }).collect(),
        ProgramLimits::new(3, 256, MAX_BYTES).unwrap()).unwrap()
}
fn grant(k: &CoordinationKernel) -> CoordinationGrant {
    CoordinationGrant {
        id: GrantId::new(&fixture_id("local-coordinator-v1")).unwrap(),
        program: k.state().definition().id().clone(),
        baseline: rev(1),
        actor: ActorId::new(&fixture_id("operator")).unwrap(),
        tasks: vec![task("A"), task("B")],
        permissions: vec![
            CoordinationPermission::Plan,
            CoordinationPermission::Claim,
            CoordinationPermission::Review,
        ],
        basis: source("explicit-campaign-approval"),
    }
}
fn operation(k: &CoordinationKernel, id: &str, command: Input) -> Operation {
    Operation {
        id: op(id),
        expected: k.revision(),
        grant: grant(k),
        command,
    }
}
fn load(c: &Campaign, store: &ProgramStore) -> CoordinationKernel {
    store.load(&c.key, Access::ReadOnly).unwrap()
}
fn admit(c: &Campaign, store: &ProgramStore, id: &str, command: Input) -> AdmissionReceipt {
    let before = load(c, store);
    let input = operation(&before, id, command);
    admit_from(c, store, &before, input, AdmissionReadback::Metadata)
}
fn admit_exact(c: &Campaign, store: &ProgramStore, input: Operation) -> AdmissionReceipt {
    let before = load(c, store);
    admit_from(c, store, &before, input, AdmissionReadback::Metadata)
}
enum AdmissionReadback {
    Metadata,
    SyntheticReservation,
}

fn admit_from(
    c: &Campaign,
    store: &ProgramStore,
    before: &CoordinationKernel,
    input: Operation,
    readback: AdmissionReadback,
) -> AdmissionReceipt {
    let id = input.id.as_str();
    let expected = input.expected;
    let mut diagnostics =
        cortex_storage::program_store::diagnostics::AdmissionDiagnostics::default();
    let result = store.admit_with_diagnostics(&c.key, input.clone(), &mut diagnostics);
    // Only this owned fixture's fixed synthetic operation labels are emitted.
    // Formatting and file I/O happen after admission has released its lock.
    let preceding_launch_slots = fs::read_dir(c.root.join("evidence"))
        .and_then(|mut entries| {
            entries.try_fold(0usize, |count, entry| {
                entry.map(|entry| {
                    count
                        + usize::from(
                            entry
                                .file_name()
                                .to_string_lossy()
                                .starts_with("fixture-dispatch-"),
                        )
                })
            })
        })
        .map_err(|error| error.kind());
    let text = format!(
        "operation={id} expected={expected} preceding_dispatch_slots={preceding_launch_slots:?} diagnostics={diagnostics:?}\nadmission={result:?}\n"
    );
    let path = c.evidence.join(format!("admission-{id}.txt"));
    let retained = fs::write(&path, &text)
        .and_then(|()| fs::set_permissions(&path, fs::Permissions::from_mode(0o600)))
        .map_err(|error| error.kind());
    // Report emission must not hide an uncertain admission behind an I/O panic.
    let observed = match readback {
        AdmissionReadback::Metadata => {
            gpe_admission::confirm_metadata(result, &input, store.id(), || {
                store.reconcile(&c.key, before.initial(), &input, Access::ReadOnly)
            })
        }
        AdmissionReadback::SyntheticReservation => {
            assert_eq!(input.id, op("reserve-synthetic-unknown"));
            gpe_admission::confirm_synthetic_reservation(
                result,
                &input,
                store.id(),
                before,
                store,
                &c.key,
            )
        }
    }
    .unwrap_or_else(|failure| {
        panic!("{text}confirmation={failure}; diagnostic_retention={retained:?}")
    });
    let receipt = match observed {
        gpe_admission::ObservedAdmission::Admitted(receipt) => receipt,
        gpe_admission::ObservedAdmission::Confirmed { original, receipt } => {
            let confirmation = format!(
                "original_admission={original:?}\ncanonical_presence={receipt:?}\nno_admission_retry=true\n"
            );
            let path = c.evidence.join(format!("confirmation-{id}.txt"));
            let written = fs::write(&path, &confirmation)
                .and_then(|()| fs::set_permissions(&path, fs::Permissions::from_mode(0o600)))
                .map_err(|error| error.kind());
            assert!(
                written.is_ok(),
                "{text}{confirmation}confirmation_retention={written:?}"
            );
            receipt
        }
    };
    assert!(
        retained.is_ok(),
        "fixture admission observation; diagnostic_retention={retained:?}; {text}"
    );
    assert!(
        preceding_launch_slots.is_ok(),
        "fixture admission observation; dispatch count unavailable; {text}"
    );
    receipt
}
fn effect(c: &Campaign, store: &ProgramStore, id: &str, event: EffectEvent) {
    admit(c, store, id, Input::SupervisedEffect(Box::new(event)));
}
fn notify(c: &Campaign, store: &ProgramStore, id: &str, event: Event) {
    let k = load(c, store);
    let notification = Notification::new(
        NotificationId::new(&fixture_id(id)).unwrap(),
        k.state().definition().graph().reference().clone(),
        event,
    )
    .unwrap();
    admit(c, store, id, Input::Notify(notification));
}
fn policy() -> Policy {
    Policy {
        reference: source("direct-child-policy"),
        program: ProgramId::new(&fixture_id("gpe-func-2t")).unwrap(),
        baseline: rev(1),
        mode: EffectMode::SupervisedFixture,
        host: source("trusted-local-host"),
        privacy: source("synthetic-private-manual-retention"),
        capacities: [
            (Resource::WorkerSlots, 3, 1),
            (Resource::ElapsedMillis, 120_000, 30_000),
            (Resource::OutputBytes, 32_768, 8192),
        ]
        .into_iter()
        .map(|(resource, available, verification_reserve)| Capacity {
            resource,
            available: Some(available),
            verification_reserve,
            declared_enforceable: true,
        })
        .collect(),
        cumulative: vec![Resource::ElapsedMillis, Resource::OutputBytes],
        max_effects: 4,
        max_retries: 0,
    }
}
fn demand() -> Vec<Demand> {
    [
        (Resource::WorkerSlots, 1),
        (Resource::ElapsedMillis, 10_500),
        (Resource::OutputBytes, 4096),
    ]
    .into_iter()
    .map(|(resource, amount)| Demand {
        resource,
        amount: Some(amount),
    })
    .collect()
}
fn fixture_executor_request(package: &PackageRequest) -> ExecutorInputRequest {
    let descriptor = ExecutorDescriptor::new(
        source("finite-rust-descriptor"),
        ExecutorIdentity::new(
            "gpe-trusted-rust-fixture",
            "1",
            source("stripped-owned-fixture"),
            policy().host,
        )
        .unwrap(),
        Transport::StandardInputJsonLines,
        [Control::StandardInput, Control::StructuredOutput]
            .into_iter()
            .map(|control| CapabilityObservation {
                control,
                kind: cortex_core::program::executor::ObservationKind::ControlledFixture,
                support: Support::Supported,
                basis: source("trusted-direct-child-development"),
            })
            .collect(),
        2,
    )
    .unwrap();
    let requirements = Requirements::new(
        source("trusted-direct-child-profile"),
        [Control::StandardInput, Control::StructuredOutput]
            .into_iter()
            .map(|control| Requirement {
                control,
                kind: cortex_core::program::executor::ObservationKind::ControlledFixture,
            })
            .collect(),
        2,
    )
    .unwrap();
    ExecutorInputRequest {
        package: package.id.clone(),
        revision: package.revision,
        descriptor,
        requirements,
        max_bytes: 65_536,
    }
}

fn fixture_process(
    c: &Campaign,
    name: &str,
    label: &str,
    input_len: u64,
    approval: SourceReference,
) -> ProcessSpec {
    ProcessSpec {
        reference: source(&format!("process-{label}")),
        executable: c.binary.clone(),
        executable_identity: source("stripped-owned-fixture"),
        cwd: c.fixture.clone(),
        args: vec![
            if name == "A" {
                "produce".into()
            } else {
                "consume".into()
            },
            c.fixture.to_str().unwrap().into(),
            c.output.join(label).to_str().unwrap().into(),
            c.output.join("A").to_str().unwrap().into(),
            c.output
                .join(format!("launched-{label}"))
                .to_str()
                .unwrap()
                .into(),
        ],
        prepared_input: op(&format!("prepared-{label}")),
        input_len,
        host: policy().host,
        profile: source("trusted-direct-child-profile"),
        privacy: policy().privacy,
        approval,
        limits: ProcessLimits {
            wall_ms: 10_000,
            cleanup_ms: 500,
            input_bytes: 65_536,
            output_bytes: 4096,
            frame_bytes: 1024,
            authority_poll_ms: 20,
            executable_bytes: 1_000_000,
            arguments: 5,
            argument_bytes: 4096,
            transport: Some(TransportLimits {
                startup_ms: 5000,
                idle_ms: 5000,
                stdout_bytes: 4096,
                stderr_bytes: 1024,
                records: 2,
                buffer_bytes: 131_072,
            }),
        },
        containment: None,
        identity_checkpoint: None,
    }
}

#[path = "support/operational_coordinator.rs"]
mod operational_coordinator;

fn prepare(
    c: &Campaign,
    store: &ProgramStore,
    name: &str,
    label: &str,
    consumed: Option<&CapturedArtifact>,
) -> (
    EffectScope,
    EffectId,
    ProcessSpec,
    PinnedFixture,
    ExecutorPreparation,
) {
    let k = load(c, store);
    let scope = k.state().readiness().scope(&task(name)).unwrap();
    let attempt = Attempt {
        id: AttemptId::new(&fixture_id(&format!("attempt-{label}"))).unwrap(),
        increment: IncrementId::new(&fixture_id(name)).unwrap(),
        owner: ActorId::new(&fixture_id("operator")).unwrap(),
        baseline: rev(1),
        generation: scope.generation(),
        status: AttemptStatus::Claimed,
        basis: source("separate-explicit-claim"),
    };

    let sources = if name == "B" {
        let retained = consumed.expect("consumer requires an actual retained producer capture");
        let current = capture_artifact(
            &k,
            retained.effect(),
            retained.artifact(),
            &c.output,
            &c.output.join("A"),
            std::slice::from_ref(&c.cortex),
            256,
        )
        .unwrap();
        assert_eq!(current.bytes(), retained.bytes());
        assert_eq!(current.sha256(), retained.sha256());
        vec![SourceInput {
            source: SourceReference::operator_reported(
                "gpe-fixture-controller",
                "released-prime-artifact-v1",
                retained.sha256(),
            )
            .unwrap(),
            candidate: scope.context().contract().candidate().clone(),
            text: String::from_utf8(retained.bytes().to_vec()).unwrap(),
        }]
    } else {
        vec![]
    };
    let package = PackageRequest { id: PackageId::new(&fixture_id(&format!("package-{label}"))).unwrap(), revision: rev(1), task: task(name), baseline: rev(1), generation: scope.generation(),
        sources,
        methods: vec![], omissions: "Only audited local synthetic fixture source and explicit artifact; no provider or containment qualification".into(), deliverable: "Bounded versioned artifact bytes in an explicit private output directory".into(), stop_rules: "Stop on changed correspondence, cancellation, timeout or unknown outcome; no automatic retry".into() };
    let current = inspect_candidate(
        &c.fixture,
        scope.context().contract(),
        ObservationId::new(
            scope.context().capture().unwrap().id().scope(),
            scope.context().capture().unwrap().id().sequence() + 1,
        )
        .unwrap(),
    )
    .unwrap();
    let profile = verification_context(&k, name, None).1;
    let package = program_composition::advisory_package(k.state(), package, &current, &profile);
    let claim = operation(&k, &format!("claim-{label}"), Input::Claim(attempt.clone()));
    let mut package_operation = operation(
        &k,
        &format!("package-{label}"),
        Input::Package(package.clone()),
    );
    package_operation.expected = claim.expected.checked_add(1).unwrap();
    let before = snapshot(&c.state);
    let operations = if label == "synthetic-unknown" {
        // This explicit synthetic control attempt is not a scheduling selection.
        vec![claim, package_operation]
    } else {
        use cortex_cli::program_preparation::{TaskOperations, prepare};
        let proposal = Proposal {
            task: task(name),
            band: 0,
            waiting_rounds: 0,
            risk_rank: 0,
            cost_rank: 0,
            demand: demand(),
        };
        let prepared = prepare(
            &k,
            k.revision(),
            &[proposal],
            &[TaskOperations {
                task: task(name),
                candidate: scope.context().contract().candidate().clone(),
                operations: Some([claim, package_operation]),
            }],
        )
        .unwrap();
        assert_eq!(prepared.operations().len(), 2);
        assert!(prepared.decisions()[0].scheduling.proposed);
        assert!(prepared.decisions()[0].coordination_hold.is_none());
        prepared.into_operations()
    };
    assert_eq!(snapshot(&c.state), before);
    assert_eq!(load(c, store).revision(), k.revision());
    for input in operations {
        admit_exact(c, store, input);
    }
    let journal = load(c, store);
    let prepared_operation = operation(
        &journal,
        &format!("prepared-{label}"),
        Input::PrepareExecutorInput(Box::new(fixture_executor_request(&package))),
    );
    let preparation = prepare_executor_input(
        &journal,
        journal.revision(),
        &task(name),
        scope.context().contract().candidate(),
        prepared_operation,
    )
    .unwrap();
    admit_exact(c, store, preparation.operation().clone());
    let scope = EffectScope {
        program: k.state().definition().id().clone(),
        baseline: rev(1),
        task: task(name),
        candidate: scope.context().contract().candidate().clone(),
        attempt: attempt.id,
        generation: scope.generation(),
        package: package.id,
        package_revision: rev(1),
    };
    if let Some(namespace) = fixture_namespace() {
        namespace
            .check_attempt(
                scope.program.as_str(),
                scope.task.as_str(),
                scope.attempt.as_str(),
                scope.package.as_str(),
            )
            .unwrap();
    }
    let k = load(c, store);
    let operational = OperationalGrant {
        id: GrantId::new(&fixture_id(&format!("operational-{label}"))).unwrap(),
        principal: ActorId::new(&fixture_id("operator")).unwrap(),
        policy: policy().reference,
        scope: scope.clone(),
        not_before: k.fixture_effects().last_tick(),
        expires_before: 240_000,
        ceilings: demand(),
        basis: source("owner-authorized-finite-fixture"),
    };
    effect(
        c,
        store,
        &format!("grant-{label}"),
        EffectEvent::Grant(operational.clone()),
    );
    let id = EffectId::new(&fixture_id(&format!("effect-{label}"))).unwrap();
    let before_reservation = load(c, store);
    let reservation = operation(
        &before_reservation,
        &format!("reserve-{label}"),
        Input::SupervisedEffect(Box::new(EffectEvent::Reserve {
            intent: EffectIntent {
                id: id.clone(),
                grant: operational.id,
                scope: scope.clone(),
                demand: demand(),
                retry_of: None,
            },
            at: before_reservation.fixture_effects().last_tick(),
        })),
    );
    let readback = if label == "synthetic-unknown" {
        AdmissionReadback::SyntheticReservation
    } else {
        AdmissionReadback::Metadata
    };
    admit_from(c, store, &before_reservation, reservation, readback);
    let k = load(c, store);
    let prepared = &k
        .prepared_inputs()
        .iter()
        .find(|(id, _)| id == &op(&format!("prepared-{label}")))
        .unwrap()
        .1;
    let spec = fixture_process(
        c,
        name,
        label,
        prepared.bytes().len() as u64,
        operational.basis,
    );
    let pin = inspect_fixture(&spec, prepared, k.state().definition().limits()).unwrap();
    (scope, id, spec, pin, preparation)
}
fn process(
    c: &Campaign,
    store: &ProgramStore,
    scope: &EffectScope,
    id: &EffectId,
    spec: &ProcessSpec,
    label: &str,
) {
    effect(
        c,
        store,
        &format!("process-{label}"),
        EffectEvent::PrepareProcess {
            effect: id.clone(),
            scope: scope.clone(),
            at: load(c, store).fixture_effects().last_tick(),
            spec: Box::new(spec.clone()),
        },
    );
}
fn dispatch(k: &CoordinationKernel, scope: &EffectScope, id: &EffectId, label: &str) -> Operation {
    operation(
        k,
        &format!("dispatch-{label}"),
        Input::SupervisedEffect(Box::new(EffectEvent::Dispatch {
            effect: id.clone(),
            scope: scope.clone(),
            at: k.fixture_effects().last_tick(),
            basis: source("durable-fresh-dispatch"),
        })),
    )
}
fn capture_ids(label: &str) -> CaptureIds {
    CaptureIds {
        started_operation: op(&format!("started-{label}")),
        finished_operation: op(&format!("finished-{label}")),
        started_observation: ObservationId::new(&fixture_id(&format!("process-{label}")), 1)
            .unwrap(),
        finished_observation: ObservationId::new(&fixture_id(&format!("process-{label}")), 2)
            .unwrap(),
        basis: source("native-harness-v1"),
        admission_attempts: 2,
    }
}
fn run(
    c: &Campaign,
    store: &ProgramStore,
    scope: &EffectScope,
    id: &EffectId,
    spec: &ProcessSpec,
    fixture: (&PinnedFixture, &ExecutorPreparation),
    label: &str,
) -> Operation {
    let kernel = load(c, store);
    let dispatch = dispatch(&kernel, scope, id, label);
    let accepted = c.evidence.join("accepted-fixture-executable");
    let executable = fs::read(&spec.executable).unwrap();
    assert!(executable.len() <= spec.limits.executable_bytes as usize);
    if accepted.exists() {
        assert_eq!(fs::read(&accepted).unwrap(), executable);
    } else {
        private_file(&accepted, &executable);
    }
    let manifest = serde_json::json!({
        "namespace": fixture_namespace().map(|n| n.as_str()), "campaign_root": c.root,
        "store_id": store.id().bytes().iter().map(|b| format!("{b:02x}")).collect::<String>(),
        "program": scope.program.as_str(), "baseline": scope.baseline.get(), "task": scope.task.as_str(),
        "attempt": scope.attempt.as_str(), "generation": scope.generation.get(), "effect": id.as_str(),
        "package": scope.package.as_str(), "package_revision": scope.package_revision.get(),
        "dispatch_operation": dispatch.id.as_str(), "expected_predecessor": dispatch.expected,
        "started_operation": capture_ids(label).started_operation.as_str(), "started_expected": dispatch.expected + 1,
        "finished_operation": capture_ids(label).finished_operation.as_str(), "finished_expected": dispatch.expected + 2,
        "retained_operations": kernel.operations().iter().map(|o| serde_json::json!({"id":o.id.as_str(),"expected":o.expected})).collect::<Vec<_>>(),
        "planned_attempts": [fixture_id("attempt-A"),fixture_id("attempt-B"),fixture_id("attempt-synthetic-unknown")],
        "release": fixture_id("release-AB"), "executable": spec.executable, "accepted_executable_bytes": executable.len(),
        "executable_content_evidence": accepted, "working_directory": spec.cwd, "arguments": spec.args,
        "input_bytes": spec.input_len, "wall_ms": spec.limits.wall_ms, "cleanup_ms": spec.limits.cleanup_ms,
        "output_bytes": spec.limits.output_bytes, "max_designated_fixture_children": 2,
        "cleanup": "native direct-child reaped and streams closed observation plus current PID absence",
        "retry": "NONE for uncertain effects", "containment": "NOT_QUALIFIED",
        "controller_helpers": "bounded fixture Git/rustc preparation, production Git inspection, mkfifo negative path and CLI inspection; separate from fixture children"
    });
    c.record(
        &format!("launch-manifest-{label}.json"),
        &serde_json::to_string_pretty(&manifest).unwrap(),
    );
    c.launch_slot();
    let result = run_prepared_fixture(
        store,
        &c.key,
        PreparedFixtureRun {
            preparation: fixture.1,
            dispatch: dispatch.clone(),
            pin: fixture.0,
            captures: capture_ids(label),
            workspace_observation: ObservationId::new(scope.task.as_str(), 4).unwrap(),
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        result.workspace.observation().contract().candidate(),
        &scope.candidate
    );
    let result = result.run;
    assert!(
        result.recording_failure.is_none(),
        "terminal observation recording failed"
    );
    assert!(result.started.is_some() && result.recorded.is_some());
    let observation = result.observation.unwrap();
    assert_eq!(observation.outcome(spec), Some(EffectOutcome::Succeeded));
    assert!(observation.quiescent(spec));
    assert_eq!(observation.input_bytes, spec.input_len);
    let process = observation.process.as_ref().unwrap();
    assert!(
        !Path::new(&format!("/proc/{}", process.pid)).exists(),
        "trusted fixture child still present"
    );
    c.record(&format!("process-{label}.txt"), &format!("effect={} attempt={} observation={}:{} pid={} start_ticks={} input={} output={} elapsed_ms={} completion={:?} quiescence={:?}\n", id.as_str(), scope.attempt.as_str(), observation.id.scope(), observation.id.sequence(), process.pid, process.start_ticks, observation.input_bytes, observation.stdout_bytes + observation.stderr_bytes, observation.elapsed_ms, observation.completion, observation.quiescence));
    dispatch
}
fn settle(c: &Campaign, store: &ProgramStore, scope: &EffectScope, id: &EffectId, label: &str) {
    let k = load(c, store);
    let observation = k
        .fixture_effects()
        .effects()
        .iter()
        .find(|r| &r.intent.id == id)
        .unwrap()
        .process_observations
        .last()
        .unwrap();
    let usage = [
        (Resource::WorkerSlots, 1),
        (Resource::ElapsedMillis, observation.elapsed_ms),
        (
            Resource::OutputBytes,
            observation.stdout_bytes + observation.stderr_bytes,
        ),
    ]
    .into_iter()
    .map(|(resource, amount)| Demand {
        resource,
        amount: Some(amount),
    })
    .collect();
    effect(
        c,
        store,
        &format!("settle-{label}"),
        EffectEvent::Reconcile {
            effect: id.clone(),
            scope: scope.clone(),
            at: k.fixture_effects().last_tick(),
            resolution: Reconciliation::Observed {
                outcome: EffectOutcome::Succeeded,
                usage,
            },
            basis: source("actual-terminal-observation-and-explicit-accounting"),
        },
    );
}
fn snapshot(path: &Path) -> Vec<(PathBuf, Vec<u8>, u64, u64, i64, i64)> {
    let mut entries: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let m = fs::metadata(&path).unwrap();
            (
                path.clone(),
                fs::read(path).unwrap(),
                m.ino(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
            )
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}
fn reject(c: &Campaign, store: &ProgramStore, id: &str, command: Input) {
    let before = snapshot(&c.state);
    let k = load(c, store);
    let error = store.admit(&c.key, operation(&k, id, command)).unwrap_err();
    assert_eq!(error.completion, Completion::Rejected);
    assert_eq!(
        snapshot(&c.state),
        before,
        "rejected transition changed journal"
    );
    assert_eq!(load(c, store).revision(), k.revision());
}
struct Verification {
    contract: CandidateBindingContract,
    profile: VerificationProfile,
    observation: ObservationId,
    examination: Examination,
    assessment: AssessmentId,
}
fn verification_context(
    k: &CoordinationKernel,
    name: &str,
    captured: Option<&CapturedArtifact>,
) -> (CandidateBindingContract, VerificationProfile) {
    let task_contract = k
        .state()
        .definition()
        .graph()
        .tasks()
        .iter()
        .find(|t| t.id() == &task(if name == "wrong" { "A" } else { name }))
        .unwrap()
        .context()
        .contract();
    let contract = CandidateBindingContract::new(
        TaskContract::new(
            task(&format!("verify-{name}")),
            "Independently verify exact fixture bytes",
            task_contract.task().acceptance(),
        )
        .unwrap(),
        rev(1),
        task_contract.candidate().clone(),
    );
    let profile = VerificationProfile::new(Provenance::new("parent literal byte oracle, separately defined from producer and consumer algorithms", &format!("artifact=prime-list revision=1 source={} task={name}; {}", contract.candidate().commit().as_str(), captured.map(|a| format!("sha256={} effect={} observation={}:{}", a.sha256(), a.effect().as_str(), a.observation().scope(), a.observation().sequence())).unwrap_or_else(|| "consumer file and consumed-byte receipt".into())), "offline finite trusted fixture; exact-byte criterion; no authentication or containment claim").unwrap(), "gpe-func-2t-oracle-v1", "trusted-local-linux", &[("cortex-harness", "supervised-fixture-v1"), ("oracle", "literal-v1")]).unwrap();
    (contract, profile)
}
fn observe(
    c: &Campaign,
    store: &ProgramStore,
    name: &str,
    bytes: &[u8],
    expected: &[u8],
    captured: Option<&CapturedArtifact>,
) -> Verification {
    let (contract, profile) = verification_context(&load(c, store), name, captured);
    let observation = ObservationId::new(&fixture_id(&format!("oracle-{name}")), 1).unwrap();
    let before = inspect_candidate(
        &c.fixture,
        &contract,
        ObservationId::new(&fixture_id(&format!("candidate-{name}")), 1).unwrap(),
    )
    .unwrap();
    let report = ReportedObservation::new(
        observation.clone(),
        before,
        profile.clone(),
        ObservationTimes::new(SystemTime::now(), SystemTime::now()).unwrap(),
        cortex_evidence::unified::ObservationKind::Execution(
            VerificationOutcome::new(
                ExecutionObservation::Exited { code: 0 },
                1,
                Some(
                    OracleEvaluation::new(
                        if bytes == expected {
                            OracleConclusion::Satisfied
                        } else {
                            OracleConclusion::Violated
                        },
                        "Independent literal equality over actual bounded artifact bytes",
                    )
                    .unwrap(),
                ),
            )
            .unwrap(),
        ),
        "Actual private file bytes examined; local caller provenance is unauthenticated",
    )
    .unwrap();
    admit(c, store, &format!("observe-{name}"), Input::Observe(report));
    let examination = Examination {
        observation: observation.clone(),
        contract: contract.clone(),
        candidate: Some(
            inspect_candidate(
                &c.fixture,
                &contract,
                ObservationId::new(&fixture_id(&format!("candidate-{name}")), 2).unwrap(),
            )
            .unwrap(),
        ),
        profile: Some(profile.clone()),
    };
    Verification {
        contract,
        profile,
        observation,
        examination,
        assessment: AssessmentId::new(&fixture_id(&format!("assessment-{name}"))).unwrap(),
    }
}
fn assess(c: &Campaign, store: &ProgramStore, v: &Verification, name: &str) {
    admit(
        c,
        store,
        &format!("assess-{name}"),
        Input::Assess(Box::new(AssessmentInput {
            examination: v.examination.clone(),
            id: v.assessment.clone(),
            at: SystemTime::now(),
            result: AssessmentResult::Pass,
            applicability: Applicability::Applicable,
            rationale: "Actual bytes exactly match separately established literal oracle".into(),
        })),
    );
}
fn release(k: &CoordinationKernel, v: &Verification, report: &NotificationId) -> ReleaseInput {
    let scope = EdgeScopeSnapshot::capture(
        k.state().readiness(),
        &EdgeId::new(&fixture_id("AB")).unwrap(),
    )
    .unwrap();
    let limits = SupportLimits::new(8, 8, 8, MAX_BYTES).unwrap();
    let predicate = PredicateId::new(&fixture_id("exact-prime-bytes-v1")).unwrap();
    let contract = EdgeEvidenceContract::new(
        scope.clone(),
        rev(1),
        v.contract.candidate().clone(),
        vec![PredicateCorrespondence::new(
            predicate.clone(),
            v.contract.clone(),
            v.profile.clone(),
            CriterionCoverage::ExactSingleCriterion,
        )],
        source("complete-exact-byte-correspondence"),
        limits,
    )
    .unwrap();
    let review = Notification::new(
        NotificationId::new(&fixture_id("coordinator-review-AB")).unwrap(),
        k.state().definition().graph().reference().clone(),
        Event::Review(Box::new(EdgeReview::new(
            EdgeId::new(&fixture_id("AB")).unwrap(),
            report.clone(),
            scope.producer().clone(),
            scope.consumer().clone(),
            ReviewRole::Coordinator,
            ReviewAttestation::new(
                source("independent-parent-oracle-review"),
                source("separate-scoped-coordinator-authorization"),
                None,
            ),
            vec![PredicateSupport::new(
                predicate.clone(),
                source("actual-oracle-observation"),
                source("explicit-artifact-assessment"),
                source("actual-source-reinspection"),
            )],
        ))),
    )
    .unwrap();
    ReleaseInput {
        contract,
        producer: Some(
            inspect_candidate(
                scope
                    .producer()
                    .context()
                    .contract()
                    .candidate()
                    .repository()
                    .root(),
                scope.producer().context().contract(),
                ObservationId::new(&fixture_id("A"), 2).unwrap(),
            )
            .unwrap(),
        ),
        consumer: Some(
            inspect_candidate(
                scope
                    .consumer()
                    .context()
                    .contract()
                    .candidate()
                    .repository()
                    .root(),
                scope.consumer().context().contract(),
                ObservationId::new(&fixture_id("B"), 2).unwrap(),
            )
            .unwrap(),
        ),
        examinations: vec![v.examination.clone()],
        bindings: vec![WitnessBinding::new(
            predicate,
            EvidenceReference::new(v.observation.clone(), Some(v.assessment.clone())),
            scope.requirement().artifact().clone(),
        )],
        review,
        limits,
    }
}

#[test]
fn two_tasks_produce_release_reconstruct_consume_invalidate_and_retain_unknown() {
    let c = Campaign::new();
    let initial = definition(&c);
    let artifact = initial.graph().edges()[0].requirement().artifact().clone();
    let protected = c.fixture.join("forbidden-state");
    assert!(
        ProgramStore::create_supervised_fixture(&protected, &c.key, initial.clone(), read_limits())
            .is_err()
    );
    if protected.exists() {
        fs::remove_dir_all(&protected).unwrap();
    }
    let store =
        ProgramStore::create_supervised_fixture(&c.state, &c.key, initial.clone(), read_limits())
            .unwrap();
    let mut workflow_evaluation = gpe_evaluation::WorkflowEvaluation::new(&c, &load(&c, &store));
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
                source: source("actual-development-fixture-prerequisites"),
            },
        );
    }
    notify(
        &c,
        &store,
        "capacity",
        Event::Capacity {
            available: 2,
            source: source("logical-fixture-slots"),
        },
    );
    effect(&c, &store, "policy", EffectEvent::Policy(policy()));
    let (a, aid, spec, pin, a_preparation) = prepare(&c, &store, "A", "A", None);
    let k = load(&c, &store);
    let prepared = &k.prepared_inputs()[0].1;
    let mut wrong = spec.clone();
    wrong.cwd = c.output.clone();
    reject(
        &c,
        &store,
        "wrong-cwd",
        Input::SupervisedEffect(Box::new(EffectEvent::PrepareProcess {
            effect: aid.clone(),
            scope: a.clone(),
            at: k.fixture_effects().last_tick(),
            spec: Box::new(wrong),
        })),
    );
    let mut wrong = spec.clone();
    wrong.profile = source("stale-profile");
    reject(
        &c,
        &store,
        "wrong-profile",
        Input::SupervisedEffect(Box::new(EffectEvent::PrepareProcess {
            effect: aid.clone(),
            scope: a.clone(),
            at: k.fixture_effects().last_tick(),
            spec: Box::new(wrong),
        })),
    );
    let mut wrong = spec.clone();
    wrong.limits.executable_bytes = 1;
    assert!(inspect_fixture(&wrong, prepared, initial.limits()).is_err());
    let mut wrong = spec.clone();
    wrong.limits.executable_bytes = MAX_BYTES as u64 + 1;
    assert!(inspect_fixture(&wrong, prepared, initial.limits()).is_err());
    process(&c, &store, &a, &aid, &spec, "A");
    assert!(
        capture_artifact(
            &load(&c, &store),
            &aid,
            &artifact,
            &c.output,
            &c.output.join("A"),
            std::slice::from_ref(&c.cortex),
            256
        )
        .is_err()
    );
    c.mark("Q0");
    workflow_evaluation.register(&c, &store, &a, &aid, "A");
    let clean = fs::read(c.fixture.join("main.rs")).unwrap();
    fs::write(c.fixture.join("main.rs"), b"fn main() {}").unwrap();
    let dirty_dispatch = dispatch(&load(&c, &store), &a, &aid, "dirty-guard");
    let before_dirty_refusal = snapshot(&c.state);
    let refused = run_prepared_fixture(
        &store,
        &c.key,
        PreparedFixtureRun {
            preparation: &a_preparation,
            dispatch: dirty_dispatch,
            pin: &pin,
            captures: capture_ids("dirty-guard"),
            workspace_observation: ObservationId::new(a.task.as_str(), 3).unwrap(),
        },
        &AtomicBool::new(false),
    );
    assert!(matches!(refused, Err(ExecutionError::Workspace(_))));
    assert_eq!(snapshot(&c.state), before_dirty_refusal);
    assert!(!c.output.join("launched-A").exists());
    fs::write(c.fixture.join("main.rs"), clean).unwrap();
    let dispatch_a = run(&c, &store, &a, &aid, &spec, (&pin, &a_preparation), "A");
    let k = load(&c, &store);
    assert!(k.records().is_empty() && k.decisions().is_empty());
    assert!(
        !k.state()
            .readiness()
            .task(&task("B"))
            .unwrap()
            .dependency_ready()
    );
    let producer_file = c.output.join("A");
    let output = |expected_sha256| FixtureArtifactRequest {
        effect: &aid,
        artifact: &artifact,
        output_root: &c.output,
        path: &producer_file,
        additional_sources: std::slice::from_ref(&c.cortex),
        max_bytes: 256,
        expected_sha256,
        workspace_observation: ObservationId::new(a.task.as_str(), 5).unwrap(),
    };
    let captured = capture_fixture_output(
        &store,
        &c.key,
        output("a1a410bd6438bb73e09ee3220cf8a87470d1d6401757f68be666fd8399e78df0"),
    )
    .unwrap();
    assert_eq!(captured.revision, k.revision());
    assert_eq!(
        captured.workspace.observation().contract().candidate(),
        &a.candidate
    );
    let captured = captured.content;
    assert!(matches!(
        capture_fixture_output(&store, &c.key, output("different-output-digest")),
        Err(ExecutionError::ArtifactDigest)
    ));
    assert_eq!(captured.bytes(), ARTIFACT);
    assert_eq!(
        captured.sha256(),
        "a1a410bd6438bb73e09ee3220cf8a87470d1d6401757f68be666fd8399e78df0"
    );
    assert_ne!(captured.bytes(), b"cortex-artifact-v1\n2\n3\n5\n9\n");
    assert_eq!(captured.artifact(), &artifact);
    let wrong_artifact =
        ArtifactReference::new(artifact.id().clone(), rev(2), artifact.candidate().clone());
    assert!(matches!(
        capture_artifact(
            &k,
            &aid,
            &wrong_artifact,
            &c.output,
            &c.output.join("A"),
            &[],
            256
        ),
        Err(ArtifactError::Correspondence)
    ));
    assert!(matches!(
        capture_artifact(
            &k,
            &aid,
            &artifact,
            &c.output,
            &c.output.join("A"),
            &[],
            ARTIFACT.len() - 1
        ),
        Err(ArtifactError::Limit)
    ));
    let link = c.output.join("symlink");
    symlink(c.output.join("A"), &link).unwrap();
    assert!(capture_artifact(&k, &aid, &artifact, &c.output, &link, &[], 256).is_err());
    fs::remove_file(link).unwrap();
    let hardlink = c.output.join("hardlink");
    fs::hard_link(c.output.join("A"), &hardlink).unwrap();
    assert!(matches!(
        capture_artifact(
            &k,
            &aid,
            &artifact,
            &c.output,
            &c.output.join("A"),
            &[],
            256
        ),
        Err(ArtifactError::Changed)
    ));
    fs::remove_file(hardlink).unwrap();
    assert!(matches!(
        capture_artifact(
            &k,
            &aid,
            &artifact,
            &c.fixture,
            &c.fixture.join("main.rs"),
            &[],
            65_536
        ),
        Err(ArtifactError::Location)
    ));
    c.record(
        "artifact.txt",
        &format!(
            "artifact=prime-list revision=1 candidate={} sha256={} effect={} observation={}:{}\n",
            artifact.candidate().as_str(),
            captured.sha256(),
            captured.effect().as_str(),
            captured.observation().scope(),
            captured.observation().sequence()
        ),
    );
    let alias = c
        .root
        .join("controller")
        .join(format!("source-alias-{}", std::process::id()));
    symlink(&c.output, &alias).unwrap();
    let alias_source =
        RepositoryIdentity::new(alias.clone(), alias.clone(), alias.clone()).unwrap();
    assert!(matches!(
        capture_artifact(
            &k,
            &aid,
            &artifact,
            &c.output,
            &c.output.join("A"),
            &[alias_source],
            256
        ),
        Err(ArtifactError::Location)
    ));
    fs::remove_file(alias).unwrap();
    let fifo = c.output.join("fifo");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    assert!(matches!(
        capture_artifact(&k, &aid, &artifact, &c.output, &fifo, &[], 256),
        Err(ArtifactError::Changed)
    ));
    fs::remove_file(fifo).unwrap();
    c.mark("Q1");
    settle(&c, &store, &a, &aid, "A");
    let v = observe(&c, &store, "A", captured.bytes(), ARTIFACT, Some(&captured));
    let k = load(&c, &store);
    assert!(k.records()[0].assessment().is_none());
    let report_id = NotificationId::new(&fixture_id("artifact-A-v1")).unwrap();
    notify(
        &c,
        &store,
        "artifact-A-v1",
        Event::Artifact(ArtifactReport::new(
            k.state().readiness().scope(&task("A")).unwrap(),
            artifact.clone(),
            DependencyKind::Artifact,
            ReadinessLevel::ContractStable,
            source("actual-artifact-capture"),
        )),
    );
    let r = release(&load(&c, &store), &v, &report_id);
    admit(
        &c,
        &store,
        "release-unassessed-held",
        Input::Release(Box::new(r.clone())),
    );
    assert!(!load(&c, &store).decisions().last().unwrap().released);
    assess(&c, &store, &v, "A");
    workflow_evaluation.record(
        &c,
        &store,
        &a,
        "A",
        cortex_evidence::evaluation::AttemptExecution::Observed(ExecutionObservation::Exited {
            code: 0,
        }),
    );
    let mut missing = r.clone();
    missing.bindings.clear();
    admit(
        &c,
        &store,
        "release-missing-held",
        Input::Release(Box::new(missing)),
    );
    assert!(!load(&c, &store).decisions().last().unwrap().released);
    let mut forged = r.clone();
    forged.bindings = vec![WitnessBinding::new(
        PredicateId::new(&fixture_id("exact-prime-bytes-v1")).unwrap(),
        EvidenceReference::new(
            v.observation.clone(),
            Some(AssessmentId::new(&fixture_id("forged-assessment")).unwrap()),
        ),
        artifact.clone(),
    )];
    admit(
        &c,
        &store,
        "release-forged-held",
        Input::Release(Box::new(forged)),
    );
    assert!(!load(&c, &store).decisions().last().unwrap().released);
    let k = load(&c, &store);
    let mut unauthorized = operation(
        &k,
        "release-wrong-authority",
        Input::Release(Box::new(r.clone())),
    );
    unauthorized.grant.tasks = vec![task("A")];
    assert!(store.admit(&c.key, unauthorized).is_err());
    let release_operation = operation(
        &load(&c, &store),
        "release-AB",
        Input::Release(Box::new(r.clone())),
    );
    if let Some(namespace) = fixture_namespace() {
        namespace
            .check_release(
                release_operation.id.as_str(),
                initial.graph().edges()[0].id().as_str(),
                artifact.id().as_str(),
            )
            .unwrap();
    }
    let release_receipt = store.admit(&c.key, release_operation.clone()).unwrap();
    let released = load(&c, &store);
    assert!(released.decisions().last().unwrap().released);
    assert!(
        released
            .state()
            .readiness()
            .task(&task("B"))
            .unwrap()
            .eligible()
    );
    assert_eq!(
        released
            .state()
            .readiness()
            .task(&task("A"))
            .unwrap()
            .status(),
        TaskStatus::Active
    );
    assert_eq!(released.fixture_effects().effects().len(), 1);
    let wrong_bytes = b"cortex-artifact-v1\n2\n3\n5\n9\n";
    private_file(&c.output.join("incorrect-neighbor"), wrong_bytes);
    let wrong_capture = capture_artifact(
        &load(&c, &store),
        &aid,
        &artifact,
        &c.output,
        &c.output.join("incorrect-neighbor"),
        std::slice::from_ref(&c.cortex),
        256,
    )
    .unwrap();
    let wrong_v = observe(
        &c,
        &store,
        "wrong",
        wrong_capture.bytes(),
        ARTIFACT,
        Some(&wrong_capture),
    );
    reject(
        &c,
        &store,
        "incorrect-oracle-cannot-pass",
        Input::Assess(Box::new(AssessmentInput {
            examination: wrong_v.examination.clone(),
            id: wrong_v.assessment.clone(),
            at: SystemTime::now(),
            result: AssessmentResult::Pass,
            applicability: Applicability::Applicable,
            rationale: "Incorrect bytes must not pass despite successful process exit".into(),
        })),
    );
    admit(
        &c,
        &store,
        "assess-incorrect-fail",
        Input::Assess(Box::new(AssessmentInput {
            examination: wrong_v.examination.clone(),
            id: wrong_v.assessment.clone(),
            at: SystemTime::now(),
            result: AssessmentResult::Fail,
            applicability: Applicability::Applicable,
            rationale: "Actual neighboring bytes violate the independent literal oracle".into(),
        })),
    );
    let mut contradictory = r.clone();
    contradictory.examinations.push(wrong_v.examination.clone());
    contradictory.bindings.push(WitnessBinding::new(
        PredicateId::new(&fixture_id("exact-prime-bytes-v1")).unwrap(),
        EvidenceReference::new(
            wrong_v.observation.clone(),
            Some(wrong_v.assessment.clone()),
        ),
        artifact.clone(),
    ));
    admit(
        &c,
        &store,
        "contradictory-release-held",
        Input::Release(Box::new(contradictory)),
    );
    assert!(!load(&c, &store).decisions().last().unwrap().released);
    c.mark("Q2");
    // Controlled cut: the actual direct child is reaped, input/output closed,
    // and no B preparation exists. Only the trusted parent retains the key.
    let before = snapshot(&c.state);
    let cut_state = load(&c, &store);
    let revision = cut_state.revision();
    let operations = cut_state.operations().to_vec();
    drop(cut_state);
    drop(released);
    drop(store);
    let store = ProgramStore::open(&c.state, &c.key, Access::ReadOnly, read_limits()).unwrap();
    for _ in 0..2 {
        let reopened = load(&c, &store);
        assert!(reopened.state().definition() == &initial);
        assert!(reopened.operations() == operations);
        assert_eq!(reopened.revision(), revision);
        assert_eq!(
            reopened
                .state()
                .readiness()
                .edge(&EdgeId::new(&fixture_id("AB")).unwrap())
                .unwrap()
                .releases()
                .len(),
            1
        );
        assert_eq!(reopened.fixture_effects().effects().len(), 1);
        assert_eq!(reopened.state().attempts().len(), 1);
        assert_eq!(
            capture_artifact(
                &reopened,
                &aid,
                &artifact,
                &c.output,
                &c.output.join("A"),
                std::slice::from_ref(&c.cortex),
                256
            )
            .unwrap()
            .sha256(),
            captured.sha256()
        );
    }
    assert_eq!(snapshot(&c.state), before);
    // Explicit admission access is reopened separately after read-only reconstruction.
    drop(store);
    let store = ProgramStore::open(&c.state, &c.key, Access::Recover, read_limits()).unwrap();
    let duplicate = store.admit(&c.key, release_operation.clone()).unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.ordinal, release_receipt.ordinal);
    let (receipt, callback) = store
        .dispatch_fixture(&c.key, dispatch_a.clone(), |_| {
            panic!("reconstruction relaunched producer")
        })
        .unwrap();
    assert!(receipt.duplicate && callback.is_none());
    assert_eq!(snapshot(&c.state), before);
    let mut changed = release_operation.clone();
    changed.expected += 1;
    assert!(store.admit(&c.key, changed).is_err());
    c.mark("Q3");
    let (b, bid, bspec, bpin, b_preparation) = prepare(&c, &store, "B", "B", Some(&captured));
    process(&c, &store, &b, &bid, &bspec, "B");
    workflow_evaluation.register(&c, &store, &b, &bid, "B");
    let wrong_package = dispatch(&load(&c, &store), &b, &bid, "wrong-input-guard");
    let before_input_refusal = snapshot(&c.state);
    let refused = run_prepared_fixture(
        &store,
        &c.key,
        PreparedFixtureRun {
            preparation: &a_preparation,
            dispatch: wrong_package,
            pin: &bpin,
            captures: capture_ids("wrong-input-guard"),
            workspace_observation: ObservationId::new(b.task.as_str(), 3).unwrap(),
        },
        &AtomicBool::new(false),
    );
    assert!(matches!(refused, Err(ExecutionError::PreparedInput)));
    assert_eq!(snapshot(&c.state), before_input_refusal);
    assert!(!c.output.join("launched-B").exists());
    let dispatch_b = run(&c, &store, &b, &bid, &bspec, (&bpin, &b_preparation), "B");
    assert_eq!(
        fs::read(c.output.join("B.consumed")).unwrap(),
        captured.bytes()
    );
    let returned = cortex_harness::native_execution::artifacts::capture(
        fs::File::open(&c.output).unwrap(),
        &[cortex_harness::native_execution::artifacts::ArtifactSpec {
            path: PathBuf::from("B"),
            max_bytes: 256,
            sha256: None,
        }],
        256,
    )
    .unwrap();
    let actual = returned[0].bytes.as_slice();
    assert_eq!(actual, CONSUMED);
    assert_ne!(actual, b"cortex-consumer-v1\ncount=4\nsum=18\nsquares=87\n");
    let before_settlement = load(&c, &store);
    assert!(
        before_settlement
            .fixture_effects()
            .accounted(Resource::ElapsedMillis)
            .unwrap()
            >= 10_500
    );
    settle(&c, &store, &b, &bid, "B");
    let bv = observe(&c, &store, "B", actual, CONSUMED, None);
    assess(&c, &store, &bv, "B");
    workflow_evaluation.record(
        &c,
        &store,
        &b,
        "B",
        cortex_evidence::evaluation::AttemptExecution::Observed(ExecutionObservation::Exited {
            code: 0,
        }),
    );
    let k = load(&c, &store);
    assert_eq!(k.fixture_effects().effects().len(), 2);
    assert!(
        k.fixture_effects()
            .effects()
            .iter()
            .all(|r| r.reconciliation.is_some())
    );
    assert!(
        k.fixture_effects()
            .accounted(Resource::OutputBytes)
            .unwrap()
            > 0
    );
    let elapsed = k
        .fixture_effects()
        .accounted(Resource::ElapsedMillis)
        .unwrap();
    let output = k
        .fixture_effects()
        .accounted(Resource::OutputBytes)
        .unwrap();
    let replay = execute_fixture(
        &store,
        &c.key,
        dispatch_b,
        capture_ids("B"),
        &bpin,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(replay.dispatch.duplicate && replay.observation.is_none());
    assert_eq!(
        fs::read(c.output.join("launched-A")).unwrap(),
        b"launched\n"
    );
    assert_eq!(
        fs::read(c.output.join("launched-B")).unwrap(),
        b"launched\n"
    );
    c.mark("Q4");
    // A fresh synthetic attempt shares this same authoritative journal, but its
    // dispatch has no child callback. Its missing outcome is deliberately retained.
    admit(
        &c,
        &store,
        "retire-A-metadata",
        Input::Attempt {
            id: a.attempt.clone(),
            generation: a.generation,
            status: AttemptStatus::Cancelled,
            basis: source("explicit-metadata-retirement-after-settlement"),
        },
    );
    assert!(matches!(
        capture_artifact(
            &load(&c, &store),
            &aid,
            &artifact,
            &c.output,
            &c.output.join("A"),
            std::slice::from_ref(&c.cortex),
            256
        ),
        Err(ArtifactError::Correspondence)
    ));
    let (unknown, uid, uspec, _, _) = prepare(&c, &store, "A", "synthetic-unknown", None);
    process(&c, &store, &unknown, &uid, &uspec, "synthetic-unknown");
    workflow_evaluation.register(&c, &store, &unknown, &uid, "synthetic-unknown");
    let synthetic_dispatch = dispatch(&load(&c, &store), &unknown, &uid, "synthetic-unknown");
    let (_, callback) = store
        .dispatch_fixture(
            &c.key,
            synthetic_dispatch.clone(),
            |_| "synthetic-no-child-operation",
        )
        .unwrap();
    assert_eq!(callback, Some("synthetic-no-child-operation"));
    effect(
        &c,
        &store,
        "synthetic-unknown",
        EffectEvent::Unknown {
            effect: uid.clone(),
            scope: unknown.clone(),
            at: load(&c, &store).fixture_effects().last_tick(),
            basis: source("synthetic-unresolved-outcome-no-child-launched"),
        },
    );
    let held = load(&c, &store)
        .fixture_effects()
        .accounted(Resource::ElapsedMillis)
        .unwrap();
    assert_eq!(held, elapsed + 10_500);
    reject(
        &c,
        &store,
        "unknown-refund",
        Input::SupervisedEffect(Box::new(EffectEvent::Reconcile {
            effect: uid.clone(),
            scope: unknown.clone(),
            at: load(&c, &store).fixture_effects().last_tick(),
            resolution: Reconciliation::Absent,
            basis: source("unjustified-absence"),
        })),
    );
    reject(
        &c,
        &store,
        "unknown-invented-settlement",
        Input::SupervisedEffect(Box::new(EffectEvent::Reconcile {
            effect: uid.clone(),
            scope: unknown.clone(),
            at: load(&c, &store).fixture_effects().last_tick(),
            resolution: Reconciliation::Observed {
                outcome: EffectOutcome::Succeeded,
                usage: demand(),
            },
            basis: source("unobserved-usage"),
        })),
    );
    reject(
        &c,
        &store,
        "unknown-retry",
        Input::SupervisedEffect(Box::new(EffectEvent::Reserve {
            intent: EffectIntent {
                id: EffectId::new(&fixture_id("forbidden-retry")).unwrap(),
                grant: GrantId::new(&fixture_id("operational-synthetic-unknown")).unwrap(),
                scope: unknown.clone(),
                demand: demand(),
                retry_of: Some(uid.clone()),
            },
            at: load(&c, &store).fixture_effects().last_tick(),
        })),
    );
    let (_, callback) = store
        .dispatch_fixture(&c.key, synthetic_dispatch, |_| {
            panic!("Unknown automatically retried")
        })
        .unwrap();
    assert!(callback.is_none());
    assert!(!c.output.join("launched-synthetic-unknown").exists());
    admit(
        &c,
        &store,
        "synthetic-attempt-unknown",
        Input::Attempt {
            id: unknown.attempt.clone(),
            generation: unknown.generation,
            status: AttemptStatus::Unknown,
            basis: source("synthetic-uncertainty-retains-metadata-claim"),
        },
    );
    workflow_evaluation.record(
        &c,
        &store,
        &unknown,
        "synthetic-unknown",
        cortex_evidence::evaluation::AttemptExecution::Unknown,
    );
    c.mark("Q6");
    let unaffected = load(&c, &store)
        .state()
        .readiness()
        .scope(&task("A"))
        .unwrap();
    let mut changed_support = v.examination.clone();
    changed_support.profile = Some(
        VerificationProfile::new(
            v.profile.provenance().clone(),
            "gpe-func-2t-oracle-v2",
            "trusted-local-linux",
            &[
                ("cortex-harness", "supervised-fixture-v1"),
                ("oracle", "literal-v2"),
            ],
        )
        .unwrap(),
    );
    admit(
        &c,
        &store,
        "material-oracle-change",
        Input::Examine(changed_support),
    );
    let k = load(&c, &store);
    assert_eq!(
        k.state()
            .readiness()
            .scope(&task("A"))
            .unwrap()
            .generation(),
        unaffected.generation()
    );
    assert_eq!(
        k.state().readiness().task(&task("A")).unwrap().status(),
        TaskStatus::Active
    );
    assert!(
        k.state()
            .readiness()
            .scope(&task("B"))
            .unwrap()
            .generation()
            .get()
            > b.generation.get()
    );
    assert!(
        !k.state()
            .readiness()
            .task(&task("B"))
            .unwrap()
            .dependency_ready()
    );
    assert_eq!(
        k.records()[0].assessment().unwrap().result(),
        AssessmentResult::Pass
    );
    assert_eq!(
        k.inspect_record(&v.examination).unwrap().validity,
        Some(Validity::Stale)
    );
    assert_eq!(
        k.fixture_effects()
            .accounted(Resource::ElapsedMillis)
            .unwrap(),
        held
    );
    assert_eq!(
        k.fixture_effects()
            .accounted(Resource::OutputBytes)
            .unwrap(),
        output + 4096
    );
    reject(
        &c,
        &store,
        "stale-B-attempt",
        Input::Attempt {
            id: b.attempt.clone(),
            generation: b.generation,
            status: AttemptStatus::Reported,
            basis: source("late-stale-report"),
        },
    );
    reject(&c, &store, "stale-release", Input::Release(Box::new(r)));
    c.mark("Q5");
    let before = snapshot(&c.state);
    let default = c.cli(None, true);
    let explicit = c.cli(Some("3"), true);
    assert!(default.status.success() && explicit.status.success());
    assert_eq!(default.stdout, explicit.stdout);
    let human_default = c.cli(None, false);
    let human_explicit = c.cli(Some("3"), false);
    assert!(human_default.status.success() && human_explicit.status.success());
    assert_eq!(human_default.stdout, human_explicit.stdout);
    let human = String::from_utf8(human_default.stdout).unwrap();
    assert!(human.contains("Retained Evidence (no live revalidation)"));
    assert!(human.contains("assessment-A") && human.contains("STALE"));
    let view: serde_json::Value = serde_json::from_slice(&default.stdout).unwrap();
    assert_eq!(view["schema"], "cortex.program.v3");
    assert_eq!(view["tasks"].as_array().unwrap().len(), 2);
    assert_eq!(view["edges"][0]["artifact"], fixture_id("prime-list"));
    assert_eq!(view["edges"][0]["prior_releases"], 1);
    let evidence = view["evidence"].as_array().unwrap();
    let stale = evidence
        .iter()
        .find(|e| e["observation_scope"] == fixture_id("oracle-A"))
        .unwrap();
    assert_eq!(stale["historical_result"], "PASS");
    assert_eq!(stale["contextual_validity"], "STALE");
    assert_eq!(stale["assessment"], fixture_id("assessment-A"));
    assert_eq!(
        stale["validity_basis"],
        "RETAINED_ASSESSMENT_CONTEXT_NOT_LIVE_VERIFICATION"
    );
    let released = view["support_decisions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["operation"] == fixture_id("release-AB"))
        .unwrap();
    assert_eq!(
        released["witnesses"][0]["observation_scope"],
        fixture_id("oracle-A")
    );
    assert_eq!(released["witnesses"][0]["artifact_revision"], 1);
    let text = String::from_utf8(default.stdout.clone()).unwrap();
    for required in [
        "synthetic-unknown",
        "UNKNOWN",
        "attempt-A",
        "attempt-B",
        "exact-prime-bytes-v1",
        "material-oracle-change",
        "HELD",
        "UNAVAILABLE",
    ] {
        assert!(
            text.contains(required),
            "missing CLI contract field: {required}"
        );
    }
    for version in ["1", "2"] {
        let old = c.cli(Some(version), true);
        assert!(old.status.success());
        let old: serde_json::Value = serde_json::from_slice(&old.stdout).unwrap();
        assert_eq!(old["schema"], format!("cortex.program.v{version}"));
        assert_eq!(old["journal_revision"], view["journal_revision"]);
        assert!(old.to_string().contains("UNKNOWN"));
        assert!(old.get("evidence").is_none());
        assert!(old["support_decisions"][0].get("witnesses").is_none());
    }
    assert_eq!(snapshot(&c.state), before);
    let reopened = ProgramStore::open(&c.state, &c.key, Access::ReadOnly, read_limits()).unwrap();
    let final_state = load(&c, &reopened);
    assert_eq!(
        final_state
            .fixture_effects()
            .effects()
            .last()
            .unwrap()
            .status,
        EffectStatus::Unknown
    );
    assert_eq!(
        final_state
            .fixture_effects()
            .accounted(Resource::ElapsedMillis)
            .unwrap(),
        held
    );
    assert_eq!(snapshot(&c.state), before);
    c.record(
        "cli-schema3.json",
        &String::from_utf8(default.stdout).unwrap(),
    );
    c.record("final.txt", &format!("program={} baseline=1 journal_revision={} operations={} fixture_children=2 terminated=true synthetic_unknown=1 elapsed_accounted={} output_accounted={}\n", final_state.state().definition().id().as_str(), final_state.revision(), final_state.operations().len(), held, output + 4096));
    c.mark("Q7-main");
    workflow_evaluation.finish(&c, &reopened);
}
