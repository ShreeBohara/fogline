//! The files at HEAD: their paths, sizes and line counts (spec sections 5.2
//! and 5.3).
//!
//! Two git commands answer everything, and no file is read here.
//! `ls-tree` lists every entry of the commit's tree with its size.
//! `diff --numstat` against the empty tree shows every file as added, so its
//! count of added lines is that file's line count, and a `-` marks the files
//! git treats as binary.

use std::borrow::Cow;
use std::collections::HashMap;

use crate::classify::{self, Flags};
use crate::git::{self, Version};
use crate::repo::Repository;

/// One file at HEAD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Relative to the top of the repository, with `/` between the parts.
    pub path: String,
    /// The size of the file's content.
    pub bytes: u64,
    /// The number of line endings, plus one when the file is not empty and
    /// does not end with one. −1 for a binary file, 0 for a symbolic link.
    pub lines: i64,
    /// Guessed from the file's name; `None` when the name says nothing.
    pub language: Option<&'static str>,
    pub flags: Flags,
}

/// Every file at HEAD, and what was left out or approximated on the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Files {
    /// Sorted by path, byte by byte.
    pub entries: Vec<FileEntry>,
    /// Submodule entries. They point into another repository and are not
    /// files, so they are counted and left out.
    pub submodules_skipped: u64,
    /// True when a path was not valid UTF-8 and had to be converted.
    pub lossy_paths: bool,
    /// True when `.gitattributes` was read from the commit itself. A git
    /// older than 2.41 can only read it from the work tree, so there an
    /// uncommitted edit, or a bare repository, changes which files count as
    /// binary.
    pub attributes_from_commit: bool,
}

impl Files {
    /// The lines of every file that has a line count.
    pub fn lines(&self) -> u64 {
        self.entries
            .iter()
            .filter_map(|entry| u64::try_from(entry.lines).ok())
            .sum()
    }

    /// How many files git treats as binary.
    pub fn binary(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.flags.contains(Flags::BINARY))
            .count()
    }
}

/// Why the files could not be listed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The repository is a partial clone, and some of what this commit
    /// needs was never downloaded. The engine does not download it.
    #[error("{missing} of the objects this commit needs are not in the repository")]
    MissingObjects { missing: u64 },
    #[error(transparent)]
    Git(#[from] git::Error),
}

/// Lists the files at the repository's HEAD.
pub fn list(repo: &Repository) -> Result<Files, Error> {
    // The commit id, not the name HEAD: every command then reads the same
    // commit, even if someone commits while the scan runs.
    let head = repo.head.as_str();

    // A partial clone leaves objects on its remote, and git downloads one
    // whenever a command asks for it: a network request, and a write into
    // the repository. A scan may do neither, so it looks first and stops if
    // anything is missing. With everything present, the commands below
    // have nothing to download.
    if repo.partial_clone {
        let missing = missing_objects(repo)?;
        if missing > 0 {
            return Err(Error::MissingObjects { missing });
        }
    }

    let tree_args = ["ls-tree", "-r", "-l", "-z", head];
    let tree = git::succeeded(&tree_args, git::output_in(&repo.root, &tree_args)?)?.stdout;
    let tree_entries = parse_tree(&tree).ok_or_else(|| unexpected(&tree_args, &tree))?;

    // The id of the empty tree. It differs between SHA-1 and SHA-256
    // repositories, so git is asked. This computes the id and writes nothing.
    const EMPTY_TREE: [&str; 4] = ["hash-object", "-t", "tree", "--stdin"];
    let output = git::succeeded(&EMPTY_TREE, git::output_in(&repo.root, &EMPTY_TREE)?)?;
    let empty_tree = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if empty_tree.is_empty() {
        return Err(unexpected(&EMPTY_TREE, &output.stdout).into());
    }

    let numstat_args = [
        "diff",
        "--numstat",
        "-z",
        "--no-renames",
        "--no-textconv",
        "--no-ext-diff",
        &empty_tree,
        head,
    ];
    // `.gitattributes` can mark a file as binary. Left alone, git reads it
    // from the work tree: an uncommitted edit would change the numbers, and
    // a bare repository would ignore the file. This makes git 2.41 and newer
    // read it from the commit instead. Older gits ignore the variable.
    let pin_attributes = [("GIT_ATTR_SOURCE", head)];
    let numstat = git::succeeded(
        &numstat_args,
        git::output_in_with(&repo.root, &pin_attributes, &numstat_args)?,
    )?
    .stdout;
    let line_counts = parse_numstat(&numstat).ok_or_else(|| unexpected(&numstat_args, &numstat))?;

    let mut files = Files {
        entries: Vec::with_capacity(tree_entries.len()),
        submodules_skipped: 0,
        lossy_paths: false,
        attributes_from_commit: repo.git >= Version::ATTRIBUTES_FROM_COMMIT,
    };
    for entry in tree_entries {
        if entry.kind == Kind::Submodule {
            files.submodules_skipped += 1;
            continue;
        }
        let path = match String::from_utf8_lossy(entry.path) {
            Cow::Borrowed(path) => path.to_owned(),
            Cow::Owned(path) => {
                files.lossy_paths = true;
                path
            }
        };

        let mut flags = Flags::default();
        if classify::is_vendored(&path) {
            flags.insert(Flags::VENDORED);
        }
        let (lines, language) = if entry.kind == Kind::Link {
            // A link's content is the path it points to, which git would
            // count as one line. It is not a line of anything.
            flags.insert(Flags::SYMLINK);
            (0, None)
        } else {
            let lines = *line_counts.get(entry.path).ok_or_else(|| {
                unexpected(
                    &numstat_args,
                    format!("no line count for {path}").as_bytes(),
                )
            })?;
            if lines < 0 {
                flags.insert(Flags::BINARY);
            }
            if classify::is_generated(&path, entry.bytes, lines) {
                flags.insert(Flags::GENERATED);
            }
            (lines, classify::language(&path))
        };

        files.entries.push(FileEntry {
            path,
            bytes: entry.bytes,
            lines,
            language,
            flags,
        });
    }
    // git already lists paths in byte order. Sorting again costs little and
    // keeps the order right after a path had to be converted.
    files.entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// Counts the objects that the commit's files and folders need and the
/// repository does not have.
///
/// `rev-list --missing=print` lists them instead of downloading them. It
/// turns downloading off itself, and already did at git 2.25.0, so this is
/// safe on every git the engine supports.
fn missing_objects(repo: &Repository) -> Result<u64, git::Error> {
    let args = [
        "rev-list",
        "--objects",
        "--missing=print",
        "--no-walk",
        repo.head.as_str(),
    ];
    let output = git::succeeded(&args, git::output_in(&repo.root, &args)?)?;
    // A missing object is printed as `?` followed by its id.
    let missing = output
        .stdout
        .split(|&byte| byte == b'\n')
        .filter(|line| line.starts_with(b"?"))
        .count();
    Ok(missing as u64)
}

/// What an entry of a commit's tree is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    File,
    /// A symbolic link: a blob whose content is the path it points to.
    Link,
    /// A pointer to a commit in another repository.
    Submodule,
}

