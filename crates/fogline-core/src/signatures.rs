//! The table of agent signatures (spec section 5.4): the marks that coding
//! agents and bots leave on the commits they take part in.
//!
//! A mark is one of three things:
//!
//! - a trailer, which is a line of the message of the form `Key: value`,
//!   such as `Co-authored-by: Cursor <cursoragent@cursor.com>`;
//! - the commit's author, such as `devin-ai-integration[bot]`;
//! - a line of the message that holds a fixed text.
//!
//! The table is data. It is written in Rust and not in a file that is read
//! at run time, so a mistake in it is a compile error or a failing test and
//! never a scan that goes wrong. It needs no parser and no pattern
//! language: every rule is "is", "starts with", "ends with" or "holds".
//!
//! No mark is here without a real commit behind it. Each one names a public
//! commit that carries it and quotes the line, and a test that needs the
//! network reads those commits again (`tests/signatures.rs`). Each one also
//! names the commit of fixture F2 that carries it.
//!
//! What the table cannot see is an agent that leaves no mark. That is why
//! every count made with it is a lower bound.

/// The version of the table. A scan's output records it, so that two scans
/// can be told apart when the table has changed between them.
pub const VERSION: &str = "2026.11.1";

/// What a signature says about a commit that carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A coding agent took part in the commit.
    Agent,
    /// A bot made the commit, such as one that updates dependencies.
    Automation,
}

/// One tool and the marks it leaves. Any one mark is enough.
#[derive(Debug)]
pub struct Signature {
    /// A short name that never changes, for data.
    pub id: &'static str,
    pub kind: Kind,
    /// The tool's name, for people.
    pub tool: &'static str,
    pub marks: &'static [Mark],
}

/// One mark, and where it can be seen.
#[derive(Debug)]
pub struct Mark {
    pub rule: Rule,
    /// The first line of the commit of fixture F2 that carries this mark.
    pub fixture: &'static str,
    pub seen: Seen,
}

