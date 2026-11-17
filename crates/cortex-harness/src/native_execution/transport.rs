//! Finite raw standard-stream pumping for an already owned trusted child.
//! This component does not launch, authorize or claim whole-domain containment.
//! Raw collection preserves binary stdout. Explicit structured collection adds
//! the existing bounded JSON-lines/startup/record contract, without promoting
//! syntax, process exit or stream EOF into accepted work or domain closure.
use super::ExecutionError;
use cortex_core::program::supervision::{
    InterruptionReason, ProcessCompletion, ProcessLimits, Quiescence,
};
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fs::{OFlags, fcntl_getfl, fcntl_setfl},
};
use std::{
    io::{Read, Write},
    os::{
        fd::{AsFd, OwnedFd},
        unix::process::ExitStatusExt,
    },
    process::Child,
    time::{Duration, Instant},
};
/// Bounded original streams with independently visible completeness and cause.
pub struct CapturedStreams {
    /// Finite stdout prefix; private content is not formatted by Debug.
    pub stdout: Vec<u8>,
    /// Finite stderr prefix.
    pub stderr: Vec<u8>,
    /// Original completion/interruption, not overwritten by cleanup.
    pub completion: ProcessCompletion,
    /// Direct-child-only observation. Group closure requires a separate oracle.
    pub quiescence: Quiescence,
    /// Whether every intended input byte was accepted by the pipe.
    pub input_complete: bool,
    /// Whether stdout reached EOF without truncation.
    pub stdout_complete: bool,
    /// Whether stderr reached EOF without truncation.
    pub stderr_complete: bool,
    /// Elapsed pump time, excluding prior preparation.
    pub elapsed: Duration,
}

fn nonblock(fd: impl AsFd) -> Result<(), ExecutionError> {
    fcntl_setfl(&fd, fcntl_getfl(&fd)? | OFlags::NONBLOCK)?;
    Ok(())
}
struct Pipes {
    stdin: Option<std::fs::File>,
    stdout: Option<std::fs::File>,
    stderr: Option<std::fs::File>,
}
trait Process {
    fn poll(&mut self, end: Instant) -> std::io::Result<Option<ProcessCompletion>>;
    fn interrupt(&mut self);
    fn direct_reap(&self) -> bool {
        true
    }
}
impl Process for Child {
    fn poll(&mut self, _: Instant) -> std::io::Result<Option<ProcessCompletion>> {
        self.try_wait().map(|s| {
            s.map(|s| ProcessCompletion::Exited {
                code: s.code(),
                signal: s.signal(),
            })
        })
    }
    fn interrupt(&mut self) {
        let _ = self.kill();
    }
}
struct ManagedProcess<F>(F);
impl<F: FnMut(Instant) -> std::io::Result<Option<ProcessCompletion>>> Process
    for ManagedProcess<F>
{
    fn poll(&mut self, end: Instant) -> std::io::Result<Option<ProcessCompletion>> {
        (self.0)(end)
    }
    // Stop belongs to canonical durable Cancel, not an I/O callback. The selected
    // independent manager retains its installed lifetime/stop obligations.
    fn interrupt(&mut self) {}
    fn direct_reap(&self) -> bool {
        false
    }
}
/// Raw byte collection, preserving the original D profile. No JSON mode is
/// selected merely because TransportLimits is present.
pub fn collect(
    child: &mut Child,
    input: &[u8],
    limits: &ProcessLimits,
    end: Instant,
    current: impl FnMut() -> Result<(), InterruptionReason>,
) -> Result<CapturedStreams, ExecutionError> {
    validate(limits, input)?;
    let pipes = child_pipes(child)?;
    collect_io(
        child,
        pipes,
        input,
        limits,
        Instant::now(),
        end,
        current,
        None,
    )
    .map(|v| v.0)
}
/// Raw capture plus explicit existing-schema JSON-lines observation.
pub struct StructuredCapture {
    /// Original finite streams and process/cleanup observations.
    pub streams: CapturedStreams,
    /// Syntax/startup/event observation, not successful external work.
    pub structured: crate::structured_output::StructuredSummary,
}
/// Explicit structured mode. original_start is the original pre-admission clock;
/// startup, idle and wall are not restarted when collection begins.
pub fn collect_structured(
    child: &mut Child,
    input: &[u8],
    limits: &ProcessLimits,
    original_start: Instant,
    end: Instant,
    current: impl FnMut() -> Result<(), InterruptionReason>,
) -> Result<StructuredCapture, ExecutionError> {
    validate(limits, input)?;
    let observer = observer(limits, input)?;
    let pipes = child_pipes(child)?;
    let (streams, summary) = collect_io(
        child,
        pipes,
        input,
        limits,
        original_start,
        end,
        current,
        Some(observer),
    )?;
    Ok(StructuredCapture {
        streams,
        structured: summary.ok_or(ExecutionError::Policy)?,
    })
}
/// Manager-owned process variant. Only pipe ownership is transferred; no Child
/// handle/PID is reconstructed or reaped by this adapter.
#[allow(clippy::too_many_arguments)]
pub(crate) fn collect_managed(
    pipes: super::managed::ManagedPipeEndpoints,
    input: &[u8],
    limits: &ProcessLimits,
    original_start: Instant,
    end: Instant,
    current: impl FnMut() -> Result<(), InterruptionReason>,
    poll: impl FnMut(Instant) -> std::io::Result<Option<ProcessCompletion>>,
    structured: bool,
) -> Result<
    (
        CapturedStreams,
        Option<crate::structured_output::StructuredSummary>,
    ),
    ExecutionError,
