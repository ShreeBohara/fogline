//! Finding the repository a scan reads, and the facts about its HEAD
//! (spec sections 5.1 and 5.3).
//!
//! [`discover`] takes any folder inside a repository and answers, in order:
//! is there a usable git, is this a repository, where is its top, and which
//! commit does HEAD point to. Each question that fails has its own [`Error`]
//! variant, because the command line gives each a different exit code.

use std::path::{Path, PathBuf};
use std::process::Output;

use crate::git::{self, Version};

/// How the repository names its objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectFormat {
    Sha1,
    Sha256,
}

impl ObjectFormat {
    /// The name git uses, which the bundle records.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
        }
    }

    /// Length of an object id in hexadecimal digits.
    fn hex_len(self) -> usize {
        match self {
            Self::Sha1 => 40,
            Self::Sha256 => 64,
        }
    }
}

/// What discovery learned about the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    /// Where every later git command runs: the top of the work tree, or the
    /// git folder itself for a bare repository or a path inside `.git`.
    /// Running there means no command depends on where the user started.
    pub root: PathBuf,
    /// A name for people to read (see [`display_name`]).
    pub name: String,
    pub bare: bool,
    pub shallow: bool,
    pub object_format: ObjectFormat,
    /// The full id of the commit HEAD points to.
    pub head: String,
    /// The branch HEAD is on, or `None` when HEAD is detached.
    pub branch: Option<String>,
    /// The git that answered. Later steps check it for what it can do.
    pub git: Version,
    /// True for a partial clone: one made with `git clone --filter`, which
    /// leaves some objects on its remote. Git downloads such an object when
    /// a command asks for it, so everything that reads objects checks this
    /// first.
    pub partial_clone: bool,
}

/// Why there is no repository to scan.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{} does not exist", .0.display())]
    NoSuchPath(PathBuf),
    #[error("{} is not a folder", .0.display())]
    NotAFolder(PathBuf),
    #[error("git {found} is older than {}", Version::MINIMUM)]
    GitTooOld { found: Version },
    #[error("{} is not inside a git repository", .0.display())]
    NotARepository(PathBuf),
    /// HEAD names no commit: nothing has been committed on its branch yet.
    #[error("the repository at {} has no commits yet", .0.display())]
    NoCommits(PathBuf),
    #[error(transparent)]
    Git(#[from] git::Error),
}

/// Finds the repository that contains `path`, and the commit HEAD points to.
pub fn discover(path: &Path) -> Result<Repository, Error> {
    // Checked here because git would only say "cannot change to".
    if !path.exists() {
        return Err(Error::NoSuchPath(path.to_owned()));
    }
    if !path.is_dir() {
        return Err(Error::NotAFolder(path.to_owned()));
    }

    let version = git::detect_version()?;
    if version < Version::MINIMUM {
        return Err(Error::GitTooOld { found: version });
    }

    const FACTS: [&str; 5] = [
        "rev-parse",
        "--is-bare-repository",
        "--is-inside-git-dir",
        "--is-shallow-repository",
        "--show-object-format",
    ];
    let output = git::output_in(path, &FACTS)?;
    if !output.status.success() {
        let stderr = lossy(&output.stderr);
        // LC_ALL=C makes git word this the same way everywhere.
        if stderr.contains("not a git repository") {
            return Err(Error::NotARepository(path.to_owned()));
        }
        return Err(git::Error::Failed { stderr }.into());
    }
    let facts = Facts::parse(&output.stdout).ok_or_else(|| unexpected(&FACTS, &output))?;

    // `--show-toplevel` fails where there is no work tree, so a bare
    // repository, or a path inside `.git`, runs in the git folder instead.
    let in_git_folder = facts.bare || facts.inside_git_dir;
    let top = if in_git_folder {
        ["rev-parse", "--absolute-git-dir"]
    } else {
        ["rev-parse", "--show-toplevel"]
    };
    let root = path_from_git(git::succeeded(&top, git::output_in(path, &top)?)?.stdout)?;

    const PROMISORS: [&str; 4] = [
        "config",
        "-z",
        "--get-regexp",
        r"^(extensions\.partialclone|remote\..*\.promisor)$",
    ];
    let output = git::output_in(&root, &PROMISORS)?;
    // `config --get-regexp` exits with 1 when no setting matches.
    let partial_clone = if output.status.code() == Some(1) {
        false
    } else {
        promises_objects(&git::succeeded(&PROMISORS, output)?.stdout)
    };

    const HEAD: [&str; 4] = ["rev-parse", "--verify", "--quiet", "HEAD^{commit}"];
    let output = git::output_in(&root, &HEAD)?;
    // `--verify --quiet` exits with 1, silently, when HEAD names no commit.
    if output.status.code() == Some(1) {
        return Err(Error::NoCommits(root));
    }
    let output = git::succeeded(&HEAD, output)?;
    let head = lossy(&output.stdout);
    if !is_object_id(&head, facts.object_format) {
        return Err(unexpected(&HEAD, &output).into());
    }

    const BRANCH: [&str; 4] = ["symbolic-ref", "--quiet", "--short", "HEAD"];
    let output = git::output_in(&root, &BRANCH)?;
    // `symbolic-ref --quiet` exits with 1 when HEAD is detached.
    let branch = if output.status.code() == Some(1) {
        None
    } else {
        Some(lossy(&git::succeeded(&BRANCH, output)?.stdout))
    };

    Ok(Repository {
        name: display_name(&root, in_git_folder),
        root,
        bare: facts.bare,
        shallow: facts.shallow,
        object_format: facts.object_format,
        head,
        branch,
        git: version,
        partial_clone,
    })
}

