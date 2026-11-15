# Contributing

Fogline is pre-alpha and is being built by one person, phase by phase. The [plan](docs/plan.md) says where it is going, and the current [phase spec](docs/specs/phase-1.md) says what is being built now.

At this stage the most useful contributions are issues: a bug, a repository the tool gets wrong, or a hole in the plan. Please open an issue before starting on a pull request, so the work lands where the current phase can use it.

## Working on the code

The [README](README.md#build-and-test) lists what to install and how to run the checks. A pull request should:

- pass `cargo xtask ci`;
- say how the change was verified;
- update the spec in the same pull request when it changes behaviour the spec describes;
- explain any new dependency in a sentence.

Commit subjects follow [Conventional Commits](https://www.conventionalcommits.org/), for example `feat(scan): count lines per file`.

## Ground rules for the product

These are not negotiable in a pull request, because the tool's value depends on them:

1. It never writes into a repository it scans.
2. Its facts come from git, parsers and test runs, not from a language model.
3. Its numbers can be reproduced with a command.
