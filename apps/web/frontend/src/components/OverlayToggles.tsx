import {
  OVERLAY_LABELS,
  OVERLAY_LAYERS,
  type OverlayConfig,
  type OverlayLayer,
} from "../observer";

const EDITABLE_OVERLAY_LAYERS = OVERLAY_LAYERS.filter(
  (layer): layer is Exclude<OverlayLayer, "cardinals"> => layer !== "cardinals",
);

type Props = {
  config: OverlayConfig;
  onChange: (next: OverlayConfig) => void;
};

/// Checkbox list of overlay layers plus grid-step and opacity sliders. The
/// layer set and naming mirror the CLI's `--overlays` flag exactly, so users
/// who consult the CLI help find the same vocabulary here.
export function OverlayToggles({ config, onChange }: Props) {
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
    <>
      <div style={{ display: "grid", gap: 4 }}>
        {EDITABLE_OVERLAY_LAYERS.map((layer) => {
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
              <span style={{ opacity: checked ? 1 : 0.65 }}>{OVERLAY_LABELS[layer]}</span>
            </label>
          );
        })}
      </div>

      <div style={{ marginTop: 12, display: "grid", gap: 8 }}>
        <Slider
          label="Grid step"
          value={config.gridStepDeg}
          min={5}
          max={45}
          step={1}
          suffix="°"
          onChange={(v) => onChange({ ...config, gridStepDeg: v })}
        />
        <Slider
          label="Opacity"
          value={config.opacity}
          min={0}
          max={1}
          step={0.05}
          format={(v) => v.toFixed(2)}
          onChange={(v) => onChange({ ...config, opacity: v })}
        />
      </div>
    </>
  );
}

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
