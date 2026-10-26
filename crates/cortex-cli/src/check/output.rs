//! Versioned presentation projection; domain decisions stay in Core and Evidence.

use super::*;
use serde_json::{Value, json};

const MAX_OUTPUT: usize = 65_536;
const LIMITS: [&str; 5] = [
    "Only committed-HEAD equality is checked; no source-code correctness or test-suite claim",
    "Tracked and non-ignored untracked Git status coverage; ignored files excluded",
    "Two inspections, each with two matching samples; not an atomic snapshot or permanent freshness",
    "HEAD identifies committed revision only; no uncommitted byte identity",
    "Provenance is unauthenticated; raw configuration, index, filenames and comparison bytes omitted",
];

#[derive(Default)]
struct Buffer {
    bytes: Vec<u8>,
    exceeded: bool,
}
impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_OUTPUT.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(io::Error::from(io::ErrorKind::FileTooLarge));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn render(report: &mut Report, format: Format) -> Result<Vec<u8>, ErrorKind> {
    let mut buffer = Buffer::default();
    let result = match format {
        Format::Json => serde_json::to_writer(&mut buffer, &project(report)?)
            .map_err(io::Error::other)
            .and_then(|()| writeln!(&mut buffer)),
        Format::Human => human(report, &mut buffer),
    };
    if buffer.exceeded {
        return Err(ErrorKind::Limit);
    }
    result.map_err(ErrorKind::Output)?;
    Ok(buffer.bytes)
}

pub(super) fn write_error(error: &CheckError, out: &mut impl Write) -> io::Result<()> {
    // Error fields are static diagnostics, never payloads or raw transcripts.
    serde_json::to_writer(
        &mut *out,
        &json!({
            "schema_version":1, "type":"error", "assessment":null,
            "error":{"category":error.category(), "message":error.to_string()},
            "exit_code":error.exit_code(), "authority":"NONE",
        }),
    )
    .map_err(io::Error::other)?;
    writeln!(out)
}

pub(super) fn inspection_category(error: &InspectionError) -> &'static str {
    match error {
        InspectionError::InvalidPath(_) | InspectionError::NotDirectory => "INVALID_PATH",
        InspectionError::MissingGit => "GIT_UNAVAILABLE",
        InspectionError::Io(_) | InspectionError::GitFailed(_) => "INSPECTION_FAILURE",
        InspectionError::Unsupported(_) => "UNSUPPORTED",
        InspectionError::Limit(_) => "RESOURCE_LIMIT",
        InspectionError::Inconsistent => "INCONSISTENT_OBSERVATION",
        InspectionError::Malformed | InspectionError::InvalidObservation(_) => {
            "INVALID_OBSERVATION"
        }
    }
}

