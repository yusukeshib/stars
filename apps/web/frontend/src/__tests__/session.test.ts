import { describe, expect, it } from "vitest";
import {
  buildStarSession,
  parseStarSessionJson,
  timeScalesFromUnixMs,
  type SessionState,
} from "../session";
import {
  DEFAULT_ATMOSPHERE_CONFIG,
  DEFAULT_AURORA_CONFIG,
  DEFAULT_COMETS_CONFIG,
  DEFAULT_EYEPIECE_CONFIG,
  DEFAULT_METEORS_CONFIG,
  DEFAULT_OUTPUT_COLOURSPACE,
  DEFAULT_OVERLAY_CONFIG,
  DEFAULT_PLANETS_CONFIG,
  DEFAULT_PROJECTION_CONFIG,
  DEFAULT_SATELLITES_CONFIG,
  DEFAULT_SCINTILLATION_CONFIG,
} from "../observer";

const state = (): SessionState => ({
  observer: { latitudeDeg: 35, longitudeDeg: 139 },
  view: { azimuthDeg: 180, altitudeDeg: 45, fovDeg: 60 },
  overlays: DEFAULT_OVERLAY_CONFIG,
  atmosphere: DEFAULT_ATMOSPHERE_CONFIG,
  scintillation: DEFAULT_SCINTILLATION_CONFIG,
  planets: DEFAULT_PLANETS_CONFIG,
  satellites: DEFAULT_SATELLITES_CONFIG,
  meteors: DEFAULT_METEORS_CONFIG,
  aurora: DEFAULT_AURORA_CONFIG,
  comets: DEFAULT_COMETS_CONFIG,
  projection: DEFAULT_PROJECTION_CONFIG,
  eyepiece: DEFAULT_EYEPIECE_CONFIG,
  outputColourspace: DEFAULT_OUTPUT_COLOURSPACE,
  timeMs: Date.UTC(2025, 0, 1),
});

describe("portable sessions", () => {
  it("exports only authoritative UTC and DUT1 time inputs", () => {
    expect(timeScalesFromUnixMs(0)).toEqual({ jdUtc: 2440587.5, dut1Seconds: 0 });
  });

  it("migrates a v1 atmosphere without a frontend leap-second table", () => {
    const session = buildStarSession(state()) as unknown as Record<string, unknown>;
    session.schemaVersion = 1;
    delete session.scintillation;
    delete session.satellites;
    delete session.outputColourspace;
    const atmosphere = session.atmosphere as Record<string, unknown>;
    delete atmosphere.aerosolBeta;
    delete atmosphere.aerosolAlpha;
    delete atmosphere.surfaceAlbedo;
    atmosphere.turbidity = 3;
    atmosphere.visibilityKm = 25;

    const restored = parseStarSessionJson(JSON.stringify(session));
    expect(restored.atmosphere.aerosolBeta).toBeCloseTo(0.171, 6);
    expect(restored.atmosphere.aerosolAlpha).toBe(1.3);
    expect(restored.scintillation).toEqual(DEFAULT_SCINTILLATION_CONFIG);
  });
});