/// How a mark is recognised.
#[derive(Debug)]
pub enum Rule {
    /// A trailer `Key: Name <email>`. The key is compared without regard to
    /// case, like everything else.
    Trailer {
        key: &'static str,
        name: Text,
        email: Text,
    },
    /// A trailer with this key, whatever follows it.
    TrailerKey { key: &'static str },
    /// The commit's author.
    Author { name: Text, email: Text },
    /// A line of the message that holds this text.
    Line { holds: &'static str },
}

/// What a name or an email has to be. Letters are compared without regard
/// to case.
#[derive(Debug)]
pub enum Text {
    Any,
    Is(&'static str),
    StartsWith(&'static str),
    EndsWith(&'static str),
}

/// A public commit that carries a mark.
#[derive(Debug)]
pub struct Seen {
    /// The repository on GitHub, as `owner/name`.
    pub repository: &'static str,
    /// The commit's full id.
    pub commit: &'static str,
    /// The mark as it stands in that commit: the line of the message, or
    /// for an author the author as `Name <email>`.
    pub line: &'static str,
}

/// The trailer git and GitHub use for a second author. Agents sign with it.
const CO_AUTHOR: &str = "Co-authored-by";

/// The signatures, agents first. The order is the order in which a
/// commit's signatures are listed.
pub static TABLE: &[Signature] = &[
    Signature {
        id: "claude-code",
        kind: Kind::Agent,
        tool: "Claude Code",
        marks: &[
            // The name in the trailer is the model, such as "Claude Fable
            // 5.2", so only the email is fixed.
            Mark {
                rule: Rule::Trailer {
                    key: CO_AUTHOR,
                    name: Text::Any,
                    email: Text::Is("noreply@anthropic.com"),
                },
                fixture: "claude-trailer",
                seen: Seen {
                    repository: "Arize-ai/openinference",
                    commit: "a380ac07a41377ce55ef07d77fb59a2a18150280",
                    line: "Co-authored-by: Claude Sonnet 4.6 <noreply@anthropic.com>",
                },
            },
            // The line Claude Code puts above its trailer. The tool's name
            // is a link, so it stands in square brackets.
            Mark {
                rule: Rule::Line {
                    holds: "Generated with [Claude Code]",
                },
                fixture: "claude-marker",
                seen: Seen {
                    repository: "usenotra/notra-mcp",
                    commit: "cc5e4acdc2366e13a318d6f8d4270d6411eb5e94",
                    line: "\u{1f916} Generated with [Claude Code](https://claude.com/claude-code)",
                },
            },
        ],
    },
    Signature {
        id: "claude-app",
        kind: Kind::Agent,
        tool: "Claude (GitHub app)",
        marks: &[Mark {
            rule: Rule::Author {
                name: Text::Is("claude[bot]"),
                email: Text::Any,
            },
            fixture: "claude-app-author",
            seen: Seen {
                repository: "Arize-ai/openinference",
                commit: "a380ac07a41377ce55ef07d77fb59a2a18150280",
                line: "claude[bot] <209825114+claude[bot]@users.noreply.github.com>",
            },
        }],
    },
    Signature {
        id: "cursor",
        kind: Kind::Agent,
        tool: "Cursor",
        marks: &[
            Mark {
                rule: Rule::Trailer {
                    key: CO_AUTHOR,
                    name: Text::Any,
                    email: Text::Is("cursoragent@cursor.com"),
                },
                fixture: "cursor-trailer",
                seen: Seen {
                    repository: "mattermost/mattermost",
                    commit: "d99a529bd732d2b87c9b35b0dccb0efdc7d8e202",
                    line: "Co-authored-by: Cursor <cursoragent@cursor.com>",
                },
            },
            Mark {
                rule: Rule::Author {
                    name: Text::Is("cursor[bot]"),
                    email: Text::Any,
                },
                fixture: "cursor-author",
                seen: Seen {
                    repository: "getsentry/sentry",
                    commit: "3a9f08fedc2896e6cf29189c555c9bd1e221335a",
                    line: "cursor[bot] <206951365+cursor[bot]@users.noreply.github.com>",
                },
            },
        ],
    },
    Signature {
        id: "copilot",
        kind: Kind::Agent,
        tool: "GitHub Copilot",
        marks: &[
            // GitHub writes the address of its account "Copilot" as a number,
            // a plus sign and the account's name. The name in front of it
            // varies: "Copilot", "Copilot App", "copilot-swe-agent[bot]".
            Mark {
                rule: Rule::Trailer {
                    key: CO_AUTHOR,
                    name: Text::Any,
                    email: Text::EndsWith("+Copilot@users.noreply.github.com"),
                },
                fixture: "copilot-trailer",
                seen: Seen {
                    repository: "open-telemetry/semantic-conventions-genai",
                    commit: "aaed88a8f1462ff599af0b0d535d37e4e302e5e6",
                    line: "Co-authored-by: Copilot App <223556219+Copilot@users.noreply.github.com>",
                },
            },
            Mark {
                rule: Rule::Author {
                    name: Text::Any,
                    email: Text::EndsWith("+Copilot@users.noreply.github.com"),
                },
                fixture: "copilot-author",
                seen: Seen {
                    repository: "Open-Coding-Society/pages",
                    commit: "a10bc9870d01f28c9eef5bff34582fcc49509f05",
                    line: "copilot-swe-agent[bot] <198982749+Copilot@users.noreply.github.com>",
                },
            },
        ],
    },
    Signature {
        id: "codex",
        kind: Kind::Agent,
        tool: "Codex",
        marks: &[Mark {
            rule: Rule::Trailer {
                key: CO_AUTHOR,
                name: Text::Any,
                email: Text::Is("codex@openai.com"),
            },
            fixture: "codex-trailer",
            seen: Seen {
                repository: "tmt-ninja/tmt",
                commit: "4b93575293c0f5c6a7fb8bb35bd26d16e7d8eeb1",
                line: "Co-authored-by: Codex <codex@openai.com>",
            },
        }],
    },
    Signature {
        id: "jules",
        kind: Kind::Agent,
        tool: "Jules",
        marks: &[Mark {
            rule: Rule::Author {
                name: Text::Is("google-labs-jules[bot]"),
                email: Text::Any,
            },
            fixture: "jules-author",
            seen: Seen {
                repository: "googleapis/python-bigquery",
                commit: "fa76e310a16ea6cba0071ff1d767ca1c71514da7",
                line: "google-labs-jules[bot] <161369871+google-labs-jules[bot]@users.noreply.github.com>",
            },
        }],
    },
    Signature {
        id: "gemini",
        kind: Kind::Agent,
        tool: "Gemini",
        marks: &[
            // The address is one that anything at Google could use, so the
            // name has to say Gemini too. The model follows it.
            Mark {
                rule: Rule::Trailer {
                    key: CO_AUTHOR,
                    name: Text::StartsWith("Gemini"),
                    email: Text::Is("noreply@google.com"),
                },
                fixture: "gemini-trailer",
                seen: Seen {
                    repository: "tmt-ninja/tmt",
                    commit: "c5c0d3b835b311b36fb7b9f6937ca302a4ae6dd0",
                    line: "Co-Authored-By: Gemini 4 Hydrogen <noreply@google.com>",
                },
            },
        ],
    },
    Signature {
        id: "amp",
        kind: Kind::Agent,
        tool: "Amp",
        marks: &[Mark {
            rule: Rule::TrailerKey {
                key: "Amp-Thread-ID",
            },
            fixture: "amp-thread",
            seen: Seen {
                repository: "sourcegraph/amp-examples-and-guides",
                commit: "f402cae841c00a73837586c9d2862da1ac29276f",
                line: "Amp-Thread-ID: https://ampcode.com/threads/T-445c508d-1b63-4bc1-bbac-b5b1102cc0a1",
            },
        }],
    },
    Signature {
        id: "factory",
        kind: Kind::Agent,
        tool: "Factory Droid",
        marks: &[Mark {
            rule: Rule::Trailer {
                key: CO_AUTHOR,
                name: Text::Is("factory-droid[bot]"),
                email: Text::Any,
            },
            fixture: "factory-trailer",
            seen: Seen {
                repository: "Factory-AI/factory",
                commit: "74ae7347838832ce0251906cceda24ae63da8538",
                line: "Co-authored-by: factory-droid[bot] <138933559+factory-droid[bot]@users.noreply.github.com>",
            },
        }],
    },
    Signature {
        id: "warp",
        kind: Kind::Agent,
        tool: "Warp",
        marks: &[
            Mark {
                rule: Rule::Trailer {
                    key: CO_AUTHOR,
                    name: Text::Any,
                    email: Text::Is("agent@warp.dev"),
                },
                fixture: "warp-trailer",
                seen: Seen {
                    repository: "warpdotdev/warp",
                    commit: "842e6dea0d90ebb799f728c3c265008a62ee7dcd",
                    line: "Co-authored-by: Warp <agent@warp.dev>",
                },
            },
            // Warp's agent Oz signs with its own address.
            Mark {
                rule: Rule::Trailer {
                    key: CO_AUTHOR,
                    name: Text::Any,
                    email: Text::Is("oz-agent@warp.dev"),
                },
                fixture: "warp-oz-trailer",
                seen: Seen {
                    repository: "warpdotdev/docs",
                    commit: "e77b356179c380f6022466b465291591fdd3ae04",
                    line: "Co-Authored-By: Oz <oz-agent@warp.dev>",
                },
            },
        ],
    },
    Signature {
        id: "kiro",
        kind: Kind::Agent,
        tool: "Kiro",
        marks: &[Mark {
            rule: Rule::Trailer {
                key: CO_AUTHOR,
                name: Text::Any,
                email: Text::Is("noreply@kiro.dev"),
            },
            fixture: "kiro-trailer",
            seen: Seen {
                repository: "bleachbr/bleach-idle",
                commit: "1643b4de260c9a210ca0e4b80b32bdd330b785ee",
                line: "Co-Authored-By: Kiro <noreply@kiro.dev>",
            },
        }],
    },
    Signature {
        id: "junie",
        kind: Kind::Agent,
        tool: "Junie",
        marks: &[Mark {
            rule: Rule::Trailer {
                key: CO_AUTHOR,
                name: Text::Any,
                email: Text::Is("junie@jetbrains.com"),
            },
            fixture: "junie-trailer",
            seen: Seen {
                repository: "JetBrains/intellij-platform-gradle-plugin",
                commit: "8fff8bde25461e8f7f3f8a6ca62e9c1b37b03d9b",
                line: "Co-authored-by: Junie <junie@jetbrains.com>",
            },
        }],
    },
    Signature {
        id: "devin",
        kind: Kind::Agent,
        tool: "Devin",
        marks: &[Mark {
            rule: Rule::Author {
                name: Text::Is("devin-ai-integration[bot]"),
                email: Text::Any,
            },
            fixture: "devin-author",
            seen: Seen {
                repository: "airbytehq/airbyte-python-cdk",
                commit: "bde4377f26e9a6bc8187cef03688ce3c8330c693",
                line: "devin-ai-integration[bot] <158243242+devin-ai-integration[bot]@users.noreply.github.com>",
            },
        }],
    },
    Signature {
        id: "aider",
        kind: Kind::Agent,
        tool: "Aider",
        marks: &[
            // The name is "aider" and the model in brackets.
            Mark {
                rule: Rule::Trailer {
                    key: CO_AUTHOR,
                    name: Text::Any,
                    email: Text::Is("aider@aider.chat"),
                },
                fixture: "aider-trailer",
                seen: Seen {
                    repository: "Aider-AI/aider",
                    commit: "308b1546a035ef012ac91ffb83a2139d01325526",
                    line: "Co-authored-by: aider (gpt-5.4) <aider@aider.chat>",
                },
            },
            // Aider can also sign by adding "(aider)" to the name of the
            // person it works for, and older versions only did that. The
            // commit below is one whose author is not a person's name.
            Mark {
                rule: Rule::Author {
                    name: Text::EndsWith("(aider)"),
                    email: Text::Any,
                },
                fixture: "aider-author",
                seen: Seen {
                    repository: "uwislab/urobot-mcp",
                    commit: "478a4e9c25d5cc475e48023253eb11236ea94f60",
                    line: "\u{201c}aider\u{201d} (aider) <\u{201c}aider@bilibili.com\u{201d}>",
                },
            },
        ],
    },
    Signature {
        id: "opencode",
        kind: Kind::Agent,
        tool: "opencode",
        marks: &[
            // People set this trailer up themselves, so the address varies
            // and is sometimes missing: "OpenCode <noreply@opencode.ai>",
            // "opencode (glm-5.3) <ai@local>", "opencode". The name always
            // starts the same way.
            Mark {
                rule: Rule::Trailer {
                    key: CO_AUTHOR,
                    name: Text::StartsWith("opencode"),
                    email: Text::Any,
                },
                fixture: "opencode-trailer",
                seen: Seen {
                    repository: "eclipse-oniro-mirrors/arkcompiler_ets_frontend",
                    commit: "c69e6dc3566b65d70a1358a75afc16eec630e828",
                    line: "Co-authored-by: opencode (glm-5.3) <ai@local>",
                },
            },
            // With opencode's own address the name is often the model.
            Mark {
                rule: Rule::Trailer {
                    key: CO_AUTHOR,
                    name: Text::Any,
                    email: Text::Is("noreply@opencode.ai"),
                },
                fixture: "opencode-address-trailer",
                seen: Seen {
                    repository: "EbookFoundation/free-programming-books",
                    commit: "334c8c632724379aa2cfa628fc8cb1964f09e850",
                    line: "Co-Authored-By: OpenCode <noreply@opencode.ai>",
                },
            },
            Mark {
                rule: Rule::Author {
                    name: Text::Is("opencode-agent[bot]"),
                    email: Text::Any,
                },
                fixture: "opencode-author",
                seen: Seen {
                    repository: "sst/opencode",
                    commit: "a86ac3f1b9d5fb7de846c29d6e0dc2136c503f0d",
                    line: "opencode-agent[bot] <219766164+opencode-agent[bot]@users.noreply.github.com>",
                },
            },
        ],
    },
    Signature {
        id: "cline",
        kind: Kind::Agent,
        tool: "Cline",
        marks: &[Mark {
            rule: Rule::Trailer {
                key: CO_AUTHOR,
                name: Text::Any,
                email: Text::Is("noreply@cline.bot"),
            },
            fixture: "cline-trailer",
            seen: Seen {
                repository: "cline/cline",
                commit: "5dea21ab89f7d5a719a532a07f7d13db37a68bd8",
                line: "Co-authored-by: Cline <noreply@cline.bot>",
            },
        }],
    },
    Signature {
        id: "goose",
        kind: Kind::Agent,
        tool: "Goose",
        marks: &[
            // The address is the one Block uses for all its open source, so
            // the name has to say Goose too.
            Mark {
                rule: Rule::Trailer {
                    key: CO_AUTHOR,
                    name: Text::Is("Goose"),
                    email: Text::Is("opensource@block.xyz"),
                },
                fixture: "goose-trailer",
                seen: Seen {
                    repository: "block/ghost",
                    commit: "c55888ea181aeef7b4f5bd2af85129239986702f",
                    line: "Co-authored-by: Goose <opensource@block.xyz>",
                },
            },
            // Goose's own address, since the project moved out of Block.
            Mark {
                rule: Rule::Trailer {
                    key: CO_AUTHOR,
                    name: Text::Any,
                    email: Text::Is("goose@aaif.dev"),
                },
                fixture: "goose-aaif-trailer",
                seen: Seen {
                    repository: "aaif-goose/goose",
                    commit: "f2350f8d6814db2c4d4d4ba3772fb94ed5a87152",
                    line: "Co-authored-by: goose <goose@aaif.dev>",
                },
            },
        ],
    },
    // Any other account whose name ends with "[bot]", which is how GitHub
    // names the accounts of its apps. The agents above come first, so their
    // bot accounts never land here.
    Signature {
        id: "bot",
        kind: Kind::Automation,
        tool: "a bot",
        marks: &[Mark {
            rule: Rule::Author {
                name: Text::EndsWith("[bot]"),
                email: Text::Any,
            },
            fixture: "bot-author",
            seen: Seen {
                repository: "actions/setup-node",
                commit: "436c65619b12c9e37ff20a4700145cc387898a7d",
                line: "dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>",
            },
        }],
    },
];

/// The signature with this id.
pub fn by_id(id: &str) -> Option<&'static Signature> {
    TABLE.iter().find(|signature| signature.id == id)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn marks() -> impl Iterator<Item = (&'static Signature, &'static Mark)> {
        TABLE
            .iter()
            .flat_map(|signature| signature.marks.iter().map(move |mark| (signature, mark)))
    }

    #[test]
    fn ids_and_fixture_names_are_not_used_twice() {
        let mut ids = HashSet::new();
        for signature in TABLE {
            assert!(ids.insert(signature.id), "{} is there twice", signature.id);
            assert!(!signature.marks.is_empty(), "{} has no mark", signature.id);
            assert!(!signature.tool.is_empty());
            assert_eq!(
                by_id(signature.id).map(|found| found.id),
                Some(signature.id)
            );
        }
        assert!(by_id("no-such-tool").is_none());

        let mut fixtures = HashSet::new();
        for (signature, mark) in marks() {
            assert!(
                !mark.fixture.is_empty() && fixtures.insert(mark.fixture),
                "{}: the fixture name {:?} is empty or used twice",
                signature.id,
                mark.fixture
            );
        }
    }

    /// A commit's signatures are listed agents first, and the summary
    /// relies on it.
    #[test]
    fn agents_come_before_bots() {
        let first_bot = TABLE
            .iter()
            .position(|signature| signature.kind == Kind::Automation)
            .unwrap_or(TABLE.len());
        assert!(
            TABLE[first_bot..]
                .iter()
                .all(|signature| signature.kind == Kind::Automation)
        );
        assert!(first_bot > 0, "there are agents");
    }

    #[test]
    fn every_mark_names_a_commit_it_was_seen_in() {
        for (signature, mark) in marks() {
            let seen = &mark.seen;
            let (owner, name) = seen.repository.split_once('/').unwrap_or_else(|| {
                panic!("{}: {:?} is not owner/name", signature.id, seen.repository)
            });
            assert!(!owner.is_empty() && !name.is_empty() && !name.contains('/'));
            assert!(
                seen.commit.len() == 40
                    && seen
                        .commit
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
                "{}: {:?} is not a full commit id",
                signature.id,
                seen.commit
            );
            assert!(!seen.line.is_empty(), "{}: no line is quoted", signature.id);
        }
    }

    #[test]
    fn the_version_is_a_year_a_month_and_a_number() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "{VERSION}");
        assert!(
            parts.iter().all(|part| part.parse::<u32>().is_ok()),
            "{VERSION}"
        );
    }
}
