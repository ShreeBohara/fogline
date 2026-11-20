//! The Fogline engine.
//!
//! This crate turns a git repository into facts. It never prints and never
//! exits the process; the `fogline` binary owns both.
//!
//! Phase 1 fills it in milestone by milestone (`docs/specs/phase-1.md`). So
//! far it knows how to call git safely, how to tell which git it found, how
//! to find a repository and the commit its HEAD points to, and how to list
//! the files at that commit with their sizes and line counts.

pub mod classify;
pub mod files;
pub mod git;
pub mod repo;
