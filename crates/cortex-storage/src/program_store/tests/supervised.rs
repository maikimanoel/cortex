//! Profile separation and real encrypted admission/launch-boundary fault cases.
use super::fixture;
use super::*;
#[path = "../../../../cortex-evidence/tests/support/supervision.rs"]
mod supervised_fixture;
use cortex_core::program::{
    operational::{EffectEvent, EffectScope, EffectStatus},
    supervision::*,
};
use std::sync::atomic::AtomicUsize;

fn stored(f: &Temp) -> (ProgramStore, CoordinationKernel, EffectScope, ProcessSpec) {
    let (k, scope, spec) = supervised_fixture::prepared_kernel(None);
    let s =
        ProgramStore::create_supervised_fixture(&f.store(), &key(), k.initial().clone(), limits())
            .unwrap();
    for op in k.operations() {
        s.admit(&key(), op.clone()).unwrap();
    }
    (s, k, scope, spec)
}

#[test]
fn supervised_profile_roundtrips_encrypted_contracts_and_old_formats_refuse() {
    let f = Temp::new();
    let (s, mut k, scope, spec) = stored(&f);
    let dispatch = supervised_fixture::dispatch_operation(&k, &scope);
    s.admit(&key(), dispatch.clone()).unwrap();
    k.admit(dispatch).unwrap();
    for (n, completion, quiescence) in [
        (1, ProcessCompletion::Spawned, Quiescence::Unknown),
        (2, ProcessCompletion::Unknown, Quiescence::Unknown),
        (
            3,
            ProcessCompletion::Exited {
                code: Some(0),
                signal: None,
            },
            Quiescence::DirectChildReapedStreamsClosed,
        ),
    ] {
        let observation = supervised_fixture::observation(&spec, n, completion, quiescence);
        let op = operation(
            &k,
            &format!("observation-{n}"),
            Input::SupervisedEffect(Box::new(EffectEvent::ProcessObserved {
                effect: supervised_fixture::intent(&scope).id,
                scope: scope.clone(),
                at: 3 + n,
                observation,
            })),
        );
        s.admit(&key(), op.clone()).unwrap();
        k.admit(op).unwrap();
    }
    let reopened = ProgramStore::open(&f.store(), &key(), Access::ReadOnly, limits()).unwrap();
    assert_eq!(reopened.profile(), ProgramProfile::SupervisedFixturesV3);
    let actual = reopened.load(&key(), Access::ReadOnly).unwrap();
    assert!(actual.fixture_effects() == k.fixture_effects());
    assert_eq!(
        actual.fixture_effects().effects()[0]
            .process_observations
            .len(),
        3
    );
    assert!(actual.fixture_effects().effects()[0].pending()); // A report is not settlement.
    let bytes = fs::read(f.store().join("program.db")).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3"));
    for sentinel in [
        b"supervised-effect-v1".as_slice(),
        b"process-observed-v1",
        b"trusted-native-fixture",
    ] {
        assert!(!bytes.windows(sentinel.len()).any(|part| part == sentinel));
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
    for profile in [ProgramProfile::MetadataV1, ProgramProfile::FixturesV2] {
        let old = Temp::new();
        let old_store = ProgramStore::create_profile(
            &old.store(),
            &key(),
            k.initial().clone(),
            limits(),
            profile,
        )
        .unwrap();
        let before = snapshot(&old);
        let op = operation(
            &CoordinationKernel::new(k.initial().clone()).unwrap(),
            "forbidden-supervised-policy",
            Input::SupervisedEffect(Box::new(EffectEvent::Policy(supervised_fixture::policy()))),
        );
        let failure = old_store.admit(&key(), op).unwrap_err();
        assert_eq!(failure.completion, Completion::Rejected);
        assert!(matches!(failure.error, ProgramStoreError::Unsupported));
        assert_eq!(snapshot(&old), before);
    }
}

#[test]
fn dispatch_callback_runs_only_after_one_new_committed_intent() {
    let f = Temp::new();
    let (s, k, scope, _) = stored(&f);
    let input = supervised_fixture::dispatch_operation(&k, &scope);
    let launches = AtomicUsize::new(0);
    let (receipt, result) = s
        .dispatch_fixture(&key(), input.clone(), |draft| {
            assert_eq!(
                draft.fixture_effects().effects()[0].status,
                EffectStatus::DispatchUncertain
            );
            launches.fetch_add(1, Ordering::SeqCst);
            "fixture callback result"
        })
        .unwrap();
    assert!(!receipt.duplicate);
    assert_eq!(result, Some("fixture callback result"));
    let (duplicate, repeated) = s
        .dispatch_fixture(&key(), input.clone(), |_| {
            launches.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
    assert!(duplicate.duplicate);
    assert!(repeated.is_none());
    let loaded = s.load(&key(), Access::ReadOnly).unwrap();
    assert_eq!(
        loaded.fixture_effects().effects()[0].status,
        EffectStatus::DispatchUncertain
    );
    assert_eq!(launches.load(Ordering::SeqCst), 1);
    let mut changed = input;
    changed.id = OperationId::new("cannot-relaunch-with-new-id").unwrap();
    changed.expected = loaded.revision();
    assert!(
        s.dispatch_fixture(&key(), changed, |_| {
            launches.fetch_add(1, Ordering::SeqCst);
        })
        .is_err()
    );
    assert_eq!(launches.load(Ordering::SeqCst), 1);
}

#[test]
fn refused_or_panicking_dispatch_retains_exact_failure_boundary() {
    let f = Temp::new();
    let (s, k, scope, _) = stored(&f);
    let mut stale = supervised_fixture::dispatch_operation(&k, &scope);
    stale.expected = 0;
    let before = snapshot(&f);
    assert!(
        s.dispatch_fixture(&key(), stale, |_| panic!("refused callback ran"))
            .is_err()
    );
    assert_eq!(snapshot(&f), before);
    let dispatch = supervised_fixture::dispatch_operation(&k, &scope);
    let lost = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        s.dispatch_fixture(&key(), dispatch.clone(), |_| {
            panic!("controlled lost launch reply")
        })
        .unwrap();
    }));
    assert!(lost.is_err());
    assert!(matches!(
        s.reconcile(&key(), k.initial(), &dispatch, Access::ReadOnly)
            .unwrap(),
        Reconciliation::Present(_)
    ));
    assert_eq!(
        s.load(&key(), Access::ReadOnly)
            .unwrap()
            .fixture_effects()
            .effects()[0]
            .status,
        EffectStatus::DispatchUncertain
    );
    let (_, result) = s
        .dispatch_fixture(&key(), dispatch, |_| panic!("uncertain launch repeated"))
        .unwrap();
    assert!(result.is_none());
}

