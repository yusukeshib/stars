//! Shared helpers for the native host binaries.
//!
//! Anything that depends on `clap` / `chrono`, or that glues multiple engine
//! crates together only for native hosts, lives here. This keeps the engine
//! crates (`astronomy`, `catalog`, `renderer`) free of CLI / time-parsing
//! dependencies and prevents native host apps from duplicating catalog→renderer
//! adapter logic. Despite living under `crates/`, this is *not* engine-tier —
//! only the host apps under `apps/` consume it.

use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use astronomy::{FalchiAtlas, TimeScales};
use catalog::{load_from_file, CatalogSource};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

mod goto;
mod presets;
mod render;
mod satellites;
mod session;
pub use goto::{resolve_goto_id, resolve_goto_query, GotoTarget};
// L-19 CDS deep-link helpers. The pure URL builders live in `catalog` so the
// WASM web binding can share the single source of truth; we re-export them on
// the documented `stars_host_common` path for the native hosts.
pub use catalog::{simbad_query_url, vizier_query_url, StarIdentifiers};
pub use presets::*;
pub use render::*;
pub use renderer::OpticalDesign;
pub use renderer::DEFAULT_SCREEN_LIMITING_MAGNITUDE;
use renderer::{
    build_star_instance, Atmosphere, AtmospherePreset, ExternalViewpoint, EyepieceSimulation,
    LightPollution, OutputColourSpace, OverlayConfig, OverlayKind, Scintillation, SkyProjection,
    SkyViewpoint, StarInstance,
};
pub use satellites::{
    curated_satellite_layer, curated_satellite_tles, CURATED_TLE_TEXT,
    DEFAULT_SATELLITE_EXPOSURE_SECONDS,
};
pub use session::*;

/// CLI-facing mirror of [`OverlayKind`] that derives [`ValueEnum`] so `clap`
/// can render kebab-case help text and parse user input. Kept in this crate
/// (not in `renderer`) so the engine doesn't take a `clap` dependency.
///
/// The variant set, the kebab-case spelling, and the [`From`] mapping are
/// pinned to [`OverlayKind`] by [`overlay_arg_round_trips`] below — adding a
/// variant in `renderer` will fail this crate's tests until it's mirrored here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
    DeepSkyObjects,
    DeepSkyLabels,
    StarLabels,
    PlanetLabels,
    ConstellationLabels,
    CardinalLabels,
    DegreeLabels,
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
            OverlayArg::DeepSkyObjects => OverlayKind::DeepSkyObjects,
            OverlayArg::DeepSkyLabels => OverlayKind::DeepSkyLabels,
            OverlayArg::StarLabels => OverlayKind::StarLabels,
            OverlayArg::PlanetLabels => OverlayKind::PlanetLabels,
            OverlayArg::ConstellationLabels => OverlayKind::ConstellationLabels,
            OverlayArg::CardinalLabels => OverlayKind::CardinalLabels,
            OverlayArg::DegreeLabels => OverlayKind::DegreeLabels,
        }
    }
}

/// CLI-facing mirror of [`SkyProjection`] for `clap` parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectionArg {
    Perspective,
    Mollweide,
    Aitoff,
    Hammer,
}

impl std::fmt::Display for ProjectionArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(SkyProjection::from(*self).as_kebab_str())
    }
}

impl From<ProjectionArg> for SkyProjection {
    fn from(p: ProjectionArg) -> Self {
        match p {
            ProjectionArg::Perspective => SkyProjection::Perspective,
            ProjectionArg::Mollweide => SkyProjection::Mollweide,
            ProjectionArg::Aitoff => SkyProjection::Aitoff,
            ProjectionArg::Hammer => SkyProjection::Hammer,
        }
    }
}

/// CLI-facing mirror of [`SkyViewpoint`] for `clap` parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewpointArg {
    Earth,
    GalacticNorth,
    CustomExternal,
}

