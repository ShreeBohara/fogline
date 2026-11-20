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
- `fogline scan` finds the repository from any folder inside it, bare and shallow repositories included, and exits with 3 outside a repository or in one with no commits, and with 4 when git is missing or older than 2.25. It ignores `GIT_DIR` and similar variables, so it reads the folder it was given even when started from a git hook. Reading history comes next.
- `fogline scan` lists the files at HEAD and prints how many there are, how many lines they hold and how many are binary. Each file gets a language guessed from its name and labels for binary, generated, vendored and symbolic link. `.gitattributes` is read from the commit, so an uncommitted edit or a bare repository does not change the numbers (git 2.41 or newer). A scan writes nothing into the repository, and a test now proves it byte for byte. In a partial clone it never downloads: when files of the commit are not in the clone, it stops with exit code 3.
- `fogline scan` reads history: every commit HEAD can reach, and the lines each one added to and deleted from each file, with renames followed at 50% similarity and merges counted without line statistics. The summary gains a history line. Up to `--jobs` git processes read at once (1 to 64, by default one for each core and at most 12), and the result is the same for any number. `--history none` scans the files alone. Line counts are those of the smallest possible diff, which git 2.50 and newer compute exactly; with an older git the summary says that it may count a few lines more. A partial clone that lacks earlier versions of its files stops with exit code 3 and names `--history none`.

### Fixed

- A user's or a machine's git attributes file no longer changes which files a scan counts as binary.
