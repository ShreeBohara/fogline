//! The command-line grammar (spec section 5.1). Commands are added here as
//! the milestones that implement them land.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "fogline",
    version,
    about = "Maps which parts of a codebase arrived in agent-attributed commits.",
    after_help = "Pre-alpha: `version` is the only command that works so far.",
    arg_required_else_help = true,
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Scan a repository and print a summary (not implemented yet)
    Scan {
        /// Repository to scan
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Print the version
    Version {
        /// Print one line of JSON with build and environment details
        #[arg(long)]
        json: bool,
    },
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn the_grammar_is_internally_consistent() {
        Cli::command().debug_assert();
    }
}
