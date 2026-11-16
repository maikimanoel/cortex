//! CAP-WFL-002/CAP-SKL-001/CAP-SKL-002/CAP-CTX-001: explicit advisory composition.
//!
//! Supplied captures are caller assertions, not authenticated filesystem observations.
//! Structural basis equality and package reassembly do not establish live freshness.

use super::*;
use cortex_core::{
    binding::{BindingDecision, BindingError, bind_candidate},
    program::{
        ProgramError, ProgramState,
        context::{InstructionPackage, MethodProposal, PackageRequest, assemble},
    },
};
use std::{error::Error, fmt};

/// Exact selected method, current target observations and existing package inputs.
/// No reference grants execution, method adoption or protected exception authority.
pub struct MethodPackageRequest<'a> {
    /// Explicit selection from the supplied method set.
    pub method: &'a MethodReference,
    /// Exact method ID and decimal revision, with caller-supplied provenance.
    pub method_source: &'a SourceReference,
    /// Current examinations of the separately declared qualification cases.
    pub qualification: Option<&'a QualificationRequest<'a>>,
    /// Explicit reference, if supplied, must equal the qualification contract basis.
    pub qualification_source: Option<&'a SourceReference>,
    /// Exact declared target scope; no implicit language or scope conversion.
    pub target_scope: &'a TargetScope,
    /// Current target capture, compared separately with the program task baseline.
    pub current_target: Option<&'a CandidateObservation>,
    /// Explicit expected target verification profile.
    pub expected_profile: &'a VerificationProfile,
    /// Actual current target profile; absence cannot establish current qualification.
    pub current_profile: Option<&'a VerificationProfile>,
    /// Existing explicit package inputs; the selected proposal is appended once.
    pub package: PackageRequest,
}

/// Successful advisory composition and its complete scoped inspection.
pub struct MethodPackage<'d, 'r> {
    inspection: MethodInspection<'d, 'r>,
    package: InstructionPackage,
}
impl<'d, 'r> MethodPackage<'d, 'r> {
    /// Read every qualification finding and unresolved exception.
    pub fn inspection(&self) -> &MethodInspection<'d, 'r> {
        &self.inspection
    }
    /// Read the existing Core package, including its exact explicit request.
    pub fn package(&self) -> &InstructionPackage {
        &self.package
    }
}
opaque!(MethodPackageRequest<'_>, MethodPackage<'_, '_>);

/// Composition failure. Messages exclude supplied source and evidence contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodProposalError {
    /// Existing inspection validation or bounded-input failure.
    Inspection(MethodInspectionError),
    /// Existing package or program invariant failure.
    Program(ProgramError),
    /// Malformed current candidate correspondence.
    Binding(BindingError),
    /// Current capture could not establish the required target correspondence.
    TargetNotCurrent(BindingDecision),
    /// Declared target scope differs from the selected method or qualification.
    TargetScopeMismatch,
    /// Current target profile differs from its declared or applicability basis.
    TargetProfileMismatch,
    /// Method, qualification, or applicability reference does not correspond.
    SourceCorrespondence,
    /// The package already includes a proposal for the selected method identity.
    DuplicateMethod,
}
impl fmt::Display for MethodProposalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Inspection(_) => "method inspection rejected composition inputs",
            Self::Program(_) => "program package rejected composition inputs",
            Self::Binding(_) => "invalid target candidate correspondence",
            Self::TargetNotCurrent(_) => "current target correspondence is not established",
            Self::TargetScopeMismatch => "method target scope changed",
            Self::TargetProfileMismatch => {
                "current target profile correspondence is not established"
            }
            Self::SourceCorrespondence => {
                "selected method or qualification source does not correspond"
            }
            Self::DuplicateMethod => "selected method already occurs in package",
        })
    }
}
impl Error for MethodProposalError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Inspection(e) => Some(e),
            Self::Program(e) => Some(e),
            Self::Binding(e) => Some(e),
            _ => None,
        }
    }
}
impl From<MethodInspectionError> for MethodProposalError {
    fn from(e: MethodInspectionError) -> Self {
        Self::Inspection(e)
    }
}
impl From<ProgramError> for MethodProposalError {
    fn from(e: ProgramError) -> Self {
        Self::Program(e)
    }
}
impl From<BindingError> for MethodProposalError {
    fn from(e: BindingError) -> Self {
        Self::Binding(e)
    }
}

