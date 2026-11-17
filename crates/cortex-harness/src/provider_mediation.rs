//! Non-networked mediation development boundary for exact admitted fixture work.
//!
//! Existing coordination and operational grants carry no live-provider authority.
//! Consequently every live route is refused, irrespective of a matching fixture
//! grant. The sole implemented route handles one explicitly public bounded nonce
//! in memory. It reads existing canonical projections but creates no journal,
//! reservation, dispatch, settlement, credential container or provider connection.

use cortex_core::program::{
    OperationId,
    operational::{EffectEvent, EffectId, EffectScope, OperationalError},
};
use cortex_evidence::program::{Command, CoordinationKernel, Operation};
use std::{error::Error, fmt};

/// Selected route. A child cannot supply an alternate executable or endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediationRoute {
    /// In-memory, public synthetic development data only; no provider call.
    PublicNonceFixture,
    /// Explicit refusal case pending actual provider authority/privacy decisions.
    LiveProvider,
}

/// A bounded source-level request retaining exact existing dispatch correspondence.
/// No credential, environment, arbitrary prompt, URL or authentication field exists.
/// This type intentionally implements no serialization contract.
#[derive(Clone, PartialEq, Eq)]
pub struct MediationRequest {
    dispatch: Operation,
    route: MediationRoute,
    public_nonce: String,
}
impl MediationRequest {
    /// Bind to an existing supervised dispatch proposal and validate public data.
    /// Construction authenticates neither admission nor authority. The consumer
    /// rechecks the complete operation against its current canonical projection.
    pub fn new(
        dispatch: Operation,
        route: MediationRoute,
        public_nonce: &str,
    ) -> Result<Self, MediationError> {
        dispatch_scope(&dispatch)?;
        if public_nonce.is_empty()
            || public_nonce.len() > 64
            || !public_nonce
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(MediationError::DataScope);
        }
        Ok(Self {
            dispatch,
            route,
            public_nonce: public_nonce.into(),
        })
    }
    /// Exact journal dispatch identity, not an independent permission token.
    pub fn dispatch(&self) -> &OperationId {
        &self.dispatch.id
    }
    /// Selected closed route; no implicit route fallback is supported.
    pub fn route(&self) -> MediationRoute {
        self.route
    }
}
impl fmt::Debug for MediationRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MediationRequest")
            .field("route", &self.route)
            .field("context", &"[redacted]")
            .finish()
    }
}

/// Controlled non-networked fixture outcome. Unknown and cancellation are
/// explicit observations, never converted to a returned nonce or success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureDisposition {
    /// The deterministic in-memory fixture returns the exact public nonce.
    Reply,
    /// The fixture reports cancellation, without asserting process quiescence.
    Cancelled,
    /// The fixture reports an unresolved outcome; no replay is performed.
    Unknown,
}

/// Child-visible fixture data contains no scope, path, grant, policy or source.
/// It is a borrowed view, not a provider session or a durable acknowledgement.
pub struct ChildMediationObservation<'a> {
    disposition: FixtureDisposition,
    public_nonce: Option<&'a str>,
}
impl ChildMediationObservation<'_> {
    /// Original outcome, preserving Unknown and cancellation.
    pub fn disposition(&self) -> FixtureDisposition {
        self.disposition
    }
    /// Exact public-only fixture response; absent on Unknown or cancellation.
    pub fn public_nonce(&self) -> Option<&str> {
        self.public_nonce
    }
}

/// Host-side development observation; it is not a child transport structure.
/// It cannot settle an effect or authorize dispatch.
#[derive(Clone, PartialEq, Eq)]
pub struct MediationObservation {
    dispatch: OperationId,
    effect: EffectId,
    scope: EffectScope,
    disposition: FixtureDisposition,
    public_nonce: Option<String>,
}
impl MediationObservation {
    /// Exact original dispatch operation represented by this observation.
    pub fn dispatch(&self) -> &OperationId {
        &self.dispatch
    }
    /// Exact existing effect, with no replacement identity generated.
    pub fn effect(&self) -> &EffectId {
        &self.effect
    }
    /// Complete original candidate, generation, attempt and package correspondence.
    pub fn scope(&self) -> &EffectScope {
        &self.scope
    }
    /// Original fixture outcome, independent of engineering acceptance.
    pub fn disposition(&self) -> FixtureDisposition {
        self.disposition
    }
    /// Only the bounded public fixture nonce, absent on cancellation or Unknown.
    pub fn public_nonce(&self) -> Option<&str> {
        self.public_nonce.as_deref()
    }
    /// Project only explicitly public fixture data for a possible child transport.
    /// Host paths, candidate scope, authority and source references stay outside.
    pub fn child_visible(&self) -> ChildMediationObservation<'_> {
        ChildMediationObservation {
            disposition: self.disposition,
            public_nonce: self.public_nonce.as_deref(),
        }
    }
}
impl fmt::Debug for MediationObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MediationObservation")
            .field("disposition", &self.disposition)
            .field("context", &"[redacted]")
            .finish()
    }
}

