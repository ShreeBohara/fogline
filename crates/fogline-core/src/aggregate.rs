//! Sums over a history (spec section 5.2): the totals of the repository,
//! what history says about each path, and what each agent signature adds up
//! to.
//!
//! All of them are plain sums over the commits, so they do not depend on
//! the order of the commits or on how the work of reading them was divided.
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

/// What history says about one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathHistory {
    /// The commits that changed the file. A commit that only changed a
    /// binary file counts too.
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

/// Sums the history of each path that any commit changed, keyed by path.
///
/// A renamed file is counted under the path it had at the time of each
/// commit: the commit that renames it counts for the new path, and what
/// happened before stays with the old one. Carrying that history over to
/// the new path is task 3.5 of the Phase 1 spec.
pub fn by_path(history: &History) -> BTreeMap<&str, PathHistory> {
    struct Sum<'a> {
        history: PathHistory,
        authors: HashSet<&'a str>,
    }

    let mut sums: HashMap<&str, Sum> = HashMap::new();
    for commit in &history.commits {
        let class = attribution::of(commit).class;
        for change in &commit.changes {
            let sum = sums.entry(change.path.as_str()).or_insert_with(|| Sum {
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
            });
            sum.history.commits += 1;
            sum.history.added += change.added;
            sum.history.deleted += change.deleted;
            match class {
                Class::AgentAttributed => {
                    sum.history.commits_agent += 1;
                    sum.history.added_agent += change.added;
                }
                Class::Automation => sum.history.added_automation += change.added,
                Class::Unsigned => {}
            }
            sum.history.first_at = sum.history.first_at.min(commit.author_time);
            sum.history.last_at = sum.history.last_at.max(commit.author_time);
            sum.authors.insert(commit.author_email.as_str());
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

    /// Newest first, as git lists commits. The dates are deliberately out
    /// of step with that order, as they are after a rebase.
    fn history() -> History {
        History {
            commits: vec![
                commit(
                    1,
                    "ada@example.com",
                    500,
                    vec![
                        Change {
                            renamed_from: Some("old.rs".to_owned()),
                            ..change("new.rs", 2, 1)
                        },
                        change("lib.rs", 10, 0),
                    ],
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
            ["lib.rs", "logo.png", "new.rs", "old.rs"],
            "sorted by path"
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

    /// Until task 3.5, a rename splits a file's history in two.
    #[test]
    fn a_renamed_file_is_counted_under_the_path_of_the_time() {
        let history = history();
        let paths = by_path(&history);
        assert_eq!((paths["new.rs"].commits, paths["new.rs"].added), (1, 2));
        assert_eq!((paths["old.rs"].commits, paths["old.rs"].added), (1, 5));
    }

    #[test]
    fn the_sums_do_not_depend_on_the_order_of_the_commits() {
        let forwards = signed();
        let mut backwards = signed();
        backwards.commits.reverse();
        assert_eq!(totals(&forwards), totals(&backwards));
        assert_eq!(by_path(&forwards), by_path(&backwards));
        assert_eq!(by_signature(&forwards), by_signature(&backwards));
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
        // A file no marked commit touched.
        let old = paths["old.rs"];
        assert_eq!(
            (old.commits_agent, old.added_agent, old.added_automation),
            (0, 0, 0)
        );
    }
}
