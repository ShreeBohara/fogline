//! `fogline scan`. So far it finds the repository and lists its files with
//! their sizes and line counts. Reading history and writing the bundle
//! arrive with the rest of milestone M3 (spec section 6).

use std::io::{self, Write};
use std::path::{self, Path};

use fogline_core::files::{self, Files};
use fogline_core::git::{self, Version};
use fogline_core::repo::{self, Error, Repository};

use crate::{EXIT_FAILURE, EXIT_GIT_UNUSABLE, EXIT_NOT_A_REPOSITORY, EXIT_USAGE, Failure};

pub fn run(path: &Path) -> Result<(), Failure> {
    let found = repo::discover(path).map_err(explain)?;
    let files = files::list(&found).map_err(|error| explain_listing(error, &found))?;

    // The summary is a message for a person, so it goes to stderr. stdout is
    // kept for data that was asked for (spec section 5.1, rule 2). If stderr
    // cannot be written to, there is nowhere left to say so.
    let _ = io::stderr()
        .lock()
        .write_all(summary(&found, &files).as_bytes());
    Ok(())
}

/// What a scan found, as the lines a person reads.
fn summary(found: &Repository, files: &Files) -> String {
    let branch = found.branch.as_deref().unwrap_or("detached");
    let mut text = format!(
        "fogline {} \u{b7} {} @ {} ({branch})\n",
        env!("CARGO_PKG_VERSION"),
        found.name,
        short(&found.head),
    );

    let mut detail = vec![count(files.lines(), "line")];
    if files.binary() > 0 {
        detail.push(format!("{} binary", grouped(files.binary() as u64)));
    }
    if files.submodules_skipped > 0 {
        detail.push(format!(
            "{} skipped",
            count(files.submodules_skipped, "submodule")
        ));
    }
    text.push_str(&format!(
        "  files     {} ({})\n",
        grouped(files.entries.len() as u64),
        detail.join(", ")
    ));
    text.push_str("  history   not read yet: that part of the scan is still to be built\n");

    // Each of these limits what the numbers mean, so each is said out loud.
    if found.shallow {
        text.push_str("  note      this is a shallow clone, so only part of the history is here\n");
    }
    if files.lossy_paths {
        text.push_str("  note      some paths are not valid UTF-8 and were converted\n");
    }
    if !files.attributes_from_commit {
        text.push_str(&format!(
            "  note      git {} reads .gitattributes from the work tree, not from the commit; \
             git {} or newer reads the commit\n",
            found.git,
            Version::ATTRIBUTES_FROM_COMMIT,
        ));
    }
    text
}

/// A number with a comma between each group of three digits: 1,122,175.
fn grouped(number: u64) -> String {
    let digits = number.to_string();
    let mut text = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            text.push(',');
        }
        text.push(digit);
    }
    text
}

/// A number with its noun: "1 line", "40 lines".
fn count(number: u64, noun: &str) -> String {
    let plural = if number == 1 { "" } else { "s" };
    format!("{} {noun}{plural}", grouped(number))
}

/// The same for an error from listing the files.
fn explain_listing(error: files::Error, found: &Repository) -> Failure {
    match error {
        files::Error::MissingObjects { missing } => Failure::new(
            EXIT_NOT_A_REPOSITORY,
            format!(
                "the repository at {} is a partial clone, and {} that this commit needs {} not \
                 in it. Fogline never downloads anything. Use a full clone, or fetch the missing \
                 objects with git, then run it again.",
                shown(&found.root),
                count(missing, "object"),
                if missing == 1 { "is" } else { "are" },
            ),
        ),
        files::Error::Git(error) => explain(Error::Git(error)),
    }
}

