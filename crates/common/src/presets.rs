//! Deterministic scene presets shared by the native hosts.
//!
//! The presets intentionally compile down to the same schema-versioned
//! [`StarSession`] representation as imported/exported sessions.  They are a
//! reproducibility layer, not a second scene format: every named scene can be
//! rendered directly, exported as JSON, imported by another host, and used as a
//! stable validation/demo target.

use std::path::Path;

use anyhow::Result;
use clap::ValueEnum;
use renderer::{
    Atmosphere, AtmospherePreset, ExternalViewpoint, EyepieceSimulation, LightPollution, LocalView,
    MeteorLayer, OutputColourSpace, OverlayConfig, OverlayKind, SatelliteLayer, Scintillation,
    SkyProjection, SkyViewpoint,
};
use serde::{Deserialize, Serialize};

use crate::{
    curated_satellite_layer, hyg_catalog_snapshot, parse_time_to_time_scales, CatalogSnapshot,
    CorrectionSnapshot, SessionScene, StarSession,
};

/// CLI-facing stable identifiers for built-in deterministic scenes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScenePresetArg {
    TokyoTonight,
    #[value(name = "tokyo-bortle-8")]
    TokyoBortle8,
    DarkSky,
    #[value(name = "dark-sky-bortle-1")]
    DarkSkyBortle1,
    Noon,
    Sunset,
    CivilTwilight,
    CivilTwilightAntisolarTokyo,
    NauticalTwilight,
    AstronomicalTwilight,
    MoonlitNight,
    EclipseAid,
    SolarEclipse,
    VenusTransit,
    JupiterShadowTransit,
    IssPass,
    AllSkyHammer,
    AllSkyMollweide,
    GalacticNorth,
    CustomExternal,
}

impl ScenePresetArg {
    pub const ALL: &'static [Self] = &[
        Self::TokyoTonight,
        Self::TokyoBortle8,
        Self::DarkSky,
        Self::DarkSkyBortle1,
        Self::Noon,
        Self::Sunset,
        Self::CivilTwilight,
        Self::CivilTwilightAntisolarTokyo,
        Self::NauticalTwilight,
        Self::AstronomicalTwilight,
        Self::MoonlitNight,
        Self::EclipseAid,
        Self::SolarEclipse,
        Self::VenusTransit,
        Self::JupiterShadowTransit,
        Self::IssPass,
        Self::AllSkyHammer,
        Self::AllSkyMollweide,
        Self::GalacticNorth,
        Self::CustomExternal,
    ];

    pub const fn as_kebab_str(self) -> &'static str {
        match self {
            Self::TokyoTonight => "tokyo-tonight",
            Self::TokyoBortle8 => "tokyo-bortle-8",
            Self::DarkSky => "dark-sky",
            Self::DarkSkyBortle1 => "dark-sky-bortle-1",
            Self::Noon => "noon",
            Self::Sunset => "sunset",
            Self::CivilTwilight => "civil-twilight",
            Self::CivilTwilightAntisolarTokyo => "civil-twilight-antisolar-tokyo",
            Self::NauticalTwilight => "nautical-twilight",
            Self::AstronomicalTwilight => "astronomical-twilight",
            Self::MoonlitNight => "moonlit-night",
            Self::EclipseAid => "eclipse-aid",
            Self::SolarEclipse => "solar-eclipse",
            Self::VenusTransit => "venus-transit",
            Self::JupiterShadowTransit => "jupiter-shadow-transit",
            Self::IssPass => "iss-pass",
            Self::AllSkyHammer => "all-sky-hammer",
            Self::AllSkyMollweide => "all-sky-mollweide",
            Self::GalacticNorth => "galactic-north",
            Self::CustomExternal => "custom-external",
        }
    }
}

impl std::fmt::Display for ScenePresetArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_kebab_str())
    }
}

/// Human-readable preset metadata for listing, docs, and gallery generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScenePresetInfo {
    pub id: ScenePresetArg,
    pub title: &'static str,
    pub description: &'static str,
    pub validation_focus: &'static str,
}

