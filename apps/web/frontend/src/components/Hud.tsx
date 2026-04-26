import type { Observer, View } from "../observer";

type Props = {
  observer: Observer;
  view: View;
  timeMs: number;
  paused: boolean;
  onSetObserver: (next: Observer) => void;
  onSetTime: (timeMs: number) => void;
  onSetPaused: (paused: boolean) => void;
  onUseGeolocation: () => void;
  onResetTime: () => void;
};

const fmtDeg = (n: number) => `${n.toFixed(2)}°`;

function toLocalDatetimeInput(timeMs: number): string {
  const d = new Date(timeMs);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function Hud({
  observer,
  view,
  timeMs,
  paused,
  onSetObserver,
  onSetTime,
  onSetPaused,
  onUseGeolocation,
  onResetTime,
}: Props) {
  return (
    <div
      style={{
        position: "absolute",
        top: 12,
        left: 12,
        padding: "12px 14px",
        background: "rgba(10, 12, 22, 0.78)",
        color: "#cfd8e3",
        font: "13px/1.4 ui-monospace, SFMono-Regular, Menlo, monospace",
        borderRadius: 8,
        backdropFilter: "blur(6px)",
        boxShadow: "0 4px 18px rgba(0,0,0,0.45)",
        minWidth: 260,
        userSelect: "none",
      }}
    >
      <div style={{ fontSize: 11, opacity: 0.6, marginBottom: 6 }}>OBSERVER</div>
      <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", gap: "4px 8px" }}>
        <label htmlFor="hud-lat">Lat</label>
        <input
          id="hud-lat"
          type="number"
          step="0.01"
          value={observer.latitudeDeg}
          onChange={(e) => {
            const v = Number(e.target.value);
            if (e.target.value !== "" && Number.isFinite(v)) {
              onSetObserver({ ...observer, latitudeDeg: v });
            }
          }}
          style={inputStyle}
        />
        <label htmlFor="hud-lng">Lng</label>
        <input
          id="hud-lng"
          type="number"
          step="0.01"
          value={observer.longitudeDeg}
          onChange={(e) => {
            const v = Number(e.target.value);
            if (e.target.value !== "" && Number.isFinite(v)) {
              onSetObserver({ ...observer, longitudeDeg: v });
            }
          }}
          style={inputStyle}
        />
      </div>
      <button onClick={onUseGeolocation} style={buttonStyle}>
        Use my location
      </button>

      <div style={{ fontSize: 11, opacity: 0.6, margin: "12px 0 6px" }}>TIME (local)</div>
      <input
        type="datetime-local"
        value={toLocalDatetimeInput(timeMs)}
        onChange={(e) => {
          const v = new Date(e.target.value).getTime();
          if (!Number.isNaN(v)) onSetTime(v);
        }}
        style={{ ...inputStyle, width: "100%" }}
      />
      <div style={{ display: "flex", gap: 6, marginTop: 6 }}>
        <button onClick={() => onSetPaused(!paused)} style={{ ...buttonStyle, flex: 1 }}>
          {paused ? "▶ Resume" : "⏸ Pause"}
        </button>
        <button onClick={onResetTime} style={{ ...buttonStyle, flex: 1 }}>
          Now
        </button>
      </div>

      <div style={{ fontSize: 11, opacity: 0.6, margin: "12px 0 6px" }}>VIEW</div>
      <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", gap: "2px 8px" }}>
        <span>Az</span>
        <span>{fmtDeg(view.azimuthDeg)} ({compass(view.azimuthDeg)})</span>
        <span>Alt</span>
        <span>{fmtDeg(view.altitudeDeg)}</span>
        <span>FOV</span>
        <span>{fmtDeg(view.fovDeg)}</span>
      </div>

      <div style={{ fontSize: 11, opacity: 0.45, marginTop: 12 }}>
        drag to look around · scroll to zoom
      </div>
    </div>
  );
}

const inputStyle: React.CSSProperties = {
  background: "rgba(255,255,255,0.08)",
  color: "#e6edf5",
  border: "1px solid rgba(255,255,255,0.12)",
  borderRadius: 4,
  padding: "3px 6px",
  font: "inherit",
};

const buttonStyle: React.CSSProperties = {
  background: "rgba(80, 130, 220, 0.25)",
  color: "#e6edf5",
  border: "1px solid rgba(120, 160, 230, 0.35)",
  borderRadius: 4,
  padding: "5px 8px",
  marginTop: 6,
  cursor: "pointer",
  font: "inherit",
};

const COMPASS_DIRS = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

function compass(az: number): string {
  // Caller passes already-wrapped [0, 360) azimuth.
  return COMPASS_DIRS[Math.round(az / 45) % 8];
}