/// Typed refusal with no raw request, provider error, credential or source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediationError {
    /// Only an exact supervised fixture dispatch can supply correspondence.
    DispatchContract,
    /// The public-only finite data scope was exceeded or malformed.
    DataScope,
    /// Current source supports no live provider route or fallback.
    UnsupportedRoute,
    /// The exact dispatch has not been canonically admitted in this projection.
    NotAdmitted,
    /// Existing operational scope, policy, grant or lifecycle refused the request.
    Authority(OperationalError),
}
impl fmt::Display for MediationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authority(_) => f.write_str("mediation: existing fixture authority refused"),
            _ => write!(f, "mediation: {self:?}"),
        }
    }
}
impl Error for MediationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Authority(error) => Some(error),
            _ => None,
        }
    }
}

fn dispatch_scope(operation: &Operation) -> Result<(&EffectId, &EffectScope), MediationError> {
    match &operation.command {
        Command::SupervisedEffect(event) => match event.as_ref() {
            EffectEvent::Dispatch { effect, scope, .. } => Ok((effect, scope)),
            _ => Err(MediationError::DispatchContract),
        },
        _ => Err(MediationError::DispatchContract),
    }
}

/// Consume one exact admitted fixture request using the existing Core checks.
///
/// This function is pure with respect to host and canonical state. It performs no
/// network or child I/O, never retries Unknown, and retains no second ledger.
/// A caller-supplied/replayed kernel is only as current/authentic as its source;
/// this development observation cannot confer real provider permission or close
/// any durable effect. A future privileged provider must independently enforce
/// authenticated authority, disclosure, cancellation, audit and retention policy.
pub fn mediate_fixture(
    kernel: &CoordinationKernel,
    request: &MediationRequest,
    at: u64,
    disposition: FixtureDisposition,
) -> Result<MediationObservation, MediationError> {
    if request.route != MediationRoute::PublicNonceFixture {
        return Err(MediationError::UnsupportedRoute);
    }
    if !kernel
        .operations()
        .iter()
        .any(|operation| operation == &request.dispatch)
    {
        return Err(MediationError::NotAdmitted);
    }
    let (effect, scope) = dispatch_scope(&request.dispatch)?;
    kernel
        .fixture_effects()
        .check_supervised_current(kernel.state(), effect, scope, at)
        .map_err(MediationError::Authority)?;
    Ok(MediationObservation {
        dispatch: request.dispatch.id.clone(),
        effect: effect.clone(),
        scope: scope.clone(),
        disposition,
        public_nonce: (disposition == FixtureDisposition::Reply)
            .then(|| request.public_nonce.clone()),
    })
}