impl std::fmt::Display for ViewpointArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(SkyViewpoint::from(*self).as_kebab_str())
    }
}

impl From<ViewpointArg> for SkyViewpoint {
    fn from(v: ViewpointArg) -> Self {
        match v {
            ViewpointArg::Earth => SkyViewpoint::Earth,
            ViewpointArg::GalacticNorth => SkyViewpoint::GalacticNorth,
            ViewpointArg::CustomExternal => SkyViewpoint::CustomExternal,
        }
    }
}

/// CLI-facing mirror of [`AtmospherePreset`] for `clap` parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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

/// CLI-facing mirror of [`OutputColourSpace`] for `clap` parsing and session
/// serialization (V-50). Kept here (not in `renderer`) so the engine crate
/// stays free of `clap` / `serde`. The variant set and kebab-case spelling are
/// pinned to [`OutputColourSpace`] by [`output_colourspace_arg_round_trips`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputColourspaceArg {
    #[default]
    Srgb,
    DisplayP3,
    Rec2020,
}

impl std::fmt::Display for OutputColourspaceArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(OutputColourSpace::from(*self).as_str())
    }
}

impl From<OutputColourspaceArg> for OutputColourSpace {
    fn from(c: OutputColourspaceArg) -> Self {
        match c {
            OutputColourspaceArg::Srgb => OutputColourSpace::Srgb,
            OutputColourspaceArg::DisplayP3 => OutputColourSpace::DisplayP3,
            OutputColourspaceArg::Rec2020 => OutputColourSpace::Rec2020,
        }
    }
}

impl From<OutputColourSpace> for OutputColourspaceArg {
    fn from(c: OutputColourSpace) -> Self {
        match c {
            OutputColourSpace::Srgb => OutputColourspaceArg::Srgb,
            OutputColourSpace::DisplayP3 => OutputColourspaceArg::DisplayP3,
            OutputColourSpace::Rec2020 => OutputColourspaceArg::Rec2020,
        }
    }
}

/// CLI / session mirror of [`astronomy::AuroraSeason`] for `clap` parsing and
/// serde (V-48). Kept here so the engine crate stays free of `clap` / `serde`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuroraSeasonArg {
    Winter,
    #[default]
    Equinox,
    Summer,
}

impl From<AuroraSeasonArg> for astronomy::AuroraSeason {
    fn from(s: AuroraSeasonArg) -> Self {
        match s {
            AuroraSeasonArg::Winter => astronomy::AuroraSeason::Winter,
            AuroraSeasonArg::Equinox => astronomy::AuroraSeason::Equinox,
            AuroraSeasonArg::Summer => astronomy::AuroraSeason::Summer,
        }
    }
}

impl From<astronomy::AuroraSeason> for AuroraSeasonArg {
    fn from(s: astronomy::AuroraSeason) -> Self {
        match s {
            astronomy::AuroraSeason::Winter => AuroraSeasonArg::Winter,
            astronomy::AuroraSeason::Equinox => AuroraSeasonArg::Equinox,
            astronomy::AuroraSeason::Summer => AuroraSeasonArg::Summer,
        }
    }
}

/// V-48 aurora overrides applied on top of a session/preset scene by the
/// native hosts.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuroraOverrides {
    pub kp: Option<f32>,
    pub season: Option<AuroraSeasonArg>,
}

/// Build a renderer [`AuroraLayer`] from native-host CLI values. `enabled`
/// gates the layer; `kp` is clamped to `[0, 9]`.
pub fn aurora_from_args(enabled: bool, kp: f32, season: AuroraSeasonArg) -> renderer::AuroraLayer {
    renderer::AuroraLayer {
        enabled,
        kp: if kp.is_finite() {
            kp.clamp(0.0, 9.0)
        } else {
            0.0
        },
        season: season.into(),
    }
}

