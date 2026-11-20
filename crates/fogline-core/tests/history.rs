//! History against real repositories.
//!
//! The engine takes its numbers from `git diff-tree`, run by several
//! processes at once. These tests check them by three other routes:
//!
//! - by the definition in spec section 5.2: read both versions of every
//!   changed file and work out the longest run of lines they share;
//! - against one `git log --numstat` process, which does the same work
//!   without it being cut into runs;
//! - against itself, with other numbers of workers.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::env;
use std::fs;
use std::io;
use std::path::Path;

use fogline_core::aggregate::{self, PathHistory, Totals};
use fogline_core::git::{self, Version};
use fogline_core::history::{self, Change, Commit, History};
use fogline_core::repo::{self, Repository};
use fogline_testkit::{Fixture, Identity, RepoBuilder, f1, f3, f5};

const DAY: i64 = 86_400;

fn discover(fixture: &Fixture) -> Repository {
    repo::discover(&fixture.path()).expect("the fixture is a repository")
}

fn read(fixture: &Fixture) -> History {
    history::read(&discover(fixture), 3).expect("its history can be read")
}

/// The first line of a commit's message.
fn subject(commit: &Commit) -> &str {
    commit.message.lines().next().unwrap_or_default()
}

/// The commit whose message starts with `subject`.
fn commit_named<'a>(history: &'a History, subject_line: &str) -> &'a Commit {
    history
        .commits
        .iter()
        .find(|commit| subject(commit) == subject_line)
        .unwrap_or_else(|| panic!("no commit is called {subject_line:?}"))
}

fn change(path: &str, added: u64, deleted: u64) -> Change {
    Change {
        path: path.to_owned(),
        renamed_from: None,
        added,
        deleted,
        binary: false,
    }
}

fn renamed(from: &str, to: &str, added: u64, deleted: u64) -> Change {
    Change {
        renamed_from: Some(from.to_owned()),
        ..change(to, added, deleted)
    }
}

