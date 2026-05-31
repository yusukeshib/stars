//! L-23 Guided education mode: cross-host tour content.
//!
//! A [`Tour`] is an ordered list of [`TourStep`]s. Each step pairs a
//! human-readable caption (and an optional reference URL motivating the
//! concept) with a [`TourScene`] — a small, host-agnostic, *fully declarative*
//! scene descriptor. The renderer itself knows nothing about tours: native
//! hosts turn a [`TourScene`] into the same [`SessionScene`] every other scene
//! path produces (via [`TourScene::to_session_scene`]), and the web host maps
//! the identical JSON onto its existing React state setters. Because every
//! step pins a fixed observer time string, each step reduces to a
//! deterministic render with no wall-clock dependence.
//!
//! The built-in [`first_night_tour`] walks a newcomer through the seven
//! reference structures the overlay library (`V-01`/`V-02`/`V-08`) can draw:
//! the local horizon and cardinal points, the celestial equator and the
//! sky's daily spin, the ecliptic, the Milky Way / galactic plane, twilight,
//! and whole-sky projections.
//!
//! Scientific framing per step references the literature that motivates the
//! overlay (obliquity of the ecliptic, the IAU 1958 galactic pole, solar
//! depression twilight bands, equal-area map projections).

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::presets::{earth_scene, overlay_config};
use crate::{hyg_catalog_snapshot, AtmospherePresetArg, OverlayArg, ProjectionArg, SessionScene};
use renderer::{AtmospherePreset, OverlayKind, SkyProjection};

/// A host-agnostic, fully declarative scene for one tour step.
///
/// Every field is a primitive or a `crate`-level arg enum so the same struct
/// round-trips through JSON, drives the native [`SessionScene`] path, and maps
/// onto the web host's React state without the renderer needing to know about
/// tours. `time` is a fixed ISO-8601 UTC instant so each step renders
/// deterministically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TourScene {
    pub latitude_deg: f64,
    pub longitude_deg: f64,
    /// Fixed observer instant (ISO-8601 UTC). Pinned, never "now", so the
    /// step is reproducible.
    pub time: String,
    pub azimuth_deg: f64,
    pub altitude_deg: f64,
    pub fov_deg: f64,
    pub overlays: Vec<OverlayArg>,
    pub projection: ProjectionArg,
    pub atmosphere_preset: AtmospherePresetArg,
    pub planets_enabled: bool,
}

impl TourScene {
    /// Build the engine-ready [`SessionScene`] for this step, reusing the same
    /// `earth_scene` construction the deterministic presets use so a tour step
    /// and an equivalent preset render identically.
    pub fn to_session_scene(
        &self,
        catalog_path: impl AsRef<Path>,
        limiting_magnitude: f32,
    ) -> Result<SessionScene> {
        let catalog = hyg_catalog_snapshot(catalog_path.as_ref(), limiting_magnitude);
        let layers: Vec<OverlayKind> = self
            .overlays
            .iter()
            .copied()
            .map(OverlayKind::from)
            .collect();
        let mut scene = earth_scene(
            self.latitude_deg,
            self.longitude_deg,
            &self.time,
            self.azimuth_deg,
            self.altitude_deg,
            self.fov_deg,
            overlay_config(&layers),
            AtmospherePreset::from(self.atmosphere_preset),
            catalog,
        )?;
        scene.projection = SkyProjection::from(self.projection);
        scene.planets_enabled = self.planets_enabled;
        Ok(scene)
    }
}

/// One step of a [`Tour`]: a caption, an optional reference URL, and the
/// declarative scene the host applies while the caption is shown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TourStep {
    /// Stable kebab-case identifier for the step (used by `--tour-step`).
    pub id: String,
    pub title: String,
    pub caption: String,
    /// Optional citation / further-reading URL shown next to the caption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_url: Option<String>,
    pub scene: TourScene,
}

/// An ordered, named guided tour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tour {
    pub id: String,
    pub title: String,
    pub description: String,
    pub steps: Vec<TourStep>,
}

impl Tour {
    /// Serialize the tour to pretty JSON (the canonical cross-host content
    /// form consumed by the web frontend).
    pub fn to_json(&self) -> String {
        // The struct is plain data, so serialization cannot fail.
        serde_json::to_string_pretty(self).expect("tour serializes to JSON")
    }
}

// Tokyo and Mauna Kea anchor sites, matching the deterministic presets so the
// tour reuses already-validated framing.
const TOKYO_LAT: f64 = 35.68;
const TOKYO_LNG: f64 = 139.69;
const MAUNA_KEA_LAT: f64 = 19.8207;
const MAUNA_KEA_LNG: f64 = -155.4681;

