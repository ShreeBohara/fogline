//! Sums over a history (spec section 5.2): the totals of the repository,
//! what history says about each file, and what each agent signature adds up
//! to.
//!
//! The totals and the sums for each signature are plain sums over the
//! commits, so they do not depend on the order of the commits. The sums for
//! each file follow a file through its renames, and for that they read the
//! commits in the order [`History`] keeps them: newest first. None of them
//! depends on how the work of reading the history was divided.
//!
//! The sums about agents and bots follow [`crate::attribution`], which
//! reads each commit's class from the marks on it. A commit's lines all go
//! to the commit's class, whoever wrote them, so the commit counts are
//! lower bounds and the line counts are estimates.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::attribution::{self, Class};
use crate::history::{Commit, History};
use crate::signatures;

/// What the whole history adds up to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Totals {
    /// Every commit HEAD can reach, merges included.
    pub commits: u64,
    /// How many of them are merges.
    pub merges: u64,
    /// How many carry the mark of a coding agent: at least this many are
    /// agent-attributed.
    pub commits_agent: u64,
    /// How many a bot made, with no mark of an agent on them.
    pub commits_automation: u64,
    /// Lines added and deleted by the commits that are not merges, in every
    /// file they touched, including files that no longer exist.
    pub added: u64,
    /// The added lines that came in agent-attributed commits, and in
    /// commits made by bots. Both are estimates.
    pub added_agent: u64,
    pub added_automation: u64,
    pub deleted: u64,
}

/// Adds up a history.
pub fn totals(history: &History) -> Totals {
    let mut totals = Totals {
        commits: 0,
        merges: 0,
        commits_agent: 0,
        commits_automation: 0,
        added: 0,
        added_agent: 0,
        added_automation: 0,
        deleted: 0,
    };
    for commit in &history.commits {
        let added = added_by(commit);
        totals.commits += 1;
        if commit.is_merge() {
            totals.merges += 1;
        }
        totals.added += added;
        for change in &commit.changes {
            totals.deleted += change.deleted;
        }
        match attribution::of(commit).class {
            Class::AgentAttributed => {
                totals.commits_agent += 1;
                totals.added_agent += added;
            }
            Class::Automation => {
                totals.commits_automation += 1;
                totals.added_automation += added;
            }
            Class::Unsigned => {}
        }
    }
    totals
}

/// The lines a commit added, in all the files it changed.
fn added_by(commit: &Commit) -> u64 {
    commit.changes.iter().map(|change| change.added).sum()
}

/// What the commits that carry one signature add up to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignatureTotals {
    /// The signature's id in the table of [`crate::signatures`].
    pub id: &'static str,
    pub commits: u64,
    /// The lines those commits added. An estimate, as above.
    pub added: u64,
}

/// Sums the commits of each signature that decided a commit's class, in the
/// order of the table. Signatures that no commit carries are left out.
///
/// A commit that carries the marks of two agents counts for both, so these
/// sums can add up to more than the totals.
pub fn by_signature(history: &History) -> Vec<SignatureTotals> {
    let mut sums: HashMap<&'static str, (u64, u64)> = HashMap::new();
    for commit in &history.commits {
        let added = added_by(commit);
        for id in attribution::of(commit).signatures {
            let sum = sums.entry(id).or_default();
            sum.0 += 1;
            sum.1 += added;
        }
    }
    signatures::TABLE
        .iter()
        .filter_map(|signature| {
            let (commits, added) = *sums.get(signature.id)?;
            Some(SignatureTotals {
                id: signature.id,
                commits,
                added,
            })
        })
        .collect()
}

