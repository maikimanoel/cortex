//! G1 / CAP-WFL-004: pure, bounded readiness projection, not an execution system.
//!
//! Captures, coordinator roles and support references are **caller assertions**.
//! This module checks their declared correspondence, not authenticity or Evidence
//! verdicts. It performs no I/O, samples no clock, grants no permission and has no
//! dispatcher. Graphs are immutable; changing a binding requires a new projection.
//! Context generations invalidate old review use without rewriting its history.
//!
//! Limits account for logical input counts/bytes, not total RSS or allocator
//! overhead. Ordinary Rust allocation failure is not universally recoverable.

use crate::{
    CandidateId, ContractError, TaskId,
    binding::{
        CandidateBindingContract, CandidateObservation, ContractRevision, Head,
        ObservationConsistency, ObservationCoverage,
    },
    temporal::{SourceOrigin, SourceReference},
};
use std::{error::Error, fmt};

macro_rules! redacted {
    ($($ty:ty),+ $(,)?) => {$ (
        impl fmt::Debug for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($ty), "([redacted])"))
            }
        }
    )+};
}

macro_rules! key {
    ($name:ident, $field:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);
        impl $name {
            /// Validate the existing 1..=128-byte ASCII identifier syntax.
            pub fn new(value: &str) -> Result<Self, ReadinessError> {
                crate::validate_identifier($field, value)?;
                Ok(Self(value.to_owned()))
            }
            /// Explicitly inspect caller data; Debug remains redacted.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
        redacted!($name);
    };
}

key!(
    GraphId,
    "graph_id",
    "A graph-local identity, not an authority or global registry key."
);
key!(
    EdgeId,
    "edge_id",
    "A distinct consumer-edge identity scoped by a graph revision."
);
key!(
    ArtifactId,
    "artifact_id",
    "A declared output role, separate from mission/task identity."
);
key!(
    NotificationId,
    "notification_id",
    "An admitted-input identity scoped by a graph revision."
);
key!(
    PredicateId,
    "predicate_id",
    "An exact required predicate reference, not a qualified method."
);

/// Logical resource whose caller-selected bound was exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitKind {
    /// Retained missions.
    Missions,
    /// Retained edges.
    Edges,
    /// All admitted notifications, with no eviction.
    Notifications,
    /// Predicate requirements and source-reference occurrences.
    Supports,
    /// Retained input text, paths and opaque comparison bytes.
    Bytes,
}

/// Structural graph refusal, without caller identifiers or payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphIssue {
    /// A graph must have at least one mission.
    Empty,
    /// Two missions share a task identity.
    DuplicateMission,
    /// Two edges share an identity.
    DuplicateEdge,
    /// An output identity/revision has conflicting producer or candidate meaning.
    ArtifactAlias,
    /// Predicates are absent or duplicated.
    Predicates,
    /// A named mission is not present.
    MissingEndpoint,
    /// A mission depends on itself.
    SelfEdge,
    /// The directed graph contains a cycle.
    Cycle,
}

/// An incompatible review is refused atomically, never retained as a release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewRefusal {
    /// A producer-role assertion is not a coordinator review.
    NotCoordinator,
    /// The report is for a different output revision, candidate or kind.
    ArtifactContract,
    /// Levels require exact correspondence, not a universal quality ordering.
    ReadinessLevel,
    /// Support names a predicate not required by this edge, or repeats a predicate.
    Predicate,
}

/// Unsupported consequential operations have no implementation in G1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedOperation {
    /// Construct a new immutable graph instead.
    GraphEdit,
    /// Responsibility transfer is not dependency release.
    ResponsibilityHandoff,
    /// G1 has no effect recovery or rollback mechanism.
    ExternalEffect,
    /// No process/tool/mission dispatch is supplied.
    Dispatch,
}

/// Typed refusal. Any `apply` error leaves accepted state and identities unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadinessError {
    /// Existing field-level validation error, with its cause preserved.
    Contract(ContractError),
    /// Limits must be positive and representable as bounded collection sizes.
    InvalidLimits,
    /// A malformed graph.
    InvalidGraph(GraphIssue),
    /// Same task/revision was supplied with conflicting binding contents.
    ContractAlias,
    /// A supplied context/reference contradicts its frozen declaration.
    IncompatibleReference,
    /// A named mission, edge, or admitted artifact report is absent.
    MissingReference,
    /// The notification names another graph identity/revision.
    GraphMismatch,
    /// A context generation is no longer the applicable declared generation.
    StaleContext,
    /// A notification identity was reused for different data.
    IdentityConflict,
    /// A status transition would restart or reopen an existing attempt.
    InvalidTransition,
    /// Logical admission budget exceeded.
    Limit(LimitKind),
    /// A fallible reservation failed before publication.
    AllocationFailed,
    /// A structurally identified review cannot satisfy its edge.
    ReviewRejected(ReviewRefusal),
    /// An effect or unsupported graph operation was requested as data.
    Unsupported(UnsupportedOperation),
}
impl From<ContractError> for ReadinessError {
    fn from(value: ContractError) -> Self {
        Self::Contract(value)
    }
}
impl fmt::Display for ReadinessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(cause) => cause.fmt(f),
            _ => write!(f, "readiness input refused: {self:?}"),
        }
    }
}
impl Error for ReadinessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(cause) => Some(cause),
            _ => None,
        }
    }
}
impl ReadinessError {
    /// Inspect an atomic review refusal without changing the accepted edge view.
    pub fn rejected_review(&self) -> Option<EdgeDecision> {
        match self {
            Self::ReviewRejected(reason) => Some(EdgeDecision::Rejected(*reason)),
            _ => None,
        }
    }
}

/// Explicit logical bounds. Zero and values exceeding `isize::MAX` are invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphLimits {
    missions: usize,
    edges: usize,
    notifications: usize,
    supports: usize,
    bytes: usize,
}
impl GraphLimits {
    /// Choose all bounds explicitly; there is no implicit operational policy.
    pub fn new(
        missions: usize,
        edges: usize,
        notifications: usize,
        supports: usize,
        bytes: usize,
    ) -> Result<Self, ReadinessError> {
        if [missions, edges, notifications, supports, bytes]
            .iter()
            .any(|n| *n == 0 || *n > isize::MAX as usize)
        {
            return Err(ReadinessError::InvalidLimits);
        }
        Ok(Self {
            missions,
            edges,
            notifications,
            supports,
            bytes,
        })
    }
    /// Read the configured bound, not a host enforcement guarantee.
    pub fn get(self, kind: LimitKind) -> usize {
        match kind {
            LimitKind::Missions => self.missions,
            LimitKind::Edges => self.edges,
            LimitKind::Notifications => self.notifications,
            LimitKind::Supports => self.supports,
            LimitKind::Bytes => self.bytes,
        }
    }
}

