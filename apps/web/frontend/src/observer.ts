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
] as const;
export type OverlayLayer = (typeof OVERLAY_LAYERS)[number];

export type OverlayConfig = {
  layers: OverlayLayer[];
  gridStepDeg: number;
  opacity: number;
};

export const DEFAULT_OVERLAY_CONFIG: OverlayConfig = {
  layers: ["horizon", "cardinals"],
  gridStepDeg: 15,
  opacity: 0.6,
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
