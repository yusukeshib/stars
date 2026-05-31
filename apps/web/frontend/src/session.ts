import {
  DEFAULT_ATMOSPHERE_CONFIG,
  DEFAULT_EYEPIECE_CONFIG,
  DEFAULT_METEORS_CONFIG,
  DEFAULT_OVERLAY_CONFIG,
  DEFAULT_PLANETS_CONFIG,
  DEFAULT_PROJECTION_CONFIG,
  DEFAULT_OUTPUT_COLOURSPACE,
  DEFAULT_SATELLITES_CONFIG,
  DEFAULT_SCINTILLATION_CONFIG,
  MAX_FOV_DEG,
  MIN_FOV_DEG,
  isAtmospherePreset,
  isOutputColourspace,
  isOverlayLayer,
  isSkyProjection,
  isSkyViewpoint,
  sanitizedOpticalDesign,
  type AtmosphereConfig,
  type EyepieceConfig,
  type ExternalViewpointConfig,
  type MeteorsConfig,
  type Observer,
  type OutputColourspace,
  type OverlayConfig,
  type PlanetsConfig,
  type ProjectionConfig,
  type SatellitesConfig,
  type ScintillationConfig,
  type Vec3,
  type View,
} from "./observer";

// Must track `SESSION_SCHEMA_VERSION` in `crates/common/src/session.rs`.
// v2 unified spectral extinction (V-37); v3 added `surfaceAlbedo` for the
// Hošek-Wilkie daylight model (V-38); v4 added the `scintillation` block
// for V-24; v6 added the `outputColourspace` field for V-50 output colour
// management (matching the Rust host bump that also covered intermediate
// native-only schema changes). The Rust hosts and
// `docs/presets/sessions/*.json` all emit v6, so the web UI must accept and
// emit v6 too or cross-host session import/export is broken.
export const SESSION_SCHEMA_VERSION = 6;
const APP_VERSION = "0.1.0";
const UNIX_EPOCH_JD = 2440587.5;
const SECONDS_PER_DAY = 86400;
const TT_MINUS_TAI_SECONDS = 32.184;

export type SessionTime = {
  jdUtc: number;
  jdUt1: number;
  jdTai: number;
  jdTt: number;
  jdTdb: number;
  taiMinusUtcSeconds: number;
  dut1Seconds: number;
};

export type StarSession = {
  schemaVersion: number;
  appVersion: string;
  createdBy: string;
  observer: Observer;
  time: SessionTime;
  view: View;
  overlays: OverlayConfig;
  projection: ProjectionConfig;
  atmosphere: AtmosphereConfig;
  scintillation: ScintillationConfig;
  planets: PlanetsConfig;
  satellites: SatellitesConfig;
  meteors: MeteorsConfig;
  eyepiece: EyepieceConfig;
  outputColourspace: OutputColourspace;
  catalog: {
    backend: string;
    source: string;
    version: string | null;
    path: string | null;
    hash: string | null;
    limitingMagnitude: number;
  };
  corrections: {
    timeScales: boolean;
    precession: boolean;
    nutation: boolean;
    annualAberration: boolean;
    properMotion: boolean;
    atmosphericRefraction: boolean;
    topocentricSolarSystem: boolean;
  };
};

export type SessionState = {
  observer: Observer;
  view: View;
  overlays: OverlayConfig;
  atmosphere: AtmosphereConfig;
  scintillation: ScintillationConfig;
  planets: PlanetsConfig;
  satellites: SatellitesConfig;
  meteors: MeteorsConfig;
  projection: ProjectionConfig;
  eyepiece: EyepieceConfig;
  outputColourspace: OutputColourspace;
  timeMs: number;
};

const LEAP_SECONDS: Array<[number, number]> = [
  [2441317.5, 10], [2441499.5, 11], [2441683.5, 12], [2442048.5, 13],
  [2442413.5, 14], [2442778.5, 15], [2443144.5, 16], [2443509.5, 17],
  [2443874.5, 18], [2444239.5, 19], [2444786.5, 20], [2445151.5, 21],
  [2445516.5, 22], [2446247.5, 23], [2447161.5, 24], [2447892.5, 25],
  [2448257.5, 26], [2448804.5, 27], [2449169.5, 28], [2449534.5, 29],
  [2450083.5, 30], [2450630.5, 31], [2451179.5, 32], [2453736.5, 33],
  [2454832.5, 34], [2456109.5, 35], [2457204.5, 36], [2457754.5, 37],
];

const isFiniteNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);

const inRange = (value: unknown, min: number, max: number): value is number =>
  isFiniteNumber(value) && value >= min && value <= max;

