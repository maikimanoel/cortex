//! CAP-CLI-003: bounded composition, not an execution runner or permission engine.

mod input;
mod output;

use cortex_cli::{InspectionError, RepositoryObservation, inspect};
use cortex_core::{CandidateId, ContractError, binding::*};
use cortex_evidence::{ExecutionObservation, Provenance, unified::*};
use input::Input;
use std::{
    error::Error,
    ffi::OsString,
    fmt,
    io::{self, Write},
    time::{SystemTime, UNIX_EPOCH},
};

const CRITERION: &str = "git.head-equals.v1";
const HELP: &str = "Usage: cortex check [--json] --task <ID> --revision <N> --candidate <OID> --expect-head <OID> [--repo <PATH>]

Check only whether locally resolved committed HEAD equals --expect-head.
--candidate pins the subject, independently of the expected value in the criterion.
--revision is a positive task-contract revision; --repo defaults to current directory.
--json must immediately follow check and selects output schema version 1.
A conclusive result needs matching fresh observations of a clean committed candidate.
Exit: 0 current PASS; 8 current FAIL; 9 unresolved; 1 output; 2 input;
      3 missing Git; 4 inspection failure; 5 unsupported; 6 resource; 7 inconsistent.
No arbitrary software tests or agents run. Evidence grants no permission.
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Human,
    Json,
}

#[derive(Debug)]
enum ErrorKind {
    Input(&'static str),
    Contract(ContractError),
    Binding(BindingError),
    Evidence(SemanticError),
    Inspection(InspectionError),
    Output(io::Error),
    Limit,
    Clock,
}

#[derive(Debug)]
pub(super) struct CheckError {
    format: Format,
    kind: ErrorKind,
}
impl CheckError {
    fn new(format: Format, kind: ErrorKind) -> Self {
        Self { format, kind }
    }
    pub fn exit_code(&self) -> u8 {
        match &self.kind {
            ErrorKind::Input(_) | ErrorKind::Contract(_) | ErrorKind::Binding(_) => 2,
            ErrorKind::Output(_) => 1,
            ErrorKind::Inspection(e) => e.exit_code(),
            ErrorKind::Limit => 6,
            ErrorKind::Evidence(_) | ErrorKind::Clock => 7,
        }
    }
    pub fn write_machine_error(&self, out: &mut impl Write) -> io::Result<()> {
        if self.format == Format::Json && !matches!(self.kind, ErrorKind::Output(_)) {
            output::write_error(self, out)?;
        }
        Ok(())
    }
    fn category(&self) -> &'static str {
        match &self.kind {
            ErrorKind::Input(_) | ErrorKind::Contract(_) | ErrorKind::Binding(_) => "INVALID_INPUT",
            ErrorKind::Output(_) => "OUTPUT_FAILURE",
            ErrorKind::Inspection(e) => output::inspection_category(e),
            ErrorKind::Limit => "RESOURCE_LIMIT",
            ErrorKind::Evidence(_) | ErrorKind::Clock => "INVALID_OBSERVATION",
        }
    }
}
impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::Input(message) => f.write_str(message),
            ErrorKind::Contract(e) => e.fmt(f),
            ErrorKind::Binding(e) => e.fmt(f),
            ErrorKind::Evidence(_) => {
                f.write_str("evidence construction or validation failed; no accepted result")
            }
            ErrorKind::Inspection(e) => e.fmt(f),
            ErrorKind::Output(_) => f.write_str("could not write check output"),
            ErrorKind::Limit => f.write_str("check output exceeds 65536 bytes; no complete report"),
            ErrorKind::Clock => {
                f.write_str("system clock cannot supply ordered evidence timestamps")
            }
        }
    }
}
impl Error for CheckError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            ErrorKind::Contract(e) => Some(e),
            ErrorKind::Binding(e) => Some(e),
            ErrorKind::Evidence(e) => Some(e),
            ErrorKind::Inspection(e) => Some(e),
            ErrorKind::Output(e) => Some(e),
            _ => None,
        }
    }
}

struct Report {
    expected: CandidateId,
    contract: CandidateBindingContract,
    evidence: EvidenceRecord,
    latest: Option<CandidateObservation>,
    profile: VerificationProfile,
    binding: BindingDecision,
    refresh_error: Option<InspectionError>,
}
impl Report {
    fn view(&mut self) -> EvidenceView<'_> {
        self.evidence.inspect(&ValidationContext::new(
            &self.contract,
            self.latest.as_ref(),
            Some(&self.profile),
        ))
    }
    fn exit_code(&mut self) -> u8 {
        if let Some(e) = &self.refresh_error {
            return e.exit_code();
        }
        let view = self.view();
        if view.current_pass() {
            0
        } else if view.result() == Some(AssessmentResult::Fail)
            && view.validity() == Some(Validity::Current)
        {
            8
        } else {
            9
        }
    }
}