/// Turns an error into its exit code and a message that says what to do
/// next (spec section 5.1, rules 3 and 6).
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use fogline_core::classify::Flags;
    use fogline_core::files::FileEntry;
    use fogline_core::repo::ObjectFormat;

    use super::*;

    fn repository() -> Repository {
        Repository {
            root: PathBuf::from("/work/demo"),
            name: "demo".to_owned(),
            bare: false,
            shallow: false,
            object_format: ObjectFormat::Sha1,
            head: "822e3536cbae5f3404efab40519ba1de1102e6a0".to_owned(),
            branch: Some("main".to_owned()),
            git: Version::ATTRIBUTES_FROM_COMMIT,
            partial_clone: false,
        }
    }

    fn file(path: &str, lines: i64, flags: Flags) -> FileEntry {
        FileEntry {
            path: path.to_owned(),
            bytes: 10,
            lines,
            language: None,
            flags,
        }
    }

    fn files(entries: Vec<FileEntry>) -> Files {
        Files {
            entries,
            submodules_skipped: 0,
            lossy_paths: false,
            attributes_from_commit: true,
        }
    }

    #[test]
    fn a_partial_clone_with_missing_objects_is_a_reason_to_stop() {
        let failure = explain_listing(files::Error::MissingObjects { missing: 6 }, &repository());
        assert_eq!(failure.code, EXIT_NOT_A_REPOSITORY);
        assert!(
            failure
                .message
                .contains("is a partial clone, and 6 objects that this commit needs are not in it"),
            "{}",
            failure.message
        );
        assert!(failure.message.contains("never downloads anything"));

        let one = explain_listing(files::Error::MissingObjects { missing: 1 }, &repository());
        assert!(
            one.message
                .contains("1 object that this commit needs is not in it")
        );
    }

    #[test]
    fn groups_digits_in_threes() {
        let cases = [
            (0, "0"),
            (7, "7"),
            (999, "999"),
            (1_000, "1,000"),
            (7_102, "7,102"),
            (35_123, "35,123"),
            (1_122_175, "1,122,175"),
            (u64::MAX, "18,446,744,073,709,551,615"),
        ];
        for (number, expected) in cases {
            assert_eq!(grouped(number), expected);
        }
    }

    #[test]
    fn counts_agree_with_their_nouns() {
        assert_eq!(count(0, "line"), "0 lines");
        assert_eq!(count(1, "line"), "1 line");
        assert_eq!(count(1_200, "submodule"), "1,200 submodules");
    }

    #[test]
    fn a_plain_summary_has_a_header_the_files_and_the_missing_part() {
        let files = files(vec![
            file("a.rs", 1_000, Flags::default()),
            file("b.rs", 234, Flags::default()),
        ]);
        let expected = format!(
            "fogline {} \u{b7} demo @ 822e353 (main)\n\
             \x20 files     2 (1,234 lines)\n\
             \x20 history   not read yet: that part of the scan is still to be built\n",
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(summary(&repository(), &files), expected);
    }

    #[test]
    fn a_summary_says_what_was_left_out_and_what_limits_it() {
        let mut found = repository();
        found.branch = None;
        found.shallow = true;
        found.git = Version {
            major: 2,
            minor: 39,
            patch: 5,
        };
        let mut files = files(vec![
            file("a.rs", 1, Flags::default()),
            file("logo.png", -1, Flags::BINARY),
        ]);
        files.submodules_skipped = 1;
        files.lossy_paths = true;
        files.attributes_from_commit = false;

        let text = summary(&found, &files);
        assert!(text.contains("demo @ 822e353 (detached)\n"), "{text}");
        assert!(
            text.contains("  files     2 (1 line, 1 binary, 1 submodule skipped)\n"),
            "{text}"
        );
        assert!(
            text.contains("  note      this is a shallow clone"),
            "{text}"
        );
        assert!(
            text.contains("  note      some paths are not valid UTF-8"),
            "{text}"
        );
        assert!(
            text.contains("  note      git 2.39.5 reads .gitattributes from the work tree"),
            "{text}"
        );
        assert!(
            text.contains("git 2.41.0 or newer reads the commit\n"),
            "{text}"
        );
    }
}
