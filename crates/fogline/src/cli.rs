//! The command-line grammar (spec section 5.1). Commands are added here as
//! the milestones that implement them land.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "fogline",
    version,
    about = "Maps which parts of a codebase arrived in agent-attributed commits.",
    after_help = "Pre-alpha: `scan` counts a repository's files, lines and commits, says how many commits are agent-attributed, and prints a summary. It does not write a bundle or draw a map yet.",
    arg_required_else_help = true,
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Scan a repository and print a summary of its files and its history
    Scan {
        /// The repository to scan, or any folder inside it
        #[arg(default_value = ".")]
        path: PathBuf,
        /// How much history to read
        #[arg(long, value_enum, value_name = "WHICH", default_value_t = HistoryChoice::All)]
        history: HistoryChoice,
        /// How many git processes read history at the same time, from 1 to
        /// 64 [default: the number of processor cores, at most 12]
        #[arg(long, value_name = "N", value_parser = clap::value_parser!(u8).range(1..=64))]
        jobs: Option<u8>,
    },
    /// Print the version
    Version {
        /// Print one line of JSON with build and environment details
        #[arg(long)]
        json: bool,
    },
}

/// What `--history` accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum HistoryChoice {
    /// Every commit that HEAD can reach
    All,
    /// No history: only the files at HEAD
    None,
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::{Cli, Command, HistoryChoice};

    #[test]
    fn the_grammar_is_internally_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn scan_reads_all_history_unless_told_otherwise() {
        let cli = Cli::try_parse_from(["fogline", "scan"]).unwrap();
        let Command::Scan { history, jobs, .. } = cli.command else {
            panic!("expected the scan command");
        };
        assert_eq!(history, HistoryChoice::All);
        assert_eq!(jobs, None);

        let cli =
            Cli::try_parse_from(["fogline", "scan", "--history", "none", "--jobs", "4"]).unwrap();
        let Command::Scan { history, jobs, .. } = cli.command else {
            panic!("expected the scan command");
        };
        assert_eq!(history, HistoryChoice::None);
        assert_eq!(jobs, Some(4));
    }

    #[test]
    fn jobs_is_a_number_from_1_to_64() {
        // The engine's own limit, which the grammar and its help text repeat.
        assert_eq!(fogline_core::history::MOST_JOBS, 64);
        for good in ["1", "11", "64"] {
            assert!(Cli::try_parse_from(["fogline", "scan", "--jobs", good]).is_ok());
        }
        for bad in ["0", "65", "-1", "many", ""] {
            assert!(
                Cli::try_parse_from(["fogline", "scan", "--jobs", bad]).is_err(),
                "--jobs {bad:?} should be refused"
            );
        }
    }
}
