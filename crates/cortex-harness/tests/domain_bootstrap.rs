//! Finite trusted development processes. Native probes use private namespaces;
//! no real manager activation or shared cgroup modification is performed.
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
    network_canary: Option<std::net::TcpListener>,
}
impl Fixture {
    fn start(extra: Option<&File>, timeout: &str) -> Self {
        Self::start_native(extra, timeout, false)
    }
    fn start_native(extra: Option<&File>, timeout: &str, isolated: bool) -> Self {
        Self::start_options(extra, timeout, isolated, false, false)
    }
    fn start_options(
        extra: Option<&File>,
        timeout: &str,
        isolated: bool,
        nonnull_stdin: bool,
        overlong: bool,
    ) -> Self {
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
            network_canary: if isolated {
                Some(std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap())
            } else {
                None
            },
        };
        if isolated {
            fs::write(fixture.root.join("outside-canary"), b"owned-canary").unwrap();
            // Valid neighbor: the same loopback endpoint is reachable outside
            // the child namespace before using its denial as an oracle.
            let listener = fixture.network_canary.as_ref().unwrap();
            let client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
            let (accepted, _) = listener.accept().unwrap();
            drop(accepted);
            drop(client);
        }
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
        actions
            .add_dup2(
                if isolated {
                    null.as_raw_fd()
                } else {
                    diagnostic.as_raw_fd()
                },
                2,
            )
            .unwrap();
        if let Some(extra) = extra {
            actions
                .add_dup2(extra.as_raw_fd(), if nonnull_stdin { 0 } else { 3 })
                .unwrap();
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
            if fd > 2 && !(fd == 3 && extra.is_some() && !nonnull_stdin) {
                actions.add_close(fd).unwrap();
            }
        }
        let binary = CString::new(env!("CARGO_BIN_EXE_cortex-domain-bootstrap")).unwrap();
        let mut args = vec![
            binary.clone(),
            CString::new(endpoint.as_os_str().as_bytes()).unwrap(),
            CString::new("request-1").unwrap(),
            CString::new(timeout).unwrap(),
        ];
        if isolated {
            let mount = fixture.root.join("mount");
            fs::create_dir(&mount).unwrap();
            fs::set_permissions(&mount, fs::Permissions::from_mode(0o700)).unwrap();
            let policy =
                cortex_harness::native_linux::NativeIsolation::inspect(&mount, 65536, 16).unwrap();
            for arg in [
                "isolated-probe-v1".into(),
                mount.to_str().unwrap().to_string(),
                "65536".into(),
                "16".into(),
                policy.digest(),
                fixture
                    .network_canary
                    .as_ref()
                    .unwrap()
                    .local_addr()
                    .unwrap()
                    .port()
                    .to_string(),
            ] {
                args.push(CString::new(arg).unwrap());
            }
        }
        if overlong {
            args.push(CString::new("public-extra").unwrap());
        }
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

/// A successful refusal is evidence of fail-closed behavior, not enforcement.
/// The diagnostic explicitly distinguishes that outcome in the retained log.
#[test]
fn native_isolation_installs_before_ready_or_refuses_without_payload() {
    let mut fixture = Fixture::start_native(None, "1000", true);
    let mut stream = fixture.accept();
    let observed = read(&mut stream);
    if observed[1] == "isolation-refused" {
        assert!(
            matches!(observed[3].as_str(), Some("UserNamespace" | "Namespaces"))
                && matches!(observed[4].as_i64(), Some(1 | 13 | 28 | 38)),
            "unexpected native setup failure requires investigation: {observed}"
        );
        println!("NATIVE_HOST_ENFORCEMENT=BLOCKED setup={observed}");
        assert_eq!(observed[2], "request-1");
        assert!(matches!(fixture.wait(), WaitStatus::Exited(_, 1)));
        assert_eq!(stream.read(&mut [0]).unwrap(), 0);
    } else {
        assert_eq!(observed.as_array().unwrap().len(), 7);
        assert_eq!(observed[1], "hello");
        assert_eq!(observed[3], fixture.child.unwrap().as_raw());
        stream.set_nonblocking(true).unwrap();
        assert_eq!(
            stream.read(&mut [0]).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        stream.set_nonblocking(false).unwrap();
        write(
            &mut stream,
            json!([1, "release", "request-1", "public-native"]),
        );
        assert_eq!(
            read(&mut stream),
            json!([1, "nonce", "request-1", "public-native"])
        );
        assert!(matches!(fixture.wait(), WaitStatus::Exited(_, 0)));
        let listener = fixture.network_canary.as_ref().unwrap();
        listener.set_nonblocking(true).unwrap();
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert_eq!(
            fs::read(fixture.root.join("outside-canary")).unwrap(),
            b"owned-canary"
        );
        println!(
            "NATIVE_HOST_SETUP=OBSERVED fixed_nonce=1 readonly_root=DENIED outside_canary=DENIED symlink_escape=DENIED traversal=DENIED scratch_overflow=ENOSPC local_network=DENIED whole_domain_qualification=NOT_RUN"
        );
    }
    drop(stream);
    fixture.finish();
}
#[test]
fn native_extra_descriptor_is_refused_before_setup_and_never_announces_ready() {
    let file = File::open("/dev/null").unwrap();
    let mut fixture = Fixture::start_native(Some(&file), "1000", true);
    assert!(matches!(fixture.wait(), WaitStatus::Exited(_, 1)));
    fixture.no_connection();
    fixture.finish();
}

/// Explicit finite overhead harness: four trusted child launches, at most two
/// concurrent. This is a bootstrap microfixture, not a GPE strategy comparison.
/// It includes owned setup and endpoint cleanup, preserves exact valid outputs,
/// and refuses a sample if native setup is unavailable. No speedup is asserted.
#[test]
#[ignore = "explicit four-launch native overhead observation; not an automatic benchmark"]
fn native_fixture_sequential_parallel_observation() {
    fn finish(mut f: Fixture) {
        let mut stream = f.accept();
        let hello = read(&mut stream);
        assert_eq!(
            hello[1], "hello",
            "enforcement prerequisite missing: {hello}"
        );
        assert_eq!(hello.as_array().unwrap().len(), 7);
        write(
            &mut stream,
            json!([1, "release", "request-1", "public-measurement"]),
        );
        assert_eq!(
            read(&mut stream),
            json!([1, "nonce", "request-1", "public-measurement"])
        );
        assert!(matches!(f.wait(), WaitStatus::Exited(_, 0)));
        drop(stream);
        f.finish();
    }
    let start = Instant::now();
    for _ in 0..2 {
        finish(Fixture::start_native(None, "1000", true));
    }
    let sequential_ns = start.elapsed().as_nanos();
    let start = Instant::now();
    let a = Fixture::start_native(None, "1000", true);
    let b = Fixture::start_native(None, "1000", true);
    finish(a);
    finish(b);
    let parallel_ns = start.elapsed().as_nanos();
    println!(
        "{}",
        json!({"workload":"native-bootstrap-canary-v1", "offered_per_arm":2,
        "valid_per_arm":2,"failed":0,"uncertain":0,"rejected":0,"unfinished":0,
        "sequential_wall_ns":sequential_ns,"parallel_wall_ns":parallel_ns,
        "parallel_limit":2,"sample_pairs":1,"cpu_time":"NOT_RUN", "peak_memory":"NOT_RUN",
        "gpe_strategy_comparison":"NOT_RUN", "speedup_claim":"NONE"})
    );
}

#[test]
fn native_stdio_directory_cannot_bypass_root_and_descriptor_policy() {
    let directory = File::open(std::env::temp_dir()).unwrap();
    let mut fixture = Fixture::start_options(Some(&directory), "1000", true, true, false);
    let mut stream = fixture.accept();
    let failure = read(&mut stream);
    assert_eq!(failure[1], "isolation-refused");
    assert_eq!(failure[3], "Descriptors");
    assert!(matches!(fixture.wait(), WaitStatus::Exited(_, 1)));
    assert_eq!(stream.read(&mut [0]).unwrap(), 0);
    drop(stream);
    fixture.finish();
}
#[test]
fn native_binary_rejects_valid_prefix_with_extra_argument_without_truncation() {
    let mut fixture = Fixture::start_options(None, "1000", true, false, true);
    assert!(matches!(fixture.wait(), WaitStatus::Exited(_, 1)));
    fixture.no_connection();
    fixture.finish();
}
