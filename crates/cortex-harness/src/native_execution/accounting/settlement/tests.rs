//! SOURCE_SIMULATION. Counter and authenticated observer inputs here are
//! controlled; the encrypted journal and reducer are the production owners.

use super::*;
use crate::{
    domain_runtime::tests::fixture,
    native_execution::accounting::{ManagedCpuCut, ManagedCpuReadback},
};
use cortex_core::program::domain::{DomainIdentity, DomainPhase};
use cortex_storage::program_store::ReadLimits;
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf, time::Duration};

struct Fixture {
    temp: PathBuf,
    store: ProgramStore,
    key: StoreKey,
    intent: DomainIntent,
    identity: DomainIdentity,
    start: Instant,
}

impl Fixture {
    fn new(case: &str, cancelled: bool) -> Self {
        let (mut kernel, intent) = fixture::coordinator();
        let identity = fixture::identity(&intent);
        for (id, event) in [
            (
                "reserve",
                DomainEvent::Reserve {
                    intent: intent.clone(),
                    at: 1,
                },
            ),
            (
                "create",
                DomainEvent::CreateRequested {
                    domain: intent.id.clone(),
                    at: 2,
                },
            ),
            (
                "bind",
                DomainEvent::BoundObserved {
                    domain: intent.id.clone(),
                    observation: fixture::binding(&intent),
                    at: 3,
                },
            ),
            (
                "release",
                DomainEvent::ReleaseRequested {
                    domain: intent.id.clone(),
                    identity: identity.clone(),
                    at: 4,
                },
            ),
        ] {
            fixture::event(&mut kernel, id, event);
        }
        let event = if cancelled {
            DomainEvent::Cancel {
                domain: intent.id.clone(),
                at: 5,
                basis: fixture::source("source-simulation-cancel"),
            }
        } else {
            DomainEvent::Report {
                domain: intent.id.clone(),
                identity: identity.clone(),
                outcome: EffectOutcome::Succeeded,
                at: 5,
                basis: fixture::source("source-simulation-report"),
            }
        };
        fixture::event(&mut kernel, "fixture-finish", event);

        let temp = std::env::temp_dir().join(format!(
            "cortex-collected-cpu-{}-{case}",
            std::process::id()
        ));
        fs::DirBuilder::new().mode(0o700).create(&temp).unwrap();
        let key = StoreKey::new(&[41; 32]).unwrap();
        let store = ProgramStore::create_domain_fixture(
            &temp.join("store"),
            &key,
            kernel.initial().clone(),
            ReadLimits::new(16_000_000, Duration::from_secs(5)).unwrap(),
        )
        .unwrap();
        for operation in kernel.operations() {
            store.admit(&key, operation.clone()).unwrap();
        }
        Self {
            temp,
            store,
            key,
            intent,
            identity,
            start: Instant::now(),
        }
    }

    fn sample(&self, cut: ManagedCpuCut, offset: u64) -> ManagedCpuReadback {
        ManagedCpuReadback {
            cut,
            began_at: self.start + Duration::from_millis(offset),
            observed_at: self.start + Duration::from_millis(offset + 1),
            monotonic_ns: Some((offset * 100, offset * 100 + 50)),
            populated: Some(false),
            counter: CpuCounterObservation::Unavailable,
        }
    }

    fn history(&self) -> ManagedCpuObservations {
        let first = ManagedCpuReadback {
            populated: Some(true),
            counter: CpuCounterObservation::LowerBoundMicros(1500),
            ..self.sample(ManagedCpuCut::FirstHeld, 2)
        };
        ManagedCpuObservations::new(
            self.store.id(),
            self.intent.id.clone(),
            self.identity.clone(),
            self.start,
            Some(0),
            self.start + Duration::from_secs(60),
            first,
        )
    }

    fn load(&self) -> cortex_evidence::program::CoordinationKernel {
        self.store.load(&self.key, Access::ReadOnly).unwrap()
    }

    fn request(
        &self,
        id: &str,
        action: CollectedCpuClosureAction,
        cut: CollectedCpuClosureCut,
    ) -> CollectedCpuClosure {
        let kernel = self.load();
        let prototype = fixture::domain_operation(
            &kernel,
            id,
            Command::Domain(Box::new(DomainEvent::Closed {
                receipt: fixture::closure(&self.intent, None),
                at: 7,
            })),
        );
        CollectedCpuClosure {
            operation: prototype.id,
            expected_revision: prototype.expected,
            at: 7,
            grant: prototype.grant,
            action,
            cut,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temp);
    }
}

