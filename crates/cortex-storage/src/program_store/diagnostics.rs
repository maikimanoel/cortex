//! Opt-in, bounded admission observations. These are not a journal or a receipt.
//!
//! Only the store incarnation, operation identifier, expected predecessor, typed
//! stages and monotonic elapsed intervals are retained. Identifiers are available
//! for explicit private correspondence; `Debug` redacts them. No operation
//! payload, path, key, raw engine error or wall-clock timestamp is collected.
use super::{AdmissionFailure, AdmissionReceipt, ProgramStoreError};
use crate::history_store::{Completion, ErrorKind, StoreId};
use cortex_core::program::OperationId;
use std::{
    fmt,
    time::{Duration, Instant},
};

/// Maximum number of observations retained for one call.
pub const ADMISSION_OBSERVATION_CAPACITY: usize = 32;

/// Non-sensitive cause category, distinct from collection completeness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionErrorKind {
    /// Existing shared storage error category, including lock contention (`Busy`).
    Storage(ErrorKind),
    /// Canonical domain validation rejected the operation.
    Domain,
    /// Stored representation or reconstruction is invalid.
    Corrupt,
    /// The selected profile or format is unsupported.
    Unsupported,
    /// A caller-selected capacity was exceeded.
    Capacity,
    /// A cooperative admission budget check expired.
    Deadline,
    /// The store incarnation changed.
    TargetChanged,
    /// Existing store error reports an indeterminate operation.
    Indeterminate,
}
impl From<&ProgramStoreError> for AdmissionErrorKind {
    fn from(error: &ProgramStoreError) -> Self {
        match error {
            ProgramStoreError::Storage(error) => Self::Storage(error.kind()),
            ProgramStoreError::Domain(_) => Self::Domain,
            ProgramStoreError::Corrupt => Self::Corrupt,
            ProgramStoreError::Unsupported => Self::Unsupported,
            ProgramStoreError::Capacity => Self::Capacity,
            ProgramStoreError::Deadline => Self::Deadline,
            ProgramStoreError::TargetChanged => Self::TargetChanged,
            ProgramStoreError::Indeterminate => Self::Indeterminate,
        }
    }
}

/// Caller-visible result; does not strengthen the existing durability contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionOutcome {
    /// An append was acknowledged under the existing SQLCipher contract.
    Appended {
        /// Newly retained canonical operation ordinal.
        ordinal: u64,
    },
    /// Exact replay returned an existing receipt without a new append.
    Duplicate {
        /// Original retained canonical operation ordinal.
        ordinal: u64,
    },
    /// Existing failure classification, without inferred absence or settlement.
    Failed(Completion),
}

/// Actual implementation boundaries, not independent transaction phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionStage {
    /// The explicit diagnostic call entered admission.
    Entered,
    /// Immediately before lifecycle lock acquisition.
    LockRequested,
    /// Lifecycle lock acquisition returned successfully.
    LockAcquired,
    /// Location readiness validation is about to run.
    LocationCheck,
    /// The post-lock budget check is about to run.
    PostLockDeadlineCheck,
    /// Open/configure the encrypted connection, permitting existing recovery.
    Connect,
    /// Begin the immediate transaction.
    BeginTransaction,
    /// Load and reconstruct the retained journal under the transaction.
    LoadJournal,
    /// Validate profile, encode input, admit the draft and check source exclusion.
    ValidateOperation,
    /// Validation returned the canonical ordinal and duplicate predicate.
    Validated {
        /// Canonical ordinal returned by draft admission.
        ordinal: u64,
        /// Whether this is an exact already-retained operation.
        duplicate: bool,
    },
    /// The persistent INSERT is about to run.
    Insert,
    /// The persistent INSERT returned successfully; not a commit receipt.
    InsertAcknowledged,
    /// The existing budget check before commit is about to run.
    PreCommitDeadlineCheck,
    /// Transaction commit is about to run.
    Commit,
    /// Transaction commit returned successfully; later checks can still fail.
    CommitAcknowledged,
    /// The existing budget check after commit is about to run.
    PostCommitDeadlineCheck,
    /// A failure was observed at the preceding boundary.
    Failure(AdmissionErrorKind),
    /// Explicit rollback is about to run.
    RollbackRequested,
    /// Explicit rollback returned successfully.
    RollbackAcknowledged,
    /// Explicit rollback returned an error; no absence is inferred.
    RollbackFailed,
    /// SQLite already ended the transaction after the failed INSERT.
    TransactionAlreadyEnded,
    /// Admission returned and its lifecycle lock was released.
    Finished(AdmissionOutcome),
}

