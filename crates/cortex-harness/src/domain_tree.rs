//! Bounded descriptor-relative cgroup tree observations. Matched traversals are
//! deliberately non-atomic: this module supplies no closure or snapshot proof.
use crate::domain_manager::{DomainCause, DomainDiagnostic, DomainPhaseDiagnostic, issue};
use cortex_core::program::domain::{DomainCgroupObservation, DomainProcessObservation};
use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, open, openat, statat};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::Path,
    time::Instant,
};

struct Budget {
    bytes: usize,
    end: Instant,
}
impl Budget {
    fn check(&self) -> Result<(), DomainDiagnostic> {
        if Instant::now() >= self.end {
            Err(issue(DomainPhaseDiagnostic::Closure, DomainCause::Deadline))
        } else {
            Ok(())
        }
    }
    fn charge(&mut self, bytes: usize) -> Result<(), DomainDiagnostic> {
        self.check()?;
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(capacity)?;
        if self.bytes > 1_048_576 {
            Err(capacity())
        } else {
            Ok(())
        }
    }
}
fn capacity() -> DomainDiagnostic {
    issue(DomainPhaseDiagnostic::Closure, DomainCause::Capacity)
}
fn correspondence() -> DomainDiagnostic {
    issue(DomainPhaseDiagnostic::Closure, DomainCause::Correspondence)
}
fn io_error(e: impl Into<io::Error>) -> DomainDiagnostic {
    issue(
        DomainPhaseDiagnostic::Closure,
        DomainCause::Io(e.into().kind()),
    )
}
fn directory(parent: &File, name: &str) -> Result<File, DomainDiagnostic> {
    openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(io_error)
}
fn read(parent: &File, name: &str, budget: &mut Budget) -> Result<String, DomainDiagnostic> {
    budget.check()?;
    let file = File::from(
        openat(
            parent,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(io_error)?,
    );
    if !file.metadata().map_err(io_error)?.is_file() {
        return Err(correspondence());
    }
    let mut bytes = Vec::new();
    file.take(16_385)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    budget.charge(bytes.len())?;
    if bytes.len() > 16_384 {
        return Err(capacity());
    }
    String::from_utf8(bytes).map_err(|_| correspondence())
}
fn children(parent: &File, budget: &mut Budget) -> Result<Vec<String>, DomainDiagnostic> {
    let mut names = Vec::new();
    let mut entries = 0;
    let mut dir = Dir::read_from(parent).map_err(io_error)?;
    while let Some(entry) = dir.read() {
        budget.check()?;
        let entry = entry.map_err(io_error)?;
        let name = entry.file_name().to_str().map_err(|_| correspondence())?;
        if name == "." || name == ".." {
            continue;
        }
        entries += 1;
        if entries > 256 {
            return Err(capacity());
        }
        budget.charge(name.len())?;
        let kind = FileType::from_raw_mode(
            statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(io_error)?
                .st_mode,
        );
        if kind == FileType::Symlink {
            return Err(correspondence());
        }
        if kind == FileType::Directory {
            if name.is_empty()
                || name.len() > 255
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            {
                return Err(correspondence());
            }
            names.push(name.to_owned());
            if names.len() > 31 {
                return Err(capacity());
            }
        } else if kind != FileType::RegularFile {
            return Err(correspondence());
        }
    }
    names.sort();
    Ok(names)
}
fn population(value: &str) -> Result<bool, DomainDiagnostic> {
    let mut found = None;
    for line in value.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() == Some("populated") {
            if found.is_some() {
                return Err(correspondence());
            }
            found = match (parts.next(), parts.next()) {
                (Some("0"), None) => Some(false),
                (Some("1"), None) => Some(true),
                _ => return Err(correspondence()),
            };
        }
    }
    found.ok_or_else(correspondence)
}
struct CapturedTree {
    nodes: Vec<DomainCgroupObservation>,
    pids: BTreeSet<u32>,
}
fn walk<F>(
    file: &File,
    relative: &str,
    group: &str,
    device: u64,
    budget: &mut Budget,
    lookup: &mut F,
    tree: &mut CapturedTree,
) -> Result<(), DomainDiagnostic>
where
    F: FnMut(u32, &str) -> Result<(DomainProcessObservation, usize), DomainDiagnostic>,
{
    budget.check()?;
    if tree.nodes.len() >= 32 || relative.split('/').filter(|s| !s.is_empty()).count() > 8 {
        return Err(capacity());
    }
    let before = file.metadata().map_err(io_error)?;
    if !before.is_dir() || before.dev() != device {
        return Err(correspondence());
    }
    let populated = population(&read(file, "cgroup.events", budget)?)?;
    let raw = read(file, "cgroup.procs", budget)?;
    let mut ids = BTreeSet::new();
    for part in raw.split_whitespace() {
        let pid: u32 = part.parse().map_err(|_| correspondence())?;
        if pid == 0 || !ids.insert(pid) || !tree.pids.insert(pid) || tree.pids.len() > 64 {
            return Err(capacity());
        }
    }
    let mut members = Vec::new();
    for pid in ids {
        budget.check()?;
        let (member, bytes) = lookup(pid, group)?;
        budget.charge(bytes)?;
        if member.pid != pid || member.start_ticks == 0 {
            return Err(correspondence());
        }
        members.push(member);
    }
    if !populated && !members.is_empty() {
        return Err(correspondence());
    }
    let names = children(file, budget)?;
    tree.nodes.push(DomainCgroupObservation {
        relative_path: relative.into(),
        device,
        inode: before.ino(),
        populated,
        members,
    });
    for name in &names {
        let child = directory(file, name)?;
        let path = if relative.is_empty() {
            name.clone()
        } else {
            format!("{relative}/{name}")
        };
        if path.len() > 1024 {
            return Err(capacity());
        }
        walk(
            &child,
            &path,
            &format!("{group}/{name}"),
            device,
            budget,
            lookup,
            tree,
        )?;
    }
    let after = file.metadata().map_err(io_error)?;
    if before.dev() != after.dev()
        || before.ino() != after.ino()
        || names != children(file, budget)?
    {
        return Err(correspondence());
    }
    Ok(())
}
/// Root is selected by the exact manager/cgroup incarnation. Every descent/read
/// is descriptor-relative and NOFOLLOW. No foreign process is adopted or signalled.
/// Cooperative deadlines cannot interrupt arbitrary kernel filesystem latency.
pub(crate) fn capture<F>(
    root: &Path,
    device: u64,
    inode: u64,
    group: &str,
    end: Instant,
    mut lookup: F,
) -> Result<Vec<DomainCgroupObservation>, DomainDiagnostic>
where
    F: FnMut(u32, &str) -> Result<(DomainProcessObservation, usize), DomainDiagnostic>,
{
    let file = File::from(
        open(
            root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(io_error)?,
    );
    let pinned = file.metadata().map_err(io_error)?;
    if pinned.dev() != device
        || pinned.ino() != inode
        || fs::canonicalize(root).ok().as_deref() != Some(root)
    {
        return Err(correspondence());
    }
    let mut budget = Budget { bytes: 0, end };
    let mut first = CapturedTree {
        nodes: Vec::new(),
        pids: BTreeSet::new(),
    };
    walk(
        &file,
        "",
        group,
        device,
        &mut budget,
        &mut lookup,
        &mut first,
    )?;
    let mut second = CapturedTree {
        nodes: Vec::new(),
        pids: BTreeSet::new(),
    };
    walk(
        &file,
        "",
        group,
        device,
        &mut budget,
        &mut lookup,
        &mut second,
    )?;
    let current = fs::symlink_metadata(root).map_err(io_error)?;
    if first.nodes != second.nodes
        || current.file_type().is_symlink()
        || current.dev() != device
        || current.ino() != inode
    {
        return Err(correspondence());
    }
    budget.check()?;
    Ok(second.nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::{PermissionsExt, symlink},
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::Duration,
    };
    static SEQUENCE: AtomicU64 = AtomicU64::new(1);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "cortex-tree-{}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
            Self(p)
        }
        fn node(&self, path: &str, populated: bool, pids: &str) {
            let p = self.0.join(path);
            fs::create_dir_all(&p).unwrap();
            fs::write(
                p.join("cgroup.events"),
                if populated {
                    "populated 1\n"
                } else {
                    "populated 0\n"
                },
            )
            .unwrap();
            fs::write(p.join("cgroup.procs"), pids).unwrap();
        }
        fn capture(
            &self,
            lookup: impl FnMut(u32, &str) -> Result<(DomainProcessObservation, usize), DomainDiagnostic>,
        ) -> Result<Vec<DomainCgroupObservation>, DomainDiagnostic> {
            let m = fs::metadata(&self.0).unwrap();
            capture(
                &self.0,
                m.dev(),
                m.ino(),
                "/owned",
                Instant::now() + Duration::from_secs(1),
                lookup,
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn process(pid: u32, _: &str) -> Result<(DomainProcessObservation, usize), DomainDiagnostic> {
        Ok((
            DomainProcessObservation {
                pid,
                start_ticks: 1000 + u64::from(pid),
            },
            16,
        ))
    }
    #[test]
    fn owned_tree_capture_preserves_descendants_when_direct_root_is_empty() {
        let f = Fixture::new();
        f.node("", true, "");
        f.node("child", true, "42\n");
        let nodes = f.capture(process).unwrap();
        assert_eq!(nodes.len(), 2);
        assert!(nodes[0].members.is_empty());
        assert_eq!(nodes[1].relative_path, "child");
        assert_eq!(nodes[1].members[0].pid, 42);
    }
    #[test]
    fn symlinked_node_or_control_file_is_refused_without_following_owned_canary() {
        for directory_link in [true, false] {
            let f = Fixture::new();
            f.node("", false, "");
            let other = Fixture::new();
            other.node("", false, "");
            if directory_link {
                symlink(&other.0, f.0.join("escape")).unwrap();
            } else {
                fs::remove_file(f.0.join("cgroup.procs")).unwrap();
                symlink(other.0.join("cgroup.procs"), f.0.join("cgroup.procs")).unwrap();
            }
            assert!(f.capture(process).is_err());
        }
    }
    #[test]
    fn changed_birth_topology_identity_and_bounds_are_not_empty_success() {
        let f = Fixture::new();
        f.node("", true, "42\n");
        let mut calls = 0;
        assert!(
            f.capture(|pid, _| {
                calls += 1;
                Ok((
                    DomainProcessObservation {
                        pid,
                        start_ticks: calls,
                    },
                    16,
                ))
            })
            .is_err()
        );
        let f = Fixture::new();
        f.node("", true, "42\n");
        let mut calls = 0;
        assert!(
            f.capture(|pid, _| {
                calls += 1;
                if calls == 2 {
                    f.node("late", false, "");
                }
                process(pid, "/owned")
            })
            .is_err()
        );
        let f = Fixture::new();
        f.node("", false, "");
        for n in 0..32 {
            f.node(&format!("c{n}"), false, "");
        }
        assert!(f.capture(process).is_err());
        let f = Fixture::new();
        f.node(
            "",
            true,
            &(1..=65).map(|n| format!("{n}\n")).collect::<String>(),
        );
        assert!(f.capture(process).is_err());
        let m = fs::metadata(&f.0).unwrap();
        assert!(
            capture(
                &f.0,
                m.dev(),
                m.ino() + 1,
                "/owned",
                Instant::now() + Duration::from_secs(1),
                process
            )
            .is_err()
        );
        assert!(capture(&f.0, m.dev(), m.ino(), "/owned", Instant::now(), process).is_err());
    }
}
