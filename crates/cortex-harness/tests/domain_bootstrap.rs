//! Finite trusted development processes. No real manager, cgroup or namespace.
#![cfg(target_os = "linux")]
use nix::{
    spawn::{PosixSpawnAttr, PosixSpawnFileActions, posix_spawn},
    sys::{
        signal::{Signal, kill},
        wait::{WaitPidFlag, WaitStatus, waitpid},
    },
    unistd::Pid,
};
use serde_json::{Value, json};
use std::{
    ffi::CString,
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStrExt,
            fs::PermissionsExt,
            net::{UnixListener, UnixStream},
        },
    },
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

// Child-side POSIX file actions preserve the parent/Cargo descriptor table.
// This is a trusted fixture, not an arbitrary-executable containment backend.
struct Fixture {
    root: PathBuf,
    listener: UnixListener,
    child: Option<Pid>,
    end: Instant,
    cleaned: bool,
}
impl Fixture {
    fn start(extra: Option<&File>, timeout: &str) -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "cortex-bootstrap-child-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let endpoint = root.join("socket");
        let listener = UnixListener::bind(&endpoint).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut fixture = Self {
            root,
            listener,
            child: None,
            end: Instant::now() + Duration::from_secs(2),
            cleaned: false,
        };
        let null = File::options()
            .read(true)
            .write(true)
            .open("/dev/null")
            .unwrap();
        let diagnostic = File::create(fixture.root.join("diagnostic")).unwrap();
        let mut actions = PosixSpawnFileActions::init().unwrap();
        for fd in [0, 1] {
            actions.add_dup2(null.as_raw_fd(), fd).unwrap();
        }
        actions.add_dup2(diagnostic.as_raw_fd(), 2).unwrap();
        if let Some(extra) = extra {
            actions.add_dup2(extra.as_raw_fd(), 3).unwrap();
        }
        // Enumerate a bounded snapshot; no inherited descriptor is ever changed
        // in this multithreaded parent. New std-owned descriptors are CLOEXEC.
        let mut descriptors = Vec::new();
        for entry in fs::read_dir("/proc/self/fd").unwrap().take(257) {
            let fd: i32 = entry
                .unwrap()
                .file_name()
                .to_str()
                .unwrap()
                .parse()
                .unwrap();
            descriptors.push(fd);
        }
        assert!(descriptors.len() <= 256);
        for fd in descriptors {
            if fd > 2 && !(fd == 3 && extra.is_some()) {
                actions.add_close(fd).unwrap();
            }
        }
        let binary = CString::new(env!("CARGO_BIN_EXE_cortex-domain-bootstrap")).unwrap();
        let args = [
            binary.clone(),
            CString::new(endpoint.as_os_str().as_bytes()).unwrap(),
            CString::new("request-1").unwrap(),
            CString::new(timeout).unwrap(),
        ];
        let environment = [CString::new("CORTEX_SYNTHETIC_NEIGHBOR=public").unwrap()];
        fixture.child = Some(
            posix_spawn(
                binary.as_c_str(),
                &actions,
                &PosixSpawnAttr::init().unwrap(),
                &args,
                &environment,
            )
            .unwrap(),
        );
        fixture
    }
    fn diagnostic(&self) -> String {
        let mut bytes = Vec::new();
        File::open(self.root.join("diagnostic"))
            .unwrap()
            .take(257)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(
            bytes.len() <= 256,
            "fixed bootstrap diagnostic exceeded its bound"
        );
        String::from_utf8(bytes).unwrap()
    }
    fn poll(&mut self) -> Option<WaitStatus> {
        let child = self.child?;
        match waitpid(child, Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::StillAlive) | Err(nix::errno::Errno::EINTR) => None,
            Ok(status) => {
                self.child = None;
                Some(status)
            }
            Err(error) => panic!("fixture wait errno={error:?}"),
        }
    }
    fn accept(&mut self) -> UnixStream {
        loop {
            match self.listener.accept() {
                Ok((s, _)) => return s,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if let Some(status) = self.poll() {
                        panic!(
                            "bootstrap exited before connect: {status:?}; {}",
                            self.diagnostic()
                        );
                    }
                    assert!(
                        Instant::now() < self.end,
                        "bootstrap connect deadline; {}",
                        self.diagnostic()
                    );
                    thread::yield_now();
                }
                Err(e) => panic!("accept category={:?}", e.kind()),
            }
        }
    }
    fn wait(&mut self) -> WaitStatus {
        loop {
            if let Some(status) = self.poll() {
                return status;
            }
            assert!(
                Instant::now() < self.end,
                "bootstrap exit deadline; {}",
                self.diagnostic()
            );
            thread::yield_now();
        }
    }
    fn no_connection(&self) {
        assert_eq!(
            self.listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}
impl Fixture {
    fn finish(&mut self) {
        assert!(
            self.child.is_none(),
            "finish requires an observed terminal child status"
        );
        fs::remove_dir_all(&self.root)
            .map_err(|e| e.kind())
            .expect("fixture endpoint cleanup category");
        self.cleaned = true;
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.cleaned {
            return;
        }
        let mut failure = None;
        // Only the exact newly spawned, unreaped child can be signaled here.
        if let Some(pid) = self.child {
            if let Err(error) = kill(pid, Signal::SIGKILL) {
                failure = Some(format!("kill errno={error:?}"));
            }
            let end = Instant::now() + Duration::from_secs(1);
            loop {
                match waitpid(pid, Some(WaitPidFlag::WNOHANG)) {
                    Ok(WaitStatus::StillAlive) | Err(nix::errno::Errno::EINTR) => {}
                    Ok(_) => {
                        self.child = None;
                        break;
                    }
                    Err(error) => {
                        failure = Some(format!("reap errno={error:?}"));
                        break;
                    }
                }
                if Instant::now() >= end {
                    failure = Some("reap deadline".into());
                    break;
                }
                thread::yield_now();
            }
        }
        if self.child.is_none()
            && let Err(error) = fs::remove_dir_all(&self.root)
        {
            failure = Some(format!("endpoint cleanup category={:?}", error.kind()));
        }
        if let Some(failure) = failure {
            if thread::panicking() {
                eprintln!("fixture cleanup unresolved: {failure}");
            } else {
                panic!("fixture cleanup unresolved: {failure}");
            }
        }
    }
}

fn read(stream: &mut UnixStream) -> Value {
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut length = [0; 4];
    stream.read_exact(&mut length).unwrap();
    let n = u32::from_be_bytes(length) as usize;
    assert!(n <= 4096);
    let mut bytes = vec![0; n];
    stream.read_exact(&mut bytes).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
fn write(stream: &mut UnixStream, value: Value) {
    let bytes = serde_json::to_vec(&value).unwrap();
    stream
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .unwrap();
    stream.write_all(&bytes).unwrap()
}
#[test]
fn real_fixed_bootstrap_child_requires_release_and_handles_controller_loss() {
    for release in [true, false] {
        let mut fixture = Fixture::start(None, "1000");
        let mut stream = fixture.accept();
        let hello = read(&mut stream);
        assert_eq!(hello[3], fixture.child.unwrap().as_raw());
        stream.set_nonblocking(true).unwrap();
        assert_eq!(
            stream.read(&mut [0]).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        stream.set_nonblocking(false).unwrap();
        if release {
            write(
                &mut stream,
                json!([1, "release", "request-1", "public-nonce"]),
            );
            assert_eq!(
                read(&mut stream),
                json!([1, "nonce", "request-1", "public-nonce"])
            );
        }
        drop(stream);
        assert!(
            matches!(fixture.wait(), WaitStatus::Exited(_, code) if code == if release { 0 } else { 1 })
        );
        let diagnostic = fixture.diagnostic();
        if release {
            assert!(diagnostic.is_empty());
        } else {
            assert!(diagnostic.contains("phase=Release kind=UnexpectedEof"));
        }
        fixture.finish();
    }
}
#[test]
fn unexpected_file_and_proc_directory_descriptors_are_refused_before_connection() {
    for file in [
        File::open("/dev/null").unwrap(),
        File::open("/proc/self/fd").unwrap(),
    ] {
        let mut fixture = Fixture::start(Some(&file), "1000");
        assert!(matches!(fixture.wait(), WaitStatus::Exited(_, 1)));
        fixture.no_connection();
        assert_eq!(
            fixture.diagnostic(),
            "domain bootstrap: phase=Descriptors kind=PermissionDenied os=None\n"
        );
        fixture.finish();
    }
}
#[test]
fn missing_release_exhausts_original_deadline_and_invalid_startup_is_attributable() {
    let mut fixture = Fixture::start(None, "1000");
    let mut stream = fixture.accept();
    read(&mut stream);
    assert!(matches!(fixture.wait(), WaitStatus::Exited(_, 1)));
    assert!(fixture.diagnostic().contains("phase=Release"));
    assert_eq!(stream.read(&mut [0]).unwrap(), 0);
    fixture.finish();
    let mut invalid = Fixture::start(None, "0");
    assert!(matches!(invalid.wait(), WaitStatus::Exited(_, 1)));
    invalid.no_connection();
    assert!(
        invalid
            .diagnostic()
            .contains("phase=Arguments kind=InvalidInput")
    );
    invalid.finish();
}

#[test]
fn fixture_drop_reaps_its_exact_child_and_removes_owned_endpoints() {
    let mut fixture = Fixture::start(None, "1000");
    let mut stream = fixture.accept();
    read(&mut stream);
    let pid = fixture.child.unwrap();
    let root = fixture.root.clone();
    drop(fixture);
    assert!(!root.exists());
    assert_eq!(
        waitpid(pid, Some(WaitPidFlag::WNOHANG)),
        Err(nix::errno::Errno::ECHILD)
    );
}
