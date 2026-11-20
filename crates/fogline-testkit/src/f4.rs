//! F4 "empty": a repository on the branch `main` with no commits, so its
//! HEAD names no commit.

use std::io;

use crate::{Fixture, RepoBuilder};

/// Builds the fixture in a fresh temporary directory.
pub fn build() -> io::Result<Fixture> {
    RepoBuilder::new()?.finish()
}
