//! Building git repositories with every source of variation pinned.

use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use tempfile::TempDir;

/// Who a fixture commit is attributed to.
#[derive(Debug, Clone, Copy)]
pub struct Identity {
    pub name: &'static str,
    pub email: &'static str,
}

/// Variables that could point git at a repository other than the fixture, or
/// inject configuration into it.
const INHERITED_GIT_VARIABLES: [&str; 9] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_PARAMETERS",
];

/// Repository settings that would otherwise differ between machines.
const PINNED_CONFIG: [(&str, &str); 4] = [
    // Store bytes exactly as written, on every platform.
    ("core.autocrlf", "false"),
    // Record every file as a plain 100644 file, whatever the file system says.
    ("core.filemode", "false"),
    ("commit.gpgsign", "false"),
    // No background maintenance while a test is reading the repository.
    ("gc.auto", "0"),
];

/// A temporary directory holding `repo/`, the repository, and `home/`, an
/// empty home directory. Pointing git at the empty home keeps the machine's
/// own configuration out of the fixture.
#[derive(Debug)]
struct Sandbox {
    dir: TempDir,
}

impl Sandbox {
    fn repo(&self) -> PathBuf {
        self.dir.path().join("repo")
    }

    fn home(&self) -> PathBuf {
        self.dir.path().join("home")
    }

    fn git<I, S>(&self, args: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new("git");
        command.current_dir(self.repo()).args(args);
        for variable in INHERITED_GIT_VARIABLES {
            command.env_remove(variable);
        }
        command
            .env("HOME", self.home())
            .env("XDG_CONFIG_HOME", self.home().join(".config"))
            .env("GIT_CONFIG_GLOBAL", self.home().join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            .env("TZ", "UTC");
        command
    }
}

/// Runs git to completion and returns its trimmed standard output.
fn run(mut command: Command, input: Option<&str>) -> io::Result<String> {
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    if let Some(input) = input {
        // Dropping the handle at the end of this block closes the pipe, which
        // is how git learns that the input is complete.
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("git's stdin was not captured"))?;
        stdin.write_all(input.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{command:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Builds a git repository one commit at a time.
#[derive(Debug)]
pub struct RepoBuilder {
    sandbox: Sandbox,
}

impl RepoBuilder {
    /// Starts an empty repository on the branch `main`.
    pub fn new() -> io::Result<Self> {
        let dir = tempfile::Builder::new()
            .prefix("fogline-fixture-")
            .tempdir()?;
        let sandbox = Sandbox { dir };
        fs::create_dir_all(sandbox.repo())?;
        fs::create_dir_all(sandbox.home())?;
        fs::write(sandbox.home().join("gitconfig"), "")?;

        run(sandbox.git(["init", "--quiet"]), None)?;
        // `git init -b` needs git 2.28. This works on every version.
        run(
            sandbox.git(["symbolic-ref", "HEAD", "refs/heads/main"]),
            None,
        )?;
        for (key, value) in PINNED_CONFIG {
            run(sandbox.git(["config", key, value]), None)?;
        }
        Ok(Self { sandbox })
    }

    /// Writes a file, creating its parent directories. `path` uses `/`.
    pub fn write(&self, path: &str, contents: &str) -> io::Result<()> {
        let target = self.sandbox.repo().join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, contents)
    }

    /// Stages every change and commits it, with `who` as both author and
    /// committer at `unix_time` UTC. The message is stored exactly as given.
    /// Returns the new commit id.
    pub fn commit(&self, who: Identity, unix_time: i64, message: &str) -> io::Result<String> {
        run(self.sandbox.git(["add", "--all"]), None)?;
        let date = format!("{unix_time} +0000");
        let mut command = self.sandbox.git([
            "commit",
            "--quiet",
            "--no-verify",
            "--allow-empty",
            "--cleanup=verbatim",
            "--file=-",
        ]);
        command
            .env("GIT_AUTHOR_NAME", who.name)
            .env("GIT_AUTHOR_EMAIL", who.email)
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_NAME", who.name)
            .env("GIT_COMMITTER_EMAIL", who.email)
            .env("GIT_COMMITTER_DATE", &date);
        run(command, Some(message))?;
        run(self.sandbox.git(["rev-parse", "HEAD"]), None)
    }

    /// Finishes the recipe and hands over the repository.
    pub fn finish(self) -> io::Result<Fixture> {
        let head = run(self.sandbox.git(["rev-parse", "HEAD"]), None)?;
        Ok(Fixture {
            sandbox: self.sandbox,
            head,
        })
    }
}

/// A finished fixture repository. Its directory is deleted when this value
/// is dropped.
#[derive(Debug)]
pub struct Fixture {
    sandbox: Sandbox,
    head: String,
}

impl Fixture {
    /// The repository's work tree.
    pub fn path(&self) -> PathBuf {
        self.sandbox.repo()
    }

    /// The commit id of HEAD.
    pub fn head(&self) -> &str {
        &self.head
    }

    /// Runs git inside the fixture and returns its trimmed standard output.
    /// Tests use this to ask git itself for the expected answer.
    pub fn git<I, S>(&self, args: I) -> io::Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        run(self.sandbox.git(args), None)
    }
}
