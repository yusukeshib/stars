//! Machine-readable data provenance manifest for `stars`.
//!
//! The manifest itself lives at `data/manifest.toml` at the repository root.
//! It records every committed data artifact, every runtime web service, and
//! every regenerable derived artifact (notebook fixtures, scene presets,
//! rendered gallery PNGs) with a stable identifier, source citation, license,
//! local path, SHA-256 hash, preprocessing command, and field list.
//!
//! This module is the schema + load + verify layer:
//!
//! - [`Manifest`] / [`Artifact`] / [`ArtifactKind`] mirror the TOML schema.
//! - [`load`] parses a `manifest.toml` from disk.
//! - [`verify_artifact`] re-hashes the local file and reports mismatches.
//! - [`verify_all`] walks every artifact and returns a list of findings.
//!
//! Other crates can depend on this library to resolve artifact IDs to a pinned
//! `(path, sha256, source)` tuple — that is the lever JSON sessions
//! (`stars-host-common::session`) and the catalog backend will use to name the
//! exact data snapshot a render consumed (see roadmap P3-01, P3-07, and
//! `docs/catalog-backend-design.md`).

#![deny(missing_docs)]

use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Schema version currently understood by this crate. Bump when the on-disk
/// shape changes in a way old readers cannot ignore. Adding a new optional
/// field is not a schema-version bump; renaming or removing a field is.
pub const SCHEMA_VERSION: u32 = 1;

/// Top-level shape of `data/manifest.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Schema version. The loader rejects manifests with `schema_version`
    /// greater than [`SCHEMA_VERSION`] to avoid silently misreading future
    /// shapes.
    pub schema_version: u32,
    /// Every recorded artifact. Order is informational only; lookups should
    /// go through [`Manifest::by_id`].
    #[serde(rename = "artifact", default)]
    pub artifacts: Vec<Artifact>,
}

impl Manifest {
    /// Find an artifact by its stable `id`. Returns `None` if unknown.
    pub fn by_id(&self, id: &str) -> Option<&Artifact> {
        self.artifacts.iter().find(|a| a.id == id)
    }
}

/// How an artifact is materialised in the repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactKind {
    /// Third-party data that is checked in or fetched into the repository and
    /// embedded into the build (CSV, binary tables, …). Requires `path` and
    /// `sha256` so review hooks can pin the exact bytes.
    Embedded,
    /// Output produced from project sources via a documented command and
    /// checked in for reproducibility (scene preset JSONs, notebook fixture
    /// CSVs, validation gallery PNGs). Requires `path` and `sha256`; also
    /// requires `preprocessing` so the regeneration command is discoverable.
    Generated,
    /// Endpoint that the application calls at runtime. Has no local bytes, so
    /// `path` and `sha256` are absent.
    RuntimeService,
}

impl fmt::Display for ArtifactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ArtifactKind::Embedded => "embedded",
            ArtifactKind::Generated => "generated",
            ArtifactKind::RuntimeService => "runtime-service",
        })
    }
}

/// A single manifest entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// Stable identifier referenced from JSON sessions, catalog backends, and
    /// citation metadata. Use a short kebab-case slug, optionally suffixed by
    /// a version (`hyg-v4.2`, `d3-celestial-constellation-lines`).
    pub id: String,
    /// One-line description for human review.
    pub description: String,
    /// How the artifact is materialised. See [`ArtifactKind`].
    pub kind: ArtifactKind,
    /// Repository-relative path. Required for `embedded` and `generated`; must
    /// be absent for `runtime-service`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Lowercase hex SHA-256 of the bytes at `path`. Required for `embedded`
    /// and `generated`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Informational file size in bytes (cross-check against `path`'s metadata).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    /// Primary source — usually a URL to the upstream archive, repository, or
    /// catalogue. For pure literature references that have no machine-readable
    /// download, leave empty and put the citation in [`Artifact::citation`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Stable archive identifier (DOI, VizieR catalogue ID, ADS bibcode, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_id: Option<String>,
    /// Free-form literature citation when there is no canonical URL/ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citation: Option<String>,
    /// Upstream version or release identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// ISO 8601 date the artifact was retrieved from upstream (yyyy-mm-dd).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieved: Option<String>,
    /// SPDX identifier or short license string. Use `"public-domain"` or
    /// `"see-upstream"` only when no SPDX value applies.
    pub license: String,
    /// URL where the license text or terms can be reviewed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_url: Option<String>,
    /// Command, script, or build-script path that regenerates the artifact
    /// from its source. Required for `generated`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preprocessing: Option<String>,
    /// Endpoint URL — required when [`Artifact::kind`] is `runtime-service`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Fields, columns, or schema keys that `stars` actually reads. Documents
    /// the contract so a future upstream column rename is easy to flag.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields_used: Vec<String>,
    /// Repository-relative paths or crate names that consume the artifact.
    /// Informational; not validated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub used_by: Vec<String>,
    /// Known caveats: filtering rules applied at load, epoch caveats, license
    /// caveats, driver-dependent regeneration, …
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limitations: Option<String>,
}

