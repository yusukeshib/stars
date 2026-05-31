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
  "deep-sky-objects",
  "deep-sky-labels",
  "star-labels",
  "planet-labels",
  "constellation-labels",
  "cardinal-labels",
  "degree-labels",
] as const;
export type OverlayLayer = (typeof OVERLAY_LAYERS)[number];

/// Default Messier deep-sky magnitude cutoff. Kept in sync with
/// `DEFAULT_DEEP_SKY_MAGNITUDE_LIMIT` in `crates/renderer/src/overlay.rs`.
export const DEFAULT_DEEP_SKY_MAGNITUDE_LIMIT = 7;
export const MIN_DEEP_SKY_MAGNITUDE_LIMIT = -5;
export const MAX_DEEP_SKY_MAGNITUDE_LIMIT = 99;

/// L-24 accessibility overlay palettes. Kept in sync with `OverlayPalette`
/// in `crates/renderer/src/overlay.rs` and the CLI `--overlay-palette` flag.
export const OVERLAY_PALETTES = ["default", "colorblind-safe", "high-contrast"] as const;
export type OverlayPalette = (typeof OVERLAY_PALETTES)[number];

export type OverlayConfig = {
  layers: OverlayLayer[];
  gridStepDeg: number;
  opacity: number;
  deepSkyMagnitudeLimit: number;
  palette: OverlayPalette;
};

export const DEFAULT_OVERLAY_CONFIG: OverlayConfig = {
  layers: ["horizon", "cardinal-labels"],
  gridStepDeg: 15,
  opacity: 0.6,
  deepSkyMagnitudeLimit: DEFAULT_DEEP_SKY_MAGNITUDE_LIMIT,
  palette: "default",
};

export const ATMOSPHERE_PRESETS = ["clear-rural", "hazy-urban", "high-altitude"] as const;
export type AtmospherePreset = (typeof ATMOSPHERE_PRESETS)[number];

export type AtmosphereConfig = {
  enabled: boolean;
  preset: AtmospherePreset;
  /** Ångström aerosol optical depth at 550 nm. Drives both stellar k(λ) and
   *  daylight Mie scattering through the unified V-37 state. */
  aerosolBeta: number;
  /** Ångström wavelength exponent. Continental aerosols ≈ 1.3. */
  aerosolAlpha: number;
  observerAltitudeM: number;
  ozoneDu: number;
  pressureHpa: number;
  temperatureC: number;
  /** Ground albedo seen by the V-38 Hošek-Wilkie daylight model.
   *  Continental mixed terrain ≈ 0.10; snow ≈ 0.80; ocean ≈ 0.06. */
  surfaceAlbedo: number;
};

export type PlanetsConfig = {
  enabled: boolean;
};

/** V-55 artificial-satellite layer (TLE / SGP4) state. */
export type SatellitesConfig = {
  enabled: boolean;
  /** Frame-integration exposure (seconds). 0 = point sprites, >0 = streaks. */
  exposureSeconds: number;
};

/** V-47 meteor-shower layer state. */
export type MeteorsConfig = {
  enabled: boolean;
  /** Deterministic stream seed. */
  seed: number;
  /** Multiplier on the modelled observed rate (1.0 = physical expectation). */
  rateScale: number;
  /** Long-exposure integration window (seconds). */
  windowSeconds: number;
};

// V-48 aurora layer. Season spellings match the Rust `AuroraSeasonArg`
// kebab-case serde / `set_aurora` season argument.
export const AURORA_SEASONS = ["winter", "equinox", "summer"] as const;
export type AuroraSeason = (typeof AURORA_SEASONS)[number];

/** V-48 aurora-layer state. */
export type AuroraConfig = {
  enabled: boolean;
  /** Planetary Kp index (0..9) driving oval position and brightness. */
  kp: number;
  season: AuroraSeason;
};

export const DEFAULT_AURORA_CONFIG: AuroraConfig = {
  enabled: false,
  kp: 0,
  season: "equinox",
};

export const isAuroraSeason = (s: unknown): s is AuroraSeason =>
  typeof s === "string" && (AURORA_SEASONS as readonly string[]).includes(s);

/** V-49 comet layer (curated osculating elements) state. */
export type CometsConfig = {
  enabled: boolean;
};

/** Per-frame atmospheric scintillation state (V-24). */
export type ScintillationConfig = {
  enabled: boolean;
  /** Dimensionless Cn² column scale (1.0 = Dravins amateur-site median). */
  cN2Scale: number;
  /** Deterministic noise seed; part of the session schema. */
  seed: number;
};

export const DEFAULT_SCINTILLATION_CONFIG: ScintillationConfig = {
  enabled: true,
  cN2Scale: 1.0,
  seed: 0x5C157107,
};

