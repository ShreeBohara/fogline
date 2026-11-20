//! Which commits an agent took part in (spec sections 5.2 and 5.4).
//!
//! A commit is read for the marks in the table of [`crate::signatures`] and
//! falls into one of three classes:
//!
//! - **agent-attributed**: a mark of a coding agent is on it;
//! - **automation**: a bot made it, and no mark of an agent is on it;
//! - **unsigned**: everything else.
//!
//! Unsigned does not mean hand-written: an agent that leaves no mark cannot
//! be seen here. And an agent-attributed commit can hold lines a person
//! wrote. So the count of agent-attributed commits is a lower bound, and
//! anything about lines is an estimate.
//!
//! A mark is looked for in three places: the trailers of the message, the
//! lines of the message, and the author.

use crate::history::Commit;
use crate::signatures::{self, Kind, Rule, Signature, Text};

/// The class of a commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    AgentAttributed,
    Automation,
    Unsigned,
}

/// What the marks on a commit say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribution {
    pub class: Class,
    /// The ids of the signatures that decided the class, in the order of
    /// the table: the agents found on an agent-attributed commit, the bots
    /// on an automation commit, and nothing on an unsigned one. A commit
    /// counts once however many there are.
    pub signatures: Vec<&'static str>,
}

/// Reads the marks on a commit.
pub fn of(commit: &Commit) -> Attribution {
    among(commit, signatures::TABLE)
}

/// The same with any table, so that tests can bring their own.
fn among(commit: &Commit, table: &'static [Signature]) -> Attribution {
    let trailers: Vec<Trailer> = commit.message.lines().filter_map(trailer).collect();
    let found = |kind: Kind| -> Vec<&'static str> {
        table
            .iter()
            .filter(|signature| signature.kind == kind)
            .filter(|signature| {
                signature
                    .marks
                    .iter()
                    .any(|mark| carries(commit, &trailers, &mark.rule))
            })
            .map(|signature| signature.id)
            .collect()
    };

    // An agent beats a bot: `cursor[bot]` is a bot by its name, and what
    // matters about it is that it is an agent.
    let agents = found(Kind::Agent);
    if !agents.is_empty() {
        return Attribution {
            class: Class::AgentAttributed,
            signatures: agents,
        };
    }
    let bots = found(Kind::Automation);
    if !bots.is_empty() {
        return Attribution {
            class: Class::Automation,
            signatures: bots,
        };
    }
    Attribution {
        class: Class::Unsigned,
        signatures: Vec::new(),
    }
}

/// True when the commit carries the mark that `rule` describes.
fn carries(commit: &Commit, trailers: &[Trailer], rule: &Rule) -> bool {
    match rule {
        Rule::Trailer { key, name, email } => trailers.iter().any(|trailer| {
            let (who, address) = identity(trailer.value);
            trailer.key.eq_ignore_ascii_case(key) && is(name, who) && is(email, address)
        }),
        Rule::TrailerKey { key } => trailers
            .iter()
            .any(|trailer| trailer.key.eq_ignore_ascii_case(key)),
        Rule::Author { name, email } => {
            is(name, &commit.author_name) && is(email, &commit.author_email)
        }
        Rule::Line { holds } => commit
            .message
            .lines()
            .any(|line| holds_ignoring_case(line, holds)),
    }
}

/// A line of a message that has the form `Key: value`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Trailer<'a> {
    key: &'a str,
    value: &'a str,
}

/// Reads a line as a trailer. A key is made of letters, digits and hyphens,
/// as git's own trailers are, so a sentence with a colon in it is not one.
///
/// The line may stand anywhere in the message, and may be indented: a
/// squash merge copies the trailers of the commits it squashes into the
/// middle of its own message.
fn trailer(line: &str) -> Option<Trailer<'_>> {
    let (key, value) = line.trim().split_once(':')?;
    let is_key = !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    is_key.then_some(Trailer {
        key,
        value: value.trim(),
    })
}

/// Splits `Name <email>` into its two parts. A value with no email in
/// angle brackets is all name.
fn identity(value: &str) -> (&str, &str) {
    let Some((name, rest)) = value.rsplit_once('<') else {
        return (value.trim(), "");
    };
    match rest.split_once('>') {
        Some((email, _)) => (name.trim(), email.trim()),
        None => (value.trim(), ""),
    }
}

