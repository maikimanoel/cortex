//! A single bounded turn of the cooperative engineering coordinator.
//!
//! Callers supply the finite work population, explicit existing authority and
//! all operation identities. The coordinator selects work and materializes new
//! operations from the observed revision. The encrypted journal remains the
//! only owner of effects and conflict, replay and recovery semantics.

use crate::{
    program_execution::{
        ExecutionError, FixtureRun, PreparedFixtureRun, run_prepared_fixture_before,
    },
    program_preparation::{PreparationError, prepare_executor_input},
};
use cortex_core::{
    TaskId,
    binding::ObservationId,
    parallel::Event,
    program::{
        Attempt, CoordinationGrant, OperationId,
        context::PackageRequest,
        operational::{EffectEvent, EffectId, EffectIntent, OperationalError, OperationalGrant},
        scheduling::{Proposal, WaitReason},
        supervision::ProcessSpec,
    },
    temporal::SourceReference,
};
use cortex_evidence::program::{
    Command, CoordinationKernel, ExecutorInputRequest, KernelError, Operation,
};
use cortex_harness::{CaptureIds, HarnessError, PinnedFixture, inspect_fixture};
use cortex_storage::{
    history_store::{Access, StoreId, StoreKey},
    program_store::{
        AdmissionFailure, AdmissionReceipt, ProgramProfile, ProgramStore, ProgramStoreError,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

/// Names for the canonical operations created from one declaration.
#[derive(Clone)]
pub struct FixtureOperationIds {
    /// Responsibility claim.
    pub claim: OperationId,
    /// Instruction package.
    pub package: OperationId,
    /// Executor input preparation.
    pub input: OperationId,
    /// Existing scoped operational grant.
    pub grant: OperationId,
    /// Resource reservation.
    pub reserve: OperationId,
    /// Exact process preparation.
    pub process: OperationId,
    /// Commit-before-launch dispatch.
    pub dispatch: OperationId,
}

/// One explicit attempt declaration with no materialized journal predecessors.
pub struct FixtureWork {
    /// Task, finite demand and explicit ordering facts.
    pub proposal: Proposal,
    /// Proposed exclusive responsibility and exact generation.
    pub attempt: Attempt,
    /// Input sources, method context and task/package identity.
    pub package: PackageRequest,
    /// Selected controlled executor and exact input limits.
    pub input: ExecutorInputRequest,
    /// Already supplied authority for these metadata operations.
    pub authority: CoordinationGrant,
    /// Exact scope, current policy and limits, separately supplied by the caller.
    pub grant: OperationalGrant,
    /// Unique effect identifier.
    pub effect: EffectId,
    /// Exact process specification. The canonical input determines input_len.
    pub process: ProcessSpec,
    /// All operation identities, distinct across the selected population.
    pub operations: FixtureOperationIds,
    /// Separate process capture identities and append bounds.
    pub captures: CaptureIds,
    /// Unique candidate reinspection identity.
    pub workspace_observation: ObservationId,
    /// Source basis for dispatch.
    pub dispatch_basis: SourceReference,
}

impl FixtureWork {
    fn operation_ids(&self) -> [&OperationId; 9] {
        [
            &self.operations.claim,
            &self.operations.package,
            &self.operations.input,
            &self.operations.grant,
            &self.operations.reserve,
            &self.operations.process,
            &self.operations.dispatch,
            &self.captures.started_operation,
            &self.captures.finished_operation,
        ]
    }

    fn operation(&self, expected: u64, id: &OperationId, command: Command) -> Operation {
        Operation {
            id: id.clone(),
            expected,
            grant: self.authority.clone(),
            command,
        }
    }
}

/// An observed encrypted journal cut for the next call. It confers no authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureCursor {
    store: StoreId,
    revision: u64,
    last: Option<OperationId>,
}

impl FixtureCursor {
    /// Ordinal at the beginning of the observed turn.
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

/// A refusal before any new effect or a stop after a durable prefix.
#[derive(Debug)]
pub enum FixtureTurnError {
    /// Unsupported store or malformed population, including colliding operation IDs.
    Selection,
    /// The cursor no longer corresponds to this store and retained history.
    Cursor,
    /// Caller cancellation or a bounded call cannot fit before the given cut.
    Budget,
    /// Canonical state could not be read.
    Store(ProgramStoreError),
    /// The existing scheduling policy could not make a proposal.
    Scheduling(OperationalError),
}

/// One selected attempt did not reach durable admission.
#[derive(Debug)]
pub enum FixtureHold {
    /// A canonical operation from this attempt has been retained. Inspect or
    /// reconcile it rather than starting the same declaration again.
    Retained(TaskId),
    /// Existing advisory readiness or capacity conditions hold the proposal.
    Waiting(TaskId, Vec<WaitReason>),
    /// Exact candidate, grant, package or process correspondence is invalid.
    Contract(TaskId),
    /// The canonical draft refused a specific operation.
    Canonical(TaskId, KernelError),
    /// Existing executor input preparation refused the operation.
    Input(TaskId, PreparationError),
    /// Read-only exact executable pinning refused the process.
    Pin(TaskId, HarnessError),
}

/// Effect state after one selected attempt began canonical admission.
pub enum FixtureProgress {
    /// Cancellation or deadline blocked the next step of the retained prefix.
    Budget,
    /// A competing call already retained the same canonical input.
    ConcurrentPrefix,
    /// The next journal input was refused or uncertain. Retain the exact input
    /// for explicit reconciliation. No later step in the attempt was invoked.
    Admission {
        /// Exact materialized input, including the observed predecessor.
        operation: Box<Operation>,
        /// Original typed storage outcome.
        failure: AdmissionFailure,
    },
    /// The dispatch consumer refused or could not establish the admission result.
    Dispatch {
        /// Exact dispatch offered to the existing consumer.
        operation: Box<Operation>,
        /// Original result, which may wrap an indeterminate admission.
        error: ExecutionError,
    },
    /// Actual canonical dispatch and native process observations or uncertainty.
    Observed(Box<FixtureRun>),
}

/// Result of one turn. At most one declaration can have a new durable prefix.
pub struct FixtureTurn {
    /// Pre-turn cut. Pass it back to reconsider events retained since that cut.
    pub cursor: FixtureCursor,
    /// Ordered refusals before durable admission, including retained attempts.
    pub held: Vec<FixtureHold>,
    /// Task selected for canonical admission, if any.
    pub selected: Option<TaskId>,
    /// Metadata receipts retained before dispatch or an interrupted step.
    pub receipts: Vec<AdmissionReceipt>,
    /// Effect state of the selected attempt.
    pub progress: Option<FixtureProgress>,
}

/// Select and advance one attempt from the accepted graph and current journal.
///
/// With a cursor, reconsider consumers of new edge decisions and any broader
/// inputs affecting readiness, accounting or authority. No event queue is retained.
/// Canonical commands are always admitted against an exact expected revision.
/// A determinate draft hold leaves independent candidates eligible. After the
/// first attempted write, the turn retains its prefix and stops at the first
/// error, including uncertainty. A caller must inspect or reconcile that result
/// before evaluating a new turn. A retained in-flight/unknown attempt is held
/// and remains charged through existing resource rules.
pub fn run_fixture_turn(
    store: &ProgramStore,
    key: &StoreKey,
    work: &[FixtureWork],
    since: Option<&FixtureCursor>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<FixtureTurn, FixtureTurnError> {
    if store.profile() != ProgramProfile::SupervisedFixturesV3 {
        return Err(FixtureTurnError::Selection);
    }
    let wait = store.read_limits().timeout();
    if !fits(deadline, wait, cancel) {
        return Err(FixtureTurnError::Budget);
    }
    let kernel = store
        .load(key, Access::ReadOnly)
        .map_err(FixtureTurnError::Store)?;
    let mut ids = BTreeSet::new();
    let mut tasks = BTreeMap::new();
    if work.len() > kernel.state().definition().limits().entries
        || work.iter().any(|job| {
            tasks.insert(job.proposal.task.as_str(), job).is_some()
                || job.operation_ids().iter().any(|id| !ids.insert(*id))
        })
    {
        return Err(FixtureTurnError::Selection);
    }
    let affected = affected_tasks(&kernel, store.id(), since)?;
    let mut turn = FixtureTurn {
        cursor: FixtureCursor {
            store: store.id(),
            revision: kernel.revision(),
            last: kernel.operations().last().map(|op| op.id.clone()),
        },
        held: Vec::new(),
        selected: None,
        receipts: Vec::new(),
        progress: None,
    };
    let retained: BTreeSet<_> = kernel.operations().iter().map(|op| &op.id).collect();
    let mut proposals = Vec::new();
    for job in work {
        if affected
            .as_ref()
            .is_some_and(|tasks| !tasks.contains(job.proposal.task.as_str()))
        {
            continue;
        }
        if job.operation_ids().iter().any(|id| retained.contains(*id)) {
            turn.held
                .push(FixtureHold::Retained(job.proposal.task.clone()));
        } else {
            proposals.push(job.proposal.clone());
        }
    }
    if proposals.is_empty() {
        return Ok(turn);
    }
    let effects = kernel.fixture_effects();
    let order = effects
        .preview_schedule(kernel.state(), &proposals)
        .map_err(FixtureTurnError::Scheduling)?;
    for decision in order {
        if !fits(deadline, wait, cancel) {
            return Err(FixtureTurnError::Budget);
        }
        let job = tasks[decision.task.as_str()];
        // The batch preview provides ordering. A refused neighbor cannot
        // consume capacity in a turn which actually selects at most one.
        let own = if decision.proposed {
            decision
        } else {
            effects
                .preview_schedule(kernel.state(), std::slice::from_ref(&job.proposal))
                .map_err(FixtureTurnError::Scheduling)?
                .pop()
                .ok_or(FixtureTurnError::Selection)?
        };
        if !own.proposed {
            turn.held.push(FixtureHold::Waiting(own.task, own.held));
            continue;
        }
        let staged = match stage(&kernel, job) {
            Ok(staged) => staged,
            Err(hold) => {
                turn.held.push(hold);
                continue;
            }
        };
        turn.selected = Some(job.proposal.task.clone());
        turn.progress = Some(advance(
            store,
            key,
            job,
            staged,
            deadline,
            cancel,
            &mut turn.receipts,
        ));
        break;
    }
    Ok(turn)
}

fn fits(deadline: Instant, needed: Duration, cancel: &AtomicBool) -> bool {
    !cancel.load(Ordering::SeqCst)
        && deadline
            .checked_duration_since(Instant::now())
            .is_some_and(|left| left > needed)
}

fn affected_tasks(
    kernel: &CoordinationKernel,
    store: StoreId,
    since: Option<&FixtureCursor>,
) -> Result<Option<BTreeSet<String>>, FixtureTurnError> {
    let Some(since) = since else { return Ok(None) };
    let index = usize::try_from(since.revision).map_err(|_| FixtureTurnError::Cursor)?;
    let prefix = kernel
        .operations()
        .get(..index)
        .ok_or(FixtureTurnError::Cursor)?;
    if since.store != store || prefix.last().map(|op| &op.id) != since.last.as_ref() {
        return Err(FixtureTurnError::Cursor);
    }
    let mut affected = BTreeSet::new();
    for op in &kernel.operations()[index..] {
        match &op.command {
            Command::Notify(event) if matches!(event.event(), Event::Signal { .. }) => (),
            Command::Release(input)
                if kernel
                    .decisions()
                    .iter()
                    .any(|d| d.operation == op.id && d.released) =>
            {
                affected.insert(input.contract.scope().consumer().task().as_str().to_owned());
            }
            // Failed releases and validity observations can invalidate other
            // consumers. A shared resource or policy event can affect all.
            _ => return Ok(None),
        }
    }
    Ok(Some(affected))
}

struct Staged {
    prefix: Vec<Operation>,
    dispatch: Operation,
    preparation: crate::program_preparation::ExecutorPreparation,
    pin: PinnedFixture,
}

fn stage(kernel: &CoordinationKernel, job: &FixtureWork) -> Result<Staged, FixtureHold> {
    let task = &job.proposal.task;
    let scope = &job.grant.scope;
    if &scope.task != task
        || &job.package.task != task
        || scope.attempt != job.attempt.id
        || scope.package != job.package.id
        || scope.package_revision != job.package.revision
        || scope.package != job.input.package
        || scope.package_revision != job.input.revision
        || job.process.prepared_input != job.operations.input
        || job.process.containment.is_some()
        || job.process.identity_checkpoint.is_some()
        || job.process.limits.transport.is_none()
        || job.captures.admission_attempts == 0
        || job.captures.admission_attempts > kernel.state().definition().limits().entries
        || job.captures.started_observation == job.captures.finished_observation
        || kernel
            .fixture_effects()
            .effects()
            .iter()
            .flat_map(|e| &e.process_observations)
            .any(|old| {
                old.id == job.captures.started_observation
                    || old.id == job.captures.finished_observation
            })
    {
        return Err(FixtureHold::Contract(task.clone()));
    }
    let mut draft = kernel.clone();
    let mut prefix = Vec::with_capacity(6);
    let mut add = |draft: &mut CoordinationKernel, op: Operation| {
        draft
            .admit(op.clone())
            .map_err(|e| FixtureHold::Canonical(task.clone(), e))?;
        prefix.push(op);
        Ok::<_, FixtureHold>(())
    };
    for (id, command) in [
        (&job.operations.claim, Command::Claim(job.attempt.clone())),
        (
            &job.operations.package,
            Command::Package(job.package.clone()),
        ),
    ] {
        let op = job.operation(draft.revision(), id, command);
        add(&mut draft, op)?;
    }
    let input = job.operation(
        draft.revision(),
        &job.operations.input,
        Command::PrepareExecutorInput(Box::new(job.input.clone())),
    );
    let preparation = prepare_executor_input(
        &draft,
        draft.revision(),
        task,
        &scope.candidate,
        input.clone(),
    )
    .map_err(|e| FixtureHold::Input(task.clone(), e))?;
    add(&mut draft, input)?;
    let mut spec = job.process.clone();
    spec.input_len = u64::try_from(preparation.input().bytes().len())
        .map_err(|_| FixtureHold::Contract(task.clone()))?;
    let at = draft.fixture_effects().last_tick();
    for (id, event) in [
        (&job.operations.grant, EffectEvent::Grant(job.grant.clone())),
        (
            &job.operations.reserve,
            EffectEvent::Reserve {
                intent: EffectIntent {
                    id: job.effect.clone(),
                    grant: job.grant.id.clone(),
                    scope: scope.clone(),
                    demand: job.proposal.demand.clone(),
                    retry_of: None,
                },
                at,
            },
        ),
        (
            &job.operations.process,
            EffectEvent::PrepareProcess {
                effect: job.effect.clone(),
                scope: scope.clone(),
                at,
                spec: Box::new(spec.clone()),
            },
        ),
    ] {
        let op = job.operation(
            draft.revision(),
            id,
            Command::SupervisedEffect(Box::new(event)),
        );
        add(&mut draft, op)?;
    }
    let dispatch = job.operation(
        draft.revision(),
        &job.operations.dispatch,
        Command::SupervisedEffect(Box::new(EffectEvent::Dispatch {
            effect: job.effect.clone(),
            scope: scope.clone(),
            at,
            basis: job.dispatch_basis.clone(),
        })),
    );
    let _ = draft
        .preview_admission(&dispatch)
        .map_err(|e| FixtureHold::Canonical(task.clone(), e))?;
    let pin = inspect_fixture(
        &spec,
        preparation.input(),
        draft.state().definition().limits(),
    )
    .map_err(|e| FixtureHold::Pin(task.clone(), e))?;
    Ok(Staged {
        prefix,
        dispatch,
        preparation,
        pin,
    })
}

fn advance(
    store: &ProgramStore,
    key: &StoreKey,
    job: &FixtureWork,
    staged: Staged,
    deadline: Instant,
    cancel: &AtomicBool,
    receipts: &mut Vec<AdmissionReceipt>,
) -> FixtureProgress {
    let wait = store.read_limits().timeout();
    for op in staged.prefix {
        if !fits(deadline, wait, cancel) {
            return FixtureProgress::Budget;
        }
        match store.admit(key, op.clone()) {
            Ok(receipt) => {
                receipts.push(receipt);
                if receipt.duplicate {
                    // A competing controller owns the retained prefix.
                    return FixtureProgress::ConcurrentPrefix;
                }
            }
            Err(failure) => {
                return FixtureProgress::Admission {
                    operation: Box::new(op),
                    failure,
                };
            }
        }
    }
    // Existing supervisor operations have cooperative deadlines and short
    // store/inspection calls. Preserve the entire requested process envelope
    // before submitting dispatch. Check/capture may still fail with a typed
    // result if the host does not make progress.
    let duration = Duration::from_millis(job.process.limits.wall_ms)
        .saturating_add(Duration::from_millis(job.process.limits.cleanup_ms))
        .saturating_add(wait);
    if !fits(deadline, duration, cancel) {
        return FixtureProgress::Budget;
    }
    let dispatch = staged.dispatch;
    match run_prepared_fixture_before(
        store,
        key,
        PreparedFixtureRun {
            preparation: &staged.preparation,
            dispatch: dispatch.clone(),
            pin: &staged.pin,
            captures: job.captures.clone(),
            workspace_observation: job.workspace_observation.clone(),
        },
        deadline,
        cancel,
    ) {
        Ok(run) => FixtureProgress::Observed(Box::new(run)),
        Err(error) => FixtureProgress::Dispatch {
            operation: Box::new(dispatch),
            error,
        },
    }
}
