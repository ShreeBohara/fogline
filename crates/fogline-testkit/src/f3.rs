//! F3 "edges": the awkward cases, in one small repository.
//!
//! At HEAD: an empty file, a binary file, a file with no final newline, a
//! file with Windows line endings, a path that is not ASCII, a path with a
//! space, a deeply nested file, a symbolic link, a submodule entry, a
//! lockfile, a minified file, a file that is one very long line, and a
//! vendored file.
//!
//! In its history: a branch merged with a merge commit, a renamed file, a
//! file renamed and edited in the same commit, a deleted file, and a commit
//! that changes nothing.

use std::io;

use crate::{Fixture, Identity, RepoBuilder};

/// The commit id F3 must have on every machine. CI builds the fixture on
/// Linux, macOS and Windows and compares each result with this.
pub const HEAD: &str = "a263f2cf44a0bc9b83b7c05f36125deb0f017b8e";

/// The files at HEAD, sorted the way git sorts them: byte by byte. The
/// submodule entry is not a file and is not listed.
pub const FILES: [&str; 18] = [
    "LICENSE",
    "README.md",
    "a/b/c/d/e/f/deep.txt",
    "assets/logo.bin",
    "docs/caf\u{e9}.md",
    "docs/release notes.md",
    "latest",
    "package-lock.json",
    "scripts/run.bat",
    "src/app.py",
    "src/empty.py",
    "src/feature.py",
    "src/new_name.py",
    "src/no_newline.txt",
    "src/summary.py",
    "vendor/lib/util.js",
    "web/app.min.js",
    "web/bundle.js",
];

/// The symbolic link, and the path it points to.
pub const LINK: &str = "latest";
pub const LINK_TARGET: &str = "docs/release notes.md";

/// The submodule entry, and the commit it is pinned to. No such commit
/// exists: nothing ever follows the pointer.
pub const SUBMODULE: &str = "modules/engine";
pub const SUBMODULE_COMMIT: &str = "1234567890123456789012345678901234567890";

/// The number of commits reachable from HEAD, and how many of them are
/// merge commits.
pub const COMMITS: usize = 9;
pub const MERGES: usize = 1;

/// The lines of the seventeen files that have lines, and the one file that
/// has none because it is binary. The symbolic link counts as zero lines.
pub const LINES: u64 = 40;
pub const BINARY_FILES: usize = 1;

const ADA: Identity = Identity {
    name: "Ada Example",
    email: "ada@example.com",
};

const GRACE: Identity = Identity {
    name: "Grace Example",
    email: "grace@example.com",
};

/// 2026-02-02 09:00:00 UTC. Each commit is one day after the one before.
const START: i64 = 1_770_022_800;
const DAY: i64 = 86_400;

/// The first bytes of a PNG file. The zero bytes are what makes git, and
/// fogline, treat a file as binary.
const BINARY: [u8; 16] = [
    0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, b'I', b'H', b'D', b'R',
];

/// Builds the fixture in a fresh temporary directory.
pub fn build() -> io::Result<Fixture> {
    let mut repo = RepoBuilder::new()?;

    repo.write("README.md", "# Edges\n\nA repository of awkward cases.\n")?;
    repo.write("src/app.py", "def main():\n    print(\"hello\")\n")?;
    repo.write(
        "src/old_name.py",
        "def area(width, height):\n    return width * height\n\n\n\
         def perimeter(width, height):\n    return 2 * (width + height)\n",
    )?;
    repo.write("src/report.py", &report("Report"))?;
    repo.write("src/doomed.py", "def unused():\n    return None\n")?;
    repo.commit(ADA, START, "Start the project\n")?;

    // A branch and a merge commit. Both sides change different files, so
    // the merge has nothing to resolve.
    repo.start_branch("feature")?;
    repo.write("src/feature.py", "def feature():\n    return \"new\"\n")?;
    repo.commit(GRACE, START + DAY, "Add a feature on its own branch\n")?;
    repo.switch("main")?;
    repo.write(
        "src/app.py",
        "def main(name=\"world\"):\n    print(\"hello \" + name)\n",
    )?;
    repo.commit(ADA, START + 2 * DAY, "Greet by name\n")?;
    repo.merge(ADA, START + 3 * DAY, "feature", "Merge the feature branch")?;

    // A rename with no other change.
    repo.rename("src/old_name.py", "src/new_name.py")?;
    repo.commit(GRACE, START + 4 * DAY, "Rename a module\n")?;

    // A rename and an edit in the same commit: one line of eight changes,
    // which leaves the file similar enough for git to see the rename.
    repo.remove("src/report.py")?;
    repo.write("src/summary.py", &report("Summary"))?;
    repo.commit(
        ADA,
        START + 5 * DAY,
        "Rename the report and change its title\n",
    )?;

    repo.remove("src/doomed.py")?;
    repo.commit(GRACE, START + 6 * DAY, "Remove a module nobody uses\n")?;

    // A commit that changes no file.
    repo.commit(ADA, START + 7 * DAY, "Mark the release\n")?;

    repo.write("LICENSE", "Example licence text.\n")?;
    repo.write("src/empty.py", "")?;
    repo.write_bytes("assets/logo.bin", &BINARY)?;
    repo.write("src/no_newline.txt", "first line\nlast line has no newline")?;
    repo.write("scripts/run.bat", "@echo off\r\necho hello\r\n")?;
    repo.write(
        "docs/caf\u{e9}.md",
        "# Caf\u{e9}\n\nA path and a text that are not ASCII.\n",
    )?;
    repo.write(
        "docs/release notes.md",
        "# Release notes\n\n- Nothing yet.\n",
    )?;
    repo.write("a/b/c/d/e/f/deep.txt", "deep\n")?;
    repo.write(
        "package-lock.json",
        "{\n  \"name\": \"edges\",\n  \"lockfileVersion\": 3\n}\n",
    )?;
    repo.write(
        "web/app.min.js",
        "function a(){return 1}function b(){return 2}\n",
    )?;
    // One line of 811 bytes: long enough to count as generated.
    repo.write(
        "web/bundle.js",
        &format!("var x=[{}0];\n", "1,".repeat(400)),
    )?;
    repo.write("vendor/lib/util.js", "export const id = (x) => x;\n")?;
    repo.symlink(LINK, LINK_TARGET)?;
    repo.submodule(SUBMODULE, SUBMODULE_COMMIT);
    repo.commit(GRACE, START + 8 * DAY, "Add the awkward files\n")?;

    repo.finish()
}

/// The report module, with the given title. It is written twice, under two
/// names and with two titles.
fn report(title: &str) -> String {
    format!(
        "TITLE = \"{title}\"\n\n\n\
         def render(rows):\n    \
         lines = [TITLE, \"=\" * len(TITLE)]\n    \
         for name, value in rows:\n        \
         lines.append(f\"{{name}}: {{value}}\")\n    \
         return \"\\n\".join(lines)\n"
    )
}
