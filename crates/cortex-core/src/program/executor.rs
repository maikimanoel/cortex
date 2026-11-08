//! Non-executing executor contracts and exact instruction-input preparation.
//!
//! Descriptors retain caller-reported observations at their declared assurance
//! boundary. Help output, a fixture, and a host qualification report are different
//! inputs. This module authenticates none of them and confers no operational
//! authority. There is no process, provider, filesystem, network or dispatch API.

use super::{ProgramError, ProgramState, context::InstructionPackage, scheduling::Resource};
use crate::temporal::SourceReference;
use std::{collections::BTreeSet, error::Error, fmt};

/// A capability that an instruction transport or host may be required to supply.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Control {
    /// Read the complete explicit input from standard input.
    StandardInput,
    /// Emit structured records under the selected transport contract.
    StructuredOutput,
    /// Enforce the declared allowed filesystem operations.
    Filesystem,
    /// Enforce the declared network restrictions.
    Network,
    /// Account for and control every applicable descendant process.
    Descendants,
    /// Revoke effective authority at the protected resource boundary.
    Revocation,
    /// Enforce the selected timeout and terminal outcome contract.
    Timeout,
    /// Cancel the applicable operation with qualified cleanup semantics.
    Cancellation,
    /// Preserve correspondence between a supplied candidate and its workspace.
    WorkspaceIdentity,
    /// Resume an exact existing session without silently duplicating its effects.
    SessionContinuation,
    /// Enforce one independently accounted resource dimension.
    Resource(Resource),
}

/// What was actually inspected according to the retained caller report.
/// These classes are deliberately not ordered: a synthetic fixture is not a
/// qualification of an installed executable, and a host report is not its help.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationKind {
    /// Read-only executable discovery/help or another advertised capability.
    Advertised,
    /// Behavior observed only in a controlled synthetic fixture.
    ControlledFixture,
    /// An explicitly referenced host qualification report supplied by the caller.
    /// This label does not authenticate the report or establish its adequacy.
    HostQualificationReport,
}

/// Explicit reported support. Missing and unexamined behavior is not a denial.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// The supplied report claims support at its declared observation boundary.
    Supported,
    /// The supplied report found this control unsupported.
    Unsupported,
    /// The report does not establish either support or a demonstrated denial.
    Unknown,
}

/// Immutable input claim; a validated descriptor prevents duplicate controls.
#[derive(Clone, PartialEq, Eq)]
pub struct CapabilityObservation {
    /// Exact capability being described.
    pub control: Control,
    /// Actual class of the supplied observation.
    pub kind: ObservationKind,
    /// Support within that class only.
    pub support: Support,
    /// Exact versioned original observation/report reference.
    pub basis: SourceReference,
}

/// The one transport shape for which input preparation is currently defined.
/// This enum describes bytes; it does not claim that any installed CLI supports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// Exact package bytes on standard input, JSON-lines output observed separately.
    StandardInputJsonLines,
}

/// Exact caller-selected descriptor identity and observation context.
#[derive(Clone, PartialEq, Eq)]
pub struct ExecutorIdentity {
    name: String,
    version: String,
    executable: SourceReference,
    host: SourceReference,
}
impl ExecutorIdentity {
    /// Validate an executor name/version and retain exact executable/host references.
    /// References must identify the observed binary and configuration, not a floating
    /// installation name. The caller supplies their truth and completeness.
    pub fn new(
        name: &str,
        version: &str,
        executable: SourceReference,
        host: SourceReference,
    ) -> Result<Self, ExecutorError> {
        crate::validate_identifier("executor_name", name).map_err(ProgramError::from)?;
        crate::validate_text("executor_version", version).map_err(ProgramError::from)?;
        Ok(Self {
            name: name.into(),
            version: version.into(),
            executable,
            host,
        })
    }
    /// Explicit executor family identifier.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Exact reported version; no universal version-comparison rule is inferred.
    pub fn version(&self) -> &str {
        &self.version
    }
    /// Exact observed executable reference.
    pub fn executable(&self) -> &SourceReference {
        &self.executable
    }
    /// Exact host/configuration observation reference.
    pub fn host(&self) -> &SourceReference {
        &self.host
    }
}

