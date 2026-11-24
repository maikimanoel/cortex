//! Canonical fixture releases with actual owned committed worker/integration bytes.
#![cfg(target_os = "linux")]

#[path = "../../cortex-evidence/tests/support/program.rs"]
mod fixture;

use cortex_cli::{
    inspect_candidate,
    program_execution::{
        IntegrationConstituent, IntegrationError, IntegrationOracle, IntegrationRequest,
        inspect_integrated_candidate,
    },
};
use cortex_core::{
    CandidateId, TaskContract, binding::*, knowledge::SourcePath, parallel::*, program::*,
};
use cortex_evidence::program::{Command, CoordinationKernel};
use cortex_storage::{
    history_store::{Access, StoreKey},
    program_store::{ProgramStore, ReadLimits},
};
use fixture::{operation, prepare_release, rev, source, task};
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    process,
    time::Duration,
};

struct OwnedTestRoot(PathBuf);

impl Drop for OwnedTestRoot {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn git(path: &Path, args: &[&str]) -> String {
    let output = process::Command::new("git")
        .env_clear()
        .env("PATH", std::env::var_os("PATH").expect("Git on PATH"))
        .env("GIT_AUTHOR_DATE", "2001-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2001-01-01T00:00:00Z")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(path)
        .args([
            "-c",
            "user.name=Controlled Test",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "gc.auto=0",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "owned test Git command refused");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn candidate(path: &Path, files: &[(&str, &[u8])]) -> CandidateRequirement {
    fs::create_dir(path).unwrap();
    git(
        path,
        &["init", "-q", "--template=", "--initial-branch=main"],
    );
    for (name, bytes) in files {
        fs::write(path.join(name), bytes).unwrap();
        git(path, &["add", "--", name]);
    }
    git(path, &["commit", "-qm", "Scoped development fixture"]);
    let metadata = path.join(".git");
    CandidateRequirement::new(
        RepositoryIdentity::new(path.to_owned(), metadata.clone(), metadata).unwrap(),
        CandidateId::from_git_commit(&git(path, &["rev-parse", "HEAD"])).unwrap(),
        WorktreeRequirement::Clean,
    )
}

fn definition(
    left: &CandidateRequirement,
    right: &CandidateRequirement,
    integrated: &CandidateRequirement,
) -> ProgramDefinition {
    let original = fixture::definition();
    let lookup = |name| match name {
        "B" => Some(left),
        "E" => Some(right),
        "F" => Some(integrated),
        _ => None,
    };
    let tasks = original
        .graph()
        .tasks()
        .iter()
        .map(|old| {
            let requirement = lookup(old.id().as_str()).unwrap_or(left);
            let contract = CandidateBindingContract::new(
                TaskContract::new(
                    old.id().clone(),
                    "Combine the exact contributor data",
                    "Retain both parts with correct sum 5",
                )
                .unwrap(),
                rev(1),
                requirement.clone(),
            );
            let initial = inspect_candidate(
                requirement.repository().root(),
                &contract,
                ObservationId::new(old.id().as_str(), 1).unwrap(),
            )
            .unwrap();
            Task::new(
                TaskContext::new(contract, Some(initial)).unwrap(),
                old.kind(),
                old.coverage(),
            )
        })
        .collect();
    let mut increments = original.increments().to_vec();
    let mut edges = Vec::new();
    for increment in &mut increments {
        increment.artifacts.clear();
        let Some(worker) = lookup(increment.task.as_str()).filter(|_| increment.task != task("F"))
        else {
            continue;
        };
        let artifact = ArtifactReference::new(
            ArtifactId::new(&format!("artifact-{}", increment.task.as_str())).unwrap(),
            rev(1),
            worker.commit().clone(),
        );
        increment.artifacts.push(artifact.clone());
        edges.push(DependencyEdge::new(
            EdgeId::new(&format!("{}F", increment.task.as_str())).unwrap(),
            increment.task.clone(),
            task("F"),
            ArtifactRequirement::new(
                artifact,
                DependencyKind::Artifact,
                ReadinessLevel::ContractStable,
                vec![PredicateId::new("source-bytes").unwrap()],
            )
            .unwrap(),
        ));
    }
    ProgramDefinition::new(
        original.id().clone(),
        original.outcome().clone(),
        original.revision(),
        None,
        original.approval().clone(),
        DependencyGraph::new(
            original.graph().reference().clone(),
            tasks,
            edges,
            original.graph().limits(),
        )
        .unwrap(),
        original.workstreams().to_vec(),
        increments,
        original.limits(),
    )
    .unwrap()
}

fn prepared_observed_release(
    draft: &mut CoordinationKernel,
    edge: &str,
) -> cortex_evidence::program::ReleaseInput {
    let mut release = prepare_release(draft, edge, true);
    let fresh = |scope: &TaskScope| {
        let contract = scope.context().contract();
        inspect_candidate(
            contract.candidate().repository().root(),
            contract,
            ObservationId::new(scope.task().as_str(), 2).unwrap(),
        )
        .unwrap()
    };
    release.producer = Some(fresh(release.contract.scope().producer()));
    release.consumer = Some(fresh(release.contract.scope().consumer()));
    release
}

#[test]
fn separate_committed_candidate_requires_both_current_worker_releases_and_its_own_oracle() {
    let root = OwnedTestRoot(
        std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("cortex-integration-candidate-{}", process::id())),
    );
    fs::DirBuilder::new().mode(0o700).create(&root.0).unwrap();
    let left = candidate(
        &root.0.join("worker-b"),
        &[(
            "part.txt", b"2
",
        )],
    );
    let right = candidate(
        &root.0.join("worker-e"),
        &[(
            "part.txt", b"3
",
        )],
    );
    // The individual files correspond. A clean textual combination with the
    // wrong result still fails the independent integration oracle.
    for (name, observed, succeeds) in [
        (
            "integrated-correct",
            b"sum:5
"
            .as_slice(),
            true,
        ),
        (
            "integrated-conflict",
            b"sum:4
"
            .as_slice(),
            false,
        ),
    ] {
        let integrated = candidate(
            &root.0.join(name),
            &[
                (
                    "left.txt", b"2
",
                ),
                (
                    "right.txt",
                    b"3
",
                ),
                ("summary.txt", observed),
            ],
        );
        let definition = definition(&left, &right, &integrated);
        let mut draft = CoordinationKernel::new(definition.clone()).unwrap();

        // These are scoped source-simulation artifact reviews. Integration below
        // independently reads the actual immutable Git blobs selected by them.
        let left_release = prepared_observed_release(&mut draft, "BF");
        let left_evidence = left_release.examinations[0].observation.clone();
        let left_operation = operation(
            &draft,
            "release-BF",
            Command::Release(Box::new(left_release)),
        );
        draft.admit(left_operation).unwrap();
        let right_release = prepared_observed_release(&mut draft, "EF");
        let right_operation = operation(
            &draft,
            "release-EF",
            Command::Release(Box::new(right_release)),
        );
        draft.admit(right_operation).unwrap();

        let key = StoreKey::new(&[0x44; 32]).unwrap();
        let state = root.0.join(format!("store-{name}"));
        let store = ProgramStore::create_supervised_fixture(
            &state,
            &key,
            definition,
            ReadLimits::new(8_000_000, Duration::from_secs(10)).unwrap(),
        )
        .unwrap();
        let worker_path = SourcePath::new(b"part.txt").unwrap();
        let left_path = SourcePath::new(b"left.txt").unwrap();
        let right_path = SourcePath::new(b"right.txt").unwrap();
        let summary = SourcePath::new(b"summary.txt").unwrap();
        let a = OperationId::new("release-BF").unwrap();
        let b = OperationId::new("release-EF").unwrap();
        let f = task("F");
        let request = |revision, duplicate, max_bytes| IntegrationRequest {
            task: &f,
            expected_revision: revision,
            constituents: [
                IntegrationConstituent {
                    release: &a,
                    worker_path: &worker_path,
                    integrated_path: &left_path,
                },
                IntegrationConstituent {
                    release: if duplicate { &a } else { &b },
                    worker_path: &worker_path,
                    integrated_path: &right_path,
                },
            ],
            oracle: IntegrationOracle {
                path: &summary,
                expected: b"sum:5\n",
            },
            max_bytes,
            workspace_observation: ObservationId::new("F", 2).unwrap(),
        };

        let last = draft.operations().len() - 1;
        for retained in &draft.operations()[..last] {
            store.admit(&key, retained.clone()).unwrap();
        }
        assert!(matches!(
            inspect_integrated_candidate(&store, &key, request(last as u64, false, 100)),
            Err(IntegrationError::Release)
        ));
        store.admit(&key, draft.operations()[last].clone()).unwrap();
        let revision = draft.revision();
        assert!(matches!(
            inspect_integrated_candidate(&store, &key, request(revision, true, 100)),
            Err(IntegrationError::Context)
        ));
        assert!(matches!(
            inspect_integrated_candidate(&store, &key, request(revision, false, 7)),
            Err(IntegrationError::Blob(_) | IntegrationError::Limit)
        ));
        let result =
            inspect_integrated_candidate(&store, &key, request(revision, false, 100)).unwrap();
        assert_eq!(result.constituents_match, [true, true]);
        assert_eq!(result.output_matches, succeeds);
        assert_eq!(
            result.candidate.observation().contract().candidate(),
            &integrated
        );
        assert_eq!(
            store.load(&key, Access::ReadOnly).unwrap().revision(),
            revision
        );

        let invalidation = operation(
            &draft,
            "invalidate-BF",
            Command::InvalidateEvidence {
                observation: left_evidence,
                reason: "The previously scoped artifact verification was withdrawn.".into(),
            },
        );
        store.admit(&key, invalidation).unwrap();
        assert!(matches!(
            inspect_integrated_candidate(&store, &key, request(revision + 1, false, 100)),
            Err(IntegrationError::Release)
        ));
    }
}
