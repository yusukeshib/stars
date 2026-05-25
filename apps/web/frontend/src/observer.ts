export type Observer = {
  latitudeDeg: number;
  longitudeDeg: number;
};

export type View = {
  azimuthDeg: number;
  altitudeDeg: number;
  fovDeg: number;
};

/// All overlay layer names recognized by the WASM bindings. Kept in sync with
/// `OverlayKind` in `crates/renderer/src/overlay.rs` and the CLI `--overlays`
/// flag in `apps/cli`.
export const OVERLAY_LAYERS = [
  "horizon",
  "cardinals",
  "alt-az-grid",
  "equatorial-grid",
  "ecliptic",
  "celestial-equator",
  "meridian",
  "galactic-equator",
  "constellation-lines",
  "constellation-boundaries",
  "star-labels",
  "planet-labels",
  "constellation-labels",
  "cardinal-labels",
  "degree-labels",
] as const;
export type OverlayLayer = (typeof OVERLAY_LAYERS)[number];

export type OverlayConfig = {
  layers: OverlayLayer[];
  gridStepDeg: number;
  opacity: number;
};

export const DEFAULT_OVERLAY_CONFIG: OverlayConfig = {
  layers: ["horizon", "cardinal-labels"],
  gridStepDeg: 15,
  opacity: 0.6,
};

export const ATMOSPHERE_PRESETS = ["clear-rural", "hazy-urban", "high-altitude"] as const;
export type AtmospherePreset = (typeof ATMOSPHERE_PRESETS)[number];

export type AtmosphereConfig = {
  enabled: boolean;
  preset: AtmospherePreset;
  turbidity: number;
  observerAltitudeM: number;
  ozoneDu: number;
  visibilityKm: number;
  pressureHpa: number;
  temperatureC: number;
};

export type PlanetsConfig = {
  enabled: boolean;
};

export const SKY_PROJECTIONS = ["perspective", "mollweide", "aitoff", "hammer"] as const;
export type SkyProjection = (typeof SKY_PROJECTIONS)[number];

export type ProjectionConfig = {
  projection: SkyProjection;
};

export const DEFAULT_PROJECTION_CONFIG: ProjectionConfig = {
  projection: "perspective",
};

export const DEFAULT_PLANETS_CONFIG: PlanetsConfig = {
  enabled: true,
};

export type PlanningRow = {
  name: string;
  riseMs: number | null;
  transitMs: number | null;
  setMs: number | null;
  transitAltitudeDeg: number | null;
};

export type TwilightSegment = {
  label: string;
  startMs: number;
  endMs: number;
};

export type PlanningTable = {
  startMs: number;
  endMs: number;
  rows: PlanningRow[];
  twilight: TwilightSegment[];
};

export const DEFAULT_ATMOSPHERE_CONFIG: AtmosphereConfig = {
  enabled: true,
  preset: "clear-rural",
  turbidity: 2.5,
  observerAltitudeM: 0,
  ozoneDu: 300,
  visibilityKm: 50,
  pressureHpa: 1010,
  temperatureC: 10,
};

export const ATMOSPHERE_PRESET_DEFAULTS: Record<AtmospherePreset, Pick<AtmosphereConfig, "turbidity" | "observerAltitudeM" | "ozoneDu" | "visibilityKm" | "pressureHpa" | "temperatureC">> = {
  "clear-rural": { turbidity: 2.5, observerAltitudeM: 0, ozoneDu: 300, visibilityKm: 50, pressureHpa: 1010, temperatureC: 10 },
  "hazy-urban": { turbidity: 5.0, observerAltitudeM: 0, ozoneDu: 325, visibilityKm: 12, pressureHpa: 1010, temperatureC: 15 },
  "high-altitude": { turbidity: 2.0, observerAltitudeM: 2500, ozoneDu: 275, visibilityKm: 80, pressureHpa: 750, temperatureC: 0 },
};

export const ATMOSPHERE_PRESET_LABELS: Record<AtmospherePreset, string> = {
  "clear-rural": "Clear rural",
  "hazy-urban": "Hazy urban",
  "high-altitude": "High altitude",
};

export const isAtmospherePreset = (s: unknown): s is AtmospherePreset =>
  typeof s === "string" && (ATMOSPHERE_PRESETS as readonly string[]).includes(s);

export const isSkyProjection = (s: unknown): s is SkyProjection =>
  typeof s === "string" && (SKY_PROJECTIONS as readonly string[]).includes(s);

export const SKY_PROJECTION_LABELS: Record<SkyProjection, string> = {
  perspective: "Perspective",
  mollweide: "Mollweide (full sky)",
  aitoff: "Aitoff (full sky)",
  hammer: "Hammer (full sky)",
};

/// Human-readable labels for the UI; order also drives display order.
export const OVERLAY_LABELS: Record<OverlayLayer, string> = {
  horizon: "Horizon",
  cardinals: "Cardinal marks (N/E/S/W)",
  "alt-az-grid": "Alt-Az grid (observer)",
  "equatorial-grid": "Equatorial grid (J2000)",
  ecliptic: "Ecliptic",
  "celestial-equator": "Celestial equator",
  meridian: "Local meridian",
  "galactic-equator": "Galactic equator",
  "constellation-lines": "Constellation lines",
  "constellation-boundaries": "Constellation boundaries (IAU)",
  "star-labels": "Bright star labels",
  "planet-labels": "Sun/Moon/planet labels",
  "constellation-labels": "Constellation names",
  "cardinal-labels": "Cardinal labels (N/E/S/W)",
  "degree-labels": "Degree labels",
};

export const isOverlayLayer = (s: unknown): s is OverlayLayer =>
  typeof s === "string" && (OVERLAY_LAYERS as readonly string[]).includes(s);

export const MIN_ALTITUDE_DEG = -89.5;
export const MAX_ALTITUDE_DEG = 89.5;
export const MIN_FOV_DEG = 5;
export const MAX_FOV_DEG = 120;

export const clampAltitude = (deg: number): number =>
  Math.max(MIN_ALTITUDE_DEG, Math.min(MAX_ALTITUDE_DEG, deg));

export const wrapAzimuth = (deg: number): number => {
  const v = deg % 360;
  return v < 0 ? v + 360 : v;
};

export const clampFov = (deg: number): number =>
  Math.max(MIN_FOV_DEG, Math.min(MAX_FOV_DEG, deg));

const DEG = Math.PI / 180;
export const toRad = (d: number) => d * DEG;
