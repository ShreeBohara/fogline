//! `fogline scan`. So far it finds the repository and reports every way that
//! can fail; counting files and history arrives with the rest of milestone
//! M3 (spec section 6).

use std::path::{self, Path};

use fogline_core::git::{self, Version};
use fogline_core::repo::{self, Error};

use crate::{EXIT_FAILURE, EXIT_GIT_UNUSABLE, EXIT_NOT_A_REPOSITORY, EXIT_USAGE, Failure};

pub fn run(path: &Path) -> Result<(), Failure> {
    let found = repo::discover(path).map_err(explain)?;

    let mut what = format!("found `{}` at {}", found.name, short(&found.head));
    match &found.branch {
        Some(branch) => what.push_str(&format!(" on branch {branch}")),
        None => what.push_str(" (detached HEAD)"),
    }
    if found.shallow {
        what.push_str(", a shallow clone");
    }
    Err(Failure::new(
        EXIT_FAILURE,
        format!(
            "{what}, but `scan` is not implemented yet. Counting files and history \
             arrives with the rest of milestone M3 (docs/specs/phase-1.md)."
        ),
    ))
}

/// Turns a discovery error into its exit code and a message that says what
/// to do next (spec section 5.1, rules 3 and 6).
fn explain(error: Error) -> Failure {
    let minimum = Version::MINIMUM;
    match error {
        Error::NoSuchPath(path) => Failure::new(
            EXIT_USAGE,
            format!(
                "{} does not exist. Pass the folder of a git repository, or a folder inside one.",
                shown(&path)
            ),
        ),
        Error::NotAFolder(path) => Failure::new(
            EXIT_USAGE,
            format!(
                "{} is not a folder. Pass the folder of a git repository, or a folder inside one.",
                shown(&path)
            ),
        ),
        Error::NotARepository(path) => Failure::new(
            EXIT_NOT_A_REPOSITORY,
            format!(
                "{} is not inside a git repository. Run it from a project folder, or pass a path.",
                shown(&path)
            ),
        ),
        Error::NoCommits(root) => Failure::new(
            EXIT_NOT_A_REPOSITORY,
            format!(
                "the repository at {} has no commits yet. Make a first commit, then run it again.",
                shown(&root)
            ),
        ),
        Error::GitTooOld { found } => Failure::new(
            EXIT_GIT_UNUSABLE,
            format!("git {found} is too old. Fogline needs git {minimum} or newer."),
        ),
        Error::Git(git::Error::NotFound) => Failure::new(
            EXIT_GIT_UNUSABLE,
            format!(
                "git was not found. Install git {minimum} or newer, and make sure it is on your PATH."
            ),
        ),
        Error::Git(git::Error::UnrecognizedVersion { output }) => Failure::new(
            EXIT_GIT_UNUSABLE,
            format!(
                "could not tell which git is installed: `git --version` printed {output:?}. \
                 Fogline needs git {minimum} or newer."
            ),
        ),
        Error::Git(other) => Failure::new(EXIT_FAILURE, other.to_string()),
    }
}

/// A path as people expect to read it: absolute, so `.` shows where it is.
fn shown(path: &Path) -> String {
    path::absolute(path)
        .unwrap_or_else(|_| path.to_owned())
        .display()
        .to_string()
}

/// The first seven characters of a commit id, as git shows it.
fn short(id: &str) -> &str {
    id.get(..7).unwrap_or(id)
}
