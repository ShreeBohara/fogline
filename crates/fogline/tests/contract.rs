//! Black-box contract tests (spec section 7).
//!
//! These run a `fogline` executable and look only at what a user can observe:
//! exit code, stdout and stderr. They know nothing about the code behind it.
//! Set `FOGLINE_BIN` to point them at another build, such as a release
//! binary or a different implementation of the engine.

use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use fogline_testkit::{f1, f4, f5};

#[derive(Debug)]
struct Outcome {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn binary() -> PathBuf {
    env::var_os("FOGLINE_BIN").map_or_else(
        || PathBuf::from(env!("CARGO_BIN_EXE_fogline")),
        PathBuf::from,
    )
}

/// The binary with these arguments, ready for a test to adjust its
/// environment before running it.
fn fogline_command<I, S>(args: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(binary());
    command.args(args).stdin(Stdio::null());
    command
}

fn run(mut command: Command) -> Outcome {
    let output = command.output().expect("the fogline binary should start");
    Outcome {
        code: output.status.code(),
        stdout: String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr should be UTF-8"),
    }
}

fn fogline<I, S>(args: I) -> Outcome
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run(fogline_command(args))
}

fn scan(path: &Path) -> Outcome {
    fogline([OsStr::new("scan"), path.as_os_str()])
}

/// Every failure looks the same from outside: its exit code, nothing on
/// stdout, and one message on stderr that starts with the program name.
fn assert_fails(run: &Outcome, code: i32, says: &str) {
    assert_eq!(run.code, Some(code), "{run:?}");
    assert_eq!(run.stdout, "", "{run:?}");
    assert!(run.stderr.starts_with("fogline: "), "{run:?}");
    assert!(run.stderr.contains(says), "expected {says:?} in {run:?}");
}