#[derive(Debug, PartialEq, Eq)]
struct TreeEntry<'a> {
    kind: Kind,
    /// 0 for a submodule, which has no size.
    bytes: u64,
    path: &'a [u8],
}

/// Reads the output of `git ls-tree -r -l -z`.
///
/// Each record is `<mode> <type> <id> <size>` TAB `<path>` NUL. The size is
/// padded with spaces, and is `-` for a submodule. Returns `None` for
/// anything that does not have this shape.
fn parse_tree(output: &[u8]) -> Option<Vec<TreeEntry<'_>>> {
    let mut entries = Vec::new();
    for record in records(output)? {
        // The first tab ends the fixed fields. A path may hold tabs itself.
        let tab = record.iter().position(|&byte| byte == b'\t')?;
        let (meta, path) = (&record[..tab], &record[tab + 1..]);
        if path.is_empty() {
            return None;
        }
        let mut fields = meta
            .split(|&byte| byte == b' ')
            .filter(|field| !field.is_empty());
        let (mode, kind, _id, size) = (
            fields.next()?,
            fields.next()?,
            fields.next()?,
            fields.next()?,
        );
        if fields.next().is_some() {
            return None;
        }
        let (kind, bytes) = match (kind, mode) {
            (b"commit", _) => (Kind::Submodule, 0),
            (b"blob", b"120000") => (Kind::Link, number(size)?),
            (b"blob", _) => (Kind::File, number(size)?),
            _ => return None,
        };
        entries.push(TreeEntry { kind, bytes, path });
    }
    Some(entries)
}

/// Reads the output of `git diff --numstat -z --no-renames` and returns the
/// line count for each path, −1 for a binary file.
///
/// Each record is `<added>` TAB `<deleted>` TAB `<path>` NUL, with `-` in
/// place of both numbers for a binary file. Returns `None` for anything
/// that does not have this shape.
fn parse_numstat(output: &[u8]) -> Option<HashMap<&[u8], i64>> {
    let mut line_counts = HashMap::new();
    for record in records(output)? {
        // Split at the first two tabs only. A path may hold tabs itself.
        let mut fields = record.splitn(3, |&byte| byte == b'\t');
        let (added, deleted, path) = (fields.next()?, fields.next()?, fields.next()?);
        if path.is_empty() {
            return None;
        }
        let lines = match (added, deleted) {
            (b"-", b"-") => -1,
            _ => {
                number(deleted)?;
                i64::try_from(number(added)?).ok()?
            }
        };
        line_counts.insert(path, lines);
    }
    Some(line_counts)
}

/// Splits `-z` output into its records. Every record ends with a NUL, so
/// output that ends any other way was cut short, and gives `None`.
fn records(output: &[u8]) -> Option<Vec<&[u8]>> {
    if output.is_empty() {
        return Some(Vec::new());
    }
    let body = output.strip_suffix(&[0])?;
    Some(body.split(|&byte| byte == 0).collect())
}

