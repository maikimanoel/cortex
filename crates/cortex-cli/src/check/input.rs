use super::{CheckError, ErrorKind, Format};
use cortex_core::{CandidateId, TaskContract, TaskId, binding::ContractRevision};
use std::{ffi::OsString, path::PathBuf};

pub(super) struct Input {
    pub task: TaskContract,
    pub revision: ContractRevision,
    pub candidate: CandidateId,
    pub expected: CandidateId,
    pub repo: PathBuf,
}

impl Input {
    pub fn parse(args: impl Iterator<Item = OsString>, format: Format) -> Result<Self, CheckError> {
        let args: Vec<_> = args.take(13).collect();
        let invalid = |message| CheckError::new(format, ErrorKind::Input(message));
        if args.len() > 12 || args.iter().any(|a| a.as_encoded_bytes().len() > 16_384) {
            return Err(invalid(
                "check input exceeds 12 tokens or 16384 bytes per token",
            ));
        }
        let (mut task, mut revision, mut candidate, mut expected, mut repo) =
            (None, None, None, None, None);
        let (pairs, remainder) = args.as_chunks::<2>();
        for pair in pairs {
            let target = match pair[0].to_str() {
                Some("--task") => &mut task,
                Some("--revision") => &mut revision,
                Some("--candidate") => &mut candidate,
                Some("--expect-head") => &mut expected,
                Some("--repo") => &mut repo,
                _ => return Err(invalid("unsupported check option; use check --help")),
            };
            if target.replace(pair[1].clone()).is_some() {
                return Err(invalid("duplicate check option; supply each option once"));
            }
        }
        if !remainder.is_empty() {
            return Err(invalid(
                "each check option requires a value; use check --help",
            ));
        }
        let required = |value: Option<OsString>| {
            value.ok_or_else(|| {
                invalid("check requires --task, --revision, --candidate and --expect-head")
            })
        };
        let text = |value: OsString| {
            value
                .into_string()
                .map_err(|_| invalid("task, revision and commit inputs must be UTF-8"))
        };
        let id = TaskId::new(&text(required(task)?)?)
            .map_err(|e| CheckError::new(format, ErrorKind::Contract(e)))?;
        let revision = text(required(revision)?)?;
        if revision.is_empty()
            || revision.len() > 20
            || !revision.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(invalid(
                "contract revision must be a positive u64 decimal integer",
            ));
        }
        let revision = revision
            .parse::<u64>()
            .map_err(|_| invalid("contract revision exceeds u64"))?;
        let revision = ContractRevision::new(revision)
            .map_err(|e| CheckError::new(format, ErrorKind::Binding(e)))?;
        let candidate = CandidateId::from_git_commit(&text(required(candidate)?)?)
            .map_err(|e| CheckError::new(format, ErrorKind::Contract(e)))?;
        let expected = CandidateId::from_git_commit(&text(required(expected)?)?)
            .map_err(|e| CheckError::new(format, ErrorKind::Contract(e)))?;
        let repo = repo.unwrap_or_else(|| OsString::from("."));
        if repo.is_empty() {
            return Err(invalid("repository path cannot be empty"));
        }
        let task = TaskContract::new(
            id,
            "Check the selected candidate's committed Git revision",
            &format!(
                "{}: locally resolved HEAD equals {}",
                super::CRITERION,
                expected.as_str()
            ),
        )
        .map_err(|e| CheckError::new(format, ErrorKind::Contract(e)))?;
        Ok(Self {
            task,
            revision,
            candidate,
            expected,
            repo: PathBuf::from(repo),
        })
    }
}
