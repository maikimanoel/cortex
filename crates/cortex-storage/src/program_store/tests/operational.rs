//! Real encrypted fixture journal replay, competing clients and process crash cuts.
use super::fixture;
use super::*;
#[path = "../../../../cortex-evidence/tests/support/operational.rs"]
mod operational_fixture;
use cortex_core::program::operational::{EffectEvent, EffectScope, EffectStatus};

fn stored(f: &Temp) -> (ProgramStore, CoordinationKernel, EffectScope) {
    let (k, scope) = operational_fixture::fixture_kernel();
    let store =
        ProgramStore::create_fixture(&f.store(), &key(), k.initial().clone(), limits()).unwrap();
    for op in k.operations() {
        store.admit(&key(), op.clone()).unwrap();
    }
    (store, k, scope)
}
fn reserve(k: &CoordinationKernel, scope: &EffectScope, id: &str) -> Operation {
    operation(
        k,
        id,
        Input::FixtureEffect(Box::new(EffectEvent::Reserve {
            intent: operational_fixture::intent(scope, id),
            at: 1,
        })),
    )
}
fn dispatch(k: &CoordinationKernel) -> Operation {
    let intent = &k.fixture_effects().effects()[0].intent;
    operation(
        k,
        "dispatch-boundary",
        Input::FixtureEffect(Box::new(EffectEvent::Dispatch {
            effect: intent.id.clone(),
            scope: intent.scope.clone(),
            at: 2,
            basis: source("fixture-dispatch-boundary"),
        })),
    )
}

#[test]
fn explicit_fixture_format_preserves_encryption_and_original_profile_refuses_inputs() {
    let f = Temp::new();
    let (store, mut k, scope) = stored(&f);
    let input = reserve(&k, &scope, "effect");
    store.admit(&key(), input.clone()).unwrap();
    k.admit(input).unwrap();
    let prepared = operation(
        &k,
        "prepare",
        Input::PrepareExecutorInput(Box::new(operational_fixture::preparation(&scope))),
    );
    store.admit(&key(), prepared.clone()).unwrap();
    k.admit(prepared).unwrap();
    let reopened = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    assert_eq!(reopened.profile(), ProgramProfile::FixturesV2);
    let actual = reopened.load(&key(), Access::ReadOnly).unwrap();
    assert!(actual.fixture_effects() == k.fixture_effects());
    assert!(actual.prepared_inputs() == k.prepared_inputs());
    let bytes = fs::read(f.store().join("program.db")).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3"));
    for text in [
        b"fixture-policy-v1".as_slice(),
        b"fixture-grant",
        b"Controlled fixture observation only",
    ] {
        assert!(!bytes.windows(text.len()).any(|w| w == text));
    }
    assert!(
        ProgramStore::open(
            &f.store(),
            &StoreKey::new(&[0x42; 32]).unwrap(),
            Access::ReadOnly,
            limits()
        )
        .is_err()
    );
    let old = Temp::new();
    let original = create(&old);
    let before = snapshot(&old);
    let failure = original
        .admit(
            &key(),
            operation(
                &CoordinationKernel::new(definition()).unwrap(),
                "new-input",
                Input::FixtureEffect(Box::new(EffectEvent::Policy(operational_fixture::policy()))),
            ),
        )
        .unwrap_err();
    assert_eq!(failure.completion, Completion::Rejected);
    assert!(matches!(failure.error, ProgramStoreError::Unsupported));
    assert_eq!(snapshot(&old), before);
    assert_eq!(original.profile(), ProgramProfile::MetadataV1);
}

