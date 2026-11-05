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
pub fn run_nonce_bootstrap(
    mut stream: UnixStream,
    request: &str,
    timeout: Duration,
) -> io::Result<()> {
    if !token(request) || timeout.is_zero() || timeout > Duration::from_secs(5) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let end = Instant::now()
        .checked_add(timeout)
        .ok_or(io::ErrorKind::InvalidInput)?;
    let pid = std::process::id();
    let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    frame_write(
        &mut stream,
        &json!([1, "hello", request, pid, ticks(pid)?, boot.trim()]),
        end,
    )?;
    let value = frame_read(&mut stream, end)?;
    let a = value.as_array().ok_or(io::ErrorKind::InvalidData)?;
    if a.len() != 4 || a[0] != 1 || a[1] != "release" || a[2] != request {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let nonce = a[3].as_str().ok_or(io::ErrorKind::InvalidData)?;
    if !token(nonce) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    frame_write(&mut stream, &json!([1, "nonce", request, nonce]), end)
}

/// Entrypoint for the fixed manager-launched executable. Only an absolute owned
/// endpoint, request token and finite timeout are accepted. Unexpected inherited
/// descriptors are refused before connection; all payload activity is fixed Rust.
pub fn nonce_bootstrap_main(args: &[String]) -> io::Result<()> {
    if args.len() != 3 || !Path::new(&args[0]).is_absolute() || !token(&args[1]) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let timeout: u64 = args[2].parse().map_err(|_| io::ErrorKind::InvalidInput)?;
    if timeout == 0 || timeout > 5000 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    // read_dir itself owns one descriptor. Its target is the current fd
    // directory; every other inherited descriptor above stderr is unsupported.
    let own_fd_dir = format!("/proc/{}/fd", std::process::id());
    for entry in fs::read_dir("/proc/self/fd")? {
        let entry = entry?;
        let number: u32 = entry
            .file_name()
            .to_str()
            .ok_or(io::ErrorKind::InvalidData)?
            .parse()
            .map_err(|_| io::ErrorKind::InvalidData)?;
        if number > 2 && fs::read_link(entry.path())? != Path::new(&own_fd_dir) {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
    }
    run_nonce_bootstrap(
        connect_now(Path::new(&args[0]))?,
        &args[1],
        Duration::from_millis(timeout),
    )
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
