import type { OverlayConfig } from "../observer";
import { OverlayToggles } from "./OverlayToggles";

type Props = {
  overlays: OverlayConfig;
  onClose: () => void;
  onSetOverlays: (next: OverlayConfig) => void;
};

/// Modal-style settings overlay. Click the backdrop or the × to close.
export function SettingsPanel({ overlays, onClose, onSetOverlays }: Props) {
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

        <Section label="OVERLAYS">
          <OverlayToggles config={overlays} onChange={onSetOverlays} />
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
