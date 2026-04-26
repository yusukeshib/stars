export type Observer = {
  latitudeDeg: number;
  longitudeDeg: number;
};

export type View = {
  azimuthDeg: number;
  altitudeDeg: number;
  fovDeg: number;
};

export const clampAltitude = (deg: number): number =>
  Math.max(-89.5, Math.min(89.5, deg));

export const wrapAzimuth = (deg: number): number => {
  const v = deg % 360;
  return v < 0 ? v + 360 : v;
};

export const clampFov = (deg: number): number =>
  Math.max(10, Math.min(120, deg));

const DEG = Math.PI / 180;
export const toRad = (d: number) => d * DEG;