/// Errors raised by the schema layer.
#[derive(Debug)]
pub enum ManifestError {
    /// I/O failure reading the manifest or an artifact file.
    Io(io::Error),
    /// TOML parse failure.
    Toml(toml::de::Error),
    /// The manifest declares a schema version this crate does not understand.
    UnsupportedSchemaVersion {
        /// The version the manifest declared.
        found: u32,
        /// The version this crate supports.
        supported: u32,
    },
    /// The manifest has structural problems independent of hash checks (e.g.
    /// an `embedded` artifact missing `path`).
    Invalid(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Io(e) => write!(f, "manifest I/O error: {e}"),
            ManifestError::Toml(e) => write!(f, "manifest parse error: {e}"),
            ManifestError::UnsupportedSchemaVersion { found, supported } => write!(
                f,
                "manifest schema_version {found} is newer than supported {supported}"
            ),
            ManifestError::Invalid(msg) => write!(f, "invalid manifest: {msg}"),
        }
    }
}

impl std::error::Error for ManifestError {}

impl From<io::Error> for ManifestError {
    fn from(value: io::Error) -> Self {
        ManifestError::Io(value)
    }
}

impl From<toml::de::Error> for ManifestError {
    fn from(value: toml::de::Error) -> Self {
        ManifestError::Toml(value)
    }
}

/// Parse a manifest from a string.
pub fn parse(text: &str) -> Result<Manifest, ManifestError> {
    let manifest: Manifest = toml::from_str(text)?;
    if manifest.schema_version > SCHEMA_VERSION {
        return Err(ManifestError::UnsupportedSchemaVersion {
            found: manifest.schema_version,
            supported: SCHEMA_VERSION,
        });
    }
    validate_structure(&manifest)?;
    Ok(manifest)
}

/// Load and parse `manifest.toml` from disk.
pub fn load(path: impl AsRef<Path>) -> Result<Manifest, ManifestError> {
    let text = fs::read_to_string(path.as_ref())?;
    parse(&text)
}

