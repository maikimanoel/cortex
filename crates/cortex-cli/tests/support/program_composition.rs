//! Shared test composition of the actual advisory method and scheduling consumers.
use cortex_core::{
    binding::CandidateObservation,
    methods::{DefinitionLimits, starter_methods},
    program::{ProgramState, context::PackageRequest},
    temporal::SourceReference,
};
use cortex_evidence::{
    methods::{
        InspectionLimits,
        proposal::{MethodPackageRequest, compose_method_package},
    },
    unified::{Applicability, VerificationProfile},
};

pub fn advisory_package(
    state: &ProgramState,
    package: PackageRequest,
    current: &CandidateObservation,
    profile: &VerificationProfile,
) -> PackageRequest {
    advisory_package_result(state, package, current, profile, false)
        .unwrap()
        .0
}

pub fn advisory_package_result(
    state: &ProgramState,
    package: PackageRequest,
    current: &CandidateObservation,
    profile: &VerificationProfile,
    exception: bool,
) -> Result<(PackageRequest, usize), cortex_evidence::methods::proposal::MethodProposalError> {
    let definitions =
        starter_methods(DefinitionLimits::new(8, 64, 128, 1024, 1_000_000).unwrap()).unwrap();
    let d = definitions.definitions()[0].clone();
    let context = d.context();
    let exceptions = if exception {
        vec![
            SourceReference::operator_reported(
                "explicit test caller",
                "unresolved-protected-exception",
                "1",
            )
            .unwrap(),
        ]
    } else {
        vec![]
    };
    let selected = cortex_core::methods::MethodDefinition::new(
        d.reference().clone(),
        d.property().clone(),
        d.mechanism().clone(),
        d.claims().to_vec(),
        d.sources().to_vec(),
        cortex_core::methods::MethodContext::new(
            context.target().clone(),
            context.applicability().clone(),
            context.requirements().to_vec(),
            exceptions,
            context.authority_requirements().clone(),
            context.dependencies().to_vec(),
        ),
    )
    .unwrap();
    let definitions = cortex_core::methods::MethodSet::new(
        vec![selected],
        DefinitionLimits::new(8, 64, 128, 1024, 1_000_000).unwrap(),
    )
    .unwrap();
    let definition = &definitions.definitions()[0];
    let method = definition.reference();
    let method_source = SourceReference::operator_reported(
        "explicit development fixture selection",
        method.id().as_str(),
        &method.revision().get().to_string(),
    )
    .unwrap();
    let mut records = [];
    let structural = cortex_evidence::methods::inspect_method(
        &definitions,
        method,
        None,
        &mut records,
        InspectionLimits::new(128, 128, 128, 4096, 8_000_000, 4096).unwrap(),
    )
    .unwrap();
    structural.check_basis(definition, None, None).unwrap();
    drop(structural);
    let composed = compose_method_package(
        &definitions,
        &mut records,
        state,
        MethodPackageRequest {
            method,
            method_source: &method_source,
            qualification: None,
            qualification_source: None,
            target_scope: definition.context().target(),
            current_target: Some(current),
            expected_profile: profile,
            current_profile: Some(profile),
            package,
        },
        InspectionLimits::new(128, 128, 128, 4096, 8_000_000, 4096).unwrap(),
    )?;
    assert_eq!(
        composed.inspection().applicability(),
        Applicability::Uncertain
    );
    assert!(
        composed
            .package()
            .request()
            .methods
            .last()
            .unwrap()
            .qualification
            .is_none()
    );
    composed
        .package()
        .check(state, composed.package().bytes())
        .unwrap();
    Ok((
        composed.package().request().clone(),
        composed.inspection().exceptions().len(),
    ))
}