/// True for `1.2.3`, and for `1.2.3-alpha.1` or `1.2.3+build`.
fn is_version(text: &str) -> bool {
    let core = text.split(['-', '+']).next().unwrap_or_default();
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

#[test]
fn version_prints_the_name_and_a_version_on_stdout() {
    let run = fogline(["version"]);
    assert_eq!(run.code, Some(0));
    assert_eq!(run.stderr, "");
    let line = run
        .stdout
        .strip_suffix('\n')
        .expect("output should end with a newline");
    let version = line
        .strip_prefix("fogline ")
        .expect("output should start with the name");
    assert!(is_version(version), "not a version: {version:?}");
}

#[test]
fn the_version_flag_prints_the_same_line_as_the_version_command() {
    let flag = fogline(["--version"]);
    assert_eq!(flag.code, Some(0));
    assert_eq!(flag.stdout, fogline(["version"]).stdout);
}

#[test]
fn version_json_is_one_line_with_the_documented_fields() {
    let run = fogline(["version", "--json"]);
    assert_eq!(run.code, Some(0));
    assert_eq!(run.stderr, "");
    assert_eq!(run.stdout.lines().count(), 1);

    let report: serde_json::Value = serde_json::from_str(&run.stdout).expect("stdout is JSON");
    let mut fields: Vec<&str> = report
        .as_object()
        .expect("a JSON object")
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(fields, ["commit", "git", "name", "target", "version"]);

    assert_eq!(report["name"], "fogline");
    assert!(report["version"].as_str().is_some_and(is_version));
    assert!(report["target"].as_str().is_some_and(|t| !t.is_empty()));
    assert!(report["commit"].is_null() || report["commit"].is_string());
    // git is present wherever these tests run: the fixtures are built with it.
    assert!(report["git"].as_str().is_some_and(is_version));
}

#[test]
fn an_unknown_flag_is_a_usage_error_and_prints_nothing_on_stdout() {
    let run = fogline(["--no-such-flag"]);
    assert_eq!(run.code, Some(2));
    assert_eq!(run.stdout, "");
    assert!(!run.stderr.is_empty());
}

#[test]
fn help_goes_to_stdout_and_names_the_commands() {
    let run = fogline(["--help"]);
    assert_eq!(run.code, Some(0));
    assert_eq!(run.stderr, "");
    for command in ["scan", "version"] {
        assert!(run.stdout.contains(command), "help should name `{command}`");
    }
}

// ---- scan: finding the repository (task 3.1) ----
//
// Until counting lands, a scan that finds its repository still fails with
// exit code 1 and says it is not implemented. Its message names the commit
// it found, which is how these tests see what discovery did.

/// F1's HEAD as git abbreviates it.
const F1_SHORT: &str = "822e353";

#[test]
fn scan_finds_the_repository_from_a_folder_inside_it() {
    let fixture = f1::build().unwrap();
    let run = scan(&fixture.path().join("src"));
    assert_fails(&run, 1, "not implemented");
    assert!(run.stderr.contains(F1_SHORT), "{run:?}");
    assert!(run.stderr.contains("on branch main"), "{run:?}");
}

#[test]
fn scan_reads_a_bare_repository() {
    let original = f1::build().unwrap();
    let bare = original.git_clone(&["--bare"]).unwrap();
    let run = scan(&bare.path());
    assert_fails(&run, 1, F1_SHORT);
}

#[test]
fn scan_says_when_a_clone_is_shallow() {
    let fixture = f5::build().unwrap();
    let run = scan(&fixture.path());
    assert_fails(&run, 1, "a shallow clone");
    assert!(run.stderr.contains(F1_SHORT), "{run:?}");
}

#[test]
fn scan_says_when_head_is_detached() {
    let fixture = f1::build().unwrap();
    fixture
        .git(["checkout", "--quiet", "--detach", "HEAD~1"])
        .unwrap();
    let run = scan(&fixture.path());
    assert_fails(&run, 1, "(detached HEAD)");
}

/// Git sets GIT_DIR while it runs hooks. A scan started from a hook must
/// still read the folder it was given.
#[test]
fn scan_ignores_git_dir_in_the_environment() {
    let fixture = f1::build().unwrap();
    let other = f4::build().unwrap();
    let mut command = fogline_command([OsStr::new("scan"), fixture.path().as_os_str()]);
    command.env("GIT_DIR", other.path().join(".git"));
    let run = run(command);
    assert_fails(&run, 1, F1_SHORT);
}

#[test]
fn scan_of_a_path_that_does_not_exist_is_a_usage_error() {
    let fixture = f1::build().unwrap();
    let run = scan(&fixture.path().join("no-such-folder"));
    assert_fails(&run, 2, "does not exist");
}

#[test]
fn scan_of_a_file_is_a_usage_error() {
    let fixture = f1::build().unwrap();
    let run = scan(&fixture.path().join("src/app.py"));
    assert_fails(&run, 2, "is not a folder");
}

#[test]
fn scan_outside_a_repository_exits_with_3() {
    let dir = tempfile::tempdir().unwrap();
    let run = scan(dir.path());
    assert_fails(&run, 3, "is not inside a git repository");
}

#[test]
fn scan_of_a_repository_with_no_commits_exits_with_3() {
    let fixture = f4::build().unwrap();
    let run = scan(&fixture.path());
    assert_fails(&run, 3, "has no commits yet");
}

#[test]
fn scan_without_git_exits_with_4() {
    let fixture = f1::build().unwrap();
    let empty = tempfile::tempdir().unwrap();
    let mut command = fogline_command([OsStr::new("scan"), fixture.path().as_os_str()]);
    command.env("PATH", empty.path());
    let run = run(command);
    assert_fails(&run, 4, "git was not found");
}

/// A script named `git` that only prints an old version stands in for an
/// old git. Windows cannot run a script found on PATH this way, so the case
/// runs on Unix; the version comparison itself has unit tests.
#[cfg(unix)]
#[test]
fn scan_with_a_git_older_than_2_25_exits_with_4() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let fixture = f1::build().unwrap();
    let bin = tempfile::tempdir().unwrap();
    let fake = bin.path().join("git");
    fs::write(&fake, "#!/bin/sh\necho 'git version 2.20.1'\n").unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();

    let mut command = fogline_command([OsStr::new("scan"), fixture.path().as_os_str()]);
    command.env("PATH", bin.path());
    let run = run(command);
    assert_fails(&run, 4, "git 2.20.1 is too old");
    assert!(run.stderr.contains("2.25.0 or newer"), "{run:?}");
}