fn validate_structure(manifest: &Manifest) -> Result<(), ManifestError> {
    let mut seen_ids: Vec<&str> = Vec::with_capacity(manifest.artifacts.len());
    for artifact in &manifest.artifacts {
        if artifact.id.is_empty() {
            return Err(ManifestError::Invalid("artifact id is empty".into()));
        }
        if seen_ids.contains(&artifact.id.as_str()) {
            return Err(ManifestError::Invalid(format!(
                "duplicate artifact id: {}",
                artifact.id
            )));
        }
        seen_ids.push(&artifact.id);

        match artifact.kind {
            ArtifactKind::Embedded | ArtifactKind::Generated => {
                if artifact.path.is_none() {
                    return Err(ManifestError::Invalid(format!(
                        "{}: {} artifacts require `path`",
                        artifact.id, artifact.kind
                    )));
                }
                if artifact.sha256.is_none() {
                    return Err(ManifestError::Invalid(format!(
                        "{}: {} artifacts require `sha256`",
                        artifact.id, artifact.kind
                    )));
                }
                if artifact.kind == ArtifactKind::Generated && artifact.preprocessing.is_none() {
                    return Err(ManifestError::Invalid(format!(
                        "{}: generated artifacts require `preprocessing`",
                        artifact.id
                    )));
                }
            }
            ArtifactKind::RuntimeService => {
                if artifact.path.is_some() {
                    return Err(ManifestError::Invalid(format!(
                        "{}: runtime-service artifacts must not declare `path`",
                        artifact.id
                    )));
                }
                if artifact.sha256.is_some() {
                    return Err(ManifestError::Invalid(format!(
                        "{}: runtime-service artifacts must not declare `sha256`",
                        artifact.id
                    )));
                }
                if artifact.endpoint.is_none() {
                    return Err(ManifestError::Invalid(format!(
                        "{}: runtime-service artifacts require `endpoint`",
                        artifact.id
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Result of verifying a single artifact against its on-disk bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyOutcome {
    /// File exists and SHA-256 matches the manifest.
    Ok,
    /// Artifact has no `path` to verify (e.g. runtime-service entry).
    Skipped,
    /// File does not exist at the recorded `path`.
    Missing {
        /// Repository-relative path.
        path: String,
    },
    /// File exists but its SHA-256 does not match.
    HashMismatch {
        /// Repository-relative path.
        path: String,
        /// Hash recorded in the manifest.
        expected: String,
        /// Hash recomputed from disk.
        actual: String,
    },
    /// File exists but its byte length disagrees with the recorded `bytes`.
    /// Hash check still ran and is reported separately (this variant fires
    /// only when the hash matches but `bytes` is stale, which usually means
    /// the manifest was edited by hand).
    SizeMismatch {
        /// Repository-relative path.
        path: String,
        /// Size declared in the manifest.
        expected: u64,
        /// Size measured on disk.
        actual: u64,
    },
}

impl VerifyOutcome {
    /// Whether the outcome should fail the build.
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            VerifyOutcome::Missing { .. }
                | VerifyOutcome::HashMismatch { .. }
                | VerifyOutcome::SizeMismatch { .. }
        )
    }
}

/// Verify a single artifact by re-hashing the bytes at `repo_root / artifact.path`.
pub fn verify_artifact(repo_root: &Path, artifact: &Artifact) -> Result<VerifyOutcome, io::Error> {
    let Some(rel) = &artifact.path else {
        return Ok(VerifyOutcome::Skipped);
    };
    let Some(expected_hash) = &artifact.sha256 else {
        // `validate_structure` already rejected this for embedded / generated
        // artifacts; reaching here means the schema rules allowed it (none
        // currently do). Treat as a skipped check rather than a panic so the
        // structure layer stays the single source of truth for required
        // fields.
        return Ok(VerifyOutcome::Skipped);
    };

    let abs: PathBuf = repo_root.join(rel);
    let mut file = match fs::File::open(&abs) {
        Ok(f) => f,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(VerifyOutcome::Missing { path: rel.clone() })
        }
        Err(err) => return Err(err),
    };

    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut size: u64 = 0;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        size += n as u64;
    }
    let actual = hex_lower(&hasher.finalize());

    if actual != expected_hash.to_lowercase() {
        return Ok(VerifyOutcome::HashMismatch {
            path: rel.clone(),
            expected: expected_hash.clone(),
            actual,
        });
    }

    if let Some(expected_bytes) = artifact.bytes {
        if expected_bytes != size {
            return Ok(VerifyOutcome::SizeMismatch {
                path: rel.clone(),
                expected: expected_bytes,
                actual: size,
            });
        }
    }

    Ok(VerifyOutcome::Ok)
}

/// Verify every artifact in `manifest`. Returns one outcome per artifact in
/// declaration order so the binary can print a stable report.
pub fn verify_all(
    repo_root: &Path,
    manifest: &Manifest,
) -> Result<Vec<(String, VerifyOutcome)>, io::Error> {
    let mut out = Vec::with_capacity(manifest.artifacts.len());
    for artifact in &manifest.artifacts {
        let outcome = verify_artifact(repo_root, artifact)?;
        out.push((artifact.id.clone(), outcome));
    }
    Ok(out)
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        s.push(nibble_char(byte >> 4));
        s.push(nibble_char(byte & 0x0f));
    }
    s
}

fn nibble_char(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + n - 10) as char,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn embedded_sample() -> &'static str {
        r#"
schema_version = 1

[[artifact]]
id = "demo"
description = "Demo artifact."
kind = "embedded"
path = "demo.txt"
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
license = "CC0-1.0"
"#
    }

    #[test]
    fn parse_accepts_minimal_embedded_entry() {
        let manifest = parse(embedded_sample()).expect("parse");
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.artifacts.len(), 1);
        let a = &manifest.artifacts[0];
        assert_eq!(a.id, "demo");
        assert_eq!(a.kind, ArtifactKind::Embedded);
        assert_eq!(a.path.as_deref(), Some("demo.txt"));
    }

    #[test]
    fn parse_rejects_future_schema_version() {
        let text = r#"schema_version = 99"#;
        let err = parse(text).unwrap_err();
        assert!(
            matches!(err, ManifestError::UnsupportedSchemaVersion { .. }),
            "expected UnsupportedSchemaVersion, got {err:?}"
        );
    }

    #[test]
    fn parse_rejects_duplicate_ids() {
        let text = r#"
schema_version = 1

[[artifact]]
id = "dup"
description = "first"
kind = "runtime-service"
endpoint = "https://example.test/"
license = "CC0-1.0"

[[artifact]]
id = "dup"
description = "second"
kind = "runtime-service"
endpoint = "https://example.test/2"
license = "CC0-1.0"
"#;
        let err = parse(text).unwrap_err();
        assert!(matches!(err, ManifestError::Invalid(_)));
    }

    #[test]
    fn parse_rejects_embedded_without_path() {
        let text = r#"
schema_version = 1

[[artifact]]
id = "broken"
description = "missing path"
kind = "embedded"
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
license = "CC0-1.0"
"#;
        let err = parse(text).unwrap_err();
        assert!(matches!(err, ManifestError::Invalid(_)));
    }

    #[test]
    fn parse_rejects_runtime_service_with_path() {
        let text = r#"
schema_version = 1

[[artifact]]
id = "broken"
description = "runtime service with a local path"
kind = "runtime-service"
endpoint = "https://example.test/"
path = "should-not-have-this"
license = "CC0-1.0"
"#;
        let err = parse(text).unwrap_err();
        assert!(matches!(err, ManifestError::Invalid(_)));
    }

    #[test]
    fn parse_rejects_generated_without_preprocessing() {
        let text = r#"
schema_version = 1

[[artifact]]
id = "gen"
description = "missing preprocessing"
kind = "generated"
path = "out.txt"
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
license = "CC0-1.0"
"#;
        let err = parse(text).unwrap_err();
        assert!(matches!(err, ManifestError::Invalid(_)));
    }

    #[test]
    fn verify_artifact_detects_missing_file() {
        let tmp = std::env::temp_dir();
        let artifact = Artifact {
            id: "x".into(),
            description: "x".into(),
            kind: ArtifactKind::Embedded,
            path: Some("definitely-does-not-exist-9e8d7c6b.txt".into()),
            sha256: Some("0000000000000000000000000000000000000000000000000000000000000000".into()),
            bytes: None,
            source: None,
            archive_id: None,
            citation: None,
            version: None,
            retrieved: None,
            license: "CC0-1.0".into(),
            license_url: None,
            preprocessing: None,
            endpoint: None,
            fields_used: vec![],
            used_by: vec![],
            limitations: None,
        };
        let outcome = verify_artifact(&tmp, &artifact).expect("verify");
        assert!(matches!(outcome, VerifyOutcome::Missing { .. }));
        assert!(outcome.is_fatal());
    }

    #[test]
    fn verify_artifact_detects_hash_drift() {
        let dir = std::env::temp_dir().join("stars-manifest-hashtest");
        let _ = fs::create_dir_all(&dir);
        let rel = "drift.txt";
        fs::write(dir.join(rel), b"hello world\n").expect("write fixture");
        let artifact = Artifact {
            id: "drift".into(),
            description: "x".into(),
            kind: ArtifactKind::Embedded,
            path: Some(rel.into()),
            sha256: Some("deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".into()),
            bytes: None,
            source: None,
            archive_id: None,
            citation: None,
            version: None,
            retrieved: None,
            license: "CC0-1.0".into(),
            license_url: None,
            preprocessing: None,
            endpoint: None,
            fields_used: vec![],
            used_by: vec![],
            limitations: None,
        };
        let outcome = verify_artifact(&dir, &artifact).expect("verify");
        match outcome {
            VerifyOutcome::HashMismatch {
                expected, actual, ..
            } => {
                assert!(expected.starts_with("deadbeef"));
                assert_ne!(expected, actual);
            }
            other => panic!("expected HashMismatch, got {other:?}"),
        }
    }

    #[test]
    fn verify_artifact_accepts_matching_hash() {
        let dir = std::env::temp_dir().join("stars-manifest-okhash");
        let _ = fs::create_dir_all(&dir);
        let rel = "ok.txt";
        let bytes = b"hello world\n";
        fs::write(dir.join(rel), bytes).expect("write fixture");
        // sha256("hello world\n") = a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447
        let artifact = Artifact {
            id: "ok".into(),
            description: "x".into(),
            kind: ArtifactKind::Embedded,
            path: Some(rel.into()),
            sha256: Some("a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447".into()),
            bytes: Some(bytes.len() as u64),
            source: None,
            archive_id: None,
            citation: None,
            version: None,
            retrieved: None,
            license: "CC0-1.0".into(),
            license_url: None,
            preprocessing: None,
            endpoint: None,
            fields_used: vec![],
            used_by: vec![],
            limitations: None,
        };
        let outcome = verify_artifact(&dir, &artifact).expect("verify");
        assert_eq!(outcome, VerifyOutcome::Ok);
        assert!(!outcome.is_fatal());
    }
}
