//! Black-box contract tests (spec section 7).
//!
//! These run a `fogline` executable and look only at what a user can observe:
//! exit code, stdout and stderr. They know nothing about the code behind it.
//! Set `FOGLINE_BIN` to point them at another build, such as a release
//! binary or a different implementation of the engine.

use std::env;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use fogline_testkit::f1;

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

fn fogline<I, S>(args: I) -> Outcome
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(binary())
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("the fogline binary should start");
    Outcome {
        code: output.status.code(),
        stdout: String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr should be UTF-8"),
    }
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

/// Stands in for the real scan contract, which replaces it in milestone M3.
/// It still pins how every failure must look: exit code 1, nothing on stdout,
/// and a message that starts with the program name.
#[test]
fn scan_fails_cleanly_until_it_is_implemented() {
    let fixture = f1::build().expect("fixture F1 should build");
    let path = fixture.path();
    let run = fogline([OsStr::new("scan"), path.as_os_str()]);
    assert_eq!(run.code, Some(1));
    assert_eq!(run.stdout, "");
    assert!(run.stderr.starts_with("fogline: "), "got {:?}", run.stderr);
    assert!(run.stderr.contains("not implemented"));
}
