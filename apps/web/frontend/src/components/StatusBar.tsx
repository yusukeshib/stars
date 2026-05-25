import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import {
  ATMOSPHERE_PRESET_DEFAULTS,
  ATMOSPHERE_PRESET_LABELS,
  ATMOSPHERE_PRESETS,
  SKY_PROJECTION_LABELS,
  SKY_PROJECTIONS,
  type AtmosphereConfig,
  type AtmospherePreset,
  type Observer,
  type OverlayConfig,
  type PlanetsConfig,
  type PlanningTable,
  type ProjectionConfig,
  type SkyProjection,
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
  planets: PlanetsConfig;
  projection: ProjectionConfig;
  planning: PlanningTable | null;
  onSetObserver: (next: Observer) => void;
  onSetTime: (timeMs: number) => void;
  onSetOverlays: (next: OverlayConfig) => void;
  onSetAtmosphere: (next: AtmosphereConfig) => void;
  onSetPlanets: (next: PlanetsConfig) => void;
  onSetProjection: (next: ProjectionConfig) => void;
  onCopySessionUrl: () => void | Promise<void>;
  onUseGeolocation: () => void;
};

type Popover = "location" | "time" | "settings";
type AddressLookupStatus = "idle" | "loading" | "success" | "error";
type TimeDragTarget = "date" | "clock";

type AddressLookupState = {
  status: AddressLookupStatus;
  message: string | null;
};

type NominatimPlace = {
  lat?: string;
  lon?: string;
  display_name?: string;
  name?: string;
};

type TimeDragState = {
  pointerId: number;
  target: TimeDragTarget;
  startX: number;
  baseTimeMs: number;
  lastStep: number;
  moved: boolean;
  element: HTMLElement;
};

const CLOCK_DRAG_STEP_MS = 10 * 60 * 1000;
const TIME_DRAG_PX_PER_STEP = 24;
const TIME_DRAG_CLICK_SLOP_PX = 4;
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

