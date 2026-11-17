//! Typed manager lifetime observations and explicitly owned service pipes.
//!
//! These are Harness mechanics, not a second authority or durable journal. Pure
//! readback validation cannot authenticate an asserted manager or qualify a host.
use crate::{
    DomainCause, DomainDiagnostic, DomainPhaseDiagnostic, ServiceLimits, domain_manager::issue,
};
use cortex_core::program::{
    domain::{DomainControlReadback, DomainIdentity},
    supervision::ProcessCompletion,
};
use std::{fs::File, os::fd::OwnedFd, time::Instant};

/// Controller ends of three anonymous pipes. Only the verified held bootstrap
/// receives the opposite ends; the manager never retains workload pipe writers.
pub struct ManagedPipeEndpoints {
    /// Exact finite prepared input is written here; closing it supplies EOF.
    pub stdin: File,
    /// Workload stdout, separate from the bootstrap control channel.
    pub stdout: File,
    /// Workload stderr; no cross-stream total ordering is inferred.
    pub stderr: File,
}
#[derive(Clone, Copy)]
pub(crate) struct PipeIdentity {
    pub device: u64,
    pub inode: u64,
}
pub(crate) struct ServicePipes {
    child: [OwnedFd; 3],
    identities: [PipeIdentity; 3],
}
impl ServicePipes {
    pub(crate) fn create() -> Result<(Self, ManagedPipeEndpoints), DomainDiagnostic> {
        use rustix::pipe::{PipeFlags, pipe_with};
        let io = |e: rustix::io::Errno| {
            issue(
                DomainPhaseDiagnostic::Preparation,
                DomainCause::Io(std::io::Error::from(e).kind()),
            )
        };
        let (input_read, input_write) = pipe_with(PipeFlags::CLOEXEC).map_err(io)?;
        let (output_read, output_write) = pipe_with(PipeFlags::CLOEXEC).map_err(io)?;
        let (error_read, error_write) = pipe_with(PipeFlags::CLOEXEC).map_err(io)?;
        let child = [input_read, output_write, error_write];
        let identity = |fd: &OwnedFd| -> Result<PipeIdentity, DomainDiagnostic> {
            let m = rustix::fs::fstat(fd).map_err(|e| {
                issue(
                    DomainPhaseDiagnostic::Preparation,
                    DomainCause::Io(std::io::Error::from(e).kind()),
                )
            })?;
            if m.st_mode & 0o170000 != 0o010000 {
                return Err(issue(
                    DomainPhaseDiagnostic::Preparation,
                    DomainCause::Contract,
                ));
            }
            Ok(PipeIdentity {
                device: m.st_dev,
                inode: m.st_ino,
            })
        };
        let identities = [
            identity(&child[0])?,
            identity(&child[1])?,
            identity(&child[2])?,
        ];
        Ok((
            Self { child, identities },
            ManagedPipeEndpoints {
                stdin: File::from(input_write),
                stdout: File::from(output_read),
                stderr: File::from(error_read),
            },
        ))
    }
    pub(crate) fn identities(&self) -> [PipeIdentity; 3] {
        self.identities
    }
    pub(crate) fn into_fds(self) -> [OwnedFd; 3] {
        self.child
    }
}