fn profile(
    git_version: &str,
    expected: &CandidateId,
) -> Result<VerificationProfile, SemanticError> {
    VerificationProfile::new(
        Provenance::new(
            "CORTEX git.head-equals.v1",
            &format!("resolved HEAD equals {}", expected.as_str()),
            "002-A-v1 fixed read-only controls; two inspections; AllowChanges binding; raw repository configuration omitted",
        )?,
        env!("CARGO_PKG_VERSION"),
        &format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        &[("git", git_version), ("cortex-inspection", "002-A-v1")],
    )
}

fn evaluate(
    input: Input,
    mut capture: impl FnMut(&std::path::Path) -> Result<RepositoryObservation, InspectionError>,
) -> Result<Report, ErrorKind> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ErrorKind::Clock)?;
    let scope = format!(
        "cli-{}-{}-{}",
        std::process::id(),
        stamp.as_secs(),
        stamp.subsec_nanos()
    );
    let first = capture(&input.repo).map_err(ErrorKind::Inspection)?;
    let initial_profile =
        profile(first.git_version(), &input.expected).map_err(ErrorKind::Evidence)?;
    let contract = CandidateBindingContract::new(
        input.task,
        input.revision,
        CandidateRequirement::new(
            first.repository_identity().map_err(ErrorKind::Inspection)?,
            input.candidate,
            WorktreeRequirement::AllowChanges,
        ),
    );
    let source = first
        .into_candidate(
            &contract,
            ObservationId::new(&scope, 1).map_err(ErrorKind::Binding)?,
        )
        .map_err(ErrorKind::Inspection)?;
    let latest = capture(&input.repo).and_then(|observation| {
        let p = profile(observation.git_version(), &input.expected)
            .map_err(|_| InspectionError::Malformed)?;
        let observed = observation.into_candidate(
            &contract,
            ObservationId::new(&scope, 2).map_err(InspectionError::InvalidObservation)?,
        )?;
        Ok((observed, p))
    });
    finish(
        input.expected,
        contract,
        source,
        initial_profile,
        &scope,
        latest,
    )
}