/// What history says about one file, under every name it had.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathHistory {
    /// The commits that changed the file. A commit that only changed a
    /// binary file counts too, and so does one that only renamed the file.
    pub commits: u64,
    /// How many of them are agent-attributed: at least this many.
    pub commits_agent: u64,
    /// Lines those commits added to the file and deleted from it.
    pub added: u64,
    /// The added lines that came in agent-attributed commits, and in
    /// commits made by bots. Both are estimates.
    pub added_agent: u64,
    pub added_automation: u64,
    pub deleted: u64,
    /// How many different author emails those commits have.
    pub authors: u64,
    /// The earliest and the latest author date among those commits, in
    /// seconds since 1970.
    pub first_at: i64,
    pub last_at: i64,
}

/// Sums the history of each file that any commit changed, keyed by the path
/// the file has today. For a file that was deleted, that is the last path
/// it had.
///
/// A renamed file keeps its history: what happened to it under an earlier
/// name counts for the name it has now.
///
/// How it works. The commits are read newest first, and `now` holds what
/// the renames passed so far say: for a name a file used to have, the name
/// the same file has today. A commit that renamed `a.rs` to `b.rs` means
/// that every older change to `a.rs` was a change to the file that is
/// `b.rs` today, or to whatever `b.rs` was renamed to after that.
///
/// What it cannot do:
///
/// - Git decides what a rename is: a deleted file and an added file that
///   are at least half alike (`DIFF_TREE` in [`crate::history`]). A file
///   that was renamed and mostly rewritten in one commit starts again as a
///   new file. Two files that swap their names in one commit are two
///   changed files to git, and each path keeps its own history.
/// - A file is known by its path. When a file is deleted and another one is
///   created at the same path later, the two share one history.
/// - It follows git's order of the commits, not the branches. A change made
///   under the old name, on a branch that was merged after the rename, can
///   come before the rename in that order, and then it stays with the old
///   name. In a history without merges this cannot happen.
pub fn by_path(history: &History) -> BTreeMap<&str, PathHistory> {
    struct Sum<'a> {
        history: PathHistory,
        authors: HashSet<&'a str>,
        /// The place in the history of the last commit counted here. One
        /// commit can change a file under two of its names, and it is still
        /// one commit.
        last_commit: Option<usize>,
    }

    let mut sums: HashMap<&str, Sum> = HashMap::new();
    // For a name a file used to have: the name the same file has today.
    let mut now: HashMap<&str, &str> = HashMap::new();

    for (index, commit) in history.commits.iter().enumerate() {
        let class = attribution::of(commit).class;
        // The renames this commit made: a file's old name, and the name the
        // file has today.
        let mut renames: Vec<(&str, &str)> = Vec::new();

        for change in &commit.changes {
            let path = now
                .get(change.path.as_str())
                .copied()
                .unwrap_or(change.path.as_str());
            if let Some(old) = &change.renamed_from {
                renames.push((old.as_str(), path));
            }

            let sum = sums.entry(path).or_insert_with(|| Sum {
                history: PathHistory {
                    commits: 0,
                    commits_agent: 0,
                    added: 0,
                    added_agent: 0,
                    added_automation: 0,
                    deleted: 0,
                    authors: 0,
                    first_at: commit.author_time,
                    last_at: commit.author_time,
                },
                authors: HashSet::new(),
                last_commit: None,
            });
            if sum.last_commit != Some(index) {
                sum.last_commit = Some(index);
                sum.history.commits += 1;
                if class == Class::AgentAttributed {
                    sum.history.commits_agent += 1;
                }
            }
            sum.history.added += change.added;
            sum.history.deleted += change.deleted;
            match class {
                Class::AgentAttributed => sum.history.added_agent += change.added,
                Class::Automation => sum.history.added_automation += change.added,
                Class::Unsigned => {}
            }
            sum.history.first_at = sum.history.first_at.min(commit.author_time);
            sum.history.last_at = sum.history.last_at.max(commit.author_time);
            sum.authors.insert(commit.author_email.as_str());
        }

        // Every commit from here on is older than these renames. Before
        // them a file did not have its new name, so an older change to that
        // name was a change to some other file: forget what `now` says
        // about the name. Then note each old name. Forgetting comes first
        // for all of them, so that the result does not depend on the order
        // of the changes within a commit. That would matter where one path
        // is an old name and a new name in the same commit. Git, as the
        // scan calls it, reports no such commit: it shows two files that
        // swap their names as two changed files.
        for change in &commit.changes {
            if change.renamed_from.is_some() {
                now.remove(change.path.as_str());
            }
        }
        for (old, path) in renames {
            now.insert(old, path);
        }
    }

    // A sorted map, so that walking it gives the same order on every run.
    sums.into_iter()
        .map(|(path, sum)| {
            let history = PathHistory {
                authors: sum.authors.len() as u64,
                ..sum.history
            };
            (path, history)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{Change, Commit};

    fn commit(parents: usize, email: &str, time: i64, changes: Vec<Change>) -> Commit {
        Commit {
            id: "0".repeat(40),
            parents,
            author_time: time,
            author_name: "Someone".to_owned(),
            author_email: email.to_owned(),
            message: "A change\n".to_owned(),
            changes,
        }
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

    /// A history of commits that each have one parent, newest first, one
    /// second apart. Each is given as its author and what it changed.
    fn line(commits: Vec<(&str, Vec<Change>)>) -> History {
        let newest = commits.len() as i64;
        History {
            commits: commits
                .into_iter()
                .enumerate()
                .map(|(index, (email, changes))| {
                    commit(1, email, 1_000 + newest - index as i64, changes)
                })
                .collect(),
            lossy_paths: false,
        }
    }

    /// The commits, the added lines and the deleted lines of each path.
    fn numbers(history: &History) -> Vec<(&str, u64, u64, u64)> {
        by_path(history)
            .into_iter()
            .map(|(path, sum)| (path, sum.commits, sum.added, sum.deleted))
            .collect()
    }

    /// Newest first, as git lists commits. The dates are deliberately out
    /// of step with that order, as they are after a rebase.
    fn history() -> History {
        History {
            commits: vec![
                commit(
                    1,
                    "ada@example.com",
                    500,
                    vec![renamed("old.rs", "new.rs", 2, 1), change("lib.rs", 10, 0)],
                ),
                commit(2, "ada@example.com", 400, vec![]),
                commit(
                    1,
                    "grace@example.com",
                    900,
                    vec![
                        change("lib.rs", 4, 3),
                        Change {
                            binary: true,
                            ..change("logo.png", 0, 0)
                        },
                    ],
                ),
                commit(
                    1,
                    "ada@example.com",
                    100,
                    vec![change("lib.rs", 7, 0), change("old.rs", 5, 0)],
                ),
                commit(0, "ada@example.com", 50, vec![]),
            ],
            lossy_paths: false,
        }
    }

    #[test]
    fn totals_count_every_commit_and_every_changed_line() {
        assert_eq!(
            totals(&history()),
            Totals {
                commits: 5,
                merges: 1,
                commits_agent: 0,
                commits_automation: 0,
                added: 28,
                added_agent: 0,
                added_automation: 0,
                deleted: 4,
            }
        );
    }

    #[test]
    fn an_empty_history_adds_up_to_nothing() {
        let empty = History {
            commits: vec![],
            lossy_paths: false,
        };
        assert_eq!(
            totals(&empty),
            Totals {
                commits: 0,
                merges: 0,
                commits_agent: 0,
                commits_automation: 0,
                added: 0,
                added_agent: 0,
                added_automation: 0,
                deleted: 0,
            }
        );
        assert!(by_path(&empty).is_empty());
        assert!(by_signature(&empty).is_empty());
    }

    #[test]
    fn each_path_gets_its_commits_lines_authors_and_dates() {
        let history = history();
        let paths = by_path(&history);
        assert_eq!(
            paths.keys().copied().collect::<Vec<_>>(),
            ["lib.rs", "logo.png", "new.rs"],
            "sorted by path, and a renamed file once, under the name it has now"
        );
        assert_eq!(
            paths["lib.rs"],
            PathHistory {
                commits: 3,
                commits_agent: 0,
                added: 21,
                added_agent: 0,
                added_automation: 0,
                deleted: 3,
                authors: 2,
                // The earliest and the latest date, wherever they are in
                // the order of the commits.
                first_at: 100,
                last_at: 900,
            }
        );
        // A binary file has commits and no lines.
        assert_eq!(
            paths["logo.png"],
            PathHistory {
                commits: 1,
                commits_agent: 0,
                added: 0,
                added_agent: 0,
                added_automation: 0,
                deleted: 0,
                authors: 1,
                first_at: 900,
                last_at: 900,
            }
        );
    }

    const ADA: &str = "ada@example.com";
    const GRACE: &str = "grace@example.com";
    const LIN: &str = "lin@example.com";

    /// The newest commit renamed `old.rs` to `new.rs`. What was done to
    /// `old.rs` before that is part of the history of `new.rs`.
    #[test]
    fn a_renamed_file_keeps_its_history() {
        let history = history();
        let paths = by_path(&history);
        assert_eq!(
            paths["new.rs"],
            PathHistory {
                // The commit that wrote it as `old.rs`, and the rename.
                commits: 2,
                commits_agent: 0,
                added: 7,
                added_agent: 0,
                added_automation: 0,
                deleted: 1,
                authors: 1,
                first_at: 100,
                last_at: 500,
            }
        );
        assert!(!paths.contains_key("old.rs"));
    }

    #[test]
    fn a_file_renamed_twice_keeps_all_of_it() {
        let history = line(vec![
            (ADA, vec![change("c.rs", 1, 0)]),
            (ADA, vec![renamed("b.rs", "c.rs", 2, 2)]),
            (GRACE, vec![change("b.rs", 4, 0)]),
            (ADA, vec![renamed("a.rs", "b.rs", 0, 0)]),
            (LIN, vec![change("a.rs", 8, 0)]),
        ]);
        assert_eq!(numbers(&history), [("c.rs", 5, 15, 2)]);
        let file = by_path(&history)["c.rs"];
        assert_eq!(file.authors, 3);
        assert_eq!((file.first_at, file.last_at), (1_001, 1_005));
    }

    /// Renamed away and back again: one file with one history.
    #[test]
    fn a_file_renamed_back_is_one_file() {
        let history = line(vec![
            (ADA, vec![renamed("b.rs", "a.rs", 0, 0)]),
            (ADA, vec![change("b.rs", 3, 1)]),
            (ADA, vec![renamed("a.rs", "b.rs", 0, 0)]),
            (ADA, vec![change("a.rs", 6, 0)]),
        ]);
        assert_eq!(numbers(&history), [("a.rs", 4, 9, 1)]);
    }

    /// `a.rs` became `b.rs`, and later someone wrote a new `a.rs`. The new
    /// file has its own history, and the old one's went with `b.rs`.
    #[test]
    fn a_new_file_under_an_old_name_has_its_own_history() {
        let history = line(vec![
            (ADA, vec![change("a.rs", 10, 0)]),
            (ADA, vec![renamed("a.rs", "b.rs", 0, 0)]),
            (ADA, vec![change("a.rs", 3, 0)]),
            (ADA, vec![change("a.rs", 5, 0)]),
        ]);
        assert_eq!(numbers(&history), [("a.rs", 1, 10, 0), ("b.rs", 3, 8, 0)]);
    }

    /// `old.rs` became `new.rs`, and that became `last.rs`. Long before,
    /// another file was called `new.rs`, and it was deleted. What happened
    /// to that file is not part of the history of `last.rs`.
    #[test]
    fn an_earlier_file_with_a_name_from_the_middle_keeps_its_own_history() {
        let history = line(vec![
            (ADA, vec![renamed("new.rs", "last.rs", 0, 0)]),
            (ADA, vec![renamed("old.rs", "new.rs", 0, 0)]),
            (ADA, vec![change("old.rs", 7, 0)]),
            (GRACE, vec![change("new.rs", 0, 3)]),
            (GRACE, vec![change("new.rs", 3, 0)]),
        ]);
        assert_eq!(
            numbers(&history),
            [("last.rs", 3, 7, 0), ("new.rs", 2, 3, 3)]
        );
    }

    /// Git shows two files that swap their names as two changed files, so a
    /// scan does not meet this. If a history did hold the two renames, each
    /// file would keep its own history, whatever the order of the two.
    #[test]
    fn two_renames_that_swap_names_keep_each_files_history() {
        let history = line(vec![
            (
                ADA,
                vec![renamed("b.rs", "a.rs", 0, 0), renamed("a.rs", "b.rs", 0, 0)],
            ),
            (ADA, vec![change("a.rs", 1, 0), change("b.rs", 20, 0)]),
        ]);
        // What was `a.rs` is `b.rs` today, and the other way round.
        assert_eq!(numbers(&history), [("a.rs", 2, 20, 0), ("b.rs", 2, 1, 0)]);
    }

    /// The same for a commit that moves `b.rs` to `c.rs`, and `a.rs` into
    /// its place.
    #[test]
    fn one_commit_can_move_a_file_into_the_place_of_another() {
        let history = line(vec![
            (
                ADA,
                vec![renamed("a.rs", "b.rs", 0, 0), renamed("b.rs", "c.rs", 0, 0)],
            ),
            (ADA, vec![change("a.rs", 1, 0), change("b.rs", 20, 0)]),
        ]);
        assert_eq!(numbers(&history), [("b.rs", 2, 1, 0), ("c.rs", 2, 20, 0)]);
    }

    /// A draft is written beside a file, the file is deleted, and the draft
    /// takes its name. A file is known by its path, so the two share one
    /// history. The oldest commit changed both of them, and it counts once.
    #[test]
    fn a_commit_that_changed_a_file_under_two_names_counts_once() {
        let history = line(vec![
            (ADA, vec![renamed("draft.rs", "final.rs", 0, 0)]),
            (ADA, vec![change("final.rs", 0, 4)]),
            (
                GRACE,
                vec![change("draft.rs", 3, 0), change("final.rs", 4, 0)],
            ),
        ]);
        assert_eq!(numbers(&history), [("final.rs", 3, 7, 4)]);
        assert_eq!(by_path(&history)["final.rs"].authors, 2);
    }

    #[test]
    fn a_file_renamed_and_then_deleted_is_under_its_last_name() {
        let history = line(vec![
            (ADA, vec![change("b.rs", 0, 5)]),
            (ADA, vec![renamed("a.rs", "b.rs", 0, 0)]),
            (ADA, vec![change("a.rs", 5, 0)]),
        ]);
        assert_eq!(numbers(&history), [("b.rs", 3, 5, 5)]);
    }

    /// The limit of following git's order. A branch changed `old.rs`, the
    /// main line renamed the file to `new.rs`, and then the branch was
    /// merged. Git lists the merge, then the branch, then the main line, so
    /// the branch's change comes before the rename and stays with the old
    /// name.
    #[test]
    fn a_change_on_a_branch_that_comes_before_the_rename_is_not_carried() {
        let history = History {
            commits: vec![
                commit(2, ADA, 400, vec![]),
                commit(1, GRACE, 300, vec![change("old.rs", 4, 0)]),
                commit(1, ADA, 200, vec![renamed("old.rs", "new.rs", 0, 0)]),
                commit(0, ADA, 100, vec![change("old.rs", 5, 0)]),
            ],
            lossy_paths: false,
        };
        assert_eq!(
            numbers(&history),
            [("new.rs", 2, 5, 0), ("old.rs", 1, 4, 0)]
        );
    }

    /// Two branches gave one file two names, and the merge kept both files.
    /// The older commits go to the name whose rename is nearest to them in
    /// git's order, which is the last one met.
    #[test]
    fn when_two_branches_rename_one_file_the_nearest_rename_decides() {
        let history = History {
            commits: vec![
                commit(2, ADA, 400, vec![]),
                commit(1, GRACE, 300, vec![renamed("old.rs", "theirs.rs", 0, 0)]),
                commit(1, ADA, 200, vec![renamed("old.rs", "ours.rs", 0, 0)]),
                commit(0, ADA, 100, vec![change("old.rs", 5, 0)]),
            ],
            lossy_paths: false,
        };
        assert_eq!(
            numbers(&history),
            [("ours.rs", 2, 5, 0), ("theirs.rs", 1, 0, 0)]
        );
    }

    #[test]
    fn only_a_renamed_file_depends_on_the_order_of_the_commits() {
        let forwards = signed();
        let mut backwards = signed();
        backwards.commits.reverse();
        assert_eq!(totals(&forwards), totals(&backwards));
        assert_eq!(by_signature(&forwards), by_signature(&backwards));

        // A file's history has to meet a rename before what came before it,
        // which is the order `History` promises. For a file that was never
        // renamed the order makes no difference.
        let (forwards, backwards) = (by_path(&forwards), by_path(&backwards));
        for path in ["lib.rs", "logo.png"] {
            assert_eq!(forwards[path], backwards[path], "{path}");
        }
    }

    /// The history above with marks on three of its commits: an agent's
    /// trailer on the newest, a second agent's on the merge, and a bot as
    /// the author of the one that touched the logo.
    fn signed() -> History {
        let mut history = history();
        history.commits[0].message =
            "A change\n\nCo-authored-by: Claude <noreply@anthropic.com>\n".to_owned();
        history.commits[1].message = "Merge\n\nCo-authored-by: Claude <noreply@anthropic.com>\n\
             Co-authored-by: Cursor <cursoragent@cursor.com>\n"
            .to_owned();
        history.commits[2].author_name = "dependabot[bot]".to_owned();
        history
    }

    #[test]
    fn a_commits_lines_all_go_to_its_class() {
        assert_eq!(
            totals(&signed()),
            Totals {
                commits: 5,
                merges: 1,
                // The newest commit and the merge.
                commits_agent: 2,
                commits_automation: 1,
                added: 28,
                // The newest commit added 2 and 10. The merge added none.
                added_agent: 12,
                added_automation: 4,
                deleted: 4,
            }
        );
    }

    #[test]
    fn each_signature_gets_the_commits_that_carry_it() {
        assert_eq!(
            by_signature(&signed()),
            [
                // Two commits carry the first agent's mark and one of them
                // also carries the second's, so the commits here add up to
                // more than the two that are agent-attributed.
                SignatureTotals {
                    id: "claude-code",
                    commits: 2,
                    added: 12,
                },
                SignatureTotals {
                    id: "cursor",
                    commits: 1,
                    added: 0,
                },
                SignatureTotals {
                    id: "bot",
                    commits: 1,
                    added: 4,
                },
            ],
            "in the order of the table"
        );
    }

    #[test]
    fn each_path_gets_its_share_of_each_class() {
        let history = signed();
        let paths = by_path(&history);
        let lib = paths["lib.rs"];
        assert_eq!((lib.commits, lib.commits_agent), (3, 1));
        assert_eq!(
            (lib.added, lib.added_agent, lib.added_automation),
            (21, 10, 4)
        );
        // The bot's commit changed the logo and added no line to it.
        let logo = paths["logo.png"];
        assert_eq!((logo.commits, logo.commits_agent), (1, 0));
        assert_eq!((logo.added_agent, logo.added_automation), (0, 0));
        // The agent's commit renamed this file and added two lines to it.
        // The five lines from before the rename are part of its history,
        // and they are not the agent's.
        let new = paths["new.rs"];
        assert_eq!((new.commits, new.commits_agent), (2, 1));
        assert_eq!(
            (new.added, new.added_agent, new.added_automation),
            (7, 2, 0)
        );
    }
}
