//! Immutable explicit engineering-method definitions; no discovery, execution or authority.
//!
//! Definitions and source availability are caller declarations. Exact internal revisions
//! are not semantic skill versions or authenticated execution references. Constructors
//! validate text/relationships; [`MethodSet`](crate::methods::MethodSet) bounds aggregate logical input occurrences.
//! Caller-owned vectors already exist before admission; these are not allocator/RSS limits.

//! ```
//! use cortex_core::{binding::ContractRevision, methods::*};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let set = starter_methods(DefinitionLimits::new(2, 8, 16, 128, 100_000)?)?;
//! let reference = MethodReference::new(
//!     MethodId::new("native.candidate-correspondence")?, ContractRevision::new(1)?);
//! assert_eq!(set.definition(&reference)?.claims()[0].cases().len(), 4);
//! // A definition is not a qualification result or an execution grant.
//! # Ok(()) }
//! ```

use crate::{
    ContractError, TaskId,
    binding::ContractRevision,
    temporal::{SourceOrigin, SourceReference},
    validate_text,
};
use std::{error::Error, fmt};

macro_rules! opaque {
    ($($t:ty),+ $(,)?) => {$ (
        impl fmt::Debug for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($t), "([redacted])"))
            }
        }
    )+};
}
macro_rules! identity {
    ($t:ident, $doc:literal) => {
        #[doc=$doc]
        #[derive(Clone, PartialEq, Eq, Hash)]
        pub struct $t(TaskId);
        impl $t {
            /// Validate existing TaskId syntax; this does not create a task or authority.
            pub fn new(value: &str) -> Result<Self, MethodDefinitionError> {
                TaskId::new(value)
                    .map(Self)
                    .map_err(MethodDefinitionError::Text)
            }
            /// Read private caller data explicitly, without normalization.
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }
        }
        opaque!($t);
    };
}
identity!(
    MethodId,
    "Method identity, distinct from the engineering property."
);
identity!(
    PropertyId,
    "Engineering objective identity, independent of a technique."
);
identity!(
    ClaimId,
    "Claim identity scoped to one exact method revision."
);
identity!(
    CaseId,
    "Qualification case identity scoped to one exact method revision."
);

/// Exact internal revision; no ordering, latest alias, SemVer or global registry.
#[derive(Clone, PartialEq, Eq)]
pub struct MethodReference {
    id: MethodId,
    revision: ContractRevision,
}
impl MethodReference {
    /// Pair already validated identity and positive revision.
    pub fn new(id: MethodId, revision: ContractRevision) -> Self {
        Self { id, revision }
    }
    /// Inspect method identity.
    pub fn id(&self) -> &MethodId {
        &self.id
    }
    /// Inspect the exact revision, not a quality score.
    pub fn revision(&self) -> ContractRevision {
        self.revision
    }
}
opaque!(MethodReference);