/// Build a renderer overlay configuration from native-host CLI values.
///
/// Keeping this here prevents `stars-cli` and `stars-viewer` from drifting on
/// overlay defaults, string parsing, or opacity clamping while preserving the
/// engine/host boundary: `renderer` owns the render model; this crate owns the
/// `clap`-facing mirror types.
///
/// `deep_sky_magnitude_limit` controls the density filter for the Messier
/// deep-sky markers and labels; the renderer clamps it again on the inside
/// so a stale host value cannot crash the marker builder.
pub fn overlay_config_from_args(
    overlays_disabled: bool,
    overlays: &[OverlayArg],
    grid_step_deg: f64,
    overlay_opacity: f32,
    deep_sky_magnitude_limit: f32,
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
        deep_sky_magnitude_limit,
    }
}

/// Optional custom external-viewpoint controls parsed by native hosts.
///
/// Coordinates use IAU galactic Cartesian parsecs: Sun at `(0, 0, 0)`, `+X`
/// toward `l=0°`, `+Y` toward `l=90°`, and `+Z` toward the north galactic pole.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExternalViewpointOverrides {
    pub origin_pc: Option<[f32; 3]>,
    pub target_pc: Option<[f32; 3]>,
    pub up: Option<[f32; 3]>,
}

impl ExternalViewpointOverrides {
    pub fn has_any(self) -> bool {
        self.origin_pc.is_some() || self.target_pc.is_some() || self.up.is_some()
    }
}

pub fn viewpoint_from_args(
    viewpoint: ViewpointArg,
    overrides: ExternalViewpointOverrides,
) -> (SkyViewpoint, ExternalViewpoint) {
    let mut external = ExternalViewpoint::default();
    if let Some(origin_pc) = overrides.origin_pc {
        external.origin_pc = origin_pc;
    }
    if let Some(target_pc) = overrides.target_pc {
        external.target_pc = target_pc;
    }
    if let Some(up) = overrides.up {
        external.up = up;
    }
    let viewpoint = if overrides.has_any() {
        SkyViewpoint::CustomExternal
    } else {
        viewpoint.into()
    };
    (viewpoint, external)
}

/// Optional telescope / eyepiece controls parsed by native hosts.
#[derive(Debug, Clone, Copy, Default)]
pub struct EyepieceOverrides {
    pub aperture_mm: Option<f32>,
    pub focal_length_mm: Option<f32>,
    pub eyepiece_focal_length_mm: Option<f32>,
    pub apparent_fov_deg: Option<f32>,
    pub field_stop_mm: Option<f32>,
    /// V-45 optical-design family (`apo-refractor`, `achromat-refractor`,
    /// `newtonian`, `schmidt-cassegrain`).
    pub optical_design: Option<OpticalDesign>,
    /// V-45 OTA roll about the optical axis, degrees (rotates spider spikes).
    pub ota_rotation_deg: Option<f32>,
}

impl EyepieceOverrides {
    pub fn has_any(self) -> bool {
        self.aperture_mm.is_some()
            || self.focal_length_mm.is_some()
            || self.eyepiece_focal_length_mm.is_some()
            || self.apparent_fov_deg.is_some()
            || self.field_stop_mm.is_some()
            || self.optical_design.is_some()
            || self.ota_rotation_deg.is_some()
    }
}

/// Build a renderer eyepiece simulation from native-host CLI values.
///
/// Supplying any optic parameter enables the mode, matching the external
/// viewpoint helper's "specific control implies specific mode" behaviour.
pub fn eyepiece_from_args(enabled: bool, overrides: EyepieceOverrides) -> EyepieceSimulation {
    let mut eyepiece = if enabled || overrides.has_any() {
        EyepieceSimulation::DEFAULT_ENABLED
    } else {
        EyepieceSimulation::OFF
    };
    if let Some(aperture_mm) = overrides.aperture_mm {
        eyepiece.aperture_mm = aperture_mm;
    }
    if let Some(focal_length_mm) = overrides.focal_length_mm {
        eyepiece.focal_length_mm = focal_length_mm;
    }
    if let Some(eyepiece_focal_length_mm) = overrides.eyepiece_focal_length_mm {
        eyepiece.eyepiece_focal_length_mm = eyepiece_focal_length_mm;
    }
    if let Some(apparent_fov_deg) = overrides.apparent_fov_deg {
        eyepiece.apparent_fov_deg = apparent_fov_deg;
    }
    if let Some(field_stop_mm) = overrides.field_stop_mm {
        eyepiece.field_stop_mm = field_stop_mm;
    }
    if let Some(optical_design) = overrides.optical_design {
        eyepiece.optical_design = optical_design;
    }
    if let Some(ota_rotation_deg) = overrides.ota_rotation_deg {
        eyepiece.ota_rotation_deg = ota_rotation_deg;
    }
    eyepiece
}