pub const SCENE_PRESET_INFOS: &[ScenePresetInfo] = &[
    ScenePresetInfo {
        id: ScenePresetArg::TokyoTonight,
        title: "Tokyo summer evening",
        description: "Representative Tokyo night-sky view with horizon, labels, ecliptic, and galactic equator overlays.",
        validation_focus: "default local perspective, overlays, labels, star/planet composition",
    },
    ScenePresetInfo {
        id: ScenePresetArg::TokyoBortle8,
        title: "Tokyo (Bortle 8 city sky)",
        description: "Same Tokyo evening framing as `tokyo-tonight` but with a Bortle-8 city-sky light-pollution overlay (V-39).",
        validation_focus: "V-39 Bortle-8 artificial sky glow: warm-orange sodium/LED tint, horizon-brighter Garstang fall-off, dimmed faint stars",
    },
    ScenePresetInfo {
        id: ScenePresetArg::DarkSky,
        title: "High-altitude dark sky",
        description: "Mauna Kea dark-sky view using the high-altitude atmosphere preset and Milky Way-oriented overlays.",
        validation_focus: "dark-sky glow, extinction, Milky Way band, high-altitude atmosphere",
    },
    ScenePresetInfo {
        id: ScenePresetArg::DarkSkyBortle1,
        title: "Dark sky (Bortle 1 rural floor)",
        description: "Mauna Kea dark-sky view pinned with the V-39 Bortle 1 / dark-sky floor — must render pixel-identically to `dark-sky` because Bortle 1 adds zero artificial glow.",
        validation_focus: "V-39 Bortle 1 default: round-trips to the pre-V-39 dark-sky composition without spectral or photometric drift",
    },
    ScenePresetInfo {
        id: ScenePresetArg::Noon,
        title: "Clear-sky noon",
        description: "Tokyo local noon daylight scene aimed near the Sun with urban-haze controls disabled.",
        validation_focus: "daylight scattering domain, solar disk, star suppression by sky radiance",
    },
    ScenePresetInfo {
        id: ScenePresetArg::Sunset,
        title: "Sunset horizon",
        description: "Tokyo sunset horizon scene for warm low-Sun scattering and horizon haze.",
        validation_focus: "golden-hour colour, horizon overlays, low solar altitude continuity",
    },
    ScenePresetInfo {
        id: ScenePresetArg::CivilTwilight,
        title: "Civil twilight",
        description: "Solar-depression twilight scene just after sunset with the western horizon in frame.",
        validation_focus: "civil twilight band, additive twilight/dark-sky transition",
    },
    ScenePresetInfo {
        id: ScenePresetArg::CivilTwilightAntisolarTokyo,
        title: "Civil twilight, anti-solar (Tokyo)",
        description: "Tokyo civil-twilight scene framed on the anti-solar horizon to expose the Belt of Venus (pink anti-twilight arch) above the Earth-shadow band (V-27).",
        validation_focus: "V-27 anti-solar twilight structure: Belt of Venus chromaticity, Earth-shadow band luminance, R/G ratio at pinned ROI altitudes",
    },
    ScenePresetInfo {
        id: ScenePresetArg::NauticalTwilight,
        title: "Nautical twilight",
        description: "Darker western-horizon scene for the nautical twilight regime.",
        validation_focus: "nautical twilight band, first bright-star visibility",
    },
    ScenePresetInfo {
        id: ScenePresetArg::AstronomicalTwilight,
        title: "Astronomical twilight",
        description: "Late twilight scene where the model should approach the dark-sky background.",
        validation_focus: "astronomical twilight boundary, dark-sky continuity",
    },
    ScenePresetInfo {
        id: ScenePresetArg::MoonlitNight,
        title: "Moonlit night",
        description: "Bright Moon night scene for lunar disk phase and moonlit sky contribution checks.",
        validation_focus: "Moon disk, lunar illuminance, moonlit sky additive term",
    },
    ScenePresetInfo {
        id: ScenePresetArg::EclipseAid,
        title: "Lunar eclipse aid",
        description: "Tight Moon-oriented aid scene around a lunar-eclipse date with planet labels and horizon context.",
        validation_focus: "Moon phase rendering, Earth-shadow aid input path, narrow-field labels",
    },
    ScenePresetInfo {
        id: ScenePresetArg::SolarEclipse,
        title: "Total solar eclipse",
        description: "2024-04-08 total solar eclipse from Mazatl\u{00e1}n at greatest eclipse. Wires V-51 occultation primitives (analytic Moon-on-Sun mask, Koomen 1952 daylight falloff, Baumbach 1937 corona) to a real validated event.",
        validation_focus: "V-51c solar-eclipse pipeline: analytic-mask Sun subtract, daylight darkening, corona during totality",
    },
    ScenePresetInfo {
        id: ScenePresetArg::VenusTransit,
        title: "Venus transit of the Sun",
        description: "2012-06-06 Venus transit observed from Tokyo near greatest transit. Wires V-51e planet-on-Sun analytic-mask occluder against the only Venus transit in the validation canon until 2117.",
        validation_focus: "V-51e planet-on-Sun pipeline: planet apparent disk as analytic mask inside the solar disk, daylight-band sky unchanged outside the disk",
    },
    ScenePresetInfo {
        id: ScenePresetArg::JupiterShadowTransit,
        title: "Jupiter Galilean shadow transit",
        description: "2008-12-20 ~14:00 UT Io shadow transit on Jupiter observed from Roque de los Muchachos, Canary Islands (Io ingress at 13:14 UT, mid-transit ~14:30 UT; Jupiter rides at altitude ~39° SE from this site). Wires the V-52d Galilean-shadow producer onto the V-51b analytic-mask Planet-on-Planet path so Io's silhouette darkens the Jovian disk inside the eyepiece field.",
        validation_focus: "V-52d Galilean-shadow pipeline: Sun-projected moon position emits a small Planet(Jupiter)-targeted occluder; V-52b moon sprites unaffected outside Jupiter's disk",
    },
    ScenePresetInfo {
        id: ScenePresetArg::IssPass,
        title: "ISS pass (dark-sky)",
        description: "2026-05-30 ~05:57 UTC near-zenith International Space Station pass from a fixed 45°S / 0° observer (alt ≈ 82°, az ≈ 242°, fully sunlit) at astronomical twilight from the curated CelesTrak TLE snapshot. Wires the V-55 SGP4 satellite layer: TEME→topocentric reduction, Earth-shadow visibility, and McCants apparent magnitude.",
        validation_focus: "V-55 satellite layer: SGP4 propagation, sunlit/above-horizon gating, apparent-magnitude sprite against a dark sky at the curated TLE epoch",
    },
    ScenePresetInfo {
        id: ScenePresetArg::AllSkyHammer,
        title: "Hammer all-sky map",
        description: "Full-sky Hammer projection with equatorial, ecliptic, and galactic overlays.",
        validation_focus: "Hammer projection, full-sky overlay clipping, map-scale uniforms",
    },
    ScenePresetInfo {
        id: ScenePresetArg::AllSkyMollweide,
        title: "Mollweide all-sky map",
        description: "Full-sky equal-area Mollweide projection using the same fixed inputs as the Hammer map.",
        validation_focus: "Mollweide projection, full-sky Milky Way and coordinate grid",
    },
    ScenePresetInfo {
        id: ScenePresetArg::GalacticNorth,
        title: "Galactic-north viewpoint",
        description: "External parsec-scale top-down Milky Way view from the built-in galactic-north camera.",
        validation_focus: "external viewpoint, HYG distances, analytic Milky Way disk",
    },
    ScenePresetInfo {
        id: ScenePresetArg::CustomExternal,
        title: "Custom external viewpoint",
        description: "Oblique custom external galactic-frame camera with non-default origin, target, and up vector.",
        validation_focus: "custom external camera serialization, orientation, distance-scaled star field",
    },
];