/// Logical input dimension; fixed enum/numeric sizes and allocator overhead are excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionLimitKind {
    /// Input definition occurrences before identical duplicates collapse.
    Definitions,
    /// Claim occurrences across all supplied definitions.
    Claims,
    /// Qualification case occurrences.
    Cases,
    /// Source, component, dependency, requirement and exception entry occurrences.
    References,
    /// UTF-8/identifier/source material byte occurrences.
    MaterialBytes,
}
impl DefinitionLimitKind {
    fn index(self) -> usize {
        self as usize
    }
}
/// Positive finite caller-selected logical limits, not host enforcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefinitionLimits([usize; 5]);
impl DefinitionLimits {
    /// Validate every bound; none is implicit or unlimited.
    pub fn new(
        definitions: usize,
        claims: usize,
        cases: usize,
        references: usize,
        material: usize,
    ) -> Result<Self, MethodDefinitionError> {
        let values = [definitions, claims, cases, references, material];
        if values.iter().any(|n| *n == 0 || *n > isize::MAX as usize) {
            return Err(MethodDefinitionError::InvalidLimits);
        }
        Ok(Self(values))
    }
    /// Inspect a selected bound.
    pub fn get(self, kind: DefinitionLimitKind) -> usize {
        self.0[kind.index()]
    }
}
/// Charged occurrences, independently inspectable from the actual heap footprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DefinitionUsage([usize; 5]);
impl DefinitionUsage {
    /// Read charged usage.
    pub fn get(self, kind: DefinitionLimitKind) -> usize {
        self.0[kind.index()]
    }
    fn add(
        &mut self,
        kind: DefinitionLimitKind,
        count: usize,
    ) -> Result<(), MethodDefinitionError> {
        let n = &mut self.0[kind.index()];
        *n = n
            .checked_add(count)
            .ok_or(MethodDefinitionError::Overflow)?;
        Ok(())
    }
    fn text(&mut self, value: &str) -> Result<(), MethodDefinitionError> {
        self.add(DefinitionLimitKind::MaterialBytes, value.len())
    }
    fn source(&mut self, s: &SourceReference) -> Result<(), MethodDefinitionError> {
        self.add(DefinitionLimitKind::References, 1)?;
        self.text(s.source())?;
        self.text(s.version())?;
        match s.origin() {
            SourceOrigin::OperatorReported { author } => self.text(author),
            SourceOrigin::ObservationReference(id) => self.text(id.scope()),
        }
    }
    fn sources(&mut self, sources: &[SourceReference]) -> Result<(), MethodDefinitionError> {
        for s in sources {
            self.source(s)?;
        }
        Ok(())
    }
    fn merge(&mut self, other: Self) -> Result<(), MethodDefinitionError> {
        for (n, m) in self.0.iter_mut().zip(other.0) {
            *n = n.checked_add(m).ok_or(MethodDefinitionError::Overflow)?;
        }
        Ok(())
    }
}
/// Invalid declaration; diagnostics never repeat a private payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodDefinitionError {
    /// Existing bounded text/identity validation failed.
    Text(ContractError),
    /// A bound was zero or not representable as a finite collection limit.
    InvalidLimits,
    /// A required exact reference is absent.
    MissingReference,
    /// An identity has conflicting or ambiguous declarations.
    IdentityConflict,
    /// No claims were declared.
    MissingClaim,
    /// A finite logical budget was exceeded.
    Limit(DefinitionLimitKind),
    /// Checked accounting overflowed.
    Overflow,
}
impl fmt::Display for MethodDefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Text(_) => "invalid method text or identity",
            Self::InvalidLimits => "method limits must be positive and finite",
            Self::MissingReference => "method reference is unavailable",
            Self::IdentityConflict => "method declaration identity conflicts",
            Self::MissingClaim => "method needs an explicit claim",
            Self::Limit(_) => "method input exceeds a logical budget",
            Self::Overflow => "method accounting overflow",
        })
    }
}
impl Error for MethodDefinitionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Text(e) => Some(e),
            _ => None,
        }
    }
}
fn text(value: &str) -> Result<String, MethodDefinitionError> {
    validate_text("method_text", value).map_err(MethodDefinitionError::Text)?;
    Ok(value.to_owned())
}
fn optional(value: Option<&str>) -> Result<Option<String>, MethodDefinitionError> {
    value.map(text).transpose()
}