/// Logical retained input usage; collection/allocation overhead is separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InputUsage {
    bytes: usize,
    supports: usize,
}
impl InputUsage {
    /// All retained text/path/comparison material occurrences, including repeats.
    pub fn bytes(self) -> usize {
        self.bytes
    }
    /// Graph predicate requirements plus admitted SourceReference occurrences.
    pub fn supports(self) -> usize {
        self.supports
    }
    fn bytes_add(&mut self, n: usize) -> Result<(), ReadinessError> {
        self.bytes = self
            .bytes
            .checked_add(n)
            .ok_or(ReadinessError::Limit(LimitKind::Bytes))?;
        Ok(())
    }
    fn support_add(&mut self, n: usize) -> Result<(), ReadinessError> {
        self.supports = self
            .supports
            .checked_add(n)
            .ok_or(ReadinessError::Limit(LimitKind::Supports))?;
        Ok(())
    }
    fn check(self, limits: GraphLimits) -> Result<(), ReadinessError> {
        if self.bytes > limits.bytes {
            return Err(ReadinessError::Limit(LimitKind::Bytes));
        }
        if self.supports > limits.supports {
            return Err(ReadinessError::Limit(LimitKind::Supports));
        }
        Ok(())
    }
}

/// A graph's identity and exact revision; numeric magnitude is not compatibility.
#[derive(Clone, PartialEq, Eq)]
pub struct GraphReference {
    id: GraphId,
    revision: ContractRevision,
}
impl GraphReference {
    /// Retain already validated identity/revision without external lookup.
    pub fn new(id: GraphId, revision: ContractRevision) -> Self {
        Self { id, revision }
    }
    /// Read the graph identity.
    pub fn id(&self) -> &GraphId {
        &self.id
    }
    /// Read the exact graph revision.
    pub fn revision(&self) -> ContractRevision {
        self.revision
    }
}

/// Frozen task/candidate contract and optional original supplied capture.
#[derive(Clone, PartialEq, Eq)]
pub struct MissionContext {
    contract: CandidateBindingContract,
    capture: Option<CandidateObservation>,
}
impl MissionContext {
    /// Reject contract aliasing or known candidate contradictions, without I/O.
    /// Missing/incomplete captures remain representable and block readiness.
    pub fn new(
        contract: CandidateBindingContract,
        capture: Option<CandidateObservation>,
    ) -> Result<Self, ReadinessError> {
        if let Some(c) = &capture {
            if c.contract() != &contract {
                return Err(
                    if c.contract().task().id() == contract.task().id()
                        && c.contract().revision() == contract.revision()
                    {
                        ReadinessError::ContractAlias
                    } else {
                        ReadinessError::IncompatibleReference
                    },
                );
            }
            if c.state()
                .repository()
                .is_some_and(|r| r != contract.candidate().repository())
            {
                return Err(ReadinessError::IncompatibleReference);
            }
            match c.state().head() {
                Head::Attached { commit, .. } | Head::Detached { commit }
                    if commit != contract.candidate().commit() =>
                {
                    return Err(ReadinessError::IncompatibleReference);
                }
                Head::Unborn { .. } => return Err(ReadinessError::IncompatibleReference),
                _ => {}
            }
        }
        Ok(Self { contract, capture })
    }
    /// Explicitly borrow the protected task/revision/candidate contract.
    pub fn contract(&self) -> &CandidateBindingContract {
        &self.contract
    }
    /// Explicitly borrow caller-supplied capture data, including its limits.
    pub fn capture(&self) -> Option<&CandidateObservation> {
        self.capture.as_ref()
    }
    fn available(&self) -> bool {
        self.capture.as_ref().is_some_and(|c| {
            c.state().repository().is_some()
                && matches!(
                    c.state().head(),
                    Head::Attached { .. } | Head::Detached { .. }
                )
                && c.state().entries().is_empty()
                && c.state().coverage() == ObservationCoverage::GitStatus
                && c.state().consistency() == ObservationConsistency::TwoMatchingSamples
                && c.basis().is_some()
        })
    }
}

/// Scoped frozen context plus a positive projection generation, not a freshness proof.
#[derive(Clone, PartialEq, Eq)]
pub struct MissionScope {
    context: MissionContext,
    generation: ContractRevision,
}
impl MissionScope {
    /// Declare an exact model generation. Actual reinspection is not performed.
    pub fn new(context: MissionContext, generation: ContractRevision) -> Self {
        Self {
            context,
            generation,
        }
    }
    /// Read the frozen complete context.
    pub fn context(&self) -> &MissionContext {
        &self.context
    }
    /// Read the projection generation, separate from task/graph revisions.
    pub fn generation(&self) -> ContractRevision {
        self.generation
    }
    /// The existing task ID identifies this mission.
    pub fn mission(&self) -> &TaskId {
        self.context.contract.task().id()
    }
}

/// Completeness asserted for declared dependency/non-impact coverage, not verified truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyCoverage {
    /// The caller explicitly declares its relevant dependency basis complete.
    DeclaredComplete,
    /// Absent coverage cannot substantiate current release use or non-impact.
    Unknown,
}
/// Intent being modeled, never an effect grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumerKind {
    /// Non-effectful reasoning/inspection preview.
    Preview,
    /// A hypothetical operational consumer; only integrated inputs can be modeled.
    Effectful,
}
/// One cohesive mission uses the existing task contract, not a second task model.
#[derive(Clone, PartialEq, Eq)]
pub struct Mission {
    context: MissionContext,
    kind: ConsumerKind,
    coverage: DependencyCoverage,
}
impl Mission {
    /// Retain the supplied context and explicit dependency coverage.
    pub fn new(context: MissionContext, kind: ConsumerKind, coverage: DependencyCoverage) -> Self {
        Self {
            context,
            kind,
            coverage,
        }
    }
    /// Read the existing task identity.
    pub fn id(&self) -> &TaskId {
        self.context.contract.task().id()
    }
    /// Borrow the original contract/capture.
    pub fn context(&self) -> &MissionContext {
        &self.context
    }
    /// Read the modeled consumer kind.
    pub fn kind(&self) -> ConsumerKind {
        self.kind
    }
    /// Inspect the supplied dependency-coverage assertion.
    pub fn coverage(&self) -> DependencyCoverage {
        self.coverage
    }
}

