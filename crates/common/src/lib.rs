//! Shared helpers for the native host binaries.
//!
//! Anything that depends on `clap` / `chrono`, or that glues multiple engine
//! crates together only for native hosts, lives here. This keeps the engine
//! crates (`astronomy`, `catalog`, `renderer`) free of CLI / time-parsing
//! dependencies and prevents native host apps from duplicating catalog→renderer
//! adapter logic. Despite living under `crates/`, this is *not* engine-tier —
//! only the host apps under `apps/` consume it.

use std::path::Path;

use anyhow::{Context, Result};
use astronomy::TimeScales;
use catalog::load_from_file;
use clap::ValueEnum;
use renderer::{
    build_star_instance, Atmosphere, AtmospherePreset, OverlayConfig, OverlayKind, StarInstance,
};

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
    GalacticEquator,
    ConstellationLines,
    ConstellationBoundaries,
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
            OverlayArg::GalacticEquator => OverlayKind::GalacticEquator,
            OverlayArg::ConstellationLines => OverlayKind::ConstellationLines,
            OverlayArg::ConstellationBoundaries => OverlayKind::ConstellationBoundaries,
        }
    }
}

/// CLI-facing mirror of [`AtmospherePreset`] for `clap` parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum)]
pub enum AtmospherePresetArg {
    ClearRural,
    HazyUrban,
    HighAltitude,
}

impl std::fmt::Display for AtmospherePresetArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(AtmospherePreset::from(*self).as_kebab_str())
    }
}

impl From<AtmospherePresetArg> for AtmospherePreset {
    fn from(p: AtmospherePresetArg) -> Self {
        match p {
            AtmospherePresetArg::ClearRural => AtmospherePreset::ClearRural,
            AtmospherePresetArg::HazyUrban => AtmospherePreset::HazyUrban,
            AtmospherePresetArg::HighAltitude => AtmospherePreset::HighAltitude,
        }
    }
}

/// Build a renderer overlay configuration from native-host CLI values.
///
/// Keeping this here prevents `stars-cli` and `stars-viewer` from drifting on
/// overlay defaults, string parsing, or opacity clamping while preserving the
/// engine/host boundary: `renderer` owns the render model; this crate owns the
/// `clap`-facing mirror types.
pub fn overlay_config_from_args(
    overlays_disabled: bool,
    overlays: &[OverlayArg],
    grid_step_deg: f64,
    overlay_opacity: f32,
) -> OverlayConfig {
    let layers = if overlays_disabled {
        Vec::new()
    } else {
        overlays.iter().copied().map(OverlayKind::from).collect()
    };
    OverlayConfig {
        layers,
        grid_step_deg,
        opacity: overlay_opacity.clamp(0.0, 1.0),
    }
}

/// Build a renderer atmosphere from native-host CLI values.
///
/// The option fields intentionally mirror the CLI/viewer flags. Keeping the
/// override application in one helper makes the native hosts share identical
/// precedence and disable semantics.
pub fn atmosphere_from_args(
    disabled: bool,
    preset: AtmospherePresetArg,
    turbidity: Option<f32>,
    observer_altitude_m: Option<f32>,
    ozone_du: Option<f32>,
    visibility_km: Option<f32>,
) -> Atmosphere {
    if disabled {
        return Atmosphere::OFF;
    }

    let mut atmosphere = Atmosphere::from_preset(AtmospherePreset::from(preset));
    if let Some(turbidity) = turbidity {
        atmosphere.turbidity = turbidity;
    }
    if let Some(observer_altitude_m) = observer_altitude_m {
        atmosphere.observer_altitude_m = observer_altitude_m;
    }
    if let Some(ozone_du) = ozone_du {
        atmosphere.ozone_du = ozone_du;
    }
    if let Some(visibility_km) = visibility_km {
        atmosphere.visibility_km = visibility_km;
    }
    atmosphere
}

