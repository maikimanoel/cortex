//! Pure correspondence and owned marker checks: no fixture child or historical store.
#[path = "support/gpe_identity.rs"]
mod identity;
use identity::*;
use std::{fs, os::unix::fs::PermissionsExt};
fn ns() -> FixtureNamespace {
    FixtureNamespace::new("campaign-b-fixture-01").unwrap()
}
#[test]
fn fresh_namespace_produces_exact_bounded_labels() {
    assert_eq!(
        ns().qualify("attempt-A").unwrap(),
        "campaign-b-fixture-01-attempt-A"
    );
    assert!(ns().qualify(&"a".repeat(128)).is_err());
    assert!(ns().qualify("../foreign").is_err());
}
#[test]
fn invalid_namespaces_are_refused_before_effects() {
    for invalid in [
        "",
        "../a",
        "UPPER",
        "a/b",
        "1starts-with-digit",
        "a b",
        "a\n",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        assert!(FixtureNamespace::new(invalid).is_err(), "{invalid:?}");
    }
}
#[test]
fn duplicate_claim_is_not_retried_or_refunded() {
    let root = std::env::temp_dir().join(format!("cortex-gpe-identity-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    ns().claim(&root).unwrap();
    let before = fs::read(root.join("namespace-campaign-b-fixture-01")).unwrap();
    assert!(matches!(
        ns().claim(&root),
        Err(FixtureIdentityError::DuplicateNamespace)
    ));
    assert_eq!(
        fs::read(root.join("namespace-campaign-b-fixture-01")).unwrap(),
        before
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn broken_program_attempt_correspondence_is_rejected() {
    let n = ns();
    let p = n.qualify("gpe-func-2t").unwrap();
    let a = n.qualify("A").unwrap();
    let attempt = n.qualify("attempt-A").unwrap();
    let package = n.qualify("package-A").unwrap();
    n.check_attempt(&p, &a, &attempt, &package).unwrap();
    assert!(
        n.check_attempt("historical-program", &a, &attempt, &package)
            .is_err()
    );
    assert!(
        n.check_attempt(&p, &a, &n.qualify("attempt-B").unwrap(), &package)
            .is_err()
    );
}
#[test]
fn broken_operation_predecessor_is_rejected() {
    let n = ns();
    let id = n.qualify("claim-A").unwrap();
    n.check_link(&id, "claim-A", 7, 7).unwrap();
    assert!(n.check_link(&id, "claim-A", 6, 7).is_err());
    assert!(n.check_link("old-claim-A", "claim-A", 7, 7).is_err());
}
#[test]
fn inconsistent_package_and_release_references_are_rejected() {
    let n = ns();
    assert!(
        n.check_attempt(
            &n.qualify("gpe-func-2t").unwrap(),
            &n.qualify("A").unwrap(),
            &n.qualify("attempt-A").unwrap(),
            &n.qualify("package-B").unwrap()
        )
        .is_err()
    );
    n.check_release(
        &n.qualify("release-AB").unwrap(),
        &n.qualify("AB").unwrap(),
        &n.qualify("prime-list").unwrap(),
    )
    .unwrap();
    assert!(
        n.check_release(
            &n.qualify("release-AB").unwrap(),
            "AB",
            &n.qualify("prime-list").unwrap()
        )
        .is_err()
    );
}
#[test]
fn separate_fresh_fixtures_have_disjoint_corresponding_identities() {
    let a = ns();
    let b = FixtureNamespace::new("campaign-b-fixture-02").unwrap();
    for label in [
        "gpe-func-2t",
        "A",
        "B",
        "attempt-A",
        "attempt-B",
        "attempt-synthetic-unknown",
        "claim-A",
        "package-A",
        "release-AB",
    ] {
        assert_ne!(a.qualify(label).unwrap(), b.qualify(label).unwrap());
    }
    assert!(
        a.check_attempt(
            &b.qualify("gpe-func-2t").unwrap(),
            &a.qualify("A").unwrap(),
            &a.qualify("attempt-A").unwrap(),
            &a.qualify("package-A").unwrap()
        )
        .is_err()
    );
}

#[test]
fn non_unicode_namespace_is_rejected_without_legacy_fallback() {
    use std::os::unix::ffi::OsStringExt;
    let invalid = std::ffi::OsString::from_vec(vec![b'b', 0xff]);
    assert!(matches!(
        FixtureNamespace::from_os(&invalid),
        Err(FixtureIdentityError::InvalidNamespace)
    ));
}
