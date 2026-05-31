declare module "stars-web" {
  export default function init(): Promise<void>;

  export class StarView {
    static create(canvasId: string): Promise<StarView>;
    set_observer(latDeg: number, lngDeg: number, timeUnixMs: number): void;
    set_view(azimuthRad: number, altitudeRad: number, fovYRad: number): void;
    set_overlays(
      layers: string[],
      gridStepDeg: number,
      opacity: number,
      deepSkyMagnitudeLimit: number,
    ): void;
    set_planets_enabled(enabled: boolean): void;
    set_satellites(enabled: boolean, exposureSeconds: number): void;
    set_meteors(enabled: boolean, seed: number, rateScale: number, windowSeconds: number): void;
    set_aurora(enabled: boolean, kp: number, season: string): void;
    set_eyepiece_simulation(
      enabled: boolean,
      apertureMm: number,
      focalLengthMm: number,
      eyepieceFocalLengthMm: number,
      apparentFovDeg: number,
      fieldStopMm: number,
    ): void;
    set_telescope_optics(
      design: string,
      spiderVanes: number,
      otaRotationDeg: number,
    ): void;
    set_projection(projection: string): void;
    set_viewpoint(viewpoint: string): void;
    set_external_viewpoint(
      originXpc: number,
      originYpc: number,
      originZpc: number,
      targetXpc: number,
      targetYpc: number,
      targetZpc: number,
      upX: number,
      upY: number,
      upZ: number,
    ): void;
    planning_table_json(): string;
    planning_recommended_json(): string;
    planning_ical(): string;
    set_atmosphere_config(
      enabled: boolean,
      preset: string,
      aerosolBeta: number,
      aerosolAlpha: number,
      observerAltitudeM: number,
      ozoneDu: number,
      pressureHpa: number,
      temperatureC: number,
      surfaceAlbedo: number,
    ): void;
    set_light_pollution(
      enabled: boolean,
      kind: string,
      bortleClass: number,
      sqmMagPerArcsec2: number,
      atlasLatitudeDeg: number,
      atlasLongitudeDeg: number,
    ): void;
    set_scintillation(enabled: boolean, cN2Scale: number, seed: number): void;
    /// V-50 output colour management: "srgb" | "display-p3" | "rec2020".
    set_output_colourspace(space: string): void;
    resize(width: number, height: number): void;
    sun_altitude_deg(): number;
    /// V-56 search: free-text lookup over named stars, deep-sky catalogs,
    /// and solar-system bodies. Returns JSON.
    lookup_object(query: string, limit: number): string;
    /// V-56 GoTo: resolve a search id to an apparent topocentric
    /// (alt, az) plus an info-panel payload. Returns JSON or `"null"`.
    goto_object(id: string): string;
    render_frame(): void;
  }
}
