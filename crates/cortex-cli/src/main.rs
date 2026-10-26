//! The Cortex terminal interface and bounded local repository inspection.

mod check;

use cortex_cli as inspection;

use std::{
    error::Error,
    ffi::OsString,
    fmt,
    io::{self, Write},
    process::ExitCode,
};

const HELP: &str = "Cortex — evidence-driven software engineering

Usage: cortex [OPTIONS]
       cortex inspect [--repo <PATH>]
       cortex check [--json] --task <ID> --revision <N> --candidate <OID> --expect-head <OID> [--repo <PATH>]

Options:
  -h, --help       Print help
  -V, --version    Print version

Commands:
  inspect         Observe a local Git worktree (default: current directory)
  check           Check committed-HEAD equality with candidate-bound evidence
                  Use cortex check --help for scope and exit codes

Inspection does not verify software; no agent execution commands are available.
";

#[derive(Debug)]
enum CliError {
    InvalidArguments,
    Output(io::Error),
    Inspection(inspection::InspectionError),
    Check(check::CheckError),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArguments => f.write_str("unsupported arguments; use --help"),
            Self::Output(_) => f.write_str("could not write command output"),
            Self::Inspection(error) => error.fmt(f),
            Self::Check(error) => error.fmt(f),
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Output(error) => Some(error),
            Self::Inspection(error) => Some(error),
            Self::Check(error) => Some(error),
            Self::InvalidArguments => None,
        }
    }
}

fn run(
    mut args: impl Iterator<Item = OsString>,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
) -> Result<u8, CliError> {
    let first = args.next();
    if first.as_deref() == Some(std::ffi::OsStr::new("check")) {
        return check::run(args, output, diagnostics).map_err(CliError::Check);
    }
    if first.as_deref() == Some(std::ffi::OsStr::new("inspect")) {
        let path = match args.next() {
            None => OsString::from("."),
            Some(flag) if flag == "--repo" => args
                .next()
                .filter(|p| !p.is_empty())
                .ok_or(CliError::InvalidArguments)?,
            _ => return Err(CliError::InvalidArguments),
        };
        if args.next().is_some() {
            return Err(CliError::InvalidArguments);
        }
        let observation =
            inspection::inspect(std::path::Path::new(&path)).map_err(CliError::Inspection)?;
        return observation
            .write(output)
            .map(|()| 0)
            .map_err(CliError::Output);
    }
    if args.next().is_some() {
        return Err(CliError::InvalidArguments);
    }
    match first.as_deref().and_then(|arg| arg.to_str()) {
        None if first.is_none() => output.write_all(HELP.as_bytes()),
        Some("--help" | "-h") => output.write_all(HELP.as_bytes()),
        Some("--version" | "-V") => writeln!(output, "cortex {}", env!("CARGO_PKG_VERSION")),
        _ => return Err(CliError::InvalidArguments),
    }
    .map(|()| 0)
    .map_err(CliError::Output)
}

fn main() -> ExitCode {
    let mut output = io::stdout().lock();
    let mut diagnostics = io::stderr().lock();
    match run(std::env::args_os().skip(1), &mut output, &mut diagnostics) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            if let CliError::Check(check) = &error
                && check.write_machine_error(&mut output).is_err()
            {
                let _ = writeln!(diagnostics, "cortex: could not write check output");
                return ExitCode::FAILURE;
            }
            let _ = writeln!(diagnostics, "cortex: {error}");
            match error {
                CliError::InvalidArguments => ExitCode::from(2),
                CliError::Output(_) => ExitCode::FAILURE,
                CliError::Inspection(error) => ExitCode::from(error.exit_code()),
                CliError::Check(error) => ExitCode::from(error.exit_code()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_failure_is_propagated_with_its_cause() {
        struct BrokenOutput;
        impl Write for BrokenOutput {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::BrokenPipe))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let error = run(std::iter::empty(), &mut BrokenOutput, &mut io::sink()).unwrap_err();
        assert!(
            matches!(error, CliError::Output(ref cause) if cause.kind() == io::ErrorKind::BrokenPipe)
        );
        assert!(error.source().is_some());
    }
}
