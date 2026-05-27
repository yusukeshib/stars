import { useEffect, useRef, useState } from "react";
import {
  ATMOSPHERE_PRESET_DEFAULTS,
  ATMOSPHERE_PRESETS,
  SKY_PROJECTIONS,
  SKY_VIEWPOINTS,
  clampAltitude,
  clampFov,
  eyepieceExitPupilMm,
  eyepieceMagnification,
  eyepiecePlateScaleArcsecPerMm,
  eyepieceTrueFieldDeg,
  wrapAzimuth,
  type AtmosphereConfig,
  type AtmospherePreset,
  type EyepieceConfig,
  type Observer,
  type OverlayConfig,
  type PlanetsConfig,
  type PlanningTable,
  type ProjectionConfig,
  type SkyProjection,
  type SkyViewpoint,
  type Vec3,
  type View,
} from "../observer";
import { OverlayToggles } from "./OverlayToggles";
import { useStepDrag } from "./useStepDrag";
import {
  translateWasmBody,
  translateWasmTwilight,
  useT,
  type Translator,
} from "../i18n";

type Props = {
  observer: Observer;
  view: View;
  timeMs: number;
  sunAltitudeDeg: number | null;
  overlays: OverlayConfig;
  atmosphere: AtmosphereConfig;
  planets: PlanetsConfig;
  projection: ProjectionConfig;
  eyepiece: EyepieceConfig;
  planning: PlanningTable | null;
  onSetObserver: (next: Observer) => void;
  onSetTime: (timeMs: number) => void;
  onSetOverlays: (next: OverlayConfig) => void;
  onSetAtmosphere: (next: AtmosphereConfig) => void;
  onSetPlanets: (next: PlanetsConfig) => void;
  onSetProjection: (next: ProjectionConfig) => void;
  onSetEyepiece: (next: EyepieceConfig) => void;
  onSetView: (next: View) => void;
  onCopySessionUrl: () => void | Promise<void>;
  onCopySessionJson: () => void | Promise<void>;
  onImportSessionJson: (raw: string) => void;
  onUseGeolocation: () => void;
};

type Popover = "location" | "time" | "settings";
type AddressLookupStatus = "idle" | "loading" | "success" | "error";

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