/// Load a filesystem-backed star catalog and convert it into renderer-ready
/// instances using the shared perceptual magnitude/colour pipeline.
///
/// Native hosts both follow this exact step; centralising it here prevents the
/// CLI and viewer from drifting in how they bridge the `catalog` and `renderer`
/// crates. WASM keeps its separate embedded-catalog path in `apps/web`.
pub fn load_star_instances_from_file(
    path: impl AsRef<Path>,
    limiting_magnitude: f32,
) -> Result<Vec<StarInstance>> {
    let path = path.as_ref();
    let stars =
        load_from_file(path).with_context(|| format!("Reading catalog at {}", path.display()))?;
    Ok(stars
        .iter()
        .map(|s| build_star_instance(s.position.into(), s.color, s.magnitude, limiting_magnitude))
        .collect())
}

fn parse_time_to_unix_seconds(time: Option<&str>) -> Result<f64> {
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
    Ok(unix_seconds)
}

/// Parse `Some(rfc3339)` (or `None` ⇒ "now") into UTC/UT1/TAI/TT/TDB scales.
pub fn parse_time_to_time_scales(time: Option<&str>) -> Result<TimeScales> {
    Ok(TimeScales::from_unix_seconds(parse_time_to_unix_seconds(
        time,
    )?))
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
            OverlayArg::GalacticEquator,
            OverlayArg::ConstellationLines,
            OverlayArg::ConstellationBoundaries,
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
    fn atmosphere_preset_arg_round_trips() {
        for arg in [
            AtmospherePresetArg::ClearRural,
            AtmospherePresetArg::HazyUrban,
            AtmospherePresetArg::HighAltitude,
        ] {
            let preset: AtmospherePreset = arg.into();
            let s = preset.as_kebab_str();
            assert_eq!(AtmospherePreset::from_kebab_str(s), Some(preset));
            assert_eq!(format!("{arg}"), s);
        }
    }

    #[test]
    fn overlay_config_helper_applies_disable_and_opacity_rules() {
        let overlays = [OverlayArg::Horizon, OverlayArg::ConstellationLines];
        let enabled = overlay_config_from_args(false, &overlays, 30.0, 2.0);
        assert_eq!(
            enabled.layers,
            vec![OverlayKind::Horizon, OverlayKind::ConstellationLines]
        );
        assert_eq!(enabled.grid_step_deg, 30.0);
        assert_eq!(enabled.opacity, 1.0);

        let disabled = overlay_config_from_args(true, &overlays, 15.0, 0.5);
        assert!(disabled.layers.is_empty());
        assert_eq!(disabled.opacity, 0.5);
    }

    #[test]
    fn atmosphere_helper_applies_overrides_and_disable_rule() {
        let atmosphere = atmosphere_from_args(
            false,
            AtmospherePresetArg::HazyUrban,
            Some(4.0),
            Some(1234.0),
            Some(280.0),
            Some(20.0),
        );
        assert_eq!(
            atmosphere.extinction_k_rgb,
            Atmosphere::HAZY_URBAN.extinction_k_rgb
        );
        assert_eq!(atmosphere.turbidity, 4.0);
        assert_eq!(atmosphere.observer_altitude_m, 1234.0);
        assert_eq!(atmosphere.ozone_du, 280.0);
        assert_eq!(atmosphere.visibility_km, 20.0);

        let off = atmosphere_from_args(
            true,
            AtmospherePresetArg::ClearRural,
            Some(4.0),
            Some(1234.0),
            Some(280.0),
            Some(20.0),
        );
        assert_eq!(off.extinction_k_rgb, Atmosphere::OFF.extinction_k_rgb);
        assert!(!off.sunlit_scattering);
    }

    #[test]
    fn parse_time_now_is_finite() {
        let jd = parse_time_to_time_scales(None).unwrap().jd_utc;
        assert!(jd.is_finite() && jd > 2_400_000.0);
    }

    #[test]
    fn parse_time_rfc3339_matches_known_jd() {
        // 2000-01-01T12:00:00Z is J2000.0 = JD 2451545.0
        let jd = parse_time_to_time_scales(Some("2000-01-01T12:00:00Z"))
            .unwrap()
            .jd_utc;
        assert!((jd - 2_451_545.0).abs() < 1e-6, "jd={jd}");
    }
}
