//! Advertised Codex CLI transport, with a mandatory live-provider hold.

use cortex_core::{
    program::{
        ProgramState,
        executor::{ExecutorError, ExecutorIdentity, PreparedInput},
    },
    temporal::SourceReference,
};
use cortex_harness::native_execution::{ExecutionError, FileRole, FileSpec, PinnedFile};
use std::{error::Error, fmt, path::Path};

const VERSION: &str = "0.167.0";
const ARGS: &[&str] = &[
    "exec",
    "--json",
    "--ephemeral",
    "--ignore-user-config",
    "--color",
    "never",
    "-",
];

/// Selected Codex CLI transport cannot currently become a live execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexCliHold {
    /// The prepared descriptor does not identify the inspected version.
    Descriptor,
    /// The exact prepared package is no longer current.
    Input(ExecutorError),
    /// The supplied executable size exceeds the current program envelope.
    ExecutableBytes,
    /// Exact executable identity, sealed bytes or safe staging could not be verified.
    ExecutableIntegrity(ExecutionError),
    /// Live provider authority, installed-version conformance and credential
    /// mediation under independently qualified effective controls are absent.
    LiveProviderUnavailable,
}
impl fmt::Display for CodexCliHold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Descriptor => "Codex CLI: selected transport version unavailable",
            Self::Input(_) => "Codex CLI: exact current package unavailable",
            Self::ExecutableBytes => "Codex CLI: executable outside current byte envelope",
            Self::ExecutableIntegrity(_) => "Codex CLI: exact immutable executable unavailable",
            Self::LiveProviderUnavailable => "Codex CLI: live provider unavailable",
        })
    }
}
impl Error for CodexCliHold {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(error) => Some(error),
            Self::ExecutableIntegrity(error) => Some(error),
            _ => None,
        }
    }
}

/// Read-only lowering of the retained package into the selected advertised
/// stdin/JSON-lines shape. There is no process constructor or credential field.
pub struct CodexCliInspection<'a> {
    input: &'a PreparedInput,
    workspace: &'a Path,
    executable_hold: Option<CodexCliHold>,
    executable_bytes: u64,
    byte_limit: u64,
}
impl CodexCliInspection<'_> {
    /// The descriptor's exact executable reference, with its original provenance.
    pub fn executable(&self) -> &SourceReference {
        self.input.descriptor().identity().executable()
    }

    /// Working directory selected by the current task's candidate contract.
    pub fn workspace(&self) -> &Path {
        self.workspace
    }

    /// Advertised argument shape without a prompt argument or resume option.
    pub fn args(&self) -> &'static [&'static str] {
        ARGS
    }

    /// Complete canonical package bytes, without retokenizing or truncating.
    pub fn stdin(&self) -> &[u8] {
        self.input.bytes()
    }

    /// A size conflict with the retained program bound, if one was supplied.
    pub fn executable_hold(&self) -> Option<&CodexCliHold> {
        self.executable_hold.as_ref()
    }

    /// Pin the explicitly selected digest under the existing executable byte bound.
    ///
    /// This compatible preparation uses Harness seals and does not turn a
    /// pathname hash or later source replacement into executable identity.
    /// It retains a nonlaunchable pin while host, authority and provider gates
    /// remain held. The expected digest is a supplied source requirement.
    pub fn pin_executable(
        &self,
        spec: &FileSpec,
    ) -> Result<PinnedCodexCliExecutable, CodexCliHold> {
        if self.executable_hold.is_some() || spec.max_bytes > self.byte_limit {
            return Err(CodexCliHold::ExecutableBytes);
        }
        if spec.role != FileRole::Executable {
            return Err(CodexCliHold::ExecutableIntegrity(ExecutionError::Policy));
        }
        let pin = PinnedFile::inspect(spec).map_err(CodexCliHold::ExecutableIntegrity)?;
        if pin.len() != self.executable_bytes {
            return Err(CodexCliHold::ExecutableIntegrity(ExecutionError::Identity));
        }
        Ok(PinnedCodexCliExecutable { pin })
    }

    /// Advertised flags leave live authorization and effective controls unproved.
    pub fn live_hold(&self) -> CodexCliHold {
        CodexCliHold::LiveProviderUnavailable
    }
}

/// Existing Harness sealed-copy contract for compatible adapter preparation.
///
/// The owner-selected file may later change through its original path. This
/// descriptor is independent of that path and creates no execution authority.
pub struct PinnedCodexCliExecutable {
    pin: PinnedFile,
}
impl PinnedCodexCliExecutable {
    /// Recheck immutability and the exact originally approved digest.
    pub fn verify(&self) -> Result<(), CodexCliHold> {
        self.pin.verify().map_err(CodexCliHold::ExecutableIntegrity)
    }

    /// Exact captured executable size.
    pub fn len(&self) -> u64 {
        self.pin.len()
    }

    /// Whether the captured executable is empty.
    pub fn is_empty(&self) -> bool {
        self.pin.is_empty()
    }
}

/// Inspect the advertised exact-version transport for a prepared package.
///
/// `selected` is the original selected installation identity, whose provenance
/// remains caller-supplied. Rechecking Core's current package correspondence may
/// refuse. The supplied executable byte count can reveal a size conflict, but
/// this inspection does not read the binary, launch, read configuration or
/// forward credentials. Even the matching under-limit shape reports a live hold.
pub fn inspect_codex_cli_invocation<'a>(
    state: &'a ProgramState,
    input: &'a PreparedInput,
    selected: &ExecutorIdentity,
    executable_bytes: u64,
) -> Result<CodexCliInspection<'a>, CodexCliHold> {
    let descriptor = input.descriptor();
    if selected.name() != "codex-cli"
        || selected.version() != VERSION
        || descriptor.identity() != selected
    {
        return Err(CodexCliHold::Descriptor);
    }
    input
        .check(state, descriptor, input.requirements(), input.bytes())
        .map_err(CodexCliHold::Input)?;
    let task = state
        .definition()
        .graph()
        .tasks()
        .iter()
        .find(|task| task.id() == &input.package().request().task)
        .ok_or(CodexCliHold::Input(ExecutorError::Program(
            cortex_core::program::ProgramError::Missing,
        )))?;
    let byte_limit = state.definition().limits().bytes as u64;
    let executable_hold = (executable_bytes == 0 || executable_bytes > byte_limit)
        .then_some(CodexCliHold::ExecutableBytes);
    Ok(CodexCliInspection {
        input,
        workspace: task.context().contract().candidate().repository().root(),
        executable_hold,
        executable_bytes,
        byte_limit,
    })
}
