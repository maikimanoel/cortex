//! Evidence records keep source identity, execution reports, and judgment separate.
//!
//! Callers supply observations and provenance. This library validates consistency;
//! it neither executes verification nor authenticates that a report is truthful.

/// Candidate-bound evidence with independent result, applicability and validity.
pub mod unified;

use cortex_core::{CandidateId, TaskContract};
use std::{error::Error, fmt};

/// Maximum UTF-8 byte length of each provenance field and assessment rationale.
pub const MAX_EVIDENCE_TEXT_BYTES: usize = 4096;

/// Invalid provenance or an unsupported assessment transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceError {
    /// A required field contains no non-whitespace text.
    Blank(&'static str),
    /// A field exceeds the documented byte limit.
    TooLong(&'static str),
    /// A field contains a control character other than line feed.
    InvalidCharacter(&'static str),
    /// A record already has its terminal assessment.
    AlreadyAssessed,
    /// A pass or fail was requested without a completed execution.
    ExecutionNotCompleted,
    /// A pass was requested for an execution with a nonzero exit code.
    NonzeroExitCannotPass,
}

impl fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blank(field) => write!(f, "{field} must contain text"),
            Self::TooLong(field) => {
                write!(f, "{field} exceeds {MAX_EVIDENCE_TEXT_BYTES} bytes")
            }
            Self::InvalidCharacter(field) => write!(f, "{field} contains a forbidden character"),
            Self::AlreadyAssessed => f.write_str("evidence already has a terminal assessment"),
            Self::ExecutionNotCompleted => {
                f.write_str("pass or fail requires a completed execution")
            }
            Self::NonzeroExitCannotPass => {
                f.write_str("nonzero execution exit cannot support pass")
            }
        }
    }
}

impl Error for EvidenceError {}

/// Operator-supplied descriptions of the verification mechanism and its context.
///
/// Each field is nonblank, at most 4096 UTF-8 bytes, and excludes control
/// characters except line feed. Include tool versions, input identities, relevant
/// configuration, and environment in these descriptions. Do not include secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    mechanism: String,
    inputs: String,
    configuration: String,
}

impl Provenance {
    /// Validate provenance before creating an owned record.
    pub fn new(mechanism: &str, inputs: &str, configuration: &str) -> Result<Self, EvidenceError> {
        validate_text("mechanism", mechanism)?;
        validate_text("inputs", inputs)?;
        validate_text("configuration", configuration)?;
        Ok(Self {
            mechanism: mechanism.to_owned(),
            inputs: inputs.to_owned(),
            configuration: configuration.to_owned(),
        })
    }

    /// Read the mechanism and its version supplied by the reporter.
    pub fn mechanism(&self) -> &str {
        &self.mechanism
    }

    /// Read the description of the verification inputs.
    pub fn inputs(&self) -> &str {
        &self.inputs
    }

    /// Read relevant settings and environmental conditions.
    pub fn configuration(&self) -> &str {
        &self.configuration
    }
}

/// A caller-reported execution status, never a semantic correctness verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionObservation {
    /// Verification was not attempted.
    NotRun,
    /// A process completed with the reported exit code.
    Exited {
        /// The process's nonnegative exit code.
        code: u32,
    },
    /// Verification reached its time limit without a completed result.
    TimedOut,
    /// Verification was interrupted without a completed result.
    Cancelled,
    /// A required prerequisite prevented verification from starting.
    Unavailable,
}

/// A scoped semantic judgment about the task's acceptance criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// A completed, zero-exit mechanism plus the stated reasoning support acceptance.
    Pass,
    /// A completed mechanism plus the stated reasoning demonstrate a violation.
    Fail,
    /// Available evidence does not establish acceptance or a violation.
    Inconclusive,
}

/// An explicit judgment and the reasoning supplied by its author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assessment {
    verdict: Verdict,
    rationale: String,
}

impl Assessment {
    /// Read the scoped judgment.
    pub fn verdict(&self) -> Verdict {
        self.verdict
    }

    /// Read the reasoning connecting observations to the acceptance criterion.
    pub fn rationale(&self) -> &str {
        &self.rationale
    }
}

/// One immutable observation context and at most one terminal assessment.
///
/// Construction leaves the record unassessed, including after exit code zero.
/// Reassessment requires a new record; candidate and contract cannot be relabeled.
/// No assessment is human approval or permission to execute another operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    contract: TaskContract,
    candidate: CandidateId,
    provenance: Provenance,
    observation: ExecutionObservation,
    assessment: Option<Assessment>,
}

impl Evidence {
    /// Bind a report to its exact supplied contract, candidate, and provenance.
    pub fn new(
        contract: TaskContract,
        candidate: CandidateId,
        provenance: Provenance,
        observation: ExecutionObservation,
    ) -> Self {
        Self {
            contract,
            candidate,
            provenance,
            observation,
            assessment: None,
        }
    }

    /// Transition from unassessed to assessed, preserving the record on rejection.
    ///
    /// Pass/fail requires `Exited`; pass additionally requires exit code zero.
    /// Every verdict needs nonblank, bounded reasoning. These are necessary
    /// consistency checks, not a machine check of the reasoning's truth.
    pub fn assess(&mut self, verdict: Verdict, rationale: &str) -> Result<(), EvidenceError> {
        if self.assessment.is_some() {
            return Err(EvidenceError::AlreadyAssessed);
        }
        validate_text("rationale", rationale)?;
        if matches!(verdict, Verdict::Pass | Verdict::Fail) {
            match self.observation {
                ExecutionObservation::Exited { code: 0 } => {}
                ExecutionObservation::Exited { .. } if verdict == Verdict::Pass => {
                    return Err(EvidenceError::NonzeroExitCannotPass);
                }
                ExecutionObservation::Exited { .. } => {}
                _ => return Err(EvidenceError::ExecutionNotCompleted),
            }
        }
        self.assessment = Some(Assessment {
            verdict,
            rationale: rationale.to_owned(),
        });
        Ok(())
    }

    /// Read the task contract that this evidence concerns.
    pub fn contract(&self) -> &TaskContract {
        &self.contract
    }

    /// Read the source candidate supplied for this observation.
    pub fn candidate(&self) -> &CandidateId {
        &self.candidate
    }

    /// Read the preserved verification context.
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// Read the execution status independently of the assessment.
    pub fn observation(&self) -> ExecutionObservation {
        self.observation
    }

    /// Read the judgment, if an explicit assessment has been made.
    pub fn assessment(&self) -> Option<&Assessment> {
        self.assessment.as_ref()
    }
}

fn validate_text(field: &'static str, value: &str) -> Result<(), EvidenceError> {
    if value.len() > MAX_EVIDENCE_TEXT_BYTES {
        return Err(EvidenceError::TooLong(field));
    }
    if value.trim().is_empty() {
        return Err(EvidenceError::Blank(field));
    }
    if value.chars().any(|c| c.is_control() && c != '\n') {
        return Err(EvidenceError::InvalidCharacter(field));
    }
    Ok(())
}
