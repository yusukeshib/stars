export type Observer = {
  latitudeDeg: number;
  longitudeDeg: number;
};

export type View = {
  azimuthDeg: number;
  altitudeDeg: number;
  fovDeg: number;
};

const UNIX_EPOCH_JD = 2440587.5;
const SECONDS_PER_DAY = 86400;

export function julianDateFromUnixSeconds(unixSeconds: number): number {
  return UNIX_EPOCH_JD + unixSeconds / SECONDS_PER_DAY;
}

export function julianDateNow(): number {
  return julianDateFromUnixSeconds(Date.now() / 1000);
}

export function clampAltitude(deg: number): number {
  return Math.max(-89.5, Math.min(89.5, deg));
}

export function wrapAzimuth(deg: number): number {
  let v = deg % 360;
  if (v < 0) v += 360;
  return v;
}

export function clampFov(deg: number): number {
  return Math.max(10, Math.min(120, deg));
}

const DEG = Math.PI / 180;
export const toRad = (d: number) => d * DEG;