const CLOCK_DRAG_STEP_MS = 10 * 60 * 1000;
const TIME_DRAG_PX_PER_STEP = 24;
const LOCATION_DRAG_PX_PER_STEP = 12;
const LOCATION_DRAG_STEP_DEG = 0.1;
const VIEW_DRAG_PX_PER_STEP = 12;
const AZ_DRAG_STEP_DEG = 1;
const ALT_DRAG_STEP_DEG = 0.5;
/// Multiplicative FOV factor per step. <1 so dragging right (positive steps)
/// zooms in (smaller FOV) to mirror scroll-wheel direction in StarCanvas.
const FOV_DRAG_STEP_FACTOR = 0.97;
const fmtDeg = (n: number) => `${n.toFixed(1)}°`;
const COMPASS_DIRS = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
const compass = (az: number) => COMPASS_DIRS[Math.round(az / 45) % 8];
const pad = (n: number) => n.toString().padStart(2, "0");
function twilightLabel(t: Translator, sunAltDeg: number | null): string {
  if (sunAltDeg === null) return t("twilight.initializing");
  const alt = sunAltDeg.toFixed(1);
  if (sunAltDeg >= 0) return t("twilight.daylight", { alt });
  if (sunAltDeg >= -6) return t("twilight.civil", { alt });
  if (sunAltDeg >= -12) return t("twilight.nautical", { alt });
  if (sunAltDeg >= -18) return t("twilight.astronomical", { alt });
  return t("twilight.night", { alt });
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

function addLocalDays(ms: number, days: number): number {
  const d = new Date(ms);
  d.setDate(d.getDate() + days);
  return d.getTime();
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
  eyepiece,
  planning,
  onSetObserver,
  onSetTime,
  onSetOverlays,
  onSetAtmosphere,
  onSetPlanets,
  onSetProjection,
  onSetEyepiece,
  onSetView,
  onCopySessionUrl,
  onCopySessionJson,
  onImportSessionJson,
  onUseGeolocation,
}: Props) {
  const t = useT();
  const [openPopover, setOpenPopover] = useState<Popover | null>(null);
  const [addressQuery, setAddressQuery] = useState("");
  const [addressLookup, setAddressLookup] = useState<AddressLookupState>({
    status: "idle",
    message: null,
  });
  const addressLookupSeq = useRef(0);

  // ---- Status-strip value scrubbers -------------------------------------
  // Each useStepDrag call wires one draggable text handle. Drag right to
  // increase the value; the parent button still toggles the popover on a
  // pure click (no horizontal motion past the slop threshold).

  const dateDrag = useStepDrag<number>({
    pxPerStep: TIME_DRAG_PX_PER_STEP,
    onStart: () => timeMs,
    onStep: (base, steps) => onSetTime(addLocalDays(base, steps)),
  });
  const clockDrag = useStepDrag<number>({
    pxPerStep: TIME_DRAG_PX_PER_STEP,
    onStart: () => timeMs,
    onStep: (base, steps) => onSetTime(base + steps * CLOCK_DRAG_STEP_MS),
  });
  const latDrag = useStepDrag<number>({
    pxPerStep: LOCATION_DRAG_PX_PER_STEP,
    onStart: () => observer.latitudeDeg,
    onStep: (base, steps) =>
      onSetObserver({
        ...observer,
        latitudeDeg: clamp(base + steps * LOCATION_DRAG_STEP_DEG, -90, 90),
      }),
  });
  const lngDrag = useStepDrag<number>({
    pxPerStep: LOCATION_DRAG_PX_PER_STEP,
    onStart: () => observer.longitudeDeg,
    onStep: (base, steps) =>
      onSetObserver({
        ...observer,
        longitudeDeg: clamp(base + steps * LOCATION_DRAG_STEP_DEG, -180, 180),
      }),
  });
  const azDrag = useStepDrag<number>({
    pxPerStep: VIEW_DRAG_PX_PER_STEP,
    onStart: () => view.azimuthDeg,
    onStep: (base, steps) =>
      onSetView({ ...view, azimuthDeg: wrapAzimuth(base + steps * AZ_DRAG_STEP_DEG) }),
  });
  const altDrag = useStepDrag<number>({
    pxPerStep: VIEW_DRAG_PX_PER_STEP,
    onStart: () => view.altitudeDeg,
    onStep: (base, steps) =>
      onSetView({ ...view, altitudeDeg: clampAltitude(base + steps * ALT_DRAG_STEP_DEG) }),
  });
  const fovDrag = useStepDrag<number>({
    pxPerStep: VIEW_DRAG_PX_PER_STEP,
    onStart: () => view.fovDeg,
    onStep: (base, steps) =>
      onSetView({ ...view, fovDeg: clampFov(base * Math.pow(FOV_DRAG_STEP_FACTOR, steps)) }),
  });

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
      setAddressLookup({ status: "error", message: t("location.enterAddress") });
      return;
    }

    const lookupId = ++addressLookupSeq.current;
    setAddressLookup({ status: "loading", message: t("location.searching") });
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
      if (lookupId !== addressLookupSeq.current) return;
      if (!place) {
        setAddressLookup({ status: "error", message: t("location.noMatch") });
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
        // Nominatim returns localized place names already; show them verbatim
        // and fall back to the generic translated success message.
        message: place.display_name ?? place.name ?? t("location.updated"),
      });
    } catch {
      if (lookupId !== addressLookupSeq.current) return;
      setAddressLookup({
        status: "error",
        message: t("location.lookupFailed"),
      });
    }
  };

  const togglePopover = (popover: Popover, dragConsumed: boolean) => {
    if (dragConsumed) return;
    setOpenPopover(openPopover === popover ? null : popover);
  };

  const cycleProjection = () => {
    if (projection.viewpoint !== "earth") return;
    const i = SKY_PROJECTIONS.indexOf(projection.projection);
    const next = SKY_PROJECTIONS[(i + 1) % SKY_PROJECTIONS.length];
    onSetProjection({ ...projection, projection: next });
  };

  const twilight = twilightLabel(t, sunAltitudeDeg);
  const fovDraggable =
    projection.viewpoint === "earth" && projection.projection === "perspective" && !eyepiece.enabled;
  const projectionToggleable = projection.viewpoint === "earth";

  return (
    <>
      {openPopover !== null && (
        <div
          aria-hidden
          onPointerDown={() => setOpenPopover(null)}
          style={backdropStyle}
        />
      )}
      {openPopover === "location" && (
        <PopoverPanel title={t("location.title")} onClose={() => setOpenPopover(null)}>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 8 }}>
            <NumberInputRow
              id="quick-lat"
              label={t("location.latitude")}
              value={observer.latitudeDeg}
              min={-90}
              max={90}
              step={0.0001}
              onChange={setLatitude}
            />
            <NumberInputRow
              id="quick-lng"
              label={t("location.longitude")}
              value={observer.longitudeDeg}
              min={-180}
              max={180}
              step={0.0001}
              onChange={setLongitude}
            />
          </div>

          <form
            onSubmit={(e) => {
              e.preventDefault();
              void lookupAddress();
            }}
            style={addressLookupFormStyle}
          >
            <label htmlFor="quick-address" style={labelStyle}>
              {t("location.addressLabel")}
            </label>
            <div style={{ display: "grid", gridTemplateColumns: "1fr auto", gap: 8 }}>
              <input
                id="quick-address"
                type="search"
                value={addressQuery}
                placeholder={t("location.addressPlaceholder")}
                onChange={(e) => {
                  addressLookupSeq.current += 1;
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
                {addressLookup.status === "loading" ? t("location.finding") : t("location.find")}
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
            {t("location.useMyLocation")}
          </button>
        </PopoverPanel>
      )}

      {openPopover === "time" && (
        <PopoverPanel title={t("time.title")} onClose={() => setOpenPopover(null)}>
          <label htmlFor="quick-datetime" style={labelStyle}>
            {t("time.localDateTime")}
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

          <div style={{ display: "flex", gap: 8, marginTop: 10 }}>
            <button type="button" onClick={() => onSetTime(Date.now())} style={buttonStyle}>
              {t("time.now")}
            </button>
            <button
              type="button"
              onClick={() => onSetTime(timeMs - 60 * 60 * 1000)}
              style={buttonStyle}
            >
              {t("time.minus1h")}
            </button>
            <button
              type="button"
              onClick={() => onSetTime(timeMs + 60 * 60 * 1000)}
              style={buttonStyle}
            >
              {t("time.plus1h")}
            </button>
          </div>
          <p style={{ margin: "10px 0 0", fontSize: 11, opacity: 0.55 }}>
            {t("time.helper")}
          </p>
        </PopoverPanel>
      )}

      {openPopover === "settings" && (
        <PopoverPanel
          title={t("settings.title")}
          onClose={() => setOpenPopover(null)}
          fillViewport
        >
          <SettingsPanel
            overlays={overlays}
            atmosphere={atmosphere}
            planets={planets}
            projection={projection}
            eyepiece={eyepiece}
            planning={planning}
            onSetOverlays={onSetOverlays}
            onSetAtmosphere={onSetAtmosphere}
            onSetPlanets={onSetPlanets}
            onSetProjection={onSetProjection}
            onSetEyepiece={onSetEyepiece}
            onCopySessionUrl={onCopySessionUrl}
            onCopySessionJson={onCopySessionJson}
            onImportSessionJson={onImportSessionJson}
          />
        </PopoverPanel>
      )}

      <div style={containerStyle}>
      <div style={stripStyle}>
        <button
          type="button"
          aria-expanded={openPopover === "location"}
          aria-haspopup="dialog"
          onClick={() => togglePopover("location", latDrag.consumeDragClick() || lngDrag.consumeDragClick())}
          style={chipButtonStyle(openPopover === "location")}
        >
          <span style={mutedStyle}>{t("status.location")} </span>
          <span
            title={t("status.dragLatTitle")}
            {...latDrag.handlers}
            style={draggableTimeValueStyle(openPopover === "location")}
          >
            {observer.latitudeDeg.toFixed(2)}°
          </span>
          <span style={mutedStyle}>, </span>
          <span
            title={t("status.dragLngTitle")}
            {...lngDrag.handlers}
            style={draggableTimeValueStyle(openPopover === "location")}
          >
            {observer.longitudeDeg.toFixed(2)}°
          </span>
        </button>
        <span style={separatorStyle}>  ·  </span>
        <button
          type="button"
          aria-expanded={openPopover === "time"}
          aria-haspopup="dialog"
          onClick={() => togglePopover("time", dateDrag.consumeDragClick() || clockDrag.consumeDragClick())}
          style={chipButtonStyle(openPopover === "time")}
        >
          <span style={mutedStyle}>{t("status.time")} </span>
          <span
            title={t("status.dragDateTitle")}
            {...dateDrag.handlers}
            style={draggableTimeValueStyle(openPopover === "time")}
          >
            {fmtDate(timeMs)}
          </span>
          <span style={mutedStyle}> </span>
          <span
            title={t("status.dragClockTitle")}
            {...clockDrag.handlers}
            style={draggableTimeValueStyle(openPopover === "time")}
          >
            {fmtClock(timeMs)}
          </span>
        </button>
        <span style={separatorStyle}>  ·  </span>
        <span style={mutedStyle}>{t("status.az")} </span>
        <span
          title={t("status.dragAzTitle")}
          {...azDrag.handlers}
          style={draggableValueStyle}
        >
          {fmtDeg(view.azimuthDeg)} ({compass(view.azimuthDeg)})
        </span>
        <span style={separatorStyle}>  ·  </span>
        <span style={mutedStyle}>{t("status.alt")} </span>
        <span
          title={t("status.dragAltTitle")}
          {...altDrag.handlers}
          style={draggableValueStyle}
        >
          {fmtDeg(view.altitudeDeg)}
        </span>
        <span style={separatorStyle}>  ·  </span>
        <span style={mutedStyle}>{t("status.fov")} </span>
        {projection.viewpoint === "earth" && projection.projection !== "perspective" ? (
          t("status.fovFullSky")
        ) : eyepiece.enabled && projection.viewpoint === "earth" && projection.projection === "perspective" ? (
          t("status.fovEyepiece", { value: eyepieceTrueFieldDeg(eyepiece).toFixed(2) })
        ) : fovDraggable ? (
          <span
            title={t("status.dragFovTitle")}
            {...fovDrag.handlers}
            style={draggableValueStyle}
          >
            {fmtDeg(view.fovDeg)}
          </span>
        ) : (
          fmtDeg(view.fovDeg)
        )}
        <span style={separatorStyle}>  ·  </span>
        <span style={mutedStyle}>{t("status.projection")} </span>
        {projectionToggleable ? (
          <button
            type="button"
            onClick={cycleProjection}
            title={t("status.cycleProjection")}
            style={inlineButtonStyle}
          >
            <span style={underlinedValueStyle(false)}>{t(`projection.${projection.projection}`)}</span>
          </button>
        ) : (
          t(`projection.${projection.projection}`)
        )}
        <span style={separatorStyle}>  ·  </span>
        <span style={mutedStyle}>{t("status.viewpoint")} </span>
        {t(`viewpoint.${projection.viewpoint}`)}
        <span style={separatorStyle}>  ·  </span>
        <span style={mutedStyle}>{t("status.sky")} </span>
        {twilight}
        <span style={separatorStyle}>  ·  </span>
        <button
          type="button"
          aria-expanded={openPopover === "settings"}
          aria-haspopup="dialog"
          onClick={() => setOpenPopover(openPopover === "settings" ? null : "settings")}
          style={inlineButtonStyle}
        >
          <span style={underlinedValueStyle(openPopover === "settings")}>{t("status.settings")}</span>
        </button>
      </div>
      </div>
    </>
  );
}

