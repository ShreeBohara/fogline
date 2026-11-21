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

use fogline_testkit::{f1, f2, f3, f4, f5};

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

// ---- scan: the repository, its files and its history (tasks 3.1 to 3.3) ----
//
// A scan prints its summary on stderr and nothing on stdout: stdout is kept
// for data that was asked for.

/// The HEADs of F1 and F3 as git abbreviates them.
const F1_SHORT: &str = "822e353";
const F3_SHORT: &str = "a263f2c";

/// A scan that worked: exit code 0, nothing on stdout, and a summary on
/// stderr whose first line names the program and the commit.
fn assert_scanned(run: &Outcome, commit: &str) {
    assert_eq!(run.code, Some(0), "{run:?}");
    assert_eq!(run.stdout, "", "{run:?}");
    let header = run.stderr.lines().next().unwrap_or_default();
    assert!(header.starts_with("fogline "), "{run:?}");
    assert!(header.contains(&format!(" @ {commit} ")), "{run:?}");
}

/// The line of the summary that reports the files.
fn files_line(run: &Outcome) -> &str {
    run.stderr
        .lines()
        .find(|line| line.trim_start().starts_with("files "))
        .unwrap_or_default()
}

/// The line of the summary that reports the history.
fn history_line(run: &Outcome) -> &str {
    run.stderr
        .lines()
        .find(|line| line.trim_start().starts_with("history "))
        .unwrap_or_default()
}

/// The line of the summary about agents. Empty when there is none.
fn agents_line(run: &Outcome) -> &str {
    run.stderr
        .lines()
        .find(|line| line.trim_start().starts_with("agents "))
        .unwrap_or_default()
}

#[test]
fn scan_counts_the_files_and_lines_of_a_plain_repository() {
    let fixture = f1::build().unwrap();
    let run = scan(&fixture.path());
    assert_scanned(&run, F1_SHORT);

    let header = run.stderr.lines().next().unwrap_or_default();
    assert!(header.ends_with("\u{b7} repo @ 822e353 (main)"), "{run:?}");
    assert_eq!(
        files_line(&run),
        format!("  files     {} ({} lines)", f1::FILES.len(), f1::LINES)
    );
    assert_eq!(
        history_line(&run),
        format!(
            "  history   {} commits, {} lines added, {} deleted",
            f1::COMMITS,
            f1::ADDED,
            f1::DELETED
        )
    );
    assert_eq!(agents_line(&run), "  agents    no agent signatures found");
    // Four lines and no more: nothing about this repository needs a note.
    // An old git adds notes about itself, and the next test is about those.
    let about_git = notes_about_git(&run).len();
    assert_eq!(run.stderr.lines().count() - about_git, 4, "{run:?}");
}

/// F2 has a commit for every agent signature, one by a bot, and a few by
/// people. The count of commits reads "at least" and the share of lines
/// reads "estimated": the first is a lower bound and the second a guess
/// (spec section 5.2).
#[test]
fn scan_says_how_many_commits_are_agent_attributed() {
    let fixture = f2::build().unwrap();
    let run = scan(&fixture.path());
    assert_eq!(run.code, Some(0), "{run:?}");

    let commits = f2::commits();
    let of_class = |class: f2::Class| commits.iter().filter(move |entry| entry.class == class);
    let agents = of_class(f2::Class::AgentAttributed).count();
    let agent_lines: u64 = of_class(f2::Class::AgentAttributed)
        .map(|entry| entry.added)
        .sum();
    let lines: u64 = commits.iter().map(|entry| entry.added).sum();
    let percent = (agent_lines * 100 + lines / 2) / lines;

    let line = agents_line(&run);
    let start = format!(
        "  agents    at least {agents} agent-attributed commits out of {} (",
        commits.len()
    );
    let end = format!("), with an estimated {percent}% of added lines");
    assert!(line.starts_with(&start) && line.ends_with(&end), "{line}");
    // The tools are named, each with the commits that carry its mark.
    let tools = &line[start.len()..line.len() - end.len()];
    let carrying = |id: &str| {
        commits
            .iter()
            .filter(|entry| entry.signatures.contains(&id))
            .count()
    };
    for (tool, id) in [
        ("Claude Code", "claude-code"),
        ("Cursor", "cursor"),
        ("GitHub Copilot", "copilot"),
        ("Devin", "devin"),
    ] {
        let piece = format!("{tool} {}", carrying(id));
        assert!(
            tools.split(", ").any(|part| part == piece),
            "expected {piece:?} among {tools:?}"
        );
    }

    let bots = of_class(f2::Class::Automation).count();
    assert_eq!(bots, 1);
    assert!(
        run.stderr
            .lines()
            .any(|line| line == "  bots      1 commit by bots"),
        "{run:?}"
    );
}