const parseVec3 = (value: unknown, fallback: Vec3, min: number, max: number): Vec3 => {
  if (!value || typeof value !== "object") return fallback;
  const v = value as Partial<Vec3>;
  return inRange(v.x, min, max) && inRange(v.y, min, max) && inRange(v.z, min, max)
    ? { x: v.x, y: v.y, z: v.z }
    : fallback;
};

const taiMinusUtcAtJd = (jdUtc: number): number => {
  for (let i = LEAP_SECONDS.length - 1; i >= 0; i -= 1) {
    const [effectiveJd, offset] = LEAP_SECONDS[i];
    if (jdUtc >= effectiveJd) return offset;
  }
  return LEAP_SECONDS[0][1];
};

const approximateTdbFromTt = (jdTt: number): number => {
  const meanAnomalyRad = ((357.53 + 0.9856003 * (jdTt - 2451545.0)) * Math.PI) / 180;
  return jdTt + (0.001658 * Math.sin(meanAnomalyRad) + 0.000014 * Math.sin(2 * meanAnomalyRad)) / SECONDS_PER_DAY;
};

export const timeScalesFromUnixMs = (timeMs: number, dut1Seconds = 0): SessionTime => {
  const jdUtc = UNIX_EPOCH_JD + timeMs / 1000 / SECONDS_PER_DAY;
  const taiMinusUtcSeconds = taiMinusUtcAtJd(jdUtc);
  const jdUt1 = jdUtc + dut1Seconds / SECONDS_PER_DAY;
  const jdTai = jdUtc + taiMinusUtcSeconds / SECONDS_PER_DAY;
  const jdTt = jdTai + TT_MINUS_TAI_SECONDS / SECONDS_PER_DAY;
  return {
    jdUtc,
    jdUt1,
    jdTai,
    jdTt,
    jdTdb: approximateTdbFromTt(jdTt),
    taiMinusUtcSeconds,
    dut1Seconds,
  };
};

export const unixMsFromJdUtc = (jdUtc: number): number => (jdUtc - UNIX_EPOCH_JD) * SECONDS_PER_DAY * 1000;

export function buildStarSession(state: SessionState): StarSession {
  return {
    schemaVersion: SESSION_SCHEMA_VERSION,
    appVersion: APP_VERSION,
    createdBy: "stars-web",
    observer: state.observer,
    time: timeScalesFromUnixMs(state.timeMs),
    view: state.view,
    overlays: state.overlays,
    projection: state.projection,
    atmosphere: state.atmosphere,
    scintillation: state.scintillation,
    planets: state.planets,
    satellites: state.satellites,
    meteors: state.meteors,
    eyepiece: state.eyepiece,
    outputColourspace: state.outputColourspace,
    catalog: {
      backend: "hyg-embedded-wasm",
      source: "HYG",
      version: "4.2",
      path: "crates/catalog/data/hyg_v42.csv",
      hash: null,
      limitingMagnitude: 7.5,
    },
    corrections: {
      timeScales: true,
      precession: true,
      nutation: true,
      annualAberration: true,
      properMotion: true,
      atmosphericRefraction: state.atmosphere.enabled,
      topocentricSolarSystem: true,
    },
  };
}

export const starSessionJson = (state: SessionState): string =>
  `${JSON.stringify(buildStarSession(state), null, 2)}\n`;

export function parseStarSessionJson(raw: string): SessionState {
  const parsed = JSON.parse(raw) as unknown;
  if (!parsed || typeof parsed !== "object") throw new Error("Session JSON must be an object.");
  const s = parsed as Partial<StarSession>;
  if (s.schemaVersion !== SESSION_SCHEMA_VERSION) {
    throw new Error(`Unsupported session schemaVersion ${String(s.schemaVersion)}.`);
  }

  const observer = parseObserver(s.observer);
  const view = parseView(s.view);
  const overlays = parseOverlays(s.overlays);
  const atmosphere = parseAtmosphere(s.atmosphere);
  const scintillation = parseScintillation(s.scintillation);
  const planets = parsePlanets(s.planets);
  const satellites = parseSatellites(s.satellites);
  const meteors = parseMeteors(s.meteors);
  const projection = parseProjection(s.projection);
  const eyepiece = parseEyepiece(s.eyepiece);
  const outputColourspace = isOutputColourspace(s.outputColourspace)
    ? s.outputColourspace
    : DEFAULT_OUTPUT_COLOURSPACE;
  const timeMs = parseTimeMs(s.time);
  return {
    observer,
    view,
    overlays,
    atmosphere,
    scintillation,
    planets,
    satellites,
    meteors,
    projection,
    eyepiece,
    outputColourspace,
    timeMs,
  };
}

