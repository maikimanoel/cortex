//! Advertised Codex CLI transport, with a mandatory live-provider hold.

use cortex_core::{
    program::{
        ProgramState,
        executor::{ExecutorError, ExecutorIdentity, PreparedInput},
    },
    temporal::SourceReference,
};
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
            Self::LiveProviderUnavailable => "Codex CLI: live provider unavailable",
        })
    }
}
impl Error for CodexCliHold {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(error) => Some(error),
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

    /// Advertised flags leave live authorization and effective controls unproved.
    pub fn live_hold(&self) -> CodexCliHold {
        CodexCliHold::LiveProviderUnavailable
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
    let executable_hold = (executable_bytes == 0
        || executable_bytes > state.definition().limits().bytes as u64)
        .then_some(CodexCliHold::ExecutableBytes);
    Ok(CodexCliInspection {
        input,
        workspace: task.context().contract().candidate().repository().root(),
        executable_hold,
    })
}
