//! The caveats of a scan (spec section 5.2): what a reader has to know to
//! read its numbers right.
//!
//! Every caveat is defined here once, with the id that the bundle's
//! manifest lists and the sentence that the viewer shows for it.
//! [`that_apply`] is the one place that decides which of them a scan
//! carries. The command line says some of them out loud in its summary, and
//! it asks here which.

use crate::files::Files;
use crate::history::{Commit, History};
use crate::repo::Repository;

/// One caveat.
///
/// A new one is added in five places: here, in [`Caveat::ALL`], in
/// [`Caveat::id`], in [`Caveat::text`] and in [`that_apply`]. The compiler
/// asks for the last three, and a test compares `ALL` with the list in the
/// spec. The command line then has to say whether its summary mentions the
/// new one, and the compiler asks for that too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caveat {
    /// A scan reads one commit. What is in the work tree and not committed
    /// is not in it.
    HeadOnly,
    /// All the lines a commit adds go to the commit's class, whoever wrote
    /// them.
    CommitLevelAttribution,
    /// An agent that leaves no mark on a commit cannot be seen.
    SignaturesUndercount,
    /// The numbers count lines as they were added, not the lines that are
    /// still there.
    AddedNotSurviving,
    /// A merge commit carries no line statistics, so what was changed while
    /// resolving a merge is counted nowhere. A file that a merge commit
    /// added has no history at all.
    MergesHaveNoStats,
    /// A file's history follows it through the renames git detects, and
    /// through no others (`aggregate::by_path` has the details).
    RenamesBySimilarity,
    /// A shallow clone holds only the newest part of the history.
    ShallowClone,
    /// A path that is not valid UTF-8 was converted, and is no longer the
    /// exact path in the repository.
    LossyPaths,
    /// An old git reads `.gitattributes` from the work tree and not from
    /// the commit, so an uncommitted edit can change which files are binary.
    AttributesFromWorkTree,
    /// The scan was told not to read history.
    HistoryOff,
}

impl Caveat {
    /// Every caveat, in the order of the table in spec section 5.2. A
    /// scan's caveats are listed in this order too.
    pub const ALL: [Caveat; 10] = [
        Caveat::HeadOnly,
        Caveat::CommitLevelAttribution,
        Caveat::SignaturesUndercount,
        Caveat::AddedNotSurviving,
        Caveat::MergesHaveNoStats,
        Caveat::RenamesBySimilarity,
        Caveat::ShallowClone,
        Caveat::LossyPaths,
        Caveat::AttributesFromWorkTree,
        Caveat::HistoryOff,
    ];

    /// The name the bundle's manifest lists the caveat under. Bundles that
    /// were already written hold these, so one that is in use never changes.
    pub const fn id(self) -> &'static str {
        match self {
            Caveat::HeadOnly => "head_only",
            Caveat::CommitLevelAttribution => "commit_level_attribution",
            Caveat::SignaturesUndercount => "signatures_undercount",
            Caveat::AddedNotSurviving => "added_not_surviving",
            Caveat::MergesHaveNoStats => "merges_have_no_stats",
            Caveat::RenamesBySimilarity => "renames_by_similarity",
            Caveat::ShallowClone => "shallow_clone",
            Caveat::LossyPaths => "lossy_paths",
            Caveat::AttributesFromWorkTree => "attributes_from_work_tree",
            Caveat::HistoryOff => "history_off",
        }
    }

    /// The sentence a person reads. It follows the wording rule of the
    /// spec: "agent-attributed", and counts of commits as "at least".
    pub const fn text(self) -> &'static str {
        match self {
            Caveat::HeadOnly => {
                "Shows the committed state at this commit. Uncommitted changes are not included."
            }
            Caveat::CommitLevelAttribution => {
                "A commit with an agent signature counts all of its added lines as \
                 agent-attributed, including any a person wrote."
            }
            Caveat::SignaturesUndercount => {
                "Agents that leave no signature are invisible here. Read commit counts as \
                 \"at least\"."
            }
            Caveat::AddedNotSurviving => {
                "Shares count lines added over history, not the lines that survive today. \
                 Line-level tracking comes in a later version."
            }
            Caveat::MergesHaveNoStats => {
                "Changes made while resolving merges are not counted. A file that a merge \
                 commit added has no history."
            }
            Caveat::RenamesBySimilarity => {
                "A file keeps its history through a rename that git detects, which it does by \
                 similarity. A file that was renamed and heavily edited in one commit counts as \
                 a new file. A change made under a file's old name, on a branch that was merged \
                 after the rename, can be missing from the file's history."
            }
            Caveat::ShallowClone => "This clone holds only part of the history.",
            Caveat::LossyPaths => "Some paths are not valid UTF-8 and were converted.",
            Caveat::AttributesFromWorkTree => {
                "This git is older than 2.41, so which files count as binary follows the \
                 .gitattributes in the work tree, not the one in the commit."
            }
            Caveat::HistoryOff => "History was not read (--history none).",
        }
    }
}

