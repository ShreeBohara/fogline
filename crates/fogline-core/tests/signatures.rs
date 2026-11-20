//! The table of signatures against GitHub.
//!
//! Every mark in the table names a public commit that carries it and quotes
//! the line. This reads each of those commits again and checks that the
//! line is really there. It needs the network and the GitHub command line
//! tool `gh`, signed in, so it does not run with the other tests:
//!
//! ```text
//! cargo test -p fogline-core --test signatures -- --ignored --nocapture
//! ```
//!
//! A commit can disappear with its repository. When this fails for that
//! reason, the mark needs a new commit to stand on, or has to go.

use std::process::Command;

use fogline_core::signatures::{self, Rule};

/// Asks GitHub for one field of a commit, as plain text.
fn ask(repository: &str, commit: &str, field: &str) -> String {
    let url = format!("repos/{repository}/commits/{commit}");
    let output = Command::new("gh")
        .args(["api", &url, "--jq", field])
        .output()
        .expect("the GitHub command line tool `gh` can be started");
    assert!(
        output.status.success(),
        "gh could not read {repository} at {commit}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
#[ignore = "needs the network and the GitHub command line tool"]
fn every_mark_is_on_the_commit_it_names() {
    for signature in signatures::TABLE {
        for mark in signature.marks {
            let seen = &mark.seen;
            // GitHub answers to a shortened id too. The table has to hold
            // the whole one.
            let id = ask(seen.repository, seen.commit, ".sha");
            assert_eq!(id.trim(), seen.commit, "{}", signature.id);

            match mark.rule {
                Rule::Author { .. } => {
                    let field = r#".commit.author.name + " <" + .commit.author.email + ">""#;
                    let author = ask(seen.repository, seen.commit, field);
                    assert_eq!(author.trim_end(), seen.line, "{}", signature.id);
                }
                Rule::Trailer { .. } | Rule::TrailerKey { .. } | Rule::Line { .. } => {
                    let message = ask(seen.repository, seen.commit, ".commit.message");
                    assert!(
                        message.lines().any(|line| line.trim() == seen.line),
                        "{}: {:?} is not a line of {} at {}",
                        signature.id,
                        seen.line,
                        seen.repository,
                        seen.commit
                    );
                }
            }
            println!(
                "{:<12} {:<18} {} at {}",
                signature.id,
                mark.fixture,
                seen.repository,
                &seen.commit[..10]
            );
        }
    }
}
