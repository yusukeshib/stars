import { type CSSProperties, useEffect, useMemo, useRef, useState } from "react";

/// V-56 object search, GoTo, and info panel.
///
/// The Rust side owns the index (`catalog::search`) and the apparent-position
/// resolution (`StarView::lookup_object` / `goto_object`). This component is
/// the thin host wiring: debounced query → WASM lookup → ranked dropdown →
/// click → WASM `goto_object` → camera recentre + info panel.

export type SearchMatch = {
  id: string;
  kind: "star" | "messier" | "ngc" | "ic" | "solar-system";
  display: string;
  aka: string;
  score: number;
  raRad: number;
  decRad: number;
  magnitude: number | null;
};

export type GotoRecord = {
  id: string;
  kind: SearchMatch["kind"];
  display: string;
  aka: string;
  raRad: number;
  decRad: number;
  azimuthRad: number;
  altitudeRad: number;
  magnitude: number | null;
  distance: { value: number; unit: string } | null;
  riseSetMs: { rise: number | null; transit: number | null; set: number | null } | null;
  // L-18 canonical primary catalogue identifier (e.g. "HIP 32349"). Null for
  // solar-system bodies. Surfaced as a click-to-copy chip in the info panel.
  primaryId: string | null;
  // L-19 CDS deep links. Null for solar-system bodies, which the CDS stellar
  // archives do not catalogue.
  simbadUrl: string | null;
  vizierUrl: string | null;
  // L-20 variable-star light-curve state at the session time. Null unless the
  // target is a known variable star.
  variable: VariableInfo | null;
};

// L-20: variable-star light-curve elements + current predicted state.
export type VariableInfo = {
  name: string;
  type: string;
  periodDays: number;
  epochJd: number;
  magBright: number;
  magFaint: number;
  phase: number;
  currentMagnitude: number;
  deltaMagnitude: number;
  reference: string;
  // One-period (phase, magnitude) samples for the panel light curve.
  curve: [number, number][];
};

/// L-19: opt-in flag for showing external CDS catalogue links in the info
/// panel. Persisted on its own localStorage key (default off) so it is
/// independent of the main session schema and never triggers a network call.
const LINKS_PREF_KEY = "stars.externalLinks.v1";

const loadExternalLinksPref = (): boolean => {
  if (typeof localStorage === "undefined") return false;
  try {
    return localStorage.getItem(LINKS_PREF_KEY) === "1";
  } catch {
    return false;
  }
};

const saveExternalLinksPref = (enabled: boolean): void => {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(LINKS_PREF_KEY, enabled ? "1" : "0");
  } catch {
    // Ignore quota / private-mode failures; the toggle is non-essential.
  }
};

type LookupResponse = { matches: SearchMatch[] };

const DEBOUNCE_MS = 120;

const RAD_TO_DEG = 180 / Math.PI;

const isMatch = (value: unknown): value is SearchMatch => {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v.id === "string" &&
    (v.kind === "star" || v.kind === "messier" || v.kind === "ngc" || v.kind === "ic" || v.kind === "solar-system") &&
    typeof v.display === "string" &&
    typeof v.aka === "string" &&
    typeof v.score === "number" &&
    typeof v.raRad === "number" &&
    typeof v.decRad === "number" &&
    (v.magnitude === null || typeof v.magnitude === "number")
  );
};

const isLookupResponse = (value: unknown): value is LookupResponse => {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return Array.isArray(v.matches) && v.matches.every(isMatch);
};

const isGotoRecord = (value: unknown): value is GotoRecord => {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v.id === "string" &&
    typeof v.display === "string" &&
    typeof v.raRad === "number" &&
    typeof v.decRad === "number" &&
    typeof v.azimuthRad === "number" &&
    typeof v.altitudeRad === "number"
  );
};

const formatRa = (ra: number): string => {
  const hours = ((ra * 12) / Math.PI + 24) % 24;
  const h = Math.floor(hours);
  const minutesFractional = (hours - h) * 60;
  const m = Math.floor(minutesFractional);
  const s = (minutesFractional - m) * 60;
  return `${h.toString().padStart(2, "0")}h${m.toString().padStart(2, "0")}m${s.toFixed(1).padStart(4, "0")}s`;
};

