//! The Fogline engine.
//!
//! This crate turns a git repository into facts. It never prints and never
//! exits the process; the `fogline` binary owns both.
//!
//! Phase 1 fills it in milestone by milestone (`docs/specs/phase-1.md`). So
//! far it knows how to call git safely, how to tell which git it found, and
//! how to find a repository and the commit its HEAD points to.

pub mod git;
pub mod repo;
