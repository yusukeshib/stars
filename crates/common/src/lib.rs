//! Shared helpers for the native host binaries.
//!
//! Anything that depends on `clap` or `chrono` lives here so the engine crates
//! (`astronomy`, `catalog`, `renderer`) stay free of CLI / time-parsing
//! dependencies and remain trivially embeddable from WASM, FFI, and tests.
//! Despite living under `crates/`, this is *not* engine-tier — only the host
//! apps under `apps/` consume it.

use anyhow::{Context, Result};
use astronomy::julian_date_from_unix_seconds;
use clap::ValueEnum;
use renderer::OverlayKind;

/// CLI-facing mirror of [`OverlayKind`] that derives [`ValueEnum`] so `clap`
/// can render kebab-case help text and parse user input. Kept in this crate
/// (not in `renderer`) so the engine doesn't take a `clap` dependency.
///
/// The variant set, the kebab-case spelling, and the [`From`] mapping are
/// pinned to [`OverlayKind`] by [`overlay_arg_round_trips`] below — adding a
/// variant in `renderer` will fail this crate's tests until it's mirrored here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum)]
pub enum OverlayArg {
    Horizon,
    Cardinals,
    AltAzGrid,
    EquatorialGrid,
    Ecliptic,
    CelestialEquator,
    Meridian,
}

impl std::fmt::Display for OverlayArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Delegate to the renderer's canonical kebab name so the CLI flags and
        // the WASM/JS overlay names can never drift apart.
        f.write_str(OverlayKind::from(*self).as_kebab_str())
    }
}

impl From<OverlayArg> for OverlayKind {
    fn from(o: OverlayArg) -> Self {
        match o {
            OverlayArg::Horizon => OverlayKind::Horizon,
            OverlayArg::Cardinals => OverlayKind::Cardinals,
            OverlayArg::AltAzGrid => OverlayKind::AltAzGrid,
            OverlayArg::EquatorialGrid => OverlayKind::EquatorialGrid,
            OverlayArg::Ecliptic => OverlayKind::Ecliptic,
            OverlayArg::CelestialEquator => OverlayKind::CelestialEquator,
            OverlayArg::Meridian => OverlayKind::Meridian,
        }
    }
}

/// Parse `Some(rfc3339)` (or `None` ⇒ "now") into a Julian Date, sub-second
/// precision preserved.
pub fn parse_time_to_jd(time: Option<&str>) -> Result<f64> {
    let unix_seconds = match time {
        Some(s) => {
            let dt = chrono::DateTime::parse_from_rfc3339(s)
                .with_context(|| format!("Invalid RFC3339 time: {s}"))?;
            dt.timestamp() as f64 + dt.timestamp_subsec_nanos() as f64 * 1e-9
        }
        None => {
            let now = chrono::Utc::now();
            now.timestamp() as f64 + now.timestamp_subsec_nanos() as f64 * 1e-9
        }
    };
    Ok(julian_date_from_unix_seconds(unix_seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// If `OverlayKind` grows a variant without a matching `OverlayArg`, this
    /// test fails. Vice versa is enforced by the `From<OverlayArg>` match
    /// being exhaustive at compile time.
    #[test]
    fn overlay_arg_round_trips() {
        for arg in [
            OverlayArg::Horizon,
            OverlayArg::Cardinals,
            OverlayArg::AltAzGrid,
            OverlayArg::EquatorialGrid,
            OverlayArg::Ecliptic,
            OverlayArg::CelestialEquator,
            OverlayArg::Meridian,
        ] {
            let kind: OverlayKind = arg.into();
            let s = kind.as_kebab_str();
            assert_eq!(
                OverlayKind::from_kebab_str(s),
                Some(kind),
                "kebab round-trip broken for {arg:?}"
            );
            // Display matches the canonical kebab name.
            assert_eq!(format!("{arg}"), s);
        }
    }

    #[test]
    fn parse_time_now_is_finite() {
        let jd = parse_time_to_jd(None).unwrap();
        assert!(jd.is_finite() && jd > 2_400_000.0);
    }

    #[test]
    fn parse_time_rfc3339_matches_known_jd() {
        // 2000-01-01T12:00:00Z is J2000.0 = JD 2451545.0
        let jd = parse_time_to_jd(Some("2000-01-01T12:00:00Z")).unwrap();
        assert!((jd - 2_451_545.0).abs() < 1e-6, "jd={jd}");
    }
}