function PopoverPanel({
  title,
  children,
  onClose,
  fillViewport = false,
}: {
  title: string;
  children: React.ReactNode;
  onClose: () => void;
  /// When true, the popover stretches from near the viewport top down to the
  /// status strip. Use for the dense settings panel; leave false for the
  /// short location/time popovers that should hug their anchor chip.
  fillViewport?: boolean;
}) {
  const t = useT();
  return (
    <div
      role="dialog"
      aria-label={t("popover.dialogLabel", { title })}
      style={fillViewport ? popoverFillViewportStyle : popoverStyle}
    >
      <header style={popoverHeaderStyle}>
        <div style={{ fontSize: 11, opacity: 0.65, letterSpacing: 0.8 }}>{title.toUpperCase()}</div>
        <button
          type="button"
          aria-label={t("popover.close", { title })}
          onClick={onClose}
          style={closeButtonStyle}
        >
          ×
        </button>
      </header>
      <div style={popoverBodyStyle}>{children}</div>
    </div>
  );
}

type SettingsPanelProps = Pick<
  Props,
  | "overlays"
  | "atmosphere"
  | "planets"
  | "projection"
  | "eyepiece"
  | "planning"
  | "onSetOverlays"
  | "onSetAtmosphere"
  | "onSetPlanets"
  | "onSetProjection"
  | "onSetEyepiece"
  | "onCopySessionUrl"
  | "onCopySessionJson"
  | "onImportSessionJson"