/// One typed manager property snapshot. Optional facts remain missing instead of
/// inheriting requested values. This structure does not claim atomic capture.
#[derive(Clone)]
pub struct ManagerLifecycleReadback {
    /// Actual exact manager-created domain incarnation.
    pub identity: DomainIdentity,
    /// Observed service type. Strict executable policy requires exec.
    pub service_type: Option<String>,
    /// Observed restart policy, not the caller's intended setting.
    pub restart: Option<String>,
    /// Observed manager kill coverage.
    pub kill_mode: Option<String>,
    /// Observed controls; missing manager properties remain missing.
    pub controls: Option<DomainControlReadback>,
    /// Manager CLOCK_MONOTONIC activation timestamp, in microseconds.
    pub active_enter_monotonic_us: Option<u64>,
}
/// Located refusal of a manager guarantee; no request, source or path is formatted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagerObservationError {
    /// One or more actual property observations are absent.
    MissingReadback,
    /// Boot/manager/invocation/cgroup/request correspondence changed.
    ChangedIncarnation,
    /// Actual manager settings do not provide the selected policy.
    UnsupportedLifetime,
    /// Independent managed runtime would extend past the original deadline.
    DeadlineMismatch,
}
impl ManagerLifecycleReadback {
    /// Check actual same-incarnation manager guarantees. No controller timer,
    /// direct cgroup or PID-only assertion can satisfy these required fields.
    /// Runtime expiry is bounded by the original end; the separately installed
    /// stop grace is not a proof of quiescence or a canonical closure receipt.
    pub fn check(
        &self,
        expected: &DomainIdentity,
        limits: &ServiceLimits,
        original_end_monotonic_us: Option<u64>,
    ) -> Result<(), ManagerObservationError> {
        if self.identity != *expected {
            return Err(ManagerObservationError::ChangedIncarnation);
        }
        self.identity
            .validate()
            .map_err(|_| ManagerObservationError::ChangedIncarnation)?;
        if limits.wall_ms == 0
            || limits.wall_ms > 5000
            || limits.cpu_quota_us == 0
            || limits.cpu_period_us == 0
            || limits.memory_bytes == 0
            || limits.swap_bytes != 0
            || limits.tasks == 0
            || limits.tasks > 8
        {
            return Err(ManagerObservationError::UnsupportedLifetime);
        }
        let c = self
            .controls
            .as_ref()
            .ok_or(ManagerObservationError::MissingReadback)?;
        let kind = self
            .service_type
            .as_deref()
            .ok_or(ManagerObservationError::MissingReadback)?;
        let restart = self
            .restart
            .as_deref()
            .ok_or(ManagerObservationError::MissingReadback)?;
        let kill = self
            .kill_mode
            .as_deref()
            .ok_or(ManagerObservationError::MissingReadback)?;
        if kind != "exec"
            || restart != "no"
            || kill != "control-group"
            || !c.restart_prevented
            || !c.kill_control_group
            || !c.no_new_privileges
            || !c.send_sigkill
            || !c.remain_after_exit
            || c.runtime_max_micros
                != limits
                    .wall_ms
                    .checked_mul(1000)
                    .ok_or(ManagerObservationError::UnsupportedLifetime)?
            || c.timeout_stop_micros != 500_000
            || c.cpu_quota_us != limits.cpu_quota_us
            || c.cpu_period_us != limits.cpu_period_us
            || c.memory_bytes != limits.memory_bytes
            || c.swap_bytes != limits.swap_bytes
            || c.tasks != limits.tasks
        {
            return Err(ManagerObservationError::UnsupportedLifetime);
        }
        {
            let end = original_end_monotonic_us.ok_or(ManagerObservationError::MissingReadback)?;
            let active = self
                .active_enter_monotonic_us
                .filter(|n| *n > 0)
                .ok_or(ManagerObservationError::MissingReadback)?;
            if active
                .checked_add(c.runtime_max_micros)
                .is_none_or(|deadline| deadline > end)
            {
                return Err(ManagerObservationError::DeadlineMismatch);
            }
        }
        Ok(())
    }
}
/// Conservative mapping of the controller's original Instant to Linux manager
/// CLOCK_MONOTONIC. Sampling elapsed time is subtracted rather than extending end.
pub(crate) fn monotonic_deadline(end: Instant) -> Result<u64, DomainDiagnostic> {
    let sample = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    let remaining = end
        .checked_duration_since(Instant::now())
        .ok_or_else(|| issue(DomainPhaseDiagnostic::Preparation, DomainCause::Deadline))?;
    let seconds = u64::try_from(sample.tv_sec)
        .map_err(|_| issue(DomainPhaseDiagnostic::Preparation, DomainCause::Deadline))?;
    let nanos = u64::try_from(sample.tv_nsec)
        .map_err(|_| issue(DomainPhaseDiagnostic::Preparation, DomainCause::Deadline))?;
    seconds
        .checked_mul(1_000_000)
        .and_then(|n| n.checked_add(nanos / 1000))
        .and_then(|n| {
            u64::try_from(remaining.as_micros())
                .ok()
                .and_then(|r| n.checked_add(r))
        })
        .ok_or_else(|| issue(DomainPhaseDiagnostic::Preparation, DomainCause::Deadline))
}
/// Read-only exact manager recovery classification, never closure or settlement.
pub enum ManagerRecoveryState {
    /// Exact retained incarnation, with its current direct-process completion.
    SameIncarnation {
        /// Current manager process status, separate from cgroup emptiness.
        completion: Option<ProcessCompletion>,
        /// Actual recursive cgroup population sample.
        populated: bool,
    },
    /// The recorded lookup name is absent; physical domain absence is unproven.
    NameAbsent,
    /// Bus ownership/boot changed or the name resolves to another invocation.
    ChangedIncarnation,
    /// Typed observations were unavailable or refused; original uncertainty stays.
    Unavailable(DomainDiagnostic),
}
impl std::fmt::Debug for ManagerRecoveryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::SameIncarnation { .. } => "SameIncarnation",
            Self::NameAbsent => "NameAbsent",
            Self::ChangedIncarnation => "ChangedIncarnation",
            Self::Unavailable(_) => "Unavailable",
        })
    }
}
/// Interpret the manager's waitid-style status, never a payload's output claim.
pub(crate) fn completion(
    code: i32,
    status: i32,
    main_pid: u32,
) -> Result<Option<ProcessCompletion>, DomainDiagnostic> {
    if main_pid != 0 {
        return Ok(None);
    }
    match (code, status) {
        (0, 0) => Ok(None),
        (1, n) if (0..=255).contains(&n) => Ok(Some(ProcessCompletion::Exited {
            code: Some(n),
            signal: None,
        })),
        (2 | 3, n) if (1..=64).contains(&n) => Ok(Some(ProcessCompletion::Exited {
            code: None,
            signal: Some(n),
        })),
        _ => Err(issue(
            DomainPhaseDiagnostic::Identity,
            DomainCause::Protocol,
        )),
    }
}

#[cfg(test)]
mod tests;
