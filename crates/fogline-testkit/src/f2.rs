//! F2 "agents": one commit for each mark in the engine's table of agent
//! signatures, and the cases that are easy to get wrong.
//!
//! Every commit is named by the first line of its message, and the table
//! [`COMMITS`] says how each has to be read. The engine's table names the
//! commit that carries each of its marks, and a test holds the two
//! together.
//!
//! Commit number k, counted from 1, adds one new file of k lines. So every
//! commit adds a different number of lines, and a sum over a wrong set of
//! commits comes out wrong.

use std::io;

use crate::{Fixture, Identity, RepoBuilder};

/// The commit id F2 must have on every machine.
pub const HEAD: &str = "edd20c58e5add46905020d01894bcc3b8c021fba";

/// How a commit has to be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    AgentAttributed,
    Automation,
    Unsigned,
}

/// One commit of the fixture.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    /// The first line of the message.
    pub name: &'static str,
    pub class: Class,
    /// The ids of the signatures that decide the class, in the order of the
    /// engine's table.
    pub signatures: &'static [&'static str],
    /// The lines the commit adds: its number, and 0 for the merge.
    pub added: u64,
}

/// A person.
const ADA: Identity = Identity {
    name: "Ada Example",
    email: "ada@example.com",
};

/// The same person as Aider signs for her: with its name added to hers.
const ADA_WITH_AIDER: Identity = Identity {
    name: "Ada Example (aider)",
    email: "ada@example.com",
};

/// The accounts of GitHub apps, with the addresses GitHub gives them.
const CLAUDE_APP: Identity = Identity {
    name: "claude[bot]",
    email: "209825114+claude[bot]@users.noreply.github.com",
};
const CURSOR_APP: Identity = Identity {
    name: "cursor[bot]",
    email: "206951365+cursor[bot]@users.noreply.github.com",
};
const COPILOT_AGENT: Identity = Identity {
    name: "copilot-swe-agent[bot]",
    email: "198982749+Copilot@users.noreply.github.com",
};
const JULES: Identity = Identity {
    name: "google-labs-jules[bot]",
    email: "161369871+google-labs-jules[bot]@users.noreply.github.com",
};
const DEVIN: Identity = Identity {
    name: "devin-ai-integration[bot]",
    email: "158243242+devin-ai-integration[bot]@users.noreply.github.com",
};
const OPENCODE: Identity = Identity {
    name: "opencode-agent[bot]",
    email: "219766164+opencode-agent[bot]@users.noreply.github.com",
};
const DEPENDABOT: Identity = Identity {
    name: "dependabot[bot]",
    email: "49699333+dependabot[bot]@users.noreply.github.com",
};
const ACTIONS: Identity = Identity {
    name: "github-actions[bot]",
    email: "41898282+github-actions[bot]@users.noreply.github.com",
};

const CLAUDE: &str = "Co-Authored-By: Claude Fable 5.2 <noreply@anthropic.com>";
const CLAUDE_MARKER: &str =
    "\u{1f916} Generated with [Claude Code](https://claude.com/claude-code)";
const CURSOR: &str = "Co-authored-by: Cursor <cursoragent@cursor.com>";

/// A commit on the main line: its name, its author, what follows the first
/// line of its message, and how it has to be read.
struct Step {
    name: &'static str,
    who: Identity,
    rest: &'static [&'static str],
    class: Class,
    signatures: &'static [&'static str],
}

const fn agent(
    name: &'static str,
    who: Identity,
    rest: &'static [&'static str],
    signatures: &'static [&'static str],
) -> Step {
    Step {
        name,
        who,
        rest,
        class: Class::AgentAttributed,
        signatures,
    }
}

