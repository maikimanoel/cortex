//! A-01/11/21/24/32: actual Git capture/query -> Evidence -> advisory support.
mod support;
use cortex_cli::inspect_candidate;
use cortex_core::{
    CandidateId, TaskContract, TaskId, binding::*, parallel::*, temporal::SourceReference,
};
use cortex_evidence::{ExecutionObservation, Provenance, readiness::*, unified::*};
use std::{collections::BTreeMap, fs, path::PathBuf, time::SystemTime};
use support::Repo;

fn revision(n: u64) -> ContractRevision {
    ContractRevision::new(n).unwrap()
}
fn limits() -> SupportLimits {
    SupportLimits::new(8, 8, 8, 4_000_000).unwrap()
}
fn source() -> SourceReference {
    SourceReference::operator_reported("local fixture", "exact committed-HEAD predicate", "1")
        .unwrap()
}
fn contract(repo: &Repo, id: &str) -> CandidateBindingContract {
    let root = repo.path.canonicalize().unwrap();
    let head = String::from_utf8(repo.git(&["rev-parse", "--verify", "HEAD"]).stdout).unwrap();
    CandidateBindingContract::new(
        TaskContract::new(
            TaskId::new(id).unwrap(),
            "Check a bounded repository proposition",
            "The actual HEAD query equals the required committed revision",
        )
        .unwrap(),
        revision(1),
        CandidateRequirement::new(
            RepositoryIdentity::new(root.clone(), root.join(".git"), root.join(".git")).unwrap(),
            CandidateId::from_git_commit(head.trim()).unwrap(),
            WorktreeRequirement::Clean,
        ),
    )
}
fn capture(repo: &Repo, c: &CandidateBindingContract, n: u64) -> CandidateObservation {
    inspect_candidate(
        &repo.path,
        c,
        ObservationId::new(c.task().id().as_str(), n).unwrap(),
    )
    .unwrap()
}
fn profile(repo: &Repo) -> VerificationProfile {
    let version = String::from_utf8(repo.git(&["--version"]).stdout).unwrap();
    VerificationProfile::new(Provenance::new("actual Git query","--no-optional-locks rev-parse --verify HEAD; compare actual stdout to exact expected commit","fixture Git configuration and native inspection boundaries").unwrap(),version.trim(),&format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH),&[]).unwrap()
}
fn query(
    repo: &Repo,
    c: &CandidateBindingContract,
    p: &VerificationProfile,
    n: u64,
    subject: CandidateObservation,
) -> EvidenceRecord {
    let output = repo
        .command()
        .args(["--no-optional-locks", "rev-parse", "--verify", "HEAD"])
        .output()
        .unwrap();
    let observed = SystemTime::now();
    let conclusion = if !output.status.success() {
        OracleConclusion::Insufficient
    } else if String::from_utf8(output.stdout).unwrap().trim() == c.candidate().commit().as_str() {
        OracleConclusion::Satisfied
    } else {
        OracleConclusion::Violated
    };
    EvidenceRecord::new(
        ReportedObservation::new(
            ObservationId::new("actual-query-reports", n).unwrap(),
            subject,
            p.clone(),
            ObservationTimes::new(observed, SystemTime::now()).unwrap(),
            ObservationKind::Execution(
                VerificationOutcome::new(
                    ExecutionObservation::Exited {
                        code: u32::try_from(output.status.code().unwrap()).unwrap(),
                    },
                    1,
                    Some(
                        OracleEvaluation::new(
                            conclusion,
                            "Compare actual output, not merely successful exit",
                        )
                        .unwrap(),
                    ),
                )
                .unwrap(),
            ),
            "Actual fixture execution, still not an authenticated production report",
        )
        .unwrap(),
    )
}
fn assess(
    record: &mut EvidenceRecord,
    c: &CandidateBindingContract,
    p: &VerificationProfile,
    current: &CandidateObservation,
    id: &str,
) {
    record
        .assess(
            AssessmentId::new(id).unwrap(),
            SystemTime::now(),
            AssessmentResult::Pass,
            Applicability::Applicable,
            "One exact bounded proposition, no mission acceptance",
            &ValidationContext::new(c, Some(current), Some(p)),
        )
        .unwrap();
}
struct Fixture {
    producer: CandidateBindingContract,
    consumer: CandidateBindingContract,
    verification: CandidateBindingContract,
    projection: ReadinessProjection,
    contract: EdgeEvidenceContract,
    profile: VerificationProfile,
}
impl Fixture {
    fn new(repo: &Repo) -> Self {
        let producer = contract(repo, "producer");
        let consumer = contract(repo, "consumer");
        let verification = contract(repo, "verification");
        let p = profile(repo);
        let missions = [&producer, &consumer]
            .into_iter()
            .map(|c| {
                Mission::new(
                    MissionContext::new(c.clone(), Some(capture(repo, c, 1))).unwrap(),
                    ConsumerKind::Preview,
                    DependencyCoverage::DeclaredComplete,
                )
            })
            .collect();
        let artifact = ArtifactReference::new(
            ArtifactId::new("head-artifact").unwrap(),
            revision(1),
            verification.candidate().commit().clone(),
        );
        let edge = DependencyEdge::new(
            EdgeId::new("producer-consumer").unwrap(),
            producer.task().id().clone(),
            consumer.task().id().clone(),
            ArtifactRequirement::new(
                artifact,
                DependencyKind::Artifact,
                ReadinessLevel::IntegratedAccepted,
                vec![PredicateId::new("head-equals").unwrap()],
            )
            .unwrap(),
        );
        let projection = ReadinessProjection::new(
            DependencyGraph::new(
                GraphReference::new(GraphId::new("actual-query-fixture").unwrap(), revision(1)),
                missions,
                vec![edge],
                GraphLimits::new(2, 1, 16, 128, 4_000_000).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let scope =
            EdgeScopeSnapshot::capture(&projection, &EdgeId::new("producer-consumer").unwrap())
                .unwrap();
        let contract = EdgeEvidenceContract::new(
            scope,
            revision(1),
            verification.candidate().clone(),
            vec![PredicateCorrespondence::new(
                PredicateId::new("head-equals").unwrap(),
                verification.clone(),
                p.clone(),
                CriterionCoverage::ExactSingleCriterion,
            )],
            source(),
            limits(),
        )
        .unwrap();
        Self {
            producer,
            consumer,
            verification,
            projection,
            contract,
            profile: p,
        }
    }
    fn inspect<'e>(
        &self,
        repo: &Repo,
        records: &'e mut [EvidenceRecord],
        report_id: ObservationId,
        assessment_id: AssessmentId,
        sequence: u64,
    ) -> SupportReport<'e> {
        let producer = capture(repo, &self.producer, sequence);
        let consumer = capture(repo, &self.consumer, sequence);
        let current = capture(repo, &self.verification, sequence);
        let exams = [RecordExamination::new(
            report_id.clone(),
            &self.verification,
            Some(&current),
            Some(&self.profile),
        )];
        let bindings = [WitnessBinding::new(
            PredicateId::new("head-equals").unwrap(),
            EvidenceReference::new(report_id, Some(assessment_id)),
            self.contract.scope().requirement().artifact().clone(),
        )];
        inspect_support(
            &self.projection,
            &self.contract,
            records,
            &SupportRequest::new(Some(&producer), Some(&consumer), &exams, &bindings),
            limits(),
        )
        .unwrap()
    }
}
// Same controlled read-only property as the existing real-Git Evidence oracle:
// every fixture path and .git file/directory, content, mtime, length, read-only bit.
// Access time, inode/change/birth metadata are outside this oracle.
fn snapshot(repo: &Repo) -> BTreeMap<PathBuf, (Vec<u8>, SystemTime, u64, bool)> {
    let mut result = BTreeMap::new();
    let mut pending = vec![repo.path.clone()];
    while let Some(path) = pending.pop() {
        let meta = fs::symlink_metadata(&path).unwrap();
        assert!(!meta.is_symlink());
        let bytes = if meta.is_dir() {
            pending.extend(fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
            vec![]
        } else {
            fs::read(&path).unwrap()
        };
        result.insert(
            path.strip_prefix(&repo.path).unwrap().to_owned(),
            (
                bytes,
                meta.modified().unwrap(),
                meta.len(),
                meta.permissions().readonly(),
            ),
        );
    }
    result
}
#[test]
fn actual_query_support_is_read_only_advisory_and_preserves_source_and_judgment() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let f = Fixture::new(&repo);
    let before = snapshot(&repo);
    let graph = f.projection.clone();
    let mut record = query(
        &repo,
        &f.verification,
        &f.profile,
        1,
        capture(&repo, &f.verification, 1),
    );
    assess(
        &mut record,
        &f.verification,
        &f.profile,
        &capture(&repo, &f.verification, 2),
        "judgment-1",
    );
    let original = record.clone();
    let mut records = vec![record];
    for n in [3, 4] {
        let report = f.inspect(
            &repo,
            &mut records,
            ObservationId::new("actual-query-reports", 1).unwrap(),
            AssessmentId::new("judgment-1").unwrap(),
            n,
        );
        assert_eq!(
            report.evidence_part(),
            EvidencePart::AllRequiredPredicatesSupported
        );
        assert_eq!(
            report.inspected_records()[0].result(),
            Some(AssessmentResult::Pass)
        );
        assert_eq!(
            report.inspected_records()[0].validity(),
            Some(Validity::Current)
        );
        assert_eq!(report.authority(), ExecutionAuthority::Unavailable);
    }
    assert_eq!(records[0], original);
    assert_eq!(f.projection, graph);
    assert_eq!(snapshot(&repo), before);
}
#[test]
fn actual_changed_then_restored_source_requires_new_query_and_assessment() {
    let repo = Repo::new();
    repo.commit_file("file", "initial");
    let f = Fixture::new(&repo);
    let mut old = query(
        &repo,
        &f.verification,
        &f.profile,
        1,
        capture(&repo, &f.verification, 1),
    );
    assess(
        &mut old,
        &f.verification,
        &f.profile,
        &capture(&repo, &f.verification, 2),
        "judgment-1",
    );
    let original = old.clone();
    let mut records = vec![old];
    fs::write(repo.path.join("file"), "changed bytes").unwrap();
    let changed_snapshot = snapshot(&repo);
    let report = f.inspect(
        &repo,
        &mut records,
        ObservationId::new("actual-query-reports", 1).unwrap(),
        AssessmentId::new("judgment-1").unwrap(),
        3,
    );
    assert_eq!(report.evidence_part(), EvidencePart::Incomplete);
    assert_eq!(
        report.inspected_records()[0].validity(),
        Some(Validity::Stale)
    );
    drop(report);
    assert_eq!(snapshot(&repo), changed_snapshot);
    fs::write(repo.path.join("file"), "initial").unwrap();
    let report = f.inspect(
        &repo,
        &mut records,
        ObservationId::new("actual-query-reports", 1).unwrap(),
        AssessmentId::new("judgment-1").unwrap(),
        4,
    );
    assert_eq!(report.evidence_part(), EvidencePart::Incomplete);
    assert_eq!(
        report.inspected_records()[0].validity(),
        Some(Validity::Stale)
    );
    drop(report);
    assert_eq!(records[0].observation(), original.observation());
    assert_eq!(records[0].assessment(), original.assessment());
    let stale = records[0].clone();
    let before = snapshot(&repo);
    let mut new = query(
        &repo,
        &f.verification,
        &f.profile,
        2,
        capture(&repo, &f.verification, 5),
    );
    assess(
        &mut new,
        &f.verification,
        &f.profile,
        &capture(&repo, &f.verification, 6),
        "judgment-2",
    );
    assert_ne!(new.observation().id(), original.observation().id());
    assert_ne!(
        new.assessment().unwrap().id(),
        original.assessment().unwrap().id()
    );
    records.push(new);
    let report = f.inspect(
        &repo,
        &mut records,
        ObservationId::new("actual-query-reports", 2).unwrap(),
        AssessmentId::new("judgment-2").unwrap(),
        7,
    );
    assert_eq!(
        report.evidence_part(),
        EvidencePart::AllRequiredPredicatesSupported
    );
    assert_eq!(report.inspected_records().len(), 1);
    drop(report);
    assert_eq!(records[0], stale);
    assert_eq!(snapshot(&repo), before);
}
