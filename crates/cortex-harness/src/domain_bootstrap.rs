//! Fixed provider-free bootstrap protocol. There is no executable, shell,
//! provider, environment or credential field in the release frame.

use serde_json::{Value, json};
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::{Duration, Instant},
};

pub(crate) const FRAME_LIMIT: usize = 4096;

pub(crate) fn token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

pub(crate) fn ticks(pid: u32) -> io::Result<u64> {
    let text = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let tail = text.rsplit_once(')').ok_or(io::ErrorKind::InvalidData)?.1;
    tail.split_whitespace()
        .nth(19)
        .ok_or(io::ErrorKind::InvalidData)?
        .parse()
        .map_err(|_| io::ErrorKind::InvalidData.into())
}

pub(crate) fn frame_write(stream: &mut UnixStream, value: &Value, end: Instant) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|_| io::ErrorKind::InvalidData)?;
    if bytes.len() > FRAME_LIMIT {
        return Err(io::ErrorKind::InvalidData.into());
    }
    write_until(stream, &(bytes.len() as u32).to_be_bytes(), end)?;
    write_until(stream, &bytes, end)
}

pub(crate) fn frame_read(stream: &mut UnixStream, end: Instant) -> io::Result<Value> {
    let mut length = [0; 4];
    read_until(stream, &mut length, end)?;
    let length = u32::from_be_bytes(length) as usize;
    if length == 0 || length > FRAME_LIMIT {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut bytes = vec![0; length];
    read_until(stream, &mut bytes, end)?;
    serde_json::from_slice(&bytes).map_err(|_| io::ErrorKind::InvalidData.into())
}

fn read_until(stream: &mut UnixStream, mut bytes: &mut [u8], end: Instant) -> io::Result<()> {
    while !bytes.is_empty() {
        stream.set_read_timeout(Some(remaining(end)?))?;
        match stream.read(bytes) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => bytes = &mut bytes[n..],
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    remaining(end).map(|_| ())
}

fn write_until(stream: &mut UnixStream, mut bytes: &[u8], end: Instant) -> io::Result<()> {
    while !bytes.is_empty() {
        stream.set_write_timeout(Some(remaining(end)?))?;
        match stream.write(bytes) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(n) => bytes = &bytes[n..],
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    remaining(end).map(|_| ())
}

pub(crate) fn connect_now(path: &Path) -> io::Result<UnixStream> {
    use rustix::net::{
        AddressFamily, SocketAddrUnix, SocketFlags, SocketType, connect, socket_with,
    };
    let socket = socket_with(
        AddressFamily::UNIX,
        SocketType::STREAM,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )?;
    // A full backlog or incomplete handshake is refused, never waited on or
    // retried. Closing this descriptor leaves no workload release capability.
    connect(&socket, &SocketAddrUnix::new(path)?)?;
    let stream = UnixStream::from(socket);
    stream.set_nonblocking(false)?;
    Ok(stream)
}

fn remaining(end: Instant) -> io::Result<Duration> {
    end.checked_duration_since(Instant::now())
        .filter(|v| !v.is_zero())
        .ok_or(io::ErrorKind::TimedOut.into())
}

/// Run the fixed nonce bootstrap on an already connected, controller-owned
/// channel. The caller must establish the outer domain before launching this
/// trusted bootstrap. This function neither launches a payload nor authenticates
/// a manager; it emits one public nonce only after the exact release frame.
/// No arbitrary command, environment, private package or provider is supported.
pub fn run_nonce_bootstrap(stream: UnixStream, request: &str, timeout: Duration) -> io::Result<()> {
    let end = deadline(timeout).map_err(as_io)?;
    run_until(stream, request, end).map_err(as_io)
}

/// Redacted startup/protocol phase. No path, descriptor target or input is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootstrapPhase {
    /// Invalid fixed invocation arguments.
    Arguments,
    /// Unexpected descriptor or descriptor enumeration failure.
    Descriptors,
    /// Bounded local connection refused or failed.
    Connect,
    /// Own-process identity could not be read.
    Identity,
    /// Initial observation could not be sent.
    Hello,
    /// Exact release was missing, late or invalid.
    Release,
    /// Fixed nonce observation could not be sent.
    Result,
}
/// Finite diagnostic containing only a phase, error category and optional OS number.
#[derive(Clone, Copy, Debug)]
pub struct BootstrapFailure {
    /// Boundary at which the invocation stopped.
    pub phase: BootstrapPhase,
    /// Original I/O category, without private diagnostic text.
    pub kind: io::ErrorKind,
    /// Numeric OS error where available; no raw input is retained.
    pub os_error: Option<i32>,
}
impl std::fmt::Display for BootstrapFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "phase={:?} kind={:?} os={:?}",
            self.phase, self.kind, self.os_error
        )
    }
}
impl std::error::Error for BootstrapFailure {}
fn failure(phase: BootstrapPhase, e: io::Error) -> BootstrapFailure {
    BootstrapFailure {
        phase,
        kind: e.kind(),
        os_error: e.raw_os_error(),
    }
}
fn as_io(e: BootstrapFailure) -> io::Error {
    io::Error::new(e.kind, e)
}
fn deadline(timeout: Duration) -> Result<Instant, BootstrapFailure> {
    if timeout.is_zero() || timeout > Duration::from_secs(5) {
        return Err(failure(
            BootstrapPhase::Arguments,
            io::ErrorKind::InvalidInput.into(),
        ));
    }
    Instant::now().checked_add(timeout).ok_or_else(|| {
        failure(
            BootstrapPhase::Arguments,
            io::ErrorKind::InvalidInput.into(),
        )
    })
}
fn run_until(mut stream: UnixStream, request: &str, end: Instant) -> Result<(), BootstrapFailure> {
    use BootstrapPhase::*;
    if !token(request) {
        return Err(failure(Arguments, io::ErrorKind::InvalidInput.into()));
    }
    let pid = std::process::id();
    let boot =
        fs::read_to_string("/proc/sys/kernel/random/boot_id").map_err(|e| failure(Identity, e))?;
    let ticks = ticks(pid).map_err(|e| failure(Identity, e))?;
    frame_write(
        &mut stream,
        &json!([1, "hello", request, pid, ticks, boot.trim()]),
        end,
    )
    .map_err(|e| failure(Hello, e))?;
    let value = frame_read(&mut stream, end).map_err(|e| failure(Release, e))?;
    let a = value
        .as_array()
        .ok_or_else(|| failure(Release, io::ErrorKind::InvalidData.into()))?;
    if a.len() != 4 || a[0] != 1 || a[1] != "release" || a[2] != request {
        return Err(failure(Release, io::ErrorKind::InvalidData.into()));
    }
    let nonce = a[3]
        .as_str()
        .filter(|n| token(n))
        .ok_or_else(|| failure(Release, io::ErrorKind::InvalidData.into()))?;
    frame_write(&mut stream, &json!([1, "nonce", request, nonce]), end)
        .map_err(|e| failure(Result, e))
}