/// Immutable bounded descriptor, retaining the complete declared capability basis.
#[derive(Clone, PartialEq, Eq)]
pub struct ExecutorDescriptor {
    reference: SourceReference,
    identity: ExecutorIdentity,
    transport: Transport,
    observations: Vec<CapabilityObservation>,
}
impl ExecutorDescriptor {
    /// Validate finite, unambiguous capability input. `max_observations` is an
    /// explicit metadata bound, not an executor or host resource budget.
    pub fn new(
        reference: SourceReference,
        identity: ExecutorIdentity,
        transport: Transport,
        observations: Vec<CapabilityObservation>,
        max_observations: usize,
    ) -> Result<Self, ExecutorError> {
        if max_observations == 0 || observations.len() > max_observations {
            return Err(ExecutorError::Capacity);
        }
        let mut controls = BTreeSet::new();
        if observations
            .iter()
            .any(|o| !controls.insert(o.control.clone()))
        {
            return Err(ExecutorError::AmbiguousControl);
        }
        Ok(Self {
            reference,
            identity,
            transport,
            observations,
        })
    }
    /// Exact descriptor identity/version, separate from the executable version.
    pub fn reference(&self) -> &SourceReference {
        &self.reference
    }
    /// Exact executable and host context represented by these observations.
    pub fn identity(&self) -> &ExecutorIdentity {
        &self.identity
    }
    /// Declared transport shape; no transport is opened by inspection.
    pub fn transport(&self) -> Transport {
        self.transport
    }
    /// Complete original observations, including unsupported and unknown controls.
    pub fn observations(&self) -> &[CapabilityObservation] {
        &self.observations
    }
}

/// One explicit mandatory control and the evidence class required for this use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    /// Exact capability whose absence must hold the proposed use.
    pub control: Control,
    /// Required observation class, matched exactly rather than strengthened.
    pub kind: ObservationKind,
}

/// Immutable requirement profile; it selects no permissions or numeric budgets.
#[derive(Clone, PartialEq, Eq)]
pub struct Requirements {
    reference: SourceReference,
    required: Vec<Requirement>,
}
impl Requirements {
    /// Validate a nonempty, finite and unambiguous mandatory control profile.
    pub fn new(
        reference: SourceReference,
        required: Vec<Requirement>,
        max_controls: usize,
    ) -> Result<Self, ExecutorError> {
        if required.is_empty() || max_controls == 0 || required.len() > max_controls {
            return Err(ExecutorError::Capacity);
        }
        let mut controls = BTreeSet::new();
        if required.iter().any(|r| !controls.insert(r.control.clone())) {
            return Err(ExecutorError::AmbiguousControl);
        }
        Ok(Self {
            reference,
            required,
        })
    }
    /// Exact owner-selected requirement profile reference.
    pub fn reference(&self) -> &SourceReference {
        &self.reference
    }
    /// All required controls; no missing requirement is invented by negotiation.
    pub fn required(&self) -> &[Requirement] {
        &self.required
    }
}

/// Why a mandatory control remains unresolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldReason {
    /// No observation names this control.
    Missing,
    /// The supplied observation explicitly reports no support.
    Unsupported,
    /// The supplied observation leaves support unresolved.
    Unknown,
    /// The actual observation class differs from the required one.
    ObservationKind {
        /// Required class.
        required: ObservationKind,
        /// Actual supplied class.
        actual: ObservationKind,
    },
}

/// One deterministic causal negotiation result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlDecision {
    /// Required capability.
    pub control: Control,
    /// All causes, retaining observation-class and support failures separately.
    pub held: Vec<HoldReason>,
}

/// Inspect all mandatory controls in stable control order. An empty `held` vector
/// means only that supplied claims correspond to the declared profile. Even a
/// fully matching host report does not authenticate authority or qualify a host.
pub fn negotiate(
    descriptor: &ExecutorDescriptor,
    requirements: &Requirements,
) -> Vec<ControlDecision> {
    let mut required = requirements.required.iter().collect::<Vec<_>>();
    required.sort_by_key(|r| r.control.clone());
    required
        .into_iter()
        .map(|r| {
            let mut held = Vec::new();
            match descriptor
                .observations
                .iter()
                .find(|o| o.control.clone() == r.control.clone())
            {
                None => held.push(HoldReason::Missing),
                Some(o) => {
                    if o.kind != r.kind {
                        held.push(HoldReason::ObservationKind {
                            required: r.kind,
                            actual: o.kind,
                        });
                    }
                    match o.support {
                        Support::Supported => {}
                        Support::Unsupported => held.push(HoldReason::Unsupported),
                        Support::Unknown => held.push(HoldReason::Unknown),
                    }
                }
            }
            ControlDecision {
                control: r.control.clone(),
                held,
            }
        })
        .collect()
}

