import { useEffect, useRef, useState } from "react";
import {
  ATMOSPHERE_PRESET_DEFAULTS,
  ATMOSPHERE_PRESET_LABELS,
  ATMOSPHERE_PRESETS,
  type AtmosphereConfig,
  type AtmospherePreset,
  type Observer,
  type OverlayConfig,
  type View,
} from "../observer";
import { OverlayToggles } from "./OverlayToggles";

type Props = {
  observer: Observer;
  view: View;
  timeMs: number;
  sunAltitudeDeg: number | null;
  overlays: OverlayConfig;
  atmosphere: AtmosphereConfig;
  onSetObserver: (next: Observer) => void;
  onSetTime: (timeMs: number) => void;
  onSetOverlays: (next: OverlayConfig) => void;
  onSetAtmosphere: (next: AtmosphereConfig) => void;
  onUseGeolocation: () => void;
};

type Popover = "location" | "time" | "settings";

const fmtDeg = (n: number) => `${n.toFixed(1)}°`;
const COMPASS_DIRS = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
const compass = (az: number) => COMPASS_DIRS[Math.round(az / 45) % 8];
const pad = (n: number) => n.toString().padStart(2, "0");
function twilightLabel(sunAltDeg: number | null): string {
  if (sunAltDeg === null) return "Sky model initializing";
  if (sunAltDeg >= 0) return `Daylight (Sun ${sunAltDeg.toFixed(1)}°)`;
  if (sunAltDeg >= -6) return `Civil twilight (Sun ${sunAltDeg.toFixed(1)}°)`;
  if (sunAltDeg >= -12) return `Nautical twilight (Sun ${sunAltDeg.toFixed(1)}°)`;
  if (sunAltDeg >= -18) return `Astronomical twilight (Sun ${sunAltDeg.toFixed(1)}°)`;
  return `Night (Sun ${sunAltDeg.toFixed(1)}°)`;
}