/// The caveats of one scan, in the order of [`Caveat::ALL`]. `history` is
/// `None` when the scan was told not to read it.
pub fn that_apply(found: &Repository, files: &Files, history: Option<&History>) -> Vec<Caveat> {
    let applies = |caveat: &Caveat| match caveat {
        Caveat::HeadOnly => true,
        // What every scan that reads history has to say about its numbers.
        Caveat::CommitLevelAttribution
        | Caveat::SignaturesUndercount
        | Caveat::AddedNotSurviving
        | Caveat::RenamesBySimilarity => history.is_some(),
        Caveat::MergesHaveNoStats => {
            history.is_some_and(|history| history.commits.iter().any(Commit::is_merge))
        }
        Caveat::ShallowClone => found.shallow,
        // A path among the files at HEAD, or one that only history knows.
        Caveat::LossyPaths => {
            files.lossy_paths || history.is_some_and(|history| history.lossy_paths)
        }
        Caveat::AttributesFromWorkTree => !files.attributes_from_commit,
        Caveat::HistoryOff => history.is_none(),
    };
    Caveat::ALL.into_iter().filter(applies).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;

    use super::*;
    use crate::git::Version;
    use crate::repo::ObjectFormat;

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

    fn files() -> Files {
        Files {
            entries: vec![],
            submodules_skipped: 0,
            lossy_paths: false,
            attributes_from_commit: true,
        }
    }

    /// A history of commits with these numbers of parents, newest first.
    fn history(parents: &[usize]) -> History {
        History {
            commits: parents
                .iter()
                .map(|&parents| Commit {
                    id: "0".repeat(40),
                    parents,
                    author_time: 1_770_000_000,
                    author_name: "Ada Example".to_owned(),
                    author_email: "ada@example.com".to_owned(),
                    message: "A change\n".to_owned(),
                    changes: vec![],
                })
                .collect(),
            lossy_paths: false,
        }
    }

    fn ids(caveats: &[Caveat]) -> Vec<&'static str> {
        caveats.iter().map(|caveat| caveat.id()).collect()
    }

    /// A bundle records these ids, in this order, so a change here is a
    /// change to the bundle's format.
    #[test]
    fn the_ids_are_the_ones_a_bundle_records() {
        assert_eq!(
            ids(&Caveat::ALL),
            [
                "head_only",
                "commit_level_attribution",
                "signatures_undercount",
                "added_not_surviving",
                "merges_have_no_stats",
                "renames_by_similarity",
                "shallow_clone",
                "lossy_paths",
                "attributes_from_work_tree",
                "history_off",
            ]
        );
        let distinct: HashSet<&str> = ids(&Caveat::ALL).into_iter().collect();
        assert_eq!(distinct.len(), Caveat::ALL.len());
    }

    /// The table in spec section 5.2 is what people read, and this file is
    /// what the program says. Every row of the table is a caveat here, with
    /// the same id and the same sentence, in the same order.
    #[test]
    fn the_spec_has_the_same_caveats_with_the_same_sentences() {
        let spec = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/specs/phase-1.md"
        ))
        .expect("the spec can be read");
        let rows: Vec<(String, String)> = spec
            .lines()
            .skip_while(|line| !line.starts_with("| id | Shown as |"))
            // The heading of the table, and the line under it.
            .skip(2)
            .take_while(|line| line.starts_with('|'))
            .map(|line| {
                let mut cells = line.split('|').skip(1).map(str::trim);
                let mut cell = || cells.next().expect("a row has an id and a sentence");
                // The spec marks code with backticks. The sentences have none.
                (cell().replace('`', ""), cell().replace('`', ""))
            })
            .collect();
        let here: Vec<(String, String)> = Caveat::ALL
            .iter()
            .map(|caveat| (caveat.id().to_owned(), caveat.text().to_owned()))
            .collect();
        assert_eq!(rows, here);
    }

    #[test]
    fn every_caveat_has_a_sentence_that_follows_the_wording_rule() {
        for caveat in Caveat::ALL {
            let text = caveat.text();
            assert!(text.ends_with('.'), "{text}");
            // No line of the source was folded into a gap in the sentence.
            assert!(!text.contains("  "), "{text}");
            // Spec section 5.2: "agent-attributed", never "AI-written".
            assert!(!text.contains("AI"), "{text}");
            assert!(!text.to_lowercase().contains("written by"), "{text}");
        }
        // The sentence about an old git names the version the engine tests
        // for.
        let names = |caveat: Caveat, version: Version| {
            caveat
                .text()
                .contains(&format!("older than {}.{},", version.major, version.minor))
        };
        assert!(names(
            Caveat::AttributesFromWorkTree,
            Version::ATTRIBUTES_FROM_COMMIT
        ));
    }

    #[test]
    fn a_scan_with_history_carries_the_caveats_about_history() {
        assert_eq!(
            ids(&that_apply(
                &repository(),
                &files(),
                Some(&history(&[1, 0]))
            )),
            [
                "head_only",
                "commit_level_attribution",
                "signatures_undercount",
                "added_not_surviving",
                "renames_by_similarity",
            ]
        );
    }

    #[test]
    fn merges_are_a_caveat_when_the_history_has_one() {
        let with = that_apply(&repository(), &files(), Some(&history(&[2, 1, 1, 0])));
        assert_eq!(
            ids(&with),
            [
                "head_only",
                "commit_level_attribution",
                "signatures_undercount",
                "added_not_surviving",
                "merges_have_no_stats",
                "renames_by_similarity",
            ]
        );
    }

    #[test]
    fn a_scan_without_history_says_so_and_nothing_about_history() {
        assert_eq!(
            ids(&that_apply(&repository(), &files(), None)),
            ["head_only", "history_off"]
        );
    }

    #[test]
    fn a_shallow_clone_and_a_converted_path_are_caveats() {
        let mut found = repository();
        found.shallow = true;
        let mut listed = files();
        listed.lossy_paths = true;
        assert_eq!(
            ids(&that_apply(&found, &listed, None)),
            ["head_only", "shallow_clone", "lossy_paths", "history_off"]
        );

        // A converted path that only history knows counts too.
        let mut read = history(&[0]);
        read.lossy_paths = true;
        assert!(that_apply(&repository(), &files(), Some(&read)).contains(&Caveat::LossyPaths));
        read.lossy_paths = false;
        assert!(!that_apply(&repository(), &files(), Some(&read)).contains(&Caveat::LossyPaths));
    }

    #[test]
    fn an_old_git_is_a_caveat_for_what_it_cannot_do() {
        let mut found = repository();
        found.git = Version {
            major: 2,
            minor: 39,
            patch: 5,
        };
        // The scan found that this git read `.gitattributes` from the work
        // tree. That decides which files count as binary, so it matters
        // with history and without.
        let mut listed = files();
        listed.attributes_from_commit = false;
        for history in [Some(history(&[0])), None] {
            let caveats = that_apply(&found, &listed, history.as_ref());
            assert!(caveats.contains(&Caveat::AttributesFromWorkTree));
        }
        // The same git, when it read the commit's `.gitattributes`: the
        // caveat follows what the scan found, not the version number.
        assert!(!that_apply(&found, &files(), None).contains(&Caveat::AttributesFromWorkTree));
    }
}