/// Inspectable transport input preserving the exact complete instruction package.
/// This type has no submission method and supplies no grant or operational token.
#[derive(Clone, PartialEq, Eq)]
pub struct PreparedInput {
    descriptor: ExecutorDescriptor,
    requirements: Requirements,
    package: InstructionPackage,
}
impl PreparedInput {
    /// Exact unchanged bytes proposed for standard input. No wrapper, truncation,
    /// prompt rewriting or instruction selection is performed.
    pub fn bytes(&self) -> &[u8] {
        self.package.bytes()
    }
    /// Original package identity, request and contract-preserving bytes.
    pub fn package(&self) -> &InstructionPackage {
        &self.package
    }
    /// Exact declared executor/host observations used during preparation.
    pub fn descriptor(&self) -> &ExecutorDescriptor {
        &self.descriptor
    }
    /// Exact requirement profile used during preparation.
    pub fn requirements(&self) -> &Requirements {
        &self.requirements
    }
    /// Recheck complete descriptor/profile equality and canonical package currency.
    /// Passing is byte correspondence only; it does not authorize submission.
    pub fn check(
        &self,
        state: &ProgramState,
        descriptor: &ExecutorDescriptor,
        requirements: &Requirements,
        proposed_bytes: &[u8],
    ) -> Result<(), ExecutorError> {
        if descriptor != &self.descriptor || requirements != &self.requirements {
            return Err(ExecutorError::ChangedContext);
        }
        self.package.check(state, proposed_bytes)?;
        Ok(())
    }
}

/// Prepare bounded, exact input after all selected capability claims match.
/// Standard-input and structured-output requirements must both be explicit.
/// The package still says no execution authority; Stage 2A does not upgrade it.
pub fn prepare(
    state: &ProgramState,
    package: &InstructionPackage,
    descriptor: &ExecutorDescriptor,
    requirements: &Requirements,
    max_bytes: usize,
) -> Result<PreparedInput, ExecutorError> {
    if max_bytes == 0
        || max_bytes > state.definition().limits().bytes
        || package.bytes().len() > max_bytes
    {
        return Err(ExecutorError::Capacity);
    }
    package.check(state, package.bytes())?;
    if [Control::StandardInput, Control::StructuredOutput]
        .iter()
        .any(|control| {
            !requirements
                .required
                .iter()
                .any(|r| r.control.clone() == *control)
        })
    {
        return Err(ExecutorError::MissingTransportContract);
    }
    let decisions = negotiate(descriptor, requirements);
    if decisions.iter().any(|d| !d.held.is_empty()) {
        return Err(ExecutorError::Held(decisions));
    }
    Ok(PreparedInput {
        descriptor: descriptor.clone(),
        requirements: requirements.clone(),
        package: package.clone(),
    })
}

/// Non-disclosing preparation error. No original prompt, path, host details,
/// source text, version or report payload is formatted in diagnostic output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutorError {
    /// Existing program/package contract refused the input.
    Program(ProgramError),
    /// A positive explicit metadata bound was absent or exceeded.
    Capacity,
    /// More than one observation/requirement names the same capability.
    AmbiguousControl,
    /// Mandatory transport requirements were omitted.
    MissingTransportContract,
    /// Descriptor or mandatory-control profile changed since preparation.
    ChangedContext,
    /// At least one mandatory capability claim is absent or insufficient.
    Held(Vec<ControlDecision>),
}
impl From<ProgramError> for ExecutorError {
    fn from(value: ProgramError) -> Self {
        Self::Program(value)
    }
}
impl fmt::Display for ExecutorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program(_) => f.write_str("executor input: program contract refused"),
            Self::Held(_) => f.write_str("executor input: required capabilities unresolved"),
            other => write!(f, "executor input: {other:?}"),
        }
    }
}
impl Error for ExecutorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Program(e) => Some(e),
            _ => None,
        }
    }
}