/// Exact output identity/version/candidate, independent of the producer's moving work.
#[derive(Clone, PartialEq, Eq)]
pub struct ArtifactReference {
    id: ArtifactId,
    revision: ContractRevision,
    candidate: CandidateId,
}
impl ArtifactReference {
    /// Retain syntactic candidate identity, without Git/object/content attestation.
    pub fn new(id: ArtifactId, revision: ContractRevision, candidate: CandidateId) -> Self {
        Self {
            id,
            revision,
            candidate,
        }
    }
    /// Read the declared output role.
    pub fn id(&self) -> &ArtifactId {
        &self.id
    }
    /// Read its exact output contract revision.
    pub fn revision(&self) -> ContractRevision {
        self.revision
    }
    /// Read the declared artifact candidate; may differ from producer capture.
    pub fn candidate(&self) -> &CandidateId {
        &self.candidate
    }
}
/// Artifact availability is distinct from whole-mission acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    /// A producer can continue after this output is released.
    Artifact,
    /// Requires a distinct acceptance report, accepted producer status and review.
    MissionAcceptance,
}
/// Exact dependency predicates, not a universal numeric quality ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadinessLevel {
    /// Reviewed contract only, suitable solely for separately authorized previews.
    ContractStable,
    /// Preview source with explicit risk/revalidation/cost references.
    ImplementationProvisional,
    /// Caller-asserted integration/acceptance; real Evidence bridging is deferred.
    IntegratedAccepted,
}
/// Required consumer-specific output and predicate set. Every edge is required.
#[derive(Clone, PartialEq, Eq)]
pub struct ArtifactRequirement {
    artifact: ArtifactReference,
    kind: DependencyKind,
    level: ReadinessLevel,
    predicates: Vec<PredicateId>,
}
impl ArtifactRequirement {
    /// Validate a nonempty unique predicate set; graph admission applies aggregate bounds.
    pub fn new(
        artifact: ArtifactReference,
        kind: DependencyKind,
        level: ReadinessLevel,
        mut predicates: Vec<PredicateId>,
    ) -> Result<Self, ReadinessError> {
        predicates.sort();
        if predicates.is_empty() || predicates.windows(2).any(|w| w[0] == w[1]) {
            return Err(ReadinessError::InvalidGraph(GraphIssue::Predicates));
        }
        Ok(Self {
            artifact,
            kind,
            level,
            predicates,
        })
    }
    /// Read the frozen output reference.
    pub fn artifact(&self) -> &ArtifactReference {
        &self.artifact
    }
    /// Inspect artifact versus mission-acceptance dependency.
    pub fn kind(&self) -> DependencyKind {
        self.kind
    }
    /// Read the exact required level.
    pub fn level(&self) -> ReadinessLevel {
        self.level
    }
    /// Read predicates in stable identifier order.
    pub fn predicates(&self) -> &[PredicateId] {
        &self.predicates
    }
}
/// A required producer-output to consumer edge; no file-overlap inference is used.
#[derive(Clone, PartialEq, Eq)]
pub struct DependencyEdge {
    id: EdgeId,
    producer: TaskId,
    consumer: TaskId,
    requirement: ArtifactRequirement,
}
impl DependencyEdge {
    /// Construct declared endpoints; graph admission resolves them and rejects cycles.
    pub fn new(
        id: EdgeId,
        producer: TaskId,
        consumer: TaskId,
        requirement: ArtifactRequirement,
    ) -> Self {
        Self {
            id,
            producer,
            consumer,
            requirement,
        }
    }
    /// Read this consumer edge's identity.
    pub fn id(&self) -> &EdgeId {
        &self.id
    }
    /// Read the producer mission ID.
    pub fn producer(&self) -> &TaskId {
        &self.producer
    }
    /// Read the consumer mission ID.
    pub fn consumer(&self) -> &TaskId {
        &self.consumer
    }
    /// Read the exact required output/predicates.
    pub fn requirement(&self) -> &ArtifactRequirement {
        &self.requirement
    }
}

/// Validated immutable DAG. Changing any contract requires construction anew.
#[derive(Clone, PartialEq, Eq)]
pub struct DependencyGraph {
    reference: GraphReference,
    missions: Vec<Mission>,
    edges: Vec<DependencyEdge>,
    order: Vec<usize>,
    limits: GraphLimits,
    usage: InputUsage,
}
impl DependencyGraph {
    /// Validate bounds, unambiguous references, operational-level constraints and DAG topology.
    pub fn new(
        reference: GraphReference,
        mut missions: Vec<Mission>,
        mut edges: Vec<DependencyEdge>,
        limits: GraphLimits,
    ) -> Result<Self, ReadinessError> {
        if missions.is_empty() {
            return Err(ReadinessError::InvalidGraph(GraphIssue::Empty));
        }
        if missions.len() > limits.missions {
            return Err(ReadinessError::Limit(LimitKind::Missions));
        }
        if edges.len() > limits.edges {
            return Err(ReadinessError::Limit(LimitKind::Edges));
        }
        missions.sort_by(|a, b| a.id().as_str().cmp(b.id().as_str()));
        edges.sort_by(|a, b| a.id.cmp(&b.id));
        if missions.windows(2).any(|w| w[0].id() == w[1].id()) {
            return Err(ReadinessError::InvalidGraph(GraphIssue::DuplicateMission));
        }
        if edges.windows(2).any(|w| w[0].id == w[1].id) {
            return Err(ReadinessError::InvalidGraph(GraphIssue::DuplicateEdge));
        }
        let mut graph = Self {
            reference,
            missions,
            edges,
            order: vec![],
            limits,
            usage: InputUsage::default(),
        };
        let mut usage = InputUsage::default();
        usage.bytes_add(graph.reference.id.0.len())?;
        for mission in &graph.missions {
            count_context(&mut usage, &mission.context, limits)?;
            usage.check(limits)?;
        }
        for (i, edge) in graph.edges.iter().enumerate() {
            let p = graph
                .mission_index(&edge.producer)
                .map_err(|_| ReadinessError::InvalidGraph(GraphIssue::MissingEndpoint))?;
            let c = graph
                .mission_index(&edge.consumer)
                .map_err(|_| ReadinessError::InvalidGraph(GraphIssue::MissingEndpoint))?;
            if p == c {
                return Err(ReadinessError::InvalidGraph(GraphIssue::SelfEdge));
            }
            if graph.missions[c].kind == ConsumerKind::Effectful
                && edge.requirement.level != ReadinessLevel::IntegratedAccepted
            {
                return Err(ReadinessError::Unsupported(UnsupportedOperation::Dispatch));
            }
            for other in &graph.edges[..i] {
                if edge.requirement.artifact.id == other.requirement.artifact.id
                    && (edge.producer != other.producer
                        || (edge.requirement.artifact.revision
                            == other.requirement.artifact.revision
                            && (edge.requirement.artifact != other.requirement.artifact
                                || edge.requirement.kind != other.requirement.kind)))
                {
                    return Err(ReadinessError::InvalidGraph(GraphIssue::ArtifactAlias));
                }
            }
            usage.bytes_add(edge.id.0.len())?;
            usage.bytes_add(edge.producer.as_str().len())?;
            usage.bytes_add(edge.consumer.as_str().len())?;
            count_artifact(&mut usage, &edge.requirement.artifact)?;
            usage.support_add(edge.requirement.predicates.len())?;
            usage.check(limits)?;
            for predicate in &edge.requirement.predicates {
                usage.bytes_add(predicate.0.len())?;
                usage.check(limits)?;
            }
        }
        usage.check(limits)?;
        // Iterative Kahn traversal avoids recursion proportional to caller input depth.
        let mut degree = reserved(graph.missions.len())?;
        degree.resize(graph.missions.len(), 0usize);
        for edge in &graph.edges {
            let c = graph.mission_index(&edge.consumer)?;
            degree[c] = degree[c]
                .checked_add(1)
                .ok_or(ReadinessError::Limit(LimitKind::Edges))?;
        }
        let mut order = reserved(graph.missions.len())?;
        let mut visited = reserved(graph.missions.len())?;
        visited.resize(graph.missions.len(), false);
        for _ in 0..graph.missions.len() {
            let Some(p) = (0..degree.len()).find(|i| !visited[*i] && degree[*i] == 0) else {
                return Err(ReadinessError::InvalidGraph(GraphIssue::Cycle));
            };
            visited[p] = true;
            order.push(p);
            for edge in &graph.edges {
                if edge.producer == *graph.missions[p].id() {
                    degree[graph.mission_index(&edge.consumer)?] -= 1;
                }
            }
        }
        graph.order = order;
        graph.usage = usage;
        Ok(graph)
    }
    /// Borrow the immutable graph identity/revision.
    pub fn reference(&self) -> &GraphReference {
        &self.reference
    }
    /// Inspect missions in stable task-ID order.
    pub fn missions(&self) -> &[Mission] {
        &self.missions
    }
    /// Inspect required edges in stable edge-ID order.
    pub fn edges(&self) -> &[DependencyEdge] {
        &self.edges
    }
    /// Read configured logical bounds.
    pub fn limits(&self) -> GraphLimits {
        self.limits
    }
    /// Read initial retained graph input usage.
    pub fn input_usage(&self) -> InputUsage {
        self.usage
    }
    fn mission_index(&self, id: &TaskId) -> Result<usize, ReadinessError> {
        self.missions
            .binary_search_by(|m| m.id().as_str().cmp(id.as_str()))
            .map_err(|_| ReadinessError::MissingReference)
    }
    fn edge_index(&self, id: &EdgeId) -> Result<usize, ReadinessError> {
        self.edges
            .binary_search_by(|e| e.id.cmp(id))
            .map_err(|_| ReadinessError::MissingReference)
    }
}

