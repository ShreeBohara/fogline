# Phase 1 spec — reset and tracer bullet

Status: in progress; milestone M1 (the skeleton) is built, and M2 (distribution) is built as far as the name lock, task 2.5 · Written 2026-11-15 · Implements the [plan](../plan.md) §8, Phase 1 · Working name: **Fogline**

How to read this:

- **MUST** closes the phase. **SHOULD** is done unless it threatens the timebox. **LATER** is out of this phase on purpose.
- *Measured* means it was run on 2026-11-15 on an Apple M3 Pro (11 cores) with git 2.52.0. *Target* means the phase's own benchmark has to show it. Effort is an estimate in focused days.
- Tool versions quoted are the current releases on 2026-11-15. Lockfiles pin the exact ones.

---

## 1. Outcome

When Phase 1 closes, anyone can run `npx fogline` inside a git repository and, a few seconds later, look at a zoomable map of the codebase colored by how much of each file arrived in commits that carry an AI-agent signature. CI builds and publishes the binary for five targets across macOS, Linux and Windows. Two open decisions are closed with evidence: which renderer draws the map, and whether the engine stays in Rust.

| It proves | It does not attempt |
|---|---|
| The whole path works: repository → facts → bundle → map | Line-level provenance (Phase 2) |
| The plan's "hard part": a native binary delivered through npm on three operating systems | The evidence-of-understanding model (Phase 3) |
| The honesty mechanism: every dataset ships with its known limits, and the viewer shows them | The designed map and visual identity (Phase 4) |
| The working habits: oracle tests, reproducible numbers, written decisions | Any LLM feature, database, plugin or website |

Size: about 12 focused days. Exit checklist: §13.

---

## 2. Decisions this spec makes

Rows marked ⟲ change something the plan said.

| # | Decision | Why |
|---|---|---|
| D1 ⟲ | Build in a **new public repository**. Leave `codebaseqa` untouched apart from tags and a notice. | v1 has a working public demo and a coherent README, and nothing needs deleting. A new product under a new name gets a clean history. Public from the first commit, because the five-runner build matrix is free without limits only for public repositories and npm generates provenance only for public source. |
| D2 ⟲ | Keep every checkout **outside iCloud**. | An iCloud-synced checkout of v1 lost most of its `.git` directory to eviction, including the index file ([ADR-0001](../adr/0001-repository-name-versions-conventions.md)). |
| D3 ⟲ | Packages start at **0.1.0**. The launch release is **1.0.0**. | The redesign was first planned as "CodebaseQA 2.0". A new package whose first stable version is 2.0.0 confuses people, and 0.x tells users the format can still change. |
| D4 | The engine reads history through **git's own plumbing commands**, run in parallel. gitoxide is a timeboxed experiment. | Measured: 11 parallel `git diff-tree` processes produce Django's full line statistics in 2.1 s, byte for byte identical to one process that takes 12.6 s (§11). |
| D5 | Line statistics use the **minimal diff** with **rename detection**. | Measured on Django: git's default heuristic and the minimal diff differ by 47,941 lines (1.3%), so the algorithm has to be pinned. Without rename detection, added lines are inflated by 7.7%. |
| D6 ⟲ | Say **"agent-attributed"**. Commit counts are a lower bound; line shares are estimates that can err both ways. | A signed commit can contain hand-written lines, and an unsigned one can be agent-written. An earlier draft of the plan called the line share "a lower bound". That was wrong. |
| D7 ⟲ | Ship through npm as a small launcher plus **one package per platform**, built on native runners by a hand-written release workflow. Revisit cargo-dist in Phase 3. | Verified: Biome ships this way. All five targets have native GitHub-hosted runners, so nothing is cross-compiled. cargo-dist's npm installer fetches the binary after install and authenticates with a stored token. |
| D8 | The scan **writes nothing** unless `--out` is given, and never inside the scanned repository. | Design rule 7 in the plan. Results are served from memory. |
| D9 | Engine behavior is pinned by **black-box contract tests** that run whatever binary `FOGLINE_BIN` names. | If the language gate (§10) says switch, the tests and golden files carry over unchanged. |
| D10 | The renderer is chosen by a **two-day spike with criteria fixed in advance** (§9). | Avoids choosing whichever candidate was built first. |

---

## 3. Before the build

### 3.1 Steps only the maintainer can take

| What | Needed by | Notes |
|---|---|---|
| Decide the name | Task 2.5, before anything is published | npm and crates.io names are effectively permanent. Findings in §3.2. |
| Create the public GitHub repository | Task 1.1 | |
| npm: an account with 2FA, the scope, the first publish, the trusted publishers | Task 2.6 | npm sets up a trusted publisher from an existing package's settings page, so the first publish of each package is done by hand. No long-lived token is ever stored. |
| crates.io first publish | Task 7.3 (SHOULD) | Per the crates.io RFC, a trusted publisher can only be added after one manual publish. |
| Optional: install git-ai before the first commit | Task 1.1 | The repository would carry exact line-level provenance from day one and become Phase 2's reference fixture. |

### 3.2 Name check

Checked on 2026-11-15.

| | Fogline | Kenmap (fallback) |
|---|---|---|
| npm, crates.io, PyPI | free | free |
| Domains | `.dev`, `.com`, `.io`, `.app` and `.sh` are registered by others; `fogline.tools` looks free | `kenmap.dev` looks free |
| Same-name things | A SaaS consultancy called Fogline Group; 27 GitHub repositories, all at zero stars | Nothing found in one search |
| Fit | Matches the visual: unknown code drawn as fog | Literal: a map of one's "ken", the range of what one knows |

Fogline stays unless a `.dev` domain turns out to matter. The npm scope `@fogline` could not be checked without an account. If it is taken, the platform packages use unscoped names.

---

## 4. The slice

```
npx fogline                       npm launcher (a small script with no dependencies)
  └─ fogline                      Rust binary for this platform
       scan   ── git plumbing, read-only ──▶ facts in memory
                                              │
                                        bundle v0 (three JSON files)
                                              │
       serve  127.0.0.1:<port>/<token>/ ◀─────┘     viewer assets are embedded in the binary
                  │
                  ▼
       browser: the viewer fetches ./bundle/*, lays out a treemap, draws it, lists the caveats
```

Repository layout. Only what Phase 1 uses; `apps/site`, `plugins/`, `action/` and `evals/` are created by the phase that first needs them.

```
Cargo.toml  rust-toolchain.toml  deny.toml  package.json  pnpm-workspace.yaml  biome.json  .nvmrc
crates/
  fogline-core/        scan, attribution, bundle types and writer; never prints, never exits
    data/signatures.toml
  fogline/             the binary: CLI, local server, browser launch, export
    assets/            built viewer (ignored by git) and a committed stub page
    tests/contract.rs  black-box tests that run the binary named by FOGLINE_BIN
  fogline-testkit/     fixture repositories and golden-file helpers
xtask/                 developer commands: ci, viewer, schema, synth, bench
packages/viewer/       React package and standalone shell; spike/ holds the renderer candidates
npm/fogline/           launcher package; its lib/platforms.js is the one table of platforms
scripts/npm-pack.mjs   turns release binaries into publishable npm folders
scripts/pack-smoke.mjs installs those folders into an empty project and runs the result
schemas/bundle-v0.schema.json
tests/contract/        golden bundles the contract tests compare against (from M3)
bench/corpus.toml      pinned benchmark repositories
docs/                  plan.md, specs/, adr/, reports/
.github/workflows/     ci.yml, release.yml
README.md  LICENSE (MIT)  CONTRIBUTING.md  SECURITY.md  CHANGELOG.md  CLAUDE.md
```

Toolchain: Rust 1.100.0 (edition 2024), Node 26 LTS, pnpm 12, TypeScript 7, Vite 8, React 19.3, Vitest 5, Playwright 1.65, Biome 2.6.

Rust crates: clap 4.6, axum 0.8 on tokio, rust-embed 8, serde with schemars, regex, toml, thiserror and anyhow, indicatif, open. Tests use insta, assert_cmd, proptest and tempfile. Nothing else without a line in the pull request saying why.