fn report(history: &ManagedCpuObservations, request: &CollectedCpuClosure) -> Vec<u8> {
    let binding = challenge(history, request).unwrap();
    let observer_scope = format!(
        "controlled-cpu-{:x}",
        Sha256::digest(history.domain().as_str().as_bytes())
    );
    serde_json::to_vec(&json!([
        "whole-domain-cpu-fence-v1",
        binding,
        100,
        0,
        300,
        450,
        observer_scope,
        1
    ]))
    .unwrap()
}

/// Repeatable controlled collector and observer input for the existing B/E/F
/// managed source scenario. The resulting closure goes to the real store.
pub(crate) fn source_cpu_attempt<'a>(
    store: &'a ProgramStore,
    key: &'a StoreKey,
    intent: &'a DomainIntent,
    request: CollectedCpuClosure,
    micros: u64,
) -> impl Fn() -> Result<AdmissionReceipt, CollectedCpuClosureError> + 'a {
    let start = Instant::now();
    let identity = fixture::identity(intent);
    let mut history = ManagedCpuObservations::new(
        store.id(),
        intent.id.clone(),
        identity.clone(),
        start,
        Some(0),
        start + Duration::from_secs(60),
        ManagedCpuReadback {
            cut: ManagedCpuCut::FirstHeld,
            began_at: start,
            observed_at: start,
            monotonic_ns: Some((200, 250)),
            populated: Some(true),
            counter: CpuCounterObservation::LowerBoundMicros(micros.min(1)),
        },
    );
    let cut = match request.cut {
        CollectedCpuClosureCut::NormalEnd => ManagedCpuCut::EndOfTransport,
        CollectedCpuClosureCut::AfterCancellation => ManagedCpuCut::AfterCancellation,
    };
    history.record_bracket(
        &identity,
        cut,
        ManagedCpuReadback {
            cut,
            began_at: start,
            observed_at: start,
            monotonic_ns: Some((400, 450)),
            populated: Some(false),
            counter: CpuCounterObservation::Unavailable,
        },
        (Some(micros), Some(micros)),
    );
    let bytes = report(&history, &request);
    move || {
        admit(
            store,
            key,
            &history,
            intent,
            request.clone(),
            &bytes,
            || Ok(()),
        )
    }
}

#[test]
fn collected_cpu_survives_store_reopen_and_does_not_release_occupancy() {
    use CollectedCpuClosureAction::Close;
    use CollectedCpuClosureCut::NormalEnd;
    let f = Fixture::new("normal", false);
    let mut history = f.history();
    history.record_bracket(
        &f.identity,
        ManagedCpuCut::EndOfTransport,
        f.sample(ManagedCpuCut::EndOfTransport, 4),
        (Some(2001), Some(2001)),
    );
    let request = f.request("collected-normal-close", Close, NormalEnd);
    let bytes = report(&history, &request);
    let ordinal = request.expected_revision;
    assert!(matches!(
        admit(
            &f.store,
            &f.key,
            &history,
            &f.intent,
            f.request("other-close", Close, NormalEnd),
            &bytes,
            || Ok(()),
        ),
        Err(CollectedCpuClosureError::Observer)
    ));
    assert_eq!(f.load().revision(), ordinal);
    assert!(matches!(
        admit(
            &f.store,
            &f.key,
            &history,
            &f.intent,
            f.request("collected-normal-close", Close, NormalEnd),
            &bytes,
            || Err(CollectedCpuClosureError::Observer),
        ),
        Err(CollectedCpuClosureError::Observer)
    ));
    assert_eq!(f.load().revision(), ordinal);
    let repeated = CollectedCpuClosure {
        operation: request.operation.clone(),
        grant: request.grant.clone(),
        ..request
    };
    let confirmed = admit(
        &f.store,
        &f.key,
        &history,
        &f.intent,
        request,
        &bytes,
        || Ok(()),
    )
    .unwrap();
    assert_eq!(confirmed.ordinal, ordinal + 1);
    assert!(
        admit(
            &f.store,
            &f.key,
            &history,
            &f.intent,
            repeated,
            &bytes,
            || Ok(())
        )
        .unwrap()
        .duplicate
    );
    let reopened_store = ProgramStore::open(
        &f.temp.join("store"),
        &f.key,
        Access::ReadOnly,
        f.store.read_limits(),
    )
    .unwrap();
    assert_eq!(reopened_store.id(), confirmed.store);
    let reopened = reopened_store.load(&f.key, Access::ReadOnly).unwrap();
    let record = reopened.domains().unwrap().domain(&f.intent.id).unwrap();
    assert_eq!(record.phase, DomainPhase::Closed);
    let retained = record.closure.as_ref().unwrap();
    assert!(retained.released.is_empty());
    assert_eq!(retained.consumption[0].measured, Some(3));
    assert_eq!(reopened.revision(), ordinal + 1);
}