/// Runs git the way the engine does, in `dir`, and returns what it printed.
fn git_in(dir: &Path, env: &[(&str, &str)], args: &[&str]) -> Vec<u8> {
    let output = git::command()
        .current_dir(dir)
        .envs(env.iter().copied())
        .args(args)
        .output()
        .expect("git can be started");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

// ---- What the fixtures hold, spelled out ----

#[test]
fn reads_the_history_of_a_plain_repository() {
    let fixture = f1::build().unwrap();
    let history = read(&fixture);
    assert!(!history.lossy_paths);
    assert_eq!(history.commits.len(), f1::COMMITS);
    assert_eq!(history.commits[0].id, f1::HEAD);

    // Newest first. F1 starts on 2026-01-05 and has one commit a day.
    let start = 1_767_603_600;
    let ada = ("Ada Example", "ada@example.com");
    let grace = ("Grace Example", "grace@example.com");
    let expected = [
        (
            "Add notes and mention the tests",
            ada,
            start + 4 * DAY,
            vec![change("docs/guide.md", 2, 0), change("docs/notes.md", 3, 0)],
        ),
        (
            "Introduce a Greeting model",
            grace,
            start + 3 * DAY,
            vec![change("src/models.py", 3, 0), change("src/util.py", 4, 1)],
        ),
        (
            "Document how to run the app",
            ada,
            start + 2 * DAY,
            vec![change("docs/guide.md", 3, 0)],
        ),
        (
            "Test the greeting and make the app runnable",
            grace,
            start + DAY,
            vec![
                change("src/app.py", 4, 0),
                change("tests/test_app.py", 5, 0),
            ],
        ),
        (
            "Add the app and a greeting helper",
            ada,
            start,
            vec![change("src/app.py", 5, 0), change("src/util.py", 2, 0)],
        ),
    ];
    let actual: Vec<_> = history
        .commits
        .iter()
        .map(|commit| {
            (
                subject(commit),
                (commit.author_name.as_str(), commit.author_email.as_str()),
                commit.author_time,
                commit.changes.clone(),
            )
        })
        .collect();
    assert_eq!(actual, expected);

    // The message is kept whole, body included.
    assert_eq!(
        history.commits[1].message,
        "Introduce a Greeting model\n\n\
         The helper now builds a Greeting instead of using the bare name.\n"
    );
    let parents: Vec<usize> = history
        .commits
        .iter()
        .map(|commit| commit.parents)
        .collect();
    assert_eq!(parents, [1, 1, 1, 1, 0]);

    assert_eq!(
        aggregate::totals(&history),
        Totals {
            commits: 5,
            merges: 0,
            added: f1::ADDED,
            deleted: f1::DELETED,
        }
    );
}

#[test]
fn reads_merges_renames_deletions_and_commits_that_change_nothing() {
    let fixture = f3::build().unwrap();
    let history = read(&fixture);
    assert!(!history.lossy_paths);
    assert_eq!(history.commits.len(), f3::COMMITS);

    // The order is git's own.
    let order = fixture.git(["rev-list", "--topo-order", "HEAD"]).unwrap();
    let ids: Vec<&str> = history
        .commits
        .iter()
        .map(|commit| commit.id.as_str())
        .collect();
    assert_eq!(ids, order.lines().collect::<Vec<_>>());

    let changes = |subject_line: &str| commit_named(&history, subject_line).changes.clone();

    // A rename with no other change, and a rename with one line edited.
    assert_eq!(
        changes("Rename a module"),
        [renamed("src/old_name.py", "src/new_name.py", 0, 0)]
    );
    assert_eq!(
        changes("Rename the report and change its title"),
        [renamed("src/report.py", "src/summary.py", 1, 1)]
    );
    // A deleted file loses its lines; a commit can change nothing at all.
    assert_eq!(
        changes("Remove a module nobody uses"),
        [change("src/doomed.py", 0, 2)]
    );
    assert_eq!(changes("Mark the release"), []);
    // A merge has two parents and no line statistics of its own.
    let merge = commit_named(&history, "Merge the feature branch");
    assert_eq!(merge.parents, 2);
    assert!(merge.is_merge());
    assert_eq!(merge.changes, []);
    assert_eq!(changes("Greet by name"), [change("src/app.py", 2, 2)]);
    assert_eq!(
        changes("Add a feature on its own branch"),
        [change("src/feature.py", 2, 0)]
    );

    let first = commit_named(&history, "Start the project");
    assert_eq!(first.parents, 0);
    assert_eq!(
        first.changes,
        [
            change("README.md", 3, 0),
            change("src/app.py", 2, 0),
            change("src/doomed.py", 2, 0),
            change("src/old_name.py", 6, 0),
            change("src/report.py", 8, 0),
        ]
    );

    // The awkward files: a binary file has no lines, an empty file has
    // none either, a path may hold a space or a letter that is not ASCII,
    // the link counts the one line git shows for it, and the submodule
    // entry `modules/engine` is not there at all.
    assert_eq!(
        changes("Add the awkward files"),
        [
            change("LICENSE", 1, 0),
            change("a/b/c/d/e/f/deep.txt", 1, 0),
            Change {
                binary: true,
                ..change("assets/logo.bin", 0, 0)
            },
            change("docs/caf\u{e9}.md", 3, 0),
            change("docs/release notes.md", 3, 0),
            change(f3::LINK, 1, 0),
            change("package-lock.json", 4, 0),
            change("scripts/run.bat", 2, 0),
            change("src/empty.py", 0, 0),
            change("src/no_newline.txt", 2, 0),
            change("vendor/lib/util.js", 1, 0),
            change("web/app.min.js", 1, 0),
            change("web/bundle.js", 1, 0),
        ]
    );

    assert_eq!(
        aggregate::totals(&history),
        Totals {
            commits: 9,
            merges: 1,
            added: f3::ADDED,
            deleted: f3::DELETED,
        }
    );
}

#[test]
fn sums_the_history_of_each_path() {
    let fixture = f3::build().unwrap();
    let history = read(&fixture);
    let paths = aggregate::by_path(&history);

    // F3 starts on 2026-02-02 and has one commit a day.
    let start = 1_770_022_800;
    // Ada wrote it and Ada changed it, two days apart.
    assert_eq!(
        paths["src/app.py"],
        PathHistory {
            commits: 2,
            added: 4,
            deleted: 2,
            authors: 1,
            first_at: start,
            last_at: start + 2 * DAY,
        }
    );
    // Ada added it and Grace deleted it.
    assert_eq!(
        paths["src/doomed.py"],
        PathHistory {
            commits: 2,
            added: 2,
            deleted: 2,
            authors: 2,
            first_at: start,
            last_at: start + 6 * DAY,
        }
    );
    // A rename counts for the new path, and what came before stays with
    // the old one (carrying it over is task 3.5).
    assert_eq!(
        (
            paths["src/old_name.py"].commits,
            paths["src/old_name.py"].added
        ),
        (1, 6)
    );
    assert_eq!(
        (
            paths["src/new_name.py"].commits,
            paths["src/new_name.py"].added
        ),
        (1, 0)
    );
    // A binary file has a commit and no lines.
    assert_eq!(
        (
            paths["assets/logo.bin"].commits,
            paths["assets/logo.bin"].added
        ),
        (1, 0)
    );
    assert!(!paths.contains_key(f3::SUBMODULE));

    // Every path's lines together are the totals.
    let totals = aggregate::totals(&history);
    assert_eq!(
        paths.values().map(|path| path.added).sum::<u64>(),
        totals.added
    );
    assert_eq!(
        paths.values().map(|path| path.deleted).sum::<u64>(),
        totals.deleted
    );
}

// ---- Route one: the definition ----

/// One entry of a commit's tree: its mode and the id of its content.
type Entry = (String, String);

/// The files of a commit, by path. Submodule entries are left out, as the
/// engine leaves them out.
fn tree(dir: &Path, commit: &str) -> BTreeMap<String, Entry> {
    let output = git_in(dir, &[], &["ls-tree", "-r", "-z", commit]);
    let mut entries = BTreeMap::new();
    for record in output
        .split(|&byte| byte == 0)
        .filter(|record| !record.is_empty())
    {
        // `<mode> <type> <id>` TAB `<path>`
        let text = String::from_utf8_lossy(record);
        let (meta, path) = text.split_once('\t').expect("a tab before the path");
        let fields: Vec<&str> = meta.split(' ').collect();
        if fields[1] == "blob" {
            entries.insert(
                path.to_owned(),
                (fields[0].to_owned(), fields[2].to_owned()),
            );
        }
    }
    entries
}

/// Git calls a file binary when one of its first 8,000 bytes is zero.
fn is_binary(content: &[u8]) -> bool {
    content.iter().take(8000).any(|&byte| byte == 0)
}

/// How many lines the longest run of lines shared by `old` and `new` has,
/// where the lines of a run keep their order but need not be neighbours.
fn longest_common_subsequence(old: &[&[u8]], new: &[&[u8]]) -> usize {
    // row[j] is the answer for the old lines seen so far and the first j
    // new lines.
    let mut row = vec![0; new.len() + 1];
    for old_line in old {
        let mut diagonal = 0;
        for (j, new_line) in new.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = if old_line == new_line {
                diagonal + 1
            } else {
                above.max(row[j])
            };
            diagonal = above;
        }
    }
    row[new.len()]
}