Building without Node works: the binary then embeds the stub page, which says the viewer was not built.

---

## 5. Contracts

### 5.1 Command line

```
fogline [PATH]                          same as: fogline open PATH
fogline open   [PATH] [--port <n>] [--no-browser] [--json]
fogline scan   [PATH] [--out <dir>] [--json]   prints a summary; --out also writes the bundle
fogline export [PATH] --out <dir>       static folder: viewer + bundle          (SHOULD)
fogline version [--json]                also: --version

scan options (accepted by open and export too):
  --history <all|none>    default all
  --jobs <n>              1 to 64; default min(cores, 12)
global options:
  --quiet   --verbose   --color <auto|always|never>   --timings
```

Rules:

1. `PATH` defaults to the current directory. Any directory inside a work tree resolves to the repository root. Bare repositories work, and so does a path inside a `.git` folder.
2. Progress and messages go to stderr. stdout carries only data that was asked for (`--json`, `version`).
3. Exit codes: `0` success · `1` unexpected failure · `2` usage error, including a `PATH` that does not exist or is not a folder · `3` not a git repository, no commits yet, or a partial clone that lacks objects the scan needs · `4` git not found, or older than 2.25.
4. The scan never writes inside the repository, writes nowhere at all unless `--out` is given, and makes no network requests. Tests enforce all three (§7).
5. `open` prints the URL, opens the default browser unless `--no-browser` or the `CI` variable is set, and serves until Ctrl-C.
6. An error says what happened and what to do next. Example: `fogline: /tmp/x is not inside a git repository. Run it from a project folder, or pass a path.`

Illustrative output of `fogline` on a large repository with no agent signatures:

```
fogline 0.1.0 · django @ f24f38c (main)
  files     7,102 (1,122,175 lines, 1,379 binary)
  history   35,123 commits (591 merges), 3,378,909 lines added, 2,253,367 deleted
  agents    no agent signatures found; showing churn instead
  map       http://127.0.0.1:53412/9f3c…/      Ctrl-C to stop
```

The summary does not depend on `--jobs`: a contract test compares it for 1, 4, 11 and 64. `--timings` adds how long each line took and how many git processes read the history.

### 5.2 What the scan computes

**Unit of analysis.** The commit HEAD points to. Uncommitted changes are ignored, and the viewer says so.

**Files.** Every blob in HEAD's tree. Submodule entries are skipped and counted. Symbolic links are listed with zero lines and flagged.

| Per file | Definition |
|---|---|
| `bytes` | Blob size |
| `lines` | Number of line terminators, plus one if the file is not empty and does not end with one. −1 for binary files. Binary means what git means: a NUL byte in the first 8,000 bytes, or marked binary in `.gitattributes`. |
| `lang` | From a table of file extensions and a few whole file names (`crates/fogline-core/src/classify.rs`); otherwise unknown. One extension has one language. |
| `flags` | binary · generated (lockfiles by name, `*.min.*`, average line longer than 500 bytes) · vendored (`vendor/`, `third_party/`, `node_modules/` in the path) · symlink |

**History.** All commits reachable from HEAD.

- For each non-merge commit and each path it touches: `added` and `deleted`. With the old file O and the new file N split into lines, `deleted = |O| − LCS(O, N)` and `added = |N| − LCS(O, N)`, where LCS is their longest common subsequence of lines. By definition this does not depend on the tool that computes it. Git 2.50 and newer report exactly this with the minimal diff algorithm, and a test computes it a second way, from both versions of each file. An older git first marks some lines as changed, as a shortcut, and can then report a few lines more on both sides; the scan says so (caveat `diff_not_minimal`). Git's own `xdiff/xprepare.c` skips the shortcut for the minimal diff at the v2.50.0 tag and not at v2.49.0, and the commit that changed it (03f2915541) found shorter diffs in about 1.3% of the diffs in git's own history.
- A rename, detected by git at 50% similarity, counts only the lines that changed inside the file. Copies are not detected. In a commit where more than 1,000 files could pair up, git pairs only the ones that are exactly alike.
- Merge commits carry no line statistics of their own.
- A change to a submodule entry is left out, because the entry is not a file. A change to a symbolic link counts the one line git shows for it, the path it points to.
- Per file, summed over history: `commits`, `added`, `deleted`, `authors` (distinct author emails, as the commits record them), `first_at` and `last_at` (the earliest and the latest author date), and the agent and automation splits below. History before a rename is carried to the file's current path (SHOULD; if it is not done, a caveat says so).
- `.mailmap` is not applied. Git reads that file from the work tree, so an uncommitted edit to it would change the count of authors. Measured with git 2.52: a `.mailmap` that was never committed changed what `%aE` printed for six of ten commits.

**Attribution** is per commit:

- **agent-attributed** — an agent signature matches (§5.4);
- **automation** — a bot wrote it and no agent signature matches;
- **unsigned** — everything else. Unsigned does not mean hand-written.

A file's *agent-attributed share* is the added lines from agent-attributed commits divided by all added lines.

**Wording rule for every surface.** Say "agent-attributed", never "AI-written". Commit counts are stated as "at least N". Line shares are estimates: they undercount when agents leave no signature and overcount when a signed commit includes hand-written lines. CodebaseQA's history shows the first effect: 17 of its first 51 commits carry a Claude co-author trailer, while the project's own notes describe AI assistance throughout.

**Caveats.** The manifest lists the ones that apply, by id. The viewer shows the text.

| id | Shown as |
|---|---|
| `head_only` | Shows the committed state at this commit. Uncommitted changes are not included. |
| `commit_level_attribution` | A commit with an agent signature counts all of its added lines as agent-attributed, including any a person wrote. |
| `signatures_undercount` | Agents that leave no signature are invisible here. Read commit counts as "at least". |
| `added_not_surviving` | Shares count lines added over history, not the lines that survive today. Line-level tracking comes in a later version. |
| `merges_have_no_stats` | Changes made while resolving merges are not counted. |
| `renames_by_similarity` | Renames are detected by similarity. A heavily edited rename counts as a new file. |
| `history_before_rename_dropped` | History from before a file was renamed is missing. (Only if task 3.5 is not done.) |
| `shallow_clone` | This clone holds only part of the history. |
| `lossy_paths` | Some paths are not valid UTF-8 and were converted. |
| `attributes_from_work_tree` | This git is older than 2.41, so which files count as binary follows the `.gitattributes` in the work tree, not the one in the commit. |
| `diff_not_minimal` | This git is older than 2.50, so for some changes it counts a few more added and deleted lines than the change needs. |
| `history_off` | History was not read (`--history none`). |

### 5.3 How git is called

All repository access in Phase 1 goes through the `git` executable, version 2.25 or newer. 2.25.0 is the first release with `rev-parse --show-object-format`: git's own `builtin/rev-parse.c` has it at the v2.25.0 tag and not at v2.24.0.

| Purpose | Command |
|---|---|
| Repository facts | `git rev-parse --is-bare-repository --is-inside-git-dir --is-shallow-repository --show-object-format`, run in `PATH` |
| Repository root | `git rev-parse --show-toplevel`, or `git rev-parse --absolute-git-dir` for a bare repository or a path inside `.git`, where there is no work tree. Every later command runs at the root. |
| HEAD | `git rev-parse --verify --quiet HEAD^{commit}` (exit 1 means no commits yet) and `git symbolic-ref --quiet --short HEAD` (exit 1 means HEAD is detached) |
| Files and sizes | `git ls-tree -r -l -z <head>` |
| Line counts | `git diff --numstat -z --no-renames --no-textconv --no-ext-diff <empty-tree> <head>`, with `GIT_ATTR_SOURCE=<head>` |
| Commit metadata | `git log --topo-order -z --no-show-signature --format=%H%x00%P%x00%at%x00%an%x00%ae%x00%B <head>` |
| Commit order | The order `git log` printed. The commits that are not merges are cut into K runs of neighbours, as equal in length as they can be. |
| Line statistics | K workers, each `git diff-tree --stdin --always -r --root -M50% -l1000 --diff-algorithm=minimal --numstat -z --no-textconv --no-ext-diff --ignore-submodules=all`, with `GIT_ATTR_SOURCE=<head>`, fed one run on stdin |
| Missing objects, in a partial clone only | `git rev-list --objects --no-object-names --missing=print <head>`, with `--no-walk` before the files are listed and without it before history is read |

