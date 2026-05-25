import {
  MAX_FOV_DEG,
  MIN_FOV_DEG,
  isAtmospherePreset,
  isOverlayLayer,
  DEFAULT_ATMOSPHERE_CONFIG,
  type AtmosphereConfig,
  type Observer,
  type OverlayConfig,
  type PlanetsConfig,
  type View,
} from "./observer";

const STORAGE_KEY = "stars:config";
const CURRENT_VERSION = 1;

/// Schema for everything that survives a page reload. Bump `version` when the
/// shape changes in an incompatible way; `loadConfig` returns `null` for any
/// version it doesn't recognize so callers fall back to defaults cleanly.
///
/// We've kept `version: 1` while adding new optional fields so older saves
/// (lat/lng + view only) still hydrate. Individual fields go through their
/// own type guard, so a corrupt entry only drops that one field rather than
/// the whole config.
export type PersistedConfig = {
  version: 1;
  observer: Observer;
  view: View;
  overlays?: OverlayConfig;
  atmosphere?: AtmosphereConfig;
  planets?: PlanetsConfig;
};

export type PartialPersistedConfig = Partial<Omit<PersistedConfig, "version">>;

/// Best-effort load. Returns `null` if nothing is stored, the JSON is malformed,
/// or the version is unknown. Individual fields are validated so a broken entry
/// can never crash the app.
export function loadConfig(): PartialPersistedConfig | null {
  if (typeof localStorage === "undefined") return null;
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object") return null;
    const obj = parsed as {
      version?: unknown;
      observer?: unknown;
      view?: unknown;
      overlays?: unknown;
      atmosphere?: unknown;
      planets?: unknown;
    };
    if (obj.version !== CURRENT_VERSION) return null;
    const out: PartialPersistedConfig = {};
    if (isObserver(obj.observer)) out.observer = obj.observer;
    if (isView(obj.view)) out.view = obj.view;
    if (isOverlayConfig(obj.overlays)) out.overlays = obj.overlays;
    const atmosphere = parseAtmosphereConfig(obj.atmosphere);
    if (atmosphere) out.atmosphere = atmosphere;
    const planets = parsePlanetsConfig(obj.planets);
    if (planets) out.planets = planets;
    return out;
  } catch {
    return null;
  }
}

/// Fire-and-forget save. localStorage exceptions (quota, private mode) are
/// swallowed; persistence is a nicety, not a correctness requirement.
export function saveConfig(config: Omit<PersistedConfig, "version">): void {
  if (typeof localStorage === "undefined") return;
  try {
    const payload: PersistedConfig = { version: CURRENT_VERSION, ...config };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
  } catch {
    // Ignore: storage may be full, disabled, or unavailable in private mode.
  }
}

// Range gates protect against tampered / out-of-domain localStorage entries.
// Values outside these ranges can't represent a real observer or view, so we
// reject the whole field rather than silently clamping.
const LAT_RANGE: [number, number] = [-90, 90];
const LNG_RANGE: [number, number] = [-180, 180];
const AZ_RANGE: [number, number] = [0, 360];
const ALT_RANGE: [number, number] = [-90, 90];
const FOV_RANGE: [number, number] = [MIN_FOV_DEG, MAX_FOV_DEG];

const inRange = (n: unknown, [lo, hi]: [number, number]): n is number =>
  typeof n === "number" && Number.isFinite(n) && n >= lo && n <= hi;

function isObserver(v: unknown): v is Observer {
  if (!v || typeof v !== "object") return false;
  const o = v as Partial<Observer>;
  return inRange(o.latitudeDeg, LAT_RANGE) && inRange(o.longitudeDeg, LNG_RANGE);
}

function isView(v: unknown): v is View {
  if (!v || typeof v !== "object") return false;
  const o = v as Partial<View>;
  return (
    inRange(o.azimuthDeg, AZ_RANGE) &&
    inRange(o.altitudeDeg, ALT_RANGE) &&
    inRange(o.fovDeg, FOV_RANGE)
  );
}

const GRID_STEP_RANGE: [number, number] = [1, 90];
const OPACITY_RANGE: [number, number] = [0, 1];
const TURBIDITY_RANGE: [number, number] = [1.7, 10];
const OBSERVER_ALTITUDE_RANGE: [number, number] = [0, 9000];
const OZONE_RANGE: [number, number] = [0, 600];
const VISIBILITY_RANGE: [number, number] = [1, 200];
const PRESSURE_RANGE: [number, number] = [0, 1100];
const TEMPERATURE_RANGE: [number, number] = [-80, 60];

function isOverlayConfig(v: unknown): v is OverlayConfig {
  if (!v || typeof v !== "object") return false;
  const o = v as Partial<OverlayConfig>;
  return (
    Array.isArray(o.layers) &&
    o.layers.every(isOverlayLayer) &&
    inRange(o.gridStepDeg, GRID_STEP_RANGE) &&
    inRange(o.opacity, OPACITY_RANGE)
  );
}

function parsePlanetsConfig(v: unknown): PlanetsConfig | null {
  if (!v || typeof v !== "object") return null;
  const o = v as Partial<PlanetsConfig>;
  return typeof o.enabled === "boolean" ? { enabled: o.enabled } : null;
}

function parseAtmosphereConfig(v: unknown): AtmosphereConfig | null {
  if (!v || typeof v !== "object") return null;
  const o = v as Partial<AtmosphereConfig>;
  if (
    typeof o.enabled !== "boolean" ||
    !isAtmospherePreset(o.preset) ||
    !inRange(o.turbidity, TURBIDITY_RANGE) ||
    !inRange(o.observerAltitudeM, OBSERVER_ALTITUDE_RANGE) ||
    !inRange(o.ozoneDu, OZONE_RANGE) ||
    !inRange(o.visibilityKm, VISIBILITY_RANGE)
  ) {
    return null;
  }
  return {
    enabled: o.enabled,
    preset: o.preset,
    turbidity: o.turbidity,
    observerAltitudeM: o.observerAltitudeM,
    ozoneDu: o.ozoneDu,
    visibilityKm: o.visibilityKm,
    pressureHpa: inRange(o.pressureHpa, PRESSURE_RANGE)
      ? o.pressureHpa
      : DEFAULT_ATMOSPHERE_CONFIG.pressureHpa,
    temperatureC: inRange(o.temperatureC, TEMPERATURE_RANGE)
      ? o.temperatureC
      : DEFAULT_ATMOSPHERE_CONFIG.temperatureC,
  };
}