/// Optional atmosphere overrides parsed by native hosts.
///
/// The option fields intentionally mirror the CLI/viewer flags. Keeping the
/// override application in one helper makes the native hosts share identical
/// precedence and disable semantics without giving `atmosphere_from_args` an
/// ever-growing positional argument list.
#[derive(Debug, Clone, Copy, Default)]
pub struct AtmosphereOverrides {
    pub aerosol_beta: Option<f32>,
    pub aerosol_alpha: Option<f32>,
    pub observer_altitude_m: Option<f32>,
    pub ozone_du: Option<f32>,
    pub pressure_hpa: Option<f32>,
    pub temperature_c: Option<f32>,
    /// Override the daylight sky model's ground albedo (V-38).
    pub surface_albedo: Option<f32>,
}

/// Build a renderer atmosphere from native-host CLI values.
pub fn atmosphere_from_args(
    disabled: bool,
    preset: AtmospherePresetArg,
    overrides: AtmosphereOverrides,
) -> Atmosphere {
    if disabled {
        return Atmosphere::OFF;
    }

    let mut atmosphere = Atmosphere::from_preset(AtmospherePreset::from(preset));
    if let Some(beta) = overrides.aerosol_beta {
        atmosphere.aerosol_beta = beta;
    }
    if let Some(alpha) = overrides.aerosol_alpha {
        atmosphere.aerosol_alpha = alpha;
    }
    if let Some(observer_altitude_m) = overrides.observer_altitude_m {
        atmosphere.observer_altitude_m = observer_altitude_m;
    }
    if let Some(ozone_du) = overrides.ozone_du {
        atmosphere.ozone_du = ozone_du;
    }
    if let Some(pressure_hpa) = overrides.pressure_hpa {
        atmosphere.pressure_hpa = pressure_hpa;
    }
    if let Some(temperature_c) = overrides.temperature_c {
        atmosphere.temperature_c = temperature_c;
    }
    if let Some(surface_albedo) = overrides.surface_albedo {
        atmosphere.surface_albedo = surface_albedo;
    }
    atmosphere
}

/// Optional V-39 light-pollution overrides parsed by native hosts.
///
/// The three fields mirror the [`LightPollution`] enum variants — at most
/// one is meant to be set. Precedence (if a host accidentally supplies more
/// than one) is `bortle` first, then `sqm_mag_per_arcsec2`, then
/// `atlas_lat_lng_deg`; everything past the first match is ignored. This
/// keeps two unrelated CLI flags from contradicting each other silently.
#[derive(Debug, Clone, Copy, Default)]
pub struct LightPollutionOverrides {
    /// Bortle 2001 class index (1..=9). Clamped on the astronomy side.
    pub bortle: Option<u8>,
    /// Hand-entered V-band zenith SQM reading in mag/arcsec².
    pub sqm_mag_per_arcsec2: Option<f32>,
    /// `(latitude_deg, longitude_deg)` for the Falchi 2016 atlas lookup.
    /// Currently a `TODO(V-39-Atlas)` sentinel that falls back to the
    /// rural default; the slice lays down the schema so the GeoTIFF
    /// loader can ship without churning sessions.
    pub atlas_lat_lng_deg: Option<(f32, f32)>,
}