/// The notes of a summary that are about the git that ran. They depend on
/// the machine, not on the repository.
fn notes_about_git(run: &Outcome) -> Vec<&str> {
    run.stderr
        .lines()
        .filter(|line| line.starts_with("  note      git "))
        .collect()
}

/// The git that the binary found, as its major and minor number.
fn git_in_use() -> (u32, u32) {
    let run = fogline(["version", "--json"]);
    let report: serde_json::Value = serde_json::from_str(&run.stdout).expect("stdout is JSON");
    let version = report["git"].as_str().expect("the version of git");
    let mut numbers = version.split('.').map(str::parse::<u32>);
    match (numbers.next(), numbers.next()) {
        (Some(Ok(major)), Some(Ok(minor))) => (major, minor),
        _ => panic!("not a version of git: {version:?}"),
    }
}

#[test]
fn scan_notes_what_an_old_git_cannot_do() {
    let fixture = f1::build().unwrap();
    let run = scan(&fixture.path());
    assert_scanned(&run, F1_SHORT);

    // Spec section 5.2: before 2.41 git cannot read `.gitattributes` from
    // the commit. That is the only note about an old git. Reading history
    // adds none, because git's default diff counts the same on every git
    // from 2.25 on. A newer git gets no note.
    let git = git_in_use();
    let mut expected = Vec::new();
    if git < (2, 41) {
        expected.push("reads .gitattributes from the work tree");
    }
    let notes = notes_about_git(&run);
    assert_eq!(notes.len(), expected.len(), "{run:?}");
    for (note, says) in notes.iter().zip(expected) {
        assert!(note.contains(says), "expected {says:?} in {note:?}");
    }
}

#[test]
fn scan_reports_binary_files_and_skipped_submodules() {
    let fixture = f3::build().unwrap();
    let run = scan(&fixture.path());
    assert_scanned(&run, F3_SHORT);
    assert_eq!(
        files_line(&run),
        format!(
            "  files     {} ({} lines, {} binary, 1 submodule skipped)",
            f3::FILES.len(),
            f3::LINES,
            f3::BINARY_FILES
        )
    );
    assert_eq!(
        history_line(&run),
        format!(
            "  history   {} commits (1 merge), {} lines added, {} deleted",
            f3::COMMITS,
            f3::ADDED,
            f3::DELETED
        )
    );
}

/// History is read by several git processes at once. How many there are
/// must not show in what a scan reports.
#[test]
fn scan_prints_the_same_summary_for_any_number_of_jobs() {
    for fixture in [f1::build().unwrap(), f3::build().unwrap()] {
        let path = fixture.path();
        let by_default = scan(&path);
        assert_eq!(by_default.code, Some(0), "{by_default:?}");
        for jobs in ["1", "4", "11", "64"] {
            let run = fogline([
                OsStr::new("scan"),
                OsStr::new("--jobs"),
                OsStr::new(jobs),
                path.as_os_str(),
            ]);
            assert_eq!(run.code, Some(0), "{run:?}");
            assert_eq!(run.stdout, "", "{run:?}");
            assert_eq!(run.stderr, by_default.stderr, "--jobs {jobs}");
        }
    }
}