#[test]
fn unknown_outcome_reopen_and_exact_reconciliation_do_not_repeat_dispatch() {
    let f = Temp::new();
    let (store, k, s) = stored(&f);
    store.admit(&key(), reserve(&k, &s, "effect")).unwrap();
    let k = store.load(&key(), Access::ReadOnly).unwrap();
    let input = dispatch(&k);
    store.admit(&key(), input.clone()).unwrap(); // Deliberately discard the first receipt.
    let reopened = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    assert!(matches!(
        reopened
            .reconcile(&key(), k.initial(), &input, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Present(_)
    ));
    assert!(reopened.admit(&key(), input.clone()).unwrap().duplicate);
    let projected = reopened.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(
        projected.fixture_effects().effects()[0].status,
        EffectStatus::DispatchUncertain
    );
    let mut replayed = input;
    replayed.id = OperationId::new("new-dispatch-id").unwrap();
    replayed.expected = projected.revision();
    assert!(reopened.admit(&key(), replayed).is_err());
    assert_eq!(
        reopened.load(&key(), Access::ReadOnly).unwrap().revision(),
        projected.revision()
    );
}

#[test]
fn fixture_dispatch_crash_cuts_preserve_uncertainty_and_exact_commit_reconciliation() {
    for mode in ["effect-before", "effect-during", "effect-after"] {
        let f = Temp::new();
        let (store, k, s) = stored(&f);
        store.admit(&key(), reserve(&k, &s, "effect")).unwrap();
        let k = store.load(&key(), Access::ReadOnly).unwrap();
        let input = dispatch(&k);
        let mut c = child(&f, mode);
        wait_file(&f.0.join("ready"), &mut c);
        c.kill().unwrap();
        assert!(!c.wait_with_output().unwrap().status.success());
        if store.location.journal_exists().unwrap() {
            let before = snapshot(&f);
            assert!(store.load(&key(), Access::ReadOnly).is_err());
            assert_eq!(snapshot(&f), before);
        }
        let receipt = store
            .reconcile(&key(), k.initial(), &input, Access::Recover)
            .unwrap();
        let actual = store.load(&key(), Access::Recover).unwrap();
        if mode == "effect-after" {
            assert!(matches!(receipt, Reconciliation::Present(_)));
            assert_eq!(
                actual.fixture_effects().effects()[0].status,
                EffectStatus::DispatchUncertain
            );
            assert!(store.admit(&key(), input).unwrap().duplicate);
        } else {
            assert_eq!(receipt, Reconciliation::Absent);
            assert_eq!(
                actual.fixture_effects().effects()[0].status,
                EffectStatus::Reserved
            );
        }
    }
}

#[test]
fn separate_process_intents_cannot_admit_conflicting_effects() {
    let f = Temp::new();
    let (store, _, _) = stored(&f);
    let mut a = child(&f, "effect-race-one");
    let mut b = child(&f, "effect-race-two");
    wait_file(&f.0.join("ready-one"), &mut a);
    wait_file(&f.0.join("ready-two"), &mut b);
    fs::write(f.0.join("go"), b"go").unwrap();
    wait_child(a);
    wait_child(b);
    let results = [
        fs::read_to_string(f.0.join("result-one")).unwrap(),
        fs::read_to_string(f.0.join("result-two")).unwrap(),
    ];
    assert_eq!(
        results.iter().filter(|s| s.as_str() == "committed").count(),
        1
    );
    let actual = store.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(actual.fixture_effects().effects().len(), 1);
    assert_eq!(
        actual.fixture_effects().effects()[0].status,
        EffectStatus::Reserved
    );
}

pub(super) fn process_child(root: &Path, store: &ProgramStore, key: &StoreKey, mode: &str) {
    let k = store.load(key, Access::ReadOnly).unwrap();
    if let Some(label) = mode.strip_prefix("effect-race-") {
        let scope = k.fixture_effects().grants()[0].scope.clone();
        let input = reserve(&k, &scope, label);
        fs::write(root.join(format!("ready-{label}")), b"ready").unwrap();
        let start = Instant::now();
        while !root.join("go").exists() {
            assert!(start.elapsed() < Duration::from_secs(20));
            std::thread::sleep(Duration::from_millis(5));
        }
        fs::write(
            root.join(format!("result-{label}")),
            if store.admit(key, input).is_ok() {
                "committed"
            } else {
                "rejected"
            },
        )
        .unwrap();
        return;
    }
    store
        .admit_observed(key, dispatch(&k), |stage, conn| {
            if matches!(stage, Stage::BeforeWrite) {
                conn.execute_batch("PRAGMA cache_size=1").unwrap();
            }
            if matches!(
                (mode, stage),
                ("effect-before", Stage::BeforeWrite)
                    | ("effect-during", Stage::BeforeCommit)
                    | ("effect-after", Stage::AfterCommit)
            ) {
                fs::write(root.join("ready"), b"boundary").unwrap();
                let start = Instant::now();
                while !root.join("continue").exists() {
                    assert!(start.elapsed() < Duration::from_secs(30));
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        })
        .unwrap();
}