impl LightPollutionOverrides {
    pub fn has_any(self) -> bool {
        self.bortle.is_some()
            || self.sqm_mag_per_arcsec2.is_some()
            || self.atlas_lat_lng_deg.is_some()
    }
}

/// Build a renderer [`LightPollution`] from native-host CLI values.
///
/// `disabled` forces the rural [`LightPollution::DARK_SKY`] floor regardless
/// of overrides, matching the `--no-extinction` / `--no-light-pollution`
/// host conventions. Otherwise the first set override wins (`bortle` →
/// `sqm` → `atlas`), and the default is the Bortle 1 dark-sky floor so
/// existing sessions still render exactly the pre-V-39 way.
pub fn light_pollution_from_args(
    disabled: bool,
    overrides: LightPollutionOverrides,
) -> LightPollution {
    if disabled {
        return LightPollution::DARK_SKY;
    }
    if let Some(class) = overrides.bortle {
        return LightPollution::Bortle(class);
    }
    if let Some(sqm) = overrides.sqm_mag_per_arcsec2 {
        return LightPollution::Sqm(sqm);
    }
    if let Some((lat, lng)) = overrides.atlas_lat_lng_deg {
        // The session keeps the `Atlas2016 { lat, lng }` intent for
        // reproducibility; the zenith brightness is sampled at render time by
        // [`resolve_light_pollution`] when a Falchi atlas grid is configured.
        return LightPollution::Atlas2016 {
            latitude_deg: lat,
            longitude_deg: lng,
        };
    }
    LightPollution::DARK_SKY
}

/// Environment variable naming the compact Falchi 2016 atlas grid file
/// (`FALATL01` binary produced by `scripts/build-falchi-atlas.py`). Native
/// hosts read it once to resolve [`LightPollution::Atlas2016`] samples.
pub const FALCHI_ATLAS_ENV: &str = "STARS_FALCHI_ATLAS";

/// Load the compact Falchi 2016 atlas grid named by [`FALCHI_ATLAS_ENV`], if
/// set. Returns `None` (with a log line) when the variable is unset, the file
/// is unreadable, or the bytes do not parse — in which case `Atlas2016` keeps
/// the rural Bortle-1 floor. The result is cached after the first call so the
/// (potentially large) grid is read at most once per process.
pub fn load_falchi_atlas() -> Option<&'static FalchiAtlas> {
    static CELL: OnceLock<Option<FalchiAtlas>> = OnceLock::new();
    CELL.get_or_init(|| {
        let path = std::env::var_os(FALCHI_ATLAS_ENV)?;
        match std::fs::read(&path) {
            Ok(bytes) => match FalchiAtlas::from_bytes(&bytes) {
                Ok(atlas) => {
                    let (rows, cols) = atlas.dimensions();
                    log::info!(
                        "V-39-Atlas: loaded Falchi 2016 grid {}x{} from {}",
                        rows,
                        cols,
                        Path::new(&path).display()
                    );
                    Some(atlas)
                }
                Err(err) => {
                    log::warn!(
                        "V-39-Atlas: {} is not a valid FALATL01 grid ({err}); keeping Bortle-1 floor",
                        Path::new(&path).display()
                    );
                    None
                }
            },
            Err(err) => {
                log::warn!(
                    "V-39-Atlas: cannot read {} ({err}); keeping Bortle-1 floor",
                    Path::new(&path).display()
                );
                None
            }
        }
    })
    .as_ref()
}

/// Resolve a [`LightPollution`] for rendering, sampling the Falchi 2016 atlas
/// for the [`LightPollution::Atlas2016`] variant when a grid is configured via
/// [`FALCHI_ATLAS_ENV`]. Non-atlas variants pass through unchanged.
pub fn resolve_light_pollution(pollution: LightPollution) -> LightPollution {
    resolve_light_pollution_with_atlas(pollution, load_falchi_atlas())
}

