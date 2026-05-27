import {
  MAX_FOV_DEG,
  MIN_FOV_DEG,
  isAtmospherePreset,
  isOverlayLayer,
  isSkyProjection,
  isSkyViewpoint,
  DEFAULT_ATMOSPHERE_CONFIG,
  DEFAULT_EYEPIECE_CONFIG,
  DEFAULT_PROJECTION_CONFIG,
  type AtmosphereConfig,
  type ExternalViewpointConfig,
  type Observer,
  type OverlayConfig,
  type PlanetsConfig,
  type ProjectionConfig,
  type View,
  type EyepieceConfig,
} from "./observer";

const STORAGE_KEY = "stars:config";

/// Schema for everything that survives a page reload. Individual fields go
/// through their own type guard, so a corrupt entry only drops that one field
/// rather than the whole config.
export type PersistedConfig = {
  observer: Observer;
  view: View;
  overlays?: OverlayConfig;
  atmosphere?: AtmosphereConfig;
  planets?: PlanetsConfig;
  projection?: ProjectionConfig;
  eyepiece?: EyepieceConfig;
};

export type PartialPersistedConfig = Partial<PersistedConfig>;

/// Best-effort load. Returns `null` if nothing is stored or the JSON is
/// malformed. Individual fields are validated so a broken entry can never crash
/// the app.
export function loadConfig(): PartialPersistedConfig | null {
  if (typeof localStorage === "undefined") return null;
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object") return null;
    const obj = parsed as {
      observer?: unknown;
      view?: unknown;
      overlays?: unknown;
      atmosphere?: unknown;
      planets?: unknown;
      projection?: unknown;
      eyepiece?: unknown;
    };
    const out: PartialPersistedConfig = {};
    if (isObserver(obj.observer)) out.observer = obj.observer;
    if (isView(obj.view)) out.view = obj.view;
    if (isOverlayConfig(obj.overlays)) out.overlays = obj.overlays;
    const atmosphere = parseAtmosphereConfig(obj.atmosphere);
    if (atmosphere) out.atmosphere = atmosphere;
    const planets = parsePlanetsConfig(obj.planets);
    if (planets) out.planets = planets;
    const projection = parseProjectionConfig(obj.projection);
    if (projection) out.projection = projection;
    const eyepiece = parseEyepieceConfig(obj.eyepiece);
    if (eyepiece) out.eyepiece = eyepiece;
    return out;
  } catch {
    return null;
  }
}

/// Fire-and-forget save. localStorage exceptions (quota, private mode) are
/// swallowed; persistence is a nicety, not a correctness requirement.
export function saveConfig(config: PersistedConfig): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
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
const DEEP_SKY_MAG_RANGE: [number, number] = [-5, 99];
const AEROSOL_BETA_RANGE: [number, number] = [0, 2];
const AEROSOL_ALPHA_RANGE: [number, number] = [0, 4];
const OBSERVER_ALTITUDE_RANGE: [number, number] = [0, 9000];
const OZONE_RANGE: [number, number] = [0, 600];
const PRESSURE_RANGE: [number, number] = [0, 1100];
const TEMPERATURE_RANGE: [number, number] = [-80, 60];
const EXTERNAL_PC_RANGE: [number, number] = [-1_000_000, 1_000_000];
const EXTERNAL_UP_RANGE: [number, number] = [-10, 10];
const TELESCOPE_APERTURE_RANGE: [number, number] = [10, 2000];
const TELESCOPE_FOCAL_RANGE: [number, number] = [50, 20000];
const EYEPIECE_FOCAL_RANGE: [number, number] = [1, 100];
const EYEPIECE_AFOV_RANGE: [number, number] = [1, 120];
const EYEPIECE_FIELD_STOP_RANGE: [number, number] = [0, 120];

function isOverlayConfig(v: unknown): v is OverlayConfig {
  if (!v || typeof v !== "object") return false;
  const o = v as Partial<OverlayConfig>;
  return (
    Array.isArray(o.layers) &&
    o.layers.every(isOverlayLayer) &&
    inRange(o.gridStepDeg, GRID_STEP_RANGE) &&
    inRange(o.opacity, OPACITY_RANGE) &&
    inRange(o.deepSkyMagnitudeLimit, DEEP_SKY_MAG_RANGE)
  );
}

function parsePlanetsConfig(v: unknown): PlanetsConfig | null {
  if (!v || typeof v !== "object") return null;
  const o = v as Partial<PlanetsConfig>;
  return typeof o.enabled === "boolean" ? { enabled: o.enabled } : null;
}

