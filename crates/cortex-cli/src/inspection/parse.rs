//! Parse only the declared Git formats. Unknown or incomplete states fail closed.

use super::{Change, Entry, Head, InspectionError, MAX_ENTRIES};
use cortex_core::CandidateId;

fn split_once(bytes: &[u8], separator: u8) -> Option<(&[u8], &[u8])> {
    let position = bytes.iter().position(|byte| *byte == separator)?;
    Some((&bytes[..position], &bytes[position + 1..]))
}

pub(super) fn records(bytes: &[u8]) -> Result<impl Iterator<Item = &[u8]>, InspectionError> {
    if (!bytes.is_empty() && !bytes.ends_with(&[0]))
        || bytes.first() == Some(&0)
        || bytes.windows(2).any(|pair| pair == [0, 0])
    {
        return Err(InspectionError::Malformed);
    }
    Ok(bytes.split(|b| *b == 0).filter(|r| !r.is_empty()))
}

fn revision(bytes: &[u8]) -> Result<CandidateId, InspectionError> {
    let text = std::str::from_utf8(bytes).map_err(|_| InspectionError::Malformed)?;
    CandidateId::from_git_commit(text).map_err(|_| InspectionError::Malformed)
}

pub(super) fn index(bytes: &[u8]) -> Result<(), InspectionError> {
    for (n, record) in records(bytes)?.enumerate() {
        if n >= MAX_ENTRIES {
            return Err(InspectionError::Limit("tracked entries"));
        }
        let (metadata, path) = split_once(record, b'\t').ok_or(InspectionError::Malformed)?;
        let fields: Vec<_> = metadata.splitn(5, |b| *b == b' ').collect();
        if fields.len() != 4 || path.is_empty() {
            return Err(InspectionError::Malformed);
        }
        if fields[0] != b"H" || fields[3] != b"0" {
            return Err(InspectionError::Unsupported(
                "unmerged, sparse, or assume-unchanged index entries",
            ));
        }
        if fields[1] == b"160000" {
            return Err(InspectionError::Unsupported("submodules"));
        }
        if !matches!(fields[1], b"100644" | b"100755" | b"120000") {
            return Err(InspectionError::Unsupported("index file mode"));
        }
        revision(fields[2])?;
    }
    Ok(())
}

pub(super) fn configuration(bytes: &[u8]) -> Result<(), InspectionError> {
    for record in records(bytes)? {
        let (key, value) = split_once(record, b'\n').unwrap_or((record, b""));
        if key.starts_with(b"filter.") {
            return Err(InspectionError::Unsupported("configured content filters"));
        }
        if key == b"extensions.partialclone" || key.ends_with(b".promisor") {
            return Err(InspectionError::Unsupported(
                "partial clones/promisor objects",
            ));
        }
        if key == b"core.sparsecheckout" && !matches!(value, b"false" | b"0" | b"no" | b"off") {
            return Err(InspectionError::Unsupported("sparse checkout"));
        }
    }
    Ok(())
}

fn change(byte: u8) -> Result<Option<Change>, InspectionError> {
    match byte {
        b'.' => Ok(None),
        b'M' => Ok(Some(Change::Modified)),
        b'A' => Ok(Some(Change::Added)),
        b'D' => Ok(Some(Change::Deleted)),
        b'T' => Ok(Some(Change::TypeChanged)),
        _ => Err(InspectionError::Malformed),
    }
}