fn result_label(result: AssessmentResult) -> &'static str {
    match result {
        AssessmentResult::Pass => "PASS",
        AssessmentResult::Fail => "FAIL",
        AssessmentResult::Unknown => "UNKNOWN",
        AssessmentResult::NotRun => "NOT_RUN",
        AssessmentResult::Blocked => "BLOCKED",
        AssessmentResult::NotApplicable => "N/A",
    }
}
fn applicability_label(value: Applicability) -> &'static str {
    match value {
        Applicability::Applicable => "APPLICABLE",
        Applicability::NotApplicable => "NOT_APPLICABLE",
        Applicability::Uncertain => "UNCERTAIN",
    }
}
fn mismatch(value: BindingMismatch) -> &'static str {
    match value {
        BindingMismatch::TaskIdentity => "TASK_IDENTITY",
        BindingMismatch::Repository => "REPOSITORY",
        BindingMismatch::Commit => "COMMIT",
        BindingMismatch::CommitAbsent => "COMMIT_ABSENT",
        BindingMismatch::WorkingTree => "WORKING_TREE",
    }
}
fn stale(value: BindingStaleness) -> &'static str {
    match value {
        BindingStaleness::ContractRevision => "CONTRACT_REVISION",
        BindingStaleness::ObservationOrder => "OBSERVATION_ORDER",
        BindingStaleness::ObservedStateChanged => "OBSERVED_STATE_CHANGED",
    }
}
fn insufficient(value: BindingInsufficiency) -> &'static str {
    match value {
        BindingInsufficiency::MissingObservation => "MISSING_OBSERVATION",
        BindingInsufficiency::ReobservationRequired => "REOBSERVATION_REQUIRED",
        BindingInsufficiency::DifferentObservationScope => "DIFFERENT_OBSERVATION_SCOPE",
        BindingInsufficiency::IncompleteCoverage => "INCOMPLETE_COVERAGE",
        BindingInsufficiency::IndeterminateConsistency => "INDETERMINATE_CONSISTENCY",
        BindingInsufficiency::MissingRepository => "MISSING_REPOSITORY",
        BindingInsufficiency::MissingCommit => "MISSING_COMMIT",
        BindingInsufficiency::UncommittedIdentity => "UNCOMMITTED_IDENTITY",
        BindingInsufficiency::MissingComparisonBasis => "MISSING_COMPARISON_BASIS",
    }
}
fn binding(value: &BindingDecision) -> Value {
    let (status, reason) = match value {
        BindingDecision::Compatible(_) => ("COMPATIBLE", None),
        BindingDecision::Incompatible(r) => ("INCOMPATIBLE", Some(mismatch(*r))),
        BindingDecision::Stale(r) => ("STALE", Some(stale(*r))),
        BindingDecision::Insufficient(r) => ("INSUFFICIENT", Some(insufficient(*r))),
    };
    json!({"status":status, "reason":reason})
}
fn issue(value: &ValidityIssue) -> Value {
    let (kind, cause) = match value {
        ValidityIssue::CandidateMismatch(r) => ("CANDIDATE_MISMATCH", Some(mismatch(*r))),
        ValidityIssue::CandidateStale(r) => ("CANDIDATE_STALE", Some(stale(*r))),
        ValidityIssue::CandidateInsufficient(r) => {
            ("CANDIDATE_INSUFFICIENT", Some(insufficient(*r)))
        }
        ValidityIssue::InvalidContext(_) => ("INVALID_CONTEXT", None),
        ValidityIssue::ProfileChanged => ("PROFILE_CHANGED", None),
        ValidityIssue::MissingProfile => ("MISSING_PROFILE", None),
        ValidityIssue::ObservationIdentityCollision => ("OBSERVATION_IDENTITY_COLLISION", None),
        ValidityIssue::ExplicitInvalidation => ("EXPLICIT_INVALIDATION", None),
    };
    json!({"kind":kind, "cause":cause})
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 15) as usize] as char);
    }
    output
}
fn bytes(value: &[u8]) -> Value {
    match std::str::from_utf8(value) {
        Ok(text) => json!({"encoding":"utf-8", "value":text}),
        Err(_) => json!({"encoding":"hex", "value":hex(value)}),
    }
}
fn display_bytes(value: &[u8]) -> String {
    match std::str::from_utf8(value) {
        Ok(text) => format!("{text:?}"),
        Err(_) => format!("hex:{}", hex(value)),
    }
}
fn id(value: &ObservationId) -> Value {
    json!({"scope":value.scope(), "sequence":value.sequence()})
}
fn timestamp(value: SystemTime) -> Result<Value, ErrorKind> {
    let duration = value
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ErrorKind::Clock)?;
    Ok(json!({"unix_seconds":duration.as_secs(), "nanoseconds":duration.subsec_nanos()}))
}
fn candidate(value: &CandidateObservation) -> Value {
    let state = value.state();
    let (kind, commit, branch) = match state.head() {
        Head::Attached { branch, commit } => {
            ("ATTACHED", Some(commit.as_str()), Some(bytes(branch)))
        }
        Head::Detached { commit } => ("DETACHED", Some(commit.as_str()), None),
        Head::Unborn { branch } => ("UNBORN", None, Some(bytes(branch))),
        Head::Unknown => ("UNKNOWN", None, None),
    };
    let staged = state
        .entries()
        .iter()
        .filter(|e| {
            matches!(
                e,
                Entry::Tracked {
                    staged: Some(_),
                    ..
                }
            )
        })
        .count();
    let unstaged = state
        .entries()
        .iter()
        .filter(|e| {
            matches!(
                e,
                Entry::Tracked {
                    unstaged: Some(_),
                    ..
                }
            )
        })
        .count();
    let untracked = state
        .entries()
        .iter()
        .filter(|e| matches!(e, Entry::Untracked { .. }))
        .count();
    let coverage = match state.coverage() {
        ObservationCoverage::GitStatus => "GIT_STATUS",
        ObservationCoverage::Incomplete => "INCOMPLETE",
    };
    let consistency = match state.consistency() {
        ObservationConsistency::TwoMatchingSamples => "TWO_MATCHING_SAMPLES",
        ObservationConsistency::Indeterminate => "INDETERMINATE",
    };
    let working_tree = if state.coverage() != ObservationCoverage::GitStatus
        || state.consistency() != ObservationConsistency::TwoMatchingSamples
        || state.repository().is_none()
        || matches!(state.head(), Head::Unknown)
    {
        "INDETERMINATE"
    } else if state.entries().is_empty() {
        "CLEAN"
    } else {
        "DIRTY"
    };
    json!({"observation_id":id(value.id()),
        "repository_root":state.repository().map(|r| bytes(r.root().as_os_str().as_encoded_bytes())),
        "head":{"kind":kind,"commit":commit,"branch":branch},
        "working_tree":{"state":working_tree,"staged":staged,"unstaged":unstaged,"untracked":untracked},
        "coverage":coverage,"consistency":consistency})
}
fn observation(value: &ObservationKind) -> Value {
    match value {
        ObservationKind::Execution(outcome) => {
            let (status, code) = match outcome.execution() {
                ExecutionObservation::Exited { code } => ("EXITED", Some(code)),
                ExecutionObservation::NotRun => ("NOT_RUN", None),
                ExecutionObservation::TimedOut => ("TIMED_OUT", None),
                ExecutionObservation::Cancelled => ("CANCELLED", None),
                ExecutionObservation::Unavailable => ("UNAVAILABLE", None),
            };
            json!({"kind":"EXECUTION", "execution":{"status":status,"exit_code":code},"relevant_checks":outcome.checks(),
                "oracle":outcome.oracle().map(|o| json!({"conclusion":match o.conclusion() { OracleConclusion::Satisfied=>"SATISFIED", OracleConclusion::Violated=>"VIOLATED",OracleConclusion::Insufficient=>"INSUFFICIENT"}, "reasoning":o.reasoning()}))})
        }
        ObservationKind::Inference => {
            json!({"kind":"INFERENCE", "execution":null,"relevant_checks":0,"oracle":null})
        }
        ObservationKind::ScopeInspection(finding) => {
            json!({"kind":"SCOPE_INSPECTION", "finding":match finding { ScopeFinding::Present=>"PRESENT",ScopeFinding::Absent=>"ABSENT",ScopeFinding::Uncertain=>"UNCERTAIN"}, "execution":null,"relevant_checks":0,"oracle":null})
        }
    }
}
fn project(report: &mut Report) -> Result<Value, ErrorKind> {
    let exit_code = report.exit_code();
    let (validity, validity_issue, current_pass) = {
        let view = report.view();
        (
            view.validity(),
            view.validity_issue().map(issue),
            view.current_pass(),
        )
    };
    let record = report.evidence.observation();
    let profile = record.profile();
    let provenance = profile.provenance();
    let task = report.contract.task();
    let assessment = if let Some(a) = report.evidence.assessment() {
        json!({
            "id": a.id().as_str(),
            "result": result_label(a.result()),
            "applicability": applicability_label(a.applicability()),
            "validity": match validity {
                Some(Validity::Current) => "CURRENT",
                Some(Validity::Stale) => "STALE",
                None => "UNESTABLISHED",
            },
            "validity_issue": validity_issue,
            "current_pass": current_pass,
            "reasoning": a.rationale(),
            "assessed_at": timestamp(a.assessed_at())?,
        })
    } else {
        Value::Null
    };
    Ok(json!({
        "schema_version": 1,
        "type": "assessment",
        "exit_code": exit_code,
        "criterion": { "id": CRITERION, "expected_head": report.expected.as_str() },
        "task": {
            "id": task.id().as_str(),
            "revision": report.contract.revision().get().to_string(),
            "objective": task.objective(),
            "acceptance": task.acceptance(),
        },
        "required_candidate": report.contract.candidate().commit().as_str(),
        "source": candidate(record.subject()),
        "reobserved": report.latest.as_ref().map(candidate),
        "binding": binding(&report.binding),
        "observation": {
            "id": id(record.id()),
            "source": "CALLER_REPORTED",
            "authenticated": false,
            "producer": "CLI bounded local Git adapter",
            "observed_at": timestamp(record.times().observed())?,
            "recorded_at": timestamp(record.times().recorded())?,
            "mechanism": {
                "name": provenance.mechanism(),
                "version": profile.version(),
                "inputs": provenance.inputs(),
                "configuration": provenance.configuration(),
                "environment": profile.environment(),
                "components": profile.components().iter().map(|(name, version)|
                    json!({ "name": name, "version": version })).collect::<Vec<_>>(),
            },
            "outcome": observation(record.kind()),
            "detail": record.detail(),
        },
        "assessment": assessment,
        "error": report.refresh_error.as_ref().map(|e|
            json!({ "category": inspection_category(e), "message": e.to_string() })),
        "limits": LIMITS,
        "authority": "NONE",
    }))
}