/// The lines added and deleted between two versions of a file, by the
/// definition in spec section 5.2, and whether the file is binary.
fn by_definition(old: &[u8], new: &[u8]) -> (u64, u64, bool) {
    if is_binary(old) || is_binary(new) {
        return (0, 0, true);
    }
    // A line includes its line ending, so a last line that gains one counts
    // as changed, as it does for git.
    let old: Vec<&[u8]> = old.split_inclusive(|&byte| byte == b'\n').collect();
    let new: Vec<&[u8]> = new.split_inclusive(|&byte| byte == b'\n').collect();
    let shared = longest_common_subsequence(&old, &new);
    (
        (new.len() - shared) as u64,
        (old.len() - shared) as u64,
        false,
    )
}

/// Checks every change of every commit against the two versions of the
/// file, and checks that no changed file was left out.
fn assert_follows_the_definition(found: &Repository, history: &History) {
    let dir = found.root.as_path();
    let mut trees: HashMap<String, BTreeMap<String, Entry>> = HashMap::new();
    let mut contents: HashMap<String, Vec<u8>> = HashMap::new();
    let mut tree_of = |commit: &str| {
        trees
            .entry(commit.to_owned())
            .or_insert_with(|| tree(dir, commit))
            .clone()
    };
    let mut content_of = |entry: Option<&Entry>| -> Vec<u8> {
        let Some((_, id)) = entry else {
            // No file on this side: it was added, or deleted.
            return Vec::new();
        };
        contents
            .entry(id.clone())
            .or_insert_with(|| git_in(dir, &[], &["cat-file", "blob", id]))
            .clone()
    };
    // Before 2.50, git's minimal diff could be a little longer than the
    // smallest one. It then counts the same number of extra lines on both
    // sides.
    let exact = found.git >= Version::EXACT_MINIMAL_DIFF;

    for commit in &history.commits {
        if commit.is_merge() {
            assert!(commit.changes.is_empty(), "a merge has no changes");
            continue;
        }
        let after = tree_of(&commit.id);
        let before = if commit.parents == 0 {
            BTreeMap::new()
        } else {
            tree_of(&format!("{}^", commit.id))
        };
        // Every path whose entry is not the same on both sides.
        let mut unexplained: BTreeSet<&str> = before
            .keys()
            .chain(after.keys())
            .map(String::as_str)
            .filter(|&path| before.get(path) != after.get(path))
            .collect();

        for change in &commit.changes {
            let context = format!("{} in {}", change.path, subject(commit));
            let old_path = change.renamed_from.as_deref().unwrap_or(&change.path);
            if let Some(old) = &change.renamed_from {
                // A rename pairs a file that went away with one that came.
                assert!(
                    before.contains_key(old) && !after.contains_key(old),
                    "{context}"
                );
                assert!(!before.contains_key(&change.path), "{context}");
                assert!(unexplained.remove(old.as_str()), "{context}");
            }
            assert!(unexplained.remove(change.path.as_str()), "{context}");

            let old = content_of(before.get(old_path));
            let new = content_of(after.get(&change.path));
            let (added, deleted, binary) = by_definition(&old, &new);
            assert_eq!(change.binary, binary, "{context}");
            if exact || binary {
                assert_eq!(
                    (change.added, change.deleted),
                    (added, deleted),
                    "{context}"
                );
            } else {
                let extra = change.added.checked_sub(added).expect("never fewer lines");
                assert_eq!(change.deleted, deleted + extra, "{context}");
            }
        }
        assert!(
            unexplained.is_empty(),
            "{} changed {unexplained:?} and the history does not say so",
            subject(commit)
        );
    }
}

