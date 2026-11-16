//! Test-only fresh identity correspondence; never a production identity registry.
#![allow(dead_code)]
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};

#[derive(Debug)]
pub enum FixtureIdentityError {
    InvalidNamespace,
    DuplicateNamespace,
    Correspondence,
    Io(std::io::Error),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureNamespace(String);
impl FixtureNamespace {
    pub fn from_os(value: &std::ffi::OsStr) -> Result<Self, FixtureIdentityError> {
        Self::new(
            value
                .to_str()
                .ok_or(FixtureIdentityError::InvalidNamespace)?,
        )
    }
    pub fn new(value: &str) -> Result<Self, FixtureIdentityError> {
        if value.is_empty()
            || value.len() > 32
            || !value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || !value.as_bytes()[0].is_ascii_lowercase()
        {
            return Err(FixtureIdentityError::InvalidNamespace);
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn qualify(&self, label: &str) -> Result<String, FixtureIdentityError> {
        if label.is_empty()
            || self.0.len() + 1 + label.len() > 128
            || !label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(FixtureIdentityError::Correspondence);
        }
        Ok(format!("{}-{label}", self.0))
    }
    /// A failed or interrupted claim is never refunded or silently reused.
    pub fn claim(&self, owned_root: &Path) -> Result<(), FixtureIdentityError> {
        let metadata = fs::symlink_metadata(owned_root).map_err(FixtureIdentityError::Io)?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(FixtureIdentityError::Correspondence);
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(owned_root.join(format!("namespace-{}", self.0)))
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    FixtureIdentityError::DuplicateNamespace
                } else {
                    FixtureIdentityError::Io(error)
                }
            })?;
        file.write_all(b"claimed; no automatic retry\n")
            .map_err(FixtureIdentityError::Io)?;
        file.sync_all().map_err(FixtureIdentityError::Io)
    }
    pub fn check_attempt(
        &self,
        program: &str,
        task: &str,
        attempt: &str,
        package: &str,
    ) -> Result<(), FixtureIdentityError> {
        if program != self.qualify("gpe-func-2t")? {
            return Err(FixtureIdentityError::Correspondence);
        }
        for (task_label, attempt_label) in [("A", "A"), ("B", "B"), ("A", "synthetic-unknown")] {
            if task == self.qualify(task_label)?
                && attempt == self.qualify(&format!("attempt-{attempt_label}"))?
                && package == self.qualify(&format!("package-{attempt_label}"))?
            {
                return Ok(());
            }
        }
        Err(FixtureIdentityError::Correspondence)
    }
    pub fn check_link(
        &self,
        operation: &str,
        label: &str,
        expected: u64,
        actual: u64,
    ) -> Result<(), FixtureIdentityError> {
        if operation != self.qualify(label)? || expected != actual {
            return Err(FixtureIdentityError::Correspondence);
        }
        Ok(())
    }
    pub fn check_release(
        &self,
        operation: &str,
        edge: &str,
        artifact: &str,
    ) -> Result<(), FixtureIdentityError> {
        if operation != self.qualify("release-AB")?
            || edge != self.qualify("AB")?
            || artifact != self.qualify("prime-list")?
        {
            return Err(FixtureIdentityError::Correspondence);
        }
        Ok(())
    }
}
