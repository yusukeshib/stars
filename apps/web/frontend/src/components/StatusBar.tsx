import type { Observer, View } from "../observer";

type Props = {
  observer: Observer;
  view: View;
  timeMs: number;
};

const fmtDeg = (n: number) => `${n.toFixed(1)}°`;
const COMPASS_DIRS = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
const compass = (az: number) => COMPASS_DIRS[Math.round(az / 45) % 8];

function fmtTime(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/// Tiny read-only status strip in the bottom-left. No interactive controls live
/// here; everything editable is in the settings panel.
export function StatusBar({ observer, view, timeMs }: Props) {
  return (
    <div
      style={{
        position: "absolute",
        bottom: 14,
        left: 14,
        padding: "8px 12px",
        background: "rgba(10, 12, 22, 0.55)",
        color: "#cfd8e3",
        font: "12px/1.4 ui-monospace, SFMono-Regular, Menlo, monospace",
        borderRadius: 6,
        backdropFilter: "blur(8px)",
        boxShadow: "0 2px 10px rgba(0,0,0,0.35)",
        userSelect: "none",
        pointerEvents: "none",
        opacity: 0.85,
      }}
    >
      <div>
        <span style={{ opacity: 0.55 }}>Location </span>
        {observer.latitudeDeg.toFixed(2)}°, {observer.longitudeDeg.toFixed(2)}°
        <span style={{ opacity: 0.4 }}>  ·  </span>
        <span style={{ opacity: 0.55 }}>Time </span>
        {fmtTime(timeMs)}
      </div>
      <div style={{ marginTop: 2 }}>
        <span style={{ opacity: 0.55 }}>Az </span>
        {fmtDeg(view.azimuthDeg)} ({compass(view.azimuthDeg)})
        <span style={{ opacity: 0.4 }}>  ·  </span>
        <span style={{ opacity: 0.55 }}>Alt </span>
        {fmtDeg(view.altitudeDeg)}
        <span style={{ opacity: 0.4 }}>  ·  </span>
        <span style={{ opacity: 0.55 }}>FOV </span>
        {fmtDeg(view.fovDeg)}
      </div>
    </div>
  );
}