/// Reads what `git config -z --get-regexp` printed for the settings that
/// make a repository a partial clone, and says whether any of them is on.
///
/// Each record is `<key>` NEWLINE `<value>` NUL, or `<key>` NUL alone for a
/// setting written without a value, which git reads as true.
/// `extensions.partialclone` names the remote that holds the missing
/// objects. `remote.<name>.promisor` is a yes-or-no setting. When in doubt
/// the answer is yes: the price of a wrong yes is one extra check.
fn promises_objects(output: &[u8]) -> bool {
    output
        .split(|&byte| byte == 0)
        .filter(|record| !record.is_empty())
        .any(|record| {
            let mut parts = record.splitn(2, |&byte| byte == b'\n');
            let key = parts.next().unwrap_or_default();
            let Some(value) = parts.next() else {
                return true;
            };
            if key.eq_ignore_ascii_case(b"extensions.partialclone") {
                return !value.is_empty();
            }
            let off: [&[u8]; 5] = [b"false", b"no", b"off", b"0", b""];
            !off.iter().any(|word| value.eq_ignore_ascii_case(word))
        })
}

/// The four answers of the first `rev-parse` call, one per line.
#[derive(Debug, PartialEq, Eq)]
struct Facts {
    bare: bool,
    inside_git_dir: bool,
    shallow: bool,
    object_format: ObjectFormat,
}

impl Facts {
    fn parse(stdout: &[u8]) -> Option<Self> {
        let text = std::str::from_utf8(stdout).ok()?;
        let mut lines = text.lines();
        let mut flag = || match lines.next()? {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        };
        let (bare, inside_git_dir, shallow) = (flag()?, flag()?, flag()?);
        let object_format = match lines.next()? {
            "sha1" => ObjectFormat::Sha1,
            "sha256" => ObjectFormat::Sha256,
            _ => return None,
        };
        if lines.next().is_some() {
            return None;
        }
        Some(Self {
            bare,
            inside_git_dir,
            shallow,
            object_format,
        })
    }
}

/// A name for people to read. A work tree is named after its folder. A bare
/// repository's folder is usually `name.git`, and the `.git` folder of a
/// work tree is named after the folder that holds it.
fn display_name(root: &Path, in_git_folder: bool) -> String {
    let base = |path: &Path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    };
    let name = match base(root) {
        Some(name) if in_git_folder && name == ".git" => root.parent().and_then(base),
        Some(name) if in_git_folder => Some(name.strip_suffix(".git").unwrap_or(&name).to_owned()),
        other => other,
    };
    name.filter(|name| !name.is_empty())
        .unwrap_or_else(|| "repository".to_owned())
}

/// True for an id of the right length made of lowercase hexadecimal digits.
fn is_object_id(text: &str, format: ObjectFormat) -> bool {
    text.len() == format.hex_len()
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn unexpected(args: &[&str], output: &Output) -> git::Error {
    git::Error::Unexpected {
        command: args.join(" "),
        output: lossy(&output.stdout),
    }
}

/// Text that git printed, without the line ending at the end.
fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim_end().to_owned()
}

