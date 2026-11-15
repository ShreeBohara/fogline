//! The `fogline` command line.
//!
//! This binary owns everything a user sees: argument parsing, printing and
//! exit codes. The facts come from `fogline-core`.

mod cli;
mod version;

use std::io;
use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command};

/// Exit code for a failure that is not a usage error (spec section 5.1).
/// Usage errors exit with 2, which clap produces itself.
const EXIT_FAILURE: u8 = 1;

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("fogline: {message}");
            ExitCode::from(EXIT_FAILURE)
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Version { json } => version::write(&mut io::stdout().lock(), json)
            .map_err(|err| format!("could not write to stdout: {err}")),
        Command::Scan { path } => Err(format!(
            "cannot scan `{}`: `scan` is not implemented yet. \
             It arrives with milestone M3 of Phase 1 (docs/specs/phase-1.md).",
            path.display()
        )),
    }
}
