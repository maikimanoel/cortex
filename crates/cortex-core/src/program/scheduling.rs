//! Deterministic resource simulation. Output is a proposal, never a reservation or grant.
use super::*;
use std::collections::BTreeMap;

/// Independently accounted resource dimensions; units are explicit in each variant.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Resource {
    /// Declared logical worker capacity units. Reuse or cumulative charging is
    /// determined by the consuming profile, not by this unit or observed PIDs.
    WorkerSlots,
    /// Declared build-job units; accounting policy belongs to the profile.
    BuildJobs,
    /// Declared test-process units, not cases, assertions or test threads.
    TestProcesses,
    /// CPU rate/capacity in millicores: 1000 is one CPU equivalent.
    /// This is not cumulative CPU execution time.
    CpuMillicores,
    /// Historical bytes whose measurement/accounting basis was not retained.
    /// Replay preserves these values; they cannot justify a new memory admission.
    UnspecifiedMemoryBytes,
    /// Bytes with an explicit validated accounting basis and identity scope.
    MemoryBytes(Box<super::memory::MemoryResource>),
    /// Managed disk bytes.
    DiskBytes,
    /// Elapsed milliseconds.
    ElapsedMillis,
    /// Captured output bytes.
    OutputBytes,
    /// CI job units under the selected policy.
    CiJobs,
    /// Model tokens, when actually measurable.
    ModelTokens,
}
impl Resource {
    /// Construct a dimension with explicit validated memory meaning.
    pub fn memory(
        basis: super::memory::MemoryBasis,
        scope: super::memory::MemoryScope,
    ) -> Result<Self, ProgramError> {
        Ok(Self::MemoryBytes(Box::new(
            super::memory::MemoryResource::new(basis, scope)?,
        )))
    }
    /// Require a logical accounting pool when memory enters an allocation.
    /// Physical observation scopes are not additive reservations. Historical
    /// unestablished units remain representable; current admission rejects them.
    pub fn validate_accounting_scope(&self) -> Result<(), ProgramError> {
        if matches!(self, Self::MemoryBytes(memory)
            if !matches!(memory.scope(), super::memory::MemoryScope::AccountingPool(_)))
        {
            return Err(ProgramError::Structure);
        }
        Ok(())
    }
    /// Whether this historical dimension lacks a measurement/accounting basis.
    pub fn memory_basis_unestablished(&self) -> bool {
        matches!(self, Self::UnspecifiedMemoryBytes)
    }
}
/// Explicit simulation capacity, never a measured host guarantee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capacity {
    /// Resource dimension.
    pub resource: Resource,
    /// Caller-declared available amount; absence blocks a required resource.
    pub available: Option<u64>,
    /// Amount protected for verification/integration, never lent to workers.
    pub verification_reserve: u64,
    /// Whether the proposed host matrix declares this required limit enforceable.
    /// This assertion is not host qualification.
    pub declared_enforceable: bool,
}
/// Explicit proposed cost. Unknown cost holds rather than assuming zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Demand {
    /// Dimension.
    pub resource: Resource,
    /// Required amount.
    pub amount: Option<u64>,
}
/// Owner-supplied ordering inputs. Lower bands and risk/cost ranks sort first.
#[derive(Clone, PartialEq, Eq)]
pub struct Proposal {
    /// Target task.
    pub task: TaskId,
    /// Explicit owner value/priority band.
    pub band: u32,
    /// Declared waiting rounds, descending within a band to prevent starvation.
    pub waiting_rounds: u64,
    /// Declared risk rank; no universal risk weights are supplied.
    pub risk_rank: u32,
    /// Declared coordination/review cost rank.
    pub cost_rank: u32,
    /// Every required dimension, without duplicates.
    pub demand: Vec<Demand>,
}
/// Exact reason a simulated allocation is held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitReason {
    /// The canonical graph does not establish eligibility.
    Preconditions,
    /// Missing or unknown required quantity.
    Unknown(Resource),
    /// Required limit is not declared enforceable.
    Unenforceable(Resource),
    /// Insufficient free capacity after verification reserves and prior proposals.
    Contention(Resource),
}
/// One simulated allocation; authorized/reserved/dispatched remain false by contract.
#[derive(Clone, PartialEq, Eq)]
pub struct SchedulingDecision {
    /// Task considered in disclosed order.
    pub task: TaskId,
    /// Dependency readiness, independent of scheduling.
    pub dependency_ready: bool,
    /// Graph eligibility, independent of authority.
    pub eligible: bool,
    /// All current blocking causes; empty means this simulation fits only.
    pub held: Vec<WaitReason>,
    /// Simulated resource subtraction; never a durable/host reservation.
    pub proposed: bool,
}
/// Sort by owner band, waiting age, downstream fan-out, risk/cost, then task ID.
/// Required capacity is conservatively consumed across the proposed batch. The
/// caller must explicitly bound proposal/capacity inputs through the program.
pub fn simulate(
    state: &ProgramState,
    proposals: &[Proposal],
    capacities: &[Capacity],
) -> Result<Vec<SchedulingDecision>, ProgramError> {
    let bound = state.definition().limits().entries;
    if proposals.len() > bound || capacities.len() > 10 {
        return Err(ProgramError::Capacity);
    }
    let mut free = BTreeMap::new();
    let mut insufficient_reserve = BTreeSet::new();
    for c in capacities {
        c.resource.validate_accounting_scope()?;
        if c.available.is_some_and(|v| v < c.verification_reserve) {
            insufficient_reserve.insert(c.resource.clone());
        }
        if free
            .insert(
                c.resource.clone(),
                c.available
                    .map(|v| v.saturating_sub(c.verification_reserve)),
            )
            .is_some()
        {
            return Err(ProgramError::Structure);
        }
    }
    let mut ids = BTreeSet::new();
    for p in proposals {
        for demand in &p.demand {
            demand.resource.validate_accounting_scope()?;
        }
        if !ids.insert(p.task.as_str())
            || p.demand.len() > 10
            || p.demand
                .iter()
                .map(|d| d.resource.clone())
                .collect::<BTreeSet<_>>()
                .len()
                != p.demand.len()
        {
            return Err(ProgramError::Structure);
        }
        state.readiness().task(&p.task)?;
    }
    let downstream = |id: &TaskId| {
        let mut seen = BTreeSet::from([id.as_str()]);
        loop {
            let n = seen.len();
            for e in state.definition().graph().edges() {
                if seen.contains(e.producer().as_str()) {
                    seen.insert(e.consumer().as_str());
                }
            }
            if n == seen.len() {
                return n - 1;
            }
        }
    };
    let mut ordered: Vec<_> = proposals.iter().collect();
    ordered.sort_by(|a, b| {
        a.band
            .cmp(&b.band)
            .then_with(|| b.waiting_rounds.cmp(&a.waiting_rounds))
            .then_with(|| downstream(&b.task).cmp(&downstream(&a.task)))
            .then(a.risk_rank.cmp(&b.risk_rank))
            .then(a.cost_rank.cmp(&b.cost_rank))
            .then(a.task.as_str().cmp(b.task.as_str()))
    });
    let mut result = Vec::new();
    for p in ordered {
        let m = state.readiness().task(&p.task)?;
        let mut held = Vec::new();
        if !m.eligible() {
            held.push(WaitReason::Preconditions);
        }
        for d in &p.demand {
            if d.resource.memory_basis_unestablished() {
                held.push(WaitReason::Unknown(d.resource.clone()));
            }
            let cap = capacities
                .iter()
                .find(|c| c.resource.clone() == d.resource.clone());
            if cap.is_some_and(|c| !c.declared_enforceable) {
                held.push(WaitReason::Unenforceable(d.resource.clone()));
            }
            if insufficient_reserve.contains(&d.resource.clone()) {
                held.push(WaitReason::Contention(d.resource.clone()));
            }
            match (d.amount, free.get(&d.resource.clone()).copied().flatten()) {
                (Some(required), Some(available)) if required > available => {
                    if !insufficient_reserve.contains(&d.resource.clone()) {
                        held.push(WaitReason::Contention(d.resource.clone()));
                    }
                }
                (Some(_), Some(_)) => {}
                _ => held.push(WaitReason::Unknown(d.resource.clone())),
            }
        }
        let proposed = held.is_empty();
        if proposed {
            for d in &p.demand {
                let quantity = free
                    .get_mut(&d.resource.clone())
                    .and_then(Option::as_mut)
                    .expect("checked capacity");
                *quantity -= d.amount.expect("checked demand");
            }
        }
        result.push(SchedulingDecision {
            task: p.task.clone(),
            dependency_ready: m.dependency_ready(),
            eligible: m.eligible(),
            held,
            proposed,
        });
    }
    Ok(result)
}
