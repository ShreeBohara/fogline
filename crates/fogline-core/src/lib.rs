//! The Fogline engine.
//!
//! This crate turns a git repository into facts. It never prints and never
//! exits the process; the `fogline` binary owns both.
//!
//! Phase 1 fills it in milestone by milestone (`docs/specs/phase-1.md`). So
//! far it knows how to call git safely and how to tell which git it found.

pub mod git;
