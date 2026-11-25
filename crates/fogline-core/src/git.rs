//! Calling the `git` executable, and reading what it prints.
//!
//! In Phase 1 every repository fact comes from git's own plumbing commands
//! (spec section 5.3). Each call starts from [`command`], which pins the
//! environment and configuration so that a user's settings cannot change what
//! the engine reads, and so that git never takes a lock or stops to ask a
//! question.

use std::fmt;
use std::io;
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// A git release number, such as 2.52.0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// The oldest git the engine supports. `git rev-parse
    /// --show-object-format` first appeared in 2.25.0: git's own
    /// `builtin/rev-parse.c` has it at the v2.25.0 tag and not at v2.24.0.
    pub const MINIMUM: Version = Version {
        major: 2,
        minor: 25,
        patch: 0,
    };

    /// The first git that can read `.gitattributes` from a commit instead of
    /// from the work tree, through `GIT_ATTR_SOURCE`. git's own `git.c`
    /// knows `--attr-source` at the v2.41.0 tag and not at v2.40.0.
    pub const ATTRIBUTES_FROM_COMMIT: Version = Version {
        major: 2,
        minor: 41,
        patch: 0,
    };

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
    /// git succeeded but printed something the engine cannot read.
    #[error("`git {command}` printed {output:?}, which fogline cannot read")]
    Unexpected { command: String, output: String },
}

/// Variables that would make git read a repository other than the one it is
/// run in. Git sets some of them itself while it runs hooks, so a fogline
/// started from a hook would otherwise scan the wrong repository.
///
/// `GIT_ATTR_SOURCE` is here for a related reason: it chooses where git
/// reads `.gitattributes` from, and the engine sets it itself where that
/// matters.
const REDIRECTING_VARIABLES: [&str; 8] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_NAMESPACE",
    "GIT_ATTR_SOURCE",
];

/// Starts a git invocation with the pinned environment and configuration.
///
/// - `GIT_OPTIONAL_LOCKS=0`: never write to the repository just to read it.
/// - `GIT_TERMINAL_PROMPT=0`: fail instead of asking for input.
/// - `LC_ALL=C`: messages and sorting do not depend on the user's locale.
/// - `core.quotepath=false`, `i18n.logOutputEncoding=UTF-8`: paths and
///   messages arrive as raw UTF-8 bytes.
/// - `GIT_NO_LAZY_FETCH=1`: in a partial clone, fail instead of downloading
///   an object that is missing. A download is a network request and a write
///   into the repository, and the engine may do neither. Not every git the
///   engine supports knows the variable: 2.45.0 and newer do (its
///   `environment.h` names it at the v2.45.0 tag and not at v2.44.0), and so
///   do six maintenance releases of older lines, from 2.39.4 to 2.44.1. So
///   the engine also looks before it reads (`Repository::missing_objects`).
/// - `GIT_ATTR_NOSYSTEM=1`, `core.attributesFile=/dev/null`: ignore the
///   attributes files of the machine and of the user. An attributes file
///   can declare a file binary, which removes its line counts, so a line
///   such as `*.py binary` in `~/.config/git/attributes` would change the
///   numbers. Git reads `/dev/null` as an empty file on every platform,
///   Windows included.
/// - The variables in [`REDIRECTING_VARIABLES`] are removed, so the folder
///   git runs in is the only thing that decides which repository it reads.
///
/// Standard input is closed; callers that feed git on stdin replace it.
pub fn command() -> Command {
    let mut command = Command::new("git");
    for variable in REDIRECTING_VARIABLES {
        command.env_remove(variable);
    }
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("LC_ALL", "C")
        .args(["-c", "core.quotepath=false"])
        .args(["-c", "i18n.logOutputEncoding=UTF-8"])
        .args(["-c", "core.attributesFile=/dev/null"])
        .stdin(Stdio::null());
    command
}

/// Runs git with `args` in the folder `dir` and collects what it printed.
///
/// A git that exits with an error is still `Ok` here, because what a failure
/// means depends on the question: callers look at the exit status.
pub fn output_in(dir: &Path, args: &[&str]) -> Result<Output, Error> {
    output_in_with(dir, &[], args)
}

