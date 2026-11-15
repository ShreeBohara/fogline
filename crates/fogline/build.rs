//! Hands two build-time facts to the crate: the target triple, which only
//! build scripts can see, and the commit a release was built from, which the
//! release workflow passes in `FOGLINE_BUILD_COMMIT`. The script never runs
//! git, so a build from a source archive works the same way.

use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=FOGLINE_BUILD_COMMIT");

    let target = env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned());
    println!("cargo:rustc-env=FOGLINE_TARGET={target}");

    if let Ok(commit) = env::var("FOGLINE_BUILD_COMMIT")
        && !commit.is_empty()
    {
        println!("cargo:rustc-env=FOGLINE_BUILD_COMMIT={commit}");
    }
}