/// Return the public metadata for every built-in deterministic scene.
pub fn scene_preset_infos() -> &'static [ScenePresetInfo] {
    SCENE_PRESET_INFOS
}

/// Build the renderer-ready scene for a built-in preset.
pub fn scene_from_preset(
    preset: ScenePresetArg,
    catalog_path: impl AsRef<Path>,
    limiting_magnitude: f32,
) -> Result<SessionScene> {
    let catalog = hyg_catalog_snapshot(catalog_path.as_ref(), limiting_magnitude);
    let mut scene = match preset {
        ScenePresetArg::TokyoTonight => earth_scene(
            35.68,
            139.69,
            "2026-08-13T12:00:00Z",
            180.0,
            35.0,
            75.0,
            overlay_config(&[
                OverlayKind::Horizon,
                OverlayKind::Cardinals,
                OverlayKind::CardinalLabels,
                OverlayKind::Ecliptic,
                OverlayKind::GalacticEquator,
                OverlayKind::ConstellationLines,
                OverlayKind::PlanetLabels,
            ]),
            AtmospherePreset::ClearRural,
            catalog,
        )?,
        ScenePresetArg::TokyoBortle8 => {
            // V-39 validation: Tokyo evening with the city-sky (Bortle 8)
            // artificial-skyglow overlay. The hazy-urban atmosphere preset is
            // used because anyone seeing Bortle 8 is also looking through the
            // associated aerosol load; sharing the preset makes the rendered
            // colour balance reflect both effects at once.
            let mut scene = earth_scene(
                35.68,
                139.69,
                "2026-08-13T12:00:00Z",
                180.0,
                35.0,
                75.0,
                overlay_config(&[
                    OverlayKind::Horizon,
                    OverlayKind::Cardinals,
                    OverlayKind::CardinalLabels,
                    OverlayKind::Ecliptic,
                    OverlayKind::GalacticEquator,
                    OverlayKind::ConstellationLines,
                    OverlayKind::PlanetLabels,
                ]),
                AtmospherePreset::HazyUrban,
                catalog,
            )?;
            scene.light_pollution = LightPollution::Bortle(8);
            scene
        }
        ScenePresetArg::DarkSky => earth_scene(
            19.8207,
            -155.4681,
            "2026-07-18T10:30:00Z",
            155.0,
            55.0,
            85.0,
            overlay_config(&[
                OverlayKind::Horizon,
                OverlayKind::CardinalLabels,
                OverlayKind::GalacticEquator,
                OverlayKind::ConstellationLabels,
            ]),
            AtmospherePreset::HighAltitude,
            catalog,
        )?,
        ScenePresetArg::DarkSkyBortle1 => {
            // V-39 validation: same dark-sky scene, but explicitly pinned to
            // the Bortle 1 floor. Bortle 1 emits zero artificial S10, so the
            // gallery render of this scene must equal the `dark-sky` render
            // byte-for-byte on a deterministic GPU; the dedicated entry
            // exists so a future regression that accidentally biases the
            // artificial term has a named target to fail on.
            let mut scene = earth_scene(
                19.8207,
                -155.4681,
                "2026-07-18T10:30:00Z",
                155.0,
                55.0,
                85.0,
                overlay_config(&[
                    OverlayKind::Horizon,
                    OverlayKind::CardinalLabels,
                    OverlayKind::GalacticEquator,
                    OverlayKind::ConstellationLabels,
                ]),
                AtmospherePreset::HighAltitude,
                catalog,
            )?;
            scene.light_pollution = LightPollution::Bortle(1);
            scene
        }
        ScenePresetArg::Noon => earth_scene(
            35.68,
            139.69,
            "2026-06-21T03:00:00Z",
            180.0,
            72.0,
            70.0,
            overlay_config(&[
                OverlayKind::Horizon,
                OverlayKind::CardinalLabels,
                OverlayKind::PlanetLabels,
            ]),
            AtmospherePreset::ClearRural,
            catalog,
        )?,
        ScenePresetArg::Sunset => earth_scene(
            35.68,
            139.69,
            "2026-06-21T09:55:00Z",
            285.0,
            6.0,
            72.0,
            overlay_config(&[
                OverlayKind::Horizon,
                OverlayKind::CardinalLabels,
                OverlayKind::PlanetLabels,
            ]),
            AtmospherePreset::ClearRural,
            catalog,
        )?,
        ScenePresetArg::CivilTwilight => earth_scene(
            35.68,
            139.69,
            "2026-06-21T10:20:00Z",
            290.0,
            10.0,
            75.0,
            overlay_config(&[
                OverlayKind::Horizon,
                OverlayKind::CardinalLabels,
                OverlayKind::Ecliptic,
                OverlayKind::PlanetLabels,
            ]),
            AtmospherePreset::ClearRural,
            catalog,
        )?,
        // V-27 validation scene: civil twilight from Tokyo on 2026-06-21 at
        // 10:20 UTC (the same epoch as the `civil-twilight` preset), framed
        // toward the anti-solar horizon (az ≈ 110°, alt ≈ 8°) so the
        // Belt of Venus arch and the Earth-shadow band underneath fall
        // inside the field. The FOV is the same 75° as the symmetric
        // civil-twilight preset, so the two scenes are directly comparable.
        ScenePresetArg::CivilTwilightAntisolarTokyo => earth_scene(
            35.68,
            139.69,
            "2026-06-21T10:20:00Z",
            110.0,
            8.0,
            75.0,
            overlay_config(&[
                OverlayKind::Horizon,
                OverlayKind::CardinalLabels,
                OverlayKind::Ecliptic,
                OverlayKind::PlanetLabels,
            ]),
            AtmospherePreset::ClearRural,
            catalog,
        )?,
        ScenePresetArg::NauticalTwilight => earth_scene(
            35.68,
            139.69,
            "2026-06-21T10:50:00Z",
            295.0,
            16.0,
            80.0,
            overlay_config(&[
                OverlayKind::Horizon,
                OverlayKind::CardinalLabels,
                OverlayKind::Ecliptic,
                OverlayKind::ConstellationLines,
            ]),
            AtmospherePreset::ClearRural,
            catalog,
        )?,
        ScenePresetArg::AstronomicalTwilight => earth_scene(
            35.68,
            139.69,
            "2026-06-21T11:30:00Z",
            300.0,
            24.0,
            85.0,
            overlay_config(&[
                OverlayKind::Horizon,
                OverlayKind::CardinalLabels,
                OverlayKind::GalacticEquator,
                OverlayKind::ConstellationLines,
            ]),
            AtmospherePreset::ClearRural,
            catalog,
        )?,
        ScenePresetArg::MoonlitNight => earth_scene(
            35.68,
            139.69,
            "2026-01-04T15:00:00Z",
            115.0,
            48.0,
            60.0,
            overlay_config(&[
                OverlayKind::Horizon,
                OverlayKind::CardinalLabels,
                OverlayKind::PlanetLabels,
                OverlayKind::ConstellationLabels,
            ]),
            AtmospherePreset::ClearRural,
            catalog,
        )?,
        ScenePresetArg::EclipseAid => earth_scene(
            35.68,
            139.69,
            "2026-03-03T11:45:00Z",
            92.0,
            28.0,
            22.0,
            overlay_config(&[
                OverlayKind::Horizon,
                OverlayKind::CardinalLabels,
                OverlayKind::PlanetLabels,
                OverlayKind::DegreeLabels,
            ]),
            AtmospherePreset::ClearRural,
            catalog,
        )?,
        // V-51c validation scene: 2024-04-08 total solar eclipse over
        // Mazatlán, framed on the solar disk at greatest eclipse. Apparent
        // Sun position there is az\u2248138\u00b0, alt\u224870\u00b0; the
        // narrow 5\u00b0 field exposes the Baumbach corona and the
        // analytic Moon-on-Sun subtract, and the Koomen 1952 falloff
        // darkens the sky background to the totality limit.
        ScenePresetArg::SolarEclipse => earth_scene(
            23.219,
            -106.420,
            "2024-04-08T18:13:00Z",
            138.0,
            70.0,
            5.0,
            overlay_config(&[
                OverlayKind::Horizon,
                OverlayKind::CardinalLabels,
                OverlayKind::DegreeLabels,
            ]),
            AtmospherePreset::ClearRural,
            catalog,
        )?,
        // V-51e validation scene: 2012-06-06 Venus transit, observed
        // from Tokyo near greatest transit (≈01:29 UT / 10:29 JST).
        // The apparent Sun at the pinned epoch sits at az ≈ 125.4°,
        // alt ≈ 69.8°; a 2° field frames the solar disk so Venus'
        // ~1′ silhouette is resolvable against the photosphere via
        // the V-51b analytic-mask subtract path with the V-51e
        // planet-on-Sun occluder.
        ScenePresetArg::VenusTransit => earth_scene(
            35.68,
            139.69,
            "2012-06-06T01:29:00Z",
            125.4,
            69.8,
            2.0,
            overlay_config(&[
                OverlayKind::Horizon,
                OverlayKind::CardinalLabels,
                OverlayKind::DegreeLabels,
            ]),
            AtmospherePreset::ClearRural,
            catalog,
        )?,
        // V-52d validation scene: 2008-12-20 ~14:00 UT Io shadow
        // transit on Jupiter. The Io shadow's first contact with the
        // Jovian limb (PHEMU09 canon) is 2008-12-20 13:14 UT; ~45 min
        // later the shadow sits well inside the disk. Tokyo has
        // Jupiter below the horizon at that UT, so the preset frames
        // the event from Roque de los Muchachos (Canary Islands,
        // 28.76°N / 17.89°W) where Jupiter rides ~39° up in the SE.
        // A 0.05° eyepiece field zooms in tight on the Jovian disk so
        // Io's silhouette is the dominant pixel feature; the V-51b
        // analytic-mask Planet-on-Planet subtract path draws the
        // dark shadow spot directly.
        ScenePresetArg::JupiterShadowTransit => earth_scene(
            28.76,
            -17.89,
            "2008-12-20T14:00:00Z",
            162.1,
            38.7,
            0.05,
            overlay_config(&[OverlayKind::CardinalLabels, OverlayKind::DegreeLabels]),
            AtmospherePreset::HighAltitude,
            catalog,
        )?,
        ScenePresetArg::IssPass => {
            // V-55 validation: a real, sunlit, near-zenith ISS pass at the
            // curated TLE snapshot epoch from a fixed Southern-Ocean observer
            // (45°S, 0°) where the Sun sits 15° below the horizon (astronomical
            // twilight) so the bright sunlit ISS lands against a dark sky
            // rather than washing out in bright twilight. The satellite layer
            // is enabled with the curated CelesTrak snapshot; the view is
            // framed on the ISS (alt ≈ 82°, az ≈ 242°).
            let mut scene = earth_scene(
                -45.0,
                0.0,
                "2026-05-30T05:57:00Z",
                242.0,
                82.0,
                70.0,
                overlay_config(&[
                    OverlayKind::Horizon,
                    OverlayKind::CardinalLabels,
                    OverlayKind::ConstellationLines,
                ]),
                AtmospherePreset::ClearRural,
                catalog,
            )?;
            scene.satellites = curated_satellite_layer(true, 0.0);
            scene
        }
        ScenePresetArg::AllSkyHammer => {
            all_sky_scene("2026-08-13T12:00:00Z", SkyProjection::Hammer, catalog)?
        }
        ScenePresetArg::AllSkyMollweide => {
            all_sky_scene("2026-08-13T12:00:00Z", SkyProjection::Mollweide, catalog)?
        }
        ScenePresetArg::GalacticNorth => external_scene(
            "2026-08-13T12:00:00Z",
            SkyProjection::Perspective,
            SkyViewpoint::GalacticNorth,
            ExternalViewpoint::GALACTIC_NORTH,
            60.0,
            catalog,
        )?,
        ScenePresetArg::CustomExternal => external_scene(
            "2026-08-13T12:00:00Z",
            SkyProjection::Perspective,
            SkyViewpoint::CustomExternal,
            ExternalViewpoint::new(
                [11_000.0, -18_000.0, 8_000.0],
                [0.0, 0.0, 0.0],
                [0.0, 0.2, 1.0],
            ),
            55.0,
            catalog,
        )?,
    };
    scene.corrections = CorrectionSnapshot::for_scene(scene.atmosphere);
    Ok(scene)
}