>;

type SettingsTab = "sky" | "view" | "environment" | "session";
const SETTINGS_TABS: SettingsTab[] = ["sky", "view", "environment", "session"];

function SettingsPanel({
  overlays,
  atmosphere,
  planets,
  projection,
  eyepiece,
  planning,
  onSetOverlays,
  onSetAtmosphere,
  onSetPlanets,
  onSetProjection,
  onSetEyepiece,
  onCopySessionUrl,
  onCopySessionJson,
  onImportSessionJson,
}: SettingsPanelProps) {
  const t = useT();
  const [tab, setTab] = useState<SettingsTab>("sky");
  const sessionFileRef = useRef<HTMLInputElement>(null);
  const setAtmospherePreset = (preset: AtmospherePreset) => {
    onSetAtmosphere({
      ...atmosphere,
      preset,
      ...ATMOSPHERE_PRESET_DEFAULTS[preset],
    });
  };

  return (
    <div style={{ display: "grid", gap: 14 }}>
      <div role="tablist" aria-label={t("settings.tabsLabel")} style={settingsTabBarStyle}>
        {SETTINGS_TABS.map((id) => (
          <button
            key={id}
            role="tab"
            aria-selected={tab === id}
            type="button"
            onClick={() => setTab(id)}
            style={settingsTabButtonStyle(tab === id)}
          >
            {t(`settings.tab.${id}`)}
          </button>
        ))}
      </div>

      {tab === "sky" && (
        <>
          <SettingCard title={t("card.solarSystem.title")} description={t("card.solarSystem.description")}>
            <label style={checkboxRowStyle}>
              <input
                type="checkbox"
                checked={planets.enabled}
                onChange={(e) => onSetPlanets({ enabled: e.target.checked })}
                style={{ accentColor: "#8fb1ff" }}
              />
              {t("card.view.mercuryToNeptune")}
            </label>
          </SettingCard>

          <SettingCard
            title={t("card.overlays.title")}
            description={t("card.overlays.description")}
          >
            <OverlayToggles config={overlays} onChange={onSetOverlays} />
          </SettingCard>

          {planning && <PlanningPanel planning={planning} />}
        </>
      )}

      {tab === "view" && (
        <>
          <SettingCard title={t("card.view.title")} description={t("card.view.description")}>
            <label htmlFor="sky-viewpoint" style={labelStyle}>
          {t("card.view.viewpoint")}
        </label>
        <select
          id="sky-viewpoint"
          value={projection.viewpoint}
          onChange={(e) => onSetProjection({ ...projection, viewpoint: e.target.value as SkyViewpoint })}
          style={{ ...inputStyle, width: "100%" }}
        >
          {SKY_VIEWPOINTS.map((v) => (
            <option key={v} value={v}>
              {t(`viewpoint.${v}`)}
            </option>
          ))}
        </select>

        {projection.viewpoint === "custom-external" && (
          <div style={{ marginTop: 10, display: "grid", gap: 8 }}>
            <Vec3NumberRow
              id="external-origin-pc"
              label={t("card.view.originPc")}
              value={projection.external.originPc}
              min={-1_000_000}
              max={1_000_000}
              step={100}
              onChange={(originPc) => onSetProjection({ ...projection, external: { ...projection.external, originPc } })}
            />
            <Vec3NumberRow
              id="external-target-pc"
              label={t("card.view.targetPc")}
              value={projection.external.targetPc}
              min={-1_000_000}
              max={1_000_000}
              step={100}
              onChange={(targetPc) => onSetProjection({ ...projection, external: { ...projection.external, targetPc } })}
            />
            <Vec3NumberRow
              id="external-up"
              label={t("card.view.up")}
              value={projection.external.up}
              min={-10}
              max={10}
              step={0.1}
              onChange={(up) => onSetProjection({ ...projection, external: { ...projection.external, up } })}
            />
          </div>
        )}

        <label htmlFor="sky-projection" style={{ ...labelStyle, marginTop: 10 }}>
          {t("card.view.screenProjection")}
        </label>
        <select
          id="sky-projection"
          value={projection.projection}
          onChange={(e) => onSetProjection({ ...projection, projection: e.target.value as SkyProjection })}
          disabled={projection.viewpoint !== "earth"}
          style={{ ...inputStyle, width: "100%" }}
        >
          {SKY_PROJECTIONS.map((p) => (
            <option key={p} value={p}>
              {t(`projection.${p}`)}
            </option>
          ))}
        </select>
            <p style={helperTextStyle}>{t("card.view.helper")}</p>
          </SettingCard>

          <SettingCard
            title={t("card.telescope.title")}
            description={t("card.telescope.description")}
          >
        <label style={{ ...checkboxRowStyle, marginBottom: 10 }}>
          <input
            type="checkbox"
            checked={eyepiece.enabled}
            onChange={(e) => onSetEyepiece({ ...eyepiece, enabled: e.target.checked })}
            style={{ accentColor: "#8fb1ff" }}
          />
          {t("card.telescope.enable")}
        </label>
        <div style={advancedControlGridStyle}>
          <SliderNumberRow
            id="eyepiece-aperture"
            label={t("card.telescope.aperture")}
            value={eyepiece.apertureMm}
            min={10}
            max={2000}
            step={10}
            decimals={0}
            onChange={(apertureMm) =>
              onSetEyepiece({ ...eyepiece, enabled: true, apertureMm: clamp(apertureMm, 10, 2000) })
            }
          />
          <SliderNumberRow
            id="eyepiece-focal-length"
            label={t("card.telescope.focal")}
            value={eyepiece.focalLengthMm}
            min={50}
            max={20000}
            step={50}
            decimals={0}
            onChange={(focalLengthMm) =>
              onSetEyepiece({ ...eyepiece, enabled: true, focalLengthMm: clamp(focalLengthMm, 50, 20000) })
            }
          />
          <SliderNumberRow
            id="eyepiece-ocular-focal"
            label={t("card.telescope.eyepieceFocal")}
            value={eyepiece.eyepieceFocalLengthMm}
            min={1}
            max={100}
            step={0.5}
            decimals={1}
            onChange={(eyepieceFocalLengthMm) =>
              onSetEyepiece({
                ...eyepiece,
                enabled: true,
                eyepieceFocalLengthMm: clamp(eyepieceFocalLengthMm, 1, 100),
              })
            }
          />
          <SliderNumberRow
            id="eyepiece-afov"
            label={t("card.telescope.afov")}
            value={eyepiece.apparentFovDeg}
            min={1}
            max={120}
            step={1}
            decimals={0}
            onChange={(apparentFovDeg) =>
              onSetEyepiece({ ...eyepiece, enabled: true, apparentFovDeg: clamp(apparentFovDeg, 1, 120) })
            }
          />
          <SliderNumberRow
            id="eyepiece-field-stop"
            label={t("card.telescope.fieldStop")}
            value={eyepiece.fieldStopMm}
            min={0}
            max={120}
            step={0.5}
            decimals={1}
            onChange={(fieldStopMm) =>
              onSetEyepiece({ ...eyepiece, enabled: true, fieldStopMm: clamp(fieldStopMm, 0, 120) })
            }
          />
        </div>
            <p style={helperTextStyle}>
              {t("card.telescope.summary", {
                mag: eyepieceMagnification(eyepiece).toFixed(1),
                trueField: eyepieceTrueFieldDeg(eyepiece).toFixed(3),
                plateScale: eyepiecePlateScaleArcsecPerMm(eyepiece).toFixed(1),
                exitPupil: eyepieceExitPupilMm(eyepiece).toFixed(1),
              })}
            </p>
          </SettingCard>
        </>
      )}

      {tab === "environment" && (
        <SettingCard
          title={t("card.atmosphere.title")}
          description={t("card.atmosphere.description")}
        >
        <label style={{ ...checkboxRowStyle, marginBottom: 10 }}>
          <input
            type="checkbox"
            checked={atmosphere.enabled}
            onChange={(e) => onSetAtmosphere({ ...atmosphere, enabled: e.target.checked })}
            style={{ accentColor: "#8fb1ff" }}
          />
          {t("card.atmosphere.enable")}
        </label>

        <label htmlFor="atmosphere-preset" style={labelStyle}>
          {t("card.atmosphere.preset")}
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
              {t(`atmospherePreset.${preset}`)}
            </option>
          ))}
        </select>

        <div style={advancedControlGridStyle}>
          <SliderNumberRow
            id="atmosphere-turbidity"
            label={t("card.atmosphere.turbidity")}
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
            label={t("card.atmosphere.altitude")}
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
            label={t("card.atmosphere.ozone")}
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
            label={t("card.atmosphere.visibility")}
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
            label={t("card.atmosphere.pressure")}
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
            label={t("card.atmosphere.temperature")}
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
      )}

      {tab === "session" && (
        <SettingCard
          title={t("card.session.title")}
          description={t("card.session.description")}
        >
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <button type="button" onClick={onCopySessionUrl} style={buttonStyle}>
              {t("card.session.copyUrl")}
            </button>
            <button type="button" onClick={onCopySessionJson} style={buttonStyle}>
              {t("card.session.copyJson")}
            </button>
            <button type="button" onClick={() => sessionFileRef.current?.click()} style={buttonStyle}>
              {t("card.session.loadJson")}
            </button>
          </div>
          <input
            ref={sessionFileRef}
            type="file"
            accept="application/json,.json"
            style={{ display: "none" }}
            onChange={async (event) => {
              const file = event.currentTarget.files?.[0];
              event.currentTarget.value = "";
              if (!file) return;
              onImportSessionJson(await file.text());
            }}
          />
          <p style={{ ...helperTextStyle, marginTop: 10 }}>{t("card.session.helper")}</p>
        </SettingCard>
      )}
    </div>
  );
}

