//! Typed accepted material in the explicit protected-material program profile.

use super::*;
use cortex_core::{
    knowledge::{MAX_SOURCE_BYTES, SourcePath},
    temporal::SourceOrigin,
};

/// The regular-file modes admitted by the committed-object verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialMode {
    /// Git mode 100644.
    Regular,
    /// Git mode 100755.
    Executable,
}

/// Exact committed regular-file material. The digest is SHA-256 over the raw
/// file bytes; it is independent of Git object format and does not cover the path.
#[derive(Clone, PartialEq, Eq)]
pub struct MaterialFile {
    path: SourcePath,
    mode: MaterialMode,
    length: u64,
    sha256: [u8; 32],
}
impl MaterialFile {
    /// Construct a file description using the existing validated literal path.
    /// The narrower program and support envelopes are checked at admission.
    pub fn new(
        path: SourcePath,
        mode: MaterialMode,
        length: u64,
        sha256: [u8; 32],
    ) -> Result<Self, ProgramError> {
        if length > MAX_SOURCE_BYTES as u64 {
            return Err(ProgramError::Capacity);
        }
        Ok(Self {
            path,
            mode,
            length,
            sha256,
        })
    }

    /// Exact literal relative Git path.
    pub fn path(&self) -> &SourcePath {
        &self.path
    }
    /// Exact regular Git file mode.
    pub fn mode(&self) -> MaterialMode {
        self.mode
    }
    /// Exact number of raw file bytes.
    pub fn length(&self) -> u64 {
        self.length
    }
    /// Exact SHA-256 of the raw file bytes.
    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

/// One reviewed source and its only accepted path in the given consumer scope.
#[derive(Clone, PartialEq, Eq)]
pub struct MaterialMapping {
    /// Accepted producer artifact file.
    pub source: MaterialFile,
    /// Exact literal path of that material in the distinct consumer candidate.
    pub destination: SourcePath,
}

/// Typed subject of one independent material verification.
#[derive(Clone, PartialEq, Eq)]
pub enum MaterialSubject {
    /// One exact edge, artifact, prospective release, and producer report.
    Release {
        /// Frozen edge, artifact, criterion and supported profile contracts.
        contract: Box<EdgeEvidenceContract>,
        /// Exact prospective durable release operation.
        release: OperationId,
        /// Exact separate per-edge review notification.
        review: NotificationId,
        /// Existing producer artifact report.
        artifact_report: NotificationId,
        /// Accepted source and its consumer mapping.
        mapping: MaterialMapping,
    },
    /// One exact integration candidate with two current accepted constituents.
    Integration {
        /// Frozen consumer task including its exact criterion and candidate.
        scope: Box<TaskScope>,
        /// The two exact protected releases; legacy releases are insufficient.
        releases: [OperationId; 2],
        /// Exact terminal output expected by the independent semantic oracle.
        output: MaterialFile,
    },
}

/// Retain the typed subject alongside the original verification report.
/// The report supplies no judgment, release or permission on admission.
#[derive(Clone, PartialEq, Eq)]
pub struct MaterialObservation {
    /// Original report with exact candidate, criterion, profile and oracle.
    pub report: ReportedObservation,
    /// Exact typed verification subject.
    pub subject: MaterialSubject,
}

/// Bind one independently assessed material subject to the exact edge release.
#[derive(Clone, PartialEq, Eq)]
pub struct MaterialRelease {
    /// Existing review and full conjunctive support admission.
    pub release: ReleaseInput,
    /// Exact typed observation and expected terminal assessment.
    pub material: EvidenceReference,
}

/// Separate operator approval of one supported typed integration verification.
#[derive(Clone, PartialEq, Eq)]
pub struct IntegrationAcceptance {
    /// Exact typed integration verification and expected assessment.
    pub observation: EvidenceReference,
    /// Exact integration task identity and current candidate generation.
    pub scope: Box<TaskScope>,
    /// Operator approval in the same existing scoped grant.
    pub approval: SourceReference,
    /// Explicit current capture, exact criterion and profile at this cut.
    pub examination: Examination,
}

impl CoordinationKernel {
    /// Create the explicit opt-in, metadata-only protected material interpretation.
    /// Other constructors and older journal profiles refuse the new operation family.
    pub fn new_protected_material(initial: ProgramDefinition) -> Result<Self, KernelError> {
        let mut kernel = Self::new(initial)?;
        kernel.protected_material = true;
        Ok(kernel)
    }

