//! Bounded capture for the fixed Git inspection operations, not a task runner.

use super::InspectionError;
use std::{
    io::Read,
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

// util-linux applies RLIMIT_AS before exec, retaining the direct child PID.
// The standard library has no safe pre-exec resource-limit API.
const ADDRESS_SPACE_BYTES: &str = "536870912";

pub(super) fn git_command() -> Result<Command, InspectionError> {
    if !cfg!(target_os = "linux") {
        return Err(InspectionError::Unsupported(
            "host resource limits (Linux required)",
        ));
    }
    let mut command = Command::new("/usr/bin/prlimit");
    command.args([
        &format!("--as={ADDRESS_SPACE_BYTES}"),
        "--core=0",
        "--",
        "git",
    ]);
    Ok(command)
}

pub(super) const STDOUT_LIMIT: usize = 4 * 1024 * 1024;
pub(super) const STDERR_LIMIT: usize = 16 * 1024;
pub(super) const PROCESS_TIMEOUT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(5);

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        // No unbounded wait, including when a descendant retains a pipe. This is
        // cleanup of the direct child, not process-tree isolation or cancellation.
        let _ = self.0.kill();
        let deadline = Instant::now() + Duration::from_secs(1);
        while matches!(self.0.try_wait(), Ok(None)) && Instant::now() < deadline {
            thread::sleep(POLL);
        }
    }
}

pub(super) struct Captured {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

fn read_bounded(mut reader: impl Read, limit: usize) -> Result<Vec<u8>, InspectionError> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let n = reader.read(&mut chunk).map_err(InspectionError::Io)?;
        if n == 0 {
            return Ok(bytes);
        }
        if bytes.len() + n > limit {
            return Err(InspectionError::Limit("Git output"));
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
}

pub(super) fn capture(
    command: &mut Command,
    deadline: Instant,
    stdout_limit: usize,
    stderr_limit: usize,
) -> Result<Captured, InspectionError> {
    if Instant::now() >= deadline {
        return Err(InspectionError::Limit("Git time"));
    }
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(InspectionError::Io)?;
    let mut child = ChildGuard(child);
    let stdout = child.0.stdout.take().ok_or(InspectionError::Malformed)?;
    let stderr = child.0.stderr.take().ok_or(InspectionError::Malformed)?;
    let (sender, receiver) = mpsc::sync_channel(2);
    for (is_stdout, reader, limit) in [
        (true, Box::new(stdout) as Box<dyn Read + Send>, stdout_limit),
        (
            false,
            Box::new(stderr) as Box<dyn Read + Send>,
            stderr_limit,
        ),
    ] {
        let sender = sender.clone();
        thread::Builder::new()
            .name("git-capture".into())
            .spawn(move || {
                let _ = sender.send((is_stdout, read_bounded(reader, limit)));
            })
            .map_err(InspectionError::Io)?;
    }
    drop(sender);
    let mut stdout = None;
    let mut stderr = None;
    loop {
        while let Ok((is_stdout, result)) = receiver.try_recv() {
            if is_stdout {
                stdout = Some(result?);
            } else {
                stderr = Some(result?);
            }
        }
        if Instant::now() >= deadline {
            return Err(InspectionError::Limit("Git time"));
        }
        if let Some(status) = child.0.try_wait().map_err(InspectionError::Io)?
            && stdout.is_some()
            && stderr.is_some()
        {
            return Ok(Captured {
                status,
                stdout: stdout.take().ok_or(InspectionError::Malformed)?,
                stderr: stderr.take().ok_or(InspectionError::Malformed)?,
            });
        }
        thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn git_child_receives_the_address_space_limit() {
        use std::os::unix::fs::PermissionsExt;
        let repo = crate::inspection::support::Repo::new();
        let fake_git = repo.path.join("git");
        std::fs::write(&fake_git, "#!/bin/sh\nulimit -v\nulimit -c\n").unwrap();
        std::fs::set_permissions(&fake_git, std::fs::Permissions::from_mode(0o755)).unwrap();
        let result = capture(
            git_command().unwrap().env("PATH", &repo.path),
            Instant::now() + Duration::from_secs(2),
            1024,
            1024,
        )
        .unwrap();
        assert!(result.status.success());
        assert!(result.stderr.is_empty());
        assert_eq!(result.stdout, b"524288\n0\n");
    }

    #[test]
    fn capture_accepts_exact_byte_limit_and_rejects_next_byte() {
        assert_eq!(read_bounded(&b"1234"[..], 4).unwrap(), b"1234");
        assert!(matches!(
            read_bounded(&b"12345"[..], 4),
            Err(InspectionError::Limit(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn real_process_time_and_both_output_limits_are_enforced() {
        // Fixed fixture commands only. Production never invokes a shell.
        for script in ["while :; do :; done", "sleep 2 & exit 0"] {
            let start = Instant::now();
            let result = capture(
                Command::new("sh").args(["-c", script]),
                start + Duration::from_millis(100),
                64,
                64,
            );
            assert!(matches!(result, Err(InspectionError::Limit("Git time"))));
            assert!(start.elapsed() < Duration::from_secs(2));
        }
        for script in ["printf 12345", "printf 12345 >&2"] {
            let result = capture(
                Command::new("sh").args(["-c", script]),
                Instant::now() + Duration::from_secs(2),
                4,
                4,
            );
            assert!(matches!(result, Err(InspectionError::Limit("Git output"))));
        }
    }
}
