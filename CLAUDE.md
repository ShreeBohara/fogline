# Working in this repository

Fogline is pre-alpha. Before proposing or changing anything, read `docs/plan.md` (why, and what the product is) and the current spec in `docs/specs/` (what is being built now, task by task).

## Commands

- `cargo xtask ci` runs everything CI runs. Run it before pushing. `--skip-viewer` leaves out the Node checks.
- `cargo test -p fogline --test contract` runs the black-box contract tests. Set `FOGLINE_BIN` to point them at another binary.
- `FOGLINE_HISTORY_REPO=<path> cargo test --release -p fogline-core --test history -- --ignored --nocapture` checks the history of any repository against one `git log` process and against other numbers of workers, and each file's history across renames against a second implementation. It prints the totals, the agent-attributed commits among them. Django takes about half a minute.
- `cargo test -p fogline-core --test signatures -- --ignored --nocapture` reads from GitHub the public commit behind every mark in the table of agent signatures and checks that the mark is on it. It needs the network and `gh`, signed in.
- `pnpm --filter @fogline/viewer dev` starts the viewer's dev server.
- `node scripts/npm-pack.mjs --dry-run` lists the npm packages a release would publish. `node scripts/pack-smoke.mjs target/release/fogline` installs them into an empty project and runs the result.
- Rust comes from `rust-toolchain.toml` through rustup. Node comes from `.nvmrc` (`nvm use`); pnpm's version comes from `packageManager` in `package.json`.

## Rules

1. Never write into a repository the tool scans. Tests enforce it.
2. Facts come from git or from parsers. Anything estimated is labelled as an estimate.
3. A number in the docs or the UI must be reproducible by a test or by a command under `bench/`.
4. Say "agent-attributed", never "AI-written" (spec section 5.2 explains why).
5. No new dependency without a sentence in the pull request saying why it is needed.
6. When behaviour diverges from the spec, change the spec in the same pull request.
7. A decision that would be expensive to reverse gets a record in `docs/adr/`.

## Constraints from the maintainer

- Recurring cost stays near zero: no always-on paid backend, and LLM features use the user's own key.
- The maintainer must be able to explain every module (gate G3 in the Phase 1 spec). Prefer plain code, explain non-obvious choices in comments, and keep each change small enough to review.
- When there is a choice to make, recommend one option and give the reasons. Say plainly what should be cut.

## Git

- Work on short-lived branches and open pull requests; CI is required on `main`.
- Commit and push only when asked.
- Keep checkouts out of folders that iCloud syncs (Desktop, Documents). iCloud damaged the previous project's checkout that way.
