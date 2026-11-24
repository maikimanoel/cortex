//! Exact, finite local trial admission over the selected protected trust path.
use super::super::accounting::clock_namespace;
use super::{
    P1Refusal, RawCopyP1Binding, live_budget,
    trust::{Role, TrialTrust, TrustedChannel},
};
use crate::{
    domain_manager::{DomainCause, DomainDiagnostic, DomainPhaseDiagnostic, issue},
    domain_runtime::{DomainTrialGuard, TrialEffectCut},
};
use cortex_core::{
    CandidateId,
    program::domain::{DomainEvent, DomainIntent},
    temporal::{SourceOrigin, SourceReference},
};
use cortex_evidence::program::{Command, CoordinationKernel, Operation};
use cortex_storage::program_store::ProgramStore;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    os::{fd::OwnedFd, unix::ffi::OsStrExt},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

/// Channels and fixed external limits for one raw-copy trial.
///
/// Each channel is a separately connected local Unix sequenced-packet socket,
/// with close-on-exec and the configured kernel-verified peer. The runner sends
/// an exact trial request before Reserve, Create, and Release. Each peer must
/// compare the requested bytes with its independent approved context before
/// replying. No profile values or assertions on behalf of either peer are
/// accepted from this request.
pub struct RawCopyP1Authority {
    /// Held endpoint connected to the configured owner decision process.
    pub operator: OwnedFd,
    /// Held endpoint connected to the separately configured observer.
    pub observer: OwnedFd,
    /// Immutable context both peers independently review.
    pub scope: RawCopyP1AuthorityScope,
}

/// Shared pre-effect context for the exact locally reviewed trial.
#[derive(Clone)]
pub struct RawCopyP1AuthorityScope {
    /// Exact Git tree reviewed independently for this trial.
    pub tree: CandidateId,
    /// Exact time-namespace device and inode independently checked by the peers.
    pub clock_namespace: (u64, u64),
    /// Original Linux CLOCK_MONOTONIC deadline in that namespace, in microseconds.
    pub expires_before_micros: u64,
    /// Exact independent outer containment context to be checked by both peers.
    pub containment_sha256: [u8; 32],
    /// Exact independently approved recovery and stop procedure.
    pub stop_plan_sha256: [u8; 32],
}

pub(super) struct AuthorizedTrial<'a> {
    trust: TrialTrust,
    operator: TrustedChannel,
    observer: TrustedChannel,
    store: &'a ProgramStore,
    binding: &'a RawCopyP1Binding<'a>,
    cancel: &'a AtomicBool,
    expires_before_micros: u64,
    clock_namespace: (u64, u64),
    context: [u8; 32],
    pending: Cell<Option<[u8; 32]>>,
}

fn now_micros() -> Option<u64> {
    use rustix::time::{ClockId, clock_gettime};
    let now = clock_gettime(ClockId::Monotonic);
    u64::try_from(now.tv_sec)
        .ok()?
        .checked_mul(1_000_000)?
        .checked_add(u64::try_from(now.tv_nsec).ok()?.div_ceil(1_000))
}

fn conservative_end(
    anchor: Instant,
    monotonic_micros: u64,
    expires_before_micros: u64,
) -> Option<Instant> {
    let remaining = expires_before_micros.checked_sub(monotonic_micros)?;
    let remaining = Duration::from_micros(remaining);
    if remaining.is_zero() || remaining > super::MAX_CASE_INTERVAL {
        return None;
    }
    anchor.checked_add(remaining)
}

fn hex(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut value = String::with_capacity(64);
    for b in bytes {
        write!(&mut value, "{b:02x}").expect("write to string");
    }
    value
}

fn reference(value: &SourceReference) -> Value {
    let origin = match value.origin() {
        SourceOrigin::OperatorReported { author } => json!(["operator", author]),
        SourceOrigin::ObservationReference(id) => json!(["observation", id.scope(), id.sequence()]),
    };
    json!([origin, value.source(), value.version()])
}

/// Observe the caller's Linux clock namespace device and inode.
///
/// Both separate peers must check their own namespace before relying on the
/// shared absolute deadline. This is metadata, without authority or execution.
pub fn raw_copy_clock_namespace() -> Option<(u64, u64)> {
    clock_namespace()
}

