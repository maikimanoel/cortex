//! Private format projections. Only Core constructors/append admit decoded data.

use super::{ErrorKind, MAX_BYTES, StoreError};
use cortex_core::{CandidateId, TaskContract, TaskId, binding::*, temporal::*};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, SeqAccess, Visitor},
};
use std::{
    ffi::OsString,
    fmt,
    io::Write,
    marker::PhantomData,
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zeroize::Zeroizing;

fn invalid() -> StoreError {
    StoreError::new(ErrorKind::Corrupt)
}

fn text<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    struct Text;
    impl Visitor<'_> for Text {
        type Value = String;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("bounded text")
        }
        fn visit_str<E: de::Error>(self, s: &str) -> Result<String, E> {
            if s.len() > 4096 {
                return Err(E::custom("text limit"));
            }
            Ok(s.to_owned())
        }
    }
    d.deserialize_str(Text)
}

fn bounded<'de, D, T, const N: usize>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Items<T, const N: usize>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const N: usize> Visitor<'de> for Items<T, N> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("bounded sequence")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = seq.next_element()? {
                if values.len() == N {
                    return Err(de::Error::custom("sequence limit"));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    d.deserialize_seq(Items::<T, N>(PhantomData))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Id {
    #[serde(deserialize_with = "text")]
    scope: String,
    sequence: u64,
}
impl Id {
    fn from(id: &ObservationId) -> Self {
        Self {
            scope: id.scope().to_owned(),
            sequence: id.sequence(),
        }
    }
    fn decode(self) -> Result<ObservationId, StoreError> {
        ObservationId::new(&self.scope, self.sequence).map_err(|_| invalid())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Repo {
    #[serde(deserialize_with = "bounded::<_,u8,4194304>")]
    root: Vec<u8>,
    #[serde(deserialize_with = "bounded::<_,u8,4194304>")]
    git: Vec<u8>,
    #[serde(deserialize_with = "bounded::<_,u8,4194304>")]
    common: Vec<u8>,
}
impl Repo {
    fn from(r: &RepositoryIdentity) -> Self {
        Self {
            root: r.root().as_os_str().as_bytes().to_vec(),
            git: r.git_dir().as_os_str().as_bytes().to_vec(),
            common: r.common_dir().as_os_str().as_bytes().to_vec(),
        }
    }
    fn decode(self) -> Result<RepositoryIdentity, StoreError> {
        fn path(b: Vec<u8>) -> PathBuf {
            OsString::from_vec(b).into()
        }
        RepositoryIdentity::new(path(self.root), path(self.git), path(self.common))
            .map_err(|_| invalid())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum HeadData {
    Attached {
        #[serde(deserialize_with = "bounded::<_,u8,4194304>")]
        branch: Vec<u8>,
        #[serde(deserialize_with = "text")]
        commit: String,
    },
    Detached {
        #[serde(deserialize_with = "text")]
        commit: String,
    },
    Unborn {
        #[serde(deserialize_with = "bounded::<_,u8,4194304>")]
        branch: Vec<u8>,
    },
    Unknown,
}
fn commit(s: &str) -> Result<CandidateId, StoreError> {
    CandidateId::from_git_commit(s).map_err(|_| invalid())
}
impl HeadData {
    fn from(h: &Head) -> Self {
        match h {
            Head::Attached { branch, commit } => Self::Attached {
                branch: branch.clone(),
                commit: commit.as_str().to_owned(),
            },
            Head::Detached { commit } => Self::Detached {
                commit: commit.as_str().to_owned(),
            },
            Head::Unborn { branch } => Self::Unborn {
                branch: branch.clone(),
            },
            Head::Unknown => Self::Unknown,
        }
    }
    fn decode(self) -> Result<Head, StoreError> {
        Ok(match self {
            Self::Attached { branch, commit: s } => Head::Attached {
                branch,
                commit: commit(&s)?,
            },
            Self::Detached { commit: s } => Head::Detached {
                commit: commit(&s)?,
            },
            Self::Unborn { branch } => Head::Unborn { branch },
            Self::Unknown => Head::Unknown,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum EntryData {
    Tracked {
        #[serde(deserialize_with = "bounded::<_,u8,4194304>")]
        path: Vec<u8>,
        staged: Option<u8>,
        unstaged: Option<u8>,
    },
    Untracked {
        #[serde(deserialize_with = "bounded::<_,u8,4194304>")]
        path: Vec<u8>,
    },
}
fn change(c: &Change) -> u8 {
    match c {
        Change::Added => 0,
        Change::Modified => 1,
        Change::Deleted => 2,
        Change::TypeChanged => 3,
    }
}
fn read_change(c: u8) -> Result<Change, StoreError> {
    Ok(match c {
        0 => Change::Added,
        1 => Change::Modified,
        2 => Change::Deleted,
        3 => Change::TypeChanged,
        _ => return Err(invalid()),
    })
}
impl EntryData {
    fn from(e: &Entry) -> Self {
        match e {
            Entry::Tracked {
                path,
                staged,
                unstaged,
            } => Self::Tracked {
                path: path.clone(),
                staged: staged.as_ref().map(change),
                unstaged: unstaged.as_ref().map(change),
            },
            Entry::Untracked { path } => Self::Untracked { path: path.clone() },
        }
    }
    fn decode(self) -> Result<Entry, StoreError> {
        Ok(match self {
            Self::Tracked {
                path,
                staged,
                unstaged,
            } => Entry::Tracked {
                path,
                staged: staged.map(read_change).transpose()?,
                unstaged: unstaged.map(read_change).transpose()?,
            },
            Self::Untracked { path } => Entry::Untracked { path },
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Basis {
    #[serde(deserialize_with = "text")]
    profile: String,
    #[serde(deserialize_with = "bounded::<_,u8,12582936>")]
    material: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Context {
    #[serde(deserialize_with = "text")]
    platform: String,
    id: Id,
    #[serde(deserialize_with = "text")]
    task: String,
    #[serde(deserialize_with = "text")]
    objective: String,
    #[serde(deserialize_with = "text")]
    acceptance: String,
    revision: u64,
    expected_repo: Repo,
    #[serde(deserialize_with = "text")]
    expected_commit: String,
    worktree: u8,
    repo: Option<Repo>,
    head: HeadData,
    #[serde(deserialize_with = "bounded::<_,EntryData,10000>")]
    entries: Vec<EntryData>,
    coverage: u8,
    consistency: u8,
    basis: Option<Basis>,
}

pub(super) fn encode_context(c: &CandidateObservation) -> Result<Zeroizing<Vec<u8>>, StoreError> {
    let contract = c.contract();
    let state = c.state();
    encode(
        &Context {
            platform: "linux-bytes-v1".into(),
            id: Id::from(c.id()),
            task: contract.task().id().as_str().to_owned(),
            objective: contract.task().objective().to_owned(),
            acceptance: contract.task().acceptance().to_owned(),
            revision: contract.revision().get(),
            expected_repo: Repo::from(contract.candidate().repository()),
            expected_commit: contract.candidate().commit().as_str().to_owned(),
            worktree: match contract.candidate().worktree() {
                WorktreeRequirement::Clean => 0,
                WorktreeRequirement::AllowChanges => 1,
            },
            repo: state.repository().map(Repo::from),
            head: HeadData::from(state.head()),
            entries: state.entries().iter().map(EntryData::from).collect(),
            coverage: match state.coverage() {
                ObservationCoverage::GitStatus => 0,
                ObservationCoverage::Incomplete => 1,
            },
            consistency: match state.consistency() {
                ObservationConsistency::TwoMatchingSamples => 0,
                ObservationConsistency::Indeterminate => 1,
            },
            basis: c.basis().map(|b| Basis {
                profile: b.profile().to_owned(),
                material: b.material().to_vec(),
            }),
        },
        MAX_BYTES,
    )
}

pub(super) fn decode_context(b: &[u8]) -> Result<CandidateObservation, StoreError> {
    let c: Context = decode(b, MAX_BYTES)?;
    if c.platform != "linux-bytes-v1" {
        return Err(StoreError::new(ErrorKind::Unsupported));
    }
    let task = TaskContract::new(
        TaskId::new(&c.task).map_err(|_| invalid())?,
        &c.objective,
        &c.acceptance,
    )
    .map_err(|_| invalid())?;
    let expected = CandidateRequirement::new(
        c.expected_repo.decode()?,
        commit(&c.expected_commit)?,
        match c.worktree {
            0 => WorktreeRequirement::Clean,
            1 => WorktreeRequirement::AllowChanges,
            _ => return Err(invalid()),
        },
    );
    let contract = CandidateBindingContract::new(
        task,
        ContractRevision::new(c.revision).map_err(|_| invalid())?,
        expected,
    );
    let state = RepositoryState::new(
        c.repo.map(Repo::decode).transpose()?,
        c.head.decode()?,
        c.entries
            .into_iter()
            .map(EntryData::decode)
            .collect::<Result<_, _>>()?,
        match c.coverage {
            0 => ObservationCoverage::GitStatus,
            1 => ObservationCoverage::Incomplete,
            _ => return Err(invalid()),
        },
        match c.consistency {
            0 => ObservationConsistency::TwoMatchingSamples,
            1 => ObservationConsistency::Indeterminate,
            _ => return Err(invalid()),
        },
    )
    .map_err(|_| invalid())?;
    let basis = c
        .basis
        .map(|b| ComparisonBasis::new(&b.profile, b.material).map_err(|_| invalid()))
        .transpose()?;
    Ok(CandidateObservation::new(
        c.id.decode()?,
        &contract,
        state,
        basis,
    ))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Instant {
    seconds: u64,
    nanos: u32,
}
impl Instant {
    fn from(t: SystemTime) -> Result<Self, StoreError> {
        let d = t.duration_since(UNIX_EPOCH).map_err(|_| invalid())?;
        Ok(Self {
            seconds: d.as_secs(),
            nanos: d.subsec_nanos(),
        })
    }
    fn decode(self) -> Result<SystemTime, StoreError> {
        if self.nanos >= 1_000_000_000 {
            return Err(invalid());
        }
        UNIX_EPOCH
            .checked_add(Duration::new(self.seconds, self.nanos))
            .ok_or_else(|| StoreError::new(ErrorKind::Unsupported))
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Origin {
    Operator(#[serde(deserialize_with = "text")] String),
    Observation(Id),
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    origin: Origin,
    #[serde(deserialize_with = "text")]
    source: String,
    #[serde(deserialize_with = "text")]
    version: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    #[serde(deserialize_with = "text")]
    entity: String,
    #[serde(deserialize_with = "text")]
    version: String,
    kind: u8,
    context: Id,
    #[serde(deserialize_with = "text")]
    body: String,
    #[serde(deserialize_with = "bounded::<_,Source,32>")]
    sources: Vec<Source>,
    from: Instant,
    until: Option<Instant>,
    recorded: Instant,
    predecessor: Option<String>,
}
pub(super) fn encode_record(r: &CanonicalRecord) -> Result<Zeroizing<Vec<u8>>, StoreError> {
    encode(
        &Record {
            entity: r.entity().as_str().to_owned(),
            version: r.id().as_str().to_owned(),
            kind: match r.kind() {
                RecordKind::Requirement => 0,
                RecordKind::Decision => 1,
            },
            context: Id::from(r.context()),
            body: r.content().body().to_owned(),
            sources: r
                .content()
                .sources()
                .iter()
                .map(|s| Source {
                    origin: match s.origin() {
                        SourceOrigin::OperatorReported { author } => {
                            Origin::Operator(author.clone())
                        }
                        SourceOrigin::ObservationReference(id) => Origin::Observation(Id::from(id)),
                    },
                    source: s.source().to_owned(),
                    version: s.version().to_owned(),
                })
                .collect(),
            from: Instant::from(r.time().valid().from())?,
            until: r.time().valid().until().map(Instant::from).transpose()?,
            recorded: Instant::from(r.time().recorded_at())?,
            predecessor: r.predecessor().map(|id| id.as_str().to_owned()),
        },
        1024 * 1024,
    )
}
pub(super) fn decode_record(b: &[u8]) -> Result<CanonicalRecord, StoreError> {
    let r: Record = decode(b, 1024 * 1024)?;
    let sources = r
        .sources
        .into_iter()
        .map(|s| {
            match s.origin {
                Origin::Operator(author) => {
                    SourceReference::operator_reported(&author, &s.source, &s.version)
                }
                Origin::Observation(id) => {
                    SourceReference::observation_reference(id.decode()?, &s.source, &s.version)
                }
            }
            .map_err(StoreError::domain)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let content = RecordContent::new(&r.body, sources).map_err(StoreError::domain)?;
    let time = RecordTime::new(
        DomainInterval::new(r.from.decode()?, r.until.map(Instant::decode).transpose()?)
            .map_err(StoreError::domain)?,
        r.recorded.decode()?,
    )
    .map_err(StoreError::domain)?;
    let entity = RecordId::new(&r.entity).map_err(StoreError::domain)?;
    let id = RecordVersionId::new(&r.version).map_err(StoreError::domain)?;
    let kind = match r.kind {
        0 => RecordKind::Requirement,
        1 => RecordKind::Decision,
        _ => return Err(invalid()),
    };
    let context = r.context.decode()?;
    Ok(match r.predecessor {
        Some(p) => CanonicalRecord::correction(
            entity,
            id,
            kind,
            context,
            RecordVersionId::new(&p).map_err(StoreError::domain)?,
            content,
            time,
        ),
        None => CanonicalRecord::initial(entity, id, kind, context, content, time),
    })
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8], limit: usize) -> Result<T, StoreError> {
    if bytes.len() > limit {
        return Err(StoreError::new(ErrorKind::Capacity));
    }
    serde_json::from_slice(bytes).map_err(|_| invalid())
}
fn encode<T: Serialize>(value: &T, limit: usize) -> Result<Zeroizing<Vec<u8>>, StoreError> {
    struct Output {
        bytes: Zeroizing<Vec<u8>>,
        limit: usize,
    }
    impl Write for Output {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            if b.len() > self.limit.saturating_sub(self.bytes.len()) {
                return Err(std::io::ErrorKind::OutOfMemory.into());
            }
            self.bytes.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut out = Output {
        bytes: Zeroizing::new(Vec::new()),
        limit,
    };
    serde_json::to_writer(&mut out, value).map_err(|_| StoreError::new(ErrorKind::Capacity))?;
    Ok(out.bytes)
}
