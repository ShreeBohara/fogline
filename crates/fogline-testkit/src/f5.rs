//! F5 "shallow": a depth-1 clone of F1. It has F1's HEAD and none of the
//! commits before it.

use std::io;

use crate::{Fixture, f1};

/// The same commit as F1's HEAD.
pub const HEAD: &str = f1::HEAD;

/// The number of commits the clone holds.
pub const COMMITS: usize = 1;

/// Builds F1, clones it at depth 1 into a fresh temporary directory, and
/// keeps only the clone.
pub fn build() -> io::Result<Fixture> {
    f1::build()?.git_clone(&["--depth", "1", "--no-local"])
}
