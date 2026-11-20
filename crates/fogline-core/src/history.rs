//! The commits HEAD can reach, and what each one changed (spec sections 5.2
//! and 5.3).
//!
//! Reading history takes two steps.
//!
//! 1. One `git log` lists every commit, newest first, with its parents, its
//!    author, its date and its message.
//! 2. The commits that are not merges are cut into runs of neighbours, one
//!    run for each worker. A worker starts one `git diff-tree --stdin`,
//!    writes its run of commit ids to it, and reads back the lines each
//!    commit added to and deleted from each file.
//!
//! How the data moves. Each worker is a thread that owns one git process,
//! which the main thread started for it. The commit ids go in through a
//! pipe and the answer comes back through another. A second, short thread
//! does the writing, because git starts to answer before it has read every
//! id: one thread doing both could end up waiting to write while git waits
//! to be read. Workers share nothing. The main thread waits for all of them
//! and joins their answers in the order of the runs, so the result is the
//! same however many workers there are and whichever finishes first.
//!
//! How errors travel. Every worker returns a `Result`. The first error, in
//! the order of the runs, becomes the error of the whole read.

use std::io::{self, Write};
use std::panic;
use std::process::{Child, Stdio};
use std::thread;

use crate::git;
use crate::numstat::{self, Parsed};
use crate::repo::{self, ObjectFormat, Reach, Repository};

/// One commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    /// How many parents the commit has: none for the first commit of a
    /// history, two or more for a merge.
    pub parents: usize,
    /// When the author wrote it, in seconds since 1970. 0 for the rare
    /// commit whose author line git cannot read.
    pub author_time: i64,
    /// The author as the commit records them. `.mailmap` is not applied:
    /// git reads that file from the work tree, so an uncommitted edit to it
    /// would change the answer.
    pub author_name: String,
    pub author_email: String,
    /// The whole message, subject and body.
    pub message: String,
    /// The files the commit changed, in the order git lists them. Always
    /// empty for a merge, which carries no line statistics of its own (spec
    /// section 5.2).
    pub changes: Vec<Change>,
}

impl Commit {
    pub fn is_merge(&self) -> bool {
        self.parents > 1
    }
}

/// One file changed by one commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The file's path. For a renamed file, its new path.
    pub path: String,
    /// The path the file had before, when this commit renamed it.
    pub renamed_from: Option<String>,
    /// The lines the commit added to the file and deleted from it, counted
    /// with git's minimal diff. Both are 0 for a binary file.
    pub added: u64,
    pub deleted: u64,
    /// True for a file git treats as binary, which has no lines to count.
    pub binary: bool,
}

/// Every commit HEAD can reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    /// Newest first, in git's topological order: a commit comes before its
    /// parents.
    pub commits: Vec<Commit>,
    /// True when a path was not valid UTF-8 and had to be converted.
    pub lossy_paths: bool,
}

/// Why the history could not be read.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The repository is a partial clone, and some of what its history
    /// needs was never downloaded. The engine does not download it.
    #[error("{missing} of the objects this history needs are not in the repository")]
    MissingObjects { missing: u64 },
    #[error(transparent)]
    Git(#[from] git::Error),
}

/// The most git processes that read line statistics at once. Each one
/// costs three open files, and a process may have few: 256 on macOS unless
/// the user raised it. 64 of them fit under that.
pub const MOST_JOBS: usize = 64;

/// What `git log` prints for each commit: its id, its parents, the author's
/// date, name and email, and the whole message. The byte 0x1F stands
/// between the fields, and `-z` ends each commit with a NUL. The message is
/// last because it is the one field that may hold a 0x1F itself.
const LOG_FORMAT: &str = "--format=%H%x1f%P%x1f%at%x1f%an%x1f%ae%x1f%B";