Rules:

- Every call sets `GIT_OPTIONAL_LOCKS=0`, `GIT_TERMINAL_PROMPT=0`, `LC_ALL=C`, and passes `-c core.quotepath=false -c i18n.logOutputEncoding=UTF-8`. The flags above are explicit so the user's git configuration cannot change the numbers.
- Every call also sets `GIT_ATTR_NOSYSTEM=1` and passes `-c core.attributesFile=/dev/null`, so the attributes files of the machine and of the user are not read. Measured with git 2.52 before this rule existed: one line, `*.py binary`, in `~/.config/git/attributes` turned the line counts of three more files of a seven-file repository into `-`. Git reads `/dev/null` as an empty file on Windows too: its `compat/mingw.c` maps that name.
- `git log` gets `--no-show-signature`. With the setting `log.showSignature` on, git prints lines about a signed commit's signature into the output before the commit. Measured with git 2.52 and one commit signed with an SSH key: the line `No signature` appeared in front of the commit's id, and the record grew from 121 to 134 bytes.
- Every call removes `GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES` and `GIT_NAMESPACE`. Each would make git read a repository other than the one the scan was pointed at, and git sets some of them itself while it runs hooks.
- "Not a git repository" is recognised by that phrase in git's error message, which `LC_ALL=C` keeps in English. Any other failure is reported with git's own message and exit code 1.
- `<head>` is the commit id that discovery read, never the name `HEAD`. Every command of a scan then reads the same commit, even if someone commits while it runs.
- `<empty-tree>` is the id printed by `git hash-object -t tree --stdin` on empty input. That command computes the id without writing an object.
- `.gitattributes` can declare a file binary, which removes its line count. Left alone, git reads that file from the work tree: an uncommitted edit changes the numbers, and a bare repository ignores the file. Measured with git 2.52: a text file marked `binary` shows `-` in a work tree and 2 lines in a bare clone of the same commit. `GIT_ATTR_SOURCE=<head>` makes git read it from the commit instead. Git has had that variable since 2.41.0: its own `git.c` knows `--attr-source` at the v2.41.0 tag and not at v2.40.0. An older git ignores the variable, and the scan then records the caveat `attributes_from_work_tree`.
- Known limit: a repository's own `info/attributes` file still applies, on every git. Git has no switch to ignore it.
- Known limit: when the commit's `.gitattributes` gives a file a diff driver by name (`*.dat diff=mine`), a user's setting `diff.mine.binary=true` still makes that file binary. Measured with git 2.52. The driver's other settings are covered: `--no-textconv`, `--no-ext-diff` and `--diff-algorithm` are given on the command line.
- A partial clone, made with `git clone --filter`, leaves objects on its remote, and git downloads one whenever a command asks for it. That is a network request and a write into the repository, so the scan prevents it twice. Measured with git 2.52 before this rule existed: `ls-tree -r -l` in a blobless clone with no checkout downloaded three blobs and added three packs.
  - Every call sets `GIT_NO_LAZY_FETCH=1`. Git has honoured it since 2.45.0: its own `environment.h` names the variable at the v2.45.0 tag and not at v2.44.0. Six maintenance releases of older lines honour it too: git's `promisor-remote.c` reads the variable at the v2.39.4, v2.40.2, v2.41.1, v2.42.2, v2.43.4 and v2.44.1 tags, and not at the release before each. A git without it, such as 2.25.1, ignores the variable, so the scan does not rely on it.
  - Discovery reads the configuration for a promisor remote (`git config -z --get-regexp` for `extensions.partialclone` and `remote.*.promisor`). In such a clone the scan first runs `git rev-list --objects --no-object-names --missing=print --no-walk <head>`, which lists missing objects instead of downloading them. Its code turns downloading off, and already did at v2.25.0, so this is safe on every supported git. If anything is missing, the scan stops with exit code 3 and says that it never downloads.
  - Counting the lines of history needs every version of every file, and the usual partial clone holds only the newest. So before history is read the same command runs without `--no-walk`, over every commit. Measured with git 2.52 in a clone made with `--filter=blob:none`: nothing missing for the commit, and three earlier versions missing for a ten-commit history. With `GIT_NO_LAZY_FETCH` unset, which is how a git that does not know the variable behaves, the workers downloaded them and added three packs. The scan stops with exit code 3 and names `--history none`, which scans the files alone.
  - A partial clone that has everything the scan needs is scanned as usual.
- No command that writes is ever used: no `status`, no `mktree`, nothing that refreshes the index.

Output shapes, verified today by reading the raw bytes:

| Token | Shape with `-z` |
|---|---|
| Commit id line from `diff-tree --stdin` | `<sha>` NUL. With `--always`, a commit that changed nothing prints this line and nothing else. |
| Normal record | `<added>` TAB `<deleted>` TAB `<path>` NUL |
| Rename record | `<added>` TAB `<deleted>` TAB NUL `<old path>` NUL `<new path>` NUL |
| Binary file | `-` for both counts |
| Merge commit | never sent to `diff-tree`, so it has no records by construction |
| Metadata record | six fields, each ended by NUL: id, parents, author date, author name, author email, message |

A path is never taken for a commit id, even one that looks like one: a path comes after two tabs, or right after a rename's numbers, where the next two tokens are read as paths whatever they hold.

NUL ends every field of the metadata because it is the one byte no field can hold. Any other byte can be inside a name, an email or a message. Measured with git 2.52: a commit whose author name has the control character 0x1F in the middle keeps that name as it is, and a format with 0x1F between its fields then cut the name in two, took its second half for the email, and put the real email into the message. Git handles a commit's text as a string that ends at the first NUL. Measured with git 2.52 on commits stored by hand: with a NUL in the message, the message ends there; with a NUL in the author line, the date, the name and the email are empty; and each commit still prints its six fields. Two tests repeat both measurements with the git that runs them. The parser checks the shape as well: the number of fields has to be a multiple of six, and every id, parent and date has to look like one, so output with a field too many or too few is refused.

Every commit that is sent has to come back. `diff-tree --stdin` skips what it cannot use and still exits with 0. Measured with git 2.52 in one run: an id that names no object printed nothing, a last line without a line break was ignored, a line that is not an id was copied to the output, and git exited with 0. Without `--always` a commit that changed nothing prints nothing either, so a skipped commit could not be told from an empty one. With it, the parser checks that the ids in the output are exactly the ids that were sent, in that order, and anything else is an error.

Parallel runs are exact, not approximate. Measured on Django with the exact command above: the output of 11 workers, joined in the order of the runs, equals the single-process output byte for byte (8,104,685 bytes, 154,174 records, 2,398 of them renames).

The work is cut once, into K runs. A queue of smaller runs, handed to K processes at a time, would even out the load, and it was slower: 2.6 s with 35 runs and 3.0 s with 346, against 2.2 s with 11 (Django, 11 processes). A git process is fastest on commits that are neighbours, where it has just read most of what the next one needs.

The K processes are started one after another. Starting a process needs several open files for a moment, and macOS gives a process 256 unless the user raised it. Measured: 64 processes started at the same time ran out of them, and started in turn they did not.

### 5.4 Agent signatures

The table lives in `crates/fogline-core/data/signatures.toml`, is embedded at build time, and carries its own version (`2026.11.1`), which the manifest records.

```toml
[[signature]]
id = "claude-code"
kind = "agent"                 # agent | automation
tool = "Claude Code"
trailer = { key = "co-authored-by", email = '^noreply@anthropic\.com$' }
evidence = "GitHub commit search, 2026-11-15: about 50.6M commits"
fixture = "claude-trailer"
```