function fmtTime(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function toLocalDateInput(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function toLocalDatetimeInput(ms: number): string {
  const d = new Date(ms);
  return `${toLocalDateInput(ms)}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function setLocalDatePart(ms: number, dateValue: string): number | null {
  const [year, month, day] = dateValue.split("-").map(Number);
  if (!year || !month || !day) return null;
  const d = new Date(ms);
  d.setFullYear(year, month - 1, day);
  return d.getTime();
}

function setLocalTimePart(ms: number, hour: number, minute: number): number {
  const d = new Date(ms);
  d.setHours(hour, minute, 0, 0);
  return d.getTime();
}

const clamp = (value: number, min: number, max: number) =>
  Math.max(min, Math.min(max, value));

/// Interactive status strip in the bottom-left. Location, time, and settings
/// open lightweight popups for common changes without covering the sky.
export function StatusBar({
  observer,
  view,
  timeMs,
  sunAltitudeDeg,
  overlays,
  atmosphere,
  onSetObserver,
  onSetTime,
  onSetOverlays,
  onSetAtmosphere,
  onUseGeolocation,
}: Props) {
  const [openPopover, setOpenPopover] = useState<Popover | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!openPopover) return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpenPopover(null);
    };

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [openPopover]);

  const setLatitude = (latitudeDeg: number) => {
    onSetObserver({ ...observer, latitudeDeg: clamp(latitudeDeg, -90, 90) });
  };
  const setLongitude = (longitudeDeg: number) => {
    onSetObserver({ ...observer, longitudeDeg: clamp(longitudeDeg, -180, 180) });
  };
  const setAtmospherePreset = (preset: AtmospherePreset) => {
    onSetAtmosphere({
      ...atmosphere,
      preset,
      ...ATMOSPHERE_PRESET_DEFAULTS[preset],
    });
  };

  const time = new Date(timeMs);
  const hour = time.getHours();
  const minute = time.getMinutes();
  const twilight = twilightLabel(sunAltitudeDeg);

  return (
    <div ref={rootRef} style={containerStyle}>
      {openPopover === "location" && (
        <PopoverPanel title="Location" onClose={() => setOpenPopover(null)}>
          <SliderNumberRow
            id="quick-lat"
            label="Latitude"
            value={observer.latitudeDeg}
            min={-90}
            max={90}
            step={0.1}
            onChange={setLatitude}
          />
          <SliderNumberRow
            id="quick-lng"
            label="Longitude"
            value={observer.longitudeDeg}
            min={-180}
            max={180}
            step={0.1}
            onChange={setLongitude}
          />
          <button type="button" onClick={onUseGeolocation} style={buttonStyle}>
            Use my location
          </button>
        </PopoverPanel>
      )}

      {openPopover === "time" && (
        <PopoverPanel title="Time" onClose={() => setOpenPopover(null)}>
          <label htmlFor="quick-datetime" style={labelStyle}>
            Local date/time
          </label>
          <input
            id="quick-datetime"
            type="datetime-local"
            value={toLocalDatetimeInput(timeMs)}
            onChange={(e) => {
              const next = new Date(e.target.value).getTime();
              if (!Number.isNaN(next)) onSetTime(next);
            }}
            style={{ ...inputStyle, width: "100%" }}
          />

          <label htmlFor="quick-date" style={{ ...labelStyle, marginTop: 10 }}>
            Date picker
          </label>
          <input
            id="quick-date"
            type="date"
            value={toLocalDateInput(timeMs)}
            onChange={(e) => {
              const next = setLocalDatePart(timeMs, e.target.value);
              if (next !== null && !Number.isNaN(next)) onSetTime(next);
            }}
            style={{ ...inputStyle, width: "100%" }}
          />

          <SliderNumberRow
            id="quick-hour"
            label="Hour"
            value={hour}
            min={0}
            max={23}
            step={1}
            decimals={0}
            onChange={(nextHour) => onSetTime(setLocalTimePart(timeMs, nextHour, minute))}
          />
          <SliderNumberRow
            id="quick-minute"
            label="Minute"
            value={minute}
            min={0}
            max={59}
            step={1}
            decimals={0}
            onChange={(nextMinute) => onSetTime(setLocalTimePart(timeMs, hour, nextMinute))}
          />

          <div style={{ display: "flex", gap: 8, marginTop: 10 }}>
            <button type="button" onClick={() => onSetTime(Date.now())} style={buttonStyle}>
              Now
            </button>
            <button
              type="button"
              onClick={() => onSetTime(timeMs - 60 * 60 * 1000)}
              style={buttonStyle}
            >
              −1h
            </button>
            <button
              type="button"
              onClick={() => onSetTime(timeMs + 60 * 60 * 1000)}
              style={buttonStyle}
            >
              +1h
            </button>
          </div>
          <p style={{ margin: "10px 0 0", fontSize: 11, opacity: 0.55 }}>
            The clock keeps ticking after each quick change.
          </p>
        </PopoverPanel>
      )}

      {openPopover === "settings" && (
        <PopoverPanel title="Settings" onClose={() => setOpenPopover(null)}>
          <Section label="OVERLAYS">
            <OverlayToggles config={overlays} onChange={onSetOverlays} />
          </Section>
          <Section label="ATMOSPHERE">
            <label style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 10 }}>
              <input
                type="checkbox"
                checked={atmosphere.enabled}
                onChange={(e) => onSetAtmosphere({ ...atmosphere, enabled: e.target.checked })}
              />
              Atmosphere / extinction
            </label>

            <label htmlFor="atmosphere-preset" style={labelStyle}>
              Preset
            </label>
            <select
              id="atmosphere-preset"
              value={atmosphere.preset}
              onChange={(e) => setAtmospherePreset(e.target.value as AtmospherePreset)}
              disabled={!atmosphere.enabled}
              style={{ ...inputStyle, width: "100%" }}
            >
              {ATMOSPHERE_PRESETS.map((preset) => (
                <option key={preset} value={preset}>
                  {ATMOSPHERE_PRESET_LABELS[preset]}
                </option>
              ))}
            </select>

            <SliderNumberRow
              id="atmosphere-turbidity"
              label="Turbidity"
              value={atmosphere.turbidity}
              min={1.7}
              max={10}
              step={0.1}
              decimals={1}
              disabled={!atmosphere.enabled}
              onChange={(turbidity) =>
                onSetAtmosphere({ ...atmosphere, turbidity: clamp(turbidity, 1.7, 10) })
              }
            />
            <SliderNumberRow
              id="atmosphere-altitude"
              label="Observer altitude (m)"
              value={atmosphere.observerAltitudeM}
              min={0}
              max={9000}
              step={100}
              decimals={0}
              disabled={!atmosphere.enabled}
              onChange={(observerAltitudeM) =>
                onSetAtmosphere({
                  ...atmosphere,
                  observerAltitudeM: clamp(observerAltitudeM, 0, 9000),
                })
              }
            />
            <SliderNumberRow
              id="atmosphere-ozone"
              label="Ozone column (DU)"
              value={atmosphere.ozoneDu}
              min={0}
              max={600}
              step={25}
              decimals={0}
              disabled={!atmosphere.enabled}
              onChange={(ozoneDu) =>
                onSetAtmosphere({ ...atmosphere, ozoneDu: clamp(ozoneDu, 0, 600) })
              }
            />
            <SliderNumberRow
              id="atmosphere-visibility"
              label="Visibility (km)"
              value={atmosphere.visibilityKm}
              min={1}
              max={200}
              step={1}
              decimals={0}
              disabled={!atmosphere.enabled}
              onChange={(visibilityKm) =>
                onSetAtmosphere({ ...atmosphere, visibilityKm: clamp(visibilityKm, 1, 200) })
              }
            />
            <SliderNumberRow
              id="atmosphere-pressure"
              label="Pressure (hPa)"
              value={atmosphere.pressureHpa}
              min={0}
              max={1100}
              step={10}
              decimals={0}
              disabled={!atmosphere.enabled}
              onChange={(pressureHpa) =>
                onSetAtmosphere({ ...atmosphere, pressureHpa: clamp(pressureHpa, 0, 1100) })
              }
            />
            <SliderNumberRow
              id="atmosphere-temperature"
              label="Temperature (°C)"
              value={atmosphere.temperatureC}
              min={-80}
              max={60}
              step={1}
              decimals={0}
              disabled={!atmosphere.enabled}
              onChange={(temperatureC) =>
                onSetAtmosphere({ ...atmosphere, temperatureC: clamp(temperatureC, -80, 60) })
              }
            />
          </Section>
          <p style={{ margin: "14px 0 0", fontSize: 11, opacity: 0.45 }}>
            drag the sky to look around · scroll to zoom
          </p>
        </PopoverPanel>
      )}

      <div style={stripStyle}>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", justifyContent: "flex-start" }}>
          <button
            type="button"
            aria-expanded={openPopover === "location"}
            aria-haspopup="dialog"
            onClick={() => setOpenPopover(openPopover === "location" ? null : "location")}
            style={chipButtonStyle(openPopover === "location")}
          >
            <span style={mutedStyle}>Location </span>
            <span style={underlinedValueStyle(openPopover === "location")}>
              {observer.latitudeDeg.toFixed(2)}°, {observer.longitudeDeg.toFixed(2)}°
            </span>
          </button>
          <button
            type="button"
            aria-expanded={openPopover === "time"}
            aria-haspopup="dialog"
            onClick={() => setOpenPopover(openPopover === "time" ? null : "time")}
            style={chipButtonStyle(openPopover === "time")}
          >
            <span style={mutedStyle}>Time </span>
            <span style={underlinedValueStyle(openPopover === "time")}>{fmtTime(timeMs)}</span>
          </button>
        </div>
        <div style={{ marginTop: 4, textAlign: "left" }}>
          <span style={mutedStyle}>Az </span>
          {fmtDeg(view.azimuthDeg)} ({compass(view.azimuthDeg)})
          <span style={separatorStyle}>  ·  </span>
          <span style={mutedStyle}>Alt </span>
          {fmtDeg(view.altitudeDeg)}
          <span style={separatorStyle}>  ·  </span>
          <span style={mutedStyle}>FOV </span>
          {fmtDeg(view.fovDeg)}
          <span style={separatorStyle}>  ·  </span>
          <span style={mutedStyle}>Sky </span>
          {twilight}
          <span style={separatorStyle}>  ·  </span>
          <button
            type="button"
            aria-expanded={openPopover === "settings"}
            aria-haspopup="dialog"
            onClick={() => setOpenPopover(openPopover === "settings" ? null : "settings")}
            style={inlineButtonStyle}
          >
            <span style={underlinedValueStyle(openPopover === "settings")}>Settings</span>
          </button>
        </div>
      </div>
    </div>
  );
}

function PopoverPanel({
  title,
  children,
  onClose,
}: {
  title: string;
  children: React.ReactNode;
  onClose: () => void;
}) {
  return (
    <div role="dialog" aria-label={`${title} quick controls`} style={popoverStyle}>
      <header style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <div style={{ fontSize: 11, opacity: 0.65, letterSpacing: 0.8 }}>{title.toUpperCase()}</div>
        <button type="button" aria-label={`Close ${title} quick controls`} onClick={onClose} style={closeButtonStyle}>
          ×
        </button>
      </header>
      <div style={{ marginTop: 10 }}>{children}</div>
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

function SliderNumberRow({
  id,
  label,
  value,
  min,
  max,
  step,
  decimals = 2,
  disabled = false,
  onChange,
}: {
  id: string;
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  decimals?: number;
  disabled?: boolean;
  onChange: (value: number) => void;
}) {
  const inputValue = decimals === 0 ? String(Math.round(value)) : value.toFixed(decimals);

  const handleChange = (raw: string) => {
    if (raw === "") return;
    const next = Number(raw);
    if (Number.isFinite(next)) onChange(next);
  };

  return (
    <div style={{ marginTop: 10 }}>
      <label htmlFor={id} style={labelStyle}>
        {label}
      </label>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 76px", gap: 8, alignItems: "center" }}>
        <input
          id={id}
          type="range"
          min={min}
          max={max}
          step={step}
          value={value}
          disabled={disabled}
          onChange={(e) => handleChange(e.target.value)}
          style={{ width: "100%" }}
        />
        <input
          aria-label={`${label} value`}
          type="number"
          min={min}
          max={max}
          step={step}
          value={inputValue}
          disabled={disabled}
          onChange={(e) => handleChange(e.target.value)}
          style={inputStyle}
        />
      </div>
    </div>
  );
}

const containerStyle: React.CSSProperties = {
  position: "absolute",
  left: 14,
  bottom: 14,
  zIndex: 5,
  color: "#cfd8e3",
  font: "12px/1.4 ui-monospace, SFMono-Regular, Menlo, monospace",
  userSelect: "none",
};

const stripStyle: React.CSSProperties = {
  padding: "8px 10px",
  background: "rgba(10, 12, 22, 0.58)",
  borderRadius: 8,
  backdropFilter: "blur(8px)",
  boxShadow: "0 2px 10px rgba(0,0,0,0.35)",
  opacity: 0.9,
};

const chipButtonStyle = (_active: boolean): React.CSSProperties => ({
  appearance: "none",
  border: 0,
  borderRadius: 3,
  padding: "1px 0",
  background: "transparent",
  color: "inherit",
  cursor: "pointer",
  font: "inherit",
});

const inlineButtonStyle: React.CSSProperties = {
  appearance: "none",
  border: 0,
  padding: 0,
  background: "transparent",
  color: "inherit",
  cursor: "pointer",
  font: "inherit",
};

const underlinedValueStyle = (active: boolean): React.CSSProperties => ({
  color: active ? "#d9e8ff" : "inherit",
  textDecoration: "underline",
  textDecorationColor: active ? "rgba(145, 190, 255, 0.9)" : "rgba(207, 216, 227, 0.55)",
  textUnderlineOffset: 3,
});

const popoverStyle: React.CSSProperties = {
  position: "absolute",
  left: 0,
  bottom: "calc(100% + 10px)",
  width: "min(360px, calc(100vw - 28px))",
  padding: "12px 14px 14px",
  background: "rgba(14, 18, 30, 0.96)",
  borderRadius: 12,
  boxShadow: "0 12px 34px rgba(0, 0, 0, 0.55)",
  backdropFilter: "blur(10px)",
};

const mutedStyle: React.CSSProperties = { opacity: 0.55 };
const separatorStyle: React.CSSProperties = { opacity: 0.4 };

const labelStyle: React.CSSProperties = {
  display: "block",
  marginBottom: 5,
  opacity: 0.7,
};

const inputStyle: React.CSSProperties = {
  minWidth: 0,
  background: "rgba(255, 255, 255, 0.07)",
  color: "#e6edf5",
  border: "1px solid rgba(255, 255, 255, 0.12)",
  borderRadius: 5,
  padding: "5px 7px",
  font: "inherit",
};

const buttonStyle: React.CSSProperties = {
  background: "rgba(80, 130, 220, 0.22)",
  color: "#e6edf5",
  border: "1px solid rgba(120, 160, 230, 0.35)",
  borderRadius: 5,
  padding: "6px 10px",
  cursor: "pointer",
  font: "inherit",
};

const closeButtonStyle: React.CSSProperties = {
  width: 24,
  height: 24,
  borderRadius: 6,
  background: "transparent",
  color: "#cfd8e3",
  border: "1px solid rgba(255, 255, 255, 0.12)",
  cursor: "pointer",
  font: "15px/1 ui-monospace, monospace",
  padding: 0,
};
