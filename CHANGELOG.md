# Changelog

Notable changes, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/), and versions follow [Semantic Versioning](https://semver.org/). Until 1.0.0 any release may change behaviour.

## Unreleased

### Added

- Repository skeleton: a Rust workspace (`fogline`, `fogline-core`, `fogline-testkit`, `xtask`) and the viewer package.
- `fogline version`, with `--json` for the build target, the build commit and the git found on the machine.
- A test kit that builds fixture git repositories with the same commit ids on every machine, and black-box contract tests that run the binary.
- A viewer shell that looks for a bundle and reports what it found.
- CI for Linux, macOS and Windows, a dependency policy, and workflow linting.
- An npm launcher that finds the binary for the machine, runs it, and passes exit codes and signals through. Nothing is published to npm yet.
- A packaging script that turns built binaries into the launcher package and five platform packages, and a smoke test, run on all five platforms, that installs them into an empty project.
- A release workflow with a dry-run mode. A dry run builds five binaries and packs them into six npm tarballs and five archives, and publishes nothing.
