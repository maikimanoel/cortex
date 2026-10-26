//! Small, validated contracts shared by Cortex's engineering capabilities.
//!
//! These values describe work and source identity. They grant no execution authority.

use std::{error::Error, fmt};

/// Maximum task identifier size in UTF-8 bytes (identifiers are ASCII).
pub const MAX_TASK_ID_BYTES: usize = 128;
/// Maximum size of each objective or acceptance description in UTF-8 bytes.
pub const MAX_CONTRACT_TEXT_BYTES: usize = 4096;

/// A rejected contract input. Diagnostics identify fields without copying payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractError {
    /// A required value contains no non-whitespace text.
    Blank(&'static str),
    /// A value exceeds its documented byte limit.
    TooLong {
        /// The rejected field.
        field: &'static str,
        /// The maximum accepted byte length.
        max_bytes: usize,
    },
    /// An identifier or description contains a forbidden character.
    InvalidCharacter(&'static str),
    /// A revision is not a full lowercase hexadecimal Git object ID.
    InvalidGitRevision,
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blank(field) => write!(f, "{field} must contain text"),
            Self::TooLong { field, max_bytes } => {
                write!(f, "{field} exceeds {max_bytes} bytes")
            }
            Self::InvalidCharacter(field) => write!(f, "{field} contains a forbidden character"),
            Self::InvalidGitRevision => {
                f.write_str("candidate must be a full 40- or 64-character lowercase Git object ID")
            }
        }
    }
}

impl Error for ContractError {}

/// An operator-assigned task name, independent of a source revision.
///
/// Accepts 1..=128 ASCII bytes: an alphanumeric first character followed by
/// alphanumerics, dots, underscores, or hyphens. No normalization is performed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(String);

impl TaskId {
    /// Validate a task identifier before allocating its owned representation.
    pub fn new(value: &str) -> Result<Self, ContractError> {
        if value.len() > MAX_TASK_ID_BYTES {
            return Err(ContractError::TooLong {
                field: "task_id",
                max_bytes: MAX_TASK_ID_BYTES,
            });
        }
        if value.trim().is_empty() {
            return Err(ContractError::Blank("task_id"));
        }
        if !value.as_bytes()[0].is_ascii_alphanumeric()
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err(ContractError::InvalidCharacter("task_id"));
        }
        Ok(Self(value.to_owned()))
    }

    /// Read the validated identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A full Git object ID asserted by the caller to identify a source candidate.
///
/// Accepts Git SHA-1 (40 hex digits) and SHA-256 (64 hex digits) syntax. This does
/// not query Git, verify object existence/type, or attest to a clean working tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CandidateId(String);

impl CandidateId {
    /// Reject abbreviated, uppercase, or malformed source revisions.
    pub fn from_git_commit(revision: &str) -> Result<Self, ContractError> {
        if !matches!(revision.len(), 40 | 64)
            || !revision
                .bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
        {
            return Err(ContractError::InvalidGitRevision);
        }
        Ok(Self(revision.to_owned()))
    }

    /// Read the full revision supplied by the caller.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One task objective and its observable acceptance criterion.
///
/// Descriptions contain non-whitespace text, are at most 4096 UTF-8 bytes each,
/// and contain no control characters except line feed. Their original text is
/// preserved. Semantic adequacy remains the contract author's responsibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskContract {
    id: TaskId,
    objective: String,
    acceptance: String,
}

impl TaskContract {
    /// Construct an immutable task contract, rejecting invalid descriptions.
    pub fn new(id: TaskId, objective: &str, acceptance: &str) -> Result<Self, ContractError> {
        validate_text("objective", objective)?;
        validate_text("acceptance", acceptance)?;
        Ok(Self {
            id,
            objective: objective.to_owned(),
            acceptance: acceptance.to_owned(),
        })
    }

    /// Identify the task independently of any candidate.
    pub fn id(&self) -> &TaskId {
        &self.id
    }

    /// Read the desired engineering change.
    pub fn objective(&self) -> &str {
        &self.objective
    }

    /// Read the observable criterion against which evidence is assessed.
    pub fn acceptance(&self) -> &str {
        &self.acceptance
    }
}

fn validate_text(field: &'static str, value: &str) -> Result<(), ContractError> {
    if value.len() > MAX_CONTRACT_TEXT_BYTES {
        return Err(ContractError::TooLong {
            field,
            max_bytes: MAX_CONTRACT_TEXT_BYTES,
        });
    }
    if value.trim().is_empty() {
        return Err(ContractError::Blank(field));
    }
    if value.chars().any(|c| c.is_control() && c != '\n') {
        return Err(ContractError::InvalidCharacter(field));
    }
    Ok(())
}
