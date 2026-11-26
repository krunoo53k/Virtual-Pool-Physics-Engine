//! Runtime-reported build identity for the compatibility manifest check.
//!
//! The model-version string is hand-maintained and has already drifted once;
//! a client that reads it at runtime needs more than that one string. This
//! build script stamps the three pinned revisions from the workspace's
//! `compat.json`, the git revision of the checkout, and the compilation target
//! into the extension. These values are informational: they never enter
//! physics, the state hash or the canary.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    // A linked worktree has a `.git` *file*, so `../.git/refs` is not a path
    // there; ask git for the real, shared git directory instead.
    let git_dir = git_common_dir();
    for name in ["HEAD", "refs", "packed-refs"] {
        let path = match &git_dir {
            Some(dir) => dir.join(name),
            None => PathBuf::from("../.git").join(name),
        };
        println!("cargo:rerun-if-changed={}", path.display());
    }
    // Cargo stops watching the package directory once any rerun-if-changed is
    // emitted, so list the manifest explicitly.
    println!("cargo:rerun-if-changed=../compat.json");

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

    // The compatibility pins are read from the manifest, not from the build's
    // HEAD: a checkout can have local commits (or none at all) and still need
    // to report the revisions the protocol was pinned to.
    let manifest = std::fs::read_to_string("../compat.json").unwrap_or_default();
    for (field, variable) in [
        ("core_rev", "POOL_CORE_REV"),
        ("intent_rev", "POOL_INTENT_REV"),
        ("rules_rev", "POOL_RULES_REV"),
    ] {
        let pinned = string_field(&manifest, field)
            .filter(|rev| !rev.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        println!("cargo:rustc-env={variable}={pinned}");
    }

    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=POOL_TARGET={target}");
}

/// The absolute path to the shared git directory, or `None` when git is not
/// available (a source tarball, for example). A relative `--git-common-dir` is
/// already relative to this script's working directory, the package root.
fn git_common_dir() -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if text.is_empty() {
        return None;
    }
    let path = PathBuf::from(text);
    Some(std::fs::canonicalize(&path).unwrap_or(path))
}

/// The value of a quoted string field in a small JSON object. Enough for a
/// flat manifest like `compat.json`, and it keeps the build script dependency
/// free.
fn string_field(text: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
    let after_key = &text[text.find(&marker)? + marker.len()..];
    let after_colon = &after_key[after_key.find(':')? + 1..];
    let quoted = after_colon.trim_start().strip_prefix('"')?;
    let end = quoted.find('"')?;
    Some(quoted[..end].to_string())
}
