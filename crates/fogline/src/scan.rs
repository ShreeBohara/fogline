//! `fogline scan`. So far it finds the repository, lists its files with
//! their sizes and line counts, reads its history, and says how many
//! commits are agent-attributed. The bundle arrives with the rest of
//! milestone M3 (spec section 6).

use std::io::{self, Write};
use std::num::NonZero;
use std::path::{self, Path};
use std::thread;

use fogline_core::aggregate::{self, SignatureTotals, Totals};
use fogline_core::caveats::{self, Caveat};
use fogline_core::files::{self, Files};
use fogline_core::git::{self, Version};
use fogline_core::history::{self, History};
use fogline_core::repo::{self, Error, Repository};
use fogline_core::signatures::{self, Kind};

use crate::cli::HistoryChoice;
use crate::{EXIT_FAILURE, EXIT_GIT_UNUSABLE, EXIT_NOT_A_REPOSITORY, EXIT_USAGE, Failure};

/// Without `--jobs`, history is read by one git process for each processor
/// core, and by no more than this many (spec section 5.1).
const MOST_JOBS_BY_DEFAULT: usize = 12;

pub fn run(path: &Path, history: HistoryChoice, jobs: Option<usize>) -> Result<(), Failure> {
    let found = repo::discover(path).map_err(explain)?;
    let files = files::list(&found).map_err(|error| explain_listing(error, &found))?;
    let history = match history {
        HistoryChoice::All => {
            let jobs = jobs.unwrap_or_else(default_jobs);
            Some(history::read(&found, jobs).map_err(|error| explain_history(error, &found))?)
        }
        HistoryChoice::None => None,
    };

    // The summary is a message for a person, so it goes to stderr. stdout is
    // kept for data that was asked for (spec section 5.1, rule 2). If stderr
    // cannot be written to, there is nowhere left to say so.
    let _ = io::stderr()
        .lock()
        .write_all(summary(&found, &files, history.as_ref()).as_bytes());
    Ok(())
}

/// How many git processes read history when `--jobs` is not given. A system
/// that cannot say how many cores it has gets one.
fn default_jobs() -> usize {
    thread::available_parallelism()
        .map_or(1, NonZero::get)
        .min(MOST_JOBS_BY_DEFAULT)
}

/// What a scan found, as the lines a person reads. `history` is `None`
/// when the scan was told not to read it.
fn summary(found: &Repository, files: &Files, history: Option<&History>) -> String {
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
    match history {
        Some(history) => {
            let totals = aggregate::totals(history);
            let mut commits = count(totals.commits, "commit");
            if totals.merges > 0 {
                commits.push_str(&format!(" ({})", count(totals.merges, "merge")));
            }
            text.push_str(&format!(
                "  history   {commits}, {} added, {} deleted\n",
                count(totals.added, "line"),
                grouped(totals.deleted)
            ));
            text.push_str(&agents(&totals, &aggregate::by_signature(history)));
        }
        None => text.push_str("  history   not read (--history none)\n"),
    }

    // The caveats of this scan that are worth a line here (see `note`).
    for caveat in caveats::that_apply(found, files, history) {
        if let Some(note) = note(caveat, found) {
            text.push_str(&format!("  note      {note}\n"));
        }
    }
    text
}

/// What the summary says about a caveat of this scan, or `None` for one it
/// leaves to the viewer.
///
/// A caveat that comes from this clone or from this machine's git limits
/// what the numbers mean, and a person can do something about it, so it is
/// said out loud. The ones that hold for every scan would only repeat
/// themselves on every run.
fn note(caveat: Caveat, found: &Repository) -> Option<String> {
    match caveat {
        Caveat::ShallowClone => {
            Some("this is a shallow clone, so only part of the history is here".to_owned())
        }
        Caveat::LossyPaths => Some("some paths are not valid UTF-8 and were converted".to_owned()),
        Caveat::AttributesFromWorkTree => Some(format!(
            "git {} reads .gitattributes from the work tree, not from the commit; git {} or \
             newer reads the commit",
            found.git,
            Version::ATTRIBUTES_FROM_COMMIT,
        )),
        // The history line already says that history was not read.
        Caveat::HistoryOff => None,
        Caveat::HeadOnly
        | Caveat::CommitLevelAttribution
        | Caveat::SignaturesUndercount
        | Caveat::AddedNotSurviving
        | Caveat::MergesHaveNoStats
        | Caveat::RenamesBySimilarity => None,
    }
}