// Private seam permits controlled changes/failures without weakening real capture.
fn finish(
    expected: CandidateId,
    contract: CandidateBindingContract,
    source: CandidateObservation,
    initial_profile: VerificationProfile,
    scope: &str,
    latest: Result<(CandidateObservation, VerificationProfile), InspectionError>,
) -> Result<Report, ErrorKind> {
    let (latest, current_profile, refresh_error) = match latest {
        Ok((candidate, profile)) => (Some(candidate), profile, None),
        Err(error) => (None, initial_profile.clone(), Some(error)),
    };
    let binding =
        bind_candidate(&contract, Some(&source), latest.as_ref()).map_err(ErrorKind::Binding)?;
    let (kind, result, detail) = if let Some(error) = &refresh_error {
        if matches!(
            error,
            InspectionError::Inconsistent
                | InspectionError::Malformed
                | InspectionError::InvalidObservation(_)
        ) {
            (
                ObservationKind::Inference,
                AssessmentResult::Unknown,
                "A source was captured, but fresh candidate consistency could not be established; no criterion comparison was performed",
            )
        } else {
            (
                ObservationKind::Execution(
                    VerificationOutcome::new(ExecutionObservation::Unavailable, 0, None)
                        .map_err(ErrorKind::Evidence)?,
                ),
                AssessmentResult::Blocked,
                "Fresh repository observation prerequisite was unavailable; no criterion comparison was performed",
            )
        }
    } else {
        // Completed 002-A capture establishes that the actual porcelain status
        // queries exited zero. This is their outcome, never the CORTEX exit code.
        // The separate oracle below performs exactly one committed-HEAD comparison.
        let conclusion = match source.state().head() {
            Head::Attached { commit, .. } | Head::Detached { commit } if commit == &expected => {
                OracleConclusion::Satisfied
            }
            Head::Attached { .. } | Head::Detached { .. } => OracleConclusion::Violated,
            Head::Unborn { .. } | Head::Unknown => OracleConclusion::Insufficient,
        };
        let result = match conclusion {
            OracleConclusion::Satisfied => AssessmentResult::Pass,
            OracleConclusion::Violated => AssessmentResult::Fail,
            OracleConclusion::Insufficient => AssessmentResult::Unknown,
        };
        (ObservationKind::Execution(VerificationOutcome::new(ExecutionObservation::Exited { code: 0 }, 1,
            Some(OracleEvaluation::new(conclusion, "Compare the locally resolved source HEAD with the criterion's expected full commit; absence is insufficient. This establishes no source-code correctness.").map_err(ErrorKind::Evidence)?)).map_err(ErrorKind::Evidence)?), result,
            "Actual bounded Git status/HEAD capture plus one explicit committed-revision equality oracle; ignored files and uncommitted byte identity are outside coverage")
    };
    let observation = ReportedObservation::new(
        ObservationId::new(scope, 3).map_err(ErrorKind::Binding)?,
        source,
        initial_profile,
        ObservationTimes::new(SystemTime::now(), SystemTime::now()).map_err(ErrorKind::Evidence)?,
        kind,
        detail,
    )
    .map_err(ErrorKind::Evidence)?;
    let mut evidence = EvidenceRecord::new(observation);
    let assessment_id =
        AssessmentId::new(&format!("assessment-{scope}")).map_err(ErrorKind::Evidence)?;
    let context = ValidationContext::new(&contract, latest.as_ref(), Some(&current_profile));
    let unresolved = "Candidate/profile correspondence or observation coverage prevents a conclusive assessment; inspect the typed binding and validity cause";
    let assessment = evidence.assess(assessment_id.clone(), SystemTime::now(), result,
        Applicability::Applicable,
        match result {
            AssessmentResult::Pass => "The equality oracle is satisfied for the separately bound committed candidate within Git status coverage only",
            AssessmentResult::Fail => "The equality oracle demonstrates a different committed HEAD for the separately bound candidate",
            AssessmentResult::Blocked => "Reinspection prerequisite failed; retained source cannot establish current criterion acceptance",
            _ => unresolved,
        }, &context);
    // Let the canonical Evidence boundary decide currentness; do not duplicate
    // its freshness policy in the CLI. Only this specific qualification permits
    // an explicit UNKNOWN after a rejected conclusive judgment. No terminal
    // assessment is replaced, and demonstrated invalidation is retained by 002-C.
    match assessment {
        Err(SemanticError::CurrentContextRequired)
            if matches!(result, AssessmentResult::Pass | AssessmentResult::Fail) =>
        {
            evidence
                .assess(
                    assessment_id,
                    SystemTime::now(),
                    AssessmentResult::Unknown,
                    Applicability::Applicable,
                    unresolved,
                    &context,
                )
                .map_err(ErrorKind::Evidence)?;
        }
        other => other.map_err(ErrorKind::Evidence)?,
    }
    Ok(Report {
        expected,
        contract,
        evidence,
        latest,
        profile: current_profile,
        binding,
        refresh_error,
    })
}

pub(super) fn run(
    args: impl Iterator<Item = OsString>,
    out: &mut impl Write,
    diagnostics: &mut impl Write,
) -> Result<u8, CheckError> {
    let mut args = args.peekable();
    let first = args.next();
    if first.as_deref() == Some(std::ffi::OsStr::new("--help")) && args.peek().is_none() {
        out.write_all(HELP.as_bytes())
            .map_err(|e| CheckError::new(Format::Human, ErrorKind::Output(e)))?;
        return Ok(0);
    }
    let format = if first.as_deref() == Some(std::ffi::OsStr::new("--json")) {
        Format::Json
    } else {
        Format::Human
    };
    let remaining = first
        .into_iter()
        .filter(|_| format != Format::Json)
        .chain(args);
    let input = Input::parse(remaining, format)?;
    let mut report = evaluate(input, inspect).map_err(|e| CheckError::new(format, e))?;
    let exit = report.exit_code();
    let bytes = output::render(&mut report, format).map_err(|e| CheckError::new(format, e))?;
    out.write_all(&bytes)
        .map_err(|e| CheckError::new(format, ErrorKind::Output(e)))?;
    if let Some(error) = &report.refresh_error {
        writeln!(diagnostics, "cortex: {error}")
            .map_err(|e| CheckError::new(format, ErrorKind::Output(e)))?;
    }
    Ok(exit)
}

#[cfg(test)]
mod tests;