/// True when `text` is what `wanted` asks for. Letters compare without
/// regard to case: tools do not agree on `Co-Authored-By`, and people
/// retype these lines.
fn is(wanted: &Text, text: &str) -> bool {
    match wanted {
        Text::Any => true,
        Text::Is(whole) => text.eq_ignore_ascii_case(whole),
        Text::StartsWith(start) => text
            .get(..start.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(start)),
        Text::EndsWith(end) => text
            .len()
            .checked_sub(end.len())
            .and_then(|from| text.get(from..))
            .is_some_and(|tail| tail.eq_ignore_ascii_case(end)),
    }
}

/// True when `line` holds `part`, letters compared without regard to case.
/// It compares bytes at each place in the line, which needs no copy of the
/// line: this runs on every line of every message.
fn holds_ignoring_case(line: &str, part: &str) -> bool {
    let (line, part) = (line.as_bytes(), part.as_bytes());
    part.is_empty()
        || line
            .windows(part.len())
            .any(|window| window.eq_ignore_ascii_case(part))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signatures::{Mark, Seen};

    fn commit(name: &str, email: &str, message: &str) -> Commit {
        Commit {
            id: "0".repeat(40),
            parents: 1,
            author_time: 0,
            author_name: name.to_owned(),
            author_email: email.to_owned(),
            message: message.to_owned(),
            changes: Vec::new(),
        }
    }

    /// A commit by a person, with this message.
    fn written(message: &str) -> Commit {
        commit("Ada Example", "ada@example.com", message)
    }

    #[test]
    fn reads_a_line_as_a_trailer() {
        let read = |line| trailer(line).map(|trailer| (trailer.key, trailer.value));
        assert_eq!(
            read("Co-authored-by: Ada <ada@example.com>"),
            Some(("Co-authored-by", "Ada <ada@example.com>"))
        );
        // Indented, as inside the body of a squash merge.
        assert_eq!(
            read("    Amp-Thread-ID: T-1  "),
            Some(("Amp-Thread-ID", "T-1"))
        );
        // The value may have colons of its own, and may be empty.
        assert_eq!(
            read("See: https://example.com"),
            Some(("See", "https://example.com"))
        );
        assert_eq!(read("Reviewed-by:"), Some(("Reviewed-by", "")));
    }

    #[test]
    fn a_sentence_with_a_colon_is_not_a_trailer() {
        for line in [
            "Fix the parser: it dropped the last line",
            "* Co-authored-by: Ada <ada@example.com>",
            "Thanks to Co-authored-by: Ada <ada@example.com>",
            ": no key",
            "no colon here",
            "",
        ] {
            assert_eq!(trailer(line), None, "{line:?}");
        }
    }

    #[test]
    fn splits_a_name_and_an_email() {
        assert_eq!(
            identity("Ada Example <ada@example.com>"),
            ("Ada Example", "ada@example.com")
        );
        assert_eq!(identity("<ada@example.com>"), ("", "ada@example.com"));
        assert_eq!(identity("Ada Example"), ("Ada Example", ""));
        assert_eq!(identity("Ada <unfinished"), ("Ada <unfinished", ""));
        // The last pair of brackets is the email.
        assert_eq!(identity("A <b> <c@d>"), ("A <b>", "c@d"));
        assert_eq!(identity(""), ("", ""));
    }

    #[test]
    fn compares_text_without_regard_to_case() {
        assert!(is(&Text::Any, ""));
        assert!(is(&Text::Is("noreply@example.com"), "NoReply@Example.com"));
        assert!(!is(
            &Text::Is("noreply@example.com"),
            "x-noreply@example.com"
        ));
        assert!(is(&Text::StartsWith("Copilot"), "copilot"));
        assert!(is(&Text::StartsWith("Copilot"), "Copilot Agent"));
        assert!(!is(&Text::StartsWith("Copilot"), "Co"));
        assert!(is(&Text::EndsWith("[bot]"), "dependabot[BOT]"));
        assert!(!is(&Text::EndsWith("[bot]"), "bot]"));
        // A name that is not ASCII must not break the comparison: the cut
        // would fall inside a character here.
        assert!(!is(&Text::StartsWith("ab"), "aé"));
        assert!(!is(&Text::EndsWith("ab"), "éb"));
    }

    const SEEN: Seen = Seen {
        repository: "example/example",
        commit: "0000000000000000000000000000000000000000",
        line: "",
    };

    static TABLE: [Signature; 3] = [
        Signature {
            id: "helper",
            kind: Kind::Agent,
            tool: "Helper",
            marks: &[
                Mark {
                    rule: Rule::Trailer {
                        key: "co-authored-by",
                        name: Text::Any,
                        email: Text::Is("helper@example.com"),
                    },
                    fixture: "",
                    seen: SEEN,
                },
                Mark {
                    rule: Rule::Author {
                        name: Text::Is("helper[bot]"),
                        email: Text::Any,
                    },
                    fixture: "",
                    seen: SEEN,
                },
            ],
        },
        Signature {
            id: "marker",
            kind: Kind::Agent,
            tool: "Marker",
            marks: &[
                Mark {
                    rule: Rule::Line {
                        holds: "Made with [Marker]",
                    },
                    fixture: "",
                    seen: SEEN,
                },
                Mark {
                    rule: Rule::TrailerKey { key: "Marker-Run" },
                    fixture: "",
                    seen: SEEN,
                },
            ],
        },
        Signature {
            id: "bot",
            kind: Kind::Automation,
            tool: "A bot",
            marks: &[Mark {
                rule: Rule::Author {
                    name: Text::EndsWith("[bot]"),
                    email: Text::Any,
                },
                fixture: "",
                seen: SEEN,
            }],
        },
    ];

    fn read(commit: &Commit) -> (Class, Vec<&'static str>) {
        let attribution = among(commit, &TABLE);
        (attribution.class, attribution.signatures)
    }

    #[test]
    fn a_commit_with_no_mark_is_unsigned() {
        let plain = written("Fix the parser\n\nIt dropped the last line.\n");
        assert_eq!(read(&plain), (Class::Unsigned, vec![]));
        // A message that talks about a tool carries no mark of it.
        let talk = written("Document the helper@example.com co-authored-by line\n");
        assert_eq!(read(&talk), (Class::Unsigned, vec![]));
    }

    #[test]
    fn a_trailer_of_an_agent_makes_a_commit_agent_attributed() {
        for message in [
            "Fix\n\nCo-authored-by: Helper <helper@example.com>\n",
            "Fix\n\nCO-AUTHORED-BY: Helper 3 <HELPER@example.com>\n",
            // In the middle of the message, indented: a squash merge.
            "Squash\n\n* one\n\n    Co-Authored-By: Helper <helper@example.com>\n\n* two\n",
            // Windows line endings.
            "Fix\r\n\r\nCo-authored-by: Helper <helper@example.com>\r\n",
        ] {
            assert_eq!(
                read(&written(message)),
                (Class::AgentAttributed, vec!["helper"]),
                "{message:?}"
            );
        }
    }

    #[test]
    fn a_trailer_needs_its_key_and_its_email() {
        for message in [
            // The right email under another key.
            "Fix\n\nReviewed-by: Helper <helper@example.com>\n",
            // The right key with another email.
            "Fix\n\nCo-authored-by: Helper <helper@example.org>\n",
            "Fix\n\nCo-authored-by: Helper <not-helper@example.com>\n",
            // No email at all.
            "Fix\n\nCo-authored-by: helper@example.com\n",
        ] {
            assert_eq!(
                read(&written(message)),
                (Class::Unsigned, vec![]),
                "{message:?}"
            );
        }
    }

    #[test]
    fn a_line_or_a_key_can_be_the_mark() {
        let line = written("Fix\n\n\u{1f916} Made with [Marker](https://example.com)\n");
        assert_eq!(read(&line), (Class::AgentAttributed, vec!["marker"]));
        let key = written("Fix\n\nMarker-Run: 1234\n");
        assert_eq!(read(&key), (Class::AgentAttributed, vec!["marker"]));
    }

    #[test]
    fn every_agent_found_is_recorded_and_the_commit_counts_once() {
        let both = written(
            "Fix\n\nMade with [Marker]\n\nCo-authored-by: Helper <helper@example.com>\n\
             Co-authored-by: Helper <helper@example.com>\n",
        );
        // In the order of the table, each id once.
        assert_eq!(
            read(&both),
            (Class::AgentAttributed, vec!["helper", "marker"])
        );
    }

    #[test]
    fn a_marker_text_is_found_anywhere_in_a_line() {
        assert!(holds_ignoring_case(
            "Made with [Marker](x)",
            "made with [marker]"
        ));
        assert!(holds_ignoring_case(
            "\u{1f916} MADE WITH [MARKER]",
            "Made with [Marker]"
        ));
        assert!(!holds_ignoring_case(
            "Made with Marker",
            "Made with [Marker]"
        ));
        assert!(!holds_ignoring_case("short", "longer than the line"));
        assert!(holds_ignoring_case("anything", ""));
    }

    /// The real table against its own evidence: each mark is found on a
    /// commit that carries the line the mark was seen in, and that commit
    /// gets the mark's signature. The line is what a public commit really
    /// holds; `tests/signatures.rs` checks that against GitHub.
    #[test]
    fn every_mark_of_the_table_is_found_where_it_was_seen() {
        for signature in signatures::TABLE {
            for mark in signature.marks {
                let seen = mark.seen.line;
                let commit = match mark.rule {
                    Rule::Author { .. } => {
                        let (name, email) = identity(seen);
                        commit(name, email, "A change\n")
                    }
                    _ => written(&format!("A change\n\nMore about it.\n\n{seen}\n")),
                };
                // This mark by itself, so that a second mark of the same
                // tool cannot stand in for it.
                let trailers: Vec<Trailer> = commit.message.lines().filter_map(trailer).collect();
                assert!(
                    carries(&commit, &trailers, &mark.rule),
                    "{}: the rule of {:?} does not find {seen:?}",
                    signature.id,
                    mark.fixture
                );

                let attribution = of(&commit);
                let class = match signature.kind {
                    Kind::Agent => Class::AgentAttributed,
                    Kind::Automation => Class::Automation,
                };
                assert_eq!(attribution.class, class, "{}: {seen:?}", signature.id);
                assert!(
                    attribution.signatures.contains(&signature.id),
                    "{}: not found in {seen:?}, which gave {:?}",
                    signature.id,
                    attribution.signatures
                );
            }
        }
    }

    /// What people write is not what agents sign. None of these is a mark.
    #[test]
    fn the_table_does_not_take_people_for_agents() {
        for message in [
            "Fix the parser\n",
            "Fix\n\nCo-authored-by: Grace Example <grace@example.com>\n",
            "Fix\n\nSigned-off-by: Ada Example <ada@example.com>\n",
            // The right addresses under a key that is not a second author.
            "Fix\n\nReviewed-by: Claude <noreply@anthropic.com>\n",
            "Fix\n\nReported-by: Cursor <cursoragent@cursor.com>\n",
            // Talk about the tools.
            "Tried Claude Code and Cursor on this and wrote it by hand\n",
            "Docs: explain the Co-authored-by: Claude <noreply@anthropic.com> line\n",
            // A name that says Gemini with an address that is not Google's,
            // and Google's address with another name.
            "Fix\n\nCo-authored-by: Gemini <gemini@example.com>\n",
            "Fix\n\nCo-authored-by: Build Robot <noreply@google.com>\n",
            // An address that only ends like Copilot's.
            "Fix\n\nCo-authored-by: Someone <1+NotCopilot@users.noreply.github.com>\n",
            // Block's address for all its open source, without Goose's name.
            "Fix\n\nCo-authored-by: Release Robot <opensource@block.xyz>\n",
            "Fix\n\nCo-authored-by: Goose Keeper <opensource@block.xyz>\n",
            // Addresses at the right domain that are not the agent's.
            "Fix\n\nCo-authored-by: Someone <someone@warp.dev>\n",
            "Fix\n\nCo-authored-by: Someone <someone@jetbrains.com>\n",
            // Claude Code's line without the brackets its real form has.
            "Fix\n\nGenerated with Claude Code\n",
        ] {
            let attribution = of(&written(message));
            assert_eq!(attribution.class, Class::Unsigned, "{message:?}");
            assert!(attribution.signatures.is_empty(), "{message:?}");
        }
    }

    /// A person's name with a tool's name added to it is the tool's mark;
    /// the same word elsewhere in a name is not.
    #[test]
    fn an_author_is_read_by_the_rules_of_the_table() {
        let class = |name: &str| of(&commit(name, "ada@example.com", "Fix\n")).class;
        assert_eq!(class("Ada Example (aider)"), Class::AgentAttributed);
        assert_eq!(class("Aider Example"), Class::Unsigned);
        assert_eq!(class("google-labs-jules[bot]"), Class::AgentAttributed);
        assert_eq!(class("renovate[bot]"), Class::Automation);
        assert_eq!(class("Bot Example"), Class::Unsigned);
    }

    #[test]
    fn a_bot_is_automation_unless_it_is_an_agent() {
        let updater = commit(
            "updater[bot]",
            "1+updater[bot]@example.com",
            "Bump a version\n",
        );
        assert_eq!(read(&updater), (Class::Automation, vec!["bot"]));

        // The agent's own bot account: an agent, and not counted as a bot.
        let agent = commit("helper[bot]", "2+helper[bot]@example.com", "Fix\n");
        assert_eq!(read(&agent), (Class::AgentAttributed, vec!["helper"]));

        // A bot whose commit carries an agent's trailer.
        let helped = commit(
            "updater[bot]",
            "1+updater[bot]@example.com",
            "Bump\n\nCo-authored-by: Helper <helper@example.com>\n",
        );
        assert_eq!(read(&helped), (Class::AgentAttributed, vec!["helper"]));
    }
}
