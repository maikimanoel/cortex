//! Finite raw standard-stream pumping for an already owned trusted child.
//! This component does not launch, authorize or claim whole-domain containment.
//! Only input, aggregate/per-stream bytes, idle, authority polling and wall/cleanup
//! bounds are consumed from ProcessLimits. JSON frame/record and startup-record
//! requirements are not implemented by raw transport and confer no capability.
//! A caller requiring structured/startup-record validation must add that consumer
//! before claiming its full executor profile has been satisfied.
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
    os::{fd::AsFd, unix::process::ExitStatusExt},
    process::{Child, ChildStderr, ChildStdin, ChildStdout},
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
fn drain<R: Read>(stream: &mut Option<R>, out: &mut Vec<u8>, cap: usize) -> std::io::Result<bool> {
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
                    return Ok(false);
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}
/// Feed exact bounded input while draining both output pipes. One original end
/// instant covers release and execution; cleanup has its explicit separate bound.
/// The callback checks current canonical authority/cancellation and may refuse.
/// On interruption only the exclusive unreaped direct child is killed here.
/// Whole-group cancellation must separately use a verified owned cgroup.
pub fn collect(
    child: &mut Child,
    input: &[u8],
    limits: &ProcessLimits,
    end: Instant,
    mut current: impl FnMut() -> Result<(), InterruptionReason>,
) -> Result<CapturedStreams, ExecutionError> {
    let Some(t) = &limits.transport else {
        return Err(ExecutionError::Policy);
    };
    if input.len() as u64 > limits.input_bytes
        || limits.output_bytes > 1024 * 1024
        || limits.output_bytes == 0
        || limits.cleanup_ms == 0
        || limits.cleanup_ms > 15000
        || t.stdout_bytes > limits.output_bytes
        || t.stderr_bytes > limits.output_bytes
        || limits.authority_poll_ms == 0
    {
        return Err(ExecutionError::Policy);
    }
    let mut stdin: Option<ChildStdin> = child.stdin.take();
    let mut stdout: Option<ChildStdout> = child.stdout.take();
    let mut stderr: Option<ChildStderr> = child.stderr.take();
    if stdin.is_none() || stdout.is_none() || stderr.is_none() {
        return Err(ExecutionError::Policy);
    }
    nonblock(stdin.as_ref().unwrap())?;
    nonblock(stdout.as_ref().unwrap())?;
    nonblock(stderr.as_ref().unwrap())?;
    let start = Instant::now();
    let mut offset = 0;
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut reason = None;
    let mut status = None;
    let mut last = start;
    loop {
        if let Err(r) = current() {
            reason = Some(r);
            break;
        }
        if Instant::now() >= end {
            reason = Some(InterruptionReason::Timeout);
            break;
        }
        let previous = offset + out.len() + err.len();
        if offset == input.len() {
            stdin = None;
        }
        if let Some(s) = &mut stdin {
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
        let out_cap =
            (t.stdout_bytes as usize).min((limits.output_bytes as usize).saturating_sub(err.len()));
        let err_cap =
            (t.stderr_bytes as usize).min((limits.output_bytes as usize).saturating_sub(out.len()));
        match drain(&mut stdout, &mut out, out_cap) {
            Ok(true) => {}
            Ok(false) => {
                reason = Some(if out_cap < t.stdout_bytes as usize {
                    InterruptionReason::OutputLimit
                } else {
                    InterruptionReason::StdoutLimit
                });
                break;
            }
            Err(_) => {
                reason = Some(InterruptionReason::Io);
                break;
            }
        }
        let err_cap = err_cap.min((limits.output_bytes as usize).saturating_sub(out.len()));
        match drain(&mut stderr, &mut err, err_cap) {
            Ok(true) => {}
            Ok(false) => {
                reason = Some(if err_cap < t.stderr_bytes as usize {
                    InterruptionReason::OutputLimit
                } else {
                    InterruptionReason::StderrLimit
                });
                break;
            }
            Err(_) => {
                reason = Some(InterruptionReason::Io);
                break;
            }
        }
        if out.len() + err.len() > limits.output_bytes as usize {
            reason = Some(InterruptionReason::OutputLimit);
            break;
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(s) => status = s,
                Err(_) => {
                    reason = Some(InterruptionReason::Io);
                    break;
                }
            }
        }
        if stdout.is_none() && stderr.is_none() && status.is_some() {
            break;
        }
        if previous != offset + out.len() + err.len() {
            last = Instant::now();
        }
        if t.idle_ms == 0 || last.elapsed() >= Duration::from_millis(t.idle_ms) {
            reason = Some(InterruptionReason::IdleTimeout);
            break;
        }
        let mut fds = Vec::with_capacity(3);
        if let Some(s) = &stdin {
            fds.push(PollFd::new(s, PollFlags::OUT));
        }
        if let Some(s) = &stdout {
            fds.push(PollFd::new(s, PollFlags::IN));
        }
        if let Some(s) = &stderr {
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
    let stdout_complete = stdout.is_none();
    let stderr_complete = stderr.is_none();
    if reason.is_some() {
        stdin = None;
        if status.is_none() {
            let _ = child.kill();
        }
        let cleanup = Instant::now() + Duration::from_millis(limits.cleanup_ms);
        while status.is_none() && Instant::now() < cleanup {
            match child.try_wait() {
                Ok(s) => status = s,
                Err(_) => break,
            }
            if status.is_none() {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
    drop(stdin);
    drop(stdout);
    drop(stderr);
    let quiescence = if status.is_some() && stdout_complete && stderr_complete {
        Quiescence::DirectChildReapedStreamsClosed
    } else {
        Quiescence::Unknown
    };
    let completion = match reason {
        Some(reason) => ProcessCompletion::Interrupted { reason },
        None => match status {
            Some(s) => ProcessCompletion::Exited {
                code: s.code(),
                signal: s.signal(),
            },
            None => ProcessCompletion::Unknown,
        },
    };
    Ok(CapturedStreams {
        stdout: out,
        stderr: err,
        completion,
        quiescence,
        input_complete: offset == input.len(),
        stdout_complete,
        stderr_complete,
        elapsed: start.elapsed(),
    })
}
