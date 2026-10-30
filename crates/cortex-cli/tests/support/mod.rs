use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub struct Repo {
    pub path: PathBuf,
}

impl Repo {
    pub fn new() -> Self {
        let base = std::env::temp_dir();
        let path = loop {
            let path = base.join(format!(
                "cortex-002a-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => break path,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("create fixture: {e}"),
            }
        };
        let repo = Self { path };
        repo.git(&["init", "--initial-branch=main"]);
        repo.git(&["config", "user.name", "Cortex Test"]);
        repo.git(&["config", "user.email", "cortex-test@example.invalid"]);
        repo.git(&["config", "commit.gpgSign", "false"]);
        // Complete fixture maintenance before read-only tests snapshot all Git metadata.
        // Detached maintenance can otherwise remove its lock after commit returns.
        // Automatic maintenance also honors gc.autoDetach when its own setting is absent.
        repo.git(&["config", "gc.autoDetach", "false"]);
        repo
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new("git");
        command.current_dir(&self.path);
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("GIT_") {
                command.env_remove(key);
            }
        }
        command
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .env("GIT_AUTHOR_DATE", "2001-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2001-01-01T00:00:00Z")
            .env("LC_ALL", "C");
        command
    }

    pub fn git(&self, args: &[&str]) -> Output {
        let output = self.command().args(args).output().unwrap();
        assert!(
            output.status.success(),
            "fixture git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    pub fn commit_file(&self, name: &str, contents: &str) {
        fs::write(self.path.join(name), contents).unwrap();
        self.git(&["add", "--", name]);
        self.git(&["commit", "-m", "fixture"]);
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
