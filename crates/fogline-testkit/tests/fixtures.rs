//! Every fixture must be what its recipe says, on every machine.

use std::collections::BTreeSet;

use fogline_testkit::f1;

#[test]
fn f1_has_its_recorded_head() {
    let fixture = f1::build().unwrap();
    assert_eq!(
        fixture.head(),
        f1::HEAD,
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
