// build.rs - Extract version from git tags at compile time
//
// This ensures the helper reports the same git-based version as the CLI.

use std::process::Command;

fn main() {
    // Tell Cargo to rerun this script if git HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags");

    let version = get_git_version().unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

    println!("cargo:rustc-env=GIT_VERSION={}", version);
}

/// Get version from the nearest git tag
fn get_git_version() -> Option<String> {
    // Try exact tag first
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

    // Fallback: nearest tag with commit distance
    let output = Command::new("git")
        .args(["describe", "--tags", "--always"])
        .output()
        .ok()?;

    if output.status.success() {
        let desc = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let desc = desc.trim_start_matches('v');

        if desc.contains('-') {
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
