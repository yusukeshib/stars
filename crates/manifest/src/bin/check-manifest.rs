//! Verify `data/manifest.toml` against the on-disk bytes of every artifact it
//! declares. Invoked by `make manifest-check` and wired into `make ci`.
//!
//! Exit codes:
//! - 0 — every artifact verified (or has nothing to verify, e.g. runtime
//!   services).
//! - 1 — at least one fatal finding (missing file, hash drift, size drift).
//! - 2 — manifest could not be parsed.
//!
//! Run from the repository root; pass `--root <dir>` to point elsewhere
//! (mostly useful when running under integration tests).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use stars_manifest::{load, verify_all, VerifyOutcome};

#[derive(Debug, Parser)]
#[command(
    name = "check-manifest",
    about = "Verify data/manifest.toml against on-disk bytes."
)]
struct Args {
    /// Repository root containing `data/manifest.toml` and the artifact paths
    /// it references. Defaults to the current working directory.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// Print every artifact, including those that verified cleanly.
    #[arg(long)]
    verbose: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let manifest_path = args.root.join("data").join("manifest.toml");

    let manifest = match load(&manifest_path) {
        Ok(m) => m,
        Err(err) => {
            eprintln!(
                "check-manifest: failed to load {}: {err}",
                manifest_path.display()
            );
            return ExitCode::from(2);
        }
    };

    let outcomes = match verify_all(&args.root, &manifest) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("check-manifest: I/O error while verifying: {err}");
            return ExitCode::from(1);
        }
    };

    let mut fatal = 0usize;
    let mut ok = 0usize;
    let mut skipped = 0usize;
    for (id, outcome) in &outcomes {
        match outcome {
            VerifyOutcome::Ok => {
                ok += 1;
                if args.verbose {
                    println!("ok       {id}");
                }
            }
            VerifyOutcome::Skipped => {
                skipped += 1;
                if args.verbose {
                    println!("skipped  {id} (no local path)");
                }
            }
            VerifyOutcome::Missing { path } => {
                fatal += 1;
                eprintln!("MISSING  {id}: {path} does not exist");
            }
            VerifyOutcome::HashMismatch {
                path,
                expected,
                actual,
            } => {
                fatal += 1;
                eprintln!(
                    "DRIFT    {id}: sha256 mismatch at {path}\n  expected {expected}\n  actual   {actual}\n  fix:     update `sha256` in data/manifest.toml after \
                     reviewing the diff, or revert the file."
                );
            }
            VerifyOutcome::SizeMismatch {
                path,
                expected,
                actual,
            } => {
                fatal += 1;
                eprintln!(
                    "SIZE     {id}: bytes mismatch at {path} (manifest {expected}, on-disk {actual})"
                );
            }
        }
    }

    println!(
        "check-manifest: {ok} verified, {skipped} skipped, {fatal} failing (total {})",
        outcomes.len()
    );

    if fatal > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