/// Reads a number made of ASCII digits and nothing else.
fn number(digits: &[u8]) -> Option<u64> {
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(digits).ok()?.parse().ok()
}

/// The error for output that could not be read. Only the start of the
/// output goes into the message: the whole of it can be megabytes.
fn unexpected(args: &[&str], output: &[u8]) -> git::Error {
    let start = &output[..output.len().min(200)];
    git::Error::Unexpected {
        command: args.join(" "),
        output: String::from_utf8_lossy(start).into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `git ls-tree -r -l -z` printed for a small repository, with the
    /// object ids shortened.
    const TREE: &[u8] = b"100644 blob 8697fef      13\t.gitattributes\0\
        100644 blob 7989678       8\tdata.bin\0\
        100644 blob 45b983b       3\tdir with space/my file.txt\0\
        100644 blob e69de29       0\tempty.txt\0\
        120000 blob 1bb1a2c       9\tlink\0\
        100755 blob de98044       6\trun.sh\0\
        160000 commit 1111111       -\tvendor/sub\0";

    /// What `git diff --numstat -z --no-renames` printed for the same one.
    const NUMSTAT: &[u8] = b"1\t0\t.gitattributes\0\
        -\t-\tdata.bin\0\
        1\t0\tdir with space/my file.txt\0\
        0\t0\tempty.txt\0\
        1\t0\tlink\0\
        3\t0\trun.sh\0\
        1\t0\tvendor/sub\0";

    #[test]
    fn reads_a_tree_listing() {
        let entries = parse_tree(TREE).unwrap();
        let seen: Vec<(Kind, u64, &str)> = entries
            .iter()
            .map(|entry| {
                (
                    entry.kind,
                    entry.bytes,
                    std::str::from_utf8(entry.path).unwrap(),
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                (Kind::File, 13, ".gitattributes"),
                (Kind::File, 8, "data.bin"),
                (Kind::File, 3, "dir with space/my file.txt"),
                (Kind::File, 0, "empty.txt"),
                (Kind::Link, 9, "link"),
                (Kind::File, 6, "run.sh"),
                (Kind::Submodule, 0, "vendor/sub"),
            ]
        );
    }

    #[test]
    fn reads_line_counts() {
        let line_counts = parse_numstat(NUMSTAT).unwrap();
        assert_eq!(line_counts.len(), 7);
        assert_eq!(line_counts[&b"data.bin"[..]], -1);
        assert_eq!(line_counts[&b"empty.txt"[..]], 0);
        assert_eq!(line_counts[&b"run.sh"[..]], 3);
        assert_eq!(line_counts[&b"dir with space/my file.txt"[..]], 1);
    }

    #[test]
    fn a_path_may_hold_tabs_and_line_breaks() {
        let entries = parse_tree(b"100644 blob abc 5\todd\tname\n.txt\0").unwrap();
        assert_eq!(entries[0].path, b"odd\tname\n.txt");

        let line_counts = parse_numstat(b"2\t0\todd\tname\n.txt\0").unwrap();
        assert_eq!(line_counts[&b"odd\tname\n.txt"[..]], 2);
    }

    #[test]
    fn empty_output_means_no_files() {
        assert_eq!(parse_tree(b"").unwrap(), []);
        assert!(parse_numstat(b"").unwrap().is_empty());
    }

    /// Output that was cut short must be refused, never half-read, and
    /// never a crash. A prefix is acceptable only where it ends a record.
    #[test]
    fn output_cut_short_is_refused() {
        for end in 0..=TREE.len() {
            let prefix = &TREE[..end];
            let whole_records = prefix.is_empty() || prefix.ends_with(&[0]);
            assert_eq!(
                parse_tree(prefix).is_some(),
                whole_records,
                "tree cut at {end}"
            );
        }
        for end in 0..=NUMSTAT.len() {
            let prefix = &NUMSTAT[..end];
            let whole_records = prefix.is_empty() || prefix.ends_with(&[0]);
            assert_eq!(
                parse_numstat(prefix).is_some(),
                whole_records,
                "numstat cut at {end}"
            );
        }
    }

    #[test]
    fn output_of_another_shape_is_refused() {
        for output in [
            &b"100644 blob abc 5 no-tab.txt\0"[..],
            b"100644 blob abc\tthree-fields.txt\0",
            b"100644 blob abc five\tbad-size.txt\0",
            b"100644 blob abc +5\tsigned-size.txt\0",
            b"040000 tree abc -\ta-folder\0",
            b"100644 blob abc 5\t\0",
            b"\0",
        ] {
            assert_eq!(
                parse_tree(output),
                None,
                "{:?}",
                String::from_utf8_lossy(output)
            );
        }
        for output in [
            &b"1\t0\0"[..],
            b"one\t0\tfile.txt\0",
            b"-\t0\tfile.txt\0",
            b"1\t-\tfile.txt\0",
            b"1\t0\t\0",
            b"\0",
        ] {
            assert_eq!(
                parse_numstat(output),
                None,
                "{:?}",
                String::from_utf8_lossy(output)
            );
        }
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
}