/// A producer's immutable report; it alone cannot release any consumer.
#[derive(Clone, PartialEq, Eq)]
pub struct ArtifactReport {
    producer: MissionScope,
    artifact: ArtifactReference,
    kind: DependencyKind,
    level: ReadinessLevel,
    source: SourceReference,
}
impl ArtifactReport {
    /// Record exact supplied producer/output correspondence and reported level.
    pub fn new(
        producer: MissionScope,
        artifact: ArtifactReference,
        kind: DependencyKind,
        level: ReadinessLevel,
        source: SourceReference,
    ) -> Self {
        Self {
            producer,
            artifact,
            kind,
            level,
            source,
        }
    }
    /// Read the original producing context/generation.
    pub fn producer(&self) -> &MissionScope {
        &self.producer
    }
    /// Read the independent output candidate/revision.
    pub fn artifact(&self) -> &ArtifactReference {
        &self.artifact
    }
    /// Read its asserted semantic kind.
    pub fn kind(&self) -> DependencyKind {
        self.kind
    }
    /// Read its reported level; not an Evidence verdict.
    pub fn level(&self) -> ReadinessLevel {
        self.level
    }
    /// Explicitly borrow reported provenance, not authenticated capture.
    pub fn source(&self) -> &SourceReference {
        &self.source
    }
}
/// Three explicit references for one required predicate; no `qualified` flag.
#[derive(Clone, PartialEq, Eq)]
pub struct PredicateSupport {
    predicate: PredicateId,
    observation: SourceReference,
    assessment: SourceReference,
    context: SourceReference,
}
impl PredicateSupport {
    /// Record caller-asserted observation/assessment/context references.
    pub fn new(
        predicate: PredicateId,
        observation: SourceReference,
        assessment: SourceReference,
        context: SourceReference,
    ) -> Self {
        Self {
            predicate,
            observation,
            assessment,
            context,
        }
    }
    /// Read the exact declared predicate role.
    pub fn predicate(&self) -> &PredicateId {
        &self.predicate
    }
    /// Inspect references in observation, assessment, context order.
    pub fn references(&self) -> [&SourceReference; 3] {
        [&self.observation, &self.assessment, &self.context]
    }
}
/// Explicit non-effectful provisional-use policy references, not operational permission.
#[derive(Clone, PartialEq, Eq)]
pub struct ProvisionalUse {
    risk: SourceReference,
    revalidation: SourceReference,
    cost_owner: SourceReference,
}
impl ProvisionalUse {
    /// Require all three declared policy references for a provisional preview.
    pub fn new(
        risk: SourceReference,
        revalidation: SourceReference,
        cost_owner: SourceReference,
    ) -> Self {
        Self {
            risk,
            revalidation,
            cost_owner,
        }
    }
    /// Inspect risk, revalidation and cost-owner assertions.
    pub fn references(&self) -> [&SourceReference; 3] {
        [&self.risk, &self.revalidation, &self.cost_owner]
    }
}
/// Supplied reviewer/approval identities, not authenticated principals or grants.
#[derive(Clone, PartialEq, Eq)]
pub struct ReviewAttestation {
    reviewer: SourceReference,
    approval: SourceReference,
    provisional: Option<ProvisionalUse>,
}
impl ReviewAttestation {
    /// Retain separate declared reviewer and approval references.
    pub fn new(
        reviewer: SourceReference,
        approval: SourceReference,
        provisional: Option<ProvisionalUse>,
    ) -> Self {
        Self {
            reviewer,
            approval,
            provisional,
        }
    }
    /// Explicitly inspect reviewer and approval assertions.
    pub fn references(&self) -> [&SourceReference; 2] {
        [&self.reviewer, &self.approval]
    }
    /// Inspect any bounded preview provisional-use policy.
    pub fn provisional(&self) -> Option<&ProvisionalUse> {
        self.provisional.as_ref()
    }
}
/// Role is supplied data; even Coordinator is not an authenticated authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewRole {
    /// Explicit coordinator-role assertion permitted only by the G1 preview policy.
    Coordinator,
    /// Producer messages cannot substitute for the coordinator review.
    Producer,
}
/// A proposal for one edge/report and both exact current model contexts.
#[derive(Clone, PartialEq, Eq)]
pub struct EdgeReview {
    edge: EdgeId,
    report: NotificationId,
    producer: MissionScope,
    consumer: MissionScope,
    role: ReviewRole,
    attestation: ReviewAttestation,
    supports: Vec<PredicateSupport>,
}
impl EdgeReview {
    /// Supply a per-edge review. Admission validates all references and required support.
    pub fn new(
        edge: EdgeId,
        report: NotificationId,
        producer: MissionScope,
        consumer: MissionScope,
        role: ReviewRole,
        attestation: ReviewAttestation,
        supports: Vec<PredicateSupport>,
    ) -> Self {
        Self {
            edge,
            report,
            producer,
            consumer,
            role,
            attestation,
            supports,
        }
    }
    /// Read the selected consumer edge.
    pub fn edge(&self) -> &EdgeId {
        &self.edge
    }
    /// Identify the exact admitted artifact report.
    pub fn report(&self) -> &NotificationId {
        &self.report
    }
    /// Borrow producer and consumer scopes, respectively.
    pub fn scopes(&self) -> [&MissionScope; 2] {
        [&self.producer, &self.consumer]
    }
    /// Inspect the unauthenticated role assertion.
    pub fn role(&self) -> ReviewRole {
        self.role
    }
    /// Borrow declared reviewer/approval/provisional policy references.
    pub fn attestation(&self) -> &ReviewAttestation {
        &self.attestation
    }
    /// Inspect predicate-support assertions, never Evidence assessments.
    pub fn supports(&self) -> &[PredicateSupport] {
        &self.supports
    }
}