/// Pure resolver behind [`resolve_light_pollution`]: when `pollution` is
/// [`LightPollution::Atlas2016`] and `atlas` yields a sample at the observer
/// location, return an equivalent [`LightPollution::Sqm`] zenith brightness;
/// otherwise return `pollution` unchanged so the renderer keeps the rural
/// floor. Exposed (and IO-free) so the resolution rule is unit-testable.
pub fn resolve_light_pollution_with_atlas(
    pollution: LightPollution,
    atlas: Option<&FalchiAtlas>,
) -> LightPollution {
    if let LightPollution::Atlas2016 {
        latitude_deg,
        longitude_deg,
    } = pollution
    {
        match atlas.and_then(|a| {
            a.sample_zenith_mag_per_arcsec2(latitude_deg as f64, longitude_deg as f64)
        }) {
            Some(mu) => return LightPollution::Sqm(mu as f32),
            None => {
                log::info!(
                    "V-39-Atlas: no Falchi sample at ({latitude_deg}, {longitude_deg}); \
                     keeping Bortle-1 floor (set {FALCHI_ATLAS_ENV} to a built grid)"
                );
            }
        }
    }
    pollution
}

/// Optional scintillation overrides parsed by native hosts (V-24).
#[derive(Debug, Clone, Copy, Default)]
pub struct ScintillationOverrides {
    pub c_n2_scale: Option<f32>,
    pub seed: Option<u32>,
}

/// Build a renderer scintillation config from native-host CLI values.
///
/// `disabled` forces [`Scintillation::OFF`] regardless of overrides, matching
/// `--no-extinction` semantics on the atmosphere side. Otherwise the
/// default-on [`Scintillation::DEFAULT`] is patched with any overrides.
pub fn scintillation_from_args(disabled: bool, overrides: ScintillationOverrides) -> Scintillation {
    if disabled {
        return Scintillation::OFF;
    }
    let mut scintillation = Scintillation::DEFAULT;
    if let Some(c_n2_scale) = overrides.c_n2_scale {
        scintillation.c_n2_scale = c_n2_scale;
    }
    if let Some(seed) = overrides.seed {
        scintillation.seed = seed;
    }
    scintillation
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
        .map(|s| {
            build_star_instance(
                s.position.into(),
                s.proper_motion.into(),
                s.color,
                s.magnitude,
                limiting_magnitude,
                s.distance_pc,
            )
        })
        .collect())
}

