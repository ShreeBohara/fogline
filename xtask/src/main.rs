//! Developer commands for this repository. Run `cargo xtask` to list them.
//!
//! They live in Rust, not in shell scripts, so they behave the same on
//! macOS, Linux and Windows.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const USAGE: &str = "\
Usage: cargo xtask <command>

Commands:
  ci [--skip-viewer]   Run the checks CI runs: formatting, lints and tests for
                       the Rust workspace, then lint, type check, tests and
                       build for the viewer
";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let outcome = match args.split_first() {
        None => Err(format!("no command given\n\n{USAGE}")),
        Some((command, rest)) => match command.as_str() {
            "ci" => ci(rest),
            "help" | "--help" | "-h" => {
                print!("{USAGE}");
                Ok(())
            }
            other => Err(format!("unknown command `{other}`\n\n{USAGE}")),
        },
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}

fn ci(args: &[String]) -> Result<(), String> {
    let skip_viewer = match args {
        [] => false,
        [flag] if flag == "--skip-viewer" => true,
        _ => return Err(format!("`ci` accepts only --skip-viewer\n\n{USAGE}")),
    };
    let root = workspace_root()?;

    // Cargo sets CARGO for the commands it runs, so the same toolchain that
    // is running xtask runs the checks.
    let cargo_program = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let cargo = |args: &[&str]| {
        let mut command = Command::new(&cargo_program);
        command.args(args).current_dir(&root);
        command
    };

    step("format", cargo(&["fmt", "--all", "--", "--check"]))?;
    step(
        "lint",
        cargo(&[
            "clippy",
            "--workspace",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ]),
    )?;
    if succeeds(cargo(&["nextest", "--version"])) {
        step(
            "test",
            cargo(&["nextest", "run", "--workspace", "--locked"]),
        )?;
    } else {
        step("test", cargo(&["test", "--workspace", "--locked"]))?;
    }

    if skip_viewer {
        println!("\n==> viewer checks skipped");
        return Ok(());
    }

    let pnpm_program = if cfg!(windows) { "pnpm.cmd" } else { "pnpm" };
    let pnpm = |args: &[&str]| {
        let mut command = Command::new(pnpm_program);
        command.args(args).current_dir(&root);
        command
    };
    if !succeeds(pnpm(&["--version"])) {
        return Err(
            "pnpm was not found. Install Node 26 and pnpm, or rerun with --skip-viewer.".to_owned(),
        );
    }
    step("viewer: install", pnpm(&["install", "--frozen-lockfile"]))?;
    step("viewer: lint", pnpm(&["run", "lint"]))?;
    let viewer = |script: &'static str| pnpm(&["--filter", "@fogline/viewer", "run", script]);
    step("viewer: type check", viewer("typecheck"))?;
    step("viewer: test", viewer("test"))?;
    step("viewer: build", viewer("build"))
}

/// Runs one named step, letting its output through.
fn step(name: &str, mut command: Command) -> Result<(), String> {
    println!("\n==> {name}");
    let status = command
        .status()
        .map_err(|err| format!("could not start {:?}: {err}", command.get_program()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{name} failed"))
    }
}

/// Runs a command silently and reports whether it worked.
fn succeeds(mut command: Command) -> bool {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn workspace_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "the xtask crate has no parent directory".to_owned())
}