// ---- Route two: one git process ----

/// Reads the same history from a single `git log --numstat`, which walks
/// the commits itself and prints each one's changes under its id.
fn one_process(found: &Repository) -> Vec<(String, Vec<Change>)> {
    let head = found.head.as_str();
    let output = git_in(
        &found.root,
        &[("GIT_ATTR_SOURCE", head)],
        &[
            "log",
            "--topo-order",
            "-z",
            "--no-show-signature",
            "--format=%H",
            "--numstat",
            "--root",
            "-M50%",
            "-l1000",
            "--diff-algorithm=minimal",
            "--no-textconv",
            "--no-ext-diff",
            "--ignore-submodules=all",
            head,
        ],
    );
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    let body = output
        .strip_suffix(&[0])
        .expect("the output ends with a NUL");

    let mut commits: Vec<(String, Vec<Change>)> = Vec::new();
    let mut pieces = body.split(|&byte| byte == 0);
    while let Some(piece) = pieces.next() {
        // `git log` puts a line break before the first change of a commit.
        let piece = piece.strip_prefix(b"\n").unwrap_or(piece);
        let fields: Vec<&[u8]> = piece.splitn(3, |&byte| byte == b'\t').collect();
        match fields.as_slice() {
            [id] => commits.push((text(id), Vec::new())),
            [added, deleted, path] => {
                let (path, renamed_from) = if path.is_empty() {
                    let old = pieces.next().expect("the old path of a rename");
                    let new = pieces.next().expect("the new path of a rename");
                    (text(new), Some(text(old)))
                } else {
                    (text(path), None)
                };
                let binary = *added == b"-";
                let number = |digits: &[u8]| -> u64 {
                    if binary {
                        0
                    } else {
                        text(digits).parse().expect("a number of lines")
                    }
                };
                let changes = &mut commits.last_mut().expect("a commit before a change").1;
                changes.push(Change {
                    path,
                    renamed_from,
                    added: number(added),
                    deleted: number(deleted),
                    binary,
                });
            }
            other => panic!("a piece with {} fields", other.len()),
        }
    }
    commits
}

