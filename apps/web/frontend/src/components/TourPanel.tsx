import { type CSSProperties, useState } from "react";
import type { Tour, TourScene } from "../tour";

/// L-23 Guided education mode (web host).
///
/// A thin, self-contained presenter for the "first night" tour. The tour
/// content is host-agnostic data (`../tour.ts`, mirroring the canonical Rust
/// `first_night_tour`); this component shows one step's caption + reference at
/// a time and asks the parent to apply the step's declarative scene through
/// the existing renderer state setters. The renderer itself knows nothing
/// about tours.

type Props = {
  tour: Tour;
  /// Apply a step's scene to the live renderer state (observer, time, view,
  /// overlays, projection, atmosphere, planets). Owned by `App` because it
  /// holds the React state setters.
  onApplyScene: (scene: TourScene) => void;
};

export function TourPanel({ tour, onApplyScene }: Props) {
  // `null` = tour inactive (only the launch button shows).
  const [stepIndex, setStepIndex] = useState<number | null>(null);

  const start = () => {
    setStepIndex(0);
    onApplyScene(tour.steps[0].scene);
  };
  const go = (next: number) => {
    if (next < 0 || next >= tour.steps.length) return;
    setStepIndex(next);
    onApplyScene(tour.steps[next].scene);
  };
  const exit = () => setStepIndex(null);

  if (stepIndex === null) {
    return (
      <button
        type="button"
        style={LAUNCH_STYLE}
        onClick={start}
        aria-label={`Start guided tour: ${tour.title}`}
        title={tour.description}
      >
        ★ Guided tour
      </button>
    );
  }

  const step = tour.steps[stepIndex];
  const isFirst = stepIndex === 0;
  const isLast = stepIndex === tour.steps.length - 1;

  return (
    <section style={PANEL_STYLE} aria-label="Guided tour">
      <header style={HEADER_STYLE}>
        <span style={COUNTER_STYLE}>
          Step {stepIndex + 1} / {tour.steps.length}
        </span>
        <button type="button" style={CLOSE_STYLE} onClick={exit} aria-label="Exit guided tour">
          ✕
        </button>
      </header>
      <h2 style={TITLE_STYLE}>{step.title}</h2>
      <p style={CAPTION_STYLE}>{step.caption}</p>
      {step.referenceUrl ? (
        <a style={LINK_STYLE} href={step.referenceUrl} target="_blank" rel="noopener noreferrer">
          Further reading ↗
        </a>
      ) : null}
      <footer style={FOOTER_STYLE}>
        <button
          type="button"
          style={NAV_BUTTON_STYLE}
          onClick={() => go(stepIndex - 1)}
          disabled={isFirst}
        >
          ‹ Back
        </button>
        {isLast ? (
          <button type="button" style={NAV_PRIMARY_STYLE} onClick={exit}>
            Finish
          </button>
        ) : (
          <button type="button" style={NAV_PRIMARY_STYLE} onClick={() => go(stepIndex + 1)}>
            Next ›
          </button>
        )}
      </footer>
    </section>
  );
}

const FONT =
  "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif";

const LAUNCH_STYLE: CSSProperties = {
  position: "absolute",
  bottom: 16,
  left: 16,
  height: 36,
  padding: "0 14px",
  background: "rgba(10, 14, 22, 0.78)",
  border: "1px solid rgba(255,255,255,0.18)",
  borderRadius: 18,
  color: "#e8edf2",
  cursor: "pointer",
  fontSize: 14,
  fontFamily: FONT,
  backdropFilter: "blur(8px)",
  WebkitBackdropFilter: "blur(8px)",
  zIndex: 4,
};

const PANEL_STYLE: CSSProperties = {
  position: "absolute",
  bottom: 16,
  left: 16,
  width: "min(380px, calc(100% - 32px))",
  padding: 16,
  background: "rgba(10, 14, 22, 0.88)",
  border: "1px solid rgba(255,255,255,0.18)",
  borderRadius: 14,
  color: "#e8edf2",
  fontFamily: FONT,
  backdropFilter: "blur(8px)",
  WebkitBackdropFilter: "blur(8px)",
  zIndex: 4,
};

const HEADER_STYLE: CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
};

const COUNTER_STYLE: CSSProperties = {
  fontSize: 12,
  letterSpacing: 0.4,
  textTransform: "uppercase",
  opacity: 0.7,
};

const CLOSE_STYLE: CSSProperties = {
  width: 28,
  height: 28,
  borderRadius: 14,
  background: "transparent",
  border: "1px solid rgba(255,255,255,0.18)",
  color: "#e8edf2",
  cursor: "pointer",
  fontSize: 14,
  lineHeight: 1,
};

const TITLE_STYLE: CSSProperties = {
  margin: "10px 0 6px",
  fontSize: 17,
  fontWeight: 600,
};

const CAPTION_STYLE: CSSProperties = {
  margin: "0 0 10px",
  fontSize: 14,
  lineHeight: 1.5,
  opacity: 0.92,
};

const LINK_STYLE: CSSProperties = {
  display: "inline-block",
  marginBottom: 12,
  fontSize: 13,
  color: "#8ec5ff",
  textDecoration: "none",
};

const FOOTER_STYLE: CSSProperties = {
  display: "flex",
  gap: 8,
  justifyContent: "space-between",
};

const NAV_BUTTON_STYLE: CSSProperties = {
  height: 34,
  padding: "0 14px",
  background: "rgba(255,255,255,0.06)",
  border: "1px solid rgba(255,255,255,0.18)",
  borderRadius: 17,
  color: "#e8edf2",
  cursor: "pointer",
  fontSize: 14,
};

const NAV_PRIMARY_STYLE: CSSProperties = {
  ...NAV_BUTTON_STYLE,
  background: "rgba(80, 130, 200, 0.45)",
};
