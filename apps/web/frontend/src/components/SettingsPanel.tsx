import type { Observer } from "../observer";

type Props = {
  observer: Observer;
  timeMs: number;
  onClose: () => void;
  onSetObserver: (next: Observer) => void;
  onSetTime: (timeMs: number) => void;
  onUseGeolocation: () => void;
};

function toLocalDatetimeInput(timeMs: number): string {
  const d = new Date(timeMs);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/// Modal-style settings overlay. Click the backdrop or the × to close.
export function SettingsPanel({
  observer,
  timeMs,
  onClose,
  onSetObserver,
  onSetTime,
  onUseGeolocation,
}: Props) {
  return (
    <div
      // Backdrop. Clicks here close the panel.
      onClick={onClose}
      style={{
        position: "absolute",
        inset: 0,
        background: "rgba(0, 0, 0, 0.45)",
        backdropFilter: "blur(2px)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 10,
      }}
    >
      <div
        // Panel itself. Stop click propagation so backdrop dismiss doesn't fire.
        onClick={(e) => e.stopPropagation()}
        style={{
          width: "min(420px, calc(100vw - 32px))",
          maxHeight: "calc(100vh - 64px)",
          overflowY: "auto",
          padding: "18px 20px 20px",
          background: "rgba(14, 18, 30, 0.95)",
          color: "#e6edf5",
          font: "13px/1.45 ui-monospace, SFMono-Regular, Menlo, monospace",
          borderRadius: 12,
          border: "1px solid rgba(160, 180, 220, 0.18)",
          boxShadow: "0 12px 40px rgba(0, 0, 0, 0.6)",
          userSelect: "none",
        }}
      >
        <header
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            marginBottom: 16,
          }}
        >
          <h2 style={{ margin: 0, fontSize: 14, fontWeight: 600, letterSpacing: 0.6 }}>
            SETTINGS
          </h2>
          <button
            type="button"
            aria-label="Close settings"
            onClick={onClose}
            style={{
              width: 28,
              height: 28,
              borderRadius: 6,
              background: "transparent",
              color: "#cfd8e3",
              border: "1px solid rgba(255, 255, 255, 0.12)",
              cursor: "pointer",
              font: "16px/1 ui-monospace, monospace",
              padding: 0,
            }}
          >
            ×
          </button>
        </header>

        <Section label="OBSERVER">
          <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", gap: "6px 10px" }}>
            <label htmlFor="set-lat" style={labelStyle}>Lat</label>
            <input
              id="set-lat"
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
            <label htmlFor="set-lng" style={labelStyle}>Lng</label>
            <input
              id="set-lng"
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
        </Section>

        <Section label="TIME (local)">
          <input
            type="datetime-local"
            value={toLocalDatetimeInput(timeMs)}
            onChange={(e) => {
              const v = new Date(e.target.value).getTime();
              if (!Number.isNaN(v)) onSetTime(v);
            }}
            style={{ ...inputStyle, width: "100%" }}
          />
          <p style={{ margin: "8px 0 0", fontSize: 11, opacity: 0.55 }}>
            Setting a time rebases the clock; it then ticks forward from there.
          </p>
        </Section>

        <p style={{ margin: "16px 0 0", fontSize: 11, opacity: 0.45 }}>
          drag the sky to look around · scroll to zoom
        </p>
      </div>
    </div>
  );
}

function Section({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <section style={{ marginBottom: 14 }}>
      <div style={{ fontSize: 11, opacity: 0.55, marginBottom: 6, letterSpacing: 0.6 }}>
        {label}
      </div>
      {children}
    </section>
  );
}

const labelStyle: React.CSSProperties = {
  alignSelf: "center",
  opacity: 0.7,
};

const inputStyle: React.CSSProperties = {
  background: "rgba(255, 255, 255, 0.07)",
  color: "#e6edf5",
  border: "1px solid rgba(255, 255, 255, 0.12)",
  borderRadius: 5,
  padding: "5px 8px",
  font: "inherit",
};

const buttonStyle: React.CSSProperties = {
  background: "rgba(80, 130, 220, 0.22)",
  color: "#e6edf5",
  border: "1px solid rgba(120, 160, 230, 0.35)",
  borderRadius: 5,
  padding: "6px 10px",
  marginTop: 10,
  cursor: "pointer",
  font: "inherit",
};
