# ADR-0001: Repository, name, versions and conventions

Status: accepted · 2026-11-15

## Context

Fogline replaces an earlier project, CodebaseQA. The plan for its second version changed the product, from asking questions about a repository to mapping and verifying human understanding of it ([plan](../plan.md)). The rewrite shares no code with v1. Three things had to be settled before the first line was written: where the code lives, what it is called for now, and how releases are numbered.

Two facts shaped the answer:

- v1 has a working public demo and a coherent repository of its own.
- The v1 checkout sat in a folder synced by iCloud, and iCloud removed parts of its `.git` directory. On 2026-11-15 the index file disappeared, which made every tracked file look staged for deletion.

## Decision

1. **A new public repository.** v1 stays in its own repository, untouched apart from tags. Fogline starts with a clean history.
2. **Public from the first push.** GitHub's hosted runners for all five release targets are free without limits only for public repositories, and npm generates provenance only for public source.
3. **Working name: Fogline.** The name becomes final before the first publish to npm or crates.io, where names are effectively permanent (spec task 2.5).
4. **Versions start at 0.x, and the launch release is 1.0.0.** "2.0" is the plan's name for the redesign, not a package version. Every package in this repository shares one version.
5. **Checkouts live outside folders that iCloud syncs.**
6. **Conventions.**
   - Rust is pinned by `rust-toolchain.toml`, Node by `.nvmrc`, pnpm by `packageManager`.
   - Work happens on short-lived branches and lands through pull requests with required CI.
   - Commit subjects follow Conventional Commits.
   - `cargo xtask ci` is the one command that reproduces CI.
   - A decision that would be expensive to reverse gets a record in this folder.

## Options considered

| Option | Why not |
|---|---|
| Replace `main` in the v1 repository (the plan's first draft) | It removes a working product from the front page of its own repository for months and joins two unrelated histories. |
| An orphan `main` in the v1 repository, with v1 on a branch | One repository and a clean history, but its name, tags, releases and deployed site would describe two different products. |
| Private until the first alpha | It gives up free runners for macOS, Windows and Arm, and npm provenance. |

## Consequences

- Nothing in v1 has to be deleted, and its demo keeps running.
- A rename before the first publish is a search-and-replace over the places the name appears, plus a repository rename. Those places are: the repository; the crates `fogline`, `fogline-core` and `fogline-testkit`; the binary; the npm package `@fogline/viewer`; the bundle format id `fogline.bundle/0`; the `FOGLINE_*` environment variables; and the `.fogline` CSS scope.
- Building in public from the first commit means the unfinished state is visible. The README says plainly what works and what does not.