function SettingCard({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <section style={settingCardStyle}>
      <div style={settingCardTitleStyle}>{title}</div>
      {description ? <p style={settingCardDescriptionStyle}>{description}</p> : null}
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
  const t = useT();
  return (
    <SettingCard
      title={t("card.planning.title")}
      description={t("card.planning.description")}
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
            <span>{translateWasmBody(t, row.name)}</span>
            <span title={t("card.planning.rise")}>↑ {fmtEventTime(row.riseMs)}</span>
            <span title={t("card.planning.transit")}>↟ {fmtEventTime(row.transitMs)}</span>
            <span title={t("card.planning.set")}>↓ {fmtEventTime(row.setMs)}</span>
            <span>{row.transitAltitudeDeg === null ? "—" : `${row.transitAltitudeDeg.toFixed(0)}°`}</span>
          </div>
        ))}
      </div>
      <div style={{ marginTop: 8, display: "grid", gap: 3 }}>
        {planning.twilight.map((segment) => (
          <div key={`${segment.label}-${segment.startMs}`} style={{ opacity: 0.72 }}>
            {translateWasmTwilight(t, segment.label)}: {fmtEventTime(segment.startMs)}–{fmtEventTime(segment.endMs)}
          </div>
        ))}
      </div>
    </SettingCard>
  );
}