/// Turns a path that git printed on its own line into a `PathBuf`.
///
/// On Unix a path is any bytes, so they are kept exactly; elsewhere git
/// prints UTF-8. Only the final line ending is removed, because a path may
/// itself end in spaces.
fn path_from_git(mut bytes: Vec<u8>) -> Result<PathBuf, git::Error> {
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    if bytes.is_empty() {
        return Err(git::Error::Unexpected {
            command: "rev-parse".to_owned(),
            output: String::new(),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(bytes)
            .map(PathBuf::from)
            .map_err(|err| git::Error::Unexpected {
                command: "rev-parse".to_owned(),
                output: String::from_utf8_lossy(err.as_bytes()).into_owned(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_four_facts() {
        let facts = Facts::parse(b"false\ntrue\nfalse\nsha256\n").unwrap();
        assert_eq!(
            facts,
            Facts {
                bare: false,
                inside_git_dir: true,
                shallow: false,
                object_format: ObjectFormat::Sha256,
            }
        );
        // Windows line endings read the same.
        assert!(Facts::parse(b"true\r\ntrue\r\ntrue\r\nsha1\r\n").is_some());
    }

    #[test]
    fn refuses_facts_it_does_not_understand() {
        for output in [
            &b""[..],
            b"false\nfalse\nfalse\n",
            b"false\nfalse\nfalse\nmd5\n",
            b"no\nfalse\nfalse\nsha1\n",
            b"false\nfalse\nfalse\nsha1\nextra\n",
        ] {
            assert_eq!(
                Facts::parse(output),
                None,
                "{:?}",
                String::from_utf8_lossy(output)
            );
        }
    }

    #[test]
    fn recognises_the_settings_of_a_partial_clone() {
        let on: [&[u8]; 6] = [
            b"remote.origin.promisor\ntrue\0",
            b"remote.origin.promisor\nYes\0",
            b"remote.origin.promisor\0",
            b"remote.origin.promisor\n1\0",
            b"extensions.partialclone\norigin\0",
            b"remote.a.promisor\nfalse\0remote.b.promisor\ntrue\0",
        ];
        for output in on {
            assert!(
                promises_objects(output),
                "{:?}",
                String::from_utf8_lossy(output)
            );
        }
        let off: [&[u8]; 5] = [
            b"",
            b"remote.origin.promisor\nfalse\0",
            b"remote.origin.promisor\nOFF\0",
            b"remote.origin.promisor\n0\0",
            b"extensions.partialclone\n\0",
        ];
        for output in off {
            assert!(
                !promises_objects(output),
                "{:?}",
                String::from_utf8_lossy(output)
            );
        }
    }

    #[test]
    fn names_the_repository_for_people() {
        let cases = [
            ("/work/fogline", false, "fogline"),
            ("/srv/git/fogline.git", true, "fogline"),
            ("/srv/git/fogline", true, "fogline"),
            ("/work/fogline/.git", true, "fogline"),
            ("/", false, "repository"),
        ];
        for (root, in_git_folder, expected) in cases {
            assert_eq!(
                display_name(Path::new(root), in_git_folder),
                expected,
                "{root}"
            );
        }
    }

    #[test]
    fn checks_the_shape_of_an_object_id() {
        let sha1 = "822e3536cbae5f3404efab40519ba1de1102e6a0";
        assert!(is_object_id(sha1, ObjectFormat::Sha1));
        assert!(!is_object_id(sha1, ObjectFormat::Sha256));
        assert!(!is_object_id(&sha1.to_uppercase(), ObjectFormat::Sha1));
        assert!(!is_object_id("HEAD", ObjectFormat::Sha1));
    }

    #[test]
    fn keeps_a_path_exactly_apart_from_the_line_ending() {
        let path = path_from_git(b"/tmp/my repo \n".to_vec()).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/my repo "));
        let path = path_from_git(b"C:/work/repo\r\n".to_vec()).unwrap();
        assert_eq!(path, PathBuf::from("C:/work/repo"));
        assert!(path_from_git(b"\n".to_vec()).is_err());
    }
}