// V-45 telescope optical design driving the eyepiece diffraction artifacts.
// Kebab spellings match the Rust `OpticalDesign::as_kebab_str`.
export type OpticalDesign =
  | "apo-refractor"
  | "achromat-refractor"
  | "newtonian"
  | "schmidt-cassegrain";

export const OPTICAL_DESIGNS: OpticalDesign[] = [
  "apo-refractor",
  "achromat-refractor",
  "newtonian",
  "schmidt-cassegrain",
];

export type EyepieceConfig = {
  enabled: boolean;
  apertureMm: number;
  focalLengthMm: number;
  eyepieceFocalLengthMm: number;
  apparentFovDeg: number;
  fieldStopMm: number;
  // V-45 telescope-side optics (live render state; not part of the shared
  // JSON session schema this cycle).
  opticalDesign: OpticalDesign;
  spiderVanes: number;
  otaRotationDeg: number;
};

// V-50 output colour management. Kebab spellings match the Rust
// `OutputColourSpace::as_str` / session `outputColourspace` field.
export const OUTPUT_COLOURSPACES = ["srgb", "display-p3", "rec2020"] as const;
export type OutputColourspace = (typeof OUTPUT_COLOURSPACES)[number];
export const DEFAULT_OUTPUT_COLOURSPACE: OutputColourspace = "srgb";

export const SKY_PROJECTIONS = ["perspective", "mollweide", "aitoff", "hammer"] as const;
export type SkyProjection = (typeof SKY_PROJECTIONS)[number];

export const SKY_VIEWPOINTS = ["earth", "galactic-north", "custom-external"] as const;
export type SkyViewpoint = (typeof SKY_VIEWPOINTS)[number];

export type Vec3 = {
  x: number;
  y: number;
  z: number;
};

export type ExternalViewpointConfig = {
  originPc: Vec3;
  targetPc: Vec3;
  up: Vec3;
};

export type ProjectionConfig = {
  projection: SkyProjection;
  viewpoint: SkyViewpoint;
  external: ExternalViewpointConfig;
};

export const DEFAULT_EXTERNAL_VIEWPOINT: ExternalViewpointConfig = {
  originPc: { x: 0, y: 0, z: 30000 },
  targetPc: { x: 0, y: 0, z: 0 },
  up: { x: 0, y: 1, z: 0 },
};

export const DEFAULT_PROJECTION_CONFIG: ProjectionConfig = {
  projection: "perspective",
  viewpoint: "earth",
  external: DEFAULT_EXTERNAL_VIEWPOINT,
};

export const DEFAULT_PLANETS_CONFIG: PlanetsConfig = {
  enabled: true,
};

export const DEFAULT_SATELLITES_CONFIG: SatellitesConfig = {
  enabled: false,
  exposureSeconds: 0,
};

export const DEFAULT_METEORS_CONFIG: MeteorsConfig = {
  enabled: false,
  seed: 1,
  rateScale: 1.0,
  windowSeconds: 120.0,
};

export const DEFAULT_COMETS_CONFIG: CometsConfig = {
  enabled: false,
};

export const DEFAULT_EYEPIECE_CONFIG: EyepieceConfig = {
  enabled: false,
  apertureMm: 200,
  focalLengthMm: 2000,
  eyepieceFocalLengthMm: 25,
  apparentFovDeg: 50,
  fieldStopMm: 21,
  opticalDesign: "apo-refractor",
  spiderVanes: 4,
  otaRotationDeg: 0,
};

export const sanitizedOpticalDesign = (value: unknown): OpticalDesign =>
  OPTICAL_DESIGNS.includes(value as OpticalDesign)
    ? (value as OpticalDesign)
    : DEFAULT_EYEPIECE_CONFIG.opticalDesign;

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

/// L-09: one entry in "tonight's recommended objects", carrying its
/// visibility score and Moon-impact (Krisciunas-Schaefer 1991).
export type RecommendedTarget = {
  name: string;
  score: number;
  maxAltitudeDeg: number;
  observableDarkHours: number;
  windowStartMs: number | null;
  windowEndMs: number | null;
  moonDeltaVMag: number;
  moonAltitudeDeg: number;
  moonIlluminatedFraction: number;
};

export type RecommendedPlan = {
  darkSkyZenithVMag: number;
  recommended: RecommendedTarget[];
};

export const DEFAULT_ATMOSPHERE_CONFIG: AtmosphereConfig = {
  enabled: true,
  preset: "clear-rural",
  aerosolBeta: 0.10,
  aerosolAlpha: 1.30,
  observerAltitudeM: 0,
  ozoneDu: 300,
  pressureHpa: 1010,
  temperatureC: 10,
  surfaceAlbedo: 0.10,
};

