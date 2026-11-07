//! Explicit-source instruction preparation. Packages are inspectable data, not prompts with authority.
use super::*;
use crate::{binding::CandidateRequirement, temporal::SourceOrigin};

/// Declared explicit source input; caller assertions retain their source and omission limits.
#[derive(Clone, PartialEq, Eq)]
pub struct SourceInput {
    /// Exact source reference/version.
    pub source: SourceReference,
    /// Exact repository/worktree/commit requirement to which these bytes apply.
    pub candidate: CandidateRequirement,
    /// Exact bounded supplied text, not a reference to an unavailable transient file.
    pub text: String,
}
/// A contextual proposal, never automatic method adoption or blocking authority.
#[derive(Clone, PartialEq, Eq)]
pub struct MethodProposal {
    /// Exact existing method identity/version reference.
    pub method: SourceReference,
    /// Contextual applicability reason.
    pub applicability: String,
    /// Explicit qualification reference, or absent/unassessed.
    pub qualification: Option<SourceReference>,
    /// Protected exception text; absence must be explicit.
    pub exceptions: String,
}
/// Untrusted package request. Assembly fills protected fields from canonical state.
#[derive(Clone, PartialEq, Eq)]
pub struct PackageRequest {
    /// Immutable package identity.
    pub id: PackageId,
    /// Caller-assigned positive package revision.
    pub revision: ContractRevision,
    /// Target task.
    pub task: TaskId,
    /// Exact current baseline.
    pub baseline: ContractRevision,
    /// Exact expected task generation.
    pub generation: ContractRevision,
    /// Explicit source bytes, in their intended order.
    pub sources: Vec<SourceInput>,
    /// Explicit selected methods; unqualified is still a proposal.
    pub methods: Vec<MethodProposal>,
    /// Missing context and coverage, never silently omitted.
    pub omissions: String,
    /// Expected non-effectful deliverable structure.
    pub deliverable: String,
    /// Stop, retry and uncertainty instructions within metadata-only scope.
    pub stop_rules: String,
}
/// Immutable deterministic bytes, identified by exact contents as well as revision.
#[derive(Clone, PartialEq, Eq)]
pub struct InstructionPackage {
    request: PackageRequest,
    bytes: Vec<u8>,
}
impl InstructionPackage {
    /// Original explicit inputs.
    pub fn request(&self) -> &PackageRequest {
        &self.request
    }
    /// Complete canonical package. No hidden provider rewrite or automatic submission.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Reject structural alterations by comparing with current canonical reassembly.
    /// This establishes byte/contract correspondence, not semantic understanding.
    pub fn check(&self, state: &ProgramState, submitted: &[u8]) -> Result<(), ProgramError> {
        let current = assemble(state, self.request.clone())?;
        if current.bytes != self.bytes || submitted != self.bytes {
            return Err(ProgramError::Stale);
        }
        Ok(())
    }
}
fn field(out: &mut Vec<u8>, name: &str, value: &[u8], max: usize) -> Result<(), ProgramError> {
    // Length-delimited framing makes embedded instruction-like text unambiguous data.
    let header = format!("{name} {}\n", value.len());
    if out
        .len()
        .checked_add(header.len())
        .and_then(|n| n.checked_add(value.len() + 1))
        .is_none_or(|n| n > max)
    {
        return Err(ProgramError::Capacity);
    }
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(value);
    out.push(b'\n');
    Ok(())
}
fn reference(out: &mut Vec<u8>, value: &SourceReference, max: usize) -> Result<(), ProgramError> {
    match value.origin() {
        SourceOrigin::OperatorReported { author: operator } => {
            field(out, "operator-assertion", operator.as_bytes(), max)?
        }
        SourceOrigin::ObservationReference(id) => {
            field(out, "observation-producer", id.scope().as_bytes(), max)?;
            field(
                out,
                "observation-sequence",
                id.sequence().to_string().as_bytes(),
                max,
            )?;
        }
    }
    field(out, "source", value.source().as_bytes(), max)?;
    field(out, "source-version", value.version().as_bytes(), max)
}
/// Assemble protected clauses and explicit data in a fixed, versioned framing.
pub fn assemble(
    state: &ProgramState,
    request: PackageRequest,
) -> Result<InstructionPackage, ProgramError> {
    let d = state.definition();
    let scope = state.readiness().scope(&request.task)?;
    if request.baseline != d.revision() || request.generation != scope.generation() {
        return Err(ProgramError::Stale);
    }
    if request.sources.len() > d.limits().entries || request.methods.len() > d.limits().entries {
        return Err(ProgramError::Capacity);
    }
    let mut bytes = Vec::new();
    let max = d.limits().bytes;
    for (name, value) in [
        ("format", "cortex-instruction-v1"),
        ("assembler", "cortex-core-stage1-v1"),
        (
            "authority",
            "NONE: coordination metadata only; no execution, tools, adoption or provider submission",
        ),
        (
            "privacy",
            "explicit local operator metadata; no automatic capture/export/provider disclosure",
        ),
        ("program", d.id().as_str()),
        ("package", request.id.as_str()),
        ("mission", request.task.as_str()),
        ("objective", scope.context().contract().task().objective()),
        ("acceptance", scope.context().contract().task().acceptance()),
        (
            "candidate",
            scope.context().contract().candidate().commit().as_str(),
        ),
    ] {
        field(&mut bytes, name, value.as_bytes(), max)?;
    }
    for (name, value) in [
        ("baseline", request.baseline.get()),
        ("generation", request.generation.get()),
        ("package-revision", request.revision.get()),
        (
            "contract-revision",
            scope.context().contract().revision().get(),
        ),
        (
            "plan-revision",
            state.ordering_plan().map_or(0, |p| p.revision.get()),
        ),
    ] {
        field(&mut bytes, name, value.to_string().as_bytes(), max)?;
    }
    reference(&mut bytes, d.approval(), max)?;
    field(
        &mut bytes,
        "program-objective",
        d.outcome().objective().as_bytes(),
        max,
    )?;
    field(
        &mut bytes,
        "program-acceptance",
        d.outcome().acceptance().as_bytes(),
        max,
    )?;
    let repository = scope.context().contract().candidate().repository();
    for (name, path) in [
        ("repository-root", repository.root()),
        ("git-dir", repository.git_dir()),
        ("common-dir", repository.common_dir()),
    ] {
        field(&mut bytes, name, path.as_os_str().as_encoded_bytes(), max)?;
    }
    field(
        &mut bytes,
        "worktree-requirement",
        match scope.context().contract().candidate().worktree() {
            crate::binding::WorktreeRequirement::Clean => b"CLEAN",
            crate::binding::WorktreeRequirement::AllowChanges => b"ALLOW_CHANGES",
        },
        max,
    )?;

    field(
        &mut bytes,
        "graph",
        d.graph().reference().id().as_str().as_bytes(),
        max,
    )?;
    field(
        &mut bytes,
        "graph-revision",
        d.graph()
            .reference()
            .revision()
            .get()
            .to_string()
            .as_bytes(),
        max,
    )?;
    field(
        &mut bytes,
        "logical-input-byte-limit",
        d.limits().bytes.to_string().as_bytes(),
        max,
    )?;
    field(
        &mut bytes,
        "operational-budgets",
        b"UNAVAILABLE; no executor may consume this package as a grant",
        max,
    )?;
    for i in d.increments().iter().filter(|i| i.task == request.task) {
        field(&mut bytes, "increment", i.id.as_str().as_bytes(), max)?;
        for r in &i.claim_scopes {
            field(
                &mut bytes,
                "declared-responsibility",
                r.as_str().as_bytes(),
                max,
            )?;
        }
    }
    let task = state.readiness().task(&request.task)?;
    field(
        &mut bytes,
        "dependency-ready",
        task.dependency_ready().to_string().as_bytes(),
        max,
    )?;
    field(
        &mut bytes,
        "eligible",
        task.eligible().to_string().as_bytes(),
        max,
    )?;
    for blocker in task.blockers() {
        match blocker {
            TaskBlocker::Dependency(edge) => {
                field(&mut bytes, "blocked-edge", edge.as_str().as_bytes(), max)?
            }
            // The v1 assembler predates the public type rename. Its opaque blocker
            // bytes are a retained format token, independent of Debug/type names.
            _ => field(&mut bytes, "blocker", b"MissionBlocker([redacted])", max)?,
        }
    }

    for e in d
        .graph()
        .edges()
        .iter()
        .filter(|e| e.consumer() == &request.task)
    {
        field(&mut bytes, "edge", e.id().as_str().as_bytes(), max)?;
        field(
            &mut bytes,
            "producer",
            e.producer().as_str().as_bytes(),
            max,
        )?;
        field(
            &mut bytes,
            "artifact",
            e.requirement().artifact().id().as_str().as_bytes(),
            max,
        )?;
        field(
            &mut bytes,
            "artifact-revision",
            e.requirement()
                .artifact()
                .revision()
                .get()
                .to_string()
                .as_bytes(),
            max,
        )?;
        field(
            &mut bytes,
            "artifact-candidate",
            e.requirement().artifact().candidate().as_str().as_bytes(),
            max,
        )?;
        for predicate in e.requirement().predicates() {
            field(
                &mut bytes,
                "required-predicate",
                predicate.as_str().as_bytes(),
                max,
            )?;
        }
        match state.readiness().edge(e.id())?.decision() {
            EdgeDecision::Released { review, report } => {
                field(
                    &mut bytes,
                    "release-review",
                    review.as_str().as_bytes(),
                    max,
                )?;
                field(
                    &mut bytes,
                    "release-report",
                    report.as_str().as_bytes(),
                    max,
                )?;
            }
            EdgeDecision::Held(HoldReason::Invalidated(id)) => {
                field(&mut bytes, "invalidated-by", id.as_str().as_bytes(), max)?
            }
            _ => {}
        }

        field(
            &mut bytes,
            "dependency-level",
            match e.requirement().level() {
                ReadinessLevel::ContractStable => b"CONTRACT_STABLE",
                ReadinessLevel::ImplementationProvisional => b"IMPLEMENTATION_PROVISIONAL",
                ReadinessLevel::IntegratedAccepted => b"INTEGRATED_ACCEPTED",
            },
            max,
        )?;
        field(
            &mut bytes,
            "dependency-disposition",
            match state.readiness().edge(e.id())?.decision() {
                EdgeDecision::Released { .. } => b"RELEASED_NON_EFFECTFUL",
                _ => b"HELD",
            },
            max,
        )?;
    }
    for s in &request.sources {
        crate::validate_text("source_text", &s.text)?;
        if s.candidate != *scope.context().contract().candidate() {
            return Err(ProgramError::Stale);
        }
        reference(&mut bytes, &s.source, max)?;
        field(&mut bytes, "source-bytes", s.text.as_bytes(), max)?;
    }
    for m in &request.methods {
        crate::validate_text("method_applicability", &m.applicability)?;
        crate::validate_text("method_exceptions", &m.exceptions)?;
        reference(&mut bytes, &m.method, max)?;
        field(
            &mut bytes,
            "method-disposition",
            b"EXPLICIT_PROPOSAL_ONLY",
            max,
        )?;
        field(&mut bytes, "applicability", m.applicability.as_bytes(), max)?;
        field(&mut bytes, "exceptions", m.exceptions.as_bytes(), max)?;
        if let Some(q) = &m.qualification {
            reference(&mut bytes, q, max)?;
        } else {
            field(&mut bytes, "qualification", b"UNASSESSED", max)?;
        }
    }
    for (name, value) in [
        ("omissions", &request.omissions),
        ("deliverable", &request.deliverable),
        ("stop-rules", &request.stop_rules),
    ] {
        crate::validate_text(name, value)?;
        field(&mut bytes, name, value.as_bytes(), max)?;
    }
    Ok(InstructionPackage { request, bytes })
}