#[test]
fn concurrent_processes_share_one_fresh_dispatch_callback() {
    let f = Temp::new();
    let (s, _, _, _) = stored(&f);
    let mut a = child(&f, "supervised-race-one");
    let mut b = child(&f, "supervised-race-two");
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
        results.iter().filter(|r| r.as_str() == "launched").count(),
        1
    );
    assert_eq!(
        results.iter().filter(|r| r.as_str() == "duplicate").count(),
        1
    );
    assert_eq!(
        fs::read(f.0.join("launch-marker")).unwrap(),
        b"one callback\n"
    );
    assert_eq!(
        s.load(&key(), Access::ReadOnly)
            .unwrap()
            .fixture_effects()
            .effects()[0]
            .status,
        EffectStatus::DispatchUncertain
    );
}

#[test]
fn crashes_before_commit_after_commit_and_after_effect_never_replay_callback() {
    for mode in [
        "supervised-before",
        "supervised-after",
        "supervised-callback",
    ] {
        let f = Temp::new();
        let (s, k, scope, _) = stored(&f);
        let input = supervised_fixture::dispatch_operation(&k, &scope);
        let mut c = child(&f, mode);
        wait_file(&f.0.join("ready"), &mut c);
        c.kill().unwrap();
        assert!(!c.wait_with_output().unwrap().status.success());
        if s.location.journal_exists().unwrap() {
            let before = snapshot(&f);
            assert!(s.load(&key(), Access::ReadOnly).is_err());
            assert_eq!(snapshot(&f), before);
        }
        let reconciliation = s
            .reconcile(&key(), k.initial(), &input, Access::Recover)
            .unwrap();
        let actual = s.load(&key(), Access::Recover).unwrap();
        if mode == "supervised-before" {
            assert_eq!(reconciliation, Reconciliation::Absent);
            assert_eq!(
                actual.fixture_effects().effects()[0].status,
                EffectStatus::Reserved
            );
            assert!(!f.0.join("launch-marker").exists());
        } else {
            assert!(matches!(reconciliation, Reconciliation::Present(_)));
            assert_eq!(
                actual.fixture_effects().effects()[0].status,
                EffectStatus::DispatchUncertain
            );
            let (_, result) = s
                .dispatch_fixture(&key(), input, |_| panic!("restart repeated effect"))
                .unwrap();
            assert!(result.is_none());
            assert_eq!(
                f.0.join("launch-marker").exists(),
                mode == "supervised-callback"
            );
        }
    }
}

pub(super) fn process_child(root: &Path, s: &ProgramStore, key: &StoreKey, mode: &str) {
    let k = s.load(key, Access::ReadOnly).unwrap();
    let scope = k.fixture_effects().effects()[0].intent.scope.clone();
    let dispatch = supervised_fixture::dispatch_operation(&k, &scope);
    let await_file = |name: &str| {
        let start = Instant::now();
        while !root.join(name).exists() {
            assert!(start.elapsed() < Duration::from_secs(30));
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    if let Some(label) = mode.strip_prefix("supervised-race-") {
        fs::write(root.join(format!("ready-{label}")), b"ready").unwrap();
        await_file("go");
        let (_, result) = s
            .dispatch_fixture(key, dispatch, |_| {
                let mut file = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(root.join("launch-marker"))
                    .unwrap();
                file.write_all(b"one callback\n").unwrap();
            })
            .unwrap();
        fs::write(
            root.join(format!("result-{label}")),
            if result.is_some() {
                "launched"
            } else {
                "duplicate"
            },
        )
        .unwrap();
        return;
    }
    s.admit_coordinated(
        key,
        dispatch,
        |stage, _| {
            if matches!(
                (mode, stage),
                ("supervised-before", Stage::BeforeCommit)
                    | ("supervised-after", Stage::AfterCommit)
            ) {
                fs::write(root.join("ready"), b"boundary").unwrap();
                await_file("continue");
            }
        },
        |_| {
            fs::write(root.join("launch-marker"), b"one callback\n").unwrap();
            fs::write(root.join("ready"), b"effect-boundary").unwrap();
            await_file("continue");
        },
    )
    .unwrap();
}
