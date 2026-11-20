//! Building git repositories with every source of variation pinned.

use std::collections::BTreeMap;
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
    /// Makes the temporary directory, with an empty `repo/` and a `home/`
    /// that holds an empty git configuration.
    fn new() -> io::Result<Self> {
        let dir = tempfile::Builder::new()
            .prefix("fogline-fixture-")
            .tempdir()?;
        let sandbox = Sandbox { dir };
        fs::create_dir_all(sandbox.repo())?;
        fs::create_dir_all(sandbox.home())?;
        fs::write(sandbox.home().join("gitconfig"), "")?;
        Ok(sandbox)
    }

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

/// Runs git to completion and returns its standard output, byte for byte.
fn run_bytes(mut command: Command, input: Option<&[u8]>) -> io::Result<Vec<u8>> {
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
        stdin.write_all(input)?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{command:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

/// Runs git to completion and returns its trimmed standard output as text.
fn run(command: Command, input: Option<&str>) -> io::Result<String> {
    let stdout = run_bytes(command, input.map(str::as_bytes))?;
    Ok(String::from_utf8_lossy(&stdout).trim().to_owned())
}

/// Makes `who` the author and the committer of the commit that `command`
/// creates, at `unix_time` UTC.
fn sign(command: &mut Command, who: Identity, unix_time: i64) {
    let date = format!("{unix_time} +0000");
    command
        .env("GIT_AUTHOR_NAME", who.name)
        .env("GIT_AUTHOR_EMAIL", who.email)
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_NAME", who.name)
        .env("GIT_COMMITTER_EMAIL", who.email)
        .env("GIT_COMMITTER_DATE", &date);
}

/// The commit HEAD points to, or `None` in a repository with no commits.
fn head_of(sandbox: &Sandbox) -> io::Result<Option<String>> {
    let output = sandbox
        .git(["rev-parse", "--verify", "--quiet", "HEAD"])
        .stdin(Stdio::null())
        .output()?;
    match output.status.code() {
        Some(0) => Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        )),
        // `--verify --quiet` exits with 1, silently, when HEAD names no commit.
        Some(1) => Ok(None),
        _ => Err(io::Error::other(format!(
            "git rev-parse HEAD failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))),
    }
}

/// An entry that is written straight into git's index and has no file in
/// the work tree: a symbolic link or a submodule.
#[derive(Debug)]
struct IndexOnly {
    /// git's file mode: `120000` for a link, `160000` for a submodule.
    mode: &'static str,
    /// The blob that holds the link's target, or the submodule's commit.
    id: String,
    path: String,
}

/// Builds a git repository one commit at a time.
#[derive(Debug)]
pub struct RepoBuilder {
    sandbox: Sandbox,
    index_only: Vec<IndexOnly>,
}

impl RepoBuilder {
    /// Starts an empty repository on the branch `main`.
    pub fn new() -> io::Result<Self> {
        let sandbox = Sandbox::new()?;
        run(sandbox.git(["init", "--quiet"]), None)?;
        // `git init -b` needs git 2.28. This works on every version.
        run(
            sandbox.git(["symbolic-ref", "HEAD", "refs/heads/main"]),
            None,
        )?;
        for (key, value) in PINNED_CONFIG {
            run(sandbox.git(["config", key, value]), None)?;
        }
        Ok(Self {
            sandbox,
            index_only: Vec::new(),
        })
    }

    /// Writes a file, creating its parent directories. `path` uses `/`.
    pub fn write(&self, path: &str, contents: &str) -> io::Result<()> {
        self.write_bytes(path, contents.as_bytes())
    }

    /// Writes a file from raw bytes, for content that is not text.
    pub fn write_bytes(&self, path: &str, contents: &[u8]) -> io::Result<()> {
        let target = self.sandbox.repo().join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, contents)
    }

    /// Deletes a file. The next commit records the deletion.
    pub fn remove(&self, path: &str) -> io::Result<()> {
        fs::remove_file(self.sandbox.repo().join(path))
    }

    /// Moves a file. A commit records only the result; git works out that
    /// it was a rename later, when it compares the two commits.
    pub fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        let target = self.sandbox.repo().join(to);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(self.sandbox.repo().join(from), target)
    }

    /// Adds a symbolic link at `path` that points to `target`, from the next
    /// commit on.
    ///
    /// The link goes straight into git's index and is never made on disk, so
    /// the fixture is the same on systems that cannot make symbolic links.
    pub fn symlink(&mut self, path: &str, target: &str) -> io::Result<()> {
        // A link is stored as a blob whose content is the target.
        let id = run(
            self.sandbox.git(["hash-object", "-w", "--stdin"]),
            Some(target),
        )?;
        self.index_only.push(IndexOnly {
            mode: "120000",
            id,
            path: path.to_owned(),
        });
        Ok(())
    }

    /// Adds a submodule entry at `path` pinned to `commit`, from the next
    /// commit on. The commit does not have to exist anywhere: to the
    /// repository that holds it, a submodule entry is only a pointer.
    pub fn submodule(&mut self, path: &str, commit: &str) {
        self.index_only.push(IndexOnly {
            mode: "160000",
            id: commit.to_owned(),
            path: path.to_owned(),
        });
    }

    /// Starts a branch at the current commit and switches to it.
    pub fn start_branch(&self, name: &str) -> io::Result<()> {
        run(self.sandbox.git(["checkout", "--quiet", "-b", name]), None)?;
        Ok(())
    }

    /// Switches to a branch that already exists.
    pub fn switch(&self, name: &str) -> io::Result<()> {
        run(self.sandbox.git(["checkout", "--quiet", name]), None)?;
        Ok(())
    }

    /// Merges `branch` into the current one. It always makes a merge commit,
    /// also where git could simply have moved the branch forward. Returns
    /// the new commit id.
    pub fn merge(
        &self,
        who: Identity,
        unix_time: i64,
        branch: &str,
        message: &str,
    ) -> io::Result<String> {
        let mut command = self.sandbox.git([
            "merge",
            "--quiet",
            "--no-ff",
            "--no-edit",
            "-m",
            message,
            branch,
        ]);
        sign(&mut command, who, unix_time);
        run(command, None)?;
        run(self.sandbox.git(["rev-parse", "HEAD"]), None)
    }

    /// Stages every change and commits it, with `who` as both author and
    /// committer at `unix_time` UTC. The message is stored exactly as given.
    /// Returns the new commit id.
    pub fn commit(&self, who: Identity, unix_time: i64, message: &str) -> io::Result<String> {
        run(self.sandbox.git(["add", "--all"]), None)?;
        // `add --all` drops whatever has no file in the work tree, which
        // includes these. Put them back every time.
        for entry in &self.index_only {
            let cacheinfo = format!("{},{},{}", entry.mode, entry.id, entry.path);
            run(
                self.sandbox
                    .git(["update-index", "--add", "--cacheinfo", &cacheinfo]),
                None,
            )?;
        }
        let mut command = self.sandbox.git([
            "commit",
            "--quiet",
            "--no-verify",
            "--allow-empty",
            "--cleanup=verbatim",
            "--file=-",
        ]);
        sign(&mut command, who, unix_time);
        run(command, Some(message))?;
        run(self.sandbox.git(["rev-parse", "HEAD"]), None)
    }

    /// Finishes the recipe and hands over the repository. A recipe with no
    /// commits gives a repository whose HEAD names no commit.
    pub fn finish(self) -> io::Result<Fixture> {
        let head = head_of(&self.sandbox)?;
        Ok(Fixture {
            sandbox: self.sandbox,
            head,
        })
    }
}

/// Everything under a fixture's folder: each file with its content, and
/// each folder with `None`. Two snapshots are equal only if nothing was
/// added, removed or changed in between.
pub type Snapshot = BTreeMap<PathBuf, Option<Vec<u8>>>;

/// A finished fixture repository. Its directory is deleted when this value
/// is dropped.
#[derive(Debug)]
pub struct Fixture {
    sandbox: Sandbox,
    head: Option<String>,
}

impl Fixture {
    /// The repository's folder: its work tree, or the repository itself for
    /// a bare clone.
    pub fn path(&self) -> PathBuf {
        self.sandbox.repo()
    }

    /// The commit id of HEAD, or `None` when nothing has been committed.
    pub fn head(&self) -> Option<&str> {
        self.head.as_deref()
    }

    /// Clones this fixture into a new temporary directory, passing `options`
    /// to `git clone`: `["--bare"]` for a bare copy, or
    /// `["--depth", "1", "--no-local"]` for a shallow one. (git ignores
    /// `--depth` when it clones a local path directly; `--no-local` makes it
    /// use its normal transport, which honours it.)
    pub fn git_clone(&self, options: &[&str]) -> io::Result<Fixture> {
        let sandbox = Sandbox::new()?;
        let source = self.path();
        let mut args: Vec<&OsStr> = vec![OsStr::new("clone"), OsStr::new("--quiet")];
        args.extend(options.iter().map(OsStr::new));
        // Clone into `repo/` itself, which is empty, so `path()` finds it.
        args.extend([source.as_os_str(), OsStr::new(".")]);
        run(sandbox.git(args), None)?;
        let head = head_of(&sandbox)?;
        Ok(Fixture { sandbox, head })
    }

    /// Makes a partial clone of this fixture: one that leaves every file's
    /// content on its remote, which is this fixture, to be downloaded when a
    /// git command asks for it.
    ///
    /// With `checkout`, cloning checks the files out, which downloads the
    /// ones at HEAD. Without it, none of them is in the clone.
    pub fn partial_clone(&self, checkout: bool) -> io::Result<Fixture> {
        // A repository serves partial clones only when it is told to.
        self.git(["config", "uploadpack.allowFilter", "true"])?;
        let mut options = vec!["--filter=blob:none", "--no-local"];
        if !checkout {
            options.push("--no-checkout");
        }
        self.git_clone(&options)
    }

    /// Records everything under the fixture's folder, its `.git` folder
    /// included. Comparing the snapshots from before and after a scan shows
    /// whether the scan left the repository alone.
    pub fn snapshot(&self) -> io::Result<Snapshot> {
        let root = self.path();
        let mut found = Snapshot::new();
        let mut folders = vec![root.clone()];
        while let Some(folder) = folders.pop() {
            for entry in fs::read_dir(&folder)? {
                let entry = entry?;
                let path = entry.path();
                let relative = path
                    .strip_prefix(&root)
                    .map_err(io::Error::other)?
                    .to_owned();
                if entry.file_type()?.is_dir() {
                    found.insert(relative, None);
                    folders.push(path);
                } else {
                    found.insert(relative, Some(fs::read(&path)?));
                }
            }
        }
        Ok(found)
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

    /// Runs git inside the fixture and returns its standard output byte for
    /// byte, for answers that are not text: a blob, or a `-z` listing.
    pub fn git_bytes<I, S>(&self, args: I) -> io::Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        run_bytes(self.sandbox.git(args), None)
    }
}