/// Build a schema-versioned JSON session for a built-in preset.
pub fn session_from_preset(
    preset: ScenePresetArg,
    app_version: impl Into<String>,
    created_by: impl Into<String>,
    catalog_path: impl AsRef<Path>,
    limiting_magnitude: f32,
) -> Result<StarSession> {
    let scene = scene_from_preset(preset, catalog_path, limiting_magnitude)?;
    Ok(StarSession::from_scene(app_version, created_by, &scene))
}

#[allow(clippy::too_many_arguments)]
fn earth_scene(
    latitude_deg: f64,
    longitude_deg: f64,
    time: &str,
    azimuth_deg: f64,
    altitude_deg: f64,
    fov_deg: f64,
    overlays: OverlayConfig,
    atmosphere_preset: AtmospherePreset,
    catalog: CatalogSnapshot,
) -> Result<SessionScene> {
    let atmosphere = Atmosphere::from_preset(atmosphere_preset);
    Ok(SessionScene {
        latitude_deg,
        longitude_deg,
        time: parse_time_to_time_scales(Some(time))?,
        view: local_view(azimuth_deg, altitude_deg, fov_deg),
        overlays,
        atmosphere_preset,
        atmosphere,
        light_pollution: LightPollution::default(),
        scintillation: Scintillation::default(),
        planets_enabled: true,
        satellites: SatelliteLayer::default(),
        projection: SkyProjection::Perspective,
        viewpoint: SkyViewpoint::Earth,
        external_viewpoint: ExternalViewpoint::GALACTIC_NORTH,
        eyepiece: EyepieceSimulation::OFF,
        catalog,
        corrections: CorrectionSnapshot::for_scene(atmosphere),
        output_colourspace: OutputColourSpace::default(),
        meteors: MeteorLayer::default(),
        aurora: renderer::AuroraLayer::default(),
    })
}