const formatDec = (dec: number): string => {
  const deg = dec * RAD_TO_DEG;
  const sign = deg >= 0 ? "+" : "−";
  const abs = Math.abs(deg);
  const d = Math.floor(abs);
  const minutesFractional = (abs - d) * 60;
  const m = Math.floor(minutesFractional);
  const s = (minutesFractional - m) * 60;
  return `${sign}${d.toString().padStart(2, "0")}°${m.toString().padStart(2, "0")}′${s.toFixed(1).padStart(4, "0")}″`;
};

const formatAltAz = (altRad: number, azRad: number): string =>
  `alt ${(altRad * RAD_TO_DEG).toFixed(2)}°, az ${((azRad * RAD_TO_DEG + 360) % 360).toFixed(2)}°`;

const formatTime = (ms: number | null): string => {
  if (ms === null || !Number.isFinite(ms)) return "—";
  const d = new Date(ms);
  const h = d.getHours().toString().padStart(2, "0");
  const m = d.getMinutes().toString().padStart(2, "0");
  return `${h}:${m}`;
};

const kindIcon = (kind: SearchMatch["kind"]): string => {
  switch (kind) {
    case "star":
      return "✦";
    case "messier":
    case "ngc":
    case "ic":
      return "◇";
    case "solar-system":
      return "○";
  }
};

const PANEL_STYLE: CSSProperties = {
  position: "absolute",
  top: 16,
  left: "50%",
  transform: "translateX(-50%)",
  width: "min(420px, calc(100% - 32px))",
  fontFamily:
    "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif",
  color: "#e8edf2",
  zIndex: 4,
};

const INPUT_ROW_STYLE: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
};

const INPUT_STYLE: CSSProperties = {
  flex: 1,
  height: 36,
  padding: "0 12px",
  background: "rgba(10, 14, 22, 0.78)",
  border: "1px solid rgba(255,255,255,0.18)",
  borderRadius: 18,
  color: "#e8edf2",
  fontSize: 14,
  outline: "none",
  backdropFilter: "blur(8px)",
  WebkitBackdropFilter: "blur(8px)",
};

const CLOSE_BUTTON_STYLE: CSSProperties = {
  width: 32,
  height: 32,
  borderRadius: 16,
  background: "rgba(10, 14, 22, 0.78)",
  border: "1px solid rgba(255,255,255,0.18)",
  color: "#e8edf2",
  cursor: "pointer",
  fontSize: 18,
  lineHeight: 1,
};

const DROPDOWN_STYLE: CSSProperties = {
  listStyle: "none",
  margin: "4px 0 0",
  padding: 4,
  background: "rgba(10, 14, 22, 0.92)",
  border: "1px solid rgba(255,255,255,0.18)",
  borderRadius: 12,
  backdropFilter: "blur(8px)",
  WebkitBackdropFilter: "blur(8px)",
  maxHeight: "60vh",
  overflowY: "auto",
};

const ROW_BUTTON_STYLE: CSSProperties = {
  display: "grid",
  gridTemplateColumns: "20px 1fr auto",
  alignItems: "baseline",
  gap: "0 10px",
  width: "100%",
  padding: "8px 10px",
  background: "transparent",
  border: "none",
  borderRadius: 8,
  color: "#e8edf2",
  textAlign: "left",
  cursor: "pointer",
  fontSize: 13,
};

const ROW_AKA_STYLE: CSSProperties = {
  gridColumn: "2 / 3",
  fontSize: 11,
  opacity: 0.65,
};

const ROW_MAG_STYLE: CSSProperties = {
  gridColumn: "3 / 4",
  gridRow: "1 / 3",
  fontVariantNumeric: "tabular-nums",
  fontSize: 11,
  opacity: 0.7,
};

const INFO_STYLE: CSSProperties = {
  marginTop: 8,
  padding: 12,
  background: "rgba(10, 14, 22, 0.92)",
  border: "1px solid rgba(255,255,255,0.18)",
  borderRadius: 12,
  backdropFilter: "blur(8px)",
  WebkitBackdropFilter: "blur(8px)",
  fontSize: 13,
};

