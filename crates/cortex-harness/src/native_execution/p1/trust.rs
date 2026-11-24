//! Local principal verification for one contained host's trial protocol.
//!
//! The selected launcher must use an administrator-controlled mount and user
//! namespace. Configuration is read below a fixed system path. Each reply is
//! checked for the actual kernel-supplied sender credential; a connection
//! credential alone would not detect passing the peer descriptor to a new UID.
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fs::{Mode, OFlags, PROC_SUPER_MAGIC, ResolveFlags, fstatfs, open, openat2, readlinkat},
    io::{FdFlags, fcntl_getfd},
    net::{
        self, AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
        SendFlags, SocketType, sockopt,
    },
};
use std::{
    fs::File,
    io::{self, IoSliceMut, Read},
    mem::MaybeUninit,
    os::{fd::OwnedFd, unix::fs::MetadataExt},
    time::{Duration, Instant},
};

const PROFILE: &str = "p1-trial-trust-v1";
const PROFILE_BOUND: usize = 128;
pub(crate) const PACKET_BOUND: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    Operator,
    Observer,
}

pub(crate) struct TrustedChannel {
    socket: OwnedFd,
    uid: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Principals {
    operator: u32,
    observer: u32,
}

#[derive(PartialEq, Eq)]
pub(crate) struct TrialTrust {
    principals: Principals,
    device: u64,
    inode: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

fn denied() -> io::Error {
    io::ErrorKind::PermissionDenied.into()
}

fn remaining(end: Instant) -> io::Result<Duration> {
    end.checked_duration_since(Instant::now())
        .filter(|value| !value.is_zero())
        .ok_or_else(|| io::ErrorKind::TimedOut.into())
}

fn protected_dir(parent: &File, component: &str) -> io::Result<File> {
    let dir = File::from(openat2(
        parent,
        component,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )?);
    let metadata = dir.metadata()?;
    if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err(denied());
    }
    Ok(dir)
}

fn full_uid_map(bytes: &[u8]) -> bool {
    if bytes.len() > PROFILE_BOUND {
        return false;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    text.split_ascii_whitespace().eq(["0", "0", "4294967295"])
}

fn check_unremapped_uid_namespace(root: &File) -> io::Result<()> {
    let proc = File::from(openat2(
        root,
        "proc",
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )?);
    let pid = rustix::process::getpid().as_raw_pid().to_string();
    if fstatfs(&proc)?.f_type != PROC_SUPER_MAGIC
        || readlinkat(&proc, "self", Vec::new())?.as_bytes() != pid.as_bytes()
    {
        return Err(denied());
    }
    let same_proc = ResolveFlags::BENEATH
        | ResolveFlags::NO_SYMLINKS
        | ResolveFlags::NO_MAGICLINKS
        | ResolveFlags::NO_XDEV;
    let pid_dir = File::from(openat2(
        &proc,
        pid.as_str(),
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY,
        Mode::empty(),
        same_proc,
    )?);
    let map = File::from(openat2(
        &pid_dir,
        "uid_map",
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
        same_proc,
    )?);
    let mut bytes = Vec::with_capacity(PROFILE_BOUND + 1);
    map.take(PROFILE_BOUND as u64 + 1).read_to_end(&mut bytes)?;
    if !full_uid_map(&bytes) {
        return Err(denied());
    }
    Ok(())
}

fn principals(bytes: &[u8], runner: u32) -> io::Result<Principals> {
    let text = std::str::from_utf8(bytes).map_err(|_| denied())?;
    let ids: Vec<_> = text.split_ascii_whitespace().collect();
    let [protocol, operator, observer] = ids.as_slice() else {
        return Err(denied());
    };
    let operator = operator.parse::<u32>().map_err(|_| denied())?;
    let observer = observer.parse::<u32>().map_err(|_| denied())?;
    if *protocol != "cortex-p1-trust-v1"
        || runner == 0
        || operator == 0
        || observer == 0
        || operator == observer
        || operator == runner
        || observer == runner
        || operator == u32::MAX
        || observer == u32::MAX
    {
        return Err(denied());
    }
    Ok(Principals { operator, observer })
}

impl TrialTrust {
    pub(crate) fn load() -> io::Result<Self> {
        let runner = rustix::process::getuid().as_raw();
        if rustix::process::geteuid().as_raw() != runner || runner == 0 {
            return Err(denied());
        }
        let root = File::from(open(
            "/",
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY,
            Mode::empty(),
        )?);
        let metadata = root.metadata()?;
        if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return Err(denied());
        }
        check_unremapped_uid_namespace(&root)?;
        let cortex = protected_dir(&protected_dir(&root, "etc")?, "cortex")?;
        let profile = File::from(openat2(
            &cortex,
            PROFILE,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        )?);
        let metadata = profile.metadata()?;
        if !metadata.is_file()
            || metadata.uid() != 0
            || metadata.mode() & 0o022 != 0
            || metadata.nlink() != 1
            || metadata.len() > PROFILE_BOUND as u64
        {
            return Err(denied());
        }
        let mut bytes = Vec::with_capacity(PROFILE_BOUND + 1);
        (&profile)
            .take(PROFILE_BOUND as u64 + 1)
            .read_to_end(&mut bytes)?;
        let after = profile.metadata()?;
        if bytes.len() as u64 != metadata.len()
            || metadata.len() != after.len()
            || metadata.uid() != after.uid()
            || metadata.mode() != after.mode()
            || metadata.nlink() != after.nlink()
            || metadata.mtime() != after.mtime()
            || metadata.mtime_nsec() != after.mtime_nsec()
            || metadata.ctime() != after.ctime()
            || metadata.ctime_nsec() != after.ctime_nsec()
        {
            return Err(denied());
        }
        Ok(Self {
            principals: principals(&bytes, runner)?,
            device: metadata.dev(),
            inode: metadata.ino(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        })
    }

    pub(crate) fn check_current(&self) -> io::Result<()> {
        if *self != Self::load()? {
            return Err(denied());
        }
        Ok(())
    }

    pub(crate) fn channel(&self, fd: OwnedFd, role: Role) -> io::Result<TrustedChannel> {
        let uid = match role {
            Role::Operator => self.principals.operator,
            Role::Observer => self.principals.observer,
        };
        TrustedChannel::inspect(fd, uid)
    }

    pub(crate) fn receive(
        &self,
        channel: &TrustedChannel,
        bytes: &mut [u8],
        end: Instant,
    ) -> io::Result<usize> {
        self.check_current()?;
        let size = channel.receive(bytes, end)?;
        self.check_current()?;
        remaining(end)?;
        Ok(size)
    }
}

impl TrustedChannel {
    fn inspect(socket: OwnedFd, uid: u32) -> io::Result<Self> {
        if sockopt::socket_domain(&socket)? != AddressFamily::UNIX
            || sockopt::socket_type(&socket)? != SocketType::SEQPACKET
            || !fcntl_getfd(&socket)?.contains(FdFlags::CLOEXEC)
            || sockopt::socket_peercred(&socket)?.uid.as_raw() != uid
        {
            return Err(denied());
        }
        sockopt::set_socket_passcred(&socket, true)?;
        Ok(Self { socket, uid })
    }

    fn wait(&self, flags: PollFlags, end: Instant) -> io::Result<()> {
        loop {
            let limit = Timespec::try_from(remaining(end)?).map_err(|_| denied())?;
            let mut fds = [PollFd::new(&self.socket, flags)];
            match poll(&mut fds, Some(&limit)) {
                Ok(0) => return Err(io::ErrorKind::TimedOut.into()),
                Ok(_) if fds[0].revents() == flags => {
                    remaining(end)?;
                    return Ok(());
                }
                Ok(_) => return Err(denied()),
                Err(rustix::io::Errno::INTR) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }

    pub(super) fn reply_to(
        &self,
        trust: &TrialTrust,
        exact: &[u8],
        end: Instant,
    ) -> io::Result<()> {
        if exact.is_empty() || exact.len() > PACKET_BOUND {
            return Err(denied());
        }
        self.wait(PollFlags::OUT, end)?;
        let sent = net::send(
            &self.socket,
            exact,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        )?;
        if sent != exact.len() {
            return Err(denied());
        }
        let mut reply = [0; PACKET_BOUND];
        let size = trust.receive(self, &mut reply, end)?;
        if reply[..size] != *exact {
            return Err(denied());
        }
        Ok(())
    }

    pub(super) fn check_no_revocation(&self) -> io::Result<()> {
        let mut fds = [PollFd::new(&self.socket, PollFlags::IN)];
        let zero = Timespec::try_from(Duration::ZERO).map_err(|_| denied())?;
        if poll(&mut fds, Some(&zero))? != 0 {
            // Extra messages, disconnect, or I/O errors revoke this case.
            return Err(denied());
        }
        Ok(())
    }

    fn receive(&self, bytes: &mut [u8], end: Instant) -> io::Result<usize> {
        if bytes.is_empty() || bytes.len() > PACKET_BOUND {
            return Err(denied());
        }
        self.wait(PollFlags::IN, end)?;
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = net::recvmsg(
            &self.socket,
            &mut [IoSliceMut::new(bytes)],
            &mut ancillary,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        )?;
        let mut matching_sender = false;
        for entry in ancillary.drain() {
            match entry {
                RecvAncillaryMessage::ScmCredentials(credentials)
                    if !matching_sender
                        && credentials.uid.as_raw() == self.uid
                        && credentials.pid.as_raw_pid() > 0 =>
                {
                    matching_sender = true;
                }
                _ => return Err(denied()),
            }
        }
        remaining(end)?;
        if !matching_sender
            || received.bytes == 0
            || received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        {
            return Err(denied());
        }
        Ok(received.bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::net::SocketFlags;

    #[test]
    fn profile_rejects_aliases_and_the_runner_or_privileged_principal() {
        assert!(principals(b"cortex-p1-trust-v1 21 22", 20).is_ok());
        for invalid in [
            b"cortex-p1-trust-v1 21 21".as_slice(),
            b"cortex-p1-trust-v1 20 22",
            b"cortex-p1-trust-v1 21 20",
            b"cortex-p1-trust-v1 0 21",
            b"cortex-p1-trust-v1 22 0",
            b"cortex-p1-trust-v1 22 21 extra",
        ] {
            assert!(principals(invalid, 20).is_err());
        }
    }

    #[test]
    fn remapped_partial_and_extra_user_namespace_ranges_are_rejected() {
        assert!(full_uid_map(b"         0          0 4294967295\n"));
        for value in [
            b"0 1000 1\n".as_slice(),
            b"0 0 1\n1 1 4294967294\n",
            b"0 0 4294967295\n1000 1000 1\n",
            b"0 0 4294967296\n",
            b"1 1 4294967294\n",
        ] {
            assert!(!full_uid_map(value));
        }
    }

    fn pair() -> (OwnedFd, OwnedFd) {
        net::socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC,
            None,
        )
        .unwrap()
    }

    #[test]
    fn receives_exact_bounded_kernel_sender_and_detects_disconnect_or_queued_revocation() {
        let (server, peer) = pair();
        let own_uid = rustix::process::getuid().as_raw();
        let mut bytes = [0; 8];
        let inspected = TrustedChannel::inspect(server, own_uid).unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        assert!(inspected.check_no_revocation().is_ok());
        net::send(&peer, b"approve", SendFlags::NOSIGNAL).unwrap();
        assert!(inspected.check_no_revocation().is_err());
        assert_eq!(inspected.receive(&mut bytes, deadline).unwrap(), 7);
        assert_eq!(&bytes[..7], b"approve");

        net::send(&peer, b"truncated packet", SendFlags::NOSIGNAL).unwrap();
        assert!(inspected.receive(&mut bytes, deadline).is_err());
        drop(peer);
        assert!(inspected.check_no_revocation().is_err());

        let (server, _peer) = pair();
        assert!(TrustedChannel::inspect(server, own_uid.wrapping_add(1)).is_err());
    }
}