/// True when the history is, commit for commit and change for change, what
/// one git process reports.
fn agrees_with_one_process(found: &Repository, history: &History) -> bool {
    let reference = one_process(found);
    history.commits.len() == reference.len()
        && history
            .commits
            .iter()
            .zip(&reference)
            .all(|(commit, (id, changes))| &commit.id == id && &commit.changes == changes)
}

// ---- A larger history, made by a fixed sequence of numbers ----

/// A stream of numbers that is the same on every run (xorshift).
struct Numbers(u64);

impl Numbers {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// A number from 0 up to, and not including, `limit`.
    fn below(&mut self, limit: usize) -> usize {
        (self.next() % limit as u64) as usize
    }

    /// A line of a file. Half of them come from a handful of lines that
    /// repeat, as blank lines and closing braces do in code, because lines
    /// that repeat are what makes a diff hard to keep small.
    fn line(&mut self) -> String {
        const COMMON: [&str; 5] = ["", "}", "    return None", "import os", "# TODO"];
        if self.below(2) == 0 {
            COMMON[self.below(COMMON.len())].to_owned()
        } else {
            format!("value_{} = {}", self.below(40), self.below(1000))
        }
    }
}

/// A text file as the generator knows it.
struct Text {
    lines: Vec<String>,
    /// False when the last line has no line ending.
    ends_with_newline: bool,
}

impl Text {
    fn content(&self) -> String {
        let mut content = self.lines.join("\n");
        if self.ends_with_newline && !self.lines.is_empty() {
            content.push('\n');
        }
        content
    }
}

/// Builds a history of about fifty commits: files added, edited at random
/// places, deleted, renamed, renamed and edited at once, binary files, and
/// a merge every seventh step.
fn generated() -> io::Result<Fixture> {
    const AUTHORS: [Identity; 3] = [
        Identity {
            name: "Ada Example",
            email: "ada@example.com",
        },
        Identity {
            name: "Grace Example",
            email: "grace@example.com",
        },
        Identity {
            name: "Lin Example",
            email: "lin@example.com",
        },
    ];
    let mut numbers = Numbers(0x2545_F491_4F6C_DD1D);
    let repo = RepoBuilder::new()?;
    let mut files: BTreeMap<String, Text> = BTreeMap::new();
    let mut made = 0;
    let mut time = 1_780_000_000;

    for step in 0..36 {
        // Every seventh step also happens on a side branch, which adds one
        // file of its own and is merged back afterwards.
        let mut side = None;
        if step % 7 == 6 {
            let branch = format!("side-{step}");
            let path = format!("side/{step}.txt");
            repo.start_branch(&branch)?;
            let text = Text {
                lines: (0..6).map(|_| numbers.line()).collect(),
                ends_with_newline: true,
            };
            repo.write(&path, &text.content())?;
            time += 600;
            repo.commit(AUTHORS[1], time, &format!("Side work at step {step}\n"))?;
            repo.switch("main")?;
            side = Some((branch, path, text));
        }

        for _ in 0..=numbers.below(3) {
            let existing: Vec<String> = files.keys().cloned().collect();
            let pick = |numbers: &mut Numbers| existing[numbers.below(existing.len())].clone();
            match numbers.below(12) {
                // Edit a file in up to three places.
                0..=5 if !existing.is_empty() => {
                    let path = pick(&mut numbers);
                    let text = files.get_mut(&path).expect("the file is known");
                    for _ in 0..=numbers.below(3) {
                        let at = numbers.below(text.lines.len() + 1);
                        match numbers.below(3) {
                            0 => {
                                for _ in 0..=numbers.below(5) {
                                    text.lines.insert(at, numbers.line());
                                }
                            }
                            1 => {
                                let end = (at + 1 + numbers.below(4)).min(text.lines.len());
                                text.lines.drain(at.min(end)..end);
                            }
                            _ => {
                                if let Some(line) = text.lines.get_mut(at) {
                                    *line = numbers.line();
                                }
                            }
                        }
                    }
                    if numbers.below(6) == 0 {
                        text.ends_with_newline = !text.ends_with_newline;
                    }
                    repo.write(&path, &text.content())?;
                }
                // Delete a file.
                6 if existing.len() > 3 => {
                    let path = pick(&mut numbers);
                    files.remove(&path);
                    repo.remove(&path)?;
                }
                // Rename a file, and half of the time edit one line of it.
                7 | 8 if !existing.is_empty() => {
                    let path = pick(&mut numbers);
                    let mut text = files.remove(&path).expect("the file is known");
                    made += 1;
                    let new_path = format!("moved/{made}.py");
                    repo.rename(&path, &new_path)?;
                    if numbers.below(2) == 0 && text.lines.len() >= 8 {
                        let at = numbers.below(text.lines.len());
                        text.lines[at] = format!("renamed_and_edited = {made}");
                        repo.write(&new_path, &text.content())?;
                    }
                    files.insert(new_path, text);
                }
                // A binary file, new or written over.
                9 => {
                    let bytes = [0x89, b'P', 0, numbers.below(256) as u8, 0, 1, 2, 3];
                    repo.write_bytes(&format!("assets/{}.bin", numbers.below(3)), &bytes)?;
                }
                // Add a file.
                _ => {
                    made += 1;
                    let path = format!("src/dir{}/file{made}.py", numbers.below(3));
                    let text = Text {
                        lines: (0..3 + numbers.below(28)).map(|_| numbers.line()).collect(),
                        ends_with_newline: true,
                    };
                    repo.write(&path, &text.content())?;
                    files.insert(path, text);
                }
            }
        }
        time += 3600;
        repo.commit(AUTHORS[step % 3], time, &format!("Step {step}\n"))?;

        if let Some((branch, path, text)) = side {
            time += 600;
            repo.merge(AUTHORS[0], time, &branch, &format!("Merge {branch}"))?;
            files.insert(path, text);
        }
    }
    repo.finish()
}