function parseVec3(v: unknown, range: [number, number]): { x: number; y: number; z: number } | null {
  if (!v || typeof v !== "object") return null;
  const o = v as { x?: unknown; y?: unknown; z?: unknown };
  return inRange(o.x, range) && inRange(o.y, range) && inRange(o.z, range)
    ? { x: o.x, y: o.y, z: o.z }
    : null;
}

function parseExternalViewpointConfig(v: unknown): ExternalViewpointConfig {
  if (!v || typeof v !== "object") return DEFAULT_PROJECTION_CONFIG.external;
  const o = v as Partial<ExternalViewpointConfig>;
  return {
    originPc: parseVec3(o.originPc, EXTERNAL_PC_RANGE) ?? DEFAULT_PROJECTION_CONFIG.external.originPc,
    targetPc: parseVec3(o.targetPc, EXTERNAL_PC_RANGE) ?? DEFAULT_PROJECTION_CONFIG.external.targetPc,
    up: parseVec3(o.up, EXTERNAL_UP_RANGE) ?? DEFAULT_PROJECTION_CONFIG.external.up,
  };
}

function parseProjectionConfig(v: unknown): ProjectionConfig | null {
  if (!v || typeof v !== "object") return null;
  const o = v as Partial<ProjectionConfig>;
  if (!isSkyProjection(o.projection)) return null;
  return {
    projection: o.projection,
    viewpoint: isSkyViewpoint(o.viewpoint) ? o.viewpoint : DEFAULT_PROJECTION_CONFIG.viewpoint,
    external: parseExternalViewpointConfig(o.external),
  };
}

function parseEyepieceConfig(v: unknown): EyepieceConfig | null {
  if (!v || typeof v !== "object") return null;
  const o = v as Partial<EyepieceConfig>;
  return {
    enabled: typeof o.enabled === "boolean" ? o.enabled : DEFAULT_EYEPIECE_CONFIG.enabled,
    apertureMm: inRange(o.apertureMm, TELESCOPE_APERTURE_RANGE)
      ? o.apertureMm
      : DEFAULT_EYEPIECE_CONFIG.apertureMm,
    focalLengthMm: inRange(o.focalLengthMm, TELESCOPE_FOCAL_RANGE)
      ? o.focalLengthMm
      : DEFAULT_EYEPIECE_CONFIG.focalLengthMm,
    eyepieceFocalLengthMm: inRange(o.eyepieceFocalLengthMm, EYEPIECE_FOCAL_RANGE)
      ? o.eyepieceFocalLengthMm
      : DEFAULT_EYEPIECE_CONFIG.eyepieceFocalLengthMm,
    apparentFovDeg: inRange(o.apparentFovDeg, EYEPIECE_AFOV_RANGE)
      ? o.apparentFovDeg
      : DEFAULT_EYEPIECE_CONFIG.apparentFovDeg,
    fieldStopMm: inRange(o.fieldStopMm, EYEPIECE_FIELD_STOP_RANGE)
      ? o.fieldStopMm
      : DEFAULT_EYEPIECE_CONFIG.fieldStopMm,
  };
}

function parseAtmosphereConfig(v: unknown): AtmosphereConfig | null {
  if (!v || typeof v !== "object") return null;
  const o = v as Partial<AtmosphereConfig>;
  if (
    typeof o.enabled !== "boolean" ||
    !isAtmospherePreset(o.preset) ||
    !inRange(o.aerosolBeta, AEROSOL_BETA_RANGE) ||
    !inRange(o.aerosolAlpha, AEROSOL_ALPHA_RANGE) ||
    !inRange(o.observerAltitudeM, OBSERVER_ALTITUDE_RANGE) ||
    !inRange(o.ozoneDu, OZONE_RANGE)
  ) {
    return null;
  }
  return {
    enabled: o.enabled,
    preset: o.preset,
    aerosolBeta: o.aerosolBeta,
    aerosolAlpha: o.aerosolAlpha,
    observerAltitudeM: o.observerAltitudeM,
    ozoneDu: o.ozoneDu,
    pressureHpa: inRange(o.pressureHpa, PRESSURE_RANGE)
      ? o.pressureHpa
      : DEFAULT_ATMOSPHERE_CONFIG.pressureHpa,
    temperatureC: inRange(o.temperatureC, TEMPERATURE_RANGE)
      ? o.temperatureC
      : DEFAULT_ATMOSPHERE_CONFIG.temperatureC,
    surfaceAlbedo: inRange(o.surfaceAlbedo, [0, 1])
      ? o.surfaceAlbedo
      : DEFAULT_ATMOSPHERE_CONFIG.surfaceAlbedo,
  };
}