/// The command each worker runs. Every option that could change a number
/// is spelled out, so the user's git configuration cannot.
const DIFF_TREE: [&str; 13] = [
    "diff-tree",
    // Commit ids arrive on standard input, one on each line.
    "--stdin",
    // Print a commit's id even when the commit changed nothing. Without it
    // such a commit prints nothing, and could not be told from a commit
    // that git skipped.
    "--always",
    // Look inside folders.
    "-r",
    // Show the first commit of a history as adding all of its files.
    "--root",
    // A deleted file and an added file that are at least half alike are one
    // renamed file. In a commit where more than 1,000 files could pair up,
    // git only pairs the ones that are exactly alike.
    "-M50%",
    "-l1000",
    // The smallest possible diff, so the numbers are the ones spec section
    // 5.2 defines and do not depend on the tool that counts them.
    "--diff-algorithm=minimal",
    // Numbers, not a patch; and NUL between the pieces, so that paths
    // arrive exactly as they are stored.
    "--numstat",
    "-z",
    // Never run a program that a repository's configuration names.
    "--no-textconv",
    "--no-ext-diff",
    // A submodule entry is a pointer into another repository and not a
    // file, so a change to one is left out.
    "--ignore-submodules=all",
];

/// Reads the history of the repository's HEAD, with up to `jobs` git
/// processes reading line statistics at the same time. Fewer than 1 counts
/// as 1 and more than [`MOST_JOBS`] as that many.
pub fn read(repo: &Repository, jobs: usize) -> Result<History, Error> {
    // The commit id, not the name HEAD: every command then reads the same
    // commit, even if someone commits while the scan runs.
    let head = repo.head.as_str();

    // Counting lines needs the content of every version of every file. A
    // partial clone usually holds only the newest versions, and git would
    // download the rest. So look first, and stop if anything is missing.
    if repo.partial_clone {
        let missing = repo.missing_objects(Reach::History)?;
        if missing > 0 {
            return Err(Error::MissingObjects { missing });
        }
    }

    // `--no-show-signature` overrides the setting `log.showSignature`,
    // which makes git print lines about each signed commit's signature
    // into the middle of this output.
    let log_args = [
        "log",
        "--topo-order",
        "-z",
        "--no-show-signature",
        LOG_FORMAT,
        head,
    ];
    let log = git::succeeded(&log_args, git::output_in(&repo.root, &log_args)?)?.stdout;
    let mut commits = parse_log(&log, repo.object_format)
        // The walk starts at HEAD, so HEAD has to be the first commit.
        .filter(|commits| commits.first().is_some_and(|newest| newest.id == head))
        .ok_or_else(|| git::unexpected(&log_args, &log))?;

    // Merges are never sent to git: they carry no line statistics.
    let to_diff: Vec<&str> = commits
        .iter()
        .filter(|commit| !commit.is_merge())
        .map(|commit| commit.id.as_str())
        .collect();
    let runs = cut(&to_diff, jobs.min(MOST_JOBS));
    let answers: Vec<Result<Parsed, git::Error>> = thread::scope(|scope| {
        let mut workers = Vec::with_capacity(runs.len());
        for run in &runs {
            // The git processes are started here, one after another, and
            // not inside the threads. Starting a process needs several
            // more open files for a moment. Started all at once, 64 of
            // them ran out of the 256 that macOS allows.
            let git = start_worker(repo);
            workers.push(scope.spawn(move || line_statistics(git?, run)));
        }
        // Joining in the order of the runs is what makes the result
        // independent of which worker finishes first.
        workers
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .unwrap_or_else(|payload| panic::resume_unwind(payload))
            })
            .collect()
    });

    let mut lossy_paths = false;
    let mut changes = Vec::with_capacity(to_diff.len());
    for answer in answers {
        let answer = answer?;
        lossy_paths |= answer.lossy_paths;
        changes.extend(answer.changes);
    }
    // The answers hold one list for each commit that was sent, in the order
    // sent, which is the order of the commits that are not merges.
    let senders = commits.iter_mut().filter(|commit| !commit.is_merge());
    for (commit, changes) in senders.zip(changes) {
        commit.changes = changes;
    }

    Ok(History {
        commits,
        lossy_paths,
    })
}

