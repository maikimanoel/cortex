//! Advisory admission executes real kernel guards without changing retained state.
#[path = "support/program.rs"]
mod program_fixture;
#[path = "support/domain.rs"]
mod support;
use cortex_core::{
    parallel::{DependencyCoverage, Event},
    program::{domain::*, operational::*, scheduling::Resource, *},
};
use cortex_evidence::program::*;
use support::*;

fn reserve(kernel: &CoordinationKernel, id: &str, intent: DomainIntent, at: u64) -> Operation {
    domain_operation(
        kernel,
        id,
        Command::Domain(Box::new(DomainEvent::Reserve { intent, at })),
    )
}

#[test]
fn admission_preview_binds_exact_input_and_does_not_reserve() {
    let (mut kernel, intent) = coordinator();
    let input = reserve(&kernel, "preview-reserve", intent.clone(), 1);
    let operations = kernel.operations().to_vec();
    let domains = kernel.domains().cloned();
    let effects = kernel.fixture_effects().clone();
    let next = kernel.revision() + 1;
    for _ in 0..2 {
        let preview = kernel.preview_admission(&input).unwrap();
        assert!(std::ptr::eq(preview.operation(), &input));
        assert!(preview.definition() == kernel.state().definition());
        assert_eq!(preview.observed_revision(), next - 1);
        assert_eq!(preview.resulting_ordinal(), next);
        assert!(!preview.is_duplicate());
    }
    assert!(kernel.operations() == operations);
    assert!(kernel.domains().cloned() == domains);
    assert!(kernel.fixture_effects() == &effects);
    assert_eq!(kernel.domains().unwrap().held(Resource::WorkerSlots), Ok(0));
    let receipt = kernel.admit(input).unwrap();
    assert_eq!(receipt.ordinal, next);
    assert!(!receipt.duplicate);
    assert_eq!(
        kernel.domains().unwrap().domain(&intent.id).unwrap().phase,
        DomainPhase::Reserved
    );
    assert_eq!(kernel.domains().unwrap().held(Resource::WorkerSlots), Ok(1));
}

#[test]
fn admission_preview_exact_duplicate_keeps_original_ordinal_after_creation_uncertainty() {
    let (mut kernel, intent) = coordinator();
    let input = reserve(&kernel, "reserve", intent.clone(), 1);
    let receipt = kernel.admit(input.clone()).unwrap();
    event(
        &mut kernel,
        "create",
        DomainEvent::CreateRequested {
            domain: intent.id.clone(),
            at: 2,
        },
    );
    let revision = kernel.revision();
    let preview = kernel.preview_admission(&input).unwrap();
    assert!(preview.is_duplicate());
    assert_eq!(preview.resulting_ordinal(), receipt.ordinal);
    assert_eq!(preview.observed_revision(), revision);
    assert_eq!(
        kernel.domains().unwrap().domain(&intent.id).unwrap().phase,
        DomainPhase::CreateUncertain
    );
    assert_eq!(kernel.domains().unwrap().held(Resource::WorkerSlots), Ok(1));
    assert_eq!(kernel.revision(), revision);
}

#[test]
fn admission_preview_preserves_identity_conflict_before_stale_predecessor() {
    let (mut kernel, intent) = coordinator();
    let input = reserve(&kernel, "reserve", intent.clone(), 1);
    kernel.admit(input.clone()).unwrap();
    let mut conflict = input;
    conflict.expected = kernel.revision();
    assert_eq!(
        kernel.preview_admission(&conflict).unwrap_err(),
        KernelError::Core(ProgramError::IdentityConflict)
    );
    let mut stale = reserve(&kernel, "different-operation", intent, 2);
    stale.expected -= 1;
    assert_eq!(
        kernel.preview_admission(&stale).unwrap_err(),
        KernelError::Core(ProgramError::Stale)
    );
}