/// In-memory attempt status. Updates report data; they execute nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissionStatus {
    /// Not started; only this status can be start-eligible.
    Pending,
    /// Already active; cannot be started again.
    Active,
    /// Caller reports acceptance; a whole-mission edge still needs report/review.
    Accepted,
    /// Terminal cancellation, not rollback of existing artifacts.
    Cancelled,
}
/// Bounded notification input families. Unsupported effects are explicitly refused.
#[derive(Clone, PartialEq, Eq)]
pub enum Event {
    /// Information only; bounded text cannot release or grant anything.
    Signal {
        /// Named mission.
        mission: TaskId,
        /// Private caller message.
        message: String,
    },
    /// An exact report, not a release.
    Artifact(ArtifactReport),
    /// Coordinator-role review data, boxed to avoid inflating every notification.
    /// This is not a real approval operation.
    Review(Box<EdgeReview>),
    /// Invalidate causal closure; unknown coverage conservatively blocks all missions.
    Invalidate {
        /// Current scoped roots of the declared change.
        roots: Vec<MissionScope>,
        /// Caller-described cause/version.
        cause: SourceReference,
        /// Declared completeness of affected dependency coverage.
        coverage: DependencyCoverage,
    },
    /// Replace an explicit precondition assertion in the named current context.
    Preconditions {
        /// Applicable context.
        scope: MissionScope,
        /// Claimed readiness of non-dependency conditions.
        ready: bool,
        /// Source of the assertion.
        source: SourceReference,
    },
    /// Synthetic free slots, independently bounded by the configured mission limit.
    Capacity {
        /// Zero means unavailable, distinct from no observation yet.
        available: usize,
        /// Source of this synthetic assertion.
        source: SourceReference,
    },
    /// Update an existing attempt without reopening active/terminal work.
    Status {
        /// Applicable context.
        scope: MissionScope,
        /// New asserted status.
        status: MissionStatus,
        /// Source of the assertion.
        source: SourceReference,
    },
    /// No such effect/operation is implemented by G1.
    Unsupported(UnsupportedOperation),
}
/// An immutable input identity/revision. Construction validates new free-form text.
#[derive(Clone, PartialEq, Eq)]
pub struct Notification {
    id: NotificationId,
    graph: GraphReference,
    event: Event,
}
impl Notification {
    /// Retain an input, validating Signal text using the existing 4096-byte rules.
    pub fn new(
        id: NotificationId,
        graph: GraphReference,
        event: Event,
    ) -> Result<Self, ReadinessError> {
        if let Event::Signal { message, .. } = &event {
            crate::validate_text("signal", message)?;
        }
        Ok(Self { id, graph, event })
    }
    /// Read the scoped notification identity.
    pub fn id(&self) -> &NotificationId {
        &self.id
    }
    /// Read the exact graph reference.
    pub fn graph(&self) -> &GraphReference {
        &self.graph
    }
    /// Explicitly inspect retained private input data.
    pub fn event(&self) -> &Event {
        &self.event
    }
}

/// No view/result from this API can carry operational authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionAuthority {
    /// G1 has no authority, reservation, dispatch, integration or deployment mechanism.
    Unavailable,
}
/// Missing support is held, distinct from invalid input and Evidence verdicts.
#[derive(Clone, PartialEq, Eq)]
pub enum HoldReason {
    /// No applicable artifact report/review yet.
    ReviewRequired,
    /// Complete required predicate-support references were not supplied.
    MissingSupport,
    /// A frozen context lacks sufficient declared capture information.
    MissingContext,
    /// Relevant dependency/non-impact coverage is uncertain.
    UnknownCoverage,
    /// Preview provisional risk/revalidation/cost references are missing.
    ProvisionalPolicy,
    /// A whole-mission acceptance edge cannot be released while producer is unfinished.
    ProducerNotAccepted,
    /// The first causal invalidation notification; historical basis remains retained.
    Invalidated(NotificationId),
}
/// Per-edge projection outcome. Rejected appears on atomic review errors, not retained mutation.
#[derive(Clone, PartialEq, Eq)]
pub enum EdgeDecision {
    /// A particular admitted review/report satisfies this edge's model predicates.
    Released {
        /// Scoped coordinator review notification.
        review: NotificationId,
        /// Original producer report notification.
        report: NotificationId,
    },
    /// Missing support prevents current use.
    Held(HoldReason),
    /// Incompatible proposals are returned via ReadinessError::rejected_review.
    Rejected(ReviewRefusal),
}
/// Historical release references into retained notifications, including first invalidation.
#[derive(Clone, PartialEq, Eq)]
pub struct ReleaseRecord {
    review: NotificationId,
    report: NotificationId,
    invalidated_by: Option<NotificationId>,
}
impl ReleaseRecord {
    /// Exact original coordinator proposal and its full scopes/support.
    pub fn review(&self) -> &NotificationId {
        &self.review
    }
    /// Exact original artifact/candidate report.
    pub fn report(&self) -> &NotificationId {
        &self.report
    }
    /// First invalidating event/cause, never cleared or relabeled current.
    pub fn invalidated_by(&self) -> Option<&NotificationId> {
        self.invalidated_by.as_ref()
    }
}
/// Derived current edge use and retained historical release identities.
#[derive(Clone, PartialEq, Eq)]
pub struct EdgeState {
    decision: EdgeDecision,
    releases: Vec<ReleaseRecord>,
}
impl EdgeState {
    /// Inspect current readiness; this is not Evidence or permission.
    pub fn decision(&self) -> &EdgeDecision {
        &self.decision
    }
    /// Inspect all accepted releases with their historical invalidation basis.
    pub fn releases(&self) -> &[ReleaseRecord] {
        &self.releases
    }
    /// No operational authority follows even from Released.
    pub fn authority(&self) -> ExecutionAuthority {
        ExecutionAuthority::Unavailable
    }
}
/// Independent mission blocking causes in deterministic order.
#[derive(Clone, PartialEq, Eq)]
pub enum MissionBlocker {
    /// Active or terminal attempts cannot start again.
    NotPending,
    /// No explicit precondition observation in the current generation.
    PreconditionsUnknown,
    /// Supplied preconditions are not satisfied.
    PreconditionsUnmet,
    /// Capture correspondence/coverage is incomplete.
    MissingContext,
    /// Dependency coverage is not established.
    UnknownCoverage,
    /// A required incoming edge is not released.
    Dependency(EdgeId),
    /// No free-slot observation yet.
    CapacityUnknown,
    /// Explicitly no free synthetic slots.
    NoCapacity,
}
/// Advisory mission decision, separate from dependency readiness and actual execution.
#[derive(Clone, PartialEq, Eq)]
pub struct MissionState {
    generation: ContractRevision,
    status: MissionStatus,
    preconditions: Option<bool>,
    coverage_known: bool,
    dependency_ready: bool,
    blockers: Vec<MissionBlocker>,
}
impl MissionState {
    /// Eligibility is not a reservation or an authorization.
    pub fn eligible(&self) -> bool {
        self.blockers.is_empty()
    }
    /// All declared required dependencies/context are satisfied, independently of capacity/status.
    pub fn dependency_ready(&self) -> bool {
        self.dependency_ready
    }
    /// Current model context generation, not a task revision or freshness proof.
    pub fn generation(&self) -> ContractRevision {
        self.generation
    }
    /// Inspect the retained attempt status.
    pub fn status(&self) -> MissionStatus {
        self.status
    }
    /// Inspect stable, explicit reasons preventing eligibility.
    pub fn blockers(&self) -> &[MissionBlocker] {
        &self.blockers
    }
    /// Eligibility never grants operational authority.
    pub fn authority(&self) -> ExecutionAuthority {
        ExecutionAuthority::Unavailable
    }
}
/// A deterministic event-to-state transition report, not a dispatch request.
#[derive(Clone, PartialEq, Eq)]
pub struct TransitionReport {
    graph: GraphReference,
    notification: NotificationId,
    sequence: usize,
    duplicate: bool,
    changed_edges: Vec<EdgeId>,
    changed_missions: Vec<TaskId>,
}
impl TransitionReport {
    /// Exact immutable graph basis.
    pub fn graph(&self) -> &GraphReference {
        &self.graph
    }
    /// The notification causing this transition (or duplicate).
    pub fn notification(&self) -> &NotificationId {
        &self.notification
    }
    /// Retained accepted-input count; duplicate delivery does not increment it.
    pub fn sequence(&self) -> usize {
        self.sequence
    }
    /// Identical redelivery changes no accepted state.
    pub fn duplicate(&self) -> bool {
        self.duplicate
    }
    /// Edge changes in stable ID order, including historical invalidation changes.
    pub fn changed_edges(&self) -> &[EdgeId] {
        &self.changed_edges
    }
    /// Mission changes in stable ID order; inspect through projection.mission.
    pub fn changed_missions(&self) -> &[TaskId] {
        &self.changed_missions
    }
    /// Every result explicitly leaves operational authority unavailable.
    pub fn authority(&self) -> ExecutionAuthority {
        ExecutionAuthority::Unavailable
    }
}