/// The lines about agents and bots (spec section 5.2).
///
/// The wording is deliberate. A commit count is a lower bound, because an
/// agent that leaves no mark cannot be seen, so it reads "at least". A
/// share of lines is an estimate, because all the lines of a marked commit
/// count, whoever wrote them, so it reads "an estimated".
fn agents(totals: &Totals, by_signature: &[SignatureTotals]) -> String {
    let mut text = String::new();
    if totals.commits_agent == 0 {
        text.push_str("  agents    no agent signatures found\n");
    } else {
        // The tools with the most commits first. The sort keeps the order
        // of the table among tools with the same number.
        let mut tools: Vec<(&str, u64)> = by_signature
            .iter()
            .filter_map(|sum| {
                let signature = signatures::by_id(sum.id)?;
                (signature.kind == Kind::Agent).then_some((signature.tool, sum.commits))
            })
            .collect();
        tools.sort_by_key(|&(_, commits)| std::cmp::Reverse(commits));
        let tools: Vec<String> = tools
            .iter()
            .map(|(tool, commits)| format!("{tool} {}", grouped(*commits)))
            .collect();
        text.push_str(&format!(
            "  agents    at least {} out of {} ({})",
            count(totals.commits_agent, "agent-attributed commit"),
            grouped(totals.commits),
            tools.join(", "),
        ));
        if totals.added > 0 {
            text.push_str(&format!(
                ", with an estimated {} of added lines",
                share(totals.added_agent, totals.added)
            ));
        }
        text.push('\n');
    }
    if totals.commits_automation > 0 {
        text.push_str(&format!(
            "  bots      {} by bots\n",
            count(totals.commits_automation, "commit")
        ));
    }
    text
}

