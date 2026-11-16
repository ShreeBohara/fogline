# Fogline

Fogline maps which parts of a codebase have evidence that a person understands them, and which do not.

Coding agents now write code faster than people can read it. Fogline is meant to make that gap visible, then help close it with tours and exercises whose answers come from the repository's own tests and history, never from a language model. The reasoning and the research behind it are in the [plan](docs/plan.md).

> **Status: pre-alpha.** Nothing here is usable yet. Phase 1, a first map of any git repository, is being built; its scope is in the [Phase 1 spec](docs/specs/phase-1.md). "Fogline" is a working name.

## What exists today

| Piece | State |
|---|---|
| `fogline version` | Works. Prints the version, or build and environment details with `--json`. |
| `fogline scan` | Recognised but not implemented. It exits with an error that says so. |
| Viewer | A shell that looks for a bundle and reports what it found. There is no map yet. |
| npm launcher | Finds the binary for the machine and runs it. It is tested, and nothing is published to npm yet. |
| Test kit | Builds small git repositories that have the same commit ids on every machine. |

## Build and test

You need git, [rustup](https://rustup.rs) (it installs the Rust version pinned in `rust-toolchain.toml`), and, for the viewer and the npm packaging, Node 26 with pnpm.

Run everything CI runs:

```bash
cargo xtask ci
```

Leave out everything that needs Node:

```bash
cargo xtask ci --skip-viewer
```

Try the binary:

```bash
cargo run -p fogline -- version --json
```

## Layout

| Path | What it is |
|---|---|
| `crates/fogline-core` | The engine. It turns a git repository into facts. It never prints and never exits. |
| `crates/fogline` | The command line: arguments, output, exit codes. Its `tests/contract.rs` runs the binary as a black box. |
| `crates/fogline-testkit` | Fixture repositories for tests. |
| `xtask` | Developer commands, written in Rust so they behave the same on every OS. |
| `packages/viewer` | The viewer: React and TypeScript, built with Vite. |
| `npm/fogline` | The npm launcher: a script with no dependencies that finds the binary for the machine and runs it. |
| `scripts` | `npm-pack.mjs` turns built binaries into the npm packages. |
| `docs` | The plan, the phase specs and the decision records. |

## Ground rules

1. Facts come from git, parsers and test runs. A model may narrate them, and narration is labelled as such.
2. The tool never writes into a repository it scans.
3. Every number in the docs or the UI can be reproduced with a command.
4. Every dependency has to earn its place.

The full list is in section 4.7 of the [plan](docs/plan.md).

## History

Fogline is the successor to [CodebaseQA](https://github.com/ShreeBohara/codebaseqa), a self-hosted tool for asking questions about a repository. That project stays available. The [plan](docs/plan.md) explains why this one starts over.

## Licence

[MIT](LICENSE)