function parseObserver(value: unknown): Observer {
  if (!value || typeof value !== "object") throw new Error("Invalid observer.");
  const v = value as Partial<Observer>;
  if (!inRange(v.latitudeDeg, -90, 90) || !inRange(v.longitudeDeg, -180, 180)) {
    throw new Error("Invalid observer latitude/longitude.");
  }
  return { latitudeDeg: v.latitudeDeg, longitudeDeg: v.longitudeDeg };
}

function parseTimeMs(value: unknown): number {
  if (!value || typeof value !== "object") throw new Error("Invalid time scales.");
  const v = value as Partial<SessionTime>;
  if (!isFiniteNumber(v.jdUtc)) throw new Error("Invalid time.jdUtc.");
  return unixMsFromJdUtc(v.jdUtc);
}

function parseView(value: unknown): View {
  if (!value || typeof value !== "object") throw new Error("Invalid view.");
  const v = value as Partial<View>;
  if (!inRange(v.azimuthDeg, 0, 360) || !inRange(v.altitudeDeg, -90, 90) || !inRange(v.fovDeg, MIN_FOV_DEG, MAX_FOV_DEG)) {
    throw new Error("Invalid view azimuth/altitude/FOV.");
  }
  return { azimuthDeg: v.azimuthDeg, altitudeDeg: v.altitudeDeg, fovDeg: v.fovDeg };
}

function parseOverlays(value: unknown): OverlayConfig {
  if (!value || typeof value !== "object") return DEFAULT_OVERLAY_CONFIG;
  const v = value as Partial<OverlayConfig>;
  if (!Array.isArray(v.layers) || !v.layers.every(isOverlayLayer)) return DEFAULT_OVERLAY_CONFIG;
  return {
    layers: v.layers.filter((layer) => layer !== "cardinals"),
    gridStepDeg: inRange(v.gridStepDeg, 1, 90) ? v.gridStepDeg : DEFAULT_OVERLAY_CONFIG.gridStepDeg,
    opacity: inRange(v.opacity, 0, 1) ? v.opacity : DEFAULT_OVERLAY_CONFIG.opacity,
    deepSkyMagnitudeLimit: inRange(v.deepSkyMagnitudeLimit, -5, 99)
      ? v.deepSkyMagnitudeLimit
      : DEFAULT_OVERLAY_CONFIG.deepSkyMagnitudeLimit,
  };
}

function parseAtmosphere(value: unknown): AtmosphereConfig {
  if (!value || typeof value !== "object") return DEFAULT_ATMOSPHERE_CONFIG;
  const v = value as Partial<AtmosphereConfig>;
  if (typeof v.enabled !== "boolean" || !isAtmospherePreset(v.preset)) return DEFAULT_ATMOSPHERE_CONFIG;
  return {
    enabled: v.enabled,
    preset: v.preset,
    aerosolBeta: inRange(v.aerosolBeta, 0, 2) ? v.aerosolBeta : DEFAULT_ATMOSPHERE_CONFIG.aerosolBeta,
    aerosolAlpha: inRange(v.aerosolAlpha, 0, 4) ? v.aerosolAlpha : DEFAULT_ATMOSPHERE_CONFIG.aerosolAlpha,
    observerAltitudeM: inRange(v.observerAltitudeM, 0, 9000) ? v.observerAltitudeM : DEFAULT_ATMOSPHERE_CONFIG.observerAltitudeM,
    ozoneDu: inRange(v.ozoneDu, 0, 600) ? v.ozoneDu : DEFAULT_ATMOSPHERE_CONFIG.ozoneDu,
    pressureHpa: inRange(v.pressureHpa, 0, 1100) ? v.pressureHpa : DEFAULT_ATMOSPHERE_CONFIG.pressureHpa,
    temperatureC: inRange(v.temperatureC, -80, 60) ? v.temperatureC : DEFAULT_ATMOSPHERE_CONFIG.temperatureC,
    surfaceAlbedo: inRange(v.surfaceAlbedo, 0, 1) ? v.surfaceAlbedo : DEFAULT_ATMOSPHERE_CONFIG.surfaceAlbedo,
  };
}

function parseScintillation(value: unknown): ScintillationConfig {
  if (!value || typeof value !== "object") return DEFAULT_SCINTILLATION_CONFIG;
  const v = value as Partial<ScintillationConfig>;
  return {
    enabled: typeof v.enabled === "boolean" ? v.enabled : DEFAULT_SCINTILLATION_CONFIG.enabled,
    cN2Scale: inRange(v.cN2Scale, 0, 10) ? v.cN2Scale : DEFAULT_SCINTILLATION_CONFIG.cN2Scale,
    seed: Number.isFinite(v.seed) ? (v.seed as number) >>> 0 : DEFAULT_SCINTILLATION_CONFIG.seed,
  };
}