function Vec3NumberRow({
  id,
  label,
  value,
  min,
  max,
  step,
  onChange,
}: {
  id: string;
  label: string;
  value: Vec3;
  min: number;
  max: number;
  step: number;
  onChange: (value: Vec3) => void;
}) {
  const updateAxis = (axis: keyof Vec3, raw: string) => {
    if (raw === "") return;
    const next = Number(raw);
    if (Number.isFinite(next)) {
      onChange({ ...value, [axis]: Math.max(min, Math.min(max, next)) });
    }
  };
  return (
    <div>
      <label style={labelStyle}>{label}</label>
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 6 }}>
        {(["x", "y", "z"] as const).map((axis) => (
          <input
            key={axis}
            aria-label={`${label} ${axis}`}
            id={`${id}-${axis}`}
            type="number"
            min={min}
            max={max}
            step={step}
            value={value[axis]}
            onChange={(e) => updateAxis(axis, e.target.value)}
            style={inputStyle}
          />
        ))}
      </div>
    </div>
  );
}

function NumberInputRow({
  id,
  label,
  value,
  min,
  max,
  step,
  onChange,
}: {
  id: string;
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
}) {
  return (
    <div>
      <label htmlFor={id} style={labelStyle}>
        {label}
      </label>
      <input
        id={id}
        type="number"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => {
          const raw = e.target.value;
          if (raw === "") return;
          const next = Number(raw);
          if (Number.isFinite(next)) onChange(next);
        }}
        style={{ ...inputStyle, width: "100%" }}
      />
    </div>
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
  // No background: the strip floats directly over the sky. Text shadow keeps
  // it readable against bright Milky Way / horizon regions.
  textShadow: "0 1px 2px rgba(0, 0, 0, 0.85), 0 0 4px rgba(0, 0, 0, 0.7)",
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

const draggableValueStyle: React.CSSProperties = {
  cursor: "ew-resize",
  touchAction: "none",
  userSelect: "none",
  textDecoration: "underline",
  textDecorationColor: "rgba(207, 216, 227, 0.35)",
  textDecorationStyle: "dotted",
  textUnderlineOffset: 3,
};

/// Click-to-dismiss backdrop. Transparent so the sky stays visible while a
/// popover is open; sits below the popover (which has its own zIndex above).
const backdropStyle: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  zIndex: 4,
};