#[test]
fn scan_with_history_none_reads_only_the_files() {
    let fixture = f3::build().unwrap();
    let run = fogline([
        OsStr::new("scan"),
        OsStr::new("--history"),
        OsStr::new("none"),
        fixture.path().as_os_str(),
    ]);
    assert_scanned(&run, F3_SHORT);
    assert_eq!(history_line(&run), "  history   not read (--history none)");
    assert_eq!(files_line(&run), files_line(&scan(&fixture.path())));
    // Attribution comes from the history, so there is nothing to say.
    assert_eq!(agents_line(&run), "");
}

#[test]
fn a_number_of_jobs_that_is_not_1_to_64_is_a_usage_error() {
    for jobs in ["0", "65", "many"] {
        let run = fogline(["scan", "--jobs", jobs]);
        assert_eq!(run.code, Some(2), "--jobs {jobs}: {run:?}");
        assert_eq!(run.stdout, "", "{run:?}");
        assert!(run.stderr.contains("--jobs"), "{run:?}");
    }
}

#[test]
fn a_history_choice_that_does_not_exist_is_a_usage_error() {
    let run = fogline(["scan", "--history", "some"]);
    assert_eq!(run.code, Some(2), "{run:?}");
    assert_eq!(run.stdout, "", "{run:?}");
    assert!(run.stderr.contains("--history"), "{run:?}");
}

/// A user's own attributes file can declare files binary, which removes
/// their line counts. It must not reach a scan: the numbers describe the
/// repository and not the machine it is scanned on.
#[test]
fn scan_ignores_the_attributes_file_of_the_user() {
    let fixture = f3::build().unwrap();
    let plain = scan(&fixture.path());
    assert_scanned(&plain, F3_SHORT);

    // Git looks for the file at `$XDG_CONFIG_HOME/git/attributes`.
    let config = tempfile::tempdir().unwrap();
    std::fs::create_dir(config.path().join("git")).unwrap();
    std::fs::write(
        config.path().join("git").join("attributes"),
        "*.py binary\n*.md binary\n",
    )
    .unwrap();
    let mut command = fogline_command([OsStr::new("scan"), fixture.path().as_os_str()]);
    command.env("XDG_CONFIG_HOME", config.path());
    let run = run(command);

    assert_scanned(&run, F3_SHORT);
    assert_eq!(run.stderr, plain.stderr);
}

/// Rule 1 of the product: a scan never writes into the repository it reads.
/// Every file under the fixture, its `.git` folder included, has to be the
/// same byte for byte afterwards, and nothing may appear or disappear.
#[test]
fn scan_leaves_the_repository_exactly_as_it_was() {
    let fixture = f3::build().unwrap();
    let before = fixture.snapshot().unwrap();

    let run = scan(&fixture.path());
    assert_scanned(&run, F3_SHORT);

    assert!(
        fixture.snapshot().unwrap() == before,
        "the scan changed the repository"
    );
}

/// In a partial clone git downloads a missing object when a command asks
/// for it. A scan never uses the network, so it stops instead, and the
/// clone stays as it was.
#[test]
fn scan_of_a_partial_clone_that_lacks_its_files_exits_with_3_and_downloads_nothing() {
    let source = f1::build().unwrap();
    let clone = source.partial_clone(false).unwrap();
    let before = clone.snapshot().unwrap();

    let run = scan(&clone.path());
    assert_fails(&run, 3, "is a partial clone");
    assert!(
        run.stderr
            .contains("6 objects that this commit needs are not in it"),
        "{run:?}"
    );
    assert!(run.stderr.contains("never downloads anything"), "{run:?}");

    assert!(
        clone.snapshot().unwrap() == before,
        "the scan changed the clone"
    );
}