function parseSatellites(value: unknown): SatellitesConfig {
  if (!value || typeof value !== "object") return DEFAULT_SATELLITES_CONFIG;
  const v = value as Partial<SatellitesConfig>;
  return {
    enabled: typeof v.enabled === "boolean" ? v.enabled : DEFAULT_SATELLITES_CONFIG.enabled,
    exposureSeconds:
      typeof v.exposureSeconds === "number" && Number.isFinite(v.exposureSeconds) && v.exposureSeconds >= 0
        ? v.exposureSeconds
        : DEFAULT_SATELLITES_CONFIG.exposureSeconds,
  };
}

function parseMeteors(value: unknown): MeteorsConfig {
  if (!value || typeof value !== "object") return DEFAULT_METEORS_CONFIG;
  const v = value as Partial<MeteorsConfig>;
  const num = (x: unknown, min: number, fallback: number): number =>
    typeof x === "number" && Number.isFinite(x) && x >= min ? x : fallback;
  return {
    enabled: typeof v.enabled === "boolean" ? v.enabled : DEFAULT_METEORS_CONFIG.enabled,
    seed: num(v.seed, 0, DEFAULT_METEORS_CONFIG.seed),
    rateScale: num(v.rateScale, 0, DEFAULT_METEORS_CONFIG.rateScale),
    windowSeconds: num(v.windowSeconds, 0, DEFAULT_METEORS_CONFIG.windowSeconds),
  };
}

function parsePlanets(value: unknown): PlanetsConfig {
  if (!value || typeof value !== "object") return DEFAULT_PLANETS_CONFIG;
  const v = value as Partial<PlanetsConfig>;
  return typeof v.enabled === "boolean" ? { enabled: v.enabled } : DEFAULT_PLANETS_CONFIG;
}

function parseProjection(value: unknown): ProjectionConfig {
  if (!value || typeof value !== "object") return DEFAULT_PROJECTION_CONFIG;
  const v = value as Partial<ProjectionConfig>;
  return {
    projection: isSkyProjection(v.projection) ? v.projection : DEFAULT_PROJECTION_CONFIG.projection,
    viewpoint: isSkyViewpoint(v.viewpoint) ? v.viewpoint : DEFAULT_PROJECTION_CONFIG.viewpoint,
    external: parseExternal(v.external),
  };
}

function parseExternal(value: unknown): ExternalViewpointConfig {
  if (!value || typeof value !== "object") return DEFAULT_PROJECTION_CONFIG.external;
  const v = value as Partial<ExternalViewpointConfig>;
  return {
    originPc: parseVec3(v.originPc, DEFAULT_PROJECTION_CONFIG.external.originPc, -1_000_000, 1_000_000),
    targetPc: parseVec3(v.targetPc, DEFAULT_PROJECTION_CONFIG.external.targetPc, -1_000_000, 1_000_000),
    up: parseVec3(v.up, DEFAULT_PROJECTION_CONFIG.external.up, -10, 10),
  };
}

function parseEyepiece(value: unknown): EyepieceConfig {
  if (!value || typeof value !== "object") return DEFAULT_EYEPIECE_CONFIG;
  const v = value as Partial<EyepieceConfig>;
  return {
    enabled: typeof v.enabled === "boolean" ? v.enabled : DEFAULT_EYEPIECE_CONFIG.enabled,
    apertureMm: inRange(v.apertureMm, 10, 2000) ? v.apertureMm : DEFAULT_EYEPIECE_CONFIG.apertureMm,
    focalLengthMm: inRange(v.focalLengthMm, 50, 20000) ? v.focalLengthMm : DEFAULT_EYEPIECE_CONFIG.focalLengthMm,
    eyepieceFocalLengthMm: inRange(v.eyepieceFocalLengthMm, 1, 100) ? v.eyepieceFocalLengthMm : DEFAULT_EYEPIECE_CONFIG.eyepieceFocalLengthMm,
    apparentFovDeg: inRange(v.apparentFovDeg, 1, 120) ? v.apparentFovDeg : DEFAULT_EYEPIECE_CONFIG.apparentFovDeg,
    fieldStopMm: inRange(v.fieldStopMm, 0, 120) ? v.fieldStopMm : DEFAULT_EYEPIECE_CONFIG.fieldStopMm,
    // V-45 telescope-side optics. Not part of the shared Rust session schema
    // this cycle, so absent values fall back to the defaults.
    opticalDesign: sanitizedOpticalDesign(v.opticalDesign),
    spiderVanes: inRange(v.spiderVanes, 1, 8) ? Math.round(v.spiderVanes) : DEFAULT_EYEPIECE_CONFIG.spiderVanes,
    otaRotationDeg: typeof v.otaRotationDeg === "number" && Number.isFinite(v.otaRotationDeg) ? v.otaRotationDeg : DEFAULT_EYEPIECE_CONFIG.otaRotationDeg,
  };
}
