//! Bounded artifact capture from an already owned scratch directory descriptor.
//! Caller must prove domain quiescence before collection. This module cannot
//! establish that fact, settle reservations or create a durable artifact store.
use super::ExecutionError;
use rustix::fs::{Mode, OFlags, ResolveFlags};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::Read,
    os::{fd::AsFd, unix::fs::MetadataExt},
    path::{Component, PathBuf},
};
/// One explicitly expected output file.
#[derive(Clone, PartialEq, Eq)]
pub struct ArtifactSpec {
    /// Normal relative path below owned scratch; symlinks are never followed.
    pub path: PathBuf,
    /// Positive maximum retained bytes.
    pub max_bytes: u64,
    /// Optional expected output digest; mismatch fails capture.
    pub sha256: Option<[u8; 32]>,
}
/// Actual retained bytes and digest. Debug deliberately excludes private content.
pub struct CapturedArtifact {
    /// Exact declared relative output path.
    pub path: PathBuf,
    /// Actual finite content.
    pub bytes: Vec<u8>,
    /// SHA-256 of these exact bytes.
    pub sha256: [u8; 32],
}
pub(crate) fn validate_specs(specs: &[ArtifactSpec], max: u64) -> Result<(), ExecutionError> {
    if specs.len() > 16 {
        return Err(ExecutionError::Policy);
    }
    let mut total = 0u64;
    let mut names = BTreeSet::new();
    for s in specs {
        if s.path.as_os_str().is_empty()
            || s.path.as_os_str().len() > 256
            || s.path
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
            || s.max_bytes == 0
            || !names.insert(&s.path)
        {
            return Err(ExecutionError::Policy);
        }
        total = total
            .checked_add(s.max_bytes)
            .ok_or(ExecutionError::Policy)?;
    }
    if total > max || total > 1024 * 1024 {
        return Err(ExecutionError::Policy);
    }
    Ok(())
}
/// Read only enumerated regular files with no symlink/traversal/mount escape.
/// This does not enumerate arbitrary child output. Quiescence is a prerequisite,
/// not an inference from a successful read or direct-child exit.
pub fn capture(
    root: impl AsFd,
    specs: &[ArtifactSpec],
    max: u64,
) -> Result<Vec<CapturedArtifact>, ExecutionError> {
    validate_specs(specs, max)?;
    let mut captured = Vec::with_capacity(specs.len());
    for s in specs {
        let fd = rustix::fs::openat2(
            &root,
            &s.path,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
            ResolveFlags::BENEATH
                | ResolveFlags::NO_SYMLINKS
                | ResolveFlags::NO_MAGICLINKS
                | ResolveFlags::NO_XDEV,
        )?;
        let f = File::from(fd);
        let before = f.metadata()?;
        if !before.is_file() || before.nlink() != 1 || before.len() > s.max_bytes {
            return Err(ExecutionError::Identity);
        }
        let mut bytes = Vec::new();
        Read::take(&f, s.max_bytes + 1).read_to_end(&mut bytes)?;
        let after = f.metadata()?;
        if bytes.len() as u64 != before.len()
            || before.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return Err(ExecutionError::Identity);
        }
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        if s.sha256.is_some_and(|expected| expected != digest) {
            return Err(ExecutionError::Identity);
        }
        captured.push(CapturedArtifact {
            path: s.path.clone(),
            bytes,
            sha256: digest,
        });
    }
    Ok(captured)
}