#[test]
fn admission_preview_requires_actual_grant_and_known_resource_quantity() {
    let (kernel, intent) = coordinator();
    let mut input = reserve(&kernel, "without-authority", intent.clone(), 1);
    input.grant.id = GrantId::new("distinct-no-permission").unwrap();
    input.grant.permissions.clear();
    assert_eq!(
        kernel.preview_admission(&input).unwrap_err(),
        KernelError::Core(ProgramError::Authority)
    );
    let mut missing = intent;
    missing
        .effect
        .demand
        .iter_mut()
        .find(|d| d.resource == Resource::WorkerSlots)
        .unwrap()
        .amount = None;
    let input = reserve(&kernel, "missing-quantity", missing, 1);
    assert_eq!(
        kernel.preview_admission(&input).unwrap_err(),
        KernelError::Operational(OperationalError::UnknownResource(Resource::WorkerSlots))
    );
    assert!(kernel.domains().unwrap().domains().is_empty());
}

#[test]
fn admission_preview_rejects_invalidated_scope_and_preserves_unaffected_neighbor() {
    let (mut kernel, intent) = coordinator();
    let neighbor = setup(&mut kernel, "D");
    let root = kernel.state().readiness().scope(&task("A")).unwrap();
    let invalidation = notify(
        kernel.state(),
        "changed-support",
        Event::Invalidate {
            roots: vec![root],
            cause: source("material-support-change"),
            coverage: DependencyCoverage::DeclaredComplete,
        },
    );
    submit(&mut kernel, "invalidate", Command::Notify(invalidation));
    let old = reserve(&kernel, "stale-generation", intent, 1);
    assert_eq!(
        kernel.preview_admission(&old).unwrap_err(),
        KernelError::Operational(OperationalError::Program(ProgramError::Stale))
    );
    let independent = reserve(&kernel, "unaffected", neighbor, 1);
    assert!(
        !kernel
            .preview_admission(&independent)
            .unwrap()
            .is_duplicate()
    );
    assert!(kernel.domains().unwrap().domains().is_empty());
}

#[test]
fn admission_preview_preserves_unknown_debt_after_capacity_closure() {
    let (mut kernel, intent) = coordinator();
    bound(&mut kernel, &intent);
    event(
        &mut kernel,
        "close-without-usage",
        DomainEvent::Closed {
            receipt: closure(&intent, None),
            at: 4,
        },
    );
    let next = setup(&mut kernel, "D");
    let input = reserve(&kernel, "debt-held", next, 5);
    let revision = kernel.revision();
    assert_eq!(
        kernel.preview_admission(&input).unwrap_err(),
        KernelError::Domain(DomainError::Debt(Consumption::CpuExecutionMillis))
    );
    let retained = kernel.domains().unwrap().domain(&intent.id).unwrap();
    assert_eq!(retained.outcome, HistoricalOutcome::Unknown);
    assert!(!retained.holds(Resource::WorkerSlots));
    assert!(retained.holds(Resource::DiskBytes));
    assert!(retained.unresolved_consumption(Consumption::CpuExecutionMillis));
    assert_eq!(kernel.revision(), revision);
}

#[test]
fn admission_preview_refuses_incomplete_reconstruction_then_uses_current_guards() {
    let (kernel, intent) = coordinator();
    let mut retained = kernel.operations().iter().cloned();
    let mut observed_callbacks = 0;
    let rebuilt = CoordinationKernel::new_domain_fixture(kernel.initial().clone())
        .unwrap()
        .reconstruct::<KernelError>(|partial| {
            let input = reserve(partial, "new-reservation", intent.clone(), 1);
            assert_eq!(
                partial.preview_admission(&input).unwrap_err(),
                KernelError::Core(ProgramError::EffectUnavailable)
            );
            observed_callbacks += 1;
            Ok(retained.next())
        })
        .unwrap();
    assert_eq!(observed_callbacks, kernel.operations().len() + 1);
    assert!(rebuilt.operations() == kernel.operations());
    let input = reserve(&rebuilt, "new-reservation", intent, 1);
    assert!(!rebuilt.preview_admission(&input).unwrap().is_duplicate());
    assert!(rebuilt.domains().unwrap().domains().is_empty());
}