function fmtDate(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function fmtClock(ms: number): string {
  const d = new Date(ms);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
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

function addLocalDays(ms: number, days: number): number {
  const d = new Date(ms);
  d.setDate(d.getDate() + days);
  return d.getTime();
}

function applyTimeDragStep(ms: number, target: TimeDragTarget, steps: number): number {
  return target === "date" ? addLocalDays(ms, steps) : ms + steps * CLOCK_DRAG_STEP_MS;
}

const clamp = (value: number, min: number, max: number) =>
  Math.max(min, Math.min(max, value));

function firstGeocodingResult(value: unknown): NominatimPlace | null {
  if (!Array.isArray(value)) return null;
  const first = value[0];
  if (typeof first !== "object" || first === null) return null;
  return first as NominatimPlace;
}

/// Interactive status strip in the bottom-left. Location, time, and settings
/// open lightweight popups for common changes without covering the sky.
export function StatusBar({
  observer,
  view,
  timeMs,
  sunAltitudeDeg,
  overlays,
  atmosphere,
  planets,
  projection,
  planning,
  onSetObserver,
  onSetTime,
  onSetOverlays,
  onSetAtmosphere,
  onSetPlanets,
  onSetProjection,
  onCopySessionUrl,
  onUseGeolocation,
}: Props) {
  const [openPopover, setOpenPopover] = useState<Popover | null>(null);
  const [addressQuery, setAddressQuery] = useState("");
  const [addressLookup, setAddressLookup] = useState<AddressLookupState>({
    status: "idle",
    message: null,
  });
  const rootRef = useRef<HTMLDivElement>(null);
  const timeDragRef = useRef<TimeDragState | null>(null);
  const suppressNextTimeClick = useRef(false);

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
  const lookupAddress = async () => {
    const query = addressQuery.trim();
    if (!query) {
      setAddressLookup({ status: "error", message: "Enter an address or place name." });
      return;
    }

    setAddressLookup({ status: "loading", message: "Searching address…" });
    try {
      const params = new URLSearchParams({
        q: query,
        format: "jsonv2",
        limit: "1",
      });
      const response = await fetch(`https://nominatim.openstreetmap.org/search?${params}`, {
        headers: { Accept: "application/json" },
      });
      if (!response.ok) throw new Error(`geocoding request failed: ${response.status}`);

      const place = firstGeocodingResult(await response.json());
      if (!place) {
        setAddressLookup({ status: "error", message: "No matching place found." });
        return;
      }

      const latitudeDeg = Number(place.lat);
      const longitudeDeg = Number(place.lon);
      if (!Number.isFinite(latitudeDeg) || !Number.isFinite(longitudeDeg)) {
        throw new Error("geocoding result is missing coordinates");
      }

      onSetObserver({
        ...observer,
        latitudeDeg: clamp(latitudeDeg, -90, 90),
        longitudeDeg: clamp(longitudeDeg, -180, 180),
      });
      setAddressLookup({
        status: "success",
        message: place.display_name ?? place.name ?? "Location updated from address.",
      });
    } catch {
      setAddressLookup({
        status: "error",
        message: "Address lookup failed. Check your connection and try again.",
      });
    }
  };

  const beginTimeDrag = (
    event: ReactPointerEvent<HTMLSpanElement>,
    target: TimeDragTarget,
  ) => {
    if (event.button !== 0) return;
    const element = event.currentTarget;
    element.setPointerCapture(event.pointerId);
    timeDragRef.current = {
      pointerId: event.pointerId,
      target,
      startX: event.clientX,
      baseTimeMs: timeMs,
      lastStep: 0,
      moved: false,
      element,
    };
  };

  const updateTimeDrag = (event: ReactPointerEvent<HTMLSpanElement>) => {
    const drag = timeDragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;

    const deltaX = event.clientX - drag.startX;
    if (Math.abs(deltaX) >= TIME_DRAG_CLICK_SLOP_PX) drag.moved = true;

    const steps = Math.trunc(deltaX / TIME_DRAG_PX_PER_STEP);
    if (steps === drag.lastStep) return;
    drag.lastStep = steps;
    onSetTime(applyTimeDragStep(drag.baseTimeMs, drag.target, steps));
  };

  const endTimeDrag = (event: ReactPointerEvent<HTMLSpanElement>) => {
    const drag = timeDragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;

    if (drag.moved) suppressNextTimeClick.current = true;
    if (drag.element.hasPointerCapture(event.pointerId)) {
      drag.element.releasePointerCapture(event.pointerId);
    }
    timeDragRef.current = null;
  };

  const toggleTimePopover = () => {
    if (suppressNextTimeClick.current) {
      suppressNextTimeClick.current = false;
      return;
    }
    setOpenPopover(openPopover === "time" ? null : "time");
  };

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

          <form
            onSubmit={(e) => {
              e.preventDefault();
              void lookupAddress();
            }}
            style={addressLookupFormStyle}
          >
            <label htmlFor="quick-address" style={labelStyle}>
              Address / place lookup
            </label>
            <div style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 8 }}>
              <input
                id="quick-address"
                type="search"
                value={addressQuery}
                placeholder="Tokyo Tower, Paris, Mauna Kea…"
                onChange={(e) => {
                  setAddressQuery(e.target.value);
                  if (addressLookup.status !== "idle") {
                    setAddressLookup({ status: "idle", message: null });
                  }
                }}
                style={inputStyle}
              />
              <button
                type="submit"
                disabled={addressLookup.status === "loading"}
                style={buttonStyle}
              >
                {addressLookup.status === "loading" ? "Finding…" : "Find"}
              </button>
            </div>
            {addressLookup.message && (
              <p
                role={addressLookup.status === "error" ? "alert" : "status"}
                style={lookupMessageStyle(addressLookup.status)}
              >
                {addressLookup.message}
              </p>
            )}
          </form>

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
            The clock keeps ticking after each quick change. Drag the date in the status bar
            left/right by one day, or the time by 10 minutes.
          </p>
        </PopoverPanel>
      )}

      {openPopover === "settings" && (
        <PopoverPanel title="Settings" onClose={() => setOpenPopover(null)}>
          <SettingsPanel
            overlays={overlays}
            atmosphere={atmosphere}
            planets={planets}
            projection={projection}
            planning={planning}
            onSetOverlays={onSetOverlays}
            onSetAtmosphere={onSetAtmosphere}
            onSetPlanets={onSetPlanets}
            onSetProjection={onSetProjection}
            onCopySessionUrl={onCopySessionUrl}
          />
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
            onClick={toggleTimePopover}
            style={chipButtonStyle(openPopover === "time")}
          >
            <span style={mutedStyle}>Time </span>
            <span
              title="Drag left/right to change the date by one day per step"
              onPointerDown={(e) => beginTimeDrag(e, "date")}
              onPointerMove={updateTimeDrag}
              onPointerUp={endTimeDrag}
              onPointerCancel={endTimeDrag}
              style={draggableTimeValueStyle(openPopover === "time")}
            >
              {fmtDate(timeMs)}
            </span>
            <span style={mutedStyle}> </span>
            <span
              title="Drag left/right to change the time by 10 minutes per step"
              onPointerDown={(e) => beginTimeDrag(e, "clock")}
              onPointerMove={updateTimeDrag}
              onPointerUp={endTimeDrag}
              onPointerCancel={endTimeDrag}
              style={draggableTimeValueStyle(openPopover === "time")}
            >
              {fmtClock(timeMs)}
            </span>
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
          {projection.projection === "perspective" ? fmtDeg(view.fovDeg) : "full sky"}
          <span style={separatorStyle}>  ·  </span>
          <span style={mutedStyle}>Projection </span>
          {SKY_PROJECTION_LABELS[projection.projection]}
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

type SettingsPanelProps = Pick<
  Props,
  | "overlays"
  | "atmosphere"
  | "planets"
  | "projection"
  | "planning"
  | "onSetOverlays"
  | "onSetAtmosphere"
  | "onSetPlanets"
  | "onSetProjection"
  | "onCopySessionUrl"
>;

function SettingsPanel({
  overlays,
  atmosphere,
  planets,
  projection,
  planning,
  onSetOverlays,
  onSetAtmosphere,
  onSetPlanets,
  onSetProjection,
  onCopySessionUrl,
}: SettingsPanelProps) {
  const setAtmospherePreset = (preset: AtmospherePreset) => {
    onSetAtmosphere({
      ...atmosphere,
      preset,
      ...ATMOSPHERE_PRESET_DEFAULTS[preset],
    });
  };

  return (
    <div style={{ display: "grid", gap: 10 }}>
      <SettingCard
        title="View & objects"
        description="Choose the map projection and the solar-system bodies drawn with the stars."
      >
        <label style={checkboxRowStyle}>
          <input
            type="checkbox"
            checked={planets.enabled}
            onChange={(e) => onSetPlanets({ enabled: e.target.checked })}
            style={{ accentColor: "#8fb1ff" }}
          />
          Mercury → Neptune
        </label>

        <label htmlFor="sky-projection" style={{ ...labelStyle, marginTop: 10 }}>
          Screen projection
        </label>
        <select
          id="sky-projection"
          value={projection.projection}
          onChange={(e) => onSetProjection({ projection: e.target.value as SkyProjection })}
          style={{ ...inputStyle, width: "100%" }}
        >
          {SKY_PROJECTIONS.map((p) => (
            <option key={p} value={p}>
              {SKY_PROJECTION_LABELS[p]}
            </option>
          ))}
        </select>
        <p style={helperTextStyle}>
          Full-sky maps ignore FOV but still rotate with azimuth/altitude.
        </p>
      </SettingCard>

      <SettingCard
        title="Overlays"
        description="Reference lines and labels are grouped by purpose so it is easier to find what to turn on."
      >
        <OverlayToggles config={overlays} onChange={onSetOverlays} />
      </SettingCard>

      {planning && <PlanningPanel planning={planning} />}

      <SettingCard
        title="Atmosphere & extinction"
        description="Model sky colour, haze, refraction, and local air conditions."
      >
        <label style={{ ...checkboxRowStyle, marginBottom: 10 }}>
          <input
            type="checkbox"
            checked={atmosphere.enabled}
            onChange={(e) => onSetAtmosphere({ ...atmosphere, enabled: e.target.checked })}
            style={{ accentColor: "#8fb1ff" }}
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

        <div style={advancedControlGridStyle}>
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
        </div>
      </SettingCard>

      <SettingCard
        title="Session"
        description="Share this exact location, time, projection, and display setup."
      >
        <button type="button" onClick={onCopySessionUrl} style={buttonStyle}>
          Copy session URL
        </button>
        <p style={{ ...helperTextStyle, marginTop: 10 }}>
          Drag the sky to look around · scroll to zoom
        </p>
      </SettingCard>
    </div>
  );
}

function SettingCard({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <section style={settingCardStyle}>
      <div style={settingCardTitleStyle}>{title}</div>
      <p style={settingCardDescriptionStyle}>{description}</p>
      {children}
    </section>
  );
}

function fmtEventTime(ms: number | null): string {
  if (ms === null || !Number.isFinite(ms)) return "—";
  const d = new Date(ms);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function PlanningPanel({ planning }: { planning: PlanningTable }) {
  return (
    <SettingCard
      title="Planning"
      description="Tonight's rise, transit, set, and twilight windows for major objects."
    >
      <div style={{ maxHeight: 170, overflow: "auto", borderTop: "1px solid rgba(255,255,255,0.08)" }}>
        {planning.rows.map((row) => (
          <div
            key={row.name}
            style={{
              display: "grid",
              gridTemplateColumns: "74px repeat(3, 46px) 48px",
              gap: 6,
              padding: "3px 0",
              borderBottom: "1px solid rgba(255,255,255,0.06)",
            }}
          >
            <span>{row.name}</span>
            <span title="Rise">↑ {fmtEventTime(row.riseMs)}</span>
            <span title="Transit">↟ {fmtEventTime(row.transitMs)}</span>
            <span title="Set">↓ {fmtEventTime(row.setMs)}</span>
            <span>{row.transitAltitudeDeg === null ? "—" : `${row.transitAltitudeDeg.toFixed(0)}°`}</span>
          </div>
        ))}
      </div>
      <div style={{ marginTop: 8, display: "grid", gap: 3 }}>
        {planning.twilight.map((segment) => (
          <div key={`${segment.label}-${segment.startMs}`} style={{ opacity: 0.72 }}>
            {segment.label}: {fmtEventTime(segment.startMs)}–{fmtEventTime(segment.endMs)}
          </div>
        ))}
      </div>
    </SettingCard>
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

const draggableTimeValueStyle = (active: boolean): React.CSSProperties => ({
  ...underlinedValueStyle(active),
  cursor: "ew-resize",
  touchAction: "none",
  userSelect: "none",
});

const popoverStyle: React.CSSProperties = {
  position: "absolute",
  left: 0,
  bottom: "calc(100% + 10px)",
  width: "min(420px, calc(100vw - 28px))",
  maxHeight: "calc(100vh - 110px)",
  overflowY: "auto",
  overscrollBehavior: "contain",
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

const addressLookupFormStyle: React.CSSProperties = {
  margin: "12px 0 10px",
  padding: "10px",
  background: "rgba(255, 255, 255, 0.035)",
  border: "1px solid rgba(255, 255, 255, 0.08)",
  borderRadius: 8,
};

const lookupMessageStyle = (status: AddressLookupStatus): React.CSSProperties => ({
  margin: "8px 0 0",
  fontSize: 11,
  opacity: status === "error" ? 0.85 : 0.62,
  color: status === "error" ? "#ffb4a8" : "#cfd8e3",
});

const helperTextStyle: React.CSSProperties = {
  margin: "8px 0 0",
  fontSize: 11,
  opacity: 0.55,
};

const checkboxRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  cursor: "pointer",
};

const settingCardStyle: React.CSSProperties = {
  padding: "11px 12px 12px",
  background: "rgba(255, 255, 255, 0.035)",
  border: "1px solid rgba(255, 255, 255, 0.09)",
  borderRadius: 10,
};

const settingCardTitleStyle: React.CSSProperties = {
  color: "#dbe7ff",
  fontSize: 11,
  letterSpacing: 0.65,
  textTransform: "uppercase",
};

const settingCardDescriptionStyle: React.CSSProperties = {
  margin: "4px 0 10px",
  opacity: 0.55,
  fontSize: 11,
};

const advancedControlGridStyle: React.CSSProperties = {
  marginTop: 2,
  display: "grid",
  gap: 2,
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