Rule types: `trailer` (key, optional name pattern, optional email pattern), `author` (name or email pattern), `message` (pattern on any line).

Matching:

- Trailer keys are case-insensitive. A trailer is any line of the form `Key: value`, anywhere in the message. Squash merges copy the trailers of the commits they squash into the message body; a real commit found today has dozens of `Co-authored-by: Cursor` lines that way.
- An agent match beats an automation match. Every matching id is recorded, and the commit counts once.
- No signature ships without a real commit behind it: repository, SHA and the matching line are stored under `tests/fixtures/signatures/`.

Seed table. Counts are GitHub commit-search totals on 2026-11-15: approximate, public default branches only.

| id | Rule | Seen |
|---|---|---|
| `claude-code` | trailer email `noreply@anthropic.com` (the name carries the model, e.g. `Claude Fable 5.2`) | ≈ 50.6M |
| `claude-code-marker` | message contains `Generated with Claude Code` | ≈ 6.7M |
| `claude-app` | author `claude[bot]` | in a real repository |
| `cursor` | trailer email `cursoragent@cursor.com`; author `cursor[bot]` | ≈ 6.6M; ≈ 206k |
| `copilot` | trailer name `Copilot…` with email ending `+Copilot@users.noreply.github.com`; author `copilot-swe-agent[bot]` | ≈ 2.6M; ≈ 3.7M |
| `codex` | trailer `Codex <codex@openai.com>` | ≈ 649k |
| `jules` | author `google-labs-jules[bot]` | ≈ 1.29M |
| `gemini` | trailer name starting `Gemini` with email `noreply@google.com` | ≈ 359k |
| `amp` | trailer key `Amp-Thread-ID` | ≈ 186k |
| `factory` | trailer name `factory-droid[bot]` | ≈ 163k |
| `warp` | trailer `Warp <agent@warp.dev>` | ≈ 55k |
| `kiro` | trailer email `noreply@kiro.dev` | ≈ 16.6k |
| `devin` | author `devin-ai-integration[bot]` | in a real repository |
| `bot` (automation) | author name ends with `[bot]` and no agent rule matched, e.g. `dependabot[bot]`, `github-actions[bot]` | in real repositories |

To verify in task 3.4 before adding: aider, opencode, Windsurf, Cline, Junie, Goose. GitHub's search rate limit cut the first pass short.

### 5.5 Bundle v0

| File | Content | Deterministic |
|---|---|---|
| `manifest.json` | Identity, options, totals, caveat ids, checksums of the other files | yes |
| `files.json` | One column per attribute, one entry per file, sorted by path | yes |
| `timeline.json` | Monthly totals for the header sparkline (SHOULD) | yes |
| `run.json` | Timings, host, git version | no; the only file allowed to differ between runs |

`manifest.json`, with illustrative values:

```json
{
  "schema": "fogline.bundle/0",
  "engine": { "name": "fogline", "version": "0.1.0", "signatures": "2026.11.1" },
  "repo": {
    "name": "example",
    "head": { "sha": "3f9c2a…", "ref": "main", "committed_at": 1794700000 },
    "object_format": "sha1",
    "shallow": false,
    "submodules_skipped": 0
  },
  "options": { "history": "all", "diff": "minimal", "renames": "M50" },
  "totals": {
    "files": 412, "binary_files": 9, "lines": 58211,
    "commits": 1853, "merge_commits": 40,
    "commits_agent": 1423, "commits_automation": 30,
    "added": 240112, "added_agent": 201845, "added_automation": 512,
    "by_signature": [{ "id": "claude-code", "commits": 1411, "added": 200903 }]
  },
  "caveats": ["head_only", "commit_level_attribution", "signatures_undercount", "added_not_surviving", "merges_have_no_stats", "renames_by_similarity"],
  "files": { "href": "files.json", "sha256": "…" },
  "timeline": { "href": "timeline.json", "sha256": "…" }
}
```

`files.json` is a set of equal-length arrays:

| Column | Meaning |
|---|---|
| `n`, `langs` | Number of files; language names |
| `path` | Repository-relative, `/` separators, sorted bytewise |
| `bytes`, `lines` | As in §5.2; `lines` is −1 for binary files |
| `lang` | Index into `langs`; −1 unknown |
| `flags` | Bit 0 binary · 1 generated · 2 vendored · 3 symlink |
| `commits`, `commits_agent` | Commits touching the file |
| `added`, `added_agent`, `added_automation`, `deleted` | Line statistics over history |
| `authors` | Distinct author emails (a count only) |
| `first_at`, `last_at` | Unix seconds; 0 when there is no history |

`timeline.json`: `{ "bucket": "month", "t": ["2026-01", …], "commits": […], "commits_agent": […], "added": […], "added_agent": […] }`, bucketed by author date in UTC.

Rules:

- Integers only. Key order and number formatting are fixed, so equal inputs give equal bytes.
- No remote URL (it can hold credentials) and no author names or emails (bundles get published).
- The JSON Schema is generated from the Rust types with schemars, and the TypeScript types are generated from the schema. CI fails when either is stale.
- The viewer checks the `schema` string and the column lengths and shows a clear error on mismatch.

### 5.6 Local server

| Route | Serves |
|---|---|
| `/<token>/` | The viewer's `index.html` |
| `/<token>/assets/*` | Embedded viewer files |
| `/<token>/bundle/*` | The bundle, from memory |
| anything else | 404 |

- Binds `127.0.0.1` on a port chosen by the OS; `--port` overrides.
- `<token>` is 128 random bits, new on every run.
- A request whose `Host` header is not `127.0.0.1:<port>` or `localhost:<port>` gets 403. This blocks DNS-rebinding attacks from web pages.
- GET and HEAD only. No CORS headers. `Cache-Control: no-store`, `X-Content-Type-Options: nosniff`, and a Content-Security-Policy that allows same-origin resources and `blob:` workers only.
- The static export uses the same relative URLs, so the viewer has one data path.

### 5.7 Viewer v0

MUST:

1. Load the bundle, validate it, and show a readable error screen if it does not match.
2. Draw a treemap. Area is lines; generated and binary files are left out of the area. Directories are nested groups, labeled on the top two levels. A file's label appears once its cell is large enough.
3. Two layers, on keys `1` and `2`: agent-attributed share, and churn (commits, log scale). It starts on the first; if the repository has no agent-attributed commits it starts on churn and says why.
4. Pan, zoom about the cursor, double-click a directory to zoom to it, `0` to reset.
5. Hover shows path, lines, share, commits and last change. Click pins a side panel with the same facts and a copy-path button.
6. Header: repository name, short SHA, files, lines, commits scanned, and "at least N agent-attributed commits".
7. "About this data" lists every caveat in the manifest in plain language, one click away at all times.
8. A legend with units, a colorblind-safe ramp, a distinct "no data" color, and light or dark from the OS setting.
9. `window.__fogline.ready` and a `fogline:ready` event, for tests and benchmarks.

SHOULD: a table of the 20 files with the most agent-attributed lines, which is also the accessible alternative to the map; the monthly sparkline; layer and focus in the URL.

LATER: code view, search, time slider, dependency edges, the design system.

Engineering rules:

- The renderer sits behind one interface (`mount`, `setData`, `setColors`, `flyTo`, `pick`, `destroy`), so the spike candidates and the final choice are interchangeable.
- Camera state lives outside React.
- The page requests nothing except its own assets and the bundle. A Playwright assertion checks it.
- Styling is one hand-written stylesheet with tokens as custom properties under a `.fogline` root. The styling system for an embeddable viewer is a Phase 4 decision.
- JavaScript budget: 700 kB gzipped (target; depends on the renderer).

### 5.8 npm packaging