const popoverShellStyle: React.CSSProperties = {
  position: "fixed",
  left: 14,
  width: "min(420px, calc(100vw - 28px))",
  display: "flex",
  flexDirection: "column",
  background: "rgba(14, 18, 30, 0.96)",
  borderRadius: 12,
  boxShadow: "0 12px 34px rgba(0, 0, 0, 0.55)",
  backdropFilter: "blur(10px)",
  zIndex: 6,
  overflow: "hidden",
  // Popovers are siblings of the status strip (not nested), so they do not
  // inherit color/font from `containerStyle`. Re-apply them here so the inner
  // controls pick up the same monospace + light-grey look.
  color: "#cfd8e3",
  font: "12px/1.4 ui-monospace, SFMono-Regular, Menlo, monospace",
};

/// Short popovers (location, time): anchor just above the status strip.
const popoverStyle: React.CSSProperties = {
  ...popoverShellStyle,
  bottom: 56,
  maxHeight: "calc(100vh - 80px)",
};

/// Settings popover: dense list — stretch from near the viewport top down
/// to just above the status strip so the top stays attached to the window.
const popoverFillViewportStyle: React.CSSProperties = {
  ...popoverShellStyle,
  top: 14,
  bottom: 56,
};

const popoverHeaderStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  padding: "12px 14px 10px",
  background: "rgba(14, 18, 30, 0.98)",
  borderBottom: "1px solid rgba(255, 255, 255, 0.08)",
  flexShrink: 0,
};