/// Starts the git process of one worker. It waits for commit ids on its
/// standard input.
fn start_worker(repo: &Repository) -> Result<Child, git::Error> {
    git::command()
        .current_dir(&repo.root)
        // Which files count as binary is decided by `.gitattributes`. This
        // makes git read it from HEAD and not from the work tree, as
        // `files::list` does, so both agree on what is binary. Git older
        // than 2.41 ignores the variable.
        .env("GIT_ATTR_SOURCE", &repo.head)
        .args(DIFF_TREE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| git::could_not_start(err, &repo.root))
}

/// Asks the git process `git` what each of the commits `ids` changed.
fn line_statistics(mut git: Child, ids: &[&str]) -> Result<Parsed, git::Error> {
    let mut input = Vec::with_capacity(ids.iter().map(|id| id.len() + 1).sum());
    for id in ids {
        input.extend_from_slice(id.as_bytes());
        // Git ignores a last line that does not end with a line break.
        input.push(b'\n');
    }

    let stdin = git.stdin.take();
    let (written, output) = thread::scope(|scope| {
        let writer = scope.spawn(move || match stdin {
            // `stdin` is dropped when this ends. That closes the pipe,
            // which is how git learns that the list is complete.
            Some(mut stdin) => stdin.write_all(&input),
            None => Err(io::Error::other("git's standard input was not captured")),
        });
        // Reads everything git prints, on both of its outputs, and waits
        // for it to exit.
        let output = git.wait_with_output();
        let written = writer
            .join()
            .unwrap_or_else(|payload| panic::resume_unwind(payload));
        (written, output)
    });

    // A git that failed is reported in its own words. That comes first: a
    // git that stopped early also breaks the pipe the ids were written to,
    // and its message says more than "broken pipe" does.
    let output = git::succeeded(&DIFF_TREE, output.map_err(git::Error::Io)?)?;
    written.map_err(git::Error::Io)?;
    numstat::parse(&output.stdout, ids).ok_or_else(|| git::unexpected(&DIFF_TREE, &output.stdout))
}

/// Cuts `items` into at most `parts` runs of neighbours, as equal in length
/// as they can be. No run is empty, so fewer items than parts gives fewer
/// runs.
fn cut<T>(items: &[T], parts: usize) -> Vec<&[T]> {
    let parts = parts.clamp(1, items.len().max(1));
    let (length, longer) = (items.len() / parts, items.len() % parts);
    let mut runs = Vec::with_capacity(parts);
    let mut rest = items;
    for index in 0..parts {
        // The first `longer` runs take one item more.
        let (run, after) = rest.split_at(length + usize::from(index < longer));
        if !run.is_empty() {
            runs.push(run);
        }
        rest = after;
    }
    runs
}

/// Reads the output of `git log -z` with [`LOG_FORMAT`]. Returns `None` for
/// anything that does not have that shape.
fn parse_log(output: &[u8], format: ObjectFormat) -> Option<Vec<Commit>> {
    let mut commits = Vec::new();
    for record in git::records(output)? {
        // Six fields and no more: whatever follows the fifth separator is
        // the message, separators included.
        let mut fields = record.splitn(6, |&byte| byte == 0x1f);
        let (id, parents, time, name, email, message) = (
            fields.next()?,
            fields.next()?,
            fields.next()?,
            fields.next()?,
            fields.next()?,
            fields.next()?,
        );
        if !repo::is_object_id(id, format) {
            return None;
        }
        let mut parent_count = 0;
        for parent in parents.split(|&byte| byte == b' ') {
            // A commit with no parents has an empty field here.
            if parent.is_empty() {
                continue;
            }
            if !repo::is_object_id(parent, format) {
                return None;
            }
            parent_count += 1;
        }
        commits.push(Commit {
            id: lossy(id),
            parents: parent_count,
            author_time: author_time(time)?,
            author_name: lossy(name),
            author_email: lossy(email),
            message: lossy(message),
            changes: Vec::new(),
        });
    }
    Some(commits)
}

/// Reads the author's date. Git prints the digits it finds in the commit's
/// author line, and nothing when it cannot read that line. A commit with no
/// date, or with more digits than fit a number, counts as dated 0.
fn author_time(digits: &[u8]) -> Option<i64> {
    if !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let text = std::str::from_utf8(digits).ok()?;
    Some(text.parse().unwrap_or(0))
}