// ---- The checks ----

#[test]
fn the_numbers_follow_the_definition() {
    for fixture in [f1::build().unwrap(), f3::build().unwrap()] {
        let found = discover(&fixture);
        let history = history::read(&found, 3).unwrap();
        assert_follows_the_definition(&found, &history);
    }
}

#[test]
fn the_result_does_not_depend_on_the_number_of_workers() {
    for fixture in [f1::build().unwrap(), f3::build().unwrap()] {
        let found = discover(&fixture);
        let alone = history::read(&found, 1).unwrap();
        // 0 counts as 1, and more workers than commits as one for each.
        for jobs in [0, 2, 3, 4, 5, 7, 8, 9, 11, 12, 64, 1000] {
            let together = history::read(&found, jobs).unwrap();
            assert_eq!(together, alone, "{jobs} workers");
        }
    }
}

#[test]
fn one_git_process_reports_the_same_history() {
    for fixture in [f1::build().unwrap(), f3::build().unwrap()] {
        let found = discover(&fixture);
        let history = history::read(&found, 4).unwrap();
        assert_eq!(
            history
                .commits
                .iter()
                .map(|commit| (commit.id.clone(), commit.changes.clone()))
                .collect::<Vec<_>>(),
            one_process(&found)
        );
    }
}

/// All three routes on a history large enough to have something in every
/// worker's run, with lines that repeat.
#[test]
fn a_generated_history_passes_every_check() {
    let fixture = generated().unwrap();
    let found = discover(&fixture);
    let alone = history::read(&found, 1).unwrap();

    // The generator did what it says: enough commits, with merges, renames,
    // deletions and binary files among them.
    let totals = aggregate::totals(&alone);
    assert!(totals.commits >= 46, "{totals:?}");
    assert_eq!(totals.merges, 5);
    let changes: Vec<&Change> = alone
        .commits
        .iter()
        .flat_map(|commit| &commit.changes)
        .collect();
    assert!(
        changes
            .iter()
            .filter(|change| change.renamed_from.is_some())
            .count()
            >= 3
    );
    assert!(changes.iter().any(|change| change.binary));
    assert!(
        changes
            .iter()
            .any(|change| change.added == 0 && change.deleted > 0)
    );
    assert!(
        changes
            .iter()
            .any(|change| change.added > 0 && change.deleted > 0)
    );

    assert_follows_the_definition(&found, &alone);
    assert!(
        agrees_with_one_process(&found, &alone),
        "one process disagrees"
    );
    for jobs in [2, 5, 11, 64] {
        assert!(
            history::read(&found, jobs).unwrap() == alone,
            "{jobs} workers"
        );
    }
}

