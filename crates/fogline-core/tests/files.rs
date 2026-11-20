//! The file listing against real repositories.
//!
//! The engine never reads a file: it takes sizes and line counts from two
//! git commands. These tests check it by a second route. They read every
//! blob and count its line endings, and the two answers have to agree.

use std::fs;

use fogline_core::classify::Flags;
use fogline_core::files::{self, Files};
use fogline_core::repo;
use fogline_testkit::{Fixture, Identity, RepoBuilder, f1, f3, f5};

fn list(fixture: &Fixture) -> Files {
    let found = repo::discover(&fixture.path()).expect("the fixture is a repository");
    files::list(&found).expect("its files can be listed")
}

fn paths(files: &Files) -> Vec<&str> {
    files
        .entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect()
}

/// The line count as the spec defines it, computed the slow way: one per
/// line ending, plus one for a last line that has none, and −1 when one of
/// the first 8,000 bytes is zero, which is how git spots a binary file.
fn count_lines(content: &[u8]) -> i64 {
    if content.iter().take(8000).any(|&byte| byte == 0) {
        return -1;
    }
    let endings = content.iter().filter(|&&byte| byte == b'\n').count();
    let unfinished = usize::from(content.last().is_some_and(|&last| last != b'\n'));
    i64::try_from(endings + unfinished).expect("a fixture file is small")
}

/// Checks every entry against the blob it describes. `links` are the
/// symbolic links, whose content is a path and counts as no lines.
fn assert_agrees_with_the_blobs(fixture: &Fixture, files: &Files, links: &[&str]) {
    for entry in &files.entries {
        let blob = fixture
            .git_bytes(["cat-file", "blob", &format!("HEAD:{}", entry.path)])
            .expect("every listed path names a blob");
        let size = u64::try_from(blob.len()).expect("a fixture file is small");
        assert_eq!(entry.bytes, size, "size of {}", entry.path);

        let expected = if links.contains(&entry.path.as_str()) {
            0
        } else {
            count_lines(&blob)
        };
        assert_eq!(entry.lines, expected, "lines of {}", entry.path);
    }
}

#[test]
fn lists_the_files_of_a_plain_repository() {
    let fixture = f1::build().unwrap();
    let files = list(&fixture);

    assert_eq!(paths(&files), f1::FILES);
    assert_agrees_with_the_blobs(&fixture, &files, &[]);
    assert_eq!(files.lines(), f1::LINES);
    assert_eq!(files.binary(), 0);
    assert_eq!(files.submodules_skipped, 0);
    assert!(!files.lossy_paths);

    for entry in &files.entries {
        assert_eq!(entry.flags, Flags::default(), "{}", entry.path);
        let expected = if entry.path.ends_with(".py") {
            "Python"
        } else {
            "Markdown"
        };
        assert_eq!(entry.language, Some(expected), "{}", entry.path);
    }
}

#[test]
fn handles_the_awkward_files() {
    let fixture = f3::build().unwrap();
    let files = list(&fixture);

    assert_eq!(paths(&files), f3::FILES);
    assert_agrees_with_the_blobs(&fixture, &files, &[f3::LINK]);
    assert_eq!(files.lines(), f3::LINES);
    assert_eq!(files.binary(), f3::BINARY_FILES);
    assert_eq!(files.submodules_skipped, 1);
    assert!(!files.lossy_paths);

    // Every file, spelled out: its lines, its language and its labels.
    let none = Flags::default();
    let expected = [
        ("LICENSE", 1, None, none),
        ("README.md", 3, Some("Markdown"), none),
        ("a/b/c/d/e/f/deep.txt", 1, None, none),
        ("assets/logo.bin", -1, None, Flags::BINARY),
        ("docs/caf\u{e9}.md", 3, Some("Markdown"), none),
        ("docs/release notes.md", 3, Some("Markdown"), none),
        ("latest", 0, None, Flags::SYMLINK),
        ("package-lock.json", 4, Some("JSON"), Flags::GENERATED),
        ("scripts/run.bat", 2, Some("Batch"), none),
        ("src/app.py", 2, Some("Python"), none),
        ("src/empty.py", 0, Some("Python"), none),
        ("src/feature.py", 2, Some("Python"), none),
        ("src/new_name.py", 6, Some("Python"), none),
        ("src/no_newline.txt", 2, None, none),
        ("src/summary.py", 8, Some("Python"), none),
        ("vendor/lib/util.js", 1, Some("JavaScript"), Flags::VENDORED),
        ("web/app.min.js", 1, Some("JavaScript"), Flags::GENERATED),
        ("web/bundle.js", 1, Some("JavaScript"), Flags::GENERATED),
    ];
    let actual: Vec<_> = files
        .entries
        .iter()
        .map(|entry| {
            (
                entry.path.as_str(),
                entry.lines,
                entry.language,
                entry.flags,
            )
        })
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn a_bare_clone_lists_the_same_files() {
    let fixture = f3::build().unwrap();
    let bare = fixture.git_clone(&["--bare"]).unwrap();
    assert_eq!(list(&bare), list(&fixture));
}

#[test]
fn a_shallow_clone_lists_the_same_files() {
    let shallow = f5::build().unwrap();
    let files = list(&shallow);
    assert_eq!(paths(&files), f1::FILES);
    assert_agrees_with_the_blobs(&shallow, &files, &[]);
    assert_eq!(files, list(&f1::build().unwrap()));
}

#[test]
fn listing_from_a_subfolder_gives_paths_from_the_top() {
    let fixture = f1::build().unwrap();
    let found = repo::discover(&fixture.path().join("src")).unwrap();
    let files = files::list(&found).unwrap();
    assert_eq!(paths(&files), f1::FILES);
}

/// `.gitattributes` can declare a text file binary. The answer has to come
/// from the commit: it must not change with an edit that was never
/// committed, and a bare repository must not lose it.
#[test]
fn attributes_come_from_the_commit_not_the_work_tree() {
    let who = Identity {
        name: "Ada Example",
        email: "ada@example.com",
    };
    let repo = RepoBuilder::new().unwrap();
    repo.write(".gitattributes", "*.dat binary\n").unwrap();
    repo.write("marked.dat", "text, but declared binary\nsecond line\n")
        .unwrap();
    repo.write("plain.txt", "one\ntwo\n").unwrap();
    repo.commit(who, 1_770_000_000, "Declare the data files binary\n")
        .unwrap();
    let fixture = repo.finish().unwrap();

    let lines = |files: &Files, path: &str| {
        files
            .entries
            .iter()
            .find(|entry| entry.path == path)
            .map(|entry| entry.lines)
            .unwrap_or_else(|| panic!("{path} is not listed"))
    };

    let committed = list(&fixture);
    assert_eq!(lines(&committed, "marked.dat"), -1);
    assert_eq!(lines(&committed, "plain.txt"), 2);

    let bare = list(&fixture.git_clone(&["--bare"]).unwrap());

    // Now say the opposite in the work tree, without committing it.
    fs::write(fixture.path().join(".gitattributes"), "*.txt binary\n").unwrap();
    let after_edit = list(&fixture);

    if committed.attributes_from_commit {
        assert_eq!(bare, committed, "a bare clone reads the same attributes");
        assert_eq!(after_edit, committed, "an uncommitted edit changes nothing");
    } else {
        // A git older than 2.41 cannot read attributes from a commit. The
        // engine says so through this field, and the numbers follow the
        // work tree.
        assert!(!bare.attributes_from_commit);
        assert_eq!(lines(&bare, "marked.dat"), 2);
        assert_eq!(lines(&after_edit, "plain.txt"), -1);
    }
}
