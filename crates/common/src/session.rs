//! Schema-versioned, shareable scene sessions for native hosts.
//!
//! The JSON schema is intentionally host-facing rather than renderer-facing:
//! fields use stable kebab/camel-case names, store degrees instead of radians,
//! and include provenance/correction metadata that is useful to reviewers. The
//! conversion helpers below are the only place native hosts translate the JSON
//! into engine structs.

use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use astronomy::TimeScales;
use renderer::{
    Atmosphere, AtmospherePreset, ExternalViewpoint, EyepieceSimulation, LightPollution, LocalView,
    OverlayConfig, OverlayKind, SatelliteLayer, Scintillation, SkyProjection, SkyViewpoint,
};

use crate::curated_satellite_layer;
use serde::{Deserialize, Serialize};

use crate::{AtmospherePresetArg, OverlayArg, ProjectionArg, ViewpointArg};

/// Current JSON session schema. Increment when a breaking semantic change is
/// made to any serialized field.
///
/// v5: V-39 adds the `lightPollution` block (Bortle / SQM / atlas placeholder)
/// to the session. The field is required in v5+; older sessions on the
/// previous schema do not migrate forward automatically — the host must
/// re-run `--write-session` to bump.
/// v6: V-55 adds the `satellites` block (artificial-satellite TLE / SGP4
/// layer). The field is required in v6+; older sessions must re-run
/// `--write-session` to bump.
pub const SESSION_SCHEMA_VERSION: u32 = 6;

/// Complete scene/session file. Unknown future fields are ignored by serde, but
/// the top-level schema version must match before a host uses the data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StarSession {
    pub schema_version: u32,
    pub app_version: String,
    pub created_by: String,
    pub observer: SessionObserver,
    pub time: SessionTime,
    pub view: SessionView,
    pub overlays: SessionOverlays,
    pub projection: SessionProjection,
    pub atmosphere: SessionAtmosphere,
    pub light_pollution: SessionLightPollution,
    pub scintillation: SessionScintillation,
    pub planets: SessionPlanets,
    #[serde(default)]
    pub satellites: SessionSatellites,
    pub eyepiece: SessionEyepiece,
    pub catalog: CatalogSnapshot,
    pub corrections: CorrectionSnapshot,
}

/// Native rendering-ready scene derived from a [`StarSession`].
#[derive(Debug, Clone)]
pub struct SessionScene {
    pub latitude_deg: f64,
    pub longitude_deg: f64,
    pub time: TimeScales,
    pub view: LocalView,
    pub overlays: OverlayConfig,
    pub atmosphere_preset: AtmospherePreset,
    pub atmosphere: Atmosphere,
    pub light_pollution: LightPollution,
    pub scintillation: Scintillation,
    pub planets_enabled: bool,
    /// V-55 artificial-satellite layer (engine-ready, with curated TLEs loaded
    /// when enabled).
    pub satellites: SatelliteLayer,
    pub projection: SkyProjection,
    pub viewpoint: SkyViewpoint,
    pub external_viewpoint: ExternalViewpoint,
    pub eyepiece: EyepieceSimulation,
    pub catalog: CatalogSnapshot,
    pub corrections: CorrectionSnapshot,
}

/// Serialized V-55 satellite-layer settings. The TLE set itself is *not*
/// stored in the session — when `enabled`, the host loads the curated,
/// manifest-pinned snapshot so sessions stay small and deterministic.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSatellites {
    pub enabled: bool,
    /// Frame-integration exposure (seconds). 0 renders point sprites; a
    /// positive value renders motion streaks.
    pub exposure_seconds: f32,
}

impl Default for SessionSatellites {
    fn default() -> Self {
        Self {
            enabled: false,
            exposure_seconds: 0.0,
        }
    }
}

impl From<&SatelliteLayer> for SessionSatellites {
    fn from(layer: &SatelliteLayer) -> Self {
        Self {
            enabled: layer.enabled,
            exposure_seconds: layer.exposure_seconds,
        }
    }
}