/// Counting the lines of history needs every version of every file. A
/// partial clone that checked out HEAD holds only the newest ones, so a
/// scan of it stops too, and says how to scan the files alone.
#[test]
fn scan_of_a_partial_clone_without_its_history_exits_with_3_and_downloads_nothing() {
    let source = f1::build().unwrap();
    let clone = source.partial_clone(true).unwrap();
    let before = clone.snapshot().unwrap();

    let run = scan(&clone.path());
    assert_fails(&run, 3, "is a partial clone");
    assert!(
        run.stderr
            .contains("3 objects that its history needs are not in it"),
        "{run:?}"
    );
    assert!(run.stderr.contains("never downloads anything"), "{run:?}");
    assert!(run.stderr.contains("`--history none`"), "{run:?}");

    assert!(
        clone.snapshot().unwrap() == before,
        "the scan changed the clone"
    );
}

#[test]
fn scan_of_a_partial_clone_reads_the_files_when_history_is_left_out() {
    let source = f1::build().unwrap();
    let clone = source.partial_clone(true).unwrap();
    let before = clone.snapshot().unwrap();

    let run = fogline([
        OsStr::new("scan"),
        OsStr::new("--history"),
        OsStr::new("none"),
        clone.path().as_os_str(),
    ]);
    assert_scanned(&run, F1_SHORT);
    assert_eq!(files_line(&run), files_line(&scan(&source.path())));

    assert!(
        clone.snapshot().unwrap() == before,
        "the scan changed the clone"
    );
}

#[test]
fn scan_of_a_partial_clone_that_lacks_nothing_works_and_downloads_nothing() {
    let source = f1::build().unwrap();
    let clone = source.complete_partial_clone().unwrap();
    let before = clone.snapshot().unwrap();

    let run = scan(&clone.path());
    assert_scanned(&run, F1_SHORT);
    let whole = scan(&source.path());
    assert_eq!(files_line(&run), files_line(&whole));
    assert_eq!(history_line(&run), history_line(&whole));

    assert!(
        clone.snapshot().unwrap() == before,
        "the scan changed the clone"
    );
}

#[test]
fn scan_finds_the_repository_from_a_folder_inside_it() {
    let fixture = f1::build().unwrap();
    let from_the_top = scan(&fixture.path());
    let from_inside = scan(&fixture.path().join("src"));
    assert_scanned(&from_inside, F1_SHORT);
    assert_eq!(from_inside.stderr, from_the_top.stderr);
}

#[test]
fn scan_reads_a_bare_repository() {
    let original = f3::build().unwrap();
    let bare = original.git_clone(&["--bare"]).unwrap();
    let run = scan(&bare.path());
    assert_scanned(&run, F3_SHORT);
    let from_the_work_tree = scan(&original.path());
    assert_eq!(files_line(&run), files_line(&from_the_work_tree));
    assert_eq!(history_line(&run), history_line(&from_the_work_tree));
}

#[test]
fn scan_says_when_a_clone_is_shallow() {
    let fixture = f5::build().unwrap();
    let run = scan(&fixture.path());
    assert_scanned(&run, F1_SHORT);
    assert!(
        run.stderr.contains("  note      this is a shallow clone"),
        "{run:?}"
    );
    // The one commit it holds looks like the first, adding every line.
    assert_eq!(
        history_line(&run),
        format!("  history   1 commit, {} lines added, 0 deleted", f5::ADDED)
    );
}

#[test]
fn scan_says_when_head_is_detached() {
    let fixture = f1::build().unwrap();
    fixture
        .git(["checkout", "--quiet", "--detach", "HEAD~1"])
        .unwrap();
    let run = scan(&fixture.path());
    assert_eq!(run.code, Some(0), "{run:?}");
    let header = run.stderr.lines().next().unwrap_or_default();
    assert!(header.ends_with(" (detached)"), "{run:?}");
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
    assert_scanned(&run, F1_SHORT);
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
