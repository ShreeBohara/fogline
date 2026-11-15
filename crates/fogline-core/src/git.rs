//! Calling the `git` executable.
//!
//! In Phase 1 every repository fact comes from git's own plumbing commands
//! (spec section 5.3). Each call starts from [`command`], which pins the
//! environment and configuration so that a user's settings cannot change what
//! the engine reads, and so that git never takes a lock or stops to ask a
//! question.

use std::fmt;
use std::io;
use std::process::{Command, Stdio};

/// A git release number, such as 2.52.0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// Reads the version out of what `git --version` prints.
    ///
    /// Vendors add their own suffixes, so only the leading numbers count:
    /// `git version 2.39.5 (Apple Git-154)` is 2.39.5 and
    /// `git version 2.47.1.windows.2` is 2.47.1. A missing or non-numeric
    /// patch number reads as 0.
    pub fn parse(output: &str) -> Option<Self> {
        let rest = output.trim().strip_prefix("git version ")?;
        let mut parts = rest.split_whitespace().next()?.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
        Some(Self {
            major,
            minor,
            patch,
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Why git could not be used.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// There is no `git` executable on `PATH`.
    #[error("git was not found on PATH")]
    NotFound,
    /// The process could not be started or read.
    #[error("git could not be run: {0}")]
    Io(#[source] io::Error),
    /// git ran and reported failure.
    #[error("git exited with an error: {stderr}")]
    Failed { stderr: String },
    /// `git --version` printed something that is not a version.
    #[error("`git --version` printed {output:?}, which is not a version")]
    UnrecognizedVersion { output: String },
}

/// Starts a git invocation with the pinned environment and configuration.
///
/// - `GIT_OPTIONAL_LOCKS=0`: never write to the repository just to read it.
/// - `GIT_TERMINAL_PROMPT=0`: fail instead of asking for input.
/// - `LC_ALL=C`: messages and sorting do not depend on the user's locale.
/// - `core.quotepath=false`, `i18n.logOutputEncoding=UTF-8`: paths and
///   messages arrive as raw UTF-8 bytes.
///
/// Standard input is closed; callers that feed git on stdin replace it.
pub fn command() -> Command {
    let mut command = Command::new("git");
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .args(["-c", "core.quotepath=false"])
        .args(["-c", "i18n.logOutputEncoding=UTF-8"])
        .stdin(Stdio::null());
    command
}

/// Finds out which git is installed.
pub fn detect_version() -> Result<Version, Error> {
    read_version(command())
}

fn read_version(mut command: Command) -> Result<Version, Error> {
    let output = command
        .arg("--version")
        .output()
        .map_err(|err| match err.kind() {
            io::ErrorKind::NotFound => Error::NotFound,
            _ => Error::Io(err),
        })?;
    if !output.status.success() {
        return Err(Error::Failed {
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Version::parse(&text).ok_or_else(|| Error::UnrecognizedVersion {
        output: text.trim().to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(major: u32, minor: u32, patch: u32) -> Version {
        Version {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn parses_what_real_gits_print() {
        let cases = [
            ("git version 2.52.0\n", version(2, 52, 0)),
            ("git version 2.39.5 (Apple Git-154)\n", version(2, 39, 5)),
            ("git version 2.47.1.windows.2\r\n", version(2, 47, 1)),
            ("git version 2.25\n", version(2, 25, 0)),
            ("git version 2.53.0-rc1\n", version(2, 53, 0)),
        ];
        for (output, expected) in cases {
            assert_eq!(Version::parse(output), Some(expected), "{output:?}");
        }
    }

    #[test]
    fn rejects_text_that_is_not_a_git_version() {
        for output in [
            "",
            "git",
            "git version",
            "git version two",
            "git version 2",
            "hg 6.1",
        ] {
            assert_eq!(Version::parse(output), None, "{output:?}");
        }
    }

    #[test]
    fn compares_versions_as_numbers_not_text() {
        assert!(version(2, 9, 0) < version(2, 25, 0));
        assert!(version(2, 25, 1) > version(2, 25, 0));
        assert!(version(3, 0, 0) > version(2, 99, 99));
    }

    #[test]
    fn prints_three_numbers() {
        assert_eq!(version(2, 52, 0).to_string(), "2.52.0");
    }

    #[test]
    fn pins_the_environment_and_configuration() {
        let command = command();
        let pinned: Vec<(String, String)> = command
            .get_envs()
            .filter_map(|(key, value)| {
                Some((
                    key.to_string_lossy().into_owned(),
                    value?.to_string_lossy().into_owned(),
                ))
            })
            .collect();
        for (key, value) in [
            ("GIT_OPTIONAL_LOCKS", "0"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("LC_ALL", "C"),
        ] {
            assert!(
                pinned.contains(&(key.to_owned(), value.to_owned())),
                "{key}={value} should be set"
            );
        }
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "-c",
                "core.quotepath=false",
                "-c",
                "i18n.logOutputEncoding=UTF-8"
            ]
        );
    }

    #[test]
    fn finds_the_git_on_this_machine() {
        let found = detect_version().expect("git should be installed where tests run");
        assert!(found >= version(2, 0, 0), "unexpected git {found}");
    }

    #[test]
    fn reports_a_missing_git_as_not_found() {
        let missing = Command::new("fogline-test-no-such-git");
        assert!(matches!(read_version(missing), Err(Error::NotFound)));
    }
}