> {
    validate(limits, input)?;
    let observer = if structured {
        Some(observer(limits, input)?)
    } else {
        None
    };
    collect_io(
        &mut ManagedProcess(poll),
        Pipes {
            stdin: Some(pipes.stdin),
            stdout: Some(pipes.stdout),
            stderr: Some(pipes.stderr),
        },
        input,
        limits,
        original_start,
        end,
        current,
        observer,
    )
}
fn validate(limits: &ProcessLimits, input: &[u8]) -> Result<(), ExecutionError> {
    let t = limits.transport.as_ref().ok_or(ExecutionError::Policy)?;
    if input.len() as u64 > limits.input_bytes
        || limits.output_bytes > 1024 * 1024
        || limits.output_bytes == 0
        || limits.cleanup_ms == 0
        || limits.cleanup_ms > 15000
        || t.stdout_bytes > limits.output_bytes
        || t.stderr_bytes > limits.output_bytes
        || limits.authority_poll_ms == 0
        || t.idle_ms == 0
    {
        return Err(ExecutionError::Policy);
    }
    Ok(())
}
fn observer(
    limits: &ProcessLimits,
    input: &[u8],
) -> Result<crate::structured_output::StructuredObserver, ExecutionError> {
    let t = limits.transport.as_ref().ok_or(ExecutionError::Policy)?;
    let capacity = t
        .stdout_bytes
        .checked_add(t.stderr_bytes)
        .ok_or(ExecutionError::Policy)?;
    crate::structured_output::StructuredObserver::new(limits, input.len() as u64, capacity)
        .map_err(|_| ExecutionError::Policy)
}
fn child_pipes(child: &mut Child) -> Result<Pipes, ExecutionError> {
    // Validate the complete set before taking ownership so partial initialization
    // does not silently discard another handle. Child remains caller-owned.
    if child.stdin.is_none() || child.stdout.is_none() || child.stderr.is_none() {
        return Err(ExecutionError::Policy);
    }
    Ok(Pipes {
        stdin: child
            .stdin
            .take()
            .map(|p| std::fs::File::from(OwnedFd::from(p))),
        stdout: child
            .stdout
            .take()
            .map(|p| std::fs::File::from(OwnedFd::from(p))),
        stderr: child
            .stderr
            .take()
            .map(|p| std::fs::File::from(OwnedFd::from(p))),
    })
}
fn elapsed(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}
fn drain(
    stream: &mut Option<std::fs::File>,
    out: &mut Vec<u8>,
    cap: usize,
    mut consume: impl FnMut(&[u8]) -> Result<(), InterruptionReason>,
) -> Result<(), InterruptionReason> {
    let mut buf = [0u8; 4096];
    if let Some(s) = stream {
        match s.read(&mut buf[..4096.min(cap.saturating_sub(out.len()).saturating_add(1))]) {
            Ok(0) => {
                *stream = None;
            }
            Ok(n) => {
                let fit = n.min(cap.saturating_sub(out.len()));
                out.extend_from_slice(&buf[..fit]);
                if fit < n {
                    return Err(InterruptionReason::OutputLimit);
                }
                consume(&buf[..n])?;
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err(InterruptionReason::Io),
        }
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn collect_io(
    process: &mut impl Process,
    mut pipes: Pipes,
    input: &[u8],
    limits: &ProcessLimits,
    start: Instant,
    end: Instant,
    mut current: impl FnMut() -> Result<(), InterruptionReason>,
    mut observer: Option<crate::structured_output::StructuredObserver>,
) -> Result<
    (
        CapturedStreams,
        Option<crate::structured_output::StructuredSummary>,
    ),
    ExecutionError,
> {
    let t = limits.transport.as_ref().ok_or(ExecutionError::Policy)?;
    for fd in [&pipes.stdin, &pipes.stdout, &pipes.stderr]
        .into_iter()
        .flatten()
    {
        nonblock(fd)?;
    }
    let collecting = Instant::now();
    let mut offset = 0;
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut reason = None;
    let mut status = None;
    // Raw mode preserves the D idle origin. Structured mode consumes preparation
    // elapsed time until the first actual I/O progress.
    let mut last = if observer.is_some() {
        start
    } else {
        collecting
    };
    if observer.is_some() {
        out.try_reserve_exact(t.stdout_bytes as usize)
            .map_err(|_| ExecutionError::Io(std::io::ErrorKind::OutOfMemory))?;
        err.try_reserve_exact(t.stderr_bytes as usize)
            .map_err(|_| ExecutionError::Io(std::io::ErrorKind::OutOfMemory))?;
        // Drop the validated preliminary frame before allocating its replacement;
        // peak retained buffers must not contain two frame capacities.
        drop(observer.take());
        observer = Some(
            crate::structured_output::StructuredObserver::new(
                limits,
                input.len() as u64,
                (out.capacity() as u64)
                    .checked_add(err.capacity() as u64)
                    .ok_or(ExecutionError::Policy)?,
            )
            .map_err(|_| ExecutionError::Policy)?,
        );
    }
    loop {
        if let Err(r) = current() {
            reason = Some(r);
            break;
        }
        if Instant::now() >= end {
            reason = Some(InterruptionReason::Timeout);
            break;
        }
        if let Some(o) = &mut observer
            && let Err(r) = o.check(elapsed(start))
        {
            reason = Some(r);
            break;
        }
        if last.elapsed() >= Duration::from_millis(t.idle_ms) {
            reason = Some(InterruptionReason::IdleTimeout);
            break;
        }
        let previous = offset + out.len() + err.len();
        if offset == input.len() {
            pipes.stdin = None;
        }
        if let Some(s) = &mut pipes.stdin {
            match s.write(&input[offset..]) {
                Ok(0) => {
                    reason = Some(InterruptionReason::InputClosed);
                    break;
                }
                Ok(n) => offset += n,
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                    reason = Some(InterruptionReason::InputClosed);
                    break;
                }
                Err(_) => {
                    reason = Some(InterruptionReason::Io);
                    break;
                }
            }
        }
        let cap =
            (t.stdout_bytes as usize).min((limits.output_bytes as usize).saturating_sub(err.len()));
        let observed = drain(&mut pipes.stdout, &mut out, cap, |bytes| {
            if let Some(o) = &mut observer {
                o.feed(bytes, elapsed(start))
            } else {
                Ok(())
            }
        });
        if let Err(r) = observed {
            reason = Some(
                if r == InterruptionReason::OutputLimit && cap == t.stdout_bytes as usize {
                    InterruptionReason::StdoutLimit
                } else {
                    r
                },
            );
            break;
        }
        if pipes.stdout.is_none()
            && let Some(o) = &mut observer
            && let Err(r) = o.finish(elapsed(start))
        {
            reason = Some(r);
            break;
        }
        let cap =
            (t.stderr_bytes as usize).min((limits.output_bytes as usize).saturating_sub(out.len()));
        if let Err(r) = drain(&mut pipes.stderr, &mut err, cap, |_| Ok(())) {
            reason = Some(
                if r == InterruptionReason::OutputLimit && cap == t.stderr_bytes as usize {
                    InterruptionReason::StderrLimit
                } else {
                    r
                },
            );
            break;
        }
        if status.is_none() {
            let query_end = end
                .min(Instant::now() + Duration::from_millis(limits.authority_poll_ms))
                .min(last + Duration::from_millis(t.idle_ms));
            let query_end = if observer
                .as_ref()
                .is_some_and(|o| o.summary().first_record_ms.is_none())
            {
                query_end.min(start + Duration::from_millis(t.startup_ms))
            } else {
                query_end
            };
            match process.poll(query_end) {
                Ok(s) => status = s,
                Err(_) => {
                    reason = Some(InterruptionReason::Io);
                    break;
                }
            }
        }
        if let Err(r) = current() {
            reason = Some(r);
            break;
        }
        if Instant::now() >= end {
            reason = Some(InterruptionReason::Timeout);
            break;
        }
        if let Some(o) = &mut observer
            && let Err(r) = o.check(elapsed(start))
        {
            reason = Some(r);
            break;
        }
        if pipes.stdout.is_none() && pipes.stderr.is_none() && status.is_some() {
            break;
        }
        if previous != offset + out.len() + err.len() {
            last = Instant::now();
        }
        if Instant::now() >= end {
            reason = Some(InterruptionReason::Timeout);
            break;
        }
        if let Some(o) = &mut observer
            && let Err(r) = o.check(elapsed(start))
        {
            reason = Some(r);
            break;
        }
        let mut fds = Vec::with_capacity(3);
        if let Some(s) = &pipes.stdin {
            fds.push(PollFd::new(s, PollFlags::OUT));
        }
        if let Some(s) = &pipes.stdout {
            fds.push(PollFd::new(s, PollFlags::IN));
        }
        if let Some(s) = &pipes.stderr {
            fds.push(PollFd::new(s, PollFlags::IN));
        }
        let wait = end
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(limits.authority_poll_ms.min(10)));
        match poll(
            &mut fds,
            Some(&Timespec::try_from(wait).map_err(|_| ExecutionError::Policy)?),
        ) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => {}
            Err(_) => {
                reason = Some(InterruptionReason::Io);
                break;
            }
        }
    }
    let stdout_complete = pipes.stdout.is_none();
    let stderr_complete = pipes.stderr.is_none();
    if let Some(r) = reason {
        if let Some(o) = &mut observer {
            o.interrupt(r);
        }
        pipes.stdin = None;
        if status.is_none() {
            process.interrupt();
        }
        let cleanup = Instant::now()
            .checked_add(Duration::from_millis(limits.cleanup_ms))
            .ok_or(ExecutionError::Policy)?;
        while status.is_none() && Instant::now() < cleanup {
            match process
                .poll(cleanup.min(Instant::now() + Duration::from_millis(limits.authority_poll_ms)))
            {
                Ok(s) => status = s,
                Err(_) => break,
            }
            if status.is_none() {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
    drop(pipes);
    let quiescence =
        if process.direct_reap() && status.is_some() && stdout_complete && stderr_complete {
            Quiescence::DirectChildReapedStreamsClosed
        } else {
            Quiescence::Unknown
        };
    let completion = reason.map_or_else(
        || status.unwrap_or(ProcessCompletion::Unknown),
        |reason| ProcessCompletion::Interrupted { reason },
    );
    Ok((
        CapturedStreams {
            stdout: out,
            stderr: err,
            completion,
            quiescence,
            input_complete: offset == input.len(),
            stdout_complete,
            stderr_complete,
            elapsed: collecting.elapsed(),
        },
        observer.map(|o| o.summary()),
    ))
}

#[cfg(test)]
mod managed_pipe_tests {
    use super::*;
    #[test]
    fn direct_exit_with_retained_writer_never_fabricates_eof_or_domain_closure() {
        let (child, controller) = crate::native_execution::managed::ServicePipes::create().unwrap();
        let fds = child.into_fds();
        rustix::io::write(&fds[1], b"{}\n").unwrap();
        // Models a manager retaining the writer after direct-process exit.
        let limits = ProcessLimits {
            wall_ms: 30,
            cleanup_ms: 10,
            input_bytes: 1,
            output_bytes: 64,
            frame_bytes: 32,
            authority_poll_ms: 1,
            executable_bytes: 1,
            arguments: 0,
            argument_bytes: 0,
            transport: Some(cortex_core::program::supervision::TransportLimits {
                startup_ms: 20,
                idle_ms: 20,
                stdout_bytes: 32,
                stderr_bytes: 32,
                records: 2,
                buffer_bytes: 16384,
            }),
        };
        let start = Instant::now();
        let (streams, summary) = collect_managed(
            controller,
            b"",
            &limits,
            start,
            start + Duration::from_millis(30),
            || Ok(()),
            |_| {
                Ok(Some(ProcessCompletion::Exited {
                    code: Some(0),
                    signal: None,
                }))
            },
            true,
        )
        .unwrap();
        assert_eq!(
            streams.completion,
            ProcessCompletion::Interrupted {
                reason: InterruptionReason::IdleTimeout
            }
        );
        assert!(!streams.stdout_complete);
        assert!(!streams.stderr_complete);
        assert_eq!(streams.quiescence, Quiescence::Unknown);
        assert_eq!(summary.unwrap().records, 1);
        drop(fds);
    }
}