#[derive(Clone, PartialEq, Eq)]
struct ProjectionState {
    missions: Vec<MissionState>,
    edges: Vec<EdgeState>,
    capacity: Option<usize>,
}

/// Single-owner in-memory reducer; no scheduler, event listener or external effects.
#[derive(Clone, PartialEq, Eq)]
pub struct ReadinessProjection {
    graph: DependencyGraph,
    state: ProjectionState,
    history: Vec<Notification>,
    usage: InputUsage,
}
impl ReadinessProjection {
    /// Allocate bounded initial derived state; roots still need explicit preconditions/capacity.
    pub fn new(graph: DependencyGraph) -> Result<Self, ReadinessError> {
        let mut missions = reserved(graph.missions.len())?;
        for m in &graph.missions {
            missions.push(MissionState {
                generation: ContractRevision::new(1).expect("positive constant"),
                status: MissionStatus::Pending,
                preconditions: None,
                coverage_known: m.coverage == DependencyCoverage::DeclaredComplete,
                dependency_ready: false,
                blockers: vec![],
            });
        }
        let mut edges = reserved(graph.edges.len())?;
        for _ in &graph.edges {
            edges.push(EdgeState {
                decision: EdgeDecision::Held(HoldReason::ReviewRequired),
                releases: vec![],
            });
        }
        let usage = graph.usage;
        let mut state = ProjectionState {
            missions,
            edges,
            capacity: None,
        };
        recompute(&graph, &mut state)?;
        Ok(Self {
            graph,
            state,
            history: vec![],
            usage,
        })
    }
    /// Inspect the immutable source of truth for this projection's contracts.
    pub fn graph(&self) -> &DependencyGraph {
        &self.graph
    }
    /// Borrow an edge decision/history; unknown IDs are typed refusal.
    pub fn edge(&self, id: &EdgeId) -> Result<&EdgeState, ReadinessError> {
        Ok(&self.state.edges[self.graph.edge_index(id)?])
    }
    /// Borrow a mission decision, never an execution grant.
    pub fn mission(&self, id: &TaskId) -> Result<&MissionState, ReadinessError> {
        Ok(&self.state.missions[self.graph.mission_index(id)?])
    }
    /// Obtain the exact current model scope, retaining original contract/capture.
    pub fn scope(&self, id: &TaskId) -> Result<MissionScope, ReadinessError> {
        let i = self.graph.mission_index(id)?;
        Ok(MissionScope::new(
            self.graph.missions[i].context.clone(),
            self.state.missions[i].generation,
        ))
    }
    /// Explicitly borrow admitted private inputs; no durable storage or authenticity implied.
    pub fn notifications(&self) -> &[Notification] {
        &self.history
    }
    /// Inspect an original report/review/invalidation by its admitted identity.
    pub fn notification(&self, id: &NotificationId) -> Result<&Notification, ReadinessError> {
        self.history
            .iter()
            .find(|n| n.id == *id)
            .ok_or(ReadinessError::MissingReference)
    }
    /// Logical graph plus admitted-input usage, not total process memory.
    pub fn input_usage(&self) -> InputUsage {
        self.usage
    }
    /// No operational permission is represented by this projection.
    pub fn authority(&self) -> ExecutionAuthority {
        ExecutionAuthority::Unavailable
    }

    /// Validate, compute a derived-state draft, and publish only after all refusals/reservations.
    ///
    /// Identical duplicates are no-ops even when history is full. Any Err leaves
    /// graph, derived state, history and identity/budget accounting unchanged.
    /// Allocation panics/abort are outside that Result guarantee.
    pub fn apply(
        &mut self,
        notification: Notification,
    ) -> Result<TransitionReport, ReadinessError> {
        if notification.graph != self.graph.reference {
            return Err(ReadinessError::GraphMismatch);
        }
        // Bound a single supplied payload before comparing it with retained history.
        // Accepted duplicates must still succeed when the aggregate history is full.
        let mut input = InputUsage::default();
        count_notification(&mut input, &notification, self.graph.limits)?;
        if let Some(old) = self.history.iter().find(|n| n.id == notification.id) {
            if old != &notification {
                return Err(ReadinessError::IdentityConflict);
            }
            return Ok(TransitionReport {
                graph: self.graph.reference.clone(),
                notification: notification.id,
                sequence: self.history.len(),
                duplicate: true,
                changed_edges: vec![],
                changed_missions: vec![],
            });
        }
        if self.history.len() >= self.graph.limits.notifications {
            return Err(ReadinessError::Limit(LimitKind::Notifications));
        }
        let mut usage = self.usage;
        usage.bytes_add(input.bytes)?;
        usage.support_add(input.supports)?;
        usage.check(self.graph.limits)?;
        // Only derived state is drafted; immutable graph/history payloads are not cloned.
        let mut draft = self.state.clone();
        self.reduce(&mut draft, &notification)?;
        recompute(&self.graph, &mut draft)?;
        let mut changed_edges = reserved(self.graph.edges.len())?;
        let mut changed_missions = reserved(self.graph.missions.len())?;
        for (i, edge) in self.graph.edges.iter().enumerate() {
            if self.state.edges[i] != draft.edges[i] {
                changed_edges.push(edge.id.clone());
            }
        }
        for (i, mission) in self.graph.missions.iter().enumerate() {
            if self.state.missions[i] != draft.missions[i] {
                changed_missions.push(mission.id().clone());
            }
        }
        self.history
            .try_reserve(1)
            .map_err(|_| ReadinessError::AllocationFailed)?;
        let report = TransitionReport {
            graph: self.graph.reference.clone(),
            notification: notification.id.clone(),
            sequence: self.history.len() + 1,
            duplicate: false,
            changed_edges,
            changed_missions,
        };
        self.history.push(notification);
        self.state = draft;
        self.usage = usage;
        Ok(report)
    }

