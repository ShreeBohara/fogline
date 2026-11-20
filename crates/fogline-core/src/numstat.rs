//! Reading what `git diff-tree --stdin --always --numstat -z` prints (spec
//! section 5.3): for each commit, the lines it added to and deleted from
//! each file.
//!
//! The output is a run of pieces, and each piece ends with a NUL byte:
//!
//! - `<commit id>`: the changes of that commit follow;
//! - `<added>` TAB `<deleted>` TAB `<path>`: one changed file;
//! - `<added>` TAB `<deleted>` TAB with no path, then `<old path>` and
//!   `<new path>` as two more pieces: a file that was renamed;
//! - `-` in place of both numbers: a file git treats as binary.
//!
//! A commit that changed nothing prints its id and nothing else.

use std::borrow::Cow;

use crate::git;
use crate::history::Change;

/// What one git process answered, commit by commit.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Parsed {
    /// One list for each commit that was asked about, in the order asked.
    pub changes: Vec<Vec<Change>>,
    /// True when a path was not valid UTF-8 and had to be converted.
    pub lossy_paths: bool,
}

/// Reads the output of the command above for the commits `ids`, which are
/// the ids that were written to it, in that order.
///
/// Git skips an id it cannot use without saying so and without failing:
/// measured with git 2.52, an id that names no object printed nothing and
/// git still exited with 0. So this also checks the answer for
/// completeness. Every id has to come back, in the order sent, and
/// anything else gives `None`, as does any piece of another shape.
pub(crate) fn parse(output: &[u8], ids: &[&str]) -> Option<Parsed> {
    let mut parsed = Parsed {
        changes: Vec::with_capacity(ids.len()),
        lossy_paths: false,
    };
    let mut expected = ids.iter();
    let mut pieces = git::records(output)?.into_iter();

    while let Some(piece) = pieces.next() {
        // Split at the first two tabs only. A path may hold tabs itself.
        let mut fields = piece.splitn(3, |&byte| byte == b'\t');
        let added = fields.next()?;
        let Some(deleted) = fields.next() else {
            // No tab at all, so this is a commit id. A path can never be
            // taken for one: it comes after two tabs, or right after a
            // rename's numbers, where the two pieces are read as paths
            // whatever they look like.
            if piece != expected.next()?.as_bytes() {
                return None;
            }
            parsed.changes.push(Vec::new());
            continue;
        };
        let path = fields.next()?;

        let (path, renamed_from) = if path.is_empty() {
            let (old, new) = (pieces.next()?, pieces.next()?);
            if old.is_empty() || new.is_empty() {
                return None;
            }
            (new, Some(old))
        } else {
            (path, None)
        };
        let (added, deleted, binary) = match (added, deleted) {
            (b"-", b"-") => (0, 0, true),
            _ => (git::number(added)?, git::number(deleted)?, false),
        };

        let change = Change {
            path: text(path, &mut parsed.lossy_paths),
            renamed_from: renamed_from.map(|old| text(old, &mut parsed.lossy_paths)),
            added,
            deleted,
            binary,
        };
        // `last_mut` finds nothing when a change comes before any commit id.
        parsed.changes.last_mut()?.push(change);
    }

    // A commit that was asked about and never answered.
    if expected.next().is_some() {
        return None;
    }
    Some(parsed)
}

