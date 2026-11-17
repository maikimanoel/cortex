//! Exact bounded ancillary-FD delivery on the existing held bootstrap channel.
//! This transfers mechanics only; the canonical caller owns all authorization.
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fs::{OFlags, fcntl_getfl},
    io::{FdFlags, fcntl_getfd},
    net::{
        self, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
        SendAncillaryBuffer, SendAncillaryMessage, SendFlags,
    },
};
use std::{
    io::{self, IoSlice, IoSliceMut, Read, Write},
    mem::MaybeUninit,
    os::{
        fd::{AsFd, AsRawFd, OwnedFd},
        unix::net::UnixStream,
    },
    time::Instant,
};
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid standard-stream handoff",
    )
}
fn remaining(end: Instant) -> io::Result<std::time::Duration> {
    end.checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))
}
fn wait(channel: &UnixStream, flags: PollFlags, end: Instant) -> io::Result<()> {
    loop {
        let timeout = Timespec::try_from(remaining(end)?).map_err(|_| invalid())?;
        match poll(&mut [PollFd::new(channel, flags)], Some(&timeout)) {
            Ok(0) => return Err(io::ErrorKind::TimedOut.into()),
            Ok(_) => {
                remaining(end)?;
                return Ok(());
            }
            Err(rustix::io::Errno::INTR) => {}
            Err(e) => return Err(e.into()),
        }
    }
}
pub(crate) fn validate(fds: &[OwnedFd; 3]) -> io::Result<()> {
    let mut identities = [(0, 0); 3];
    for (index, fd) in fds.iter().enumerate() {
        let stat = rustix::fs::fstat(fd)?;
        let mode = fcntl_getfl(fd)? & OFlags::ACCMODE;
        if fd.as_raw_fd() <= 2
            || stat.st_mode & 0o170000 != 0o010000
            || mode
                != if index == 0 {
                    OFlags::RDONLY
                } else {
                    OFlags::WRONLY
                }
            || !fcntl_getfd(fd)?.contains(FdFlags::CLOEXEC)
        {
            return Err(invalid());
        }
        identities[index] = (stat.st_dev, stat.st_ino);
    }
    if identities[0] == identities[1]
        || identities[0] == identities[2]
        || identities[1] == identities[2]
    {
        return Err(invalid());
    }
    Ok(())
}
/// Send once, then await an installation acknowledgment. An uncertain send/ack
/// must never be retried by the manager adapter with regenerated pipe handles.
pub(crate) fn send(channel: &mut UnixStream, fds: &[OwnedFd; 3], end: Instant) -> io::Result<()> {
    validate(fds)?;
    let borrowed = fds.each_ref().map(AsFd::as_fd);
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
    let mut ancillary = SendAncillaryBuffer::new(&mut space);
    if !ancillary.push(SendAncillaryMessage::ScmRights(&borrowed)) {
        return Err(invalid());
    }
    wait(channel, PollFlags::OUT, end)?;
    let n = net::sendmsg(
        &*channel,
        &[IoSlice::new(b"F")],
        &mut ancillary,
        SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
    )?;
    if n != 1 {
        return Err(invalid());
    }
    let mut ack = [0; 1];
    channel.set_read_timeout(Some(remaining(end)?))?;
    if channel.read(&mut ack)? != 1 {
        return Err(invalid());
    }
    remaining(end)?;
    if ack != *b"A" {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn receive(channel: &UnixStream, end: Instant) -> io::Result<[OwnedFd; 3]> {
    wait(channel, PollFlags::IN, end)?;
    // Room for one excess descriptor makes exact-count rejection explicit.
    // CTRUNC rejects larger messages; rustix owns/closes all received rights.
    let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
    let mut ancillary = RecvAncillaryBuffer::new(&mut space);
    let mut byte = [0; 1];
    let result = net::recvmsg(
        channel,
        &mut [IoSliceMut::new(&mut byte)],
        &mut ancillary,
        RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
    )?;
    let mut fds = Vec::with_capacity(4);
    for message in ancillary.drain() {
        match message {
            RecvAncillaryMessage::ScmRights(rights) => fds.extend(rights),
            _ => return Err(invalid()),
        }
    }
    if result.bytes != 1
        || byte != *b"F"
        || result
            .flags
            .intersects(ReturnFlags::CTRUNC | ReturnFlags::TRUNC)
    {
        return Err(invalid());
    }
    let fds: [OwnedFd; 3] = fds.try_into().map_err(|_| invalid())?;
    validate(&fds)?;
    remaining(end)?;
    Ok(fds)
}
/// Called only by the dedicated single-threaded executable bootstrap. The
/// ordinary multithreaded controller/tests never replace their standard streams.
pub(crate) fn install(channel: &mut UnixStream, end: Instant) -> io::Result<()> {
    let fds = receive(channel, end)?;
    rustix::stdio::dup2_stdin(&fds[0])?;
    rustix::stdio::dup2_stdout(&fds[1])?;
    rustix::stdio::dup2_stderr(&fds[2])?;
    drop(fds);
    channel.set_write_timeout(Some(remaining(end)?))?;
    if channel.write(b"A")? != 1 {
        return Err(invalid());
    }
    remaining(end)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn pipes() -> ([OwnedFd; 3], [OwnedFd; 3]) {
        use rustix::pipe::{PipeFlags, pipe_with};
        let (ir, iw) = pipe_with(PipeFlags::CLOEXEC).unwrap();
        let (or, ow) = pipe_with(PipeFlags::CLOEXEC).unwrap();
        let (er, ew) = pipe_with(PipeFlags::CLOEXEC).unwrap();
        ([ir, ow, ew], [iw, or, er])
    }
    #[test]
    fn exact_handoff_has_owned_roles_and_no_retained_manager_writer() {
        let (mut a, mut b) = UnixStream::pair().unwrap();
        let (fds, other) = pipes();
        let end = Instant::now() + std::time::Duration::from_millis(200);
        let worker = std::thread::spawn(move || {
            let received = receive(&b, end).unwrap();
            assert_eq!(rustix::io::write(&received[1], b"x").unwrap(), 1);
            b.write_all(b"A").unwrap();
            drop(received);
        });
        send(&mut a, &fds, end).unwrap();
        drop(fds);
        worker.join().unwrap();
        let mut out = std::fs::File::from(other.into_iter().nth(1).unwrap());
        let mut bytes = Vec::new();
        out.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"x");
    }
    #[test]
    fn wrong_roles_duplicate_and_missing_rights_refuse() {
        let (fds, _) = pipes();
        assert!(
            validate(&[
                fds[0].try_clone().unwrap(),
                fds[1].try_clone().unwrap(),
                fds[1].try_clone().unwrap()
            ])
            .is_err()
        );
        assert!(
            validate(&[
                fds[1].try_clone().unwrap(),
                fds[0].try_clone().unwrap(),
                fds[2].try_clone().unwrap()
            ])
            .is_err()
        );
        let (mut a, b) = UnixStream::pair().unwrap();
        a.write_all(b"F").unwrap();
        assert_eq!(
            receive(&b, Instant::now() + std::time::Duration::from_millis(30))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
    #[test]
    fn closed_or_original_expired_channel_cannot_supply_rights() {
        let (a, b) = UnixStream::pair().unwrap();
        drop(a);
        assert!(receive(&b, Instant::now() + std::time::Duration::from_millis(10)).is_err());
        let (a, _b) = UnixStream::pair().unwrap();
        assert_eq!(
            receive(&a, Instant::now()).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
    }
    #[test]
    fn excess_truncated_duplicate_and_wrong_tag_rights_are_rejected() {
        for count in [0usize, 2, 3, 4, 6] {
            let (a, b) = UnixStream::pair().unwrap();
            let (fds, _other) = pipes();
            let rights = (0..count).map(|i| fds[i % 3].as_fd()).collect::<Vec<_>>();
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(6))];
            let mut control = SendAncillaryBuffer::new(&mut space);
            if count > 0 {
                assert!(control.push(SendAncillaryMessage::ScmRights(&rights)));
            }
            // Exact-three case deliberately uses a wrong data tag. Six causes
            // control truncation; all acquired OwnedFd values close on refusal.
            let tag = if count == 3 { b"X" } else { b"F" };
            net::sendmsg(&a, &[IoSlice::new(tag)], &mut control, SendFlags::NOSIGNAL).unwrap();
            assert_eq!(
                receive(&b, Instant::now() + std::time::Duration::from_millis(30))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }
    }
}