    fn resolve_scope(
        &self,
        state: &ProjectionState,
        scope: &MissionScope,
    ) -> Result<usize, ReadinessError> {
        let i = self.graph.mission_index(scope.mission())?;
        if scope.context != self.graph.missions[i].context {
            return Err(ReadinessError::IncompatibleReference);
        }
        if scope.generation != state.missions[i].generation {
            return Err(ReadinessError::StaleContext);
        }
        Ok(i)
    }
    fn reduce(&self, state: &mut ProjectionState, n: &Notification) -> Result<(), ReadinessError> {
        match &n.event {
            Event::Signal { mission, .. } => {
                self.graph.mission_index(mission)?;
            }
            Event::Artifact(report) => {
                self.resolve_scope(state, &report.producer)?;
                if !self.graph.edges.iter().any(|e| {
                    e.producer == *report.producer.mission()
                        && e.requirement.artifact == report.artifact
                        && e.requirement.kind == report.kind
                }) {
                    return Err(ReadinessError::IncompatibleReference);
                }
            }
            Event::Review(review) => self.review(state, n, review)?,
            Event::Invalidate {
                roots, coverage, ..
            } => {
                if roots.is_empty() || roots.len() > self.graph.missions.len() {
                    return Err(ReadinessError::IncompatibleReference);
                }
                let mut affected = reserved(self.graph.missions.len())?;
                affected.resize(self.graph.missions.len(), false);
                for scope in roots {
                    let i = self.resolve_scope(state, scope)?;
                    if affected[i] {
                        return Err(ReadinessError::IdentityConflict);
                    }
                    affected[i] = true;
                }
                if *coverage == DependencyCoverage::Unknown {
                    affected.fill(true);
                } else {
                    for &p in &self.graph.order {
                        if affected[p] {
                            for edge in &self.graph.edges {
                                if edge.producer == *self.graph.missions[p].id() {
                                    affected[self.graph.mission_index(&edge.consumer)?] = true;
                                }
                            }
                        }
                    }
                }
                for (i, m) in state.missions.iter_mut().enumerate() {
                    if affected[i] {
                        let next = m
                            .generation
                            .get()
                            .checked_add(1)
                            .ok_or(ReadinessError::Limit(LimitKind::Notifications))?;
                        m.generation =
                            ContractRevision::new(next).expect("checked positive generation");
                        m.preconditions = None;
                        if *coverage == DependencyCoverage::Unknown {
                            m.coverage_known = false;
                        }
                    }
                }
                for (i, edge) in self.graph.edges.iter().enumerate() {
                    if affected[self.graph.mission_index(&edge.producer)?]
                        || affected[self.graph.mission_index(&edge.consumer)?]
                    {
                        for release in &mut state.edges[i].releases {
                            if release.invalidated_by.is_none() {
                                release.invalidated_by = Some(n.id.clone());
                            }
                        }
                        state.edges[i].decision =
                            EdgeDecision::Held(HoldReason::Invalidated(n.id.clone()));
                    }
                }
            }
            Event::Preconditions { scope, ready, .. } => {
                let i = self.resolve_scope(state, scope)?;
                state.missions[i].preconditions = Some(*ready);
            }
            Event::Capacity { available, .. } => {
                if *available > self.graph.limits.missions {
                    return Err(ReadinessError::Limit(LimitKind::Missions));
                }
                state.capacity = Some(*available);
            }
            Event::Status { scope, status, .. } => {
                let i = self.resolve_scope(state, scope)?;
                let previous = state.missions[i].status;
                if previous != *status
                    && !matches!(
                        (previous, status),
                        (
                            MissionStatus::Pending,
                            MissionStatus::Active | MissionStatus::Cancelled
                        ) | (
                            MissionStatus::Active,
                            MissionStatus::Accepted | MissionStatus::Cancelled
                        )
                    )
                {
                    return Err(ReadinessError::InvalidTransition);
                }
                state.missions[i].status = *status;
            }
            Event::Unsupported(op) => return Err(ReadinessError::Unsupported(*op)),
        }
        Ok(())
    }
    fn review(
        &self,
        state: &mut ProjectionState,
        n: &Notification,
        r: &EdgeReview,
    ) -> Result<(), ReadinessError> {
        let i = self.graph.edge_index(&r.edge)?;
        let edge = &self.graph.edges[i];
        if r.role != ReviewRole::Coordinator {
            return Err(ReadinessError::ReviewRejected(
                ReviewRefusal::NotCoordinator,
            ));
        }
        let p = self.resolve_scope(state, &r.producer)?;
        let c = self.resolve_scope(state, &r.consumer)?;
        if edge.producer != *r.producer.mission() || edge.consumer != *r.consumer.mission() {
            return Err(ReadinessError::IncompatibleReference);
        }
        let Event::Artifact(report) = &self.notification(&r.report)?.event else {
            return Err(ReadinessError::IncompatibleReference);
        };
        if report.producer != r.producer {
            return Err(ReadinessError::StaleContext);
        }
        if report.artifact != edge.requirement.artifact || report.kind != edge.requirement.kind {
            return Err(ReadinessError::ReviewRejected(
                ReviewRefusal::ArtifactContract,
            ));
        }
        if report.level != edge.requirement.level {
            return Err(ReadinessError::ReviewRejected(
                ReviewRefusal::ReadinessLevel,
            ));
        }
        for (j, support) in r.supports.iter().enumerate() {
            if !edge.requirement.predicates.contains(&support.predicate)
                || r.supports[..j]
                    .iter()
                    .any(|other| other.predicate == support.predicate)
            {
                return Err(ReadinessError::ReviewRejected(ReviewRefusal::Predicate));
            }
        }
        let hold = if !self.graph.missions[p].context.available()
            || !self.graph.missions[c].context.available()
        {
            Some(HoldReason::MissingContext)
        } else if !state.missions[p].coverage_known || !state.missions[c].coverage_known {
            Some(HoldReason::UnknownCoverage)
        } else if r.supports.len() != edge.requirement.predicates.len() {
            Some(HoldReason::MissingSupport)
        } else if report.level == ReadinessLevel::ImplementationProvisional
            && r.attestation.provisional.is_none()
        {
            Some(HoldReason::ProvisionalPolicy)
        } else if edge.requirement.kind == DependencyKind::MissionAcceptance
            && state.missions[p].status != MissionStatus::Accepted
        {
            Some(HoldReason::ProducerNotAccepted)
        } else {
            None
        };
        if let Some(reason) = hold {
            // A held proposal cannot revoke a valid earlier release. Explicit invalidation owns revocation.
            if !matches!(state.edges[i].decision, EdgeDecision::Released { .. }) {
                state.edges[i].decision = EdgeDecision::Held(reason);
            }
        } else {
            state.edges[i]
                .releases
                .try_reserve(1)
                .map_err(|_| ReadinessError::AllocationFailed)?;
            state.edges[i].releases.push(ReleaseRecord {
                review: n.id.clone(),
                report: r.report.clone(),
                invalidated_by: None,
            });
            state.edges[i].decision = EdgeDecision::Released {
                review: n.id.clone(),
                report: r.report.clone(),
            };
        }
        Ok(())
    }
}