/// Text from a commit. Git was asked for UTF-8; bytes that still are not
/// valid UTF-8 are replaced.
fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What git 2.52 printed for a small history: a commit with a body, a
    /// merge with two parents, and the first commit, which has none.
    const LOG: &[u8] = b"0e33afdfb8427308d454c100c3f639a1831e6b47\x1f\
        f4159aae765ca47aeafe17916999e367e9634a4c\x1f1770000800\x1f\
        Grace Example\x1fgrace@example.com\x1f\
        Awkward files\n\nA body line.\nCo-authored-by: Someone <someone@example.com>\n\0\
        07b4e0f0082b28fc085d7e07b8382f8bb9236b29\x1f\
        c65c907b34604fdcd4cd618b012ab4b8f2dac353 c116e98fd4a8ca8fd78a57350eabd91ff75640d0\x1f\
        1770000300\x1fAda\x1fada@example.com\x1fMerge\n\0\
        bdb0b8f4401bbf199ee7650a8c1229dc146b9873\x1f\x1f1770000000\x1f\
        Ada Example\x1fada@example.com\x1fStart\n\0";

    fn parsed(output: &[u8]) -> Option<Vec<Commit>> {
        parse_log(output, ObjectFormat::Sha1)
    }

    #[test]
    fn reads_the_commits() {
        let commits = parsed(LOG).unwrap();
        assert_eq!(commits.len(), 3);

        let newest = &commits[0];
        assert_eq!(newest.id, "0e33afdfb8427308d454c100c3f639a1831e6b47");
        assert_eq!(newest.parents, 1);
        assert_eq!(newest.author_time, 1_770_000_800);
        assert_eq!(newest.author_name, "Grace Example");
        assert_eq!(newest.author_email, "grace@example.com");
        assert_eq!(
            newest.message,
            "Awkward files\n\nA body line.\nCo-authored-by: Someone <someone@example.com>\n"
        );
        assert!(newest.changes.is_empty());
        assert!(!newest.is_merge());

        assert_eq!(commits[1].parents, 2);
        assert!(commits[1].is_merge());
        assert_eq!(commits[2].parents, 0);
        assert!(!commits[2].is_merge());
    }

    #[test]
    fn a_message_may_hold_the_separator_and_may_be_empty() {
        let record =
            b"bdb0b8f4401bbf199ee7650a8c1229dc146b9873\x1f\x1f5\x1fA\x1fa@b\x1fone\x1ftwo\n\0\
            c65c907b34604fdcd4cd618b012ab4b8f2dac353\x1f\x1f6\x1fA\x1fa@b\x1f\0";
        let commits = parsed(record).unwrap();
        assert_eq!(commits[0].message, "one\u{1f}two\n");
        assert_eq!(commits[1].message, "");
    }

    #[test]
    fn a_commit_without_a_readable_date_counts_as_dated_0() {
        assert_eq!(author_time(b"1770000800"), Some(1_770_000_800));
        assert_eq!(author_time(b"0"), Some(0));
        assert_eq!(author_time(b""), Some(0));
        assert_eq!(author_time(b"99999999999999999999999"), Some(0));
        for text in [&b"-5"[..], b"12 ", b"noon", b"1.5"] {
            assert_eq!(
                author_time(text),
                None,
                "{:?}",
                String::from_utf8_lossy(text)
            );
        }
    }

    #[test]
    fn names_that_are_not_utf8_are_converted() {
        let record =
            b"bdb0b8f4401bbf199ee7650a8c1229dc146b9873\x1f\x1f5\x1fJos\xe9\x1fa@b\x1fHi\n\0";
        let commits = parsed(record).unwrap();
        assert_eq!(commits[0].author_name, "Jos\u{fffd}");
    }

    #[test]
    fn an_id_has_the_length_the_repository_uses() {
        // A SHA-1 id in a repository that uses SHA-256, and the reverse.
        assert_eq!(parse_log(LOG, ObjectFormat::Sha256), None);
        let long = b"3f9c2a1d5e7b8c9d0a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f6071\x1f\x1f5\x1fA\x1fa@b\x1fHi\n\0";
        assert_eq!(
            parse_log(long, ObjectFormat::Sha256).map(|c| c.len()),
            Some(1)
        );
        assert_eq!(parsed(long), None);
    }

    /// Output that was cut short must be refused, never half-read, and
    /// never a crash. A prefix is acceptable only where it ends a record.
    #[test]
    fn output_cut_short_is_refused() {
        for end in 0..=LOG.len() {
            let prefix = &LOG[..end];
            let whole_records = prefix.is_empty() || prefix.ends_with(&[0]);
            assert_eq!(parsed(prefix).is_some(), whole_records, "cut at {end}");
        }
    }

    #[test]
    fn output_of_another_shape_is_refused() {
        let cases: [(&str, &[u8]); 7] = [
            ("five fields", b"bdb0b8f4401bbf199ee7650a8c1229dc146b9873\x1f\x1f5\x1fA\x1fa@b\0"),
            ("an id that is too short", b"bdb0b8f\x1f\x1f5\x1fA\x1fa@b\x1fHi\n\0"),
            ("an id in capitals", b"BDB0B8F4401BBF199EE7650A8C1229DC146B9873\x1f\x1f5\x1fA\x1fa@b\x1fHi\n\0"),
            ("a parent that is not an id", b"bdb0b8f4401bbf199ee7650a8c1229dc146b9873\x1fHEAD\x1f5\x1fA\x1fa@b\x1fHi\n\0"),
            ("a date that is not a number", b"bdb0b8f4401bbf199ee7650a8c1229dc146b9873\x1f\x1fnoon\x1fA\x1fa@b\x1fHi\n\0"),
            ("a line about a signature first", b"No signature\nbdb0b8f4401bbf199ee7650a8c1229dc146b9873\x1f\x1f5\x1fA\x1fa@b\x1fHi\n\0"),
            ("an empty record", b"\0"),
        ];
        for (what, output) in cases {
            assert_eq!(parsed(output), None, "{what}");
        }
    }

    #[test]
    fn cuts_into_runs_of_neighbours_of_nearly_equal_length() {
        let items: Vec<u32> = (0..10).collect();
        let lengths =
            |parts| -> Vec<usize> { cut(&items, parts).iter().map(|run| run.len()).collect() };
        assert_eq!(lengths(1), [10]);
        assert_eq!(lengths(3), [4, 3, 3]);
        assert_eq!(lengths(4), [3, 3, 2, 2]);
        assert_eq!(lengths(10), [1; 10]);
        // More parts than items: one item each, and no empty run.
        assert_eq!(lengths(64), [1; 10]);
        // Nothing to ask for is treated as one part.
        assert_eq!(lengths(0), [10]);
        assert!(cut::<u32>(&[], 4).is_empty());
    }

    /// However the items are cut, the runs put back together are the items,
    /// in order, and no two runs differ in length by more than one.
    #[test]
    fn cutting_loses_nothing_and_keeps_the_order() {
        for count in 0..40_usize {
            let items: Vec<usize> = (0..count).collect();
            for parts in 0..45 {
                let runs = cut(&items, parts);
                assert_eq!(runs.concat(), items, "{count} items in {parts} parts");
                assert!(runs.len() <= parts.max(1));
                assert!(runs.iter().all(|run| !run.is_empty()));
                let longest = runs.iter().map(|run| run.len()).max().unwrap_or(0);
                let shortest = runs.iter().map(|run| run.len()).min().unwrap_or(0);
                assert!(longest - shortest <= 1, "{count} items in {parts} parts");
            }
        }
    }

    #[test]
    fn the_worker_command_pins_what_could_change_a_number() {
        for option in [
            "--stdin",
            "--always",
            "--root",
            "-M50%",
            "-l1000",
            "--diff-algorithm=minimal",
            "--no-textconv",
            "--no-ext-diff",
            "--ignore-submodules=all",
        ] {
            assert!(DIFF_TREE.contains(&option), "{option} is missing");
        }
    }
}