#[cfg(test)]
use crate::supervisor::tests::{fixture, support};

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_core::program::operational::EffectStatus;
    use support::{effect, intent, prepared_kernel, source};

    fn admitted() -> (CoordinationKernel, MediationRequest) {
        let (mut kernel, scope, _) = prepared_kernel(None);
        let dispatch = support::dispatch_operation(&kernel, &scope);
        let request = MediationRequest::new(
            dispatch.clone(),
            MediationRoute::PublicNonceFixture,
            "public-development-nonce",
        )
        .unwrap();
        kernel.admit(dispatch).unwrap();
        (kernel, request)
    }

    #[test]
    fn fixture_reply_preserves_exact_admitted_identity_without_mutation() {
        let (kernel, request) = admitted();
        let before = kernel.fixture_effects().clone();
        let count = kernel.revision();
        let observation = mediate_fixture(&kernel, &request, 3, FixtureDisposition::Reply).unwrap();
        let (effect, scope) = dispatch_scope(&request.dispatch).unwrap();
        assert_eq!(observation.dispatch(), &request.dispatch.id);
        assert_eq!(observation.effect(), effect);
        assert!(observation.scope() == scope);
        assert_eq!(observation.public_nonce(), Some("public-development-nonce"));
        assert_eq!(
            observation.child_visible().public_nonce(),
            Some("public-development-nonce")
        );
        assert_eq!(kernel.revision(), count);
        assert!(kernel.fixture_effects() == &before);
    }

    #[test]
    fn live_route_never_falls_back_to_fixture_authority() {
        let (kernel, mut request) = admitted();
        request.route = MediationRoute::LiveProvider;
        assert_eq!(
            mediate_fixture(&kernel, &request, 3, FixtureDisposition::Reply),
            Err(MediationError::UnsupportedRoute)
        );
    }

    #[test]
    fn missing_or_changed_dispatch_cannot_reuse_fixture_qualification() {
        let (kernel, request) = admitted();
        let (unadmitted, _, _) = prepared_kernel(None);
        assert_eq!(
            mediate_fixture(&unadmitted, &request, 3, FixtureDisposition::Reply),
            Err(MediationError::NotAdmitted)
        );
        let mut changed = request.clone();
        changed.dispatch.id = OperationId::new("replacement-dispatch").unwrap();
        assert_eq!(
            mediate_fixture(&kernel, &changed, 3, FixtureDisposition::Reply),
            Err(MediationError::NotAdmitted)
        );
        let Command::SupervisedEffect(event) = &mut changed.dispatch.command else {
            panic!("fixture contract")
        };
        let EffectEvent::Dispatch { scope, .. } = event.as_mut() else {
            panic!("fixture contract")
        };
        scope.generation = fixture::rev(2);
        changed.dispatch.id = request.dispatch.id.clone();
        assert_eq!(
            mediate_fixture(&kernel, &changed, 3, FixtureDisposition::Reply),
            Err(MediationError::NotAdmitted)
        );
    }

    #[test]
    fn current_unknown_or_cancellation_retains_reservation_and_blocks_reply() {
        for cancel in [false, true] {
            let (mut kernel, request) = admitted();
            let (_, scope) = dispatch_scope(&request.dispatch).unwrap();
            let event = if cancel {
                EffectEvent::Cancel {
                    effect: intent(scope).id,
                    scope: scope.clone(),
                    at: 4,
                    basis: source("fixture-cancelled"),
                }
            } else {
                EffectEvent::Unknown {
                    effect: intent(scope).id,
                    scope: scope.clone(),
                    at: 4,
                    basis: source("fixture-unknown"),
                }
            };
            effect(&mut kernel, "mediation-interrupted", event);
            let before = kernel.fixture_effects().clone();
            assert!(matches!(
                mediate_fixture(&kernel, &request, 4, FixtureDisposition::Reply),
                Err(MediationError::Authority(_))
            ));
            assert!(kernel.fixture_effects() == &before);
            assert_eq!(kernel.fixture_effects().effects().len(), 1);
            assert_eq!(
                kernel.fixture_effects().effects()[0].status,
                if cancel {
                    EffectStatus::CancellationRequested
                } else {
                    EffectStatus::Unknown
                }
            );
            assert!(kernel.fixture_effects().has_pending_attempt(&scope.attempt));
        }
    }

    #[test]
    fn expiry_revocation_and_clock_regression_use_existing_core_refusals() {
        let (mut kernel, request) = admitted();
        let grant = kernel.fixture_effects().grants()[0].clone();
        assert!(matches!(
            mediate_fixture(
                &kernel,
                &request,
                grant.expires_before,
                FixtureDisposition::Reply
            ),
            Err(MediationError::Authority(
                OperationalError::GrantUnavailable
            ))
        ));
        assert!(matches!(
            mediate_fixture(&kernel, &request, 2, FixtureDisposition::Reply),
            Err(MediationError::Authority(OperationalError::ClockRegression))
        ));
        effect(
            &mut kernel,
            "mediation-revoke",
            EffectEvent::Revoke {
                grant: grant.id,
                at: 4,
                basis: source("fixture-revocation"),
            },
        );
        assert!(matches!(
            mediate_fixture(&kernel, &request, 4, FixtureDisposition::Reply),
            Err(MediationError::Authority(
                OperationalError::GrantUnavailable
            ))
        ));
    }

    #[test]
    fn unknown_and_cancelled_fixture_observations_never_expose_success_data() {
        let (kernel, request) = admitted();
        for outcome in [FixtureDisposition::Unknown, FixtureDisposition::Cancelled] {
            let observation = mediate_fixture(&kernel, &request, 3, outcome).unwrap();
            assert_eq!(observation.disposition(), outcome);
            assert_eq!(observation.public_nonce(), None);
            assert_eq!(observation.child_visible().public_nonce(), None);
            assert_eq!(observation.child_visible().disposition(), outcome);
            assert_eq!(observation.dispatch(), request.dispatch());
            assert_eq!(kernel.fixture_effects().effects().len(), 1);
        }
    }

    #[test]
    fn public_data_scope_and_diagnostics_exclude_private_payloads() {
        let (kernel, request) = admitted();
        for invalid in [
            "",
            "credential=synthetic-secret",
            "https://alternate.invalid",
            &"x".repeat(65),
        ] {
            assert_eq!(
                MediationRequest::new(
                    request.dispatch.clone(),
                    MediationRoute::PublicNonceFixture,
                    invalid
                ),
                Err(MediationError::DataScope)
            );
        }
        let observation = mediate_fixture(&kernel, &request, 3, FixtureDisposition::Reply).unwrap();
        for diagnostic in [format!("{request:?}"), format!("{observation:?}")] {
            assert!(!diagnostic.contains("public-development-nonce"));
            assert!(!diagnostic.contains(request.dispatch().as_str()));
        }
    }
}