const INFO_TITLE_STYLE: CSSProperties = {
  fontSize: 15,
  fontWeight: 600,
};

const INFO_AKA_STYLE: CSSProperties = {
  marginTop: 2,
  fontSize: 11,
  opacity: 0.7,
};

const INFO_GRID_STYLE: CSSProperties = {
  display: "grid",
  gridTemplateColumns: "auto 1fr",
  rowGap: 4,
  columnGap: 10,
  marginTop: 10,
  fontVariantNumeric: "tabular-nums",
};

const INFO_DT_STYLE: CSSProperties = {
  opacity: 0.65,
  fontSize: 11,
  textTransform: "uppercase",
  letterSpacing: 0.4,
};

const INFO_DD_STYLE: CSSProperties = {
  margin: 0,
};

// L-18: click-to-copy primary identifier chip.
const ID_COPY_STYLE: CSSProperties = {
  margin: 0,
  padding: 0,
  border: "none",
  background: "none",
  color: "inherit",
  font: "inherit",
  cursor: "pointer",
  textDecoration: "underline dotted",
};

const LINKS_BLOCK_STYLE: CSSProperties = {
  marginTop: 10,
  paddingTop: 10,
  borderTop: "1px solid rgba(255,255,255,0.12)",
};

const VARIABLE_BLOCK_STYLE: CSSProperties = {
  marginTop: 10,
  paddingTop: 10,
  borderTop: "1px solid rgba(255,255,255,0.12)",
};

const VARIABLE_TITLE_STYLE: CSSProperties = {
  fontSize: 11,
  opacity: 0.8,
};

const VARIABLE_CAPTION_STYLE: CSSProperties = {
  fontSize: 10,
  opacity: 0.65,
  marginTop: 6,
  lineHeight: 1.4,
};

const LINKS_TOGGLE_STYLE: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
  fontSize: 11,
  opacity: 0.75,
  cursor: "pointer",
};

const LINKS_ROW_STYLE: CSSProperties = {
  display: "flex",
  gap: 10,
  marginTop: 8,
};

const LINK_STYLE: CSSProperties = {
  color: "#9ecbff",
  fontSize: 12,
  textDecoration: "none",
};

// L-20: a small SVG light curve (magnitude vs phase) with the current phase
// marked. Magnitude axis is inverted (brighter = up), matching convention.
function VariableLightCurve({ variable }: { variable: VariableInfo }) {
  const w = 240;
  const h = 96;
  const pad = 6;
  const { curve, magBright, magFaint, phase, currentMagnitude } = variable;
  const span = Math.max(1e-6, magFaint - magBright);
  const x = (p: number) => pad + p * (w - 2 * pad);
  // Brighter (smaller mag) at the top.
  const y = (m: number) => pad + ((m - magBright) / span) * (h - 2 * pad);
  const path = curve.length
    ? curve.map(([p, m], i) => `${i === 0 ? "M" : "L"}${x(p).toFixed(1)},${y(m).toFixed(1)}`).join(" ")
    : "";
  return (
    <svg
      width={w}
      height={h}
      viewBox={`0 0 ${w} ${h}`}
      role="img"
      aria-label={`Light curve for ${variable.name}: V ${magBright.toFixed(1)} to ${magFaint.toFixed(1)} over ${variable.periodDays.toFixed(2)} days`}
      style={{ background: "#0b1016", borderRadius: 4, display: "block", marginTop: 6 }}
    >
      <path d={path} fill="none" stroke="#9ecbff" strokeWidth={1.5} />
      <line x1={x(phase)} y1={pad} x2={x(phase)} y2={h - pad} stroke="#ffcc66" strokeWidth={1} />
      <circle cx={x(phase)} cy={y(currentMagnitude)} r={3} fill="#ffcc66" />
      <text x={pad} y={pad + 8} fill="#7f8c99" fontSize={9}>
        V {magBright.toFixed(1)}
      </text>
      <text x={pad} y={h - pad - 2} fill="#7f8c99" fontSize={9}>
        V {magFaint.toFixed(1)}
      </text>
    </svg>
  );
}