impl SessionSatellites {
    /// Build an engine-ready [`SatelliteLayer`], loading the curated TLE
    /// snapshot when enabled.
    pub fn to_satellite_layer(self) -> SatelliteLayer {
        curated_satellite_layer(self.enabled, self.exposure_seconds.max(0.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionObserver {
    pub latitude_deg: f64,
    pub longitude_deg: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTime {
    pub jd_utc: f64,
    pub jd_ut1: f64,
    pub jd_tai: f64,
    pub jd_tt: f64,
    pub jd_tdb: f64,
    pub tai_minus_utc_seconds: f64,
    pub dut1_seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub azimuth_deg: f64,
    pub altitude_deg: f64,
    pub fov_deg: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionOverlays {
    pub layers: Vec<OverlayArg>,
    pub grid_step_deg: f64,
    pub opacity: f32,
    /// V magnitude cutoff for `deep-sky-objects` / `deep-sky-labels`. Optional
    /// so sessions written before the field existed still round-trip: when
    /// absent, the renderer-side default is used at apply time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deep_sky_magnitude_limit: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionProjection {
    pub projection: ProjectionArg,
    pub viewpoint: ViewpointArg,
    pub external: SessionExternalViewpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionExternalViewpoint {
    pub origin_pc: SessionVec3,
    pub target_pc: SessionVec3,
    pub up: SessionVec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionAtmosphere {
    pub enabled: bool,
    pub preset: AtmospherePresetArg,
    /// Ångström aerosol optical depth at 550 nm. Drives both stellar k(λ)
    /// and the daylight Mie aerosol term (V-37).
    pub aerosol_beta: f32,
    /// Ångström wavelength exponent (continental aerosols ≈ 1.3).
    pub aerosol_alpha: f32,
    pub observer_altitude_m: f32,
    pub ozone_du: f32,
    pub pressure_hpa: f32,
    pub temperature_c: f32,
    /// Ground albedo seen by the V-38 Hošek-Wilkie daylight model.
    pub surface_albedo: f32,
}

/// V-39 light-pollution config: scales the dark-sky background by Bortle
/// class, hand-entered SQM mag/arcsec², or a (deferred) Falchi 2016 atlas
/// sample. The `kind` field selects the variant; the other fields are
/// populated for that variant only. Unused fields serialise as `null` so
/// the on-disk JSON stays human-readable.
///
/// The default is `kind = "bortle"` with `bortle = 1`, which reproduces
/// the pre-V-39 dark-sky composition bit-for-bit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLightPollution {
    /// Variant tag: `"bortle"`, `"sqm"`, or `"atlas-2016"`.
    pub kind: SessionLightPollutionKind,
    /// Bortle 2001 class index when `kind == Bortle`. `None` otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bortle: Option<u8>,
    /// V-band zenith SQM reading in mag/arcsec² when `kind == Sqm`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sqm_mag_per_arcsec2: Option<f32>,
    /// Observer latitude in decimal degrees, north positive, when
    /// `kind == Atlas2016`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas_latitude_deg: Option<f32>,
    /// Observer longitude in decimal degrees, east positive, when
    /// `kind == Atlas2016`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas_longitude_deg: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionLightPollutionKind {
    Bortle,
    Sqm,
    Atlas2016,
}

impl Default for SessionLightPollution {
    fn default() -> Self {
        Self::from(LightPollution::default())
    }
}

impl From<LightPollution> for SessionLightPollution {
    fn from(value: LightPollution) -> Self {
        match value {
            LightPollution::Bortle(class) => Self {
                kind: SessionLightPollutionKind::Bortle,
                bortle: Some(class),
                sqm_mag_per_arcsec2: None,
                atlas_latitude_deg: None,
                atlas_longitude_deg: None,
            },
            LightPollution::Sqm(mu) => Self {
                kind: SessionLightPollutionKind::Sqm,
                bortle: None,
                sqm_mag_per_arcsec2: Some(mu),
                atlas_latitude_deg: None,
                atlas_longitude_deg: None,
            },
            LightPollution::Atlas2016 {
                latitude_deg,
                longitude_deg,
            } => Self {
                kind: SessionLightPollutionKind::Atlas2016,
                bortle: None,
                sqm_mag_per_arcsec2: None,
                atlas_latitude_deg: Some(latitude_deg),
                atlas_longitude_deg: Some(longitude_deg),
            },
        }
    }
}

impl SessionLightPollution {
    pub fn to_light_pollution(self) -> Result<LightPollution> {
        match self.kind {
            SessionLightPollutionKind::Bortle => {
                let class = self
                    .bortle
                    .context("lightPollution.kind=bortle requires bortle field")?;
                if !(1..=9).contains(&class) {
                    bail!("lightPollution.bortle={class} is outside 1..=9");
                }
                Ok(LightPollution::Bortle(class))
            }
            SessionLightPollutionKind::Sqm => {
                let mu = self
                    .sqm_mag_per_arcsec2
                    .context("lightPollution.kind=sqm requires sqmMagPerArcsec2 field")?;
                if !(16.0..=22.5).contains(&mu) {
                    bail!(
                        "lightPollution.sqmMagPerArcsec2={mu} is outside the supported 16.0..=22.5 mag/arcsec² range"
                    );
                }
                Ok(LightPollution::Sqm(mu))
            }
            SessionLightPollutionKind::Atlas2016 => {
                let latitude_deg = self
                    .atlas_latitude_deg
                    .context("lightPollution.kind=atlas-2016 requires atlasLatitudeDeg field")?;
                let longitude_deg = self
                    .atlas_longitude_deg
                    .context("lightPollution.kind=atlas-2016 requires atlasLongitudeDeg field")?;
                if !(-90.0..=90.0).contains(&latitude_deg) {
                    bail!("lightPollution.atlasLatitudeDeg={latitude_deg} is outside -90..=90");
                }
                if !(-180.0..=180.0).contains(&longitude_deg) {
                    bail!("lightPollution.atlasLongitudeDeg={longitude_deg} is outside -180..=180");
                }
                Ok(LightPollution::Atlas2016 {
                    latitude_deg,
                    longitude_deg,
                })
            }
        }
    }
}

/// Per-frame atmospheric scintillation state (V-24).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionScintillation {
    pub enabled: bool,
    /// Dimensionless Cn² column scale; see `astronomy::scintillation`.
    pub c_n2_scale: f32,
    /// Deterministic noise seed. Two sessions with the same seed and the
    /// same simulated UT1 produce bit-identical pixels.
    pub seed: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlanets {
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEyepiece {
    pub enabled: bool,
    pub aperture_mm: f32,
    pub focal_length_mm: f32,
    pub eyepiece_focal_length_mm: f32,
    pub apparent_fov_deg: f32,
    pub field_stop_mm: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSnapshot {
    pub backend: String,
    pub source: String,
    pub version: Option<String>,
    pub path: Option<String>,
    pub hash: Option<String>,
    pub limiting_magnitude: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionSnapshot {
    pub time_scales: bool,
    pub precession: bool,
    pub nutation: bool,
    pub annual_aberration: bool,
    pub proper_motion: bool,
    pub atmospheric_refraction: bool,
    pub topocentric_solar_system: bool,
}

impl Default for CorrectionSnapshot {
    fn default() -> Self {
        Self {
            time_scales: true,
            precession: true,
            nutation: true,
            annual_aberration: true,
            proper_motion: true,
            atmospheric_refraction: true,
            topocentric_solar_system: true,
        }
    }
}

impl CorrectionSnapshot {
    /// Current renderer correction switch table for a scene. The apparent-place
    /// corrections are always active; atmospheric refraction follows the
    /// atmosphere enable switch because `Atmosphere::OFF` uploads zero pressure.
    pub fn for_scene(atmosphere: Atmosphere) -> Self {
        Self {
            atmospheric_refraction: atmosphere.sunlit_scattering && atmosphere.pressure_hpa > 0.0,
            ..Self::default()
        }
    }
}

impl StarSession {
    pub fn from_scene(
        app_version: impl Into<String>,
        created_by: impl Into<String>,
        scene: &SessionScene,
    ) -> Self {
        Self {
            schema_version: SESSION_SCHEMA_VERSION,
            app_version: app_version.into(),
            created_by: created_by.into(),
            observer: SessionObserver {
                latitude_deg: scene.latitude_deg,
                longitude_deg: scene.longitude_deg,
            },
            time: SessionTime::from(scene.time),
            view: SessionView {
                azimuth_deg: scene.view.azimuth_rad.to_degrees() as f64,
                altitude_deg: scene.view.altitude_rad.to_degrees() as f64,
                fov_deg: scene.view.fov_y_rad.to_degrees() as f64,
            },
            overlays: SessionOverlays {
                layers: scene
                    .overlays
                    .layers
                    .iter()
                    .copied()
                    .map(OverlayArg::from)
                    .collect(),
                grid_step_deg: scene.overlays.grid_step_deg,
                opacity: scene.overlays.opacity,
                deep_sky_magnitude_limit: Some(scene.overlays.deep_sky_magnitude_limit),
            },
            projection: SessionProjection {
                projection: ProjectionArg::from(scene.projection),
                viewpoint: ViewpointArg::from(scene.viewpoint),
                external: SessionExternalViewpoint::from(scene.external_viewpoint),
            },
            atmosphere: SessionAtmosphere::from_parts(scene.atmosphere_preset, scene.atmosphere),
            light_pollution: SessionLightPollution::from(scene.light_pollution),
            scintillation: SessionScintillation::from(scene.scintillation),
            planets: SessionPlanets {
                enabled: scene.planets_enabled,
            },
            satellites: SessionSatellites::from(&scene.satellites),
            eyepiece: SessionEyepiece::from(scene.eyepiece),
            catalog: scene.catalog.clone(),
            corrections: scene.corrections,
        }
    }

    pub fn to_scene(&self) -> Result<SessionScene> {
        if self.schema_version != SESSION_SCHEMA_VERSION {
            bail!(
                "Unsupported session schemaVersion {} (expected {})",
                self.schema_version,
                SESSION_SCHEMA_VERSION
            );
        }
        let time = TimeScales::from_utc_julian_date_with_dut1(
            finite(self.time.jd_utc, "time.jdUtc")?,
            finite(self.time.dut1_seconds, "time.dut1Seconds")?,
        );
        let atmosphere_preset: AtmospherePreset = self.atmosphere.preset.into();
        Ok(SessionScene {
            latitude_deg: finite_in_range(
                self.observer.latitude_deg,
                -90.0,
                90.0,
                "observer.latitudeDeg",
            )?,
            longitude_deg: finite_in_range(
                self.observer.longitude_deg,
                -180.0,
                180.0,
                "observer.longitudeDeg",
            )?,
            time,
            view: LocalView {
                azimuth_rad:
                    (finite_in_range(self.view.azimuth_deg, 0.0, 360.0, "view.azimuthDeg")? as f32)
                        .to_radians(),
                altitude_rad: (finite_in_range(
                    self.view.altitude_deg,
                    -90.0,
                    90.0,
                    "view.altitudeDeg",
                )? as f32)
                    .to_radians(),
                fov_y_rad: (finite_in_range(self.view.fov_deg, 0.05, 120.0, "view.fovDeg")? as f32)
                    .to_radians(),
            }
            .clamped(),
            overlays: OverlayConfig {
                layers: self
                    .overlays
                    .layers
                    .iter()
                    .copied()
                    .map(OverlayKind::from)
                    .collect(),
                grid_step_deg: finite_in_range(
                    self.overlays.grid_step_deg,
                    1.0,
                    90.0,
                    "overlays.gridStepDeg",
                )?,
                opacity: finite_in_range(
                    self.overlays.opacity as f64,
                    0.0,
                    1.0,
                    "overlays.opacity",
                )? as f32,
                deep_sky_magnitude_limit: match self.overlays.deep_sky_magnitude_limit {
                    Some(value) => {
                        finite_in_range(value as f64, -5.0, 99.0, "overlays.deepSkyMagnitudeLimit")?
                            as f32
                    }
                    None => OverlayConfig::default().deep_sky_magnitude_limit,
                },
            },
            atmosphere_preset,
            atmosphere: self.atmosphere.to_atmosphere()?,
            light_pollution: self.light_pollution.to_light_pollution()?,
            scintillation: self.scintillation.to_scintillation()?,
            planets_enabled: self.planets.enabled,
            satellites: self.satellites.to_satellite_layer(),
            projection: self.projection.projection.into(),
            viewpoint: self.projection.viewpoint.into(),
            external_viewpoint: self.projection.external.to_external_viewpoint()?,
            eyepiece: self.eyepiece.to_eyepiece()?,
            catalog: self.catalog.validated()?,
            corrections: self.corrections,
        })
    }
}

impl SessionTime {
    pub fn to_time_scales(self) -> TimeScales {
        TimeScales::from_utc_julian_date_with_dut1(self.jd_utc, self.dut1_seconds)
    }
}

impl From<TimeScales> for SessionTime {
    fn from(t: TimeScales) -> Self {
        Self {
            jd_utc: t.jd_utc,
            jd_ut1: t.jd_ut1,
            jd_tai: t.jd_tai,
            jd_tt: t.jd_tt,
            jd_tdb: t.jd_tdb,
            tai_minus_utc_seconds: t.tai_minus_utc_seconds,
            dut1_seconds: t.dut1_seconds,
        }
    }
}

impl SessionAtmosphere {
    pub fn from_parts(preset: AtmospherePreset, atmosphere: Atmosphere) -> Self {
        Self {
            enabled: atmosphere.sunlit_scattering,
            preset: AtmospherePresetArg::from(preset),
            aerosol_beta: atmosphere.aerosol_beta,
            aerosol_alpha: atmosphere.aerosol_alpha,
            observer_altitude_m: atmosphere.observer_altitude_m,
            ozone_du: atmosphere.ozone_du,
            pressure_hpa: atmosphere.pressure_hpa,
            temperature_c: atmosphere.temperature_c,
            surface_albedo: atmosphere.surface_albedo,
        }
    }

    pub fn to_atmosphere(self) -> Result<Atmosphere> {
        if !self.enabled {
            return Ok(Atmosphere::OFF);
        }
        let mut atmosphere = Atmosphere::from_preset(self.preset.into());
        atmosphere.aerosol_beta =
            finite_in_range(self.aerosol_beta as f64, 0.0, 2.0, "atmosphere.aerosolBeta")? as f32;
        atmosphere.aerosol_alpha = finite_in_range(
            self.aerosol_alpha as f64,
            0.0,
            4.0,
            "atmosphere.aerosolAlpha",
        )? as f32;
        atmosphere.observer_altitude_m = finite_in_range(
            self.observer_altitude_m as f64,
            0.0,
            9_000.0,
            "atmosphere.observerAltitudeM",
        )? as f32;
        atmosphere.ozone_du =
            finite_in_range(self.ozone_du as f64, 0.0, 600.0, "atmosphere.ozoneDu")? as f32;
        atmosphere.pressure_hpa = finite_in_range(
            self.pressure_hpa as f64,
            0.0,
            1_100.0,
            "atmosphere.pressureHpa",
        )? as f32;
        atmosphere.temperature_c = finite_in_range(
            self.temperature_c as f64,
            -80.0,
            60.0,
            "atmosphere.temperatureC",
        )? as f32;
        atmosphere.surface_albedo = finite_in_range(
            self.surface_albedo as f64,
            0.0,
            1.0,
            "atmosphere.surfaceAlbedo",
        )? as f32;
        Ok(atmosphere)
    }
}

impl From<Scintillation> for SessionScintillation {
    fn from(s: Scintillation) -> Self {
        Self {
            enabled: s.enabled,
            c_n2_scale: s.c_n2_scale,
            seed: s.seed,
        }
    }
}

impl SessionScintillation {
    pub fn to_scintillation(self) -> Result<Scintillation> {
        let c_n2_scale =
            finite_in_range(self.c_n2_scale as f64, 0.0, 10.0, "scintillation.cN2Scale")? as f32;
        Ok(Scintillation {
            enabled: self.enabled,
            c_n2_scale,
            seed: self.seed,
        })
    }
}

impl SessionEyepiece {
    fn to_eyepiece(self) -> Result<EyepieceSimulation> {
        Ok(EyepieceSimulation {
            enabled: self.enabled,
            aperture_mm: finite_in_range(
                self.aperture_mm as f64,
                10.0,
                2_000.0,
                "eyepiece.apertureMm",
            )? as f32,
            focal_length_mm: finite_in_range(
                self.focal_length_mm as f64,
                50.0,
                20_000.0,
                "eyepiece.focalLengthMm",
            )? as f32,
            eyepiece_focal_length_mm: finite_in_range(
                self.eyepiece_focal_length_mm as f64,
                1.0,
                100.0,
                "eyepiece.eyepieceFocalLengthMm",
            )? as f32,
            apparent_fov_deg: finite_in_range(
                self.apparent_fov_deg as f64,
                1.0,
                120.0,
                "eyepiece.apparentFovDeg",
            )? as f32,
            field_stop_mm: finite_in_range(
                self.field_stop_mm as f64,
                0.0,
                120.0,
                "eyepiece.fieldStopMm",
            )? as f32,
        })
    }
}

impl CatalogSnapshot {
    fn validated(&self) -> Result<Self> {
        let mut catalog = self.clone();
        catalog.limiting_magnitude = finite_in_range(
            self.limiting_magnitude as f64,
            -30.0,
            30.0,
            "catalog.limitingMagnitude",
        )? as f32;
        Ok(catalog)
    }
}

impl SessionExternalViewpoint {
    fn to_external_viewpoint(self) -> Result<ExternalViewpoint> {
        Ok(ExternalViewpoint::new(
            self.origin_pc.array_in_range(
                -1_000_000.0,
                1_000_000.0,
                "projection.external.originPc",
            )?,
            self.target_pc.array_in_range(
                -1_000_000.0,
                1_000_000.0,
                "projection.external.targetPc",
            )?,
            self.up
                .array_in_range(-10.0, 10.0, "projection.external.up")?,
        ))
    }
}

impl SessionVec3 {
    fn array_in_range(self, min: f64, max: f64, name: &str) -> Result<[f32; 3]> {
        Ok([
            finite_in_range(self.x as f64, min, max, name)? as f32,
            finite_in_range(self.y as f64, min, max, name)? as f32,
            finite_in_range(self.z as f64, min, max, name)? as f32,
        ])
    }
}

impl From<ExternalViewpoint> for SessionExternalViewpoint {
    fn from(v: ExternalViewpoint) -> Self {
        Self {
            origin_pc: SessionVec3::from(v.origin_pc),
            target_pc: SessionVec3::from(v.target_pc),
            up: SessionVec3::from(v.up),
        }
    }
}

impl From<SessionExternalViewpoint> for ExternalViewpoint {
    fn from(v: SessionExternalViewpoint) -> Self {
        Self::new(v.origin_pc.into(), v.target_pc.into(), v.up.into())
    }
}

impl From<[f32; 3]> for SessionVec3 {
    fn from(v: [f32; 3]) -> Self {
        Self {
            x: v[0],
            y: v[1],
            z: v[2],
        }
    }
}

impl From<SessionVec3> for [f32; 3] {
    fn from(v: SessionVec3) -> Self {
        [v.x, v.y, v.z]
    }
}

impl From<EyepieceSimulation> for SessionEyepiece {
    fn from(e: EyepieceSimulation) -> Self {
        Self {
            enabled: e.enabled,
            aperture_mm: e.aperture_mm,
            focal_length_mm: e.focal_length_mm,
            eyepiece_focal_length_mm: e.eyepiece_focal_length_mm,
            apparent_fov_deg: e.apparent_fov_deg,
            field_stop_mm: e.field_stop_mm,
        }
    }
}

impl From<SessionEyepiece> for EyepieceSimulation {
    fn from(e: SessionEyepiece) -> Self {
        Self {
            enabled: e.enabled,
            aperture_mm: e.aperture_mm,
            focal_length_mm: e.focal_length_mm,
            eyepiece_focal_length_mm: e.eyepiece_focal_length_mm,
            apparent_fov_deg: e.apparent_fov_deg,
            field_stop_mm: e.field_stop_mm,
        }
    }
}

impl From<OverlayKind> for OverlayArg {
    fn from(kind: OverlayKind) -> Self {
        match kind {
            OverlayKind::Horizon => Self::Horizon,
            OverlayKind::Cardinals => Self::Cardinals,
            OverlayKind::AltAzGrid => Self::AltAzGrid,
            OverlayKind::EquatorialGrid => Self::EquatorialGrid,
            OverlayKind::Ecliptic => Self::Ecliptic,
            OverlayKind::CelestialEquator => Self::CelestialEquator,
            OverlayKind::Meridian => Self::Meridian,
            OverlayKind::GalacticEquator => Self::GalacticEquator,
            OverlayKind::ConstellationLines => Self::ConstellationLines,
            OverlayKind::ConstellationBoundaries => Self::ConstellationBoundaries,
            OverlayKind::DeepSkyObjects => Self::DeepSkyObjects,
            OverlayKind::DeepSkyLabels => Self::DeepSkyLabels,
            OverlayKind::StarLabels => Self::StarLabels,
            OverlayKind::PlanetLabels => Self::PlanetLabels,
            OverlayKind::ConstellationLabels => Self::ConstellationLabels,
            OverlayKind::CardinalLabels => Self::CardinalLabels,
            OverlayKind::DegreeLabels => Self::DegreeLabels,
        }
    }
}

impl From<SkyProjection> for ProjectionArg {
    fn from(projection: SkyProjection) -> Self {
        match projection {
            SkyProjection::Perspective => Self::Perspective,
            SkyProjection::Mollweide => Self::Mollweide,
            SkyProjection::Aitoff => Self::Aitoff,
            SkyProjection::Hammer => Self::Hammer,
        }
    }
}

impl From<SkyViewpoint> for ViewpointArg {
    fn from(viewpoint: SkyViewpoint) -> Self {
        match viewpoint {
            SkyViewpoint::Earth => Self::Earth,
            SkyViewpoint::GalacticNorth => Self::GalacticNorth,
            SkyViewpoint::CustomExternal => Self::CustomExternal,
        }
    }
}

impl From<AtmospherePreset> for AtmospherePresetArg {
    fn from(preset: AtmospherePreset) -> Self {
        match preset {
            AtmospherePreset::ClearRural => Self::ClearRural,
            AtmospherePreset::HazyUrban => Self::HazyUrban,
            AtmospherePreset::HighAltitude => Self::HighAltitude,
        }
    }
}

pub fn load_session(path: impl AsRef<Path>) -> Result<StarSession> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Reading session JSON at {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("Parsing session JSON at {}", path.display()))
}

pub fn save_session(path: impl AsRef<Path>, session: &StarSession) -> Result<()> {
    let path = path.as_ref();
    let raw = serde_json::to_string_pretty(session).context("Serializing session JSON")?;
    fs::write(path, format!("{raw}\n"))
        .with_context(|| format!("Writing session JSON to {}", path.display()))
}

fn finite(value: f64, name: &str) -> Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        bail!("session field {name}={value:?} is not finite")
    }
}

fn finite_in_range(value: f64, min: f64, max: f64, name: &str) -> Result<f64> {
    if value.is_finite() && value >= min && value <= max {
        Ok(value)
    } else {
        bail!("session field {name}={value:?} is outside {min}..={max}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_scene() -> SessionScene {
        SessionScene {
            latitude_deg: 35.68,
            longitude_deg: 139.69,
            time: TimeScales::from_utc_julian_date_with_dut1(2_461_000.5, 0.1),
            view: LocalView {
                azimuth_rad: 180_f32.to_radians(),
                altitude_rad: 30_f32.to_radians(),
                fov_y_rad: 70_f32.to_radians(),
            },
            overlays: OverlayConfig {
                layers: vec![OverlayKind::Horizon, OverlayKind::CardinalLabels],
                grid_step_deg: 15.0,
                opacity: 0.6,
                deep_sky_magnitude_limit: OverlayConfig::default().deep_sky_magnitude_limit,
            },
            atmosphere_preset: AtmospherePreset::ClearRural,
            atmosphere: Atmosphere::CLEAR_RURAL,
            light_pollution: LightPollution::default(),
            scintillation: Scintillation::default(),
            planets_enabled: true,
            satellites: curated_satellite_layer(true, 1.5),
            projection: SkyProjection::Perspective,
            viewpoint: SkyViewpoint::Earth,
            external_viewpoint: ExternalViewpoint::GALACTIC_NORTH,
            eyepiece: EyepieceSimulation::OFF,
            catalog: crate::hyg_catalog_snapshot("crates/catalog/data/hyg_v42.csv", 7.5),
            corrections: CorrectionSnapshot::default(),
        }
    }

    #[test]
    fn session_json_round_trips_scene() {
        let scene = sample_scene();
        let session = StarSession::from_scene("0.1.0", "test", &scene);
        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("\"schemaVersion\":6"));
        assert!(json.contains("\"cardinal-labels\""));
        assert!(json.contains("\"scintillation\""));
        let parsed: StarSession = serde_json::from_str(&json).unwrap();
        let restored = parsed.to_scene().unwrap();
        assert_eq!(restored.latitude_deg, scene.latitude_deg);
        assert_eq!(restored.longitude_deg, scene.longitude_deg);
        assert!((restored.time.jd_utc - scene.time.jd_utc).abs() < 1e-12);
        assert_eq!(restored.overlays.layers, scene.overlays.layers);
        assert_eq!(restored.projection, scene.projection);
        assert_eq!(restored.viewpoint, scene.viewpoint);
        assert_eq!(restored.eyepiece, scene.eyepiece);
        assert_eq!(restored.scintillation, scene.scintillation);
    }

    #[test]
    fn rejects_future_schema() {
        let mut session = StarSession::from_scene("0.1.0", "test", &sample_scene());
        session.schema_version = SESSION_SCHEMA_VERSION + 1;
        assert!(session.to_scene().is_err());
    }

    #[test]
    fn light_pollution_serializes_and_round_trips() {
        for pollution in [
            LightPollution::Bortle(1),
            LightPollution::Bortle(5),
            LightPollution::Bortle(8),
            LightPollution::Sqm(19.5),
            LightPollution::Atlas2016 {
                latitude_deg: 35.68,
                longitude_deg: 139.69,
            },
        ] {
            let session_lp = SessionLightPollution::from(pollution);
            let back = session_lp.to_light_pollution().unwrap();
            assert_eq!(back, pollution, "round-trip failed for {pollution:?}");
        }
    }

    #[test]
    fn light_pollution_rejects_invalid_bortle_class() {
        let session_lp = SessionLightPollution {
            kind: SessionLightPollutionKind::Bortle,
            bortle: Some(42),
            sqm_mag_per_arcsec2: None,
            atlas_latitude_deg: None,
            atlas_longitude_deg: None,
        };
        assert!(session_lp.to_light_pollution().is_err());
    }

    #[test]
    fn light_pollution_rejects_missing_kind_fields() {
        let session_lp = SessionLightPollution {
            kind: SessionLightPollutionKind::Sqm,
            bortle: Some(3), // wrong field for kind=sqm; sqmMagPerArcsec2 is required.
            sqm_mag_per_arcsec2: None,
            atlas_latitude_deg: None,
            atlas_longitude_deg: None,
        };
        assert!(session_lp.to_light_pollution().is_err());
    }

    #[test]
    fn rejects_out_of_range_session_controls() {
        let mut session = StarSession::from_scene("0.1.0", "test", &sample_scene());
        session.atmosphere.aerosol_beta = 100.0;
        assert!(session.to_scene().is_err());

        let mut session = StarSession::from_scene("0.1.0", "test", &sample_scene());
        session.projection.external.up = SessionVec3 {
            x: 0.0,
            y: 0.0,
            z: 100.0,
        };
        assert!(session.to_scene().is_err());

        let mut session = StarSession::from_scene("0.1.0", "test", &sample_scene());
        session.catalog.limiting_magnitude = 100.0;
        assert!(session.to_scene().is_err());
    }
}