/// Scrolling body that sits under the sticky header.
const popoverBodyStyle: React.CSSProperties = {
  padding: "10px 14px 14px",
  overflowY: "auto",
  overscrollBehavior: "contain",
  flex: 1,
  minHeight: 0,
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

/// Flat "section" look: a small uppercase heading. Sections are separated
/// purely by the parent grid `gap` — no boxes, borders, or background tints,
/// to avoid the previous card-in-card-in-card depth in the settings panel.
const settingCardStyle: React.CSSProperties = {};

const settingsTabBarStyle: React.CSSProperties = {
  display: "flex",
  gap: 4,
  borderBottom: "1px solid rgba(255, 255, 255, 0.08)",
  paddingBottom: 0,
};

const settingsTabButtonStyle = (active: boolean): React.CSSProperties => ({
  appearance: "none",
  background: "transparent",
  border: 0,
  borderBottom: active ? "2px solid rgba(170, 200, 255, 0.85)" : "2px solid transparent",
  color: active ? "#e6edf5" : "#cfd8e3",
  cursor: "pointer",
  fontFamily: "inherit",
  fontSize: 11,
  letterSpacing: 0.65,
  textTransform: "uppercase",
  opacity: active ? 1 : 0.65,
  padding: "6px 10px",
  marginBottom: -1,
});

const settingCardTitleStyle: React.CSSProperties = {
  color: "#dbe7ff",
  fontSize: 11,
  letterSpacing: 0.65,
  textTransform: "uppercase",
};

const settingCardDescriptionStyle: React.CSSProperties = {
  margin: "4px 0 8px",
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