/// Derive the pre-effect correspondence digest used by both peers.
///
/// Independently verified inputs still establish the current store, candidate,
/// tree, boot, pinned execution, controls and stop conditions. The digest
/// identifies that comparison and conveys no permission.
pub fn raw_copy_authority_fingerprint(
    store: &ProgramStore,
    binding: &RawCopyP1Binding<'_>,
    scope: &RawCopyP1AuthorityScope,
) -> Result<[u8; 32], P1Refusal> {
    let reserve = store
        .derive_domain_operation(binding.reservation, |bytes| {
            <[u8; 32]>::from(Sha256::digest(bytes))
        })
        .map_err(|_| P1Refusal::Correspondence)?;
    let target = &binding.target;
    let operations = binding.operations;
    let material = json!([
        "cortex-p1-trial-v1",
        "P1-01",
        binding.store.bytes(),
        reserve,
        binding.candidate.commit().as_str(),
        scope.tree.as_str(),
        binding.controller_sha256,
        binding.controller_max_bytes,
        binding.fixture_sha256,
        binding.plan_digest,
        [
            target.pool.host,
            target.pool.boot,
            target.pool.physical,
            target.pool.incarnation,
            target.delegation.root.as_os_str().as_bytes(),
            target.delegation.device,
            target.delegation.inode,
            target.manager,
            target.manager_version
        ],
        [
            operations.create.as_str(),
            operations.bind.as_str(),
            operations.release.as_str(),
            operations.report.as_str(),
            operations.at,
            reference(&operations.basis)
        ],
        reference(binding.approval),
        reference(binding.observer),
        scope.clock_namespace,
        scope.expires_before_micros,
        scope.containment_sha256,
        scope.stop_plan_sha256,
        1
    ]);
    let bytes = serde_json::to_vec(&material).map_err(|_| P1Refusal::Correspondence)?;
    Ok(Sha256::digest(bytes).into())
}

fn request(
    stage: &str,
    context: &[u8; 32],
    operation: &[u8; 32],
    bound: Option<&[u8; 32]>,
    expected: u64,
    at: u64,
) -> String {
    json!([
        "cortex-p1-trial-v1",
        stage,
        hex(context),
        hex(operation),
        bound.map(hex),
        expected,
        at
    ])
    .to_string()
}

impl<'a> AuthorizedTrial<'a> {
    pub(super) fn authenticate(
        store: &'a ProgramStore,
        binding: &'a RawCopyP1Binding<'a>,
        authority: RawCopyP1Authority,
        cancel: &'a AtomicBool,
    ) -> Result<Self, P1Refusal> {
        let scope = &authority.scope;
        let expires_before_micros = scope.expires_before_micros;
        // Anchor before reading Linux monotonic time. Intervening scheduling
        // can only shorten the mapping from the externally selected expiry.
        let anchor = Instant::now();
        let now = now_micros().ok_or(P1Refusal::DeadlineOrCancellation)?;
        let approved_end = conservative_end(anchor, now, expires_before_micros)
            .ok_or(P1Refusal::DeadlineOrCancellation)?;
        live_budget(binding.deadline, cancel)?;
        if binding.deadline > approved_end
            || scope.tree.as_str().len() != binding.candidate.commit().as_str().len()
            || scope.containment_sha256 == [0; 32]
            || scope.stop_plan_sha256 == [0; 32]
            || clock_namespace() != Some(scope.clock_namespace)
        {
            return Err(P1Refusal::Correspondence);
        }
        let context = raw_copy_authority_fingerprint(store, binding, scope)?;
        let trust =
            TrialTrust::load().map_err(|_| P1Refusal::IndependentQualificationUnavailable)?;
        let operator = trust
            .channel(authority.operator, Role::Operator)
            .map_err(|_| P1Refusal::TrialAuthority)?;
        let observer = trust
            .channel(authority.observer, Role::Observer)
            .map_err(|_| P1Refusal::TrialAuthority)?;
        Ok(Self {
            trust,
            operator,
            observer,
            store,
            binding,
            cancel,
            expires_before_micros,
            clock_namespace: scope.clock_namespace,
            context,
            pending: Cell::new(None),
        })
    }

    pub(super) fn authorize_reservation(&self) -> Result<(), P1Refusal> {
        let reservation = self.binding.reservation;
        let Command::Domain(event) = &reservation.command else {
            return Err(P1Refusal::Correspondence);
        };
        let DomainEvent::Reserve { at, .. } = event.as_ref() else {
            return Err(P1Refusal::Correspondence);
        };
        let digest = self
            .operation_digest(reservation)
            .map_err(|_| P1Refusal::Correspondence)?;
        self.exchange(
            &request(
                "reserve",
                &self.context,
                &digest,
                None,
                reservation.expected,
                *at,
            ),
            self.binding.deadline,
        )
        .map_err(|_| P1Refusal::TrialAuthority)
    }

    pub(super) fn check_live(&self, end: Instant) -> Result<(), DomainDiagnostic> {
        self.check_clock(end)?;
        self.trust
            .check_current()
            .map_err(|_| failed(DomainCause::Authority))?;
        self.operator
            .check_no_revocation()
            .map_err(|_| failed(DomainCause::Authority))?;
        self.observer
            .check_no_revocation()
            .map_err(|_| failed(DomainCause::Authority))?;
        self.check_clock(end)
    }

    fn check_clock(&self, end: Instant) -> Result<(), DomainDiagnostic> {
        if clock_namespace() != Some(self.clock_namespace) {
            return Err(failed(DomainCause::Authority));
        }
        live_budget(end, self.cancel).map_err(|_| failed(DomainCause::Deadline))?;
        if end > self.binding.deadline
            || now_micros().is_none_or(|now| now >= self.expires_before_micros)
        {
            return Err(failed(DomainCause::Deadline));
        }
        Ok(())
    }