export const ATMOSPHERE_PRESET_DEFAULTS: Record<AtmospherePreset, Pick<AtmosphereConfig, "aerosolBeta" | "aerosolAlpha" | "observerAltitudeM" | "ozoneDu" | "pressureHpa" | "temperatureC" | "surfaceAlbedo">> = {
  "clear-rural":   { aerosolBeta: 0.10, aerosolAlpha: 1.30, observerAltitudeM: 0,    ozoneDu: 300, pressureHpa: 1010, temperatureC: 10, surfaceAlbedo: 0.10 },
  "hazy-urban":    { aerosolBeta: 0.35, aerosolAlpha: 1.10, observerAltitudeM: 0,    ozoneDu: 325, pressureHpa: 1010, temperatureC: 15, surfaceAlbedo: 0.13 },
  "high-altitude": { aerosolBeta: 0.04, aerosolAlpha: 1.30, observerAltitudeM: 2500, ozoneDu: 275, pressureHpa:  750, temperatureC:  0, surfaceAlbedo: 0.30 },
};

export const isAtmospherePreset = (s: unknown): s is AtmospherePreset =>
  typeof s === "string" && (ATMOSPHERE_PRESETS as readonly string[]).includes(s);

export const isSkyProjection = (s: unknown): s is SkyProjection =>
  typeof s === "string" && (SKY_PROJECTIONS as readonly string[]).includes(s);

export const isOutputColourspace = (s: unknown): s is OutputColourspace =>
  typeof s === "string" && (OUTPUT_COLOURSPACES as readonly string[]).includes(s);

export const isSkyViewpoint = (s: unknown): s is SkyViewpoint =>
  typeof s === "string" && (SKY_VIEWPOINTS as readonly string[]).includes(s);

export const isOverlayLayer = (s: unknown): s is OverlayLayer =>
  typeof s === "string" && (OVERLAY_LAYERS as readonly string[]).includes(s);

export const isOverlayPalette = (s: unknown): s is OverlayPalette =>
  typeof s === "string" && (OVERLAY_PALETTES as readonly string[]).includes(s);

export const MIN_ALTITUDE_DEG = -89.5;
export const MAX_ALTITUDE_DEG = 89.5;
export const MIN_FOV_DEG = 0.05;
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
const RAD = 180 / Math.PI;
export const toRad = (d: number) => d * DEG;

const positiveOr = (value: number, fallback: number): number =>
  Number.isFinite(value) && value > 0 ? value : fallback;

export const sanitizedEyepiece = (eyepiece: EyepieceConfig): EyepieceConfig => ({
  enabled: eyepiece.enabled,
  apertureMm: positiveOr(eyepiece.apertureMm, DEFAULT_EYEPIECE_CONFIG.apertureMm),
  focalLengthMm: positiveOr(eyepiece.focalLengthMm, DEFAULT_EYEPIECE_CONFIG.focalLengthMm),
  eyepieceFocalLengthMm: positiveOr(
    eyepiece.eyepieceFocalLengthMm,
    DEFAULT_EYEPIECE_CONFIG.eyepieceFocalLengthMm,
  ),
  apparentFovDeg: positiveOr(eyepiece.apparentFovDeg, DEFAULT_EYEPIECE_CONFIG.apparentFovDeg),
  fieldStopMm: Number.isFinite(eyepiece.fieldStopMm) ? Math.max(0, eyepiece.fieldStopMm) : DEFAULT_EYEPIECE_CONFIG.fieldStopMm,
  opticalDesign: sanitizedOpticalDesign(eyepiece.opticalDesign),
  spiderVanes: Number.isFinite(eyepiece.spiderVanes)
    ? Math.max(1, Math.min(8, Math.round(eyepiece.spiderVanes)))
    : DEFAULT_EYEPIECE_CONFIG.spiderVanes,
  otaRotationDeg: Number.isFinite(eyepiece.otaRotationDeg) ? eyepiece.otaRotationDeg : 0,
});

export const eyepieceMagnification = (eyepiece: EyepieceConfig): number => {
  const e = sanitizedEyepiece(eyepiece);
  return e.focalLengthMm / e.eyepieceFocalLengthMm;
};

export const eyepiecePlateScaleArcsecPerMm = (eyepiece: EyepieceConfig): number =>
  206264.806 / sanitizedEyepiece(eyepiece).focalLengthMm;

export const eyepieceExitPupilMm = (eyepiece: EyepieceConfig): number =>
  sanitizedEyepiece(eyepiece).apertureMm / eyepieceMagnification(eyepiece);

export const eyepieceTrueFieldDeg = (eyepiece: EyepieceConfig): number => {
  const e = sanitizedEyepiece(eyepiece);
  const trueFieldRad = e.fieldStopMm > 0
    ? 2 * Math.atan(e.fieldStopMm / (2 * e.focalLengthMm))
    : (e.apparentFovDeg * DEG) / eyepieceMagnification(e);
  return clampFov(trueFieldRad * RAD);
};