/// Inspect an explicit selection, append its disclosed advisory proposal, and assemble/check.
///
/// All input material is charged before the bounded staging copy. Typed errors leave
/// every caller record unchanged; successful negative findings may retain existing first
/// invalidation. Terminal assessments and the program remain unchanged. The staging copy
/// costs one bounded Evidence pool in addition to inspection's existing draft records.
/// No I/O, clock, durable admission, dispatch or automatic selection occurs. Package check
/// reassembles stored inputs; callers must separately supply actual current captures.
pub fn compose_method_package<'d, 'r>(
    definitions: &'d MethodSet,
    records: &'r mut [EvidenceRecord],
    state: &ProgramState,
    request: MethodPackageRequest<'_>,
    limits: InspectionLimits,
) -> Result<MethodPackage<'d, 'r>, MethodProposalError> {
    let definition = definitions
        .definition(request.method)
        .map_err(MethodInspectionError::from)?;
    let mut usage = measure(definition, request.qualification, records, limits)?;
    charge_scope(&mut usage, request.target_scope, limits)?;
    let scope = state
        .readiness()
        .scope(&request.package.task)
        .map_err(ProgramError::from)?;
    let context = scope.context();
    let mut material = Material::new(limits.get(InspectionLimitKind::MaterialBytes));
    material.bytes = usage.get(InspectionLimitKind::MaterialBytes);
    material.source(request.method_source)?;
    if let Some(source) = request.qualification_source {
        material.source(source)?;
    }
    if let Some(current) = request.current_target {
        material.capture(current)?;
    }
    if let Some(baseline) = context.capture() {
        material.capture(baseline)?;
    }
    material.profile(request.expected_profile)?;
    if let Some(profile) = request.current_profile {
        material.profile(profile)?;
    }
    usage.add(InspectionLimitKind::References, material.references, limits)?;

    if request.target_scope != definition.context().target()
        || request.qualification.is_some_and(|q| {
            q.scope != request.target_scope || q.contract.scope() != request.target_scope
        })
    {
        return Err(MethodProposalError::TargetScopeMismatch);
    }
    if request.method_source.source() != request.method.id().as_str()
        || request.method_source.version() != request.method.revision().get().to_string()
    {
        return Err(MethodProposalError::SourceCorrespondence);
    }
    if let Some(source) = request.qualification_source
        && request
            .qualification
            .is_none_or(|q| q.contract.basis() != source)
    {
        return Err(MethodProposalError::SourceCorrespondence);
    }
    if request
        .package
        .methods
        .iter()
        .any(|p| p.method.source() == request.method_source.source())
    {
        return Err(MethodProposalError::DuplicateMethod);
    }
    if request.package.methods.len() >= state.definition().limits().entries {
        return Err(ProgramError::Capacity.into());
    }
    if request.current_profile != Some(request.expected_profile) {
        return Err(MethodProposalError::TargetProfileMismatch);
    }
    let binding = bind_candidate(
        context.contract(),
        context.capture(),
        request.current_target,
    )?;
    if !matches!(binding, BindingDecision::Compatible(_)) {
        return Err(MethodProposalError::TargetNotCurrent(binding));
    }
    // Qualification cases concern their own fixture mappings. Only the distinct
    // applicability examination must match this actual package target and profile.
    if let Some(q) = request.qualification
        && let Some(app) = q.applicability
    {
        if app.verification() != context.contract() || app.scope() != request.target_scope {
            return Err(MethodProposalError::SourceCorrespondence);
        }
        if Some(app.profile()) != request.current_profile {
            return Err(MethodProposalError::TargetProfileMismatch);
        }
        for examination in q
            .examinations
            .iter()
            .filter(|e| e.observation == *app.evidence().observation())
        {
            if examination.contract != context.contract()
                || examination
                    .candidate
                    .is_some_and(|c| Some(c) != request.current_target)
            {
                return Err(MethodProposalError::SourceCorrespondence);
            }
            if examination
                .profile
                .is_some_and(|p| Some(p) != request.current_profile)
            {
                return Err(MethodProposalError::TargetProfileMismatch);
            }
        }
    }
    let mut staged = reserved(records.len())?;
    staged.extend(records.iter().cloned());
    let inspection = inspect_method(
        definitions,
        request.method,
        request.qualification,
        &mut staged,
        limits,
    )?;
    inspection.check_basis(
        definition,
        request.qualification.map(|q| q.contract),
        request.qualification.map(|q| q.scope),
    )?;
    let qualified = request.qualification_source.is_some()
        && inspection.applicability() == Applicability::Applicable
        && !inspection.claims().is_empty()
        && inspection
            .claims()
            .iter()
            .all(ClaimInspection::requirements_met)
        && inspection.exceptions().is_empty();
    let mut applicability = match inspection.applicability() {
        Applicability::Applicable => {
            "Applicable under supplied current observations; advisory only."
        }
        Applicability::NotApplicable => {
            "Not applicable under supplied current observations; advisory only."
        }
        Applicability::Uncertain => "Applicability is uncertain; advisory only.",
    }
    .to_owned();
    if !qualified {
        applicability.push_str(" Current qualification is not established.");
    }
    append_references(
        &mut applicability,
        "Inherited requirement",
        inspection.requirements(),
    );
    let mut exceptions =
        "Protected exceptions are unresolved declarations; no adoption or authority.".to_owned();
    append_references(
        &mut exceptions,
        "Unresolved exception",
        inspection.exceptions(),
    );
    let proposal = MethodProposal {
        method: request.method_source.clone(),
        applicability,
        qualification: if qualified {
            request.qualification_source.cloned()
        } else {
            None
        },
        exceptions,
    };
    let mut package_request = request.package;
    package_request
        .methods
        .try_reserve(1)
        .map_err(|_| MethodInspectionError::Allocation)?;
    package_request.methods.push(proposal);
    let package = assemble(state, package_request)?;
    package.check(state, package.bytes())?;
    let MethodInspection {
        definition,
        basis,
        claims,
        applicability,
        app_witness,
        usage,
        ..
    } = inspection;
    // All fallible operations are complete. Publication preserves terminal assessments.
    for (record, draft) in records.iter_mut().zip(staged) {
        *record = draft;
    }
    Ok(MethodPackage {
        inspection: MethodInspection {
            definition,
            basis,
            claims,
            applicability,
            app_witness,
            records,
            usage,
        },
        package,
    })
}

fn append_references(text: &mut String, label: &str, references: &[SourceReference]) {
    use std::fmt::Write;
    for reference in references {
        // Logical material and reference counts were charged before allocation.
        // String formatting cannot fail; final encoded bytes obey ProgramLimits.
        let _ = write!(
            text,
            " {label}: {} (version {}).",
            reference.source(),
            reference.version()
        );
    }
}
