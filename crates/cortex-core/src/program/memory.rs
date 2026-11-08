//! Explicit memory units and identity scopes. These are correspondence contracts,
//! not authentication, host qualification, or proof that a declared limit is enforced.
use super::{ProgramError, domain::PoolIdentity};

/// The accounting definition of a byte quantity; equal units do not imply equal bases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoryBasis {
    /// Resident pages attributed to one process by the selected observation mechanism.
    ProcessResident,
    /// Sum of the RSS of an explicit process set; shared pages count once per process.
    SummedProcessResident,
    /// Memory charged to a control group, including the mechanism's non-RSS charges.
    CgroupCharged,
}
/// One process lifetime, not a reusable numeric PID.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemoryProcessIdentity {
    /// Explicit host identity.
    pub host: String,
    /// Exact host boot identity.
    pub boot: String,
    /// Observed process ID.
    pub pid: u32,
    /// Birth tick in the same boot.
    pub start_ticks: u64,
}
/// The scope to which a memory quantity applies. A pool is a logical accounting
/// scope; it does not make a child cgroup reading a measurement of the whole pool.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoryScope {
    /// Reservations charged against this exact declared physical allocation.
    AccountingPool(PoolIdentity),
    /// RSS of this one observed process lifetime.
    Process(MemoryProcessIdentity),
    /// RSS sum over this sorted, nonempty, duplicate-free process set.
    ProcessSet(Vec<MemoryProcessIdentity>),
    /// Charged memory of an exactly identified cgroup incarnation.
    Cgroup {
        /// Host identity.
        host: String,
        /// Host boot identity.
        boot: String,
        /// Manager incarnation.
        manager: String,
        /// Managed invocation incarnation.
        invocation: String,
        /// Exact absolute cgroup path.
        path: String,
        /// Filesystem device identity.
        device: u64,
        /// Nonzero filesystem inode identity.
        inode: u64,
    },
}
/// A validated basis/scope pair. Construction checks correspondence shape, not
/// the truth of supplied identities. No unknown basis can enter this type.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemoryResource {
    basis: MemoryBasis,
    scope: MemoryScope,
}
impl MemoryResource {
    /// Validate the meaning of the supplied accounting or observation scope.
    pub fn new(basis: MemoryBasis, scope: MemoryScope) -> Result<Self, ProgramError> {
        fn text(s: &str) -> bool {
            !s.is_empty() && s.len() <= 1024 && !s.chars().any(char::is_control)
        }
        fn process(p: &MemoryProcessIdentity) -> bool {
            text(&p.host) && text(&p.boot) && p.pid != 0 && p.start_ticks != 0
        }
        let valid = match &scope {
            MemoryScope::AccountingPool(pool) => pool.validate().is_ok(),
            MemoryScope::Process(p) => basis == MemoryBasis::ProcessResident && process(p),
            MemoryScope::ProcessSet(members) => {
                basis == MemoryBasis::SummedProcessResident
                    && !members.is_empty()
                    && members.len() <= 256
                    && members.iter().all(process)
                    && members
                        .windows(2)
                        .all(|w| w[0] < w[1] && w[0].host == w[1].host && w[0].boot == w[1].boot)
                    && members
                        .iter()
                        .map(|p| p.pid)
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        == members.len()
            }
            MemoryScope::Cgroup {
                host,
                boot,
                manager,
                invocation,
                path,
                inode,
                ..
            } => {
                basis == MemoryBasis::CgroupCharged
                    && *inode != 0
                    && [host, boot, manager, invocation, path]
                        .iter()
                        .all(|s| text(s))
                    && path.starts_with('/')
                    && !path.split('/').any(|part| part == ".." || part == ".")
            }
        };
        if !valid {
            return Err(ProgramError::Structure);
        }
        Ok(Self { basis, scope })
    }
    /// Exact accounting/measurement definition.
    pub fn basis(&self) -> MemoryBasis {
        self.basis
    }
    /// Exact retained scope; equality does not authenticate it.
    pub fn scope(&self) -> &MemoryScope {
        &self.scope
    }
    /// Whether this is the exact logical allocation named by a pool contract.
    pub fn accounts_pool(&self, pool: &PoolIdentity) -> bool {
        matches!(&self.scope, MemoryScope::AccountingPool(p) if p == pool)
    }
    /// Add known quantities only within the same definition and scope. This is
    /// not a rule for summing different cgroups, process sets, peaks, or snapshots.
    pub fn checked_add(&self, left: u64, other: &Self, right: u64) -> Result<u64, ProgramError> {
        if self != other || !matches!(self.scope, MemoryScope::AccountingPool(_)) {
            return Err(ProgramError::Structure);
        }
        left.checked_add(right).ok_or(ProgramError::Capacity)
    }
    /// Compare known amounts only under an identical basis and scope.
    pub fn contains(&self, limit: u64, other: &Self, amount: u64) -> Result<bool, ProgramError> {
        if self != other {
            return Err(ProgramError::Structure);
        }
        Ok(amount <= limit)
    }
}
/// Available correspondence for a memory fact, without a redundant basis field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryCorrespondence {
    /// Exact structurally validated basis and scope.
    Scoped(MemoryResource),
    /// The mechanism establishes its basis but lacks complete scope identity.
    ScopeUnestablished(MemoryBasis),
}
impl MemoryCorrespondence {
    /// Known measurement/accounting definition.
    pub fn basis(&self) -> MemoryBasis {
        match self {
            Self::Scoped(r) => r.basis(),
            Self::ScopeUnestablished(b) => *b,
        }
    }
    /// Exact scope if established by the retained identity.
    pub fn resource(&self) -> Option<&MemoryResource> {
        match self {
            Self::Scoped(r) => Some(r),
            Self::ScopeUnestablished(_) => None,
        }
    }
}
/// A sampled amount. Absence is unknown, never zero. This is not a demand,
/// reservation, effective limit, or a declaration of final usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryObservation {
    correspondence: MemoryCorrespondence,
    bytes: Option<u64>,
}
impl MemoryObservation {
    /// Retain a sampled quantity without inventing missing values or identities.
    pub fn new(correspondence: MemoryCorrespondence, bytes: Option<u64>) -> Self {
        Self {
            correspondence,
            bytes,
        }
    }
    /// Available exact correspondence and its limitations.
    pub fn correspondence(&self) -> &MemoryCorrespondence {
        &self.correspondence
    }
    /// Known basis, independent of scope completeness.
    pub fn basis(&self) -> MemoryBasis {
        self.correspondence.basis()
    }
    /// Exact basis/scope if available.
    pub fn resource(&self) -> Option<&MemoryResource> {
        self.correspondence.resource()
    }
    /// Observed bytes; None is not zero.
    pub fn bytes(&self) -> Option<u64> {
        self.bytes
    }
}

