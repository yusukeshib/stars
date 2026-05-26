//! Integration test that verifies the real `data/manifest.toml` checked into
//! this repository. Runs under `cargo test --workspace`, so any data file
//! whose bytes drift from what the manifest declares will fail CI without
//! needing `make manifest-check` separately.

use std::path::{Path, PathBuf};

use stars_manifest::{load, verify_all};

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `crates/manifest`; the repo root is two levels up.
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .and_then(Path::parent)
        .expect("repo root above crates/manifest")
        .to_path_buf()
}

#[test]
fn repository_manifest_parses() {
    let root = repo_root();
    let path = root.join("data").join("manifest.toml");
    let manifest = load(&path).expect("data/manifest.toml parses");
    assert!(
        !manifest.artifacts.is_empty(),
        "repository manifest declares no artifacts"
    );
}

#[test]
fn repository_manifest_verifies_clean() {
    let root = repo_root();
    let path = root.join("data").join("manifest.toml");
    let manifest = load(&path).expect("load manifest");
    let outcomes = verify_all(&root, &manifest).expect("walk manifest");
    let mut failures = Vec::new();
    for (id, outcome) in &outcomes {
        if outcome.is_fatal() {
            failures.push(format!("{id}: {outcome:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "data/manifest.toml has drifted from on-disk bytes:\n  {}",
        failures.join("\n  ")
    );
}