/// On small files every way of diffing finds the smallest diff. On a large
/// file with many lines that repeat, git's default takes shortcuts and
/// reports more changed lines than there have to be. The engine asks for
/// the smallest diff, and this is a file where that shows.
#[test]
fn the_diff_is_the_smallest_one_even_where_gits_default_is_not() {
    let who = Identity {
        name: "Ada Example",
        email: "ada@example.com",
    };
    // 3,000 lines drawn from 60, then about half of them kept, a fifth
    // replaced, the rest dropped, and new lines put in between.
    let mut numbers = Numbers(0x9E37_79B9_7F4A_7C15);
    let line = |numbers: &mut Numbers| format!("line {}\n", numbers.below(60));
    let old: Vec<String> = (0..3000).map(|_| line(&mut numbers)).collect();
    let mut new = Vec::new();
    for kept in &old {
        match numbers.below(100) {
            0..=49 => new.push(kept.clone()),
            50..=69 => new.push(line(&mut numbers)),
            _ => {}
        }
        if numbers.below(100) < 20 {
            new.push(line(&mut numbers));
        }
    }
    let repo = RepoBuilder::new().unwrap();
    repo.write("big.txt", &old.concat()).unwrap();
    repo.commit(who, 1_770_000_000, "Add a large file\n")
        .unwrap();
    repo.write("big.txt", &new.concat()).unwrap();
    repo.commit(who, 1_770_000_100, "Rewrite half of it\n")
        .unwrap();
    let fixture = repo.finish().unwrap();

    let found = discover(&fixture);
    let history = history::read(&found, 2).unwrap();
    assert_follows_the_definition(&found, &history);

    // The check above would pass for any correct diff of a small file. What
    // makes it mean something here: git's default really is longer.
    let rewrite = &history.commits[0].changes[0];
    let default = git_in(
        &found.root,
        &[],
        &[
            "diff",
            "--numstat",
            "--diff-algorithm=myers",
            "HEAD~1",
            "HEAD",
        ],
    );
    let default = String::from_utf8_lossy(&default).into_owned();
    let default_added: u64 = default
        .split('\t')
        .next()
        .and_then(|digits| digits.parse().ok())
        .expect("a number of added lines");
    assert!(
        default_added > rewrite.added,
        "git's default counts {default_added} added lines and the engine {}",
        rewrite.added
    );
}

/// The same checks on any repository, for the ones too large to build in a
/// test. Django's history takes about a minute:
///
/// ```text
/// FOGLINE_HISTORY_REPO=/path/to/django cargo test --release -p fogline-core \
///     --test history -- --ignored --nocapture
/// ```
#[test]
#[ignore = "set FOGLINE_HISTORY_REPO to the path of a repository"]
fn a_repository_of_your_choice_passes_the_checks() {
    let path = env::var_os("FOGLINE_HISTORY_REPO")
        .expect("FOGLINE_HISTORY_REPO names the repository to read");
    let found = repo::discover(Path::new(&path)).unwrap();
    let alone = history::read(&found, 1).unwrap();
    for jobs in [4, 11] {
        assert!(
            history::read(&found, jobs).unwrap() == alone,
            "{jobs} workers"
        );
    }
    assert!(
        agrees_with_one_process(&found, &alone),
        "one process disagrees"
    );

    let totals = aggregate::totals(&alone);
    let changes: Vec<&Change> = alone
        .commits
        .iter()
        .flat_map(|commit| &commit.changes)
        .collect();
    println!(
        "{} @ {}: {} commits, {} merges, {} changes of which {} renames, \
         {} lines added, {} deleted, {} paths",
        found.name,
        &found.head[..10],
        totals.commits,
        totals.merges,
        changes.len(),
        changes
            .iter()
            .filter(|change| change.renamed_from.is_some())
            .count(),
        totals.added,
        totals.deleted,
        aggregate::by_path(&alone).len(),
    );
}

// ---- Other kinds of repository ----

#[test]
fn a_bare_clone_has_the_same_history() {
    let fixture = f3::build().unwrap();
    let bare = fixture.git_clone(&["--bare"]).unwrap();
    assert_eq!(read(&bare), read(&fixture));
}

