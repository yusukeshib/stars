// L-23 Guided education mode — web mirror of the canonical "first night" tour.
//
// The authoritative tour content lives in Rust at
// `crates/common/src/tour.rs` (`first_night_tour`), which the CLI and viewer
// consume directly. The web renderer deliberately does NOT depend on
// `stars-host-common` (it would pull `clap`/`chrono` into the WASM bundle), so
// this file mirrors the same steps and captions and drives them through the
// existing React state setters. Keep the two in sync: captions, ids, and the
// scene values here must match `first_night_tour()`.

import type { AtmospherePreset, OverlayLayer, SkyProjection } from "./observer";

/** A fully declarative, deterministic scene for one tour step. */
export type TourScene = {
  latitudeDeg: number;
  longitudeDeg: number;
  /** Fixed ISO-8601 UTC instant — never "now" — so the step is reproducible. */
  timeIso: string;
  azimuthDeg: number;
  altitudeDeg: number;
  fovDeg: number;
  overlays: OverlayLayer[];
  projection: SkyProjection;
  atmospherePreset: AtmospherePreset;
  planetsEnabled: boolean;
};

export type TourStep = {
  id: string;
  title: string;
  caption: string;
  referenceUrl?: string;
  scene: TourScene;
};

export type Tour = {
  id: string;
  title: string;
  description: string;
  steps: TourStep[];
};

export const FIRST_NIGHT_TOUR: Tour = {
  id: "first-night",
  title: "Your first night under the stars",
  description:
    "A guided walk through the reference lines astronomers use to find their way around the sky: the horizon, the celestial equator, the ecliptic, the Milky Way, twilight, and how the whole sky is mapped flat.",
  steps: [
    {
      id: "horizon",
      title: "Your local horizon",
      caption:
        "Everything starts from where you stand. The horizon ring and the four cardinal points (N, E, S, W) define the alt-azimuth frame: altitude is the angle above the horizon, azimuth is the compass bearing. Every other coordinate system is laid on top of this one.",
      referenceUrl: "https://en.wikipedia.org/wiki/Horizontal_coordinate_system",
      scene: {
        latitudeDeg: 35.68,
        longitudeDeg: 139.69,
        timeIso: "2026-08-13T12:00:00Z",
        azimuthDeg: 180,
        altitudeDeg: 20,
        fovDeg: 90,
        overlays: ["horizon", "cardinals", "cardinal-labels", "alt-az-grid"],
        projection: "perspective",
        atmospherePreset: "clear-rural",
        planetsEnabled: true,
      },
    },
    {
      id: "celestial-equator",
      title: "The celestial equator and the sky's daily spin",
      caption:
        "Earth's rotation makes the whole sky appear to turn once a day around the celestial poles. The celestial equator is the projection of Earth's equator onto the sky; the equatorial grid (right ascension and declination) is the sky's own latitude/longitude, fixed to the stars rather than the horizon.",
      referenceUrl: "https://en.wikipedia.org/wiki/Equatorial_coordinate_system",
      scene: {
        latitudeDeg: 35.68,
        longitudeDeg: 139.69,
        timeIso: "2026-08-13T12:00:00Z",
        azimuthDeg: 180,
        altitudeDeg: 35,
        fovDeg: 90,
        overlays: ["horizon", "equatorial-grid", "celestial-equator", "meridian"],
        projection: "perspective",
        atmospherePreset: "clear-rural",
        planetsEnabled: true,
      },
    },
    {
      id: "ecliptic",
      title: "The ecliptic: the road of the Sun, Moon and planets",
      caption:
        "The ecliptic is the plane of Earth's orbit projected onto the sky — the Sun's yearly path, tilted 23.44° (the obliquity) to the celestial equator. The Moon and planets never stray far from it, so it is also the line to scan for a conjunction or an eclipse.",
      referenceUrl: "https://en.wikipedia.org/wiki/Ecliptic",
      scene: {
        latitudeDeg: 35.68,
        longitudeDeg: 139.69,
        timeIso: "2026-08-13T12:00:00Z",
        azimuthDeg: 200,
        altitudeDeg: 35,
        fovDeg: 90,
        overlays: ["horizon", "ecliptic", "planet-labels"],
        projection: "perspective",
        atmospherePreset: "clear-rural",
        planetsEnabled: true,
      },
    },
    {
      id: "milky-way",
      title: "The Milky Way and the galactic plane",
      caption:
        "Our galaxy is a flat disc, and from inside it the combined light of its stars forms the Milky Way band. The galactic equator (IAU 1958 galactic pole) traces the mid-plane of that disc across the sky. Under a dark high-altitude sky the band stands out from the diffuse background.",
      referenceUrl: "https://en.wikipedia.org/wiki/Galactic_coordinate_system",
      scene: {
        latitudeDeg: 19.8207,
        longitudeDeg: -155.4681,
        timeIso: "2026-07-18T10:30:00Z",
        azimuthDeg: 155,
        altitudeDeg: 55,
        fovDeg: 95,
        overlays: ["galactic-equator", "constellation-labels"],
        projection: "perspective",
        atmospherePreset: "high-altitude",
        planetsEnabled: true,
      },
    },
    {
      id: "twilight",
      title: "How night falls: twilight",
      caption:
        "Night does not arrive all at once. As the Sun sinks below the horizon the sky darkens through civil (Sun 0–6° down), nautical (6–12°) and astronomical (12–18°) twilight; only after astronomical twilight is the sky truly dark. Watch the western horizon just after sunset.",
      referenceUrl: "https://en.wikipedia.org/wiki/Twilight",
      scene: {
        latitudeDeg: 35.68,
        longitudeDeg: 139.69,
        timeIso: "2026-08-13T09:45:00Z",
        azimuthDeg: 285,
        altitudeDeg: 8,
        fovDeg: 95,
        overlays: ["horizon", "cardinal-labels"],
        projection: "perspective",
        atmospherePreset: "clear-rural",
        planetsEnabled: true,
      },
    },
    {
      id: "projections",
      title: "Mapping the whole sky",
      caption:
        "A perspective view shows only the patch you face. To study the whole sky at once we project the celestial sphere onto a flat map. The Mollweide projection is equal-area, so it preserves the relative sizes of constellations and the sweep of the Milky Way — at the cost of bending straight lines near the edges.",
      referenceUrl: "https://en.wikipedia.org/wiki/Mollweide_projection",
      scene: {
        latitudeDeg: 35.68,
        longitudeDeg: 139.69,
        timeIso: "2026-08-13T12:00:00Z",
        azimuthDeg: 180,
        altitudeDeg: 35,
        fovDeg: 90,
        overlays: ["equatorial-grid", "ecliptic", "galactic-equator", "constellation-lines"],
        projection: "mollweide",
        atmospherePreset: "clear-rural",
        planetsEnabled: true,
      },
    },
  ],
};