fn human(report: &mut Report, out: &mut impl Write) -> io::Result<()> {
    writeln!(
        out,
        "Criterion: {CRITERION}\nExpected HEAD: {}",
        report.expected.as_str()
    )?;
    writeln!(
        out,
        "Task: {}\nContract revision: {}\nRequired candidate: {}",
        report.contract.task().id().as_str(),
        report.contract.revision().get(),
        report.contract.candidate().commit().as_str()
    )?;
    let record = report.evidence.observation();
    if let Some(repository) = record.subject().state().repository() {
        writeln!(out, "Repository: {:?}", repository.root())?;
    }
    match record.subject().state().head() {
        Head::Attached { commit, branch } => writeln!(
            out,
            "Observed HEAD: {}\nBranch: {}",
            commit.as_str(),
            display_bytes(branch)
        )?,
        Head::Detached { commit } => {
            writeln!(out, "Observed HEAD: {}\nBranch: DETACHED", commit.as_str())?
        }
        Head::Unborn { branch } => writeln!(
            out,
            "Observed HEAD: NONE (unborn)\nBranch: {}",
            display_bytes(branch)
        )?,
        Head::Unknown => writeln!(out, "Observed HEAD: UNKNOWN")?,
    }
    writeln!(out, "Binding: {}", binding(&report.binding))?;
    writeln!(
        out,
        "Mechanism: {} ({}); environment: {}",
        record.profile().provenance().mechanism(),
        record.profile().version(),
        record.profile().environment()
    )?;
    for (name, version) in record.profile().components() {
        writeln!(out, "Component: {name} {version:?}")?;
    }
    writeln!(
        out,
        "Observed: {:?}; recorded: {:?}",
        record.times().observed(),
        record.times().recorded()
    )?;
    if let Some(a) = report.evidence.assessment() {
        writeln!(
            out,
            "Reason: {}\nAssessed: {:?}",
            a.rationale(),
            a.assessed_at()
        )?;
    }
    report.view().write_redacted(out)?;
    for limit in LIMITS {
        writeln!(out, "Limit: {limit}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_limit_accepts_exact_bound_and_rejects_next_byte_without_growth() {
        let mut buffer = Buffer::default();
        buffer.write_all(&vec![b'x'; MAX_OUTPUT]).unwrap();
        assert!(buffer.write_all(b"x").is_err());
        assert_eq!(buffer.bytes.len(), MAX_OUTPUT);
        assert!(buffer.exceeded);
    }
    #[test]
    fn byte_encoding_is_lossless_and_json_is_escaped_by_the_serializer() {
        for input in [
            b"quote\"\\\n\t\0".as_slice(),
            "Unicode 雪".as_bytes(),
            &[255, 0, 34],
        ] {
            let value = bytes(input);
            let encoded = serde_json::to_vec(&value).unwrap();
            let parsed: Value = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(parsed, value);
            if let Ok(text) = std::str::from_utf8(input) {
                assert_eq!(parsed["value"], text);
            } else {
                assert_eq!(parsed["value"], "ff0022");
            }
        }
        assert!(!display_bytes(b"\x1b\n").contains('\n'));
        assert!(!display_bytes(b"\x1b\n").contains('\x1b'));
    }
}