fn all_sky_scene(
    time: &str,
    projection: SkyProjection,
    catalog: CatalogSnapshot,
) -> Result<SessionScene> {
    earth_scene(
        35.68,
        139.69,
        time,
        180.0,
        35.0,
        90.0,
        overlay_config(&[
            OverlayKind::EquatorialGrid,
            OverlayKind::Ecliptic,
            OverlayKind::GalacticEquator,
            OverlayKind::ConstellationLines,
            OverlayKind::ConstellationBoundaries,
        ]),
        AtmospherePreset::ClearRural,
        catalog,
    )
    .map(|mut scene| {
        scene.projection = projection;
        scene
    })
}

fn external_scene(
    time: &str,
    projection: SkyProjection,
    viewpoint: SkyViewpoint,
    external_viewpoint: ExternalViewpoint,
    fov_deg: f64,
    catalog: CatalogSnapshot,
) -> Result<SessionScene> {
    Ok(SessionScene {
        latitude_deg: 35.68,
        longitude_deg: 139.69,
        time: parse_time_to_time_scales(Some(time))?,
        view: local_view(0.0, 0.0, fov_deg),
        overlays: OverlayConfig {
            layers: Vec::new(),
            grid_step_deg: 15.0,
            opacity: 0.0,
            deep_sky_magnitude_limit: OverlayConfig::default().deep_sky_magnitude_limit,
        },
        atmosphere_preset: AtmospherePreset::ClearRural,
        atmosphere: Atmosphere::OFF,
        light_pollution: LightPollution::default(),
        scintillation: Scintillation::OFF,
        planets_enabled: false,
        satellites: SatelliteLayer::default(),
        projection,
        viewpoint,
        external_viewpoint,
        eyepiece: EyepieceSimulation::OFF,
        catalog,
        corrections: CorrectionSnapshot::for_scene(Atmosphere::OFF),
        output_colourspace: OutputColourSpace::default(),
        meteors: MeteorLayer::default(),
        aurora: renderer::AuroraLayer::default(),
    })
}

