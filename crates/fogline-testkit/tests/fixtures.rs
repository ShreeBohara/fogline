//! Every fixture must be what its recipe says, on every machine.

use std::collections::BTreeSet;

use fogline_testkit::{f1, f4, f5};

#[test]
fn f1_has_its_recorded_head() {
    let fixture = f1::build().unwrap();
    assert_eq!(
        fixture.head(),
        Some(f1::HEAD),
        "F1 must have the same commit id on every machine and every git version"
    );
}

#[test]
fn f1_has_the_shape_its_recipe_describes() {
    let fixture = f1::build().unwrap();

    let commits = fixture.git(["rev-list", "--count", "HEAD"]).unwrap();
    assert_eq!(commits, f1::COMMITS.to_string());

    let files = fixture
        .git(["ls-tree", "-r", "--name-only", "HEAD"])
        .unwrap();
    assert_eq!(files.lines().collect::<Vec<_>>(), f1::FILES);

    let emails = fixture.git(["log", "--format=%ae"]).unwrap();
    let authors: BTreeSet<&str> = emails.lines().collect();
    assert_eq!(authors.len(), 2);

    let branch = fixture.git(["symbolic-ref", "--short", "HEAD"]).unwrap();
    assert_eq!(branch, "main");

    let status = fixture.git(["status", "--porcelain"]).unwrap();
    assert_eq!(status, "", "the work tree should match HEAD");
}

#[test]
fn building_f1_twice_gives_the_same_repository() {
    let first = f1::build().unwrap();
    let second = f1::build().unwrap();
    assert_eq!(first.head(), second.head());
    assert_ne!(first.path(), second.path());
}

#[test]
fn f4_has_a_branch_but_no_commits() {
    let fixture = f4::build().unwrap();
    assert_eq!(fixture.head(), None);
    assert_eq!(fixture.git(["rev-list", "--all"]).unwrap(), "");
    let branch = fixture.git(["symbolic-ref", "HEAD"]).unwrap();
    assert_eq!(branch, "refs/heads/main");
}

#[test]
fn f5_is_a_shallow_clone_of_f1() {
    let fixture = f5::build().unwrap();
    assert_eq!(fixture.head(), Some(f5::HEAD));

    let shallow = fixture
        .git(["rev-parse", "--is-shallow-repository"])
        .unwrap();
    assert_eq!(shallow, "true");

    let commits = fixture.git(["rev-list", "--count", "HEAD"]).unwrap();
    assert_eq!(commits, f5::COMMITS.to_string());

    let files = fixture
        .git(["ls-tree", "-r", "--name-only", "HEAD"])
        .unwrap();
    assert_eq!(files.lines().collect::<Vec<_>>(), f1::FILES);
}

#[test]
fn a_bare_clone_keeps_the_head_and_has_no_work_tree() {
    let original = f1::build().unwrap();
    let bare = original.git_clone(&["--bare"]).unwrap();
    assert_eq!(bare.head(), Some(f1::HEAD));
    let is_bare = bare.git(["rev-parse", "--is-bare-repository"]).unwrap();
    assert_eq!(is_bare, "true");
}
