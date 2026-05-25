declare module "stars-web" {
  export default function init(): Promise<void>;

  export class StarView {
    static create(canvasId: string): Promise<StarView>;
    set_observer(latDeg: number, lngDeg: number, timeUnixMs: number): void;
    set_view(azimuthRad: number, altitudeRad: number, fovYRad: number): void;
    set_overlays(layers: string[], gridStepDeg: number, opacity: number): void;
    set_atmosphere_config(
      enabled: boolean,
      preset: string,
      turbidity: number,
      observerAltitudeM: number,
      ozoneDu: number,
      visibilityKm: number,
    ): void;
    resize(width: number, height: number): void;
    sun_altitude_deg(): number;
    render_frame(): void;
  }
}
