declare module "stars-web" {
  export default function init(): Promise<void>;

  export class StarView {
    static create(canvasId: string): Promise<unknown>;
    sun_altitude_deg(): number;
  }
}
