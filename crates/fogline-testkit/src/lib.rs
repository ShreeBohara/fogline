//! Test support for Fogline: small git repositories built on demand.
//!
//! A fixture is built by running git with every source of variation pinned:
//! names, dates, configuration and line endings. The same recipe therefore
//! produces the same commit ids on every machine. Each fixture records the
//! HEAD it must have, and a test checks it, so a fixture that drifts fails
//! loudly instead of quietly changing what other tests see.

pub mod f1;
pub mod f4;
pub mod f5;
mod repo;

pub use repo::{Fixture, Identity, RepoBuilder};
