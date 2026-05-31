//! V-55 host-tier satellite helpers: the embedded curated TLE snapshot and the
//! [`renderer::SatelliteLayer`] builders the CLI / viewer / web hosts share.
//!
//! The default render path ships a deterministic, manifest-pinned TLE snapshot
//! (`crates/common/data/satellites/curated_tle.txt`); live TLE fetching is an
//! opt-in host concern and never the default so gallery renders stay
//! reproducible (see `docs/standards-compliance.md`).

use astronomy::{parse_tle_set, Tle};
use renderer::SatelliteLayer;

/// Curated, manifest-pinned TLE snapshot embedded at build time. Provenance is
/// recorded in `DATA_SOURCES.md` and `data/manifest.toml`
/// (id `celestrak-tle-curated-2026-05`).
pub const CURATED_TLE_TEXT: &str = include_str!("../data/satellites/curated_tle.txt");

/// Default frame-integration exposure (seconds) for satellite streaks. Zero
/// renders point sprites.
pub const DEFAULT_SATELLITE_EXPOSURE_SECONDS: f32 = 0.0;

/// Parse the embedded curated TLE snapshot into [`Tle`] records.
pub fn curated_satellite_tles() -> Vec<Tle> {
    parse_tle_set(CURATED_TLE_TEXT)
}

/// Build a [`SatelliteLayer`] from the curated snapshot.
pub fn curated_satellite_layer(enabled: bool, exposure_seconds: f32) -> SatelliteLayer {
    SatelliteLayer {
        enabled,
        exposure_seconds: exposure_seconds.max(0.0),
        tles: if enabled {
            curated_satellite_tles()
        } else {
            Vec::new()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curated_snapshot_parses_to_several_satellites() {
        let tles = curated_satellite_tles();
        assert!(
            tles.len() >= 5,
            "expected the curated snapshot to carry >= 5 satellites, got {}",
            tles.len()
        );
        assert!(tles.iter().any(|t| t.name.contains("ISS")));
    }

    #[test]
    fn disabled_layer_carries_no_tles() {
        let layer = curated_satellite_layer(false, 0.0);
        assert!(!layer.enabled);
        assert!(layer.tles.is_empty());
    }

    #[test]
    fn enabled_layer_loads_curated_tles() {
        let layer = curated_satellite_layer(true, 2.0);
        assert!(layer.enabled);
        assert_eq!(layer.exposure_seconds, 2.0);
        assert!(!layer.tles.is_empty());
    }
}
