import type { CSSProperties } from "react";

import {
  MAX_DEEP_SKY_MAGNITUDE_LIMIT,
  MIN_DEEP_SKY_MAGNITUDE_LIMIT,
  OVERLAY_LAYERS,
  OVERLAY_PALETTES,
  type OverlayConfig,
  type OverlayLayer,
  type OverlayPalette,
} from "../observer";
import { useT } from "../i18n";

type EditableOverlayLayer = Exclude<OverlayLayer, "cardinals">;

type OverlayGroup = {
  titleKey: string;
  descriptionKey: string;
  layers: EditableOverlayLayer[];
};

const EDITABLE_OVERLAY_LAYERS = OVERLAY_LAYERS.filter(
  (layer): layer is EditableOverlayLayer => layer !== "cardinals",
);

const OVERLAY_GROUPS: OverlayGroup[] = [
  {
    titleKey: "overlayGroup.referenceGeometry.title",
    descriptionKey: "overlayGroup.referenceGeometry.description",
    layers: [
      "horizon",
      "alt-az-grid",
      "equatorial-grid",
      "ecliptic",
      "celestial-equator",
      "meridian",
      "galactic-equator",
    ],
  },
  {
    titleKey: "overlayGroup.constellations.title",
    descriptionKey: "overlayGroup.constellations.description",
    layers: ["constellation-lines", "constellation-boundaries"],
  },
  {
    titleKey: "overlayGroup.deepSky.title",
    descriptionKey: "overlayGroup.deepSky.description",
    layers: ["deep-sky-objects", "deep-sky-labels"],
  },
  {
    titleKey: "overlayGroup.labels.title",
    descriptionKey: "overlayGroup.labels.description",
    layers: [
      "star-labels",
      "planet-labels",
      "constellation-labels",
      "cardinal-labels",
      "degree-labels",
    ],
  },
];

type Props = {
  config: OverlayConfig;
  onChange: (next: OverlayConfig) => void;
};

/// Checkbox list of overlay layers plus grid-step and opacity sliders. The web
/// UI exposes the useful line/grid overlays while hiding legacy cardinal marks.
export function OverlayToggles({ config, onChange }: Props) {
  const t = useT();

  const toggle = (layer: OverlayLayer) => {
    const has = config.layers.includes(layer);
    const next = has
      ? config.layers.filter((l) => l !== layer && l !== "cardinals")
      // Preserve the canonical layer order on insert so the wasm call site is
      // deterministic regardless of toggle history. Hidden legacy layers are
      // omitted from web UI edits.
      : EDITABLE_OVERLAY_LAYERS.filter((l) => l === layer || config.layers.includes(l));
    onChange({ ...config, layers: next });
  };

  return (
    <div style={{ display: "grid", gap: 14 }}>
      {OVERLAY_GROUPS.map((group) => (
        <section key={group.titleKey} style={groupStyle}>
          <div style={groupLegendStyle}>{t(group.titleKey)}</div>
          <p style={groupDescriptionStyle}>{t(group.descriptionKey)}</p>
          <div style={{ display: "grid", gap: 4 }}>
            {group.layers.map((layer) => {
              const checked = config.layers.includes(layer);
              const id = `overlay-${layer}`;
              return (
                <label
                  key={layer}
                  htmlFor={id}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                    cursor: "pointer",
                    padding: "2px 0",
                  }}
                >
                  <input
                    id={id}
                    type="checkbox"
                    checked={checked}
                    onChange={() => toggle(layer)}
                    style={{ accentColor: "#8fb1ff" }}
                  />
                  <span style={{ opacity: checked ? 1 : 0.65 }}>{t(`overlay.${layer}`)}</span>
                </label>
              );
            })}
          </div>
        </section>
      ))}

      <section style={groupStyle}>
        <div style={groupLegendStyle}>{t("overlayGroup.lineStyling.title")}</div>
        <div style={{ marginTop: 8, display: "grid", gap: 8 }}>
          <Slider
            label={t("overlayGroup.gridStep")}
            value={config.gridStepDeg}
            min={5}
            max={45}
            step={1}
            suffix="°"
            onChange={(v) => onChange({ ...config, gridStepDeg: v })}
          />
          <Slider
            label={t("overlayGroup.lineOpacity")}
            value={config.opacity}
            min={0}
            max={1}
            step={0.05}
            format={(v) => v.toFixed(2)}
            onChange={(v) => onChange({ ...config, opacity: v })}
          />
        </div>
      </section>

      <section style={groupStyle}>
        <div style={groupLegendStyle}>{t("overlayGroup.deepSkyDensity.title")}</div>
        <p style={groupDescriptionStyle}>{t("overlayGroup.deepSkyDensity.description")}</p>
        <div style={{ marginTop: 4, display: "grid", gap: 8 }}>
          <Slider
            label={t("overlayGroup.deepSkyMagnitudeLimit")}
            value={config.deepSkyMagnitudeLimit}
            min={MIN_DEEP_SKY_MAGNITUDE_LIMIT}
            max={MAX_DEEP_SKY_MAGNITUDE_LIMIT}
            step={0.5}
            format={(v) => v.toFixed(1)}
            onChange={(v) => onChange({ ...config, deepSkyMagnitudeLimit: v })}
          />
        </div>
      </section>

      <section style={groupStyle}>
        <div style={groupLegendStyle}>{t("overlayGroup.palette.title")}</div>
        <p style={groupDescriptionStyle}>{t("overlayGroup.palette.description")}</p>
        <div style={{ marginTop: 4, display: "grid", gap: 6 }}>
          <label htmlFor="overlay-palette" style={paletteLabelStyle}>
            {t("overlayGroup.palette.label")}
          </label>
          <select
            id="overlay-palette"
            value={config.palette}
            onChange={(e) =>
              onChange({ ...config, palette: e.target.value as OverlayPalette })
            }
            style={paletteSelectStyle}
          >
            {OVERLAY_PALETTES.map((p) => (
              <option key={p} value={p}>
                {t(`overlayPalette.${p}`)}
              </option>
            ))}
          </select>
        </div>
      </section>
    </div>
  );
}

/// Flat sub-section: no box; the parent SettingCard already provides the
/// visual frame. Sibling sections are separated by the parent grid `gap`.
const groupStyle: CSSProperties = {};

const groupLegendStyle: CSSProperties = {
  color: "#dbe7ff",
  fontSize: 11,
  letterSpacing: 0.45,
  textTransform: "uppercase",
};

const groupDescriptionStyle: CSSProperties = {
  margin: "0 0 7px",
  opacity: 0.55,
  fontSize: 11,
};

const paletteLabelStyle: CSSProperties = {
  fontSize: 12,
  opacity: 0.8,
};

const paletteSelectStyle: CSSProperties = {
  width: "100%",
  background: "#0c1424",
  color: "#e8eefc",
  border: "1px solid #2a3650",
  borderRadius: 6,
  padding: "6px 8px",
  fontSize: 13,
};

function Slider({
  label,
  value,
  min,
  max,
  step,
  suffix,
  format,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  suffix?: string;
  format?: (v: number) => string;
  onChange: (v: number) => void;
}) {
  const shown = format ? format(value) : value.toString();
  return (
    <label style={{ display: "grid", gridTemplateColumns: "auto 1fr auto", gap: 8, alignItems: "center" }}>
      <span style={{ opacity: 0.7 }}>{label}</span>
      <input
        type="range"
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{ width: "100%" }}
      />
      <span style={{ minWidth: 44, textAlign: "right", opacity: 0.7 }}>
        {shown}
        {suffix}
      </span>
    </label>
  );
}