/// A shallow clone holds one commit and nothing before it, so that commit
/// looks like the first one and seems to add every line.
#[test]
fn a_shallow_clone_has_only_the_history_it_holds() {
    let shallow = f5::build().unwrap();
    let history = read(&shallow);
    assert_eq!(history.commits.len(), f5::COMMITS);
    assert_eq!(history.commits[0].id, f1::HEAD);
    let paths: Vec<&str> = history.commits[0]
        .changes
        .iter()
        .map(|change| change.path.as_str())
        .collect();
    assert_eq!(paths, f1::FILES);
    assert_eq!(aggregate::totals(&history).added, f5::ADDED);
    assert_eq!(aggregate::totals(&history).deleted, 0);
}

#[test]
fn history_read_from_a_subfolder_is_the_same() {
    let fixture = f1::build().unwrap();
    let inside = repo::discover(&fixture.path().join("src")).unwrap();
    assert_eq!(history::read(&inside, 2).unwrap(), read(&fixture));
}

/// `.gitattributes` can declare a text file binary. For history too, the
/// answer has to come from the commit at HEAD: it must not change with an
/// edit that was never committed, and a bare repository must not lose it.
#[test]
fn attributes_come_from_the_commit_not_the_work_tree() {
    let who = Identity {
        name: "Ada Example",
        email: "ada@example.com",
    };
    let repo = RepoBuilder::new().unwrap();
    repo.write(".gitattributes", "*.dat binary\n").unwrap();
    repo.write("marked.dat", "text, but declared binary\n")
        .unwrap();
    repo.write("plain.txt", "one\n").unwrap();
    repo.commit(who, 1_770_000_000, "Declare the data files binary\n")
        .unwrap();
    repo.write("marked.dat", "text, but declared binary\nsecond line\n")
        .unwrap();
    repo.write("plain.txt", "one\ntwo\n").unwrap();
    repo.commit(who, 1_770_000_100, "Add a line to both\n")
        .unwrap();
    let fixture = repo.finish().unwrap();

    // The newest commit's change to `path`: is it binary, and how many
    // lines did it add?
    let newest = |history: &History, path: &str| {
        let change = history.commits[0]
            .changes
            .iter()
            .find(|change| change.path == path)
            .unwrap_or_else(|| panic!("{path} did not change"));
        (change.binary, change.added)
    };

    let found = discover(&fixture);
    let committed = history::read(&found, 2).unwrap();
    assert_eq!(newest(&committed, "marked.dat"), (true, 0));
    assert_eq!(newest(&committed, "plain.txt"), (false, 1));

    let bare = read(&fixture.git_clone(&["--bare"]).unwrap());

    // Now say the opposite in the work tree, without committing it.
    fs::write(fixture.path().join(".gitattributes"), "*.txt binary\n").unwrap();
    let after_edit = read(&fixture);

    if found.git >= Version::ATTRIBUTES_FROM_COMMIT {
        assert_eq!(bare, committed, "a bare clone reads the same attributes");
        assert_eq!(after_edit, committed, "an uncommitted edit changes nothing");
    } else {
        // A git older than 2.41 cannot read attributes from a commit, and
        // the numbers follow the work tree. The scan says so.
        assert_eq!(newest(&bare, "marked.dat"), (false, 1));
        assert_eq!(newest(&after_edit, "plain.txt"), (true, 0));
    }
}

/// Counting lines needs every version of every file. A partial clone that
/// checked out HEAD holds only the newest versions, and git would download
/// the rest. Reading has to stop instead, and leave the clone as it was.
#[test]
fn a_partial_clone_without_its_history_is_refused_and_left_alone() {
    let source = f1::build().unwrap();
    let clone = source.partial_clone(true).unwrap();
    let found = discover(&clone);
    let before = clone.snapshot().unwrap();

    let result = history::read(&found, 4);

    assert!(
        matches!(result, Err(history::Error::MissingObjects { missing: 3 })),
        "{result:?}"
    );
    assert!(
        clone.snapshot().unwrap() == before,
        "reading changed the clone: something was downloaded"
    );
}

/// With every version in the clone there is nothing to download, and its
/// history is read as it is anywhere else.
#[test]
fn a_partial_clone_that_lacks_nothing_is_read_and_left_alone() {
    let source = f1::build().unwrap();
    let clone = source.complete_partial_clone().unwrap();
    let found = discover(&clone);
    assert!(found.partial_clone);
    let before = clone.snapshot().unwrap();

    let history = history::read(&found, 4).unwrap();

    assert_eq!(history, read(&source));
    assert!(
        clone.snapshot().unwrap() == before,
        "reading changed the clone"
    );
}
