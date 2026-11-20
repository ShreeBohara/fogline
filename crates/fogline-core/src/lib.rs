//! The Fogline engine.
//!
//! This crate turns a git repository into facts. It never prints and never
//! exits the process; the `fogline` binary owns both.
//!
//! Phase 1 fills it in milestone by milestone (`docs/specs/phase-1.md`). So
//! far it knows how to call git safely, how to tell which git it found, how
//! to find a repository and the commit its HEAD points to, how to list the
//! files at that commit with their sizes and line counts, and how to read
//! the history behind it: every commit, and the lines each one added to and
//! deleted from each file.

pub mod aggregate;
pub mod classify;
pub mod files;
pub mod git;
pub mod history;
mod numstat;
pub mod repo;
