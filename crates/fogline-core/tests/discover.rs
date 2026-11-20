//! Discovery against real repositories built by the test kit.
//!
//! The cases that depend on the machine's git (missing, or too old) need a
//! different PATH, so they live in the binary's contract tests instead.

use std::fs;
use std::path::Path;

use fogline_core::repo::{self, Error, ObjectFormat};
use fogline_testkit::{f1, f4, f5};

/// Compares two paths after resolving symbolic links, because git reports
/// the real path (on macOS the temporary folder is behind a link).
fn same_place(a: &Path, b: &Path) -> bool {
    let real = |path: &Path| fs::canonicalize(path).expect("both paths should exist");
    real(a) == real(b)
}

#[test]
fn finds_a_repository_from_its_top_folder() {
    let fixture = f1::build().unwrap();
    let found = repo::discover(&fixture.path()).unwrap();

    assert!(same_place(&found.root, &fixture.path()));
    assert_eq!(found.name, "repo");
    assert_eq!(found.head, f1::HEAD);
    assert_eq!(found.branch.as_deref(), Some("main"));
    assert_eq!(found.object_format, ObjectFormat::Sha1);
    assert!(!found.bare);
    assert!(!found.shallow);
    assert!(!found.partial_clone);
}

#[test]
fn notices_a_partial_clone() {
    let source = f1::build().unwrap();
    for checkout in [false, true] {
        let clone = source.partial_clone(checkout).unwrap();
        let found = repo::discover(&clone.path()).unwrap();
        assert!(found.partial_clone, "checkout: {checkout}");
        assert_eq!(found.head, f1::HEAD);
    }
}

#[test]
fn a_folder_inside_the_work_tree_resolves_to_the_top() {
    let fixture = f1::build().unwrap();
    let found = repo::discover(&fixture.path().join("src")).unwrap();
    assert!(same_place(&found.root, &fixture.path()));
    assert_eq!(found.head, f1::HEAD);
}

#[test]
fn a_path_inside_the_git_folder_reads_that_repository() {
    let fixture = f1::build().unwrap();
    let git_folder = fixture.path().join(".git");
    let found = repo::discover(&git_folder.join("refs")).unwrap();
    assert!(same_place(&found.root, &git_folder));
    assert_eq!(found.name, "repo");
    assert_eq!(found.head, f1::HEAD);
}

#[test]
fn reads_a_bare_repository() {
    let original = f1::build().unwrap();
    let bare = original.git_clone(&["--bare"]).unwrap();
    let found = repo::discover(&bare.path()).unwrap();
    assert!(found.bare);
    assert!(same_place(&found.root, &bare.path()));
    assert_eq!(found.head, f1::HEAD);
    assert_eq!(found.branch.as_deref(), Some("main"));
}

#[test]
fn notices_a_shallow_clone() {
    let fixture = f5::build().unwrap();
    let found = repo::discover(&fixture.path()).unwrap();
    assert!(found.shallow);
    assert_eq!(found.head, f5::HEAD);
}

#[test]
fn a_detached_head_has_no_branch() {
    let fixture = f1::build().unwrap();
    fixture
        .git(["checkout", "--quiet", "--detach", "HEAD~1"])
        .unwrap();
    let expected = fixture.git(["rev-parse", "HEAD"]).unwrap();

    let found = repo::discover(&fixture.path()).unwrap();
    assert_eq!(found.branch, None);
    assert_eq!(found.head, expected);
}

#[test]
fn a_repository_with_no_commits_is_refused() {
    let fixture = f4::build().unwrap();
    match repo::discover(&fixture.path()) {
        Err(Error::NoCommits(root)) => assert!(same_place(&root, &fixture.path())),
        other => panic!("expected NoCommits, got {other:?}"),
    }
}

#[test]
fn a_folder_outside_any_repository_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let result = repo::discover(dir.path());
    assert!(
        matches!(result, Err(Error::NotARepository(_))),
        "got {result:?}"
    );
}

#[test]
fn a_missing_path_and_a_file_are_refused_before_git_runs() {
    let fixture = f1::build().unwrap();

    let missing = fixture.path().join("no-such-folder");
    assert!(matches!(
        repo::discover(&missing),
        Err(Error::NoSuchPath(path)) if path == missing
    ));

    let file = fixture.path().join("src/app.py");
    assert!(matches!(
        repo::discover(&file),
        Err(Error::NotAFolder(path)) if path == file
    ));
}