- **`fogline`** — `bin/fogline.js` (CommonJS, no dependencies, Node 18 or newer), and `optionalDependencies` pinning the five platform packages to the same exact version. No install scripts, so `--ignore-scripts` works.
- **`@fogline/cli-<os>-<cpu>`** — one executable and a `package.json` with `os` and `cpu`, so npm installs only the matching one. It has no `bin` entry: the launcher owns the `fogline` command.
- **One source for each fact** — the version is written only in `Cargo.toml`. The launcher and the pack script read the platforms from one table, `npm/fogline/lib/platforms.js`. The launcher's `package.json` in the repository is a template: it is marked private and has no version, so that folder cannot be published by mistake. `scripts/npm-pack.mjs` writes the published manifests from those two sources, so the six packages cannot disagree. The workflow matrices and `deny.toml` cannot read the table, so they repeat the five targets, and a test fails when a copy differs from it.
- **Launcher logic** — honor `FOGLINE_BINARY` if set; otherwise resolve the platform package; spawn it with inherited stdio; pass SIGINT, SIGTERM and SIGHUP on to it; exit with its code. If no package matches (unsupported platform, or `--omit=optional`), list the supported platforms and the other ways to install, and exit 1. A binary that cannot be started also exits 1.
- **When a signal ends the binary** — the launcher ends by the same signal if it is one that only stops a program (SIGINT, SIGTERM, SIGHUP, SIGKILL), so a shell sees what happened. Any other signal means the binary crashed. Raising it again would make Node write a core dump about itself, so the launcher prints which signal it was and exits with 128 plus the signal's number.

| Target | Runner label | Package |
|---|---|---|
| `aarch64-apple-darwin` | `macos-latest` | `@fogline/cli-darwin-arm64` |
| `x86_64-apple-darwin` | `macos-15-intel` | `@fogline/cli-darwin-x64` |
| `x86_64-unknown-linux-musl` | `ubuntu-latest` | `@fogline/cli-linux-x64` |
| `aarch64-unknown-linux-musl` | `ubuntu-24.04-arm` | `@fogline/cli-linux-arm64` |
| `x86_64-pc-windows-msvc` | `windows-latest` | `@fogline/cli-win32-x64` |

- Linux binaries are static musl builds, so one binary runs on every distribution, Alpine included, with no glibc version floor.
- The Windows binary links the C runtime statically, so no Visual C++ redistributable is needed.
- LATER: Windows on Arm (`windows-11-arm`), Homebrew, a shell installer.

Traps to design for: GitHub's artifact upload drops the executable bit, so archive the binaries or `chmod` before packing. Windows cannot record that bit at all, so the Unix packages are packed on macOS or Linux, and the pack script refuses otherwise. Publish the platform packages before the launcher. Make the publish step safe to re-run.

Known limit: Rust 1.100.0 cannot strip binaries on macOS. Its bundled `rust-objcopy` fails to load `libLLVM.dylib`, so `strip = true` only produces a warning there and the two macOS binaries ship about 200 KB larger. Measured on 2026-11-15 for `aarch64-apple-darwin`: `cargo build --release -p fogline` gives 879,624 bytes, and running the system `strip` on a copy gives 680,128. The nightly toolchain already has the fix. Nothing is worked around; the limit goes away when `rust-toolchain.toml` moves to a release that has it.

---

## 6. Work breakdown

Each task ends with a check that is a command or an observable fact.

### M0 · Clean ground — 0.5 day

| # | Task | Check |
|---|---|---|
| 0.1 ✓ | Clone v1 fresh into a folder iCloud does not sync, and start this repository outside iCloud too. | Done 2026-11-15: the clone is clean and at `9b6fdd8` |
| 0.2 | Toolchain: Node 26, pnpm 12, and rustup's Rust first on PATH | `node -v` → v26; `rustc -V` → 1.100.0 |
| 0.3 | Close out v1: tag `v1.0.0` at `263ce72` (2026-02-19) and `v1.1.0` at `9b6fdd8` (2026-10-27), and publish a GitHub release. Keep the demo running. | `gh release view v1.1.0 -R ShreeBohara/codebaseqa` |

### M1 · Skeleton — 1 day

| # | Task | Check |
|---|---|---|
| 1.1 ✓ | New public repository with the layout in §4, MIT licence, an honest pre-alpha README, `CLAUDE.md`, and the plan and this spec under `docs/` | The first commits hold the docs and the skeleton |
| 1.2 ✓ | `fogline version`; a `scan` that fails with "not implemented"; a viewer shell that renders "no bundle". Embedding the viewer in the binary moved to task 5.2, where `open` first serves it. | `cargo run -p fogline -- version --json` |
| 1.3 ✓ | Test kit: a fixture builder with pinned dates and identities, the contract harness driven by `FOGLINE_BIN`, fixture F1 | The fixture's HEAD SHA equals the recorded constant on all three OSes |
| 1.4 | `ci.yml` (§8) | A pull request shows every job green; branch protection requires them |
| 1.5 ✓ | ADR-0001: repository, name, versioning, conventions | Merged |

### M2 · Distribution first — 2 days

The riskiest part goes first, while the binary is still trivial.

| # | Task | Check |
|---|---|---|
| 2.1 ✓ | Launcher package with `node:test` tests: platform table, missing package, exit code, signal | Pass on Node 18 and 26 |
| 2.2 ✓ | `scripts/npm-pack.mjs`: artifacts in, six publishable folders out. It checks that versions match, sets the executable bit, and runs `version` on the host's binary. | `node scripts/npm-pack.mjs --dry-run`, and `node --test scripts/npm-pack.test.mjs` |
| 2.3 ✓ | CI job `pack-smoke` on the five runners: build, pack, install the tarballs into an empty project, run `npx fogline version` | Green on all five |
| 2.4 ✓ | `release.yml` (§8) with a dry-run mode | A dry run yields five binaries, six npm tarballs, and five archives with their checksums |
| 2.5 | **Name lock** (maintainer). Rename the repository if needed. `repository.url` is written once, in `npm/fogline/package.json`, and the pack script copies it into every package. | — |
| 2.6 | Bootstrap. The maintainer publishes 0.0.1 by hand from the tarballs of a dry run, then adds a trusted publisher to each of the six packages: this repository, the workflow `release.yml`, the environment `npm`. Tag `v0.0.2`; CI publishes with no token. The publisher setting expires unless a publish succeeds within two days. | `npm view fogline@0.0.2 dist.attestations` is present; the release smoke job is green on five runners |
| 2.7 | ADR-0004: distribution | Merged |

### M3 · Scan — 2.5 days

| # | Task | Check |
|---|---|---|
| 3.1 ✓ | Repository discovery, HEAD facts, exit codes 3 and 4 | Contract tests: not a repository, no commits, git missing, git too old (simulated by a script named `git`, so on Unix only) |
| 3.2 ✓ | Files: tree listing, line counts, language table, flags | A test-only line counter that reads the blobs agrees on every fixture file |
| 3.3 ✓ | History: metadata walk, K parallel workers, stream parsers including rename records, aggregation | Identical output for `--jobs 1`, `4` and `11` on the fixtures and on Django |
| 3.4 | Attribution: signature table, matcher, classification, per-signature totals; verify the pending signatures | Every signature has a fixture commit; v1 at `f857867` gives 17 of 51 |
| 3.5 | Carry history across renames to the current path (SHOULD); caveat registry | Rename case in fixture F3 |
| 3.6 | Bundle writer, schema export, TypeScript types, drift check, `scan --out`, `--json`, `--timings` | Two scans give identical bytes in every file but `run.json` |
| 3.7 | `cargo xtask bench scan` over `bench/corpus.toml` | Django scans in 5 s or less (target) |
| 3.8 | ADR-0005: scan semantics and bundle v0 | Merged |

### M4 · Renderer spike — 2 days, hard stop

| # | Task | Check |
|---|---|---|
| 4.1 | `cargo xtask synth`: seeded synthetic bundles at 5,000, 50,000 and 200,000 files | Same seed, same bytes |
| 4.2 | Harness: shared loading, scripted camera path, metric capture | Runs headed and headless |
| 4.3 | Candidates A, B and C behind the renderer interface | Each draws the 50,000-file bundle |
| 4.4 | Measure, decide, write ADR-0003 | Table, screenshots and decision recorded |