/// A path as text. Git stores paths as bytes; one that is not valid UTF-8
/// is converted, and `lossy` records that it happened.
fn text(path: &[u8], lossy: &mut bool) -> String {
    match String::from_utf8_lossy(path) {
        Cow::Borrowed(path) => path.to_owned(),
        Cow::Owned(path) => {
            *lossy = true;
            path
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEWEST: &str = "8a948f1aeab5148a328334948a63bebfe78bc9c1";
    const AWKWARD: &str = "0e33afdfb8427308d454c100c3f639a1831e6b47";
    const EMPTY: &str = "f4159aae765ca47aeafe17916999e367e9634a4c";
    const EDITED: &str = "8f5f8fa96447d4136b0346e382ad84c3f8e927e2";
    const IDS: [&str; 4] = [NEWEST, AWKWARD, EMPTY, EDITED];

    /// What git 2.52 printed for four commits of a small repository. The
    /// newest renames a file whose old and new names both look like commit
    /// ids. The second adds a link, a binary file and a file with a tab in
    /// its name. The third changes nothing. The last renames a file and
    /// edits one of its lines.
    const OUTPUT: &[u8] = b"8a948f1aeab5148a328334948a63bebfe78bc9c1\0\
        0\t0\t\0\
        822e3536cbae5f3404efab40519ba1de1102e6a0\0\
        a263f2cf44a0bc9b83b7c05f36125deb0f017b8e\0\
        0e33afdfb8427308d454c100c3f639a1831e6b47\0\
        1\t0\t822e3536cbae5f3404efab40519ba1de1102e6a0\0\
        1\t0\tlatest\0\
        -\t-\tlogo.bin\0\
        1\t0\todd\tname.txt\0\
        f4159aae765ca47aeafe17916999e367e9634a4c\0\
        8f5f8fa96447d4136b0346e382ad84c3f8e927e2\0\
        1\t1\t\0\
        src/report.py\0\
        src/summary.py\0";

    /// Where `piece` starts in [`OUTPUT`].
    fn start_of(piece: &[u8]) -> usize {
        OUTPUT
            .windows(piece.len())
            .position(|window| window == piece)
            .expect("the piece is part of the output")
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

    #[test]
    fn reads_the_changes_commit_by_commit() {
        let parsed = parse(OUTPUT, &IDS).unwrap();
        assert!(!parsed.lossy_paths);
        assert_eq!(
            parsed.changes,
            [
                vec![renamed(
                    "822e3536cbae5f3404efab40519ba1de1102e6a0",
                    "a263f2cf44a0bc9b83b7c05f36125deb0f017b8e",
                    0,
                    0
                )],
                vec![
                    change("822e3536cbae5f3404efab40519ba1de1102e6a0", 1, 0),
                    change("latest", 1, 0),
                    Change {
                        binary: true,
                        ..change("logo.bin", 0, 0)
                    },
                    change("odd\tname.txt", 1, 0),
                ],
                vec![],
                vec![renamed("src/report.py", "src/summary.py", 1, 1)],
            ]
        );
    }

    #[test]
    fn no_commits_and_no_output_is_an_empty_answer() {
        let parsed = parse(b"", &[]).unwrap();
        assert!(parsed.changes.is_empty());
    }

    /// The second piece is a path that is, letter for letter, the id of the
    /// commit that comes next. It follows two tabs, so it is a path.
    #[test]
    fn a_file_named_like_the_next_commit_is_still_a_file() {
        let output = [
            NEWEST.as_bytes(),
            b"\0",
            b"3\t1\t",
            AWKWARD.as_bytes(),
            b"\0",
            AWKWARD.as_bytes(),
            b"\0",
        ]
        .concat();
        let parsed = parse(&output, &[NEWEST, AWKWARD]).unwrap();
        assert_eq!(parsed.changes, [vec![change(AWKWARD, 3, 1)], vec![]]);
    }

    #[test]
    fn a_binary_file_can_be_renamed_too() {
        let output = [NEWEST.as_bytes(), b"\0-\t-\t\0old.png\0new.png\0"].concat();
        let parsed = parse(&output, &[NEWEST]).unwrap();
        assert_eq!(
            parsed.changes,
            [vec![Change {
                binary: true,
                ..renamed("old.png", "new.png", 0, 0)
            }]]
        );
    }

    #[test]
    fn a_path_that_is_not_utf8_is_converted_and_reported() {
        let output = [NEWEST.as_bytes(), b"\x001\t0\tcaf\xe9.txt\0"].concat();
        let parsed = parse(&output, &[NEWEST]).unwrap();
        assert!(parsed.lossy_paths);
        assert_eq!(parsed.changes, [vec![change("caf\u{fffd}.txt", 1, 0)]]);
    }

    /// Git leaves out a commit it cannot read and still reports success, so
    /// an answer with a commit missing, added or out of place is refused.
    #[test]
    fn every_commit_has_to_answer_in_the_order_asked() {
        // The third commit of four changed nothing, so its whole answer is
        // its id. Leave that out and one commit never answers.
        let empty = start_of(EMPTY.as_bytes());
        let without_empty = [&OUTPUT[..empty], &OUTPUT[empty + EMPTY.len() + 1..]].concat();
        assert!(parse(&without_empty, &[NEWEST, AWKWARD, EDITED]).is_some());
        assert_eq!(parse(&without_empty, &IDS), None, "one commit is missing");

        assert_eq!(parse(OUTPUT, &IDS[..3]), None, "one commit too many");
        assert_eq!(
            parse(OUTPUT, &[AWKWARD, NEWEST, EMPTY, EDITED]),
            None,
            "the commits are out of order"
        );
        assert_eq!(parse(b"", &[NEWEST]), None, "nothing answered at all");
    }

    /// Output that was cut short must be refused, never half-read, and
    /// never a crash. A cut before the last commit's id leaves a commit
    /// that never answers. After that id, a cut is refused unless it ends a
    /// piece, or falls inside a rename, which needs both of its paths.
    #[test]
    fn output_cut_short_is_refused() {
        let last_id_ends = start_of(EDITED.as_bytes()) + EDITED.len() + 1;
        let rename_ends = OUTPUT.len();
        for end in 0..OUTPUT.len() {
            let prefix = &OUTPUT[..end];
            let whole = end == last_id_ends;
            assert_eq!(parse(prefix, &IDS).is_some(), whole, "cut at {end}");
        }
        assert!(parse(&OUTPUT[..rename_ends], &IDS).is_some());
    }

    #[test]
    fn output_of_another_shape_is_refused() {
        let cases: [(&str, &[u8]); 10] = [
            ("a change before any commit", b"1\t0\tfile.txt\0"),
            (
                "a commit that was not asked about",
                b"0e33afdfb8427308d454c100c3f639a1831e6b47\0",
            ),
            (
                "one tab",
                b"8a948f1aeab5148a328334948a63bebfe78bc9c1\x001\tfile.txt\0",
            ),
            (
                "a word for a number",
                b"8a948f1aeab5148a328334948a63bebfe78bc9c1\0one\t0\tfile.txt\0",
            ),
            (
                "half binary",
                b"8a948f1aeab5148a328334948a63bebfe78bc9c1\0-\t0\tfile.txt\0",
            ),
            (
                "a negative number",
                b"8a948f1aeab5148a328334948a63bebfe78bc9c1\x001\t-1\tfile.txt\0",
            ),
            (
                "a rename with no old path",
                b"8a948f1aeab5148a328334948a63bebfe78bc9c1\x001\t1\t\0\0new\0",
            ),
            (
                "a rename with no new path",
                b"8a948f1aeab5148a328334948a63bebfe78bc9c1\x001\t1\t\0old\0\0",
            ),
            (
                "an empty piece",
                b"8a948f1aeab5148a328334948a63bebfe78bc9c1\0\0",
            ),
            (
                "text that is not an id",
                b"8a948f1aeab5148a328334948a63bebfe78bc9c1\0not-an-id\n\0",
            ),
        ];
        for (what, output) in cases {
            assert_eq!(parse(output, &[NEWEST]), None, "{what}");
        }
    }

    /// Damaged output must never crash the parser. This changes the real
    /// output in 20,000 ways, chosen by a fixed sequence of numbers so that
    /// a failure repeats, and reads each result. What comes back is not
    /// checked: only that something does.
    #[test]
    fn damaged_output_never_crashes_the_parser() {
        // xorshift: a few lines that give a repeatable stream of numbers.
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        const INTERESTING: [u8; 8] = [0, b'\t', b'-', b'0', b'9', b'\n', 0x1f, 0xff];

        for _ in 0..20_000 {
            let mut damaged = OUTPUT.to_vec();
            for _ in 0..=(next() % 4) {
                let at = (next() % damaged.len() as u64) as usize;
                let byte = INTERESTING[(next() % 8) as usize];
                match next() % 3 {
                    0 => damaged[at] = byte,
                    1 => damaged.insert(at, byte),
                    _ => {
                        damaged.remove(at);
                    }
                }
                if damaged.is_empty() {
                    break;
                }
            }
            let _ = parse(&damaged, &IDS);
        }
    }
}