fn local_view(azimuth_deg: f64, altitude_deg: f64, fov_deg: f64) -> LocalView {
    LocalView {
        azimuth_rad: (azimuth_deg as f32).to_radians(),
        altitude_rad: (altitude_deg as f32).to_radians(),
        fov_y_rad: (fov_deg as f32).to_radians(),
    }
}

fn overlay_config(layers: &[OverlayKind]) -> OverlayConfig {
    OverlayConfig {
        layers: layers.to_vec(),
        grid_step_deg: 15.0,
        opacity: 0.6,
        deep_sky_magnitude_limit: OverlayConfig::default().deep_sky_magnitude_limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashSet, fs};

    #[test]
    fn all_preset_ids_are_documented_and_unique() {
        let mut ids = HashSet::new();
        for info in scene_preset_infos() {
            assert!(ScenePresetArg::ALL.contains(&info.id));
            assert!(ids.insert(info.id.as_kebab_str()), "duplicate preset id");
            assert!(!info.title.is_empty());
            assert!(!info.validation_focus.is_empty());
        }
        assert_eq!(ids.len(), ScenePresetArg::ALL.len());
    }

    #[test]
    fn presets_build_valid_round_trippable_sessions() {
        for preset in ScenePresetArg::ALL {
            let session = session_from_preset(
                *preset,
                "0.1.0",
                "preset-test",
                "crates/catalog/data/hyg_v42.csv",
                renderer::DEFAULT_SCREEN_LIMITING_MAGNITUDE,
            )
            .unwrap_or_else(|error| panic!("{preset} failed: {error:#}"));
            let json = serde_json::to_string(&session).unwrap();
            assert!(json.contains("\"schemaVersion\":7"));
            let parsed: StarSession = serde_json::from_str(&json).unwrap();
            let restored = parsed
                .to_scene()
                .unwrap_or_else(|error| panic!("{preset} did not restore: {error:#}"));
            assert!(restored.time.jd_utc.is_finite());
            assert!(restored.catalog.limiting_magnitude.is_finite());
        }
    }

    #[test]
    fn exported_preset_session_files_are_valid() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/presets/sessions");
        let mut filenames = HashSet::new();
        for entry in fs::read_dir(&dir).unwrap_or_else(|error| {
            panic!(
                "failed to read exported preset sessions at {}: {error}",
                dir.display()
            )
        }) {
            let path = entry.unwrap().path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let raw = fs::read_to_string(&path).unwrap();
            let session: StarSession = serde_json::from_str(&raw)
                .unwrap_or_else(|error| panic!("{} did not parse: {error}", path.display()));
            session
                .to_scene()
                .unwrap_or_else(|error| panic!("{} did not restore: {error:#}", path.display()));
            filenames.insert(path.file_stem().unwrap().to_string_lossy().into_owned());
        }

        let expected = ScenePresetArg::ALL
            .iter()
            .map(|preset| preset.as_kebab_str().to_string())
            .collect::<HashSet<_>>();
        assert_eq!(filenames, expected);
    }
}