/// Build the session catalog snapshot for the current HYG CSV backend.
pub fn hyg_catalog_snapshot(path: impl AsRef<Path>, limiting_magnitude: f32) -> CatalogSnapshot {
    let source = CatalogSource::HYG_CSV;
    CatalogSnapshot {
        backend: source.backend.as_kebab_str().to_string(),
        source: source.name.to_string(),
        version: source.version.map(str::to_string),
        path: Some(path.as_ref().display().to_string()),
        hash: None,
        limiting_magnitude,
    }
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

    /// Build a tiny synthetic `FALATL01` grid for resolver tests. The values
    /// are a fixture, not Falchi data; they only exercise the resolution rule.
    fn test_atlas() -> FalchiAtlas {
        let mut b = Vec::new();
        b.extend_from_slice(b"FALATL01");
        b.extend_from_slice(&2u32.to_le_bytes());
        b.extend_from_slice(&2u32.to_le_bytes());
        for v in [10.0f64, 0.0, 0.0, 10.0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        for v in [18.0f32, 18.0, 18.0, 18.0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        FalchiAtlas::from_bytes(&b).unwrap()
    }

    #[test]
    fn atlas_resolves_to_sampled_sqm() {
        let atlas = test_atlas();
        let resolved = resolve_light_pollution_with_atlas(
            LightPollution::Atlas2016 {
                latitude_deg: 5.0,
                longitude_deg: 5.0,
            },
            Some(&atlas),
        );
        match resolved {
            LightPollution::Sqm(mu) => assert!((mu - 18.0).abs() < 1e-4, "mu={mu}"),
            other => panic!("expected Sqm sample, got {other:?}"),
        }
    }

    #[test]
    fn atlas_without_grid_keeps_variant() {
        // No atlas configured → Atlas2016 passes through unchanged so the
        // renderer applies the rural Bortle-1 floor.
        let lp = LightPollution::Atlas2016 {
            latitude_deg: 5.0,
            longitude_deg: 5.0,
        };
        assert_eq!(resolve_light_pollution_with_atlas(lp, None), lp);
    }

    #[test]
    fn atlas_out_of_coverage_keeps_variant() {
        // A location outside the grid bounds is not sampled; keep the variant.
        let atlas = test_atlas();
        let lp = LightPollution::Atlas2016 {
            latitude_deg: 80.0,
            longitude_deg: 5.0,
        };
        assert_eq!(resolve_light_pollution_with_atlas(lp, Some(&atlas)), lp);
    }

    #[test]
    fn non_atlas_pollution_passes_through() {
        let atlas = test_atlas();
        for lp in [
            LightPollution::Bortle(5),
            LightPollution::Sqm(20.0),
            LightPollution::DARK_SKY,
        ] {
            assert_eq!(resolve_light_pollution_with_atlas(lp, Some(&atlas)), lp);
        }
    }

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
            OverlayArg::DeepSkyObjects,
            OverlayArg::DeepSkyLabels,
            OverlayArg::StarLabels,
            OverlayArg::PlanetLabels,
            OverlayArg::ConstellationLabels,
            OverlayArg::CardinalLabels,
            OverlayArg::DegreeLabels,
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
    fn projection_arg_round_trips() {
        for arg in [
            ProjectionArg::Perspective,
            ProjectionArg::Mollweide,
            ProjectionArg::Aitoff,
            ProjectionArg::Hammer,
        ] {
            let projection: SkyProjection = arg.into();
            let s = projection.as_kebab_str();
            assert_eq!(SkyProjection::from_kebab_str(s), Some(projection));
            assert_eq!(format!("{arg}"), s);
        }
    }

    #[test]
    fn output_colourspace_arg_round_trips() {
        for arg in [
            OutputColourspaceArg::Srgb,
            OutputColourspaceArg::DisplayP3,
            OutputColourspaceArg::Rec2020,
        ] {
            let cs: OutputColourSpace = arg.into();
            // CLI/session string spelling matches the engine identifier.
            assert_eq!(format!("{arg}"), cs.as_str());
            // Round-trips back to the same arg variant.
            assert_eq!(OutputColourspaceArg::from(cs), arg);
            // serde tag matches the kebab spelling.
            let json = serde_json::to_string(&arg).unwrap();
            assert_eq!(json, format!("\"{}\"", cs.as_str()));
        }
    }

    #[test]
    fn viewpoint_arg_round_trips() {
        for arg in [
            ViewpointArg::Earth,
            ViewpointArg::GalacticNorth,
            ViewpointArg::CustomExternal,
        ] {
            let viewpoint: SkyViewpoint = arg.into();
            let s = viewpoint.as_kebab_str();
            assert_eq!(SkyViewpoint::from_kebab_str(s), Some(viewpoint));
            assert_eq!(format!("{arg}"), s);
        }
    }

    #[test]
    fn custom_external_viewpoint_overrides_select_custom_mode() {
        let (viewpoint, external) = viewpoint_from_args(
            ViewpointArg::Earth,
            ExternalViewpointOverrides {
                origin_pc: Some([1.0, 2.0, 3.0]),
                target_pc: None,
                up: Some([0.0, 0.0, 1.0]),
            },
        );
        assert_eq!(viewpoint, SkyViewpoint::CustomExternal);
        assert_eq!(external.origin_pc, [1.0, 2.0, 3.0]);
        assert_eq!(
            external.target_pc,
            ExternalViewpoint::GALACTIC_NORTH.target_pc
        );
        assert_eq!(external.up, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn eyepiece_overrides_select_enabled_mode() {
        let off = eyepiece_from_args(false, EyepieceOverrides::default());
        assert!(!off.enabled);

        let explicit = eyepiece_from_args(true, EyepieceOverrides::default());
        assert!(explicit.enabled);
        assert_eq!(
            explicit.focal_length_mm,
            EyepieceSimulation::OFF.focal_length_mm
        );

        let overridden = eyepiece_from_args(
            false,
            EyepieceOverrides {
                focal_length_mm: Some(1200.0),
                eyepiece_focal_length_mm: Some(12.0),
                field_stop_mm: Some(14.0),
                ..EyepieceOverrides::default()
            },
        );
        assert!(overridden.enabled);
        assert_eq!(overridden.focal_length_mm, 1200.0);
        assert_eq!(overridden.eyepiece_focal_length_mm, 12.0);
        assert_eq!(overridden.field_stop_mm, 14.0);
    }

    #[test]
    fn overlay_config_helper_applies_disable_and_opacity_rules() {
        let overlays = [OverlayArg::Horizon, OverlayArg::ConstellationLines];
        let enabled = overlay_config_from_args(false, &overlays, 30.0, 2.0, 7.0);
        assert_eq!(
            enabled.layers,
            vec![OverlayKind::Horizon, OverlayKind::ConstellationLines]
        );
        assert_eq!(enabled.grid_step_deg, 30.0);
        assert_eq!(enabled.opacity, 1.0);

        let disabled = overlay_config_from_args(true, &overlays, 15.0, 0.5, 7.0);
        assert!(disabled.layers.is_empty());
        assert_eq!(disabled.opacity, 0.5);
    }

    #[test]
    fn atmosphere_helper_applies_overrides_and_disable_rule() {
        let atmosphere = atmosphere_from_args(
            false,
            AtmospherePresetArg::HazyUrban,
            AtmosphereOverrides {
                aerosol_beta: Some(0.25),
                aerosol_alpha: Some(0.9),
                observer_altitude_m: Some(1234.0),
                ozone_du: Some(280.0),
                pressure_hpa: Some(900.0),
                temperature_c: Some(5.0),
                surface_albedo: Some(0.4),
            },
        );
        // Overrides take precedence over the preset's (β, α, DU) values.
        assert_eq!(atmosphere.aerosol_beta, 0.25);
        assert_eq!(atmosphere.aerosol_alpha, 0.9);
        assert_eq!(atmosphere.observer_altitude_m, 1234.0);
        assert_eq!(atmosphere.ozone_du, 280.0);
        assert_eq!(atmosphere.pressure_hpa, 900.0);
        assert_eq!(atmosphere.temperature_c, 5.0);
        assert_eq!(atmosphere.surface_albedo, 0.4);

        let off = atmosphere_from_args(
            true,
            AtmospherePresetArg::ClearRural,
            AtmosphereOverrides {
                aerosol_beta: Some(0.25),
                aerosol_alpha: Some(0.9),
                observer_altitude_m: Some(1234.0),
                ozone_du: Some(280.0),
                pressure_hpa: Some(900.0),
                temperature_c: Some(5.0),
                surface_albedo: None,
            },
        );
        assert_eq!(off.extinction_k_rgb(), [0.0; 3]);
        assert!(!off.sunlit_scattering);
    }

    #[test]
    fn hyg_catalog_snapshot_uses_catalog_source_metadata() {
        let snapshot = hyg_catalog_snapshot("crates/catalog/data/hyg_v42.csv", 7.5);
        assert_eq!(snapshot.backend, "hyg-csv");
        assert_eq!(snapshot.source, "HYG");
        assert_eq!(snapshot.version.as_deref(), Some("4.2"));
        assert_eq!(
            snapshot.path.as_deref(),
            Some("crates/catalog/data/hyg_v42.csv")
        );
        assert_eq!(snapshot.limiting_magnitude, 7.5);
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
