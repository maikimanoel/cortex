use super::*;
use std::{
    fs::{DirBuilder, File, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Instant,
};

pub(crate) struct Location {
    path: PathBuf,
    inode: u64,
    device: u64,
    database: &'static str,
    journal: &'static str,
}
impl Location {
    pub(crate) fn create(path: &Path, context: &CandidateObservation) -> Result<Self, StoreError> {
        let mut roots = Vec::new();
        for repo in std::iter::once(context.contract().candidate().repository())
            .chain(context.state().repository())
        {
            roots.extend([
                repo.root().to_owned(),
                repo.git_dir().to_owned(),
                repo.common_dir().to_owned(),
            ]);
        }
        Self::create_roots(path, &roots)
    }
    fn create_roots(path: &Path, roots: &[PathBuf]) -> Result<Self, StoreError> {
        if !path.is_absolute() {
            return Err(StoreError::new(ErrorKind::InvalidLocation));
        }
        let parent = path
            .parent()
            .ok_or_else(|| StoreError::new(ErrorKind::InvalidLocation))?
            .canonicalize()?;
        let name = path
            .file_name()
            .ok_or_else(|| StoreError::new(ErrorKind::InvalidLocation))?;
        let target = parent.join(name);
        for root in roots {
            let resolved = root.canonicalize().unwrap_or_else(|_| root.to_owned());
            if target.starts_with(resolved) {
                return Err(StoreError::new(ErrorKind::InvalidLocation));
            }
        }
        DirBuilder::new().mode(0o700).create(&target)?;
        let location = Self::open_directory(&target)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(target.join(".lock"))?;
        lock.sync_all()?;
        location.sync()?;
        File::open(parent)?.sync_all()?;
        Ok(location)
    }
    fn open_directory(path: &Path) -> Result<Self, StoreError> {
        if !path.is_absolute() {
            return Err(StoreError::new(ErrorKind::InvalidLocation));
        }
        let m = fs::symlink_metadata(path)?;
        let uid = fs::metadata("/proc/self")?.uid();
        if !m.is_dir() || m.uid() != uid || m.mode() & 0o777 != 0o700 {
            return Err(StoreError::new(ErrorKind::InvalidLocation));
        }
        Ok(Self {
            path: path.canonicalize()?,
            inode: m.ino(),
            device: m.dev(),
            database: DATABASE,
            journal: JOURNAL,
        })
    }
    pub(crate) fn open(path: &Path) -> Result<Self, StoreError> {
        let location = Self::open_directory(path)?;
        location.files()?;
        Ok(location)
    }
    pub(crate) fn program_create(path: &Path, roots: &[PathBuf]) -> Result<Self, StoreError> {
        let mut location = Self::create_roots(path, roots)?;
        location.database = "program.db";
        location.journal = "program.db-journal";
        Ok(location)
    }
    pub(crate) fn program_open(path: &Path) -> Result<Self, StoreError> {
        let mut location = Self::open_directory(path)?;
        location.database = "program.db";
        location.journal = "program.db-journal";
        location.files()?;
        Ok(location)
    }
    pub(crate) fn database(&self) -> &'static str {
        self.database
    }
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn check(&self) -> Result<(), StoreError> {
        let m = fs::symlink_metadata(&self.path)?;
        if !m.is_dir()
            || m.ino() != self.inode
            || m.dev() != self.device
            || m.mode() & 0o777 != 0o700
        {
            return Err(StoreError::new(ErrorKind::TargetChanged));
        }
        Ok(())
    }
    pub(crate) fn lock(&self) -> Result<File, StoreError> {
        self.lock_for(Duration::from_secs(5))
    }
    pub(crate) fn lock_for(&self, timeout: Duration) -> Result<File, StoreError> {
        self.check()?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.path.join(".lock"))?;
        let m = file.metadata()?;
        if !m.is_file()
            || m.len() != 0
            || m.nlink() != 1
            || m.mode() & 0o077 != 0
            || m.uid() != fs::metadata(&self.path)?.uid()
        {
            return Err(StoreError::new(ErrorKind::InvalidLocation));
        }
        let start = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => break,
                Err(fs::TryLockError::WouldBlock) => {
                    if start.elapsed() >= timeout.min(Duration::from_secs(5)) {
                        return Err(StoreError::new(ErrorKind::Busy));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(fs::TryLockError::Error(e)) => return Err(e.into()),
            }
        }
        self.check()?;
        let current = fs::symlink_metadata(self.path.join(".lock"))?;
        if current.ino() != m.ino() || current.dev() != m.dev() {
            return Err(StoreError::new(ErrorKind::TargetChanged));
        }
        Ok(file)
    }
    pub(crate) fn files(&self) -> Result<(), StoreError> {
        self.check()?;
        let mut bytes = 0u64;
        for (count, entry) in fs::read_dir(&self.path)?.enumerate() {
            if count >= 4 {
                return Err(StoreError::new(ErrorKind::UnexpectedFiles));
            }
            let entry = entry?;
            let name = entry.file_name();
            if ![".lock", self.database, self.journal, REMOVING]
                .iter()
                .any(|n| name == *n)
            {
                return Err(StoreError::new(ErrorKind::UnexpectedFiles));
            }
            let m = fs::symlink_metadata(entry.path())?;
            if !m.is_file()
                || m.nlink() != 1
                || m.uid() != fs::metadata(&self.path)?.uid()
                || m.mode() & 0o077 != 0
            {
                return Err(StoreError::new(ErrorKind::InvalidLocation));
            }
            bytes = bytes
                .checked_add(m.len())
                .ok_or_else(|| StoreError::new(ErrorKind::Capacity))?;
            if bytes > 256 * 1024 * 1024 {
                return Err(StoreError::new(ErrorKind::Capacity));
            }
        }
        Ok(())
    }
    pub(crate) fn ready(&self) -> Result<(), StoreError> {
        self.files()?;
        if self.intent()?.is_some() {
            return Err(StoreError::new(ErrorKind::RemovalPending));
        }
        Ok(())
    }
    pub(crate) fn database_exists(&self) -> Result<bool, StoreError> {
        match fs::symlink_metadata(self.path.join(self.database)) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
    pub(crate) fn journal_exists(&self) -> Result<bool, StoreError> {
        match fs::symlink_metadata(self.path.join(self.journal)) {
            Ok(m) => Ok(m.len() > 0),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
    pub(crate) fn check_database(&self) -> Result<(), StoreError> {
        let m = fs::symlink_metadata(self.path.join(self.database))?;
        if m.mode() & 0o400 == 0 {
            return Err(StoreError::new(ErrorKind::Permission));
        }
        if !m.is_file() || m.nlink() != 1 || m.len() > MAX_BYTES as u64 {
            return Err(StoreError::new(ErrorKind::Capacity));
        }
        Ok(())
    }
    pub(crate) fn create_database(&self) -> Result<(), StoreError> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.path.join(self.database))?;
        file.sync_all()?;
        Ok(())
    }
    pub(crate) fn sync(&self) -> Result<(), StoreError> {
        File::open(&self.path)?.sync_all()?;
        Ok(())
    }
    pub(crate) fn intent(&self) -> Result<Option<StoreId>, StoreError> {
        match fs::symlink_metadata(self.path.join(REMOVING)) {
            Ok(m) if m.len() != 16 => return Err(StoreError::new(ErrorKind::RemovalIncomplete)),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        }
        match fs::read(self.path.join(REMOVING)) {
            Ok(b) => {
                Ok(Some(StoreId(b.try_into().map_err(|_| {
                    StoreError::new(ErrorKind::RemovalIncomplete)
                })?)))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    pub(crate) fn write_intent(&self, id: StoreId) -> Result<(), StoreError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.path.join(REMOVING))?;
        file.write_all(&id.0)?;
        file.sync_all()?;
        self.sync()
    }
    pub(crate) fn unlink(&self, name: &str) -> Result<(), StoreError> {
        self.files()?;
        match fs::remove_file(self.path.join(name)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
