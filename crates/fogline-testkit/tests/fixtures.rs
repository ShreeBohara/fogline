//! Every fixture must be what its recipe says, on every machine.

use std::collections::BTreeSet;

use fogline_testkit::{Fixture, f1, f3, f4, f5};

/// The lines of every file at HEAD, counted by reading each blob: one per
/// line ending, plus one for a last line that has none. `None` for a blob
/// with a zero byte, which is how git recognises a binary file.
fn lines_by_reading(fixture: &Fixture, paths: &[&str]) -> Vec<Option<u64>> {
    paths
        .iter()
        .map(|path| {
            let blob = fixture
                .git_bytes(["cat-file", "blob", &format!("HEAD:{path}")])
                .expect("every listed path names a blob");
            if blob.contains(&0) {
                return None;
            }
            let endings = blob.iter().filter(|&&byte| byte == b'\n').count();
            let unfinished = usize::from(blob.last().is_some_and(|&last| last != b'\n'));
            Some(u64::try_from(endings + unfinished).expect("a fixture file is small"))
        })
        .collect()
}

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

    let lines: u64 = lines_by_reading(&fixture, &f1::FILES)
        .into_iter()
        .flatten()
        .sum();
    assert_eq!(lines, f1::LINES);
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

#[test]
fn f3_has_its_recorded_head() {
    let fixture = f3::build().unwrap();
    assert_eq!(
        fixture.head(),
        Some(f3::HEAD),
        "F3 must have the same commit id on every machine and every git version"
    );
}

#[test]
fn f3_has_the_shape_its_recipe_describes() {
    let fixture = f3::build().unwrap();

    let commits = fixture.git(["rev-list", "--count", "HEAD"]).unwrap();
    assert_eq!(commits, f3::COMMITS.to_string());
    let merges = fixture
        .git(["rev-list", "--count", "--merges", "HEAD"])
        .unwrap();
    assert_eq!(merges, f3::MERGES.to_string());

    // `-z` gives the paths as they are. Without it git would print the one
    // that is not ASCII in quotes, with escapes.
    let listing = fixture
        .git_bytes(["ls-tree", "-r", "--name-only", "-z", "HEAD"])
        .unwrap();
    let listed: Vec<&str> = listing
        .split(|&byte| byte == 0)
        .filter(|name| !name.is_empty())
        .map(|name| std::str::from_utf8(name).unwrap())
        .collect();
    let mut expected = f3::FILES.to_vec();
    expected.push(f3::SUBMODULE);
    expected.sort_unstable();
    assert_eq!(listed, expected);

    let link = fixture.git(["ls-tree", "HEAD", f3::LINK]).unwrap();
    assert!(link.starts_with("120000 blob "), "{link}");
    let target = fixture
        .git(["cat-file", "blob", &format!("HEAD:{}", f3::LINK)])
        .unwrap();
    assert_eq!(target, f3::LINK_TARGET);

    let submodule = fixture.git(["ls-tree", "HEAD", f3::SUBMODULE]).unwrap();
    assert_eq!(
        submodule,
        format!("160000 commit {}\t{}", f3::SUBMODULE_COMMIT, f3::SUBMODULE)
    );

    // The two renames, as git sees them when it compares commits.
    let renames = fixture
        .git([
            "log",
            "--format=",
            "--name-status",
            "-M50%",
            "--diff-filter=R",
        ])
        .unwrap();
    assert!(
        renames.contains("R100\tsrc/old_name.py\tsrc/new_name.py"),
        "{renames}"
    );
    assert!(
        renames
            .lines()
            .any(|line| line.starts_with('R') && line.ends_with("\tsrc/report.py\tsrc/summary.py")),
        "{renames}"
    );

    // The link holds a path, not lines, so it is left out of the count.
    let files: Vec<&str> = f3::FILES
        .iter()
        .copied()
        .filter(|path| *path != f3::LINK)
        .collect();
    let counted = lines_by_reading(&fixture, &files);
    assert_eq!(counted.iter().flatten().sum::<u64>(), f3::LINES);
    assert_eq!(
        counted.iter().filter(|lines| lines.is_none()).count(),
        f3::BINARY_FILES
    );
}

#[test]
fn building_f3_twice_gives_the_same_repository() {
    let first = f3::build().unwrap();
    let second = f3::build().unwrap();
    assert_eq!(first.head(), second.head());
}