/// Component or cross-component scope; this is not a language analyzer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// One component's obligations.
    Component,
    /// A boundary between explicitly declared components.
    Interface,
}
/// Caller-declared component facts; missing fields remain unknown.
#[derive(Clone, PartialEq, Eq)]
pub struct ComponentScope {
    id: TaskId,
    language: Option<String>,
    ecosystem: Option<String>,
    version: Option<String>,
    sources: Vec<SourceReference>,
}
impl ComponentScope {
    /// Retain bounded facts without inferring applicability or authenticated capture.
    pub fn new(
        id: TaskId,
        language: Option<&str>,
        ecosystem: Option<&str>,
        version: Option<&str>,
        sources: Vec<SourceReference>,
    ) -> Result<Self, MethodDefinitionError> {
        Ok(Self {
            id,
            language: optional(language)?,
            ecosystem: optional(ecosystem)?,
            version: optional(version)?,
            sources,
        })
    }
    /// Read component identity.
    pub fn id(&self) -> &TaskId {
        &self.id
    }
    /// Read declared language or its absence.
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }
    /// Read declared ecosystem or its absence.
    pub fn ecosystem(&self) -> Option<&str> {
        self.ecosystem.as_deref()
    }
    /// Read declared version or its absence.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
    /// Read declared grounding references, not authenticated facts.
    pub fn sources(&self) -> &[SourceReference] {
        &self.sources
    }
}
/// Exact descriptive target scope; no version-range interpretation or automatic selection.
#[derive(Clone, PartialEq, Eq)]
pub struct TargetScope {
    id: TaskId,
    revision: ContractRevision,
    kind: ScopeKind,
    components: Vec<ComponentScope>,
    architecture: Option<String>,
    domain: Option<String>,
    environment: Option<String>,
}
impl TargetScope {
    /// Preserve unknown dimensions and reject ambiguous component identities.
    pub fn new(
        id: TaskId,
        revision: ContractRevision,
        kind: ScopeKind,
        components: Vec<ComponentScope>,
        architecture: Option<&str>,
        domain: Option<&str>,
        environment: Option<&str>,
    ) -> Result<Self, MethodDefinitionError> {
        for (i, c) in components.iter().enumerate() {
            if components[..i].iter().any(|p| p.id == c.id) {
                return Err(MethodDefinitionError::IdentityConflict);
            }
        }
        Ok(Self {
            id,
            revision,
            kind,
            components,
            architecture: optional(architecture)?,
            domain: optional(domain)?,
            environment: optional(environment)?,
        })
    }
    /// Read exact scope identity.
    pub fn id(&self) -> &TaskId {
        &self.id
    }
    /// Read declared scope revision.
    pub fn revision(&self) -> ContractRevision {
        self.revision
    }
    /// Read component/interface placement.
    pub fn kind(&self) -> ScopeKind {
        self.kind
    }
    /// Read retained component descriptions.
    pub fn components(&self) -> &[ComponentScope] {
        &self.components
    }
    /// Read declared architecture; absence is unknown.
    pub fn architecture(&self) -> Option<&str> {
        self.architecture.as_deref()
    }
    /// Read declared domain; absence is unknown.
    pub fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }
    /// Read declared environment; absence is unknown.
    pub fn environment(&self) -> Option<&str> {
        self.environment.as_deref()
    }
    /// Count source/component and text occurrences for consumer admission.
    pub fn input_usage(&self) -> Result<DefinitionUsage, MethodDefinitionError> {
        let mut u = DefinitionUsage::default();
        u.text(self.id.as_str())?;
        for s in [&self.architecture, &self.domain, &self.environment]
            .into_iter()
            .flatten()
        {
            u.text(s)?;
        }
        for c in &self.components {
            u.add(DefinitionLimitKind::References, 1)?;
            u.text(c.id.as_str())?;
            for s in [&c.language, &c.ecosystem, &c.version]
                .into_iter()
                .flatten()
            {
                u.text(s)?;
            }
            u.sources(&c.sources)?;
        }
        Ok(u)
    }
}
/// Descriptive relevance, independent of Evidence applicability and method selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyRelevance {
    /// Broad objective; associated techniques are not all mandatory.
    CrossCutting,
    /// A declared trigger matters.
    Contextual,
    /// Relevant context has not been established.
    Undetermined,
}
/// Trigger/absence descriptions; no executable predicate or approved exception.
#[derive(Clone, PartialEq, Eq)]
pub struct ApplicabilityConditions {
    relevance: PropertyRelevance,
    trigger: String,
    absence: String,
}
impl ApplicabilityConditions {
    /// Retain descriptions; absence still needs separate scoped evidence.
    pub fn new(
        relevance: PropertyRelevance,
        trigger: &str,
        absence: &str,
    ) -> Result<Self, MethodDefinitionError> {
        Ok(Self {
            relevance,
            trigger: text(trigger)?,
            absence: text(absence)?,
        })
    }
    /// Read broad objective classification.
    pub fn relevance(&self) -> PropertyRelevance {
        self.relevance
    }
    /// Read the contextual trigger.
    pub fn trigger(&self) -> &str {
        &self.trigger
    }
    /// Read the legitimate absence condition, not a finding that it holds.
    pub fn absence(&self) -> &str {
        &self.absence
    }
}
/// A property remains independent of any particular method.
#[derive(Clone, PartialEq, Eq)]
pub struct EngineeringProperty {
    id: PropertyId,
    objective: String,
    violation: String,
    requirements: Vec<SourceReference>,
}
impl EngineeringProperty {
    /// Retain the objective and meaningful violation condition.
    pub fn new(
        id: PropertyId,
        objective: &str,
        violation: &str,
        requirements: Vec<SourceReference>,
    ) -> Result<Self, MethodDefinitionError> {
        Ok(Self {
            id,
            objective: text(objective)?,
            violation: text(violation)?,
            requirements,
        })
    }
    /// Read property identity.
    pub fn id(&self) -> &PropertyId {
        &self.id
    }
    /// Read the engineering objective.
    pub fn objective(&self) -> &str {
        &self.objective
    }
    /// Read the violation condition.
    pub fn violation(&self) -> &str {
        &self.violation
    }
    /// Read canonical requirement references.
    pub fn requirements(&self) -> &[SourceReference] {
        &self.requirements
    }
}
/// Concrete mechanism description; there is no executable callback.
#[derive(Clone, PartialEq, Eq)]
pub struct MethodMechanism {
    source: SourceReference,
    procedure: String,
    observations: String,
    limitations: String,
}
impl MethodMechanism {
    /// Retain exact source version and observation requirements.
    pub fn new(
        source: SourceReference,
        procedure: &str,
        observations: &str,
        limitations: &str,
    ) -> Result<Self, MethodDefinitionError> {
        Ok(Self {
            source,
            procedure: text(procedure)?,
            observations: text(observations)?,
            limitations: text(limitations)?,
        })
    }
    /// Read mechanism provenance.
    pub fn source(&self) -> &SourceReference {
        &self.source
    }
    /// Read procedure; inspection never executes it.
    pub fn procedure(&self) -> &str {
        &self.procedure
    }
    /// Read required observations.
    pub fn observations(&self) -> &str {
        &self.observations
    }
    /// Read unsupported coverage.
    pub fn limitations(&self) -> &str {
        &self.limitations
    }
}
/// Required role of a qualification case; not an Evidence result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseRole {
    /// Check correct handling of a meaningful violation.
    Violation,
    /// Ensure legitimate neighboring behavior is accepted.
    LegitimateNeighbor,
    /// Preserve a scoped challenge to the claim.
    Counterexample,
    /// Another explicitly required case.
    Additional,
}
/// Expected behavior of a qualification procedure, not an execution record.
#[derive(Clone, PartialEq, Eq)]
pub struct QualificationCase {
    id: CaseId,
    role: CaseRole,
    fixture: SourceReference,
    expected: String,
    oracle: SourceReference,
}
impl QualificationCase {
    /// Retain fixture and oracle with their exact versions.
    pub fn new(
        id: CaseId,
        role: CaseRole,
        fixture: SourceReference,
        expected: &str,
        oracle: SourceReference,
    ) -> Result<Self, MethodDefinitionError> {
        Ok(Self {
            id,
            role,
            fixture,
            expected: text(expected)?,
            oracle,
        })
    }
    /// Read case identity.
    pub fn id(&self) -> &CaseId {
        &self.id
    }
    /// Read case role.
    pub fn role(&self) -> CaseRole {
        self.role
    }
    /// Read fixture provenance.
    pub fn fixture(&self) -> &SourceReference {
        &self.fixture
    }
    /// Read the entire expected single criterion.
    pub fn expected(&self) -> &str {
        &self.expected
    }
    /// Read oracle provenance, not authentication.
    pub fn oracle(&self) -> &SourceReference {
        &self.oracle
    }
}
/// One bounded claim with explicit exclusions, case plan and source dependencies.
#[derive(Clone, PartialEq, Eq)]
pub struct MethodClaim {
    id: ClaimId,
    statement: String,
    exclusions: String,
    cases: Vec<QualificationCase>,
    required_sources: Vec<SourceReference>,
    conflicts: Vec<SourceReference>,
}
impl MethodClaim {
    /// Missing cases/roles remain inspectable qualification gaps, not invented coverage.
    pub fn new(
        id: ClaimId,
        statement: &str,
        exclusions: &str,
        cases: Vec<QualificationCase>,
        required_sources: Vec<SourceReference>,
        conflicts: Vec<SourceReference>,
    ) -> Result<Self, MethodDefinitionError> {
        for (i, c) in cases.iter().enumerate() {
            if cases[..i].iter().any(|p| p.id == c.id) {
                return Err(MethodDefinitionError::IdentityConflict);
            }
        }
        Ok(Self {
            id,
            statement: text(statement)?,
            exclusions: text(exclusions)?,
            cases,
            required_sources,
            conflicts,
        })
    }
    /// Read claim identity.
    pub fn id(&self) -> &ClaimId {
        &self.id
    }
    /// Read supported proposition.
    pub fn statement(&self) -> &str {
        &self.statement
    }
    /// Read unsupported subclaims.
    pub fn exclusions(&self) -> &str {
        &self.exclusions
    }
    /// Read all required cases; no case disappears through deduplication.
    pub fn cases(&self) -> &[QualificationCase] {
        &self.cases
    }
    /// Read required material, distinct from background references.
    pub fn required_sources(&self) -> &[SourceReference] {
        &self.required_sources
    }
    /// Read unresolved source disagreements; inspection does not invent consensus.
    pub fn conflicts(&self) -> &[SourceReference] {
        &self.conflicts
    }
}
/// Caller-reported access; no status authenticates original material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceAvailability {
    /// Caller says exact original material was inspected.
    Inspected,
    /// Only a synthesis or excerpt is supplied.
    SummaryOnly,
    /// Required original material is unavailable.
    Unavailable,
    /// Access has not been established.
    Unspecified,
}
/// Source kind, independent of access and trust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// Original definition/code/material claimed by the caller.
    Original,
    /// A report imported from another producer.
    Imported,
    /// A synthesis of other material.
    Synthesis,
}
/// Source provenance and limits; never a trusted source flag.
#[derive(Clone, PartialEq, Eq)]
pub struct SourceEntry {
    reference: SourceReference,
    availability: SourceAvailability,
    kind: SourceKind,
    locator: String,
    note: String,
}
impl SourceEntry {
    /// Retain access and provenance claims without reading the referenced source.
    pub fn new(
        reference: SourceReference,
        availability: SourceAvailability,
        kind: SourceKind,
        locator: &str,
        note: &str,
    ) -> Result<Self, MethodDefinitionError> {
        Ok(Self {
            reference,
            availability,
            kind,
            locator: text(locator)?,
            note: text(note)?,
        })
    }
    /// Read exact source reference.
    pub fn reference(&self) -> &SourceReference {
        &self.reference
    }
    /// Read caller-reported access.
    pub fn availability(&self) -> SourceAvailability {
        self.availability
    }
    /// Read original/imported/synthesis distinction.
    pub fn kind(&self) -> SourceKind {
        self.kind
    }
    /// Read private locator explicitly.
    pub fn locator(&self) -> &str {
        &self.locator
    }
    /// Read source limits or unresolved disagreement.
    pub fn note(&self) -> &str {
        &self.note
    }
}
/// Inherited obligations, exceptions and authority requirements remain declarations.
#[derive(Clone, PartialEq, Eq)]
pub struct MethodContext {
    target: TargetScope,
    applicability: ApplicabilityConditions,
    requirements: Vec<SourceReference>,
    exceptions: Vec<SourceReference>,
    authority: SourceReference,
    dependencies: Vec<SourceReference>,
}
impl MethodContext {
    /// Retain exceptions without approving them or removing inherited requirements.
    pub fn new(
        target: TargetScope,
        applicability: ApplicabilityConditions,
        requirements: Vec<SourceReference>,
        exceptions: Vec<SourceReference>,
        authority: SourceReference,
        dependencies: Vec<SourceReference>,
    ) -> Self {
        Self {
            target,
            applicability,
            requirements,
            exceptions,
            authority,
            dependencies,
        }
    }
    /// Read supported descriptive scope.
    pub fn target(&self) -> &TargetScope {
        &self.target
    }
    /// Read applicability declarations.
    pub fn applicability(&self) -> &ApplicabilityConditions {
        &self.applicability
    }
    /// Read all inherited requirement references.
    pub fn requirements(&self) -> &[SourceReference] {
        &self.requirements
    }
    /// Read unresolved exception references.
    pub fn exceptions(&self) -> &[SourceReference] {
        &self.exceptions
    }
    /// Read adoption/revision/retirement authority requirements; not a grant.
    pub fn authority_requirements(&self) -> &SourceReference {
        &self.authority
    }
    /// Read requalification/history dependencies, including declared gaps.
    pub fn dependencies(&self) -> &[SourceReference] {
        &self.dependencies
    }
}
/// Exact immutable method value; no registration, adoption or execution.
#[derive(Clone, PartialEq, Eq)]
pub struct MethodDefinition {
    reference: MethodReference,
    property: EngineeringProperty,
    mechanism: MethodMechanism,
    claims: Vec<MethodClaim>,
    sources: Vec<SourceEntry>,
    context: MethodContext,
    usage: DefinitionUsage,
}
impl MethodDefinition {
    /// Validate relationships; aggregate admission is separately bounded by MethodSet.
    pub fn new(
        reference: MethodReference,
        property: EngineeringProperty,
        mechanism: MethodMechanism,
        claims: Vec<MethodClaim>,
        sources: Vec<SourceEntry>,
        context: MethodContext,
    ) -> Result<Self, MethodDefinitionError> {
        if claims.is_empty() {
            return Err(MethodDefinitionError::MissingClaim);
        }
        for (i, c) in claims.iter().enumerate() {
            if claims[..i].iter().any(|p| p.id == c.id) {
                return Err(MethodDefinitionError::IdentityConflict);
            }
            for src in c.required_sources.iter().chain(&c.conflicts) {
                if !sources.iter().any(|s| &s.reference == src) {
                    return Err(MethodDefinitionError::MissingReference);
                }
            }
            for case in &c.cases {
                if claims[..i]
                    .iter()
                    .any(|p| p.cases.iter().any(|pc| pc.id == case.id))
                {
                    return Err(MethodDefinitionError::IdentityConflict);
                }
            }
        }
        for (i, src) in sources.iter().enumerate() {
            if sources[..i]
                .iter()
                .any(|p| p.reference == src.reference && p != src)
            {
                return Err(MethodDefinitionError::IdentityConflict);
            }
        }
        let mut value = Self {
            reference,
            property,
            mechanism,
            claims,
            sources,
            context,
            usage: DefinitionUsage::default(),
        };
        value.usage = value.measure()?;
        Ok(value)
    }
    /// Read exact method reference.
    pub fn reference(&self) -> &MethodReference {
        &self.reference
    }
    /// Read the independent engineering property.
    pub fn property(&self) -> &EngineeringProperty {
        &self.property
    }
    /// Read mechanism without invoking it.
    pub fn mechanism(&self) -> &MethodMechanism {
        &self.mechanism
    }
    /// Read distinct scoped claims.
    pub fn claims(&self) -> &[MethodClaim] {
        &self.claims
    }
    /// Read all available, imported and missing material declarations.
    pub fn sources(&self) -> &[SourceEntry] {
        &self.sources
    }
    /// Read applicability/authority and inherited requirements.
    pub fn context(&self) -> &MethodContext {
        &self.context
    }
    /// Read logical occurrences used by MethodSet and Evidence admission.
    pub fn input_usage(&self) -> DefinitionUsage {
        self.usage
    }
    fn measure(&self) -> Result<DefinitionUsage, MethodDefinitionError> {
        let mut u = DefinitionUsage::default();
        u.add(DefinitionLimitKind::Definitions, 1)?;
        u.text(self.reference.id.as_str())?;
        u.text(self.property.id.as_str())?;
        u.text(&self.property.objective)?;
        u.text(&self.property.violation)?;
        u.sources(&self.property.requirements)?;
        u.source(&self.mechanism.source)?;
        u.text(&self.mechanism.procedure)?;
        u.text(&self.mechanism.observations)?;
        u.text(&self.mechanism.limitations)?;
        u.merge(self.context.target.input_usage()?)?;
        u.text(&self.context.applicability.trigger)?;
        u.text(&self.context.applicability.absence)?;
        u.sources(&self.context.requirements)?;
        u.sources(&self.context.exceptions)?;
        u.source(&self.context.authority)?;
        u.sources(&self.context.dependencies)?;
        for s in &self.sources {
            u.source(&s.reference)?;
            u.text(&s.locator)?;
            u.text(&s.note)?;
        }
        for c in &self.claims {
            u.add(DefinitionLimitKind::Claims, 1)?;
            u.text(c.id.as_str())?;
            u.text(&c.statement)?;
            u.text(&c.exclusions)?;
            u.sources(&c.required_sources)?;
            u.sources(&c.conflicts)?;
            for case in &c.cases {
                u.add(DefinitionLimitKind::Cases, 1)?;
                u.text(case.id.as_str())?;
                u.source(&case.fixture)?;
                u.text(&case.expected)?;
                u.source(&case.oracle)?;
            }
        }
        Ok(u)
    }
}
/// One explicit bounded set, not a global registry or knowledge index.
#[derive(Clone, PartialEq, Eq)]
pub struct MethodSet {
    definitions: Vec<MethodDefinition>,
    usage: DefinitionUsage,
}
impl MethodSet {
    /// Charge every occurrence before collapsing identical duplicate definitions.
    /// Cross-call substitution is detectable only if accepted definitions are retained/presented.
    pub fn new(
        definitions: Vec<MethodDefinition>,
        limits: DefinitionLimits,
    ) -> Result<Self, MethodDefinitionError> {
        if definitions.len() > limits.get(DefinitionLimitKind::Definitions) {
            return Err(MethodDefinitionError::Limit(
                DefinitionLimitKind::Definitions,
            ));
        }
        let mut usage = DefinitionUsage::default();
        for d in &definitions {
            usage.merge(d.input_usage())?;
        }
        for kind in [
            DefinitionLimitKind::Definitions,
            DefinitionLimitKind::Claims,
            DefinitionLimitKind::Cases,
            DefinitionLimitKind::References,
            DefinitionLimitKind::MaterialBytes,
        ] {
            if usage.get(kind) > limits.get(kind) {
                return Err(MethodDefinitionError::Limit(kind));
            }
        }
        for (i, d) in definitions.iter().enumerate() {
            for prior in &definitions[..i] {
                if (d.reference == prior.reference && d != prior)
                    || (d.property.id == prior.property.id && d.property != prior.property)
                {
                    return Err(MethodDefinitionError::IdentityConflict);
                }
            }
        }
        let mut unique = Vec::new();
        for d in definitions {
            if !unique.contains(&d) {
                unique.push(d);
            }
        }
        Ok(Self {
            definitions: unique,
            usage,
        })
    }
    /// Resolve only an exact requested revision; never choose latest or a compatible guess.
    pub fn definition(
        &self,
        reference: &MethodReference,
    ) -> Result<&MethodDefinition, MethodDefinitionError> {
        self.definitions
            .iter()
            .find(|d| &d.reference == reference)
            .ok_or(MethodDefinitionError::MissingReference)
    }
    /// Inspect all definitions without invoking methods.
    pub fn definitions(&self) -> &[MethodDefinition] {
        &self.definitions
    }
    /// Read admitted input occurrences before duplicate collapse.
    pub fn input_usage(&self) -> DefinitionUsage {
        self.usage
    }
}
opaque!(
    ComponentScope,
    TargetScope,
    ApplicabilityConditions,
    EngineeringProperty,
    MethodMechanism,
    QualificationCase,
    MethodClaim,
    SourceEntry,
    MethodContext,
    MethodDefinition,
    MethodSet
);