/// Compatible I/O entrypoint; see [`nonce_bootstrap_diagnosed`] for redacted phases.
pub fn nonce_bootstrap_main(args: &[String]) -> io::Result<()> {
    nonce_bootstrap_diagnosed(args).map_err(as_io)
}

/// Fixed manager-launched entrypoint with bounded, redacted error information.
/// Only the exact descriptor owned by enumeration is exempt from rejection.
/// Startup, enumeration, connection and protocol share one absolute deadline.
pub fn nonce_bootstrap_diagnosed(args: &[String]) -> Result<(), BootstrapFailure> {
    use BootstrapPhase::*;
    use std::os::fd::AsRawFd;
    let start = Instant::now();
    if args.len() != 3 || !Path::new(&args[0]).is_absolute() || !token(&args[1]) {
        return Err(failure(Arguments, io::ErrorKind::InvalidInput.into()));
    }
    let timeout: u64 = args[2]
        .parse()
        .map_err(|_| failure(Arguments, io::ErrorKind::InvalidInput.into()))?;
    if timeout == 0 || timeout > 5000 {
        return Err(failure(Arguments, io::ErrorKind::InvalidInput.into()));
    }
    let end = start
        .checked_add(Duration::from_millis(timeout))
        .ok_or_else(|| failure(Arguments, io::ErrorKind::InvalidInput.into()))?;
    {
        let fd = fs::File::open("/proc/self/fd").map_err(|e| failure(Descriptors, e))?;
        let mut directory = rustix::fs::Dir::new(fd).map_err(|e| failure(Descriptors, e.into()))?;
        let own = directory
            .fd()
            .map_err(|e| failure(Descriptors, e.into()))?
            .as_raw_fd();
        let mut count = 0;
        while let Some(entry) = directory.read() {
            remaining(end).map_err(|e| failure(Descriptors, e))?;
            count += 1;
            if count > 256 {
                return Err(failure(Descriptors, io::ErrorKind::InvalidData.into()));
            }
            let entry = entry.map_err(|e| failure(Descriptors, e.into()))?;
            let name = entry.file_name().to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            let number: i32 = std::str::from_utf8(name)
                .ok()
                .and_then(|n| n.parse().ok())
                .ok_or_else(|| failure(Descriptors, io::ErrorKind::InvalidData.into()))?;
            if number > 2 && number != own {
                return Err(failure(Descriptors, io::ErrorKind::PermissionDenied.into()));
            }
        }
    }
    remaining(end).map_err(|e| failure(Connect, e))?;
    let stream = connect_now(Path::new(&args[0])).map_err(|e| failure(Connect, e))?;
    remaining(end).map_err(|e| failure(Connect, e))?;
    run_until(stream, &args[1], end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn fixed_payload_waits_for_exact_release_and_emits_only_nonce() {
        let (mut controller, bootstrap) = UnixStream::pair().unwrap();
        let worker = thread::spawn(move || {
            run_nonce_bootstrap(bootstrap, "request-1", Duration::from_secs(1))
        });
        let end = Instant::now() + Duration::from_secs(1);
        let hello = frame_read(&mut controller, end).unwrap();
        assert_eq!(hello[1], "hello");
        controller.set_nonblocking(true).unwrap();
        assert_eq!(
            controller.read(&mut [0; 1]).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        controller.set_nonblocking(false).unwrap();
        frame_write(
            &mut controller,
            &json!([1, "release", "request-1", "public-7"]),
            end,
        )
        .unwrap();
        assert_eq!(
            frame_read(&mut controller, end).unwrap(),
            json!([1, "nonce", "request-1", "public-7"])
        );
        worker.join().unwrap().unwrap();
    }

    #[test]
    fn wrong_request_and_command_fields_are_not_payloads() {
        for release in [
            json!([1, "release", "other", "nonce"]),
            json!([1, "exec", "request-1", "/bin/sh"]),
            json!([1, "release", "request-1", "nonce", "extra"]),
        ] {
            let (mut controller, bootstrap) = UnixStream::pair().unwrap();
            let worker = thread::spawn(move || {
                run_nonce_bootstrap(bootstrap, "request-1", Duration::from_secs(1))
            });
            let end = Instant::now() + Duration::from_secs(1);
            frame_read(&mut controller, end).unwrap();
            frame_write(&mut controller, &release, end).unwrap();
            assert!(worker.join().unwrap().is_err());
            assert_eq!(controller.read(&mut [0; 1]).unwrap(), 0);
        }
    }

    #[test]
    fn lost_controller_and_oversized_frame_never_release() {
        let (mut controller, bootstrap) = UnixStream::pair().unwrap();
        let worker =
            thread::spawn(move || run_nonce_bootstrap(bootstrap, "r", Duration::from_secs(1)));
        frame_read(&mut controller, Instant::now() + Duration::from_secs(1)).unwrap();
        controller.write_all(&4097_u32.to_be_bytes()).unwrap();
        assert!(worker.join().unwrap().is_err());
        let (mut controller, bootstrap) = UnixStream::pair().unwrap();
        let worker =
            thread::spawn(move || run_nonce_bootstrap(bootstrap, "r", Duration::from_secs(1)));
        frame_read(&mut controller, Instant::now() + Duration::from_secs(1)).unwrap();
        drop(controller);
        assert!(worker.join().unwrap().is_err());
    }
}