fn recompute(graph: &DependencyGraph, state: &mut ProjectionState) -> Result<(), ReadinessError> {
    for (i, m) in graph.missions.iter().enumerate() {
        let mut blockers = reserved(
            graph
                .edges
                .iter()
                .filter(|edge| edge.consumer == *m.id())
                .count()
                .checked_add(5)
                .ok_or(ReadinessError::Limit(LimitKind::Edges))?,
        )?;
        let s = &state.missions[i];
        if s.status != MissionStatus::Pending {
            blockers.push(MissionBlocker::NotPending);
        }
        match s.preconditions {
            None => blockers.push(MissionBlocker::PreconditionsUnknown),
            Some(false) => blockers.push(MissionBlocker::PreconditionsUnmet),
            Some(true) => {}
        }
        let context = m.context.available();
        if !context {
            blockers.push(MissionBlocker::MissingContext);
        }
        if !s.coverage_known {
            blockers.push(MissionBlocker::UnknownCoverage);
        }
        let mut dependencies = context && s.coverage_known;
        for (j, edge) in graph.edges.iter().enumerate() {
            if edge.consumer == *m.id()
                && !matches!(state.edges[j].decision, EdgeDecision::Released { .. })
            {
                dependencies = false;
                blockers.push(MissionBlocker::Dependency(edge.id.clone()));
            }
        }
        match state.capacity {
            None => blockers.push(MissionBlocker::CapacityUnknown),
            Some(0) => blockers.push(MissionBlocker::NoCapacity),
            Some(_) => {}
        }
        state.missions[i].dependency_ready = dependencies;
        state.missions[i].blockers = blockers;
    }
    Ok(())
}

fn reserved<T>(n: usize) -> Result<Vec<T>, ReadinessError> {
    let mut out = Vec::new();
    out.try_reserve_exact(n)
        .map_err(|_| ReadinessError::AllocationFailed)?;
    Ok(out)
}
fn count_artifact(u: &mut InputUsage, a: &ArtifactReference) -> Result<(), ReadinessError> {
    u.bytes_add(a.id.0.len())?;
    u.bytes_add(a.candidate.as_str().len())
}
fn count_source(u: &mut InputUsage, s: &SourceReference) -> Result<(), ReadinessError> {
    u.support_add(1)?;
    u.bytes_add(s.source().len())?;
    u.bytes_add(s.version().len())?;
    match s.origin() {
        SourceOrigin::OperatorReported { author } => u.bytes_add(author.len()),
        SourceOrigin::ObservationReference(id) => u.bytes_add(id.scope().len()),
    }
}
fn count_contract(u: &mut InputUsage, c: &CandidateBindingContract) -> Result<(), ReadinessError> {
    u.bytes_add(c.task().id().as_str().len())?;
    u.bytes_add(c.task().objective().len())?;
    u.bytes_add(c.task().acceptance().len())?;
    u.bytes_add(c.candidate().commit().as_str().len())?;
    for p in [
        c.candidate().repository().root(),
        c.candidate().repository().git_dir(),
        c.candidate().repository().common_dir(),
    ] {
        u.bytes_add(p.as_os_str().as_encoded_bytes().len())?;
    }
    Ok(())
}
fn count_context(
    u: &mut InputUsage,
    c: &MissionContext,
    limits: GraphLimits,
) -> Result<(), ReadinessError> {
    count_contract(u, &c.contract)?;
    u.check(limits)?;
    if let Some(capture) = &c.capture {
        count_contract(u, capture.contract())?;
        u.bytes_add(capture.id().scope().len())?;
        if let Some(r) = capture.state().repository() {
            for p in [r.root(), r.git_dir(), r.common_dir()] {
                u.bytes_add(p.as_os_str().as_encoded_bytes().len())?;
            }
        }
        match capture.state().head() {
            Head::Attached { branch, commit } => {
                u.bytes_add(branch.len())?;
                u.bytes_add(commit.as_str().len())?;
            }
            Head::Detached { commit } => u.bytes_add(commit.as_str().len())?,
            Head::Unborn { branch } => u.bytes_add(branch.len())?,
            Head::Unknown => {}
        }
        for entry in capture.state().entries() {
            u.bytes_add(entry.path().len())?;
            u.check(limits)?;
        }
        if let Some(basis) = capture.basis() {
            u.bytes_add(basis.profile().len())?;
            u.bytes_add(basis.material().len())?;
        }
    }
    u.check(limits)
}
fn count_notification(
    u: &mut InputUsage,
    n: &Notification,
    limits: GraphLimits,
) -> Result<(), ReadinessError> {
    u.bytes_add(n.id.0.len())?;
    u.bytes_add(n.graph.id.0.len())?;
    match &n.event {
        Event::Signal { mission, message } => {
            u.bytes_add(mission.as_str().len())?;
            u.bytes_add(message.len())?;
        }
        Event::Artifact(r) => {
            count_context(u, &r.producer.context, limits)?;
            count_artifact(u, &r.artifact)?;
            count_source(u, &r.source)?;
        }
        Event::Review(r) => {
            u.bytes_add(r.edge.0.len())?;
            u.bytes_add(r.report.0.len())?;
            count_context(u, &r.producer.context, limits)?;
            count_context(u, &r.consumer.context, limits)?;
            for s in r.attestation.references() {
                count_source(u, s)?;
            }
            if let Some(policy) = &r.attestation.provisional {
                for s in policy.references() {
                    count_source(u, s)?;
                }
            }
            for support in &r.supports {
                u.bytes_add(support.predicate.0.len())?;
                for s in support.references() {
                    count_source(u, s)?;
                }
                u.check(limits)?;
            }
        }
        Event::Invalidate { roots, cause, .. } => {
            if roots.len() > limits.missions {
                return Err(ReadinessError::Limit(LimitKind::Missions));
            }
            for s in roots {
                count_context(u, &s.context, limits)?;
            }
            count_source(u, cause)?;
        }
        Event::Preconditions { scope, source, .. } | Event::Status { scope, source, .. } => {
            count_context(u, &scope.context, limits)?;
            count_source(u, source)?;
        }
        Event::Capacity { source, .. } => count_source(u, source)?,
        Event::Unsupported(_) => {}
    }
    u.check(limits)
}

// Every aggregate containing existing unredacted task/capture types is opaque in Debug.
redacted!(
    GraphReference,
    MissionContext,
    MissionScope,
    Mission,
    ArtifactReference,
    ArtifactRequirement,
    DependencyEdge,
    DependencyGraph,
    ArtifactReport,
    PredicateSupport,
    ProvisionalUse,
    ReviewAttestation,
    EdgeReview,
    Event,
    Notification,
    HoldReason,
    EdgeDecision,
    ReleaseRecord,
    EdgeState,
    MissionBlocker,
    MissionState,
    TransitionReport,
    ReadinessProjection
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_usage_refuses_overflow_without_wrapping() {
        let mut usage = InputUsage {
            bytes: usize::MAX,
            supports: usize::MAX,
        };
        assert_eq!(
            usage.bytes_add(1),
            Err(ReadinessError::Limit(LimitKind::Bytes))
        );
        assert_eq!(
            usage.support_add(1),
            Err(ReadinessError::Limit(LimitKind::Supports))
        );
    }

    #[test]
    fn identifiers_reuse_existing_validation_without_payload_diagnostics() {
        for value in ["", "-x", "a/b", "private@example.invalid", "á"] {
            assert!(GraphId::new(value).is_err());
            assert!(NotificationId::new(value).is_err());
        }
        assert!(GraphId::new(&"x".repeat(128)).is_ok());
        let error = EdgeId::new(&"x".repeat(129)).unwrap_err();
        assert!(!error.to_string().contains(&"x".repeat(129)));
        assert_eq!(
            format!("{:?}", ArtifactId::new("private-name").unwrap()),
            "ArtifactId([redacted])"
        );
    }
}
