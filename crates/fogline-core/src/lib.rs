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
//! deleted from each file. It also reads the marks that coding agents leave
//! on commits, and says which commits are agent-attributed. A file's
//! history follows it through its renames, and every scan knows the caveats
//! that its numbers come with.

pub mod aggregate;
pub mod attribution;
pub mod caveats;
pub mod classify;
pub mod files;
pub mod git;
pub mod history;
mod numstat;
pub mod repo;
pub mod signatures;