const fn unsigned(name: &'static str, rest: &'static [&'static str]) -> Step {
    Step {
        name,
        who: ADA,
        rest,
        class: Class::Unsigned,
        signatures: &[],
    }
}

/// The commits of the main line, oldest first. `rest` is the lines of the
/// message after its first line and the empty line that follows it.
const STEPS: [Step; 36] = [
    unsigned("start", &[]),
    // One commit for each mark of the table.
    agent("claude-trailer", ADA, &[CLAUDE], &["claude-code"]),
    agent("claude-marker", ADA, &[CLAUDE_MARKER], &["claude-code"]),
    agent("claude-app-author", CLAUDE_APP, &[], &["claude-app"]),
    agent("cursor-trailer", ADA, &[CURSOR], &["cursor"]),
    agent("cursor-author", CURSOR_APP, &[], &["cursor"]),
    agent(
        "copilot-trailer",
        ADA,
        &["Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>"],
        &["copilot"],
    ),
    agent("copilot-author", COPILOT_AGENT, &[], &["copilot"]),
    agent(
        "codex-trailer",
        ADA,
        &["Co-authored-by: Codex <codex@openai.com>"],
        &["codex"],
    ),
    agent("jules-author", JULES, &[], &["jules"]),
    agent(
        "gemini-trailer",
        ADA,
        &["Co-Authored-By: Gemini 4 Hydrogen <noreply@google.com>"],
        &["gemini"],
    ),
    agent(
        "amp-thread",
        ADA,
        &["Amp-Thread-ID: https://ampcode.com/threads/T-00000000-0000-0000-0000-000000000000"],
        &["amp"],
    ),
    agent(
        "factory-trailer",
        ADA,
        &[
            "Co-authored-by: factory-droid[bot] <138933559+factory-droid[bot]@users.noreply.github.com>",
        ],
        &["factory"],
    ),
    agent(
        "warp-trailer",
        ADA,
        &["Co-Authored-By: Warp <agent@warp.dev>"],
        &["warp"],
    ),
    agent(
        "warp-oz-trailer",
        ADA,
        &["Co-Authored-By: Oz <oz-agent@warp.dev>"],
        &["warp"],
    ),
    agent(
        "kiro-trailer",
        ADA,
        &["Co-Authored-By: Kiro <noreply@kiro.dev>"],
        &["kiro"],
    ),
    agent(
        "junie-trailer",
        ADA,
        &["Co-authored-by: Junie <junie@jetbrains.com>"],
        &["junie"],
    ),
    agent("devin-author", DEVIN, &[], &["devin"]),
    agent(
        "aider-trailer",
        ADA,
        &["Co-authored-by: aider (gpt-5.4) <aider@aider.chat>"],
        &["aider"],
    ),
    agent("aider-author", ADA_WITH_AIDER, &[], &["aider"]),
    agent(
        "opencode-trailer",
        ADA,
        &["Co-authored-by: opencode (glm-5.3) <ai@local>"],
        &["opencode"],
    ),
    agent(
        "opencode-address-trailer",
        ADA,
        &["Co-Authored-By: glm-5.3-flash <noreply@opencode.ai>"],
        &["opencode"],
    ),
    agent("opencode-author", OPENCODE, &[], &["opencode"]),
    agent(
        "cline-trailer",
        ADA,
        &["Co-authored-by: Cline <noreply@cline.bot>"],
        &["cline"],
    ),
    agent(
        "goose-trailer",
        ADA,
        &["Co-authored-by: Goose <opensource@block.xyz>"],
        &["goose"],
    ),
    agent(
        "goose-aaif-trailer",
        ADA,
        &["Co-authored-by: goose <goose@aaif.dev>"],
        &["goose"],
    ),
    Step {
        name: "bot-author",
        who: DEPENDABOT,
        rest: &[],
        class: Class::Automation,
        signatures: &["bot"],
    },
    // The cases that are easy to get wrong.
    //
    // Capitals in the key and in the address.
    agent(
        "mixed-case-key",
        ADA,
        &["CO-AUTHORED-BY: Claude <NoReply@Anthropic.com>"],
        &["claude-code"],
    ),
    // Two agents on one commit. It counts once, and both are recorded, in
    // the order of the table and not of the message.
    agent(
        "two-agents",
        ADA,
        &[CURSOR, CLAUDE],
        &["claude-code", "cursor"],
    ),
    // Two marks of the same tool are one signature.
    agent(
        "marker-and-trailer",
        ADA,
        &[CLAUDE_MARKER, "", CLAUDE],
        &["claude-code"],
    ),
    // A squash merge lists the commits it squashed, with their trailers
    // indented in the middle of its message.
    agent(
        "squash-body",
        ADA,
        &[
            "* Add the parser",
            "",
            "    Co-authored-by: Cursor <cursoragent@cursor.com>",
            "",
            "* Fix its test",
            "",
            "    Co-authored-by: Cursor <cursoragent@cursor.com>",
            "",
            "A last line that is not a trailer.",
        ],
        &["cursor"],
    ),
    // Talk about tools, with the trailer's words in the middle of a
    // sentence. No mark.
    unsigned(
        "mentions-a-tool",
        &[
            "Tried Claude Code and Cursor on this one and wrote it by hand in",
            "the end: the Co-authored-by: Claude <noreply@anthropic.com> line",
            "is left out on purpose.",
        ],
    ),
    // An agent's address under a key that does not name an author.
    unsigned(
        "another-key",
        &["Reviewed-by: Claude <noreply@anthropic.com>"],
    ),
    // An address that only ends like an agent's.
    unsigned(
        "almost-an-address",
        &["Co-authored-by: Mirror <mirror-noreply@anthropic.com>"],
    ),
    // A bot's commit with an agent's trailer: the agent decides.
    agent("bot-with-an-agent", ACTIONS, &[CLAUDE], &["claude-code"]),
    // The last commit before the side branch.
    unsigned("before-the-merge", &[]),
];

/// The commit on the side branch, and the merge that brings it in. A merge
/// carries marks like any commit, and adds no lines.
const SIDE: &str = "side-work";
const MERGE: &str = "merge-with-a-trailer";

/// Every commit of the fixture, oldest first: the main line, the commit on
/// the side branch, and the merge.
pub fn commits() -> Vec<Entry> {
    let mut entries: Vec<Entry> = STEPS
        .iter()
        .zip(1..)
        .map(|(step, number)| Entry {
            name: step.name,
            class: step.class,
            signatures: step.signatures,
            added: number,
        })
        .collect();
    entries.push(Entry {
        name: SIDE,
        class: Class::Unsigned,
        signatures: &[],
        added: STEPS.len() as u64 + 1,
    });
    entries.push(Entry {
        name: MERGE,
        class: Class::AgentAttributed,
        signatures: &["claude-code"],
        added: 0,
    });
    entries
}

/// 2026-03-02 09:00:00 UTC. Each commit is one hour after the one before.
const START: i64 = 1_772_442_000;
const HOUR: i64 = 3_600;

/// A file of `lines` lines for the commit called `name`.
fn add_file(repo: &RepoBuilder, name: &str, lines: u64) -> io::Result<()> {
    let contents: String = (1..=lines).map(|line| format!("{name} {line}\n")).collect();
    repo.write(&format!("notes/{name}.txt"), &contents)
}

/// Builds the fixture in a fresh temporary directory.
pub fn build() -> io::Result<Fixture> {
    let repo = RepoBuilder::new()?;
    let mut time = START;
    let mut number = 0;

    for step in &STEPS {
        number += 1;
        time += HOUR;
        add_file(&repo, step.name, number)?;
        let mut message = format!("{}\n", step.name);
        if !step.rest.is_empty() {
            message.push('\n');
            for line in step.rest {
                message.push_str(line);
                message.push('\n');
            }
        }
        repo.commit(step.who, time, &message)?;
    }

    repo.start_branch("side")?;
    add_file(&repo, SIDE, number + 1)?;
    repo.commit(ADA, time + HOUR, &format!("{SIDE}\n"))?;
    repo.switch("main")?;
    repo.merge(
        ADA,
        time + 2 * HOUR,
        "side",
        &format!("{MERGE}\n\n{CLAUDE}\n"),
    )?;

    repo.finish()
}