/// The built-in "first night" tour: a guided walk through the reference
/// structures of the sky for a newcomer, driven entirely by deterministic
/// declarative scenes.
pub fn first_night_tour() -> Tour {
    Tour {
        id: "first-night".to_string(),
        title: "Your first night under the stars".to_string(),
        description: "A guided walk through the reference lines astronomers use \
            to find their way around the sky: the horizon, the celestial \
            equator, the ecliptic, the Milky Way, twilight, and how the whole \
            sky is mapped flat."
            .to_string(),
        steps: vec![
            TourStep {
                id: "horizon".to_string(),
                title: "Your local horizon".to_string(),
                caption: "Everything starts from where you stand. The horizon \
                    ring and the four cardinal points (N, E, S, W) define the \
                    alt-azimuth frame: altitude is the angle above the horizon, \
                    azimuth is the compass bearing. Every other coordinate \
                    system is laid on top of this one."
                    .to_string(),
                reference_url: Some(
                    "https://en.wikipedia.org/wiki/Horizontal_coordinate_system".to_string(),
                ),
                scene: TourScene {
                    latitude_deg: TOKYO_LAT,
                    longitude_deg: TOKYO_LNG,
                    time: "2026-08-13T12:00:00Z".to_string(),
                    azimuth_deg: 180.0,
                    altitude_deg: 20.0,
                    fov_deg: 90.0,
                    overlays: vec![
                        OverlayArg::Horizon,
                        OverlayArg::Cardinals,
                        OverlayArg::CardinalLabels,
                        OverlayArg::AltAzGrid,
                    ],
                    projection: ProjectionArg::Perspective,
                    atmosphere_preset: AtmospherePresetArg::ClearRural,
                    planets_enabled: true,
                },
            },
            TourStep {
                id: "celestial-equator".to_string(),
                title: "The celestial equator and the sky's daily spin".to_string(),
                caption: "Earth's rotation makes the whole sky appear to turn \
                    once a day around the celestial poles. The celestial \
                    equator is the projection of Earth's equator onto the sky; \
                    the equatorial grid (right ascension and declination) is \
                    the sky's own latitude/longitude, fixed to the stars rather \
                    than the horizon."
                    .to_string(),
                reference_url: Some(
                    "https://en.wikipedia.org/wiki/Equatorial_coordinate_system".to_string(),
                ),
                scene: TourScene {
                    latitude_deg: TOKYO_LAT,
                    longitude_deg: TOKYO_LNG,
                    time: "2026-08-13T12:00:00Z".to_string(),
                    azimuth_deg: 180.0,
                    altitude_deg: 35.0,
                    fov_deg: 90.0,
                    overlays: vec![
                        OverlayArg::Horizon,
                        OverlayArg::EquatorialGrid,
                        OverlayArg::CelestialEquator,
                        OverlayArg::Meridian,
                    ],
                    projection: ProjectionArg::Perspective,
                    atmosphere_preset: AtmospherePresetArg::ClearRural,
                    planets_enabled: true,
                },
            },
            TourStep {
                id: "ecliptic".to_string(),
                title: "The ecliptic: the road of the Sun, Moon and planets".to_string(),
                caption: "The ecliptic is the plane of Earth's orbit projected \
                    onto the sky — the Sun's yearly path, tilted 23.44° (the \
                    obliquity) to the celestial equator. The Moon and planets \
                    never stray far from it, so it is also the line to scan for \
                    a conjunction or an eclipse."
                    .to_string(),
                reference_url: Some("https://en.wikipedia.org/wiki/Ecliptic".to_string()),
                scene: TourScene {
                    latitude_deg: TOKYO_LAT,
                    longitude_deg: TOKYO_LNG,
                    time: "2026-08-13T12:00:00Z".to_string(),
                    azimuth_deg: 200.0,
                    altitude_deg: 35.0,
                    fov_deg: 90.0,
                    overlays: vec![
                        OverlayArg::Horizon,
                        OverlayArg::Ecliptic,
                        OverlayArg::PlanetLabels,
                    ],
                    projection: ProjectionArg::Perspective,
                    atmosphere_preset: AtmospherePresetArg::ClearRural,
                    planets_enabled: true,
                },
            },
            TourStep {
                id: "milky-way".to_string(),
                title: "The Milky Way and the galactic plane".to_string(),
                caption: "Our galaxy is a flat disc, and from inside it the \
                    combined light of its stars forms the Milky Way band. The \
                    galactic equator (IAU 1958 galactic pole) traces the mid-\
                    plane of that disc across the sky. Under a dark high-\
                    altitude sky the band stands out from the diffuse \
                    background."
                    .to_string(),
                reference_url: Some(
                    "https://en.wikipedia.org/wiki/Galactic_coordinate_system".to_string(),
                ),
                scene: TourScene {
                    latitude_deg: MAUNA_KEA_LAT,
                    longitude_deg: MAUNA_KEA_LNG,
                    time: "2026-07-18T10:30:00Z".to_string(),
                    azimuth_deg: 155.0,
                    altitude_deg: 55.0,
                    fov_deg: 95.0,
                    overlays: vec![OverlayArg::GalacticEquator, OverlayArg::ConstellationLabels],
                    projection: ProjectionArg::Perspective,
                    atmosphere_preset: AtmospherePresetArg::HighAltitude,
                    planets_enabled: true,
                },
            },
            TourStep {
                id: "twilight".to_string(),
                title: "How night falls: twilight".to_string(),
                caption: "Night does not arrive all at once. As the Sun sinks \
                    below the horizon the sky darkens through civil (Sun 0–6° \
                    down), nautical (6–12°) and astronomical (12–18°) twilight; \
                    only after astronomical twilight is the sky truly dark. \
                    Watch the western horizon just after sunset."
                    .to_string(),
                reference_url: Some("https://en.wikipedia.org/wiki/Twilight".to_string()),
                scene: TourScene {
                    latitude_deg: TOKYO_LAT,
                    longitude_deg: TOKYO_LNG,
                    time: "2026-08-13T09:45:00Z".to_string(),
                    azimuth_deg: 285.0,
                    altitude_deg: 8.0,
                    fov_deg: 95.0,
                    overlays: vec![OverlayArg::Horizon, OverlayArg::CardinalLabels],
                    projection: ProjectionArg::Perspective,
                    atmosphere_preset: AtmospherePresetArg::ClearRural,
                    planets_enabled: true,
                },
            },
            TourStep {
                id: "projections".to_string(),
                title: "Mapping the whole sky".to_string(),
                caption: "A perspective view shows only the patch you face. To \
                    study the whole sky at once we project the celestial sphere \
                    onto a flat map. The Mollweide projection is equal-area, so \
                    it preserves the relative sizes of constellations and the \
                    sweep of the Milky Way — at the cost of bending straight \
                    lines near the edges."
                    .to_string(),
                reference_url: Some(
                    "https://en.wikipedia.org/wiki/Mollweide_projection".to_string(),
                ),
                scene: TourScene {
                    latitude_deg: TOKYO_LAT,
                    longitude_deg: TOKYO_LNG,
                    time: "2026-08-13T12:00:00Z".to_string(),
                    azimuth_deg: 180.0,
                    altitude_deg: 35.0,
                    fov_deg: 90.0,
                    overlays: vec![
                        OverlayArg::EquatorialGrid,
                        OverlayArg::Ecliptic,
                        OverlayArg::GalacticEquator,
                        OverlayArg::ConstellationLines,
                    ],
                    projection: ProjectionArg::Mollweide,
                    atmosphere_preset: AtmospherePresetArg::ClearRural,
                    planets_enabled: true,
                },
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_night_tour_has_expected_shape() {
        let tour = first_night_tour();
        assert_eq!(tour.id, "first-night");
        assert_eq!(tour.steps.len(), 6);
        // Every step must carry a non-empty caption and a unique id.
        let mut ids = std::collections::HashSet::new();
        for step in &tour.steps {
            assert!(
                !step.caption.trim().is_empty(),
                "{} has no caption",
                step.id
            );
            assert!(ids.insert(step.id.clone()), "duplicate step id {}", step.id);
        }
        // The pedagogical spine: these reference structures must be covered.
        assert!(ids.contains("horizon"));
        assert!(ids.contains("celestial-equator"));
        assert!(ids.contains("ecliptic"));
        assert!(ids.contains("milky-way"));
        assert!(ids.contains("twilight"));
        assert!(ids.contains("projections"));
    }

    #[test]
    fn tour_round_trips_through_json() {
        let tour = first_night_tour();
        let json = tour.to_json();
        let parsed: Tour = serde_json::from_str(&json).expect("tour parses back");
        assert_eq!(tour, parsed);
    }

    #[test]
    fn every_step_is_deterministic_and_renderable() {
        // "Deterministic" here means: no wall-clock dependence. Each step pins
        // a fixed ISO-8601 instant that parses to a concrete TimeScales, so two
        // builds of the same step produce the same observer time.
        for step in first_night_tour().steps {
            let t = crate::parse_time_to_time_scales(Some(&step.scene.time))
                .unwrap_or_else(|e| panic!("step {} time {:?}: {e}", step.id, step.scene.time));
            assert!(t.jd_utc.is_finite());
            // Field sanity that the host wiring relies on.
            assert!(step.scene.fov_deg > 0.0 && step.scene.fov_deg <= 180.0);
            assert!((-90.0..=90.0).contains(&step.scene.latitude_deg));
        }
    }
}