### M5 · Viewer and serving — 2 days

| # | Task | Check |
|---|---|---|
| 5.1 | Viewer v0 on the chosen renderer (§5.7) | Vitest and Playwright green |
| 5.2 | `open`: the server of §5.6, embedded assets, browser launch | Without the token → 404; wrong `Host` → 403 |
| 5.3 | `export` (SHOULD) | The folder, served by any static server, shows the same map |
| 5.4 | `cargo xtask bench e2e` | Django: process start to `ready` in 8 s or less (target); 15 s is the ceiling from the plan |

### M6 · Experiments and hardening — 1 day

| # | Task | Check |
|---|---|---|
| 6.1 | E1: gitoxide history backend (6 h) | Numbers recorded (§10) |
| 6.2 | E2: gitoxide blame against `git blame` (3 h) | Numbers recorded |
| 6.3 | E3: startup time of the binary against `node` | Numbers recorded |
| 6.4 | Error messages, Windows paths and encodings, README quickstart | CI runs the README's commands on a clean runner |

### M7 · Gate and release — 0.5 day

| # | Task | Check |
|---|---|---|
| 7.1 | Gate review (§10); ADR-0002 | Decision recorded |
| 7.2 | Phase report in `docs/reports/phase-1.md`: targets against measurements, deviations, what Phase 2 inherits | Merged |
| 7.3 | Tag `v0.1.0`; crates.io first publish (SHOULD) | Release smoke green; §13 complete |

---

## 7. Tests

| Layer | What it covers |
|---|---|
| Unit | Parsers for `ls-tree`, numstat (rename records included) and log records; trailer extraction; signature matching; the file classifier; rename resolution |
| Seeded, in place of property tests | The parser of the workers' output reads its real input damaged in 20,000 fixed ways and never crashes; cutting the work loses no commit and keeps the order for every count of commits and of runs up to 40; the history is the same for 1 to 1,000 jobs. A fixed sequence of numbers makes a failure repeat. This avoids the proptest dependency; it comes back if these stop finding bugs. |
| Contract (black box) | Runs the binary named by `FOGLINE_BIN` on each fixture and compares the bundle with a golden file. Also: exit codes, stdout and stderr discipline, determinism, and "the repository is byte-for-byte unchanged after a scan". |
| Differential | The parallel workers against one `git log --numstat` process; the line counts against a counter that reads the blobs; the line statistics against the definition in §5.2, computed from both versions of each changed file |
| Viewer unit (Vitest) | Bundle decoding, hierarchy building, color scales, formatters |
| Viewer end-to-end (Playwright, Chromium on Linux) | A fixture bundle served by the real binary: the ready event, cells drawn, tooltip, layer switch, caveats listed, no outside requests, one screenshot baseline |
| Launcher (`node:test`) | Platform table, missing package message, exit code and signal pass-through |
| Pack script (`node:test`) | The version source, the generated manifests, missing binaries, a wrong version or target in the host's binary, the refusal to write into a folder that has files, a checkout reached through a symlink, and the five targets repeated in the workflows and `deny.toml` |
| Pack smoke, release smoke | §8 |

Fixture repositories are built at test time by `fogline-testkit` with fixed dates, names and settings, so their SHAs are the same everywhere.

| Fixture | Contains |
|---|---|
| F1 basic | Six files in three directories, five commits, two authors, no agents |
| F2 agents | One commit per signature in §5.4; mixed-case keys; several trailers on one commit; a trailer inside a squash-style body; a message that mentions a tool without a trailer line (must not match); a `[bot]` author |
| F3 edges | Empty file, binary file, file with no final newline, CRLF file, non-ASCII path, path with spaces, deep nesting, symlink, submodule entry, deleted file, renamed file, renamed-and-edited file, lockfile, minified file, a file that is one very long line, vendored file, merge commit, empty commit |
| F4 empty | A repository with no commits |
| F5 shallow | A depth-1 clone of F1 |
| Generated | About fifty commits made by a fixed sequence of numbers: lines that repeat, edits at random places, renames, deletions, binary files, a merge every seventh step. Built inside the history tests. |
| Large file | One file of 3,000 lines, half of it rewritten: a change where git's default diff is longer than the minimal one, so a test notices if the minimal diff is ever dropped |

The differential checks also run on any repository, for the ones too large to build in a test: `FOGLINE_HISTORY_REPO=<path> cargo test --release -p fogline-core --test history -- --ignored --nocapture`.

Benchmark corpus, pinned by SHA in `bench/corpus.toml`. Counts measured today from commit messages with a subset of the seed rules, so read them as minimums:

| Repository | Role | Commits | With an agent signature |
|---|---|---|---|
| `codebaseqa` at `f857867` | Tiny, known answer | 51 | 17 (33%) |
| `colbymchenry/codegraph` | Agent-heavy | 1,853 | 1,423 (77%) |
| `Graphify-Labs/graphify` | Agent-heavy | 3,065 | 1,478 (48%) |
| `django/django` at `f24f38c6b2` | Large, none | 35,123 | 0 |

---

## 8. CI and release

**`ci.yml`**, on every pull request and on `main`:

| Job | Runs on | Does |
|---|---|---|
| `rust` | Linux, macOS, Windows | rustfmt, clippy with warnings as errors, cargo-nextest. The black-box contract tests run here: they are integration tests of the `fogline` crate. |
| `rust-old-git` | Linux, in an Ubuntu 20.04 container | The Rust tests again, with git 2.25.1 in place of a current git. 2.25 is the oldest git the scan supports (§5.3). The job stops before the tests if the git it installed is not 2.25. |
| `viewer` | Linux | Biome, type check, Vitest, build; Playwright from M5 |
| `launcher` | Linux on Node 18 and 26, Windows on Node 26 | The launcher's tests. Node 18 is the oldest version it supports. |
| `pack-smoke` | The five runners in §5.8 | Build the release binary, run the pack script's tests, then `scripts/pack-smoke.mjs`: pack, install the tarballs into an empty project with install scripts off, and compare `npx fogline version --json` with what the binary prints when run directly. The Linux rows also check that the binary is static. From M3, one fixture scan. |
| `schema-drift` | Linux | Regenerates the schema and the TypeScript types; fails on any diff |
| `deny` | Linux | cargo-deny: advisories, licences, and a ban on HTTP client crates in the engine |
| `workflows` | Linux | zizmor and actionlint on the workflows |

`rust-old-git` is there because some of what §5.3 describes depends on git's version, and every other job has a current git. Git 2.25 ignores `GIT_ATTR_SOURCE`, so the scan records the caveat `attributes_from_work_tree`. It also ignores `GIT_NO_LAZY_FETCH`, so the scan's own search for missing objects is the only thing that keeps a scan of a partial clone from downloading.

The job installs one exact build of git: the package `1:2.25.1-1ubuntu3`, which is the build Ubuntu 20.04 was released with. The newest build of that package, `1:2.25.1-1ubuntu3.14`, also calls itself 2.25.1, but a security update gave it `GIT_NO_LAZY_FETCH`. With that build a partial clone behaves as it does on a current git, and the job would pass without testing the download case. A version number does not say everything about a git that a distribution has patched. The released build still has the security bugs that the updates fixed, so the job checks the repository out before it installs git: the files arrive through GitHub's API, and the old git only ever reads the repositories that the tests build.

**`release.yml`** runs a real release on a `v*` tag. Anything else is a dry run, which stops after step 3 and keeps what it built as workflow artifacts: started by hand, or by a pull request that changes the workflow, the pack script or the launcher.