interface SearchPanelProps {
  /// Returns the lookup response JSON for the given query.
  onLookup: (query: string, limit: number) => string;
  /// Resolves an id to apparent position; the parent applies the resulting
  /// (azimuthRad, altitudeRad) to the camera view.
  onGoto: (id: string) => string;
  /// Slew the camera. Smooth-step animation is the host's job, not the
  /// engine's, so this stays out of WASM.
  onApplyView: (azimuthRad: number, altitudeRad: number) => void;
}

export function SearchPanel({ onLookup, onGoto, onApplyView }: SearchPanelProps) {
  const [query, setQuery] = useState("");
  const [matches, setMatches] = useState<SearchMatch[]>([]);
  const [showDropdown, setShowDropdown] = useState(false);
  const [selected, setSelected] = useState<GotoRecord | null>(null);
  const [showLinks, setShowLinks] = useState<boolean>(loadExternalLinksPref);
  const inputRef = useRef<HTMLInputElement | null>(null);

  // Debounced lookup. Lookup runs purely in WASM and is O(N) over ~1.2k
  // named-star rows + the deep-sky tables, which is fine on every keystroke
  // but still debounced so the dropdown does not flicker while the user is
  // mid-word.
  useEffect(() => {
    const trimmed = query.trim();
    if (trimmed.length === 0) {
      setMatches([]);
      return undefined;
    }
    const handle = window.setTimeout(() => {
      try {
        const raw = onLookup(trimmed, 12);
        const parsed: unknown = JSON.parse(raw);
        if (isLookupResponse(parsed)) {
          setMatches(parsed.matches);
        } else {
          setMatches([]);
        }
      } catch {
        setMatches([]);
      }
    }, DEBOUNCE_MS);
    return () => window.clearTimeout(handle);
  }, [query, onLookup]);

  const commit = useMemo(
    () => (id: string) => {
      try {
        const raw = onGoto(id);
        if (raw === "null") return;
        const parsed: unknown = JSON.parse(raw);
        if (!isGotoRecord(parsed)) return;
        setSelected(parsed);
        setShowDropdown(false);
        onApplyView(parsed.azimuthRad, parsed.altitudeRad);
      } catch {
        // Stay quiet on malformed responses; the WASM side returns `null`
        // for unknown ids and that path is already handled above.
      }
    },
    [onGoto, onApplyView],
  );

  return (
    <div style={PANEL_STYLE}>
      <div style={INPUT_ROW_STYLE}>
        <input
          ref={inputRef}
          style={INPUT_STYLE}
          type="search"
          placeholder="Search: Sirius · M31 · Saturn · 土星"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setShowDropdown(true);
          }}
          onFocus={() => setShowDropdown(true)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              setQuery("");
              setShowDropdown(false);
              inputRef.current?.blur();
            } else if (event.key === "Enter" && matches.length > 0) {
              commit(matches[0].id);
            }
          }}
          aria-label="Object search"
          autoComplete="off"
          spellCheck={false}
        />
        {selected !== null && (
          <button
            type="button"
            style={CLOSE_BUTTON_STYLE}
            onClick={() => setSelected(null)}
            aria-label="Close info panel"
          >
            ×
          </button>
        )}
      </div>
      {showDropdown && matches.length > 0 && (
        <ul style={DROPDOWN_STYLE} role="listbox">
          {matches.map((match) => (
            <li key={match.id}>
              <button
                type="button"
                style={ROW_BUTTON_STYLE}
                onClick={() => commit(match.id)}
              >
                <span aria-hidden="true">{kindIcon(match.kind)}</span>
                <span style={{ fontWeight: 500 }}>{match.display}</span>
                {match.magnitude !== null && (
                  <span style={ROW_MAG_STYLE}>V&nbsp;{match.magnitude.toFixed(2)}</span>
                )}
                {match.aka.length > 0 && <span style={ROW_AKA_STYLE}>{match.aka}</span>}
              </button>
            </li>
          ))}
        </ul>
      )}
      {selected !== null && (
        <div style={INFO_STYLE} role="region" aria-label="Selected object">
          <div style={INFO_TITLE_STYLE}>{selected.display}</div>
          {selected.aka.length > 0 && <div style={INFO_AKA_STYLE}>{selected.aka}</div>}
          <dl style={INFO_GRID_STYLE}>
            {selected.primaryId !== null && (
              <>
                <dt style={INFO_DT_STYLE}>ID</dt>
                <dd style={INFO_DD_STYLE}>
                  <button
                    type="button"
                    style={ID_COPY_STYLE}
                    title="Copy identifier"
                    aria-label={`Copy identifier ${selected.primaryId}`}
                    onClick={() => {
                      void navigator.clipboard?.writeText(selected.primaryId ?? "");
                    }}
                  >
                    {selected.primaryId}
                  </button>
                </dd>
              </>
            )}
            <dt style={INFO_DT_STYLE}>RA (J2000)</dt>
            <dd style={INFO_DD_STYLE}>{formatRa(selected.raRad)}</dd>
            <dt style={INFO_DT_STYLE}>Dec (J2000)</dt>
            <dd style={INFO_DD_STYLE}>{formatDec(selected.decRad)}</dd>
            <dt style={INFO_DT_STYLE}>Apparent</dt>
            <dd style={INFO_DD_STYLE}>{formatAltAz(selected.altitudeRad, selected.azimuthRad)}</dd>
            {selected.magnitude !== null && (
              <>
                <dt style={INFO_DT_STYLE}>Magnitude</dt>
                <dd style={INFO_DD_STYLE}>V&nbsp;{selected.magnitude.toFixed(2)}</dd>
              </>
            )}
            {selected.distance !== null && (
              <>
                <dt style={INFO_DT_STYLE}>Distance</dt>
                <dd style={INFO_DD_STYLE}>
                  {selected.distance.value.toFixed(selected.distance.unit === "km" ? 0 : 3)}
                  &nbsp;{selected.distance.unit}
                </dd>
              </>
            )}
            {selected.riseSetMs !== null && (
              <>
                <dt style={INFO_DT_STYLE}>Rise · Transit · Set</dt>
                <dd style={INFO_DD_STYLE}>
                  {formatTime(selected.riseSetMs.rise)} ·{" "}
                  {formatTime(selected.riseSetMs.transit)} ·{" "}
                  {formatTime(selected.riseSetMs.set)}
                </dd>
              </>
            )}
          </dl>
          {selected.variable != null && (
            <div style={VARIABLE_BLOCK_STYLE}>
              <div style={VARIABLE_TITLE_STYLE}>
                Variable star · {selected.variable.type} · P&nbsp;
                {selected.variable.periodDays.toFixed(selected.variable.periodDays < 10 ? 4 : 1)}&nbsp;d
              </div>
              <VariableLightCurve variable={selected.variable} />
              <div style={VARIABLE_CAPTION_STYLE}>
                Predicted V&nbsp;{selected.variable.currentMagnitude.toFixed(2)} (Δm&nbsp;
                {selected.variable.deltaMagnitude.toFixed(2)}, phase&nbsp;
                {selected.variable.phase.toFixed(2)}) · range V&nbsp;
                {selected.variable.magBright.toFixed(1)}–{selected.variable.magFaint.toFixed(1)} ·{" "}
                {selected.variable.reference}. Elements from AAVSO VSX / GCVS; phase-folded prediction
                (epoch {selected.variable.epochJd.toFixed(0)}).
              </div>
            </div>
          )}
          {(selected.simbadUrl !== null || selected.vizierUrl !== null) && (
            <div style={LINKS_BLOCK_STYLE}>
              <label style={LINKS_TOGGLE_STYLE}>
                <input
                  type="checkbox"
                  checked={showLinks}
                  onChange={(event) => {
                    const next = event.target.checked;
                    setShowLinks(next);
                    saveExternalLinksPref(next);
                  }}
                />
                External catalogue links
              </label>
              {showLinks && (
                <div style={LINKS_ROW_STYLE}>
                  {selected.simbadUrl !== null && (
                    <a
                      style={LINK_STYLE}
                      href={selected.simbadUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      SIMBAD ↗
                    </a>
                  )}
                  {selected.vizierUrl !== null && (
                    <a
                      style={LINK_STYLE}
                      href={selected.vizierUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      VizieR ↗
                    </a>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
