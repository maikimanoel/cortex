//! Dedicated single-threaded bootstrap for the strict trusted-executable path.
//! It blocks before namespace setup, then again before executing the payload.
use super::{ExecutionError, FileRole, FileSpec, PinnedFile};
use crate::{
    domain_bootstrap::{connect_now, frame_read, frame_write, ticks, token},
    native_linux::{NativeIsolation, install_view},
};
use serde_json::{Value, json};
use std::{
    os::{fd::AsRawFd, unix::process::CommandExt},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

fn text(v: &Value) -> Result<&str, ExecutionError> {
    v.as_str().ok_or(ExecutionError::Policy)
}
fn number(v: &Value) -> Result<u64, ExecutionError> {
    v.as_u64().ok_or(ExecutionError::Policy)
}
pub(crate) fn file_value(spec: &FileSpec) -> Value {
    json!([
        spec.source,
        spec.destination,
        spec.sha256.to_vec(),
        spec.max_bytes,
        match spec.role {
            FileRole::Executable => 0,
            FileRole::Runtime => 1,
            FileRole::Input => 2,
        }
    ])
}
fn file_spec(v: &Value) -> Result<FileSpec, ExecutionError> {
    if v.as_array().is_none_or(|a| a.len() != 5) {
        return Err(ExecutionError::Policy);
    }
    let digest = v[2]
        .as_array()
        .ok_or(ExecutionError::Policy)?
        .iter()
        .map(|v| number(v).and_then(|n| u8::try_from(n).map_err(|_| ExecutionError::Policy)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FileSpec {
        source: PathBuf::from(text(&v[0])?),
        destination: PathBuf::from(text(&v[1])?),
        sha256: digest.try_into().map_err(|_| ExecutionError::Policy)?,
        max_bytes: number(&v[3])?,
        role: match number(&v[4])? {
            0 => FileRole::Executable,
            1 => FileRole::Runtime,
            2 => FileRole::Input,
            _ => return Err(ExecutionError::Policy),
        },
    })
}
/// Run only in the purpose-built bootstrap executable. The setup frame arrives
/// only after parent-owned cgroup attachment; exact readiness precedes release.
/// Failure never falls back to host execution or a shell.
pub fn run(arguments: &[String]) -> Result<(), ExecutionError> {
    if arguments.len() != 6
        || !token(&arguments[1])
        || arguments[2].len() != 64
        || !arguments[2].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(ExecutionError::Policy);
    }
    let wall = arguments[5]
        .parse::<u64>()
        .map_err(|_| ExecutionError::Policy)?;
    if wall == 0 || wall > 15000 {
        return Err(ExecutionError::Policy);
    }
    let end = Instant::now()
        .checked_add(Duration::from_millis(wall))
        .ok_or(ExecutionError::Policy)?;
    let mut channel = connect_now(Path::new(&arguments[0]))?;
    let setup = frame_read(&mut channel, end)?;
    if setup.as_array().is_none_or(|a| a.len() != 9)
        || setup[0] != 1
        || setup[1] != "setup"
        || setup[2] != arguments[2]
    {
        return Err(ExecutionError::Identity);
    }
    let isolation = NativeIsolation::inspect(
        Path::new(&arguments[3]),
        number(&setup[3])?,
        number(&setup[4])?,
    )?;
    if isolation.digest() != arguments[4] {
        return Err(ExecutionError::Identity);
    }
    let specs = setup[5]
        .as_array()
        .ok_or(ExecutionError::Policy)?
        .iter()
        .map(file_spec)
        .collect::<Result<Vec<_>, _>>()?;
    if specs.is_empty()
        || specs.len() > 16
        || specs
            .iter()
            .filter(|f| f.role == FileRole::Executable)
            .count()
            != 1
    {
        return Err(ExecutionError::Policy);
    }
    // Parent plan is the policy authority. Bootstrap separately rejects invalid
    // mount geometry and binds precisely the bytes it verifies and seals.
    for (i, a) in specs.iter().enumerate() {
        super::policy::destination(&a.destination)?;
        for b in &specs[..i] {
            if a.destination.starts_with(&b.destination)
                || b.destination.starts_with(&a.destination)
            {
                return Err(ExecutionError::Policy);
            }
        }
    }
    let pins = specs
        .iter()
        .map(PinnedFile::inspect)
        .collect::<Result<Vec<_>, _>>()?;
    let args = setup[6]
        .as_array()
        .ok_or(ExecutionError::Policy)?
        .iter()
        .map(|v| text(v).map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    let env = setup[7]
        .as_array()
        .ok_or(ExecutionError::Policy)?
        .iter()
        .map(|v| Ok((text(&v[0])?.to_owned(), text(&v[1])?.to_owned())))
        .collect::<Result<Vec<_>, ExecutionError>>()?;
    let cwd = PathBuf::from(text(&setup[8])?);
    if !cwd.starts_with("/scratch")
        || cwd.components().any(|c| {
            !matches!(
                c,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
    {
        return Err(ExecutionError::Policy);
    }
    let executable = specs
        .iter()
        .find(|f| f.role == FileRole::Executable)
        .ok_or(ExecutionError::Policy)?
        .destination
        .clone();
    let pid = std::process::id();
    let start = ticks(pid)?;
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    install_view(&isolation, channel.as_raw_fd(), &pins).map_err(|e| ExecutionError::Io(e.kind))?;
    drop(pins);
    std::fs::create_dir_all(&cwd)?;
    frame_write(
        &mut channel,
        &json!([
            1,
            "hello",
            arguments[1],
            pid,
            start,
            boot.trim(),
            arguments[2]
        ]),
        end,
    )?;
    if frame_read(&mut channel, end)? != json!([1, "release", arguments[1], arguments[2]]) {
        return Err(ExecutionError::Identity);
    }
    // No descriptor or environment authority crosses exec except the three
    // bounded data pipes. The control channel is explicitly closed first.
    drop(channel);
    std::env::set_current_dir(cwd)?;
    let error = Command::new(executable)
        .args(args)
        .env_clear()
        .envs(env)
        .exec();
    Err(ExecutionError::Io(error.kind()))
}