1. `check` — the package manifests are consistent, and on a release the tag is `v` plus the version in `Cargo.toml`. It fails before any build starts.
2. `build` ×5 — on the native runners, with `--locked` and no build cache.
3. `pack` — `scripts/npm-pack.mjs` and `npm pack` make six tarballs, sorted into `platform/` and `launcher/` so that no later step needs a package's name; the binaries also go into five archives with a `SHA256SUMS` file.
4. `attest` — a build-provenance attestation for each archive, so a download can be checked with `gh attestation verify`. npm adds its own provenance to each package in the next step.
5. `publish-npm` — publish the five platform packages, then the launcher, through npm trusted publishing. It asks the registry whether each version exists. One that exists is skipped only if the registry's checksum is this run's tarball, which is what a run started again after a failure finds. A different file under the same version stops the release, because carrying on would mix two builds.
6. `github-release` — a pre-release with the archives and checksums.
7. `smoke` ×5 — on clean runners: `npx --yes fogline@<version> version`, then a fixture scan from M3. It retries while the registry catches up, each time with an empty npm cache, so an install made too early is not reused.

The binary does not embed the viewer yet, so there is no viewer build here. Task 5.2 adds it as the first step.

If the smoke job fails after publishing, deprecate that version on npm and fix forward.

Supply-chain rules:

- No long-lived registry token exists anywhere. Publishing uses OIDC. After bootstrap, each package's settings require 2FA and disallow tokens.
- Only `attest` and `publish-npm` can get an identity token. Neither checks out the repository, builds anything or runs its scripts: they act on files that earlier jobs made. `publish-npm` runs in a GitHub environment named `npm`, which the trusted-publisher settings name too, so a required reviewer on that environment becomes an approval step for every release.
- Third-party actions are pinned to commit SHAs, and Dependabot updates them. Workflow permissions default to none, with per-job grants.
- Lockfiles are committed. CI installs with `--frozen-lockfile` and builds with `--locked`.
- pnpm's release-age policy stays on, so a package published hours ago is not installed. When it blocks a version, take the previous release; do not add an exclusion. Dependabot waits seven days for the same reason.

Facts this section relies on, checked today:

| Fact | Source |
|---|---|
| npm trusted publishing needs npm 11.5.1+, Node 22.14+, `id-token: write` and a GitHub-hosted runner. It is set up per package from the package's settings page, and a new setting must be used within two days. Provenance is automatic for public packages from public repositories. | [npm docs](https://docs.npmjs.com/trusted-publishers) |
| Settings created after 2026-09-03 allow staged publishing by default; direct `npm publish` is a separate permission. It is granted for Phase 1, and staged approval is reconsidered before 1.0. | [npm docs](https://docs.npmjs.com/trusted-publishers) |
| Runner labels `macos-latest` (Arm), `macos-15-intel`, `ubuntu-latest`, `ubuntu-24.04-arm`, `windows-latest` and `windows-11-arm` exist for public repositories | [GitHub docs](https://docs.github.com/en/actions/reference/runners/github-hosted-runners) |
| Biome's `@biomejs/biome` has a JavaScript `bin`, pinned `optionalDependencies` per platform, no install scripts, and platform packages with `os`, `cpu` and `libc` | [npm registry](https://www.npmjs.com/package/@biomejs/biome) |
| cargo-dist 0.34.0 is maintained. Its npm package "will fetch your prebuilt archives" after install, and it publishes with an `NPM_TOKEN` secret. | [dist book](https://axodotdev.github.io/cargo-dist/book/installers/npm.html) |
| A crates.io trusted publisher "can only be created after an initial manual publishing of a crate" | [RFC 3691](https://rust-lang.github.io/rfcs/3691-trusted-publishing-cratesio.html) |
| Current action majors: checkout 7, setup-node 7, upload-artifact 7, download-artifact 8, attest 4, pnpm/action-setup 6. `attest-build-provenance` 4 is now a wrapper around `actions/attest`, and its README sends new workflows there. | GitHub releases |
| An install with only the launcher and one platform tarball succeeds while the other four platform packages do not exist on the registry: npm treats an optional dependency it cannot find as skipped. | `node scripts/pack-smoke.mjs target/release/fogline`, run on 2026-11-15 with npm 11.20.0 |
| In a clone made with `git clone --filter=blob:none --no-checkout`, `GIT_NO_LAZY_FETCH=1 git ls-tree -r -l HEAD` downloads every missing file with Ubuntu 20.04's git package at `1:2.25.1-1ubuntu3`. At `1:2.25.1-1ubuntu3.14` it downloads nothing and warns "lazy fetching disabled". `git rev-list --objects --missing=print --no-walk HEAD` downloads with neither. | Run on 2026-11-19 in the `ubuntu:20.04` image |
| With the scan's search for missing objects taken out, `fogline scan` on such a clone of fixture F1 exits with 0 on the first of those builds, having downloaded all six files, and fails without downloading on the second. With the search in place, the tests pass on both. | The job's steps, run on 2026-11-19 in the `ubuntu:20.04` image with each build |
| Upstream git reads `GIT_NO_LAZY_FETCH` from 2.45.0 on, and security releases of older series read it too: `promisor-remote.c` names the variable at the v2.39.4, v2.43.4 and v2.44.1 tags, and not at v2.25.1, v2.39.3 or v2.44.0. A version number alone therefore does not say whether a git honours it. | git's source |

---

## 9. Renderer spike

**Question.** Which renderer can draw a labeled territory map of 50,000 files at 60 frames per second, recolor it while a slider is dragged, and leave room for Phase 4's edges and overlays?

**Candidates.**

- **A** — MapLibre GL JS 6 for geometry and labels, with deck.gl 9.4 layered in through `@deck.gl/maplibre` for anything that changes every frame. Layout coordinates are converted to longitude and latitude with the inverse Web Mercator formula, so MapLibre draws them undistorted. deck.gl's guide lists MapLibre 6 as supported; the interleaved mode needs WebGL2.
- **B** — deck.gl 9.4 alone with an orthographic view: a polygon layer for cells, the text layer with its collision filter for labels.
- **C** — a small custom WebGL2 renderer with instanced rectangles and Canvas 2D labels. This is the control: it shows what the libraries buy.

**Data.** The three synthetic bundles and the Django bundle.

**Environment.** Chrome stable at 1600 × 1000 CSS pixels, device pixel ratio 2, on the M3 Pro. Repeated with 4× CPU throttling as a stand-in for slower machines. One manual pass in Safari and in Firefox.

**Script, identical for every candidate.** Fit; zoom into the densest district until file labels show (3 s); pan diagonally (2 s); zoom out (3 s); three rounds. Then 200 hover probes along a line. Then 20 layer switches. Then a 5 s scrub that recolors every cell on every frame from 24 precomputed snapshots.

**Hard checks on the 50,000-file bundle.** A candidate that fails one is out.

| Check | Limit |
|---|---|
| 95th-percentile frame time, unthrottled | 16.7 ms |
| 95th-percentile frame time, throttled 4× | 33 ms |
| Recolor every cell on a layer switch | 50 ms |
| Scrub | 30 frames per second or better |
| First frame after the data arrives | 1 s |
| Chrome, Safari and Firefox | All render correctly |

**Scored, 1 to 5.**

| Criterion | Weight |
|---|---|
| Label quality: decluttering, crisp text at fractional zoom, long and non-Latin names | 30% |
| Fit for Phase 4: bundled edges, change overlay, zoom-banded detail | 25% |
| Interaction feel and pick latency | 20% |
| Adapter size and number of workarounds | 15% |
| Download size | 10% |

**Decision rule.** The highest weighted score among the candidates that pass wins. If two are within 0.3, the one with less custom code wins. If none passes at 50,000 files, B is adopted provisionally and ADR-0003 records the gap for Phase 4.

**Timebox.** Two days in total. If candidate A's coordinate mapping does not render correctly within three hours, record that and drop it.

**Output.** `packages/viewer/spike/` (kept, excluded from the production build), raw metrics as JSON, screenshots, ADR-0003.

---

## 10. Experiments and the engine-language gate

**What Phase 1 can and cannot show.** In Phase 1 git does the heavy work, so Rust will not look faster than a Node engine here. Rust's case rests on three things this phase has to test, and one that only the maintainer can judge:

1. one self-contained binary on every platform;
2. startup fast enough for editor hooks, which Phase 6 budgets at 50 ms;
3. headroom for the Phase 2 line tracker;
4. whether the maintainer can own the code.

| Experiment | Method | Baseline to beat | Timebox |
|---|---|---|---|
| E1 gitoxide history | Implement the same statistics with `gix` 0.90 (`blob-diff` feature, `line_counts`). Output must equal the reference on the fixtures with renames off. Time it on Django. | 2.1 s (11 git processes, the pinned command) | 6 h |
| E2 blame | `gix-blame` 0.20 against `git blame --porcelain` on the same 200 Django files: per-line agreement and wall time | 33.4 s sequential; 5.2 s with 10 processes | 3 h |
| E3 startup | `fogline version` against `node -e 0`, 50 runs each, median and 95th percentile | — | 1 h |

Gate, decided in task 7.1 and written up as ADR-0002:

| | Criterion | If not met |
|---|---|---|
| G1 | `npx fogline` works on all five targets from an automated, token-free release | Switch |
| G2 | The engine tasks in M3 took at most 1.5× their estimates, and the Rust CI jobs finish within 10 minutes with a warm cache | Switch |
| G3 | The maintainer can explain, without notes, how the scan moves data between processes and threads, how errors travel, and why the bundle is deterministic, and has reviewed every Rust change | Switch |
| G4 | The E1–E3 numbers are recorded | Informs Phase 2; no veto |

G3 is the project's own thesis applied to its author.

If the answer is "switch", the engine is rewritten in TypeScript on Node (estimate: 3 days at Phase 1 scope). The bundle schema, the viewer, the contract tests, the golden files and CI all stay, and distribution collapses to a single npm package.

---

## 11. Measurements and budgets

Measured today on Django at `f24f38c6b2`: 7,102 files, 1,122,175 text lines, 1,379 binary files, 35,123 commits of which 591 are merges.

| Operation | Time |
|---|---|
| List the tree with sizes | 0.06 s |
| Line counts for every file | 0.21 s |
| Commit metadata, no diffs | 0.23 s |
| Line statistics with the pinned command (§5.3), one process | 12.6 s |
| Line statistics with the pinned command, 11 processes | 2.1 s |
| For comparison, git's defaults without rename detection: 1, 5 and 11 processes | 11.1 s, 3.9 s, 2.2 s |
| `git blame` on 200 Python files, one at a time | 33.4 s (167 ms per file) |
| `git blame` on the same files, 10 at a time | 5.2 s |

Measured again on 2026-11-19 with `fogline scan` itself: release build, git 2.52, 11 cores, median of five runs (three for `--jobs 4`, one for `--jobs 1`).

| Command | Time |
|---|---|
| `fogline scan`, which here means 11 jobs | 2.7 s |
| `fogline scan --jobs 4` | 4.4 s |
| `fogline scan --jobs 1` | 14.3 s |
| `fogline scan --history none` | 0.30 s |

It reports 35,123 commits, 591 merges, 3,378,909 lines added and 2,253,367 deleted, in 154,174 changes to 11,797 paths, 2,398 of them renames. The history is the same for 1, 4 and 11 jobs, and it is what one `git log --numstat` process reports, commit for commit. The command at the end of §7 repeats that check and prints these counts.

What the numbers say:

- The whole Phase 1 scan is under 3 s of git time on a 35,000-commit repository. The plan's 15 s target is a ceiling, not a stretch.
- Parallel plumbing is exact: the workers' output equals the single process byte for byte.
- The diff algorithm matters. With rename detection off, the minimal diff reports 47,941 fewer added lines than git's default (3,643,292 against 3,691,233) and changes the per-file sum for 862 of 9,607 paths, for 3–5% more time.
- Rename detection matters. With git's default diff, leaving it off inflates added lines by 7.7% across Django's 2,398 renames.
- Together: the pinned command reports 3,378,909 added lines over Django's history. Git's defaults without rename detection report 3,691,233, which is 9% more.
- Blaming every text file of Django would take roughly two and a half minutes with 10 processes, if the sample is representative. That is fine as a one-time oracle in Phase 2 and too slow as the everyday path, which is why Phase 2 builds an incremental tracker.

Budgets for this phase (all targets):

| Budget | Target |
|---|---|
| Scan, Django, warm disk cache | 5 s |
| Process start to map ready, Django | 8 s; ceiling 15 s |
| Viewer first frame after the bundle arrives, 7,000 files | 500 ms |
| Pan and zoom at 7,000 files | 60 frames per second |
| Binary size per target | 15 MB |
| Rust CI jobs, warm cache | 10 minutes |

---

## 12. Risks

| Risk | Response |
|---|---|
| The npm scope or name is unavailable at name lock | Unscoped platform package names; Kenmap as the fallback name |
| The first publish and trusted-publisher setup take longer than planned | It is a one-time manual runbook (task 2.6). A short-lived token used once and deleted is the fallback. |
| Windows surprises: paths, encodings, process spawning | Contract tests run on Windows from M1; M6 holds a half-day for it |
| The spike is inconclusive | The decision rule in §9 always yields an answer; B is the default |
| MapLibre's coordinate mapping fights back | Three-hour cutoff; A is dropped, B or C carries on |
| Rename following has corner cases across merges | It is a SHOULD with a caveat defined; first match wins, deterministically |
| Antivirus or Gatekeeper flags an unsigned binary | npm installs are not quarantined on macOS. Direct downloads are a LATER concern; signing costs money and is out of scope. |
| Scope creep into Phase 2 | §1's right-hand column is the list of things to refuse |
| iCloud damages a checkout again | D2: no checkout lives under Desktop or Documents |

---

## 13. Definition of done

- [ ] `npx --yes fogline@0.1.0` runs on all five targets; the release smoke job is green.
- [ ] The release was published by CI with no stored token, and the packages carry provenance.
- [ ] Django at `f24f38c6b2`: scan and end-to-end times are measured and written into the phase report next to their targets.
- [ ] `codebaseqa` at `f857867` reports 17 agent-attributed commits out of 51.
- [ ] Determinism and repository-unchanged tests are green on Linux, macOS and Windows.
- [ ] The viewer draws the map with both layers, tooltip, side panel and the caveat list; Playwright is green.
- [ ] ADR-0001 to ADR-0005 are merged: repository and conventions, engine language, renderer, distribution, scan semantics and bundle.
- [ ] The phase report is written. The plan is updated wherever reality diverged.
- [ ] v1 is tagged and its demo still works.

---

## Appendix A — working agreements

- Short-lived branches, pull requests with required CI even when working alone, squash merges, conventional-commit subjects.
- Every pull request says how it was verified.
- A decision that would be expensive to reverse gets an ADR of a page or less: context, options, decision, evidence, consequences.
- When behavior diverges from this spec, the spec changes in the same pull request.

`CLAUDE.md` in the new repository carries the rules an AI pair must follow there:

1. Never write into a scanned repository. Tests enforce it.
2. Facts come from git or from parsers. Anything estimated is labeled as an estimate.
3. A number in the docs or the UI must be reproducible by a test or by a command under `bench/`.
4. Run `cargo xtask ci` before pushing.
5. No new dependency without a sentence in the pull request saying why.

## Appendix B — other sources

- Formats this phase will meet later: the [git-ai standard](https://github.com/git-ai-project/git-ai/blob/main/specs/git_ai_standard_v3.0.0.md) stores authorship logs as git notes under `refs/notes/ai`; [Agent Trace](https://agent-trace.dev/) 0.1.0 defines a JSON record and leaves storage open.
- Renderer facts: [deck.gl with MapLibre](https://deck.gl/docs/developer-guide/base-maps/using-with-maplibre).
- gitoxide facts: [`line_counts` in gix 0.90](https://docs.rs/gix/latest/gix/object/blob/diff/struct.Platform.html).
- Name check: [Fogline Group](https://foglinegroup-com.pages.dev/about.html).
- Versions: [Node release index](https://nodejs.org/dist/index.json), [Rust stable channel](https://static.rust-lang.org/dist/channel-rust-stable.toml), the crates.io and npm registries.