/// Build the two explicit revision-1 starter definitions, without qualification records.
///
/// Their source versions identify the existing native mechanisms, not this call's
/// environment or authenticated capture. Qualification is supplied to Evidence
/// separately. This function neither invokes either mechanism nor marks it qualified.
pub fn starter_methods(limits: DefinitionLimits) -> Result<MethodSet, MethodDefinitionError> {
    fn source(path: &str, version: &str) -> Result<SourceReference, MethodDefinitionError> {
        SourceReference::operator_reported("Cortex native definition", path, version)
            .map_err(|_| MethodDefinitionError::MissingReference)
    }
    let mut definitions = Vec::new();
    for (name, property, mechanism_path, version, cases) in [
        (
            "native.candidate-correspondence",
            "engineering.candidate-correspondence",
            "crates/cortex-core/src/binding.rs",
            "git-blob:468e3246dee253bb3fb7d4eda694417b92713a0e",
            vec![
                (
                    "mismatch",
                    CaseRole::Violation,
                    "A different required commit is rejected with BindingMismatch::Commit",
                ),
                (
                    "detached",
                    CaseRole::LegitimateNeighbor,
                    "Matching detached HEAD binds within the declared clean observation model",
                ),
                (
                    "dirty",
                    CaseRole::Additional,
                    "Matching dirty state under AllowChanges remains UncommittedIdentity, not compatible",
                ),
                (
                    "matching",
                    CaseRole::Additional,
                    "Matching ordered clean observations retain a compatible bound candidate",
                ),
            ],
        ),
        (
            "native.assessment-adequacy",
            "engineering.truthful-assessment",
            "crates/cortex-evidence/src/unified/mod.rs",
            "git-blob:1914e2b1d5e3d65e5eed900fdc43339ac0d63a6a",
            vec![
                (
                    "zero-checks",
                    CaseRole::Violation,
                    "Exit zero with no relevant checks cannot receive PASS",
                ),
                (
                    "legitimate-fail",
                    CaseRole::LegitimateNeighbor,
                    "Exit zero with a violated oracle can legitimately receive FAIL",
                ),
                (
                    "legitimate-unknown",
                    CaseRole::LegitimateNeighbor,
                    "Completed zero-check evidence remains representable as UNKNOWN",
                ),
                (
                    "stale",
                    CaseRole::Additional,
                    "Known invalidation remains sticky when the previous context returns",
                ),
            ],
        ),
    ] {
        let mechanism = source(mechanism_path, version)?;
        let requirement = source(
            "docs/architecture/core-invariants.md",
            "INV-002/003/004/005",
        )?;
        let target = TargetScope::new(
            TaskId::new(name).map_err(MethodDefinitionError::Text)?,
            ContractRevision::new(1).map_err(|_| MethodDefinitionError::MissingReference)?,
            ScopeKind::Component,
            vec![ComponentScope::new(
                TaskId::new("native-fixture").map_err(MethodDefinitionError::Text)?,
                Some("Rust"),
                Some("Cortex native domain APIs"),
                Some("revision-1 fixture contract"),
                vec![mechanism.clone()],
            )?],
            None,
            Some("Bounded synthetic domain qualification cases"),
            None,
        )?;
        let exclusions = "Finite declared fixture behavior only; no authenticated capture, actual Git snapshot, dirty-byte identity, general software correctness, adoption, performance or cross-platform assurance";
        let mut required = Vec::new();
        for (id, role, expected) in cases {
            required.push(QualificationCase::new(
                CaseId::new(id)?,
                role,
                source(&format!("native-fixture/{name}/{id}"), "1")?,
                expected,
                source(&format!("literal-oracle/{name}/{id}"), "1")?,
            )?);
        }
        let mut dependencies = vec![
            mechanism.clone(),
            source(
                "cortex-core task/candidate contracts",
                "foundation-v1 and bounded 002-B",
            )?,
        ];
        dependencies.push(source(
            "rust-toolchain.toml",
            "git-blob:58eef2ccd48f957feeada49b17ad1e3fd667bf8c",
        )?);
        if name == "native.assessment-adequacy" {
            dependencies.push(source(
                "crates/cortex-evidence/src/unified/report.rs",
                "git-blob:5d117b92321e77c662394b276973b39304bc815c",
            )?);
            dependencies.push(source(
                "crates/cortex-core/src/binding.rs",
                "git-blob:468e3246dee253bb3fb7d4eda694417b92713a0e",
            )?);
        }
        let entries = dependencies
            .iter()
            .map(|dependency| {
                SourceEntry::new(
                    dependency.clone(),
                    SourceAvailability::Inspected,
                    SourceKind::Original,
                    dependency.source(),
                    "Declared native qualification dependency; no origin authentication",
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let claim = MethodClaim::new(
            ClaimId::new("bounded-cases")?,
            "The native mechanism handles every declared violation and legitimate-neighbor case",
            exclusions,
            required,
            dependencies.clone(),
            vec![],
        )?;
        let context = MethodContext::new(
            target,
            ApplicabilityConditions::new(
                PropertyRelevance::CrossCutting,
                "The declared native mechanism and supported candidate context are present",
                "Scoped inspection demonstrates the governed mechanism or subject is absent",
            )?,
            vec![requirement.clone()],
            vec![],
            source(
                "docs/decisions/open-decisions.md",
                "DEC-012 adoption/revision/retirement unresolved",
            )?,
            dependencies,
        );
        let objective = EngineeringProperty::new(
            PropertyId::new(property)?,
            "Engineering conclusions retain their actual scoped basis",
            "Unsupported correspondence or success must not become acceptance",
            vec![requirement],
        )?;
        let mechanism = MethodMechanism::new(
            mechanism,
            "Apply the existing native domain mechanism to exact declared fixture inputs",
            "Retain actual case execution, independent expected output, explicit oracle, profile and limitations",
            exclusions,
        )?;
        definitions.push(MethodDefinition::new(
            MethodReference::new(
                MethodId::new(name)?,
                ContractRevision::new(1).map_err(|_| MethodDefinitionError::MissingReference)?,
            ),
            objective,
            mechanism,
            vec![claim],
            entries,
            context,
        )?);
    }
    MethodSet::new(definitions, limits)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checked_definition_accounting_refuses_overflow() {
        let mut u = DefinitionUsage([usize::MAX; 5]);
        assert_eq!(
            u.add(DefinitionLimitKind::Claims, 1),
            Err(MethodDefinitionError::Overflow)
        );
    }
}