impl std::fmt::Debug for MemoryProcessIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MemoryProcessIdentity([redacted])")
    }
}
impl std::fmt::Debug for MemoryScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::AccountingPool(_) => "AccountingPool([redacted])",
            Self::Process(_) => "Process([redacted])",
            Self::ProcessSet(_) => "ProcessSet([redacted])",
            Self::Cgroup { .. } => "Cgroup([redacted])",
        })
    }
}
impl std::fmt::Debug for MemoryResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryResource")
            .field("basis", &self.basis)
            .field("scope", &self.scope)
            .finish()
    }
}
/// The role of a retained memory ceiling. A readback is not independent enforcement qualification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryLimitKind {
    /// Declared/requested control, not observed usage.
    Requested,
    /// Effective configuration readback, not a measurement of consumption.
    ReadBack,
}
/// A memory ceiling kept separate from a measured sample and logical reservation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryLimit {
    correspondence: MemoryCorrespondence,
    bytes: u64,
    kind: MemoryLimitKind,
}
impl MemoryLimit {
    /// Retain a positive requested/readback ceiling, never observed consumption.
    pub fn new(
        correspondence: MemoryCorrespondence,
        bytes: u64,
        kind: MemoryLimitKind,
    ) -> Result<Self, ProgramError> {
        if bytes == 0 {
            return Err(ProgramError::Structure);
        }
        Ok(Self {
            correspondence,
            bytes,
            kind,
        })
    }
    /// Exact basis/scope when available.
    pub fn resource(&self) -> Option<&MemoryResource> {
        self.correspondence.resource()
    }
    /// Mechanism's accounting definition.
    pub fn basis(&self) -> MemoryBasis {
        self.correspondence.basis()
    }
    /// Configured ceiling; not a counter.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
    /// Declaration or readback, not a host qualification claim.
    pub fn kind(&self) -> MemoryLimitKind {
        self.kind
    }
}
