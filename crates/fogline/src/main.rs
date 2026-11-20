//! The `fogline` command line.
//!
//! This binary owns everything a user sees: argument parsing, printing and
//! exit codes. The facts come from `fogline-core`.

mod cli;
mod scan;
mod version;

use std::io;
use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command};

/// Exit codes (spec section 5.1). They tell a script which kind of problem
/// happened without making it read the message. clap exits with
/// [`EXIT_USAGE`] on its own for arguments it cannot parse.
const EXIT_FAILURE: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_NOT_A_REPOSITORY: u8 = 3;
const EXIT_GIT_UNUSABLE: u8 = 4;

/// A run that went wrong: the exit code, and a message that says what
/// happened and what to do next.
#[derive(Debug)]
struct Failure {
    code: u8,
    message: String,
}

impl Failure {
    fn new(code: u8, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            eprintln!("fogline: {}", failure.message);
            ExitCode::from(failure.code)
        }
    }
}

fn run(cli: Cli) -> Result<(), Failure> {
    match cli.command {
        Command::Version { json } => version::write(&mut io::stdout().lock(), json)
            .map_err(|err| Failure::new(EXIT_FAILURE, format!("could not write to stdout: {err}"))),
        Command::Scan { path } => scan::run(&path),
    }
}