pub(super) fn status(bytes: &[u8]) -> Result<(Head, Vec<Entry>), InspectionError> {
    let mut oid = None;
    let mut branch = None;
    let mut entries = Vec::new();
    for record in records(bytes)? {
        if let Some(value) = record.strip_prefix(b"# branch.oid ") {
            if oid.replace(value).is_some() {
                return Err(InspectionError::Malformed);
            }
        } else if let Some(value) = record.strip_prefix(b"# branch.head ") {
            if branch.replace(value).is_some() {
                return Err(InspectionError::Malformed);
            }
        } else if record.starts_with(b"# branch.upstream ") || record.starts_with(b"# branch.ab ") {
            // Git can emit branch.ab even with --no-ahead-behind. These optional
            // local tracking headers are retained for comparison, not presented.
        } else if let Some(path) = record.strip_prefix(b"? ") {
            if path.is_empty() {
                return Err(InspectionError::Malformed);
            }
            if path.ends_with(b"/") {
                return Err(InspectionError::Unsupported(
                    "unexpanded untracked directory or nested repository",
                ));
            }
            entries.push(Entry::Untracked {
                path: path.to_vec(),
            });
        } else if record.starts_with(b"1 ") {
            let fields: Vec<_> = record.splitn(9, |b| *b == b' ').collect();
            if fields.len() != 9 || fields[1].len() != 2 || fields[8].is_empty() {
                return Err(InspectionError::Malformed);
            }
            if fields[2] != b"N..." || fields[3..6].contains(&b"160000".as_slice()) {
                return Err(InspectionError::Unsupported("submodules"));
            }
            for mode in &fields[3..6] {
                if !matches!(*mode, b"000000" | b"100644" | b"100755" | b"120000") {
                    return Err(InspectionError::Unsupported("worktree file mode"));
                }
            }
            revision(fields[6])?;
            revision(fields[7])?;
            let staged = change(fields[1][0])?;
            let unstaged = change(fields[1][1])?;
            if staged.is_none() && unstaged.is_none() {
                return Err(InspectionError::Malformed);
            }
            entries.push(Entry::Tracked {
                path: fields[8].to_vec(),
                staged,
                unstaged,
            });
        } else if record.starts_with(b"u ") {
            return Err(InspectionError::Unsupported("unresolved merges"));
        } else {
            return Err(InspectionError::Malformed);
        }
        if entries.len() > MAX_ENTRIES {
            return Err(InspectionError::Limit("changed entries"));
        }
    }
    let oid = oid.ok_or(InspectionError::Malformed)?;
    let branch = branch.ok_or(InspectionError::Malformed)?;
    if branch.is_empty() || branch.iter().any(|b| b.is_ascii_control()) {
        return Err(InspectionError::Malformed);
    }
    let head = match (oid, branch) {
        (b"(initial)", b"(detached)") => return Err(InspectionError::Malformed),
        (b"(initial)", name) => Head::Unborn {
            branch: name.to_vec(),
        },
        (oid, b"(detached)") => Head::Detached {
            commit: revision(oid)?,
        },
        (oid, name) => Head::Attached {
            branch: name.to_vec(),
            commit: revision(oid)?,
        },
    };
    // Git does not promise status entry order. Keep the typed result deterministic.
    entries.sort_by(|a, b| a.path().cmp(b.path()));
    if entries
        .windows(2)
        .any(|pair| pair[0].path() == pair[1].path())
    {
        return Err(InspectionError::Malformed);
    }
    Ok((head, entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_or_unknown_records_cannot_be_clean() {
        for bytes in [
            b"".as_slice(),
            b"# branch.oid (initial)\0# branch.head main\0\0",
            b"# branch.oid (initial)\0",
            b"# branch.oid (initial)\0# branch.head main\0x surprise\0",
            b"# branch.oid (initial)\0# branch.head main",
            b"# branch.oid (initial)\0# branch.head (detached)\0",
            b"# branch.oid (initial)\0# branch.head main\0? \0",
        ] {
            assert!(status(bytes).is_err());
        }
        assert!(index(b"H 100644 not-an-id 0\tx\0").is_err());
        assert!(matches!(
            index(b"h 100644 x 0\tx\0"),
            Err(InspectionError::Unsupported(_))
        ));
    }

    #[test]
    fn status_entry_limit_includes_untracked_paths() {
        let mut bytes = b"# branch.oid (initial)\0# branch.head main\0".to_vec();
        for n in 0..MAX_ENTRIES {
            bytes.extend(format!("? f{n}\0").bytes());
        }
        assert_eq!(status(&bytes).unwrap().1.len(), MAX_ENTRIES);
        bytes.extend_from_slice(b"? overflow\0");
        assert!(matches!(status(&bytes), Err(InspectionError::Limit(_))));
    }
}
