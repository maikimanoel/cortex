//! MI-01..04/17..19/22..25: public immutable-definition contracts, no method execution.
use cortex_core::{TaskId, binding::ContractRevision, methods::*, temporal::SourceReference};

fn limits() -> DefinitionLimits {
    DefinitionLimits::new(16, 64, 128, 2048, 1_000_000).unwrap()
}
fn source() -> SourceReference {
    SourceReference::operator_reported("a", "s", "1").unwrap()
}
fn reference(name: &str, revision: u64) -> MethodReference {
    MethodReference::new(
        MethodId::new(name).unwrap(),
        ContractRevision::new(revision).unwrap(),
    )
}
fn minimal() -> MethodDefinition {
    MethodDefinition::new(
        reference("m", 1),
        EngineeringProperty::new(PropertyId::new("p").unwrap(), "o", "v", vec![]).unwrap(),
        MethodMechanism::new(source(), "x", "y", "z").unwrap(),
        vec![
            MethodClaim::new(
                ClaimId::new("c").unwrap(),
                "s",
                "x",
                vec![
                    QualificationCase::new(
                        CaseId::new("k").unwrap(),
                        CaseRole::Violation,
                        source(),
                        "e",
                        source(),
                    )
                    .unwrap(),
                ],
                vec![],
                vec![],
            )
            .unwrap(),
        ],
        vec![],
        MethodContext::new(
            TargetScope::new(
                TaskId::new("t").unwrap(),
                ContractRevision::new(1).unwrap(),
                ScopeKind::Component,
                vec![],
                None,
                None,
                None,
            )
            .unwrap(),
            ApplicabilityConditions::new(PropertyRelevance::CrossCutting, "t", "a").unwrap(),
            vec![],
            vec![],
            source(),
            vec![],
        ),
    )
    .unwrap()
}
fn rebuild(
    d: &MethodDefinition,
    r: MethodReference,
    property: EngineeringProperty,
) -> MethodDefinition {
    MethodDefinition::new(
        r,
        property,
        d.mechanism().clone(),
        d.claims().to_vec(),
        d.sources().to_vec(),
        d.context().clone(),
    )
    .unwrap()
}
#[test]
fn mi01_exact_starter_lookup_is_immutable_and_missing_revision_is_not_latest() {
    let set = starter_methods(limits()).unwrap();
    assert_eq!(set.definitions().len(), 2);
    let r = reference("native.candidate-correspondence", 1);
    let d = set.definition(&r).unwrap();
    assert_eq!(
        d.property().id().as_str(),
        "engineering.candidate-correspondence"
    );
    assert_eq!(d.claims()[0].cases().len(), 4);
    assert_eq!(d.reference(), &r);
    assert!(std::ptr::eq(d, set.definition(&r).unwrap()));
    assert_eq!(
        set.definition(&reference("native.candidate-correspondence", 2)),
        Err(MethodDefinitionError::MissingReference)
    );
    assert_eq!(d.claims()[0].cases()[1].id().as_str(), "detached");
    assert_eq!(
        d.claims()[0].cases()[1].role(),
        CaseRole::LegitimateNeighbor
    );
    assert!(
        d.mechanism()
            .limitations()
            .contains("no authenticated capture")
    );
}
#[test]
fn mi02_alias_refusal_and_valid_new_revision() {
    let d = minimal();
    let changed = rebuild(
        &d,
        d.reference().clone(),
        EngineeringProperty::new(PropertyId::new("p").unwrap(), "changed", "v", vec![]).unwrap(),
    );
    assert_eq!(
        MethodSet::new(vec![d.clone(), changed], limits()),
        Err(MethodDefinitionError::IdentityConflict)
    );
    let revised = rebuild(&d, reference("m", 2), d.property().clone());
    let set = MethodSet::new(vec![d, revised], limits()).unwrap();
    assert_eq!(set.definitions().len(), 2);
    assert!(ContractRevision::new(0).is_err());
    for id in ["", " aliased", "a/b", "a\n", "☃"] {
        assert!(MethodId::new(id).is_err());
        assert!(CaseId::new(id).is_err());
    }
}
#[test]
fn mi03_mi04_shared_property_does_not_mandate_all_techniques() {
    let d = minimal();
    let other = rebuild(&d, reference("another-technique", 1), d.property().clone());
    let set = MethodSet::new(vec![d, other], limits()).unwrap();
    assert_ne!(
        set.definitions()[0].reference(),
        set.definitions()[1].reference()
    );
    assert_eq!(
        set.definitions()[0].property(),
        set.definitions()[1].property()
    );
    assert_eq!(
        set.definition(&reference("m", 1))
            .unwrap()
            .context()
            .applicability()
            .relevance(),
        PropertyRelevance::CrossCutting
    );
    assert!(
        set.definition(&reference("m", 1))
            .unwrap()
            .context()
            .requirements()
            .is_empty()
    );
}
#[test]
fn mi17_mi18_mi19_sources_preserve_overlap_conflict_and_unavailable_material() {
    let d = minimal();
    let one = SourceReference::operator_reported("first", "original", "1").unwrap();
    let two = SourceReference::operator_reported("second", "synthesis", "1").unwrap();
    let claim = MethodClaim::new(
        ClaimId::new("c").unwrap(),
        "s",
        "unsupported prevalence",
        d.claims()[0].cases().to_vec(),
        vec![one.clone()],
        vec![one.clone(), two.clone()],
    )
    .unwrap();
    let entries = vec![
        SourceEntry::new(
            one.clone(),
            SourceAvailability::Unavailable,
            SourceKind::Original,
            "original section",
            "not supplied",
        )
        .unwrap(),
        SourceEntry::new(
            two.clone(),
            SourceAvailability::SummaryOnly,
            SourceKind::Synthesis,
            "excerpt",
            "disagrees",
        )
        .unwrap(),
    ];
    let m = MethodDefinition::new(
        d.reference().clone(),
        d.property().clone(),
        d.mechanism().clone(),
        vec![claim],
        entries,
        d.context().clone(),
    )
    .unwrap();
    let set = MethodSet::new(vec![m], limits()).unwrap();
    let m = set.definition(d.reference()).unwrap();
    assert_eq!(m.sources().len(), 2);
    assert_eq!(m.claims()[0].conflicts(), [one, two]);
    assert_eq!(
        m.sources()[0].availability(),
        SourceAvailability::Unavailable
    );
    assert_eq!(m.sources()[1].kind(), SourceKind::Synthesis);
    assert_eq!(m.claims()[0].exclusions(), "unsupported prevalence");
}
#[test]
fn mi22_mi23_mixed_interface_preserves_unknown_facts_and_exception_declarations() {
    let scope = TargetScope::new(
        TaskId::new("identifier-boundary").unwrap(),
        ContractRevision::new(1).unwrap(),
        ScopeKind::Interface,
        vec![
            ComponentScope::new(
                TaskId::new("producer").unwrap(),
                Some("Rust"),
                Some("serde_json"),
                None,
                vec![source()],
            )
            .unwrap(),
            ComponentScope::new(
                TaskId::new("consumer").unwrap(),
                Some("JavaScript"),
                None,
                None,
                vec![],
            )
            .unwrap(),
        ],
        None,
        Some("9007199254740993 must retain identifier identity"),
        None,
    )
    .unwrap();
    let context = MethodContext::new(
        scope,
        ApplicabilityConditions::new(
            PropertyRelevance::Contextual,
            "components exchange numeric identifiers",
            "inspected absence of interface",
        )
        .unwrap(),
        vec![source()],
        vec![source()],
        source(),
        vec![source()],
    );
    assert_eq!(context.target().kind(), ScopeKind::Interface);
    assert_eq!(
        context.target().components()[1].language(),
        Some("JavaScript")
    );
    assert_eq!(context.target().components()[1].version(), None);
    assert_eq!(context.requirements().len(), 1);
    assert_eq!(context.exceptions().len(), 1);
    assert_eq!(context.dependencies().len(), 1);
}
#[test]
fn mi25_exact_independent_accounting_and_every_limit_neighbor() {
    let d = minimal();
    let u = d.input_usage();
    // Literal independent count of the one-byte fixture above: 27 text bytes,
    // one method/claim/case and four SourceReference occurrences.
    for (k, n) in [
        (DefinitionLimitKind::Definitions, 1),
        (DefinitionLimitKind::Claims, 1),
        (DefinitionLimitKind::Cases, 1),
        (DefinitionLimitKind::References, 4),
        (DefinitionLimitKind::MaterialBytes, 27),
    ] {
        assert_eq!(u.get(k), n);
    }
    assert!(
        MethodSet::new(
            vec![d.clone()],
            DefinitionLimits::new(1, 1, 1, 4, 27).unwrap()
        )
        .is_ok()
    );
    let exact = [2, 2, 2, 8, 54];
    let kinds = [
        DefinitionLimitKind::Definitions,
        DefinitionLimitKind::Claims,
        DefinitionLimitKind::Cases,
        DefinitionLimitKind::References,
        DefinitionLimitKind::MaterialBytes,
    ];
    let set = MethodSet::new(
        vec![d.clone(), d.clone()],
        DefinitionLimits::new(2, 2, 2, 8, 54).unwrap(),
    )
    .unwrap();
    assert_eq!(set.definitions().len(), 1);
    assert_eq!(set.input_usage().get(DefinitionLimitKind::Definitions), 2);
    for (i, k) in kinds.into_iter().enumerate() {
        let mut v = exact;
        v[i] -= 1;
        let l = DefinitionLimits::new(v[0], v[1], v[2], v[3], v[4]).unwrap();
        assert_eq!(
            MethodSet::new(vec![d.clone(), d.clone()], l),
            Err(MethodDefinitionError::Limit(k))
        );
    }
    for i in 0..5 {
        let mut v = [1; 5];
        v[i] = 0;
        assert_eq!(
            DefinitionLimits::new(v[0], v[1], v[2], v[3], v[4]),
            Err(MethodDefinitionError::InvalidLimits)
        );
        v[i] = usize::MAX;
        assert_eq!(
            DefinitionLimits::new(v[0], v[1], v[2], v[3], v[4]),
            Err(MethodDefinitionError::InvalidLimits)
        );
    }
}
#[test]
fn mi25_private_debug_text_boundaries_and_ambiguous_cases() {
    let secret = "private-marker";
    let property = EngineeringProperty::new(
        PropertyId::new(secret).unwrap(),
        secret,
        secret,
        vec![source()],
    )
    .unwrap();
    assert!(!format!("{property:?}").contains(secret));
    assert_eq!(property.objective(), secret);
    assert!(
        EngineeringProperty::new(
            PropertyId::new("p").unwrap(),
            &"x".repeat(4096),
            "v",
            vec![]
        )
        .is_ok()
    );
    let error = EngineeringProperty::new(
        PropertyId::new("p").unwrap(),
        &"x".repeat(4097),
        "v",
        vec![],
    )
    .unwrap_err();
    assert!(!format!("{error:?} {error}").contains("xxxxx"));
    let c = minimal().claims()[0].cases()[0].clone();
    assert_eq!(
        MethodClaim::new(
            ClaimId::new("c").unwrap(),
            "s",
            "x",
            vec![c.clone(), c],
            vec![],
            vec![]
        ),
        Err(MethodDefinitionError::IdentityConflict)
    );
}