/// A part of a whole as a percentage with no decimals. A part that is not
/// nothing never reads 0%, and a part that is not everything never reads
/// 100%. `whole` must not be 0.
fn share(part: u64, whole: u64) -> String {
    let rounded = (u128::from(part) * 100 + u128::from(whole) / 2) / u128::from(whole);
    if part > 0 && rounded == 0 {
        "under 1%".to_owned()
    } else if part < whole && rounded == 100 {
        "over 99%".to_owned()
    } else {
        format!("{rounded}%")
    }
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

/// The same for an error from reading the history.
fn explain_history(error: history::Error, found: &Repository) -> Failure {
    match error {
        history::Error::MissingObjects { missing } => Failure::new(
            EXIT_NOT_A_REPOSITORY,
            format!(
                "the repository at {} is a partial clone, and {} that its history needs {} not \
                 in it. Fogline never downloads anything. Use a full clone, or pass \
                 `--history none` to scan the files without their history.",
                shown(&found.root),
                count(missing, "object"),
                if missing == 1 { "is" } else { "are" },
            ),
        ),
        history::Error::Git(error) => explain(Error::Git(error)),
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
    use fogline_core::history::{Change, Commit};
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
            // A git that needs no note about itself.
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

    /// A commit with this many parents that changed one file.
    fn commit(parents: usize, added: u64, deleted: u64) -> Commit {
        Commit {
            id: "822e3536cbae5f3404efab40519ba1de1102e6a0".to_owned(),
            parents,
            author_time: 1_770_000_000,
            author_name: "Ada Example".to_owned(),
            author_email: "ada@example.com".to_owned(),
            message: "A change\n".to_owned(),
            changes: vec![Change {
                path: "a.rs".to_owned(),
                renamed_from: None,
                added,
                deleted,
                binary: false,
            }],
        }
    }

    fn history(commits: Vec<Commit>) -> History {
        History {
            commits,
            lossy_paths: false,
        }
    }

    #[test]
    fn a_partial_clone_without_its_history_is_a_reason_to_stop() {
        let failure = explain_history(history::Error::MissingObjects { missing: 3 }, &repository());
        assert_eq!(failure.code, EXIT_NOT_A_REPOSITORY);
        assert!(
            failure
                .message
                .contains("is a partial clone, and 3 objects that its history needs are not in it"),
            "{}",
            failure.message
        );
        assert!(failure.message.contains("never downloads anything"));
        assert!(failure.message.contains("`--history none`"));

        let one = explain_history(history::Error::MissingObjects { missing: 1 }, &repository());
        assert!(
            one.message
                .contains("1 object that its history needs is not in it")
        );
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
    fn a_plain_summary_has_a_header_the_files_and_the_history() {
        let files = files(vec![
            file("a.rs", 1_000, Flags::default()),
            file("b.rs", 234, Flags::default()),
        ]);
        let history = history(vec![commit(1, 1_200, 16), commit(0, 50, 0)]);
        let expected = format!(
            "fogline {} \u{b7} demo @ 822e353 (main)\n\
             \x20 files     2 (1,234 lines)\n\
             \x20 history   2 commits, 1,250 lines added, 16 deleted\n\
             \x20 agents    no agent signatures found\n",
            env!("CARGO_PKG_VERSION")
        );
        assert_eq!(summary(&repository(), &files, Some(&history)), expected);
    }

    /// A commit by this author, with this message, that added this many
    /// lines.
    fn signed(added: u64, author: &str, message: &str) -> Commit {
        Commit {
            author_name: author.to_owned(),
            message: message.to_owned(),
            ..commit(1, added, 0)
        }
    }

    const CLAUDE: &str = "Co-authored-by: Claude <noreply@anthropic.com>";
    const CURSOR: &str = "Co-authored-by: Cursor <cursoragent@cursor.com>";

    #[test]
    fn the_agents_line_says_at_least_and_estimated() {
        let history = history(vec![
            signed(60, "Ada Example", &format!("Fix\n\n{CLAUDE}\n")),
            // Two agents on one commit: it counts once, and each tool
            // counts it.
            signed(1, "Ada Example", &format!("Fix\n\n{CURSOR}\n{CLAUDE}\n")),
            signed(9, "dependabot[bot]", "Bump a version\n"),
            commit(0, 30, 0),
        ]);
        let text = summary(&repository(), &files(vec![]), Some(&history));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[3],
            "  agents    at least 2 agent-attributed commits out of 4 (Claude Code 2, Cursor 1), \
             with an estimated 61% of added lines"
        );
        assert_eq!(lines[4], "  bots      1 commit by bots");
        assert_eq!(lines.len(), 5, "{text}");
    }

    #[test]
    fn the_tools_with_the_most_commits_come_first() {
        let history = history(vec![
            signed(1, "Ada Example", &format!("Fix\n\n{CURSOR}\n")),
            signed(1, "Ada Example", &format!("Fix\n\n{CURSOR}\n")),
            signed(1, "Ada Example", &format!("Fix\n\n{CLAUDE}\n")),
            signed(1, "devin-ai-integration[bot]", "Fix\n"),
        ]);
        let text = summary(&repository(), &files(vec![]), Some(&history));
        // Claude Code comes before Devin in the table, and stays before it.
        assert!(
            text.contains("out of 4 (Cursor 2, Claude Code 1, Devin 1), with"),
            "{text}"
        );
    }

    #[test]
    fn the_agents_line_leaves_out_a_share_of_nothing() {
        // Agent-attributed commits that added no line, such as merges.
        let history = history(vec![signed(
            0,
            "Ada Example",
            &format!("Merge\n\n{CLAUDE}\n"),
        )]);
        let text = summary(&repository(), &files(vec![]), Some(&history));
        assert!(
            text.ends_with(
                "  agents    at least 1 agent-attributed commit out of 1 (Claude Code 1)\n"
            ),
            "{text}"
        );
    }

    #[test]
    fn a_share_is_never_rounded_to_nothing_or_to_everything() {
        let cases = [
            (0, 10, "0%"),
            (10, 10, "100%"),
            (1, 2, "50%"),
            (1, 3, "33%"),
            (2, 3, "67%"),
            (61, 100, "61%"),
            (1, 1_000, "under 1%"),
            (999, 1_000, "over 99%"),
            (u64::MAX - 1, u64::MAX, "over 99%"),
            (u64::MAX, u64::MAX, "100%"),
        ];
        for (part, whole, expected) in cases {
            assert_eq!(share(part, whole), expected, "{part} of {whole}");
        }
    }

    #[test]
    fn the_history_line_counts_merges_and_agrees_with_its_nouns() {
        let files = files(vec![]);
        let line = |history: &History| {
            summary(&repository(), &files, Some(history))
                .lines()
                .nth(2)
                .unwrap_or_default()
                .to_owned()
        };
        assert_eq!(
            line(&history(vec![
                commit(2, 0, 0),
                commit(1, 1, 1),
                commit(0, 0, 0)
            ])),
            "  history   3 commits (1 merge), 1 line added, 1 deleted"
        );
        assert_eq!(
            line(&history(vec![
                commit(3, 0, 0),
                commit(2, 0, 0),
                commit(0, 7, 0)
            ])),
            "  history   3 commits (2 merges), 7 lines added, 0 deleted"
        );
        assert_eq!(
            line(&history(vec![commit(0, 2, 0)])),
            "  history   1 commit, 2 lines added, 0 deleted"
        );
    }

    #[test]
    fn a_summary_says_when_history_was_not_read() {
        let files = files(vec![file("a.rs", 3, Flags::default())]);
        let text = summary(&repository(), &files, None);
        assert!(
            text.ends_with("  history   not read (--history none)\n"),
            "{text}"
        );
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

        let text = summary(&found, &files, None);
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

    /// Git's default diff, which counts the lines of history, counts the
    /// same on every git the engine supports (spec section 5.2). So reading
    /// history adds no note about the git, however old it is.
    #[test]
    fn reading_history_adds_no_note_about_an_old_git() {
        let files = files(vec![]);
        let history = history(vec![commit(0, 1, 0)]);
        let mut found = repository();
        found.git = Version::MINIMUM;
        let text = summary(&found, &files, Some(&history));
        assert!(!text.contains("  note      "), "{text}");
    }

    /// A history with a merge carries six caveats, and none of them is
    /// about this clone or this machine's git. They hold for every such
    /// scan, so the summary leaves them to the viewer.
    #[test]
    fn the_caveats_of_every_scan_are_not_notes() {
        let files = files(vec![]);
        let history = history(vec![commit(2, 0, 0), commit(1, 1, 0), commit(0, 1, 0)]);
        assert_eq!(
            caveats::that_apply(&repository(), &files, Some(&history)).len(),
            6
        );
        let text = summary(&repository(), &files, Some(&history));
        assert!(!text.contains("  note      "), "{text}");
    }

    /// A path that had to be converted is reported once, whether it was
    /// found among the files at HEAD or only in history.
    #[test]
    fn a_converted_path_in_history_is_reported_too() {
        let files = files(vec![]);
        let mut history = history(vec![commit(0, 1, 0)]);
        assert!(!summary(&repository(), &files, Some(&history)).contains("not valid UTF-8"));
        history.lossy_paths = true;
        let text = summary(&repository(), &files, Some(&history));
        assert_eq!(text.matches("not valid UTF-8").count(), 1, "{text}");
    }

    #[test]
    fn the_default_number_of_jobs_is_between_1_and_12() {
        assert!((1..=MOST_JOBS_BY_DEFAULT).contains(&default_jobs()));
    }
}