    pub(super) fn observe_material(
        &mut self,
        operation: &Operation,
        observation: &MaterialObservation,
    ) -> Result<(), KernelError> {
        let tasks = self.check_material_subject(&observation.subject, &observation.report)?;
        self.check(&operation.grant, CoordinationPermission::Review, &tasks)?;
        self.check_independence(&operation.grant.actor, &observation.subject)?;
        self.observe_record(&observation.report)
    }

    fn material_observation(
        &self,
        id: &ObservationId,
    ) -> Option<(&Operation, &MaterialObservation)> {
        if !self.protected_material {
            return None;
        }
        self.operations
            .iter()
            .find_map(|operation| match &operation.command {
                Command::ObserveMaterial(observation) if observation.report.id() == id => {
                    Some((operation, observation.as_ref()))
                }
                _ => None,
            })
    }

    pub(super) fn check_material_assessor(
        &self,
        grant: &CoordinationGrant,
        assessment: &AssessmentInput,
    ) -> Result<(), KernelError> {
        let Some((_, observation)) = self.material_observation(&assessment.examination.observation)
        else {
            return Ok(());
        };
        let tasks = self.check_material_subject(&observation.subject, &observation.report)?;
        self.check(grant, CoordinationPermission::Review, &tasks)?;
        self.check_independence(&grant.actor, &observation.subject)?;
        let examination = &assessment.examination;
        if &examination.contract != observation.report.subject().contract()
            || examination.profile.as_ref() != Some(observation.report.profile())
        {
            return Err(ProgramError::Stale.into());
        }
        Ok(())
    }

    pub(super) fn release_material(
        &mut self,
        operation: &Operation,
        material: &MaterialRelease,
    ) -> Result<(), KernelError> {
        let mapping = self.check_material_release(operation, material)?;
        self.check_material_identity(&material.release, mapping)?;
        let limits = material.release.limits;
        let material_remaining = limits
            .get(SupportLimitKind::MaterialBytes)
            .min(self.state.definition().limits().bytes)
            .checked_sub(mapping_charge(mapping)?)
            .ok_or(ProgramError::Capacity)?;
        let support = SupportLimits::new(
            limits.get(SupportLimitKind::Records),
            limits.get(SupportLimitKind::Bindings),
            limits.get(SupportLimitKind::Predicates),
            material_remaining,
        )?;
        self.reduce_release(operation, &material.release, support)?;
        if !self
            .decisions
            .last()
            .is_some_and(|decision| decision.released)
        {
            return Err(ProgramError::Transition.into());
        }
        Ok(())
    }

    fn check_material_identity(
        &self,
        current: &ReleaseInput,
        mapping: &MaterialMapping,
    ) -> Result<(), KernelError> {
        let contract = &current.contract;
        for operation in &self.operations {
            let Command::ReleaseMaterial(material) = &operation.command else {
                continue;
            };
            let past = &material.release.contract;
            if past.scope().graph().id() != contract.scope().graph().id()
                || past.scope().edge() != contract.scope().edge()
                || past.revision() != contract.revision()
            {
                continue;
            }
            let (_, assessed) = self
                .material_observation(material.material.observation())
                .ok_or(ProgramError::Structure)?;
            let MaterialSubject::Release {
                mapping: retained, ..
            } = &assessed.subject
            else {
                return Err(ProgramError::Structure.into());
            };
            if past.scope().requirement() != contract.scope().requirement()
                || past.artifact_requirement() != contract.artifact_requirement()
                || past.correspondences() != contract.correspondences()
                || past.basis() != contract.basis()
                || retained != mapping
            {
                return Err(ProgramError::IdentityConflict.into());
            }
        }
        Ok(())
    }