/// One typed event with elapsed time from this diagnostic invocation's origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmissionObservation {
    /// Actual observed boundary.
    pub stage: AdmissionStage,
    /// Monotonic elapsed duration; includes observer work, not isolated CPU time.
    pub elapsed: Duration,
}

/// Caller-owned bounded buffer, reusable only by starting a new diagnostic call.
///
/// Every call resets this report before admission. `complete()` is false before
/// return, on overflow, or if unwinding interrupts the call. A caller retaining
/// the buffer across a caught panic can inspect the prefix. Process termination
/// can lose the entire in-memory report. No observation authenticates its caller
/// or authorizes recovery, replay, dispatch, refund or settlement.
///
/// The bounded operation identifier is cloned once before the admission budget
/// and lock. Recording uses only fixed slots and monotonic clock reads; it does
/// not allocate, format, perform I/O or invoke arbitrary observer callbacks.
pub struct AdmissionDiagnostics {
    operation: Option<OperationId>,
    store: Option<StoreId>,
    expected: Option<u64>,
    start: Option<Instant>,
    observations: [Option<AdmissionObservation>; ADMISSION_OBSERVATION_CAPACITY],
    len: usize,
    dropped: usize,
    finished: bool,
}
impl Default for AdmissionDiagnostics {
    fn default() -> Self {
        Self {
            operation: None,
            store: None,
            expected: None,
            start: None,
            observations: [None; ADMISSION_OBSERVATION_CAPACITY],
            len: 0,
            dropped: 0,
            finished: false,
        }
    }
}
impl fmt::Debug for AdmissionDiagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdmissionDiagnostics")
            .field("operation", &"[redacted]")
            .field("store", &self.store)
            .field("expected", &self.expected)
            .field("observations", &&self.observations[..self.len])
            .field("dropped", &self.dropped)
            .field("complete", &self.complete())
            .finish()
    }
}
impl AdmissionDiagnostics {
    /// Exact store-local identifier; disclose only under the caller's policy.
    pub fn operation(&self) -> Option<&OperationId> {
        self.operation.as_ref()
    }
    /// Expected store incarnation.
    pub fn store(&self) -> Option<StoreId> {
        self.store
    }
    /// Expected journal predecessor supplied by the exact input.
    pub fn expected(&self) -> Option<u64> {
        self.expected
    }
    /// Retained ordered observations; absent slots are never fabricated events.
    pub fn events(&self) -> impl Iterator<Item = &AdmissionObservation> {
        self.observations[..self.len].iter().flatten()
    }
    /// Whether admission returned and no event was dropped.
    pub fn complete(&self) -> bool {
        self.finished && self.dropped == 0
    }
    /// Whether the fixed buffer exhausted its capacity.
    pub fn overflowed(&self) -> bool {
        self.dropped != 0
    }
    /// Number of dropped events, saturating only at `usize::MAX`.
    pub fn dropped_events(&self) -> usize {
        self.dropped
    }
    pub(super) fn begin(&mut self, store: StoreId, input: &cortex_evidence::program::Operation) {
        *self = Self {
            operation: Some(input.id.clone()),
            store: Some(store),
            expected: Some(input.expected),
            start: Some(Instant::now()),
            ..Self::default()
        };
        self.record(AdmissionStage::Entered);
    }
    pub(super) fn finish<R>(
        &mut self,
        result: &Result<(AdmissionReceipt, Option<R>), AdmissionFailure>,
    ) {
        let outcome = match result {
            Ok((receipt, _)) if receipt.duplicate => AdmissionOutcome::Duplicate {
                ordinal: receipt.ordinal,
            },
            Ok((receipt, _)) => AdmissionOutcome::Appended {
                ordinal: receipt.ordinal,
            },
            Err(failure) => AdmissionOutcome::Failed(failure.completion),
        };
        self.record(AdmissionStage::Finished(outcome));
        self.finished = true;
    }
}

pub(super) trait Recorder {
    fn record(&mut self, stage: AdmissionStage);
}
pub(super) struct Disabled;
impl Recorder for Disabled {
    #[inline(always)]
    fn record(&mut self, _: AdmissionStage) {}
}
impl Recorder for AdmissionDiagnostics {
    fn record(&mut self, stage: AdmissionStage) {
        let Some(start) = self.start else {
            return;
        };
        if self.len == self.observations.len() {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        self.observations[self.len] = Some(AdmissionObservation {
            stage,
            elapsed: start.elapsed(),
        });
        self.len += 1;
    }
}