#[test]
fn collected_cpu_rejects_wrong_epoch_reset_gap_and_growth_in_empty_tree() {
    use CpuCounterObservation::*;
    let f = Fixture::new("negative", false);
    let mut history = f.history();
    let mut replaced = f.identity.clone();
    replaced.cgroup_inode += 1;
    let end = f.sample(ManagedCpuCut::EndOfTransport, 4);
    assert_eq!(
        history.record_bracket(&replaced, end.cut, end, (Some(1600), Some(1600))),
        Contradictory
    );
    let mut history = f.history();
    assert_eq!(
        history.record_bracket(&f.identity, end.cut, end, (Some(100), Some(2001))),
        Contradictory
    );
    let cancel_cut = f.sample(ManagedCpuCut::AfterCancellation, 6);
    assert_eq!(
        history.record_bracket(
            &f.identity,
            cancel_cut.cut,
            cancel_cut,
            (Some(2001), Some(2001))
        ),
        Contradictory
    );
    let mut history = f.history();
    assert_eq!(
        history.record_bracket(&f.identity, end.cut, end, (Some(1500), Some(2001))),
        Contradictory
    );
    let mut history = f.history();
    assert_eq!(
        history.record_bracket(&f.identity, end.cut, end, (None, Some(2001))),
        Unavailable
    );
    assert!(history.incomplete());
    assert!(matches!(
        challenge(
            &history,
            &f.request(
                "not-admissible",
                CollectedCpuClosureAction::Close,
                CollectedCpuClosureCut::NormalEnd
            )
        ),
        Err(CollectedCpuClosureError::Counter)
    ));
}

#[test]
fn post_cancellation_collection_settles_cpu_with_historical_unknown_intact() {
    use CollectedCpuClosureAction::Settle;
    use CollectedCpuClosureCut::AfterCancellation;
    let f = Fixture::new("after-cancel", true);
    let mut history = f.history();
    let cancel_end = f.sample(ManagedCpuCut::AfterCancellation, 4);
    history.record_bracket(
        &f.identity,
        cancel_end.cut,
        cancel_end,
        (Some(2700), Some(2700)),
    );
    let k = f.load();
    let operation = fixture::domain_operation(
        &k,
        "externally-reviewed-quiescence",
        Command::Domain(Box::new(DomainEvent::Closed {
            receipt: fixture::closure(&f.intent, None),
            at: 6,
        })),
    );
    f.store.admit(&f.key, operation).unwrap();
    let request = f.request("collected-cancel-settle", Settle, AfterCancellation);
    let bytes = report(&history, &request);
    let changed = CollectedCpuClosure {
        expected_revision: request.expected_revision - 1,
        operation: request.operation.clone(),
        grant: request.grant.clone(),
        ..request
    };
    assert!(matches!(
        admit(
            &f.store,
            &f.key,
            &history,
            &f.intent,
            changed,
            &bytes,
            || Ok(())
        ),
        Err(CollectedCpuClosureError::Observer)
    ));
    assert!(
        admit(
            &f.store,
            &f.key,
            &history,
            &f.intent,
            request,
            &bytes,
            || Ok(())
        )
        .is_ok()
    );
    let reopened = f.load();
    let record = reopened.domains().unwrap().domain(&f.intent.id).unwrap();
    assert_eq!(record.outcome, HistoricalOutcome::Unknown);
    assert_eq!(record.settlements.len(), 1);
    assert_eq!(record.settlements[0].consumption[0].measured, Some(3));
}