/// Like [`output_in`], with extra environment variables for this one call.
pub fn output_in_with(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> Result<Output, Error> {
    command()
        .current_dir(dir)
        .envs(env.iter().copied())
        .args(args)
        .output()
        .map_err(|err| could_not_start(err, dir))
}

/// The error for a git that could not be started in the folder `dir`.
///
/// Starting a process in a folder that does not exist also fails with "not
/// found", so git is only called missing when the folder is there.
pub(crate) fn could_not_start(err: io::Error, dir: &Path) -> Error {
    if err.kind() == io::ErrorKind::NotFound && dir.is_dir() {
        Error::NotFound
    } else {
        Error::Io(err)
    }
}

/// Passes on the output of a git that succeeded, and turns one that failed
/// into an error carrying what git said.
pub fn succeeded(args: &[&str], output: Output) -> Result<Output, Error> {
    if output.status.success() {
        return Ok(output);
    }
    let stderr = String::from_utf8_lossy(&output.stderr)
        .trim_end()
        .to_owned();
    Err(Error::Failed {
        stderr: if stderr.is_empty() {
            format!("`git {}` failed with {}", args.join(" "), output.status)
        } else {
            stderr
        },
    })
}

/// Splits `-z` output into its records. Every record ends with a NUL, so
/// output that ends any other way was cut short, and gives `None`.
pub(crate) fn records(output: &[u8]) -> Option<Vec<&[u8]>> {
    if output.is_empty() {
        return Some(Vec::new());
    }
    let body = output.strip_suffix(&[0])?;
    Some(body.split(|&byte| byte == 0).collect())
}

/// Reads a number made of ASCII digits and nothing else.
pub(crate) fn number(digits: &[u8]) -> Option<u64> {
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(digits).ok()?.parse().ok()
}

/// The error for output that could not be read. Only the start of the
/// output goes into the message: the whole of it can be megabytes.
pub(crate) fn unexpected(args: &[&str], output: &[u8]) -> Error {
    let start = &output[..output.len().min(200)];
    Error::Unexpected {
        command: args.join(" "),
        output: String::from_utf8_lossy(start).into_owned(),
    }
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
    fn the_minimum_is_2_25_0() {
        assert_eq!(Version::MINIMUM, version(2, 25, 0));
        assert!(version(2, 24, 9) < Version::MINIMUM);
        assert!(version(2, 25, 0) >= Version::MINIMUM);
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
            ("GIT_NO_LAZY_FETCH", "1"),
            ("GIT_ATTR_NOSYSTEM", "1"),
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
                "i18n.logOutputEncoding=UTF-8",
                "-c",
                "core.attributesFile=/dev/null"
            ]
        );
    }

    #[test]
    fn removes_the_variables_that_point_git_elsewhere() {
        let command = command();
        let removed: Vec<String> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(key, _)| key.to_string_lossy().into_owned())
            .collect();
        for variable in ["GIT_DIR", "GIT_WORK_TREE", "GIT_COMMON_DIR"] {
            assert!(
                removed.iter().any(|key| key == variable),
                "{variable} should be removed"
            );
        }
        assert_eq!(removed.len(), REDIRECTING_VARIABLES.len());
    }

    #[test]
    fn output_in_reports_failure_through_the_exit_status() {
        let here = std::env::current_dir().unwrap();
        let output =
            output_in(&here, &["rev-parse", "--verify", "--quiet", "no-such-ref"]).unwrap();
        assert!(!output.status.success());
    }

    #[test]
    fn output_in_does_not_blame_git_for_a_missing_folder() {
        let missing = Path::new("fogline-test-no-such-folder");
        let error = output_in(missing, &["--version"]).unwrap_err();
        assert!(matches!(error, Error::Io(_)), "got {error:?}");
    }

    #[test]
    fn splits_output_into_records_that_each_end_with_a_nul() {
        assert_eq!(records(b""), Some(vec![]));
        assert_eq!(records(b"a\0"), Some(vec![&b"a"[..]]));
        assert_eq!(records(b"a\0\0b\0"), Some(vec![&b"a"[..], b"", b"b"]));
        assert_eq!(records(b"a\0b"), None, "the last record has no end");
        assert_eq!(records(b"a"), None);
    }

    #[test]
    fn only_digits_are_a_number() {
        assert_eq!(number(b"0"), Some(0));
        assert_eq!(number(b"1122175"), Some(1_122_175));
        for text in [
            &b""[..],
            b"-",
            b"+1",
            b"-1",
            b" 1",
            b"1.0",
            b"99999999999999999999999",
        ] {
            assert_eq!(number(text), None, "{:?}", String::from_utf8_lossy(text));
        }
    }

    #[test]
    fn an_error_about_unreadable_output_quotes_only_its_start() {
        let long = vec![b'x'; 5_000];
        let Error::Unexpected { command, output } = unexpected(&["log", "-z"], &long) else {
            panic!("expected the Unexpected variant");
        };
        assert_eq!(command, "log -z");
        assert_eq!(output.len(), 200);
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
