// build.rs - Extract version from git tags at compile time
//
// This script runs during `cargo build` and sets the GIT_VERSION environment
// variable based on the nearest git tag. This allows the binary to report
// the correct version without manually updating Cargo.toml for each release.
//
// Usage:
//   1. Create a git tag: `git tag v2.6.0`
//   2. Build: `cargo build --release`
//   3. The binary will report version "2.6.0"
//
// Fallback: If git is unavailable or there are no tags, it falls back to
// the version specified in Cargo.toml.

use std::process::Command;

fn main() {
    // Tell Cargo to rerun this script if git HEAD changes (new commits/tags)
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");

    let version = get_git_version().unwrap_or_else(|| {
        // Fallback to Cargo.toml version
        env!("CARGO_PKG_VERSION").to_string()
    });

    println!("cargo:rustc-env=GIT_VERSION={}", version);

    // Also provide git commit hash for detailed version info
    if let Some(commit) = get_git_commit_short() {
        println!("cargo:rustc-env=GIT_COMMIT={}", commit);
    }

    // Provide build timestamp
    let build_date = chrono_lite_date();
    println!("cargo:rustc-env=BUILD_DATE={}", build_date);
}

/// Get version from the nearest git tag
/// Strips the 'v' prefix if present (e.g., "v2.5.0" -> "2.5.0")
fn get_git_version() -> Option<String> {
    // Try to get the exact tag first (if HEAD is tagged)
    let output = Command::new("git")
        .args(["describe", "--tags", "--exact-match", "HEAD"])
        .output()
        .ok();

    if let Some(out) = output {
        if out.status.success() {
            let tag = String::from_utf8_lossy(&out.stdout)
                .trim()
                .trim_start_matches('v')
                .to_string();
            if !tag.is_empty() {
                return Some(tag);
            }
        }
    }

    // Fallback: get the nearest tag with commit distance (e.g., "v2.5.0-3-gabcdef")
    let output = Command::new("git")
        .args(["describe", "--tags", "--always"])
        .output()
        .ok()?;

    if output.status.success() {
        let desc = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // Parse "v2.5.0-3-gabcdef" -> "2.5.0-dev.3+gabcdef" (semver-ish)
        let desc = desc.trim_start_matches('v');

        if desc.contains('-') {
            // Has commits after tag: convert to dev version
            let parts: Vec<&str> = desc.splitn(3, '-').collect();
            if parts.len() >= 2 {
                return Some(format!("{}-dev.{}", parts[0], parts[1]));
            }
        }

        if !desc.is_empty() {
            return Some(desc.to_string());
        }
    }

    None
}

/// Get short git commit hash
fn get_git_commit_short() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;

    if output.status.success() {
        let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !commit.is_empty() {
            return Some(commit);
        }
    }

    None
}

/// Get current date in YYYY-MM-DD format without external dependencies
fn chrono_lite_date() -> String {
    // Use git to get a consistent timestamp, or fall back to a placeholder
    let output = Command::new("git")
        .args(["log", "-1", "--format=%cs"])
        .output()
        .ok();

    if let Some(out) = output {
        if out.status.success() {
            let date = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !date.is_empty() {
                return date;
            }
        }
    }

    // Fallback: use environment or placeholder
    std::env::var("SOURCE_DATE_EPOCH")
        .map(|_| "reproducible".to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}
