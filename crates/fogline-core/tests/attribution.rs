//! Attribution against a real repository: fixture F2, which has one commit
//! for each mark in the table of signatures and the cases that are easy to
//! get wrong.
//!
//! F2 says how each of its commits has to be read, in words of its own. The
//! engine has to agree commit for commit, and its sums have to be the sums
//! of F2's own table.

use std::collections::BTreeMap;

use fogline_core::aggregate::{self, SignatureTotals};
use fogline_core::attribution::{self, Class};
use fogline_core::history::{self, Commit, History};
use fogline_core::repo;
use fogline_core::signatures::{self, Kind};
use fogline_testkit::{f1, f2, f3};

fn read(fixture: &fogline_testkit::Fixture) -> History {
    let found = repo::discover(&fixture.path()).expect("the fixture is a repository");
    history::read(&found, 3).expect("its history can be read")
}

/// The first line of a commit's message, which is its name in F2.
fn name(commit: &Commit) -> &str {
    commit.message.lines().next().unwrap_or_default()
}

/// F2's word for a class, as the engine's.
fn class(entry: &f2::Entry) -> Class {
    match entry.class {
        f2::Class::AgentAttributed => Class::AgentAttributed,
        f2::Class::Automation => Class::Automation,
        f2::Class::Unsigned => Class::Unsigned,
    }
}

#[test]
fn every_commit_of_f2_is_read_as_its_recipe_says() {
    let history = read(&f2::build().unwrap());
    let entries = f2::commits();
    assert_eq!(history.commits.len(), entries.len());

    let by_name: BTreeMap<&str, &Commit> = history
        .commits
        .iter()
        .map(|commit| (name(commit), commit))
        .collect();
    assert_eq!(
        by_name.len(),
        entries.len(),
        "every commit has its own name"
    );

    for entry in &entries {
        let commit = by_name
            .get(entry.name)
            .unwrap_or_else(|| panic!("F2 has no commit called {:?}", entry.name));
        let attribution = attribution::of(commit);
        assert_eq!(attribution.class, class(entry), "{}", entry.name);
        assert_eq!(attribution.signatures, entry.signatures, "{}", entry.name);
        let added: u64 = commit.changes.iter().map(|change| change.added).sum();
        assert_eq!(added, entry.added, "{}", entry.name);
    }
}

/// The spec's check for this part: every mark of the table has a commit in
/// F2 that carries it, and that commit is read as the mark's signature.
#[test]
fn every_mark_of_the_table_has_its_commit_in_f2() {
    let history = read(&f2::build().unwrap());
    let entries = f2::commits();
    for signature in signatures::TABLE {
        for mark in signature.marks {
            let commit = history
                .commits
                .iter()
                .find(|commit| name(commit) == mark.fixture)
                .unwrap_or_else(|| panic!("F2 has no commit called {:?}", mark.fixture));
            assert_eq!(
                attribution::of(commit).signatures,
                [signature.id],
                "{}",
                mark.fixture
            );
            // F2 says the same, so the two tables cannot drift apart.
            let entry = entries
                .iter()
                .find(|entry| entry.name == mark.fixture)
                .unwrap_or_else(|| panic!("F2's table has no {:?}", mark.fixture));
            assert_eq!(entry.signatures, [signature.id], "{}", mark.fixture);
        }
    }
}

#[test]
fn the_sums_of_f2_are_the_sums_of_its_recipe() {
    let history = read(&f2::build().unwrap());
    let entries = f2::commits();
    let of_class = |wanted: f2::Class| -> (u64, u64) {
        let chosen = entries.iter().filter(|entry| entry.class == wanted);
        (
            chosen.clone().count() as u64,
            chosen.map(|entry| entry.added).sum(),
        )
    };
    let (agent_commits, agent_lines) = of_class(f2::Class::AgentAttributed);
    let (bot_commits, bot_lines) = of_class(f2::Class::Automation);
    let (unsigned_commits, _) = of_class(f2::Class::Unsigned);
    // A fixture that had none of a class would prove nothing about it.
    assert!(agent_commits > 0 && bot_commits > 0 && unsigned_commits > 0);

    let totals = aggregate::totals(&history);
    assert_eq!(totals.commits, entries.len() as u64);
    assert_eq!(totals.merges, 1);
    assert_eq!(totals.commits_agent, agent_commits);
    assert_eq!(totals.commits_automation, bot_commits);
    assert_eq!(
        totals.added,
        entries.iter().map(|entry| entry.added).sum::<u64>()
    );
    assert_eq!(totals.added_agent, agent_lines);
    assert_eq!(totals.added_automation, bot_lines);
    assert_eq!(totals.deleted, 0);

    // Each signature: the commits whose entry lists it, in the table's order.
    let expected: Vec<SignatureTotals> = signatures::TABLE
        .iter()
        .filter_map(|signature| {
            let carrying: Vec<&f2::Entry> = entries
                .iter()
                .filter(|entry| entry.signatures.contains(&signature.id))
                .collect();
            (!carrying.is_empty()).then(|| SignatureTotals {
                id: signature.id,
                commits: carrying.len() as u64,
                added: carrying.iter().map(|entry| entry.added).sum(),
            })
        })
        .collect();
    assert_eq!(aggregate::by_signature(&history), expected);
    // Every signature of the table is on some commit of F2.
    assert_eq!(expected.len(), signatures::TABLE.len());
    // One commit carries two agents, so the signatures' commits add up to
    // more than the commits that are agent-attributed.
    let agents_summed: u64 = expected
        .iter()
        .filter(|sum| signatures::by_id(sum.id).is_some_and(|s| s.kind == Kind::Agent))
        .map(|sum| sum.commits)
        .sum();
    assert_eq!(agents_summed, agent_commits + 1);

    // Each commit adds one file of its own, so a path has the class of its
    // one commit.
    let paths = aggregate::by_path(&history);
    for entry in entries.iter().filter(|entry| entry.added > 0) {
        let path = format!("notes/{}.txt", entry.name);
        let sums = paths[path.as_str()];
        let agent = entry.class == f2::Class::AgentAttributed;
        let bot = entry.class == f2::Class::Automation;
        assert_eq!(
            (sums.commits, sums.commits_agent, sums.added),
            (1, u64::from(agent), entry.added),
            "{path}"
        );
        assert_eq!(
            sums.added_agent,
            if agent { entry.added } else { 0 },
            "{path}"
        );
        assert_eq!(
            sums.added_automation,
            if bot { entry.added } else { 0 },
            "{path}"
        );
    }
}

#[test]
fn the_result_does_not_depend_on_the_number_of_workers() {
    let fixture = f2::build().unwrap();
    let found = repo::discover(&fixture.path()).unwrap();
    let alone = history::read(&found, 1).unwrap();
    for jobs in [2, 5, 64] {
        let together = history::read(&found, jobs).unwrap();
        assert_eq!(aggregate::totals(&together), aggregate::totals(&alone));
        assert_eq!(
            aggregate::by_signature(&together),
            aggregate::by_signature(&alone)
        );
    }
}

/// The fixtures without agents stay without agents.
#[test]
fn a_repository_with_no_marks_has_no_agents_and_no_bots() {
    for fixture in [f1::build().unwrap(), f3::build().unwrap()] {
        let history = read(&fixture);
        let totals = aggregate::totals(&history);
        assert_eq!((totals.commits_agent, totals.commits_automation), (0, 0));
        assert_eq!((totals.added_agent, totals.added_automation), (0, 0));
        assert!(aggregate::by_signature(&history).is_empty());
        for commit in &history.commits {
            assert_eq!(attribution::of(commit).class, Class::Unsigned);
        }
    }
}
