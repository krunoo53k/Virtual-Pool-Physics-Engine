//! Runtime-reported build identity for the compatibility manifest check.
//!
//! The model-version string is hand-maintained and has already drifted once;
//! a client that reads it at runtime needs more than that one string. This
//! build script stamps the git revision and the compilation target into the
//! extension. The values are informational: they never enter physics, the
//! state hash or the canary.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs");

    let rev = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|rev| rev.trim().to_string())
        .filter(|rev| !rev.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=POOL_GIT_REV={rev}");

    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=POOL_TARGET={target}");
}
