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
    set_eyepiece_simulation(
      enabled: boolean,
      apertureMm: number,
      focalLengthMm: number,
      eyepieceFocalLengthMm: number,
      apparentFovDeg: number,
      fieldStopMm: number,
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
    resize(width: number, height: number): void;
    sun_altitude_deg(): number;
    render_frame(): void;
  }
}