    fn check_material_release<'a>(
        &'a self,
        operation: &Operation,
        material: &MaterialRelease,
    ) -> Result<&'a MaterialMapping, KernelError> {
        let (_, observation, assessed) = self.material_support(&material.material)?;
        let MaterialSubject::Release {
            contract,
            release,
            review,
            artifact_report,
            mapping,
        } = &observation.subject
        else {
            return Err(ProgramError::Structure.into());
        };
        let input = &material.release;
        let Event::Review(edge_review) = input.review.event() else {
            return Err(ProgramError::Structure.into());
        };
        let [reviewer, approval] = edge_review.attestation().references();
        if contract.as_ref() != &input.contract
            || release != &operation.id
            || review != input.review.id()
            || artifact_report != edge_review.report()
        {
            return Err(ProgramError::Stale.into());
        }
        if edge_review.role() != ReviewRole::Coordinator
            || !matches!(reviewer.origin(), SourceOrigin::OperatorReported { author }
                if author == operation.grant.actor.as_str())
            || approval != &operation.grant.basis
        {
            return Err(ProgramError::Authority.into());
        }
        if !input.bindings.iter().any(|binding| {
            binding.evidence() == &material.material
                && binding.artifact() == contract.scope().requirement().artifact()
                && contract.correspondences().iter().any(|criterion| {
                    criterion.predicate() == binding.predicate()
                        && criterion.verification() == &assessed.examination.contract
                        && Some(criterion.profile()) == assessed.examination.profile.as_ref()
                })
        }) {
            return Err(ProgramError::Structure.into());
        }
        if mapping_charge(mapping)? >= input.limits.get(SupportLimitKind::MaterialBytes) {
            return Err(ProgramError::Capacity.into());
        }
        let tasks = self.check_material_subject(&observation.subject, &observation.report)?;
        self.check(&operation.grant, CoordinationPermission::Review, &tasks)?;
        self.check_independence(&operation.grant.actor, &observation.subject)?;
        Ok(mapping)
    }

    /// Return the exact material association while its release and supporting
    /// judgment remain current. Superseded, legacy and invalidated releases fail.
    pub fn accepted_material(
        &self,
        id: &OperationId,
    ) -> Result<(&ReleaseInput, &MaterialMapping), KernelError> {
        if !self.protected_material || self.replaying {
            return Err(ProgramError::EffectUnavailable.into());
        }
        self.current_material(id)
    }

    fn current_material(
        &self,
        id: &OperationId,
    ) -> Result<(&ReleaseInput, &MaterialMapping), KernelError> {
        let operation = self.operation(id)?;
        let Command::ReleaseMaterial(material) = &operation.command else {
            return Err(ProgramError::Missing.into());
        };
        let release = &material.release;
        let scope = release.contract.scope();
        let decision = self
            .decisions
            .iter()
            .rev()
            .find(|decision| &decision.edge == scope.edge() && decision.released);
        if decision.is_none_or(|decision| &decision.operation != id)
            || !matches!(self.state.readiness().edge(scope.edge())?.decision(),
                EdgeDecision::Released { review, .. } if review == release.review.id())
        {
            return Err(ProgramError::Stale.into());
        }
        Ok((release, self.check_material_release(operation, material)?))
    }

    pub(super) fn check_integration_acceptance(
        &self,
        operation: &Operation,
        acceptance: &IntegrationAcceptance,
    ) -> Result<&MaterialSubject, KernelError> {
        let (_, observation, assessment) = self.material_support(&acceptance.observation)?;
        let MaterialSubject::Integration { scope, .. } = &observation.subject else {
            return Err(ProgramError::Structure.into());
        };
        let Some(fresh) = acceptance.examination.candidate.as_ref() else {
            return Err(ProgramError::Stale.into());
        };
        let Some(assessed) = assessment.examination.candidate.as_ref() else {
            return Err(ProgramError::Stale.into());
        };
        if fresh.id().scope() != assessed.id().scope()
            || fresh.id().sequence() <= assessed.id().sequence()
            || scope != &acceptance.scope
            || acceptance.examination.observation != assessment.examination.observation
            || acceptance.examination.contract != assessment.examination.contract
            || acceptance.examination.profile != assessment.examination.profile
            || !self.current_pass(&acceptance.examination)?
        {
            return Err(ProgramError::Stale.into());
        }
        if acceptance.approval != operation.grant.basis {
            return Err(ProgramError::Authority.into());
        }
        let tasks = self.check_material_subject(&observation.subject, &observation.report)?;
        self.check(&operation.grant, CoordinationPermission::Review, &tasks)?;
        self.check_independence(&operation.grant.actor, &observation.subject)?;
        Ok(&observation.subject)
    }

    /// Return the exact subject of a retained protected integration acceptance
    /// only while its candidate, assessments, releases and scopes are current.
    pub fn accepted_integration(&self, id: &OperationId) -> Result<&MaterialSubject, KernelError> {
        if !self.protected_material || self.replaying {
            return Err(ProgramError::EffectUnavailable.into());
        }
        let operation = self.operation(id)?;
        let Command::AcceptIntegration(acceptance) = &operation.command else {
            return Err(ProgramError::Missing.into());
        };
        self.check_integration_acceptance(operation, acceptance)
    }

    fn operation(&self, id: &OperationId) -> Result<&Operation, KernelError> {
        self.operations
            .iter()
            .find(|operation| &operation.id == id)
            .ok_or(ProgramError::Missing.into())
    }

    fn current_pass(&self, examination: &Examination) -> Result<bool, KernelError> {
        let result = self.inspect_record(examination)?;
        Ok(result.result == Some(AssessmentResult::Pass)
            && result.applicability == Some(Applicability::Applicable)
            && result.validity == Some(Validity::Current))
    }

    fn material_support(
        &self,
        reference: &EvidenceReference,
    ) -> Result<(&Operation, &MaterialObservation, &AssessmentInput), KernelError> {
        let (observed, observation) = self
            .material_observation(reference.observation())
            .ok_or(ProgramError::Missing)?;
        let expected = reference.assessment().ok_or(ProgramError::Missing)?;
        let (judged, assessment) = self
            .operations
            .iter()
            .find_map(|operation| match &operation.command {
                Command::Assess(assessment)
                    if &assessment.id == expected
                        && &assessment.examination.observation == reference.observation() =>
                {
                    Some((operation, assessment.as_ref()))
                }
                _ => None,
            })
            .ok_or(ProgramError::Missing)?;
        if observed.grant.baseline != self.state.definition().revision()
            || judged.grant.baseline != observed.grant.baseline
            || !self.current_pass(&assessment.examination)?
        {
            return Err(ProgramError::Stale.into());
        }
        self.check_independence(&observed.grant.actor, &observation.subject)?;
        self.check_independence(&judged.grant.actor, &observation.subject)?;
        Ok((observed, observation, assessment))
    }

    fn check_material_subject(
        &self,
        subject: &MaterialSubject,
        observation: &ReportedObservation,
    ) -> Result<Vec<TaskId>, KernelError> {
        let readiness = self.state.readiness();
        let limit = self.state.definition().limits().bytes;
        let reported = observation.subject().contract();
        match subject {
            MaterialSubject::Release {
                contract,
                mapping,
                artifact_report,
                ..
            } => {
                let scope = contract.scope();
                let current = EdgeScopeSnapshot::capture(readiness, scope.edge())?;
                if current.graph() != scope.graph()
                    || current.producer() != scope.producer()
                    || current.consumer() != scope.consumer()
                    || current.requirement() != scope.requirement()
                    || current.requirement().kind() != DependencyKind::Artifact
                    || reported.candidate() != contract.artifact_requirement()
                    || contract.artifact_requirement().repository()
                        != scope
                            .producer()
                            .context()
                            .contract()
                            .candidate()
                            .repository()
                    || contract.artifact_requirement().worktree() != WorktreeRequirement::Clean
                    || !contract.correspondences().iter().any(|criterion| {
                        criterion.verification() == reported
                            && criterion.profile() == observation.profile()
                    })
                {
                    return Err(ProgramError::Stale.into());
                }
                self.check_selected_artifact_report(artifact_report, contract)?;
                if mapping_charge(mapping)? >= limit
                    || contract.correspondences().len() > self.state.definition().limits().entries
                    || scope.requirement().predicates().len()
                        > self.state.definition().limits().entries
                {
                    return Err(ProgramError::Capacity.into());
                }
                Ok(vec![
                    scope.producer().task().clone(),
                    scope.consumer().task().clone(),
                ])
            }
            MaterialSubject::Integration {
                scope,
                releases,
                output,
            } => {
                if &readiness.scope(scope.task())? != scope.as_ref()
                    || scope.context().contract() != reported
                    || reported.candidate().worktree() != WorktreeRequirement::Clean
                    || releases[0] == releases[1]
                {
                    return Err(ProgramError::Stale.into());
                }
                let [(left, left_mapping), (right, right_mapping)] = [
                    self.current_material(&releases[0])?,
                    self.current_material(&releases[1])?,
                ];
                let inputs = [(left, left_mapping), (right, right_mapping)];
                if self
                    .state
                    .definition()
                    .graph()
                    .edges()
                    .iter()
                    .filter(|edge| edge.consumer() == scope.task())
                    .count()
                    != inputs.len()
                    || left.contract.scope().edge() == right.contract.scope().edge()
                    || left.contract.scope().producer().task()
                        == right.contract.scope().producer().task()
                    || left_mapping.destination == right_mapping.destination
                    || (left.contract.artifact_requirement().commit()
                        == right.contract.artifact_requirement().commit()
                        && left_mapping.source.path() == right_mapping.source.path())
                {
                    return Err(ProgramError::Structure.into());
                }
                let mut checked_bytes = file_charge(output)?
                    .checked_mul(2)
                    .ok_or(ProgramError::Capacity)?;
                let mut tasks = vec![scope.task().clone()];
                for (release, mapping) in inputs {
                    let worker = release.contract.artifact_requirement();
                    let release_scope = release.contract.scope();
                    if release_scope.consumer() != scope.as_ref()
                        || worker.commit() == reported.candidate().commit()
                        || &mapping.destination == output.path()
                    {
                        return Err(ProgramError::Stale.into());
                    }
                    checked_bytes = checked_bytes
                        .checked_add(mapping_charge(mapping)?)
                        .ok_or(ProgramError::Capacity)?;
                    tasks.push(release_scope.producer().task().clone());
                }
                if checked_bytes > limit {
                    return Err(ProgramError::Capacity.into());
                }
                Ok(tasks)
            }
        }
    }

    fn artifact_reports<'a>(
        &'a self,
        contract: &'a EdgeEvidenceContract,
    ) -> impl Iterator<Item = (&'a NotificationId, &'a ActorId)> {
        self.operations.iter().filter_map(move |operation| {
            let Command::Notify(notification) = &operation.command else {
                return None;
            };
            let Event::Artifact(report) = notification.event() else {
                return None;
            };
            (notification.graph() == contract.scope().graph()
                && report.producer() == contract.scope().producer()
                && report.artifact() == contract.scope().requirement().artifact())
            .then_some((notification.id(), &operation.grant.actor))
        })
    }

    fn check_selected_artifact_report(
        &self,
        id: &NotificationId,
        contract: &EdgeEvidenceContract,
    ) -> Result<(), KernelError> {
        let mut authors = self
            .artifact_reports(contract)
            .filter_map(|(report, actor)| (report == id).then_some(actor));
        let author = authors.next().ok_or(ProgramError::Missing)?;
        if authors.any(|other| other != author) {
            return Err(ProgramError::Authority.into());
        }
        Ok(())
    }

    pub(super) fn check_material_review_scope(
        &self,
        grant: &CoordinationGrant,
        id: &ObservationId,
    ) -> Result<(), KernelError> {
        let Some((_, observed)) = self.material_observation(id) else {
            return Ok(());
        };
        let tasks = match &observed.subject {
            MaterialSubject::Release { contract, .. } => vec![
                contract.scope().producer().task().clone(),
                contract.scope().consumer().task().clone(),
            ],
            MaterialSubject::Integration {
                scope, releases, ..
            } => {
                let mut tasks = vec![scope.task().clone()];
                for id in releases {
                    let Command::ReleaseMaterial(material) = &self.operation(id)?.command else {
                        return Err(ProgramError::Missing.into());
                    };
                    tasks.push(material.release.contract.scope().producer().task().clone());
                }
                tasks
            }
        };
        self.check(grant, CoordinationPermission::Review, &tasks)
    }

    fn check_independence(
        &self,
        actor: &ActorId,
        subject: &MaterialSubject,
    ) -> Result<(), KernelError> {
        match subject {
            MaterialSubject::Release { contract, .. } => {
                let producer = contract.scope().producer();
                if self
                    .artifact_reports(contract)
                    .any(|(_, author)| actor == author)
                    || self.task_author(actor, producer).0
                {
                    return Err(ProgramError::Authority.into());
                }
            }
            MaterialSubject::Integration {
                scope, releases, ..
            } => {
                let (own_author, has_author) = self.task_author(actor, scope);
                if own_author || !has_author {
                    return Err(ProgramError::Authority.into());
                }
                for id in releases {
                    let Command::ReleaseMaterial(material) = &self.operation(id)?.command else {
                        return Err(ProgramError::Missing.into());
                    };
                    let (_, observation, _) = self.material_support(&material.material)?;
                    self.check_independence(actor, &observation.subject)?;
                }
            }
        }
        Ok(())
    }

    fn task_author(&self, actor: &ActorId, scope: &TaskScope) -> (bool, bool) {
        let mut exists = false;
        for attempt in self
            .state
            .attempts()
            .iter()
            .filter(|attempt| attempt.generation == scope.generation())
        {
            let Some(definition) = self
                .state
                .definitions()
                .iter()
                .find(|definition| definition.revision() == attempt.baseline)
            else {
                continue;
            };
            if definition.increments().iter().any(|increment| {
                increment.id == attempt.increment && &increment.task == scope.task()
            }) {
                exists |= attempt.baseline == self.state.definition().revision()
                    && matches!(
                        attempt.status,
                        AttemptStatus::Claimed | AttemptStatus::Reported
                    );
                if &attempt.owner == actor {
                    return (true, exists);
                }
            }
        }
        (false, exists)
    }
}

fn file_charge(file: &MaterialFile) -> Result<usize, ProgramError> {
    usize::try_from(file.length)
        .ok()
        .and_then(|length| length.checked_add(file.path.bytes().len()))
        .and_then(|total| total.checked_add(file.sha256.len() + size_of::<u64>() + 1))
        .ok_or(ProgramError::Capacity)
}

fn mapping_charge(mapping: &MaterialMapping) -> Result<usize, ProgramError> {
    file_charge(&mapping.source)?
        .checked_add(mapping.destination.bytes().len())
        .and_then(|total| total.checked_add(mapping.source.length as usize))
        .ok_or(ProgramError::Capacity)
}