    fn exchange(&self, request: &str, end: Instant) -> Result<(), DomainDiagnostic> {
        self.check_live(end)?;
        self.operator
            .reply_to(&self.trust, request.as_bytes(), end)
            .map_err(|_| failed(DomainCause::Authority))?;
        self.observer
            .reply_to(&self.trust, request.as_bytes(), end)
            .map_err(|_| failed(DomainCause::Authority))?;
        self.check_live(end)
    }

    fn operation_digest(&self, operation: &Operation) -> Result<[u8; 32], DomainDiagnostic> {
        self.store
            .derive_domain_operation(operation, |bytes| Sha256::digest(bytes).into())
            .map_err(|_| failed(DomainCause::Contract))
    }

    fn effect_request(
        &self,
        kernel: &CoordinationKernel,
        intent: &DomainIntent,
        operation: &Operation,
        cut: TrialEffectCut<'_>,
    ) -> Result<String, DomainDiagnostic> {
        let denied = failed(DomainCause::Authority);
        let reservation = self.binding.reservation;
        let ids = self.binding.operations;
        let Some(Command::Domain(reserve)) = kernel
            .operations()
            .iter()
            .find(|op| op.id == reservation.id && *op == reservation)
            .map(|op| &op.command)
        else {
            return Err(denied);
        };
        if !matches!(reserve.as_ref(), DomainEvent::Reserve { intent: approved, .. } if approved == intent)
            || operation.grant != ids.grant
        {
            return Err(denied);
        }
        let Command::Domain(event) = &operation.command else {
            return Err(denied);
        };
        let (stage, bound, at) = match (cut, event.as_ref()) {
            (TrialEffectCut::Create, DomainEvent::CreateRequested { domain, at })
                if *domain == ids.domain && operation.id == ids.create =>
            {
                ("create", None, *at)
            }
            (
                TrialEffectCut::Release(binding),
                DomainEvent::ReleaseRequested {
                    domain,
                    identity,
                    at,
                },
            ) if *domain == ids.domain
                && binding.identity == *identity
                && operation.id == ids.release =>
            {
                let accepted = kernel.operations().iter()
                    .find(|record| {
                        record.id == ids.bind && record.grant == ids.grant &&
                            matches!(&record.command, Command::Domain(event)
                                if matches!(event.as_ref(), DomainEvent::BoundObserved { domain, observation, .. }
                                    if *domain == ids.domain && observation == binding))
                    })
                    .ok_or(denied)?;
                ("release", Some(self.operation_digest(accepted)?), *at)
            }
            _ => return Err(denied),
        };
        Ok(request(
            stage,
            &self.context,
            &self.operation_digest(operation)?,
            bound.as_ref(),
            operation.expected,
            at,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_only_the_finite_absolute_interval_from_the_original_anchor() {
        let anchor = Instant::now();
        let proposed_end = anchor + Duration::from_micros(9);
        let authorized_end = conservative_end(anchor, 101, 109).unwrap();
        assert_eq!(authorized_end, anchor + Duration::from_micros(8));
        assert!(proposed_end > authorized_end);
        assert!(conservative_end(anchor, 109, 109).is_none());
        assert!(conservative_end(anchor, 110, 109).is_none());
        assert!(conservative_end(anchor, 100, 60_000_101).is_none());
    }
}

fn failed(cause: DomainCause) -> DomainDiagnostic {
    issue(DomainPhaseDiagnostic::Publication, cause)
}

impl DomainTrialGuard for AuthorizedTrial<'_> {
    fn authorize(
        &self,
        kernel: &CoordinationKernel,
        intent: &DomainIntent,
        operation: &Operation,
        cut: TrialEffectCut<'_>,
        end: Instant,
    ) -> Result<(), DomainDiagnostic> {
        if self.pending.get().is_some() {
            return Err(failed(DomainCause::Authority));
        }
        let request = self.effect_request(kernel, intent, operation, cut)?;
        self.exchange(&request, end)?;
        self.pending
            .set(Some(Sha256::digest(request.as_bytes()).into()));
        Ok(())
    }

    fn check(
        &self,
        kernel: &CoordinationKernel,
        intent: &DomainIntent,
        cut: TrialEffectCut<'_>,
        end: Instant,
    ) -> Result<(), DomainDiagnostic> {
        let retained = kernel
            .operations()
            .last()
            .ok_or_else(|| failed(DomainCause::Authority))?;
        let exact = self.effect_request(kernel, intent, retained, cut)?;
        let digest = <[u8; 32]>::from(Sha256::digest(exact.as_bytes()));
        if self.pending.take() != Some(digest) {
            return Err(failed(DomainCause::Authority));
        }
        self.check_live(end)
    }
}
