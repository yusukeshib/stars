import { useEffect, useRef, useState } from "react";
import { StarCanvas } from "./components/StarCanvas";
import { StatusBar } from "./components/StatusBar";
import {
  clampAltitude,
  clampFov,
  wrapAzimuth,
  DEFAULT_ATMOSPHERE_CONFIG,
  DEFAULT_OVERLAY_CONFIG,
  isAtmospherePreset,
  type AtmosphereConfig,
  type Observer,
  type OverlayConfig,
  type View,
} from "./observer";
import { loadConfig, saveConfig } from "./storage";

const DEFAULT_OBSERVER: Observer = {
  latitudeDeg: 35.68, // Tokyo as a sensible default
  longitudeDeg: 139.69,
};

const DEFAULT_VIEW: View = {
  azimuthDeg: 180, // facing south
  altitudeDeg: 30,
  fovDeg: 70,
};

const normalizeWebOverlays = (overlays: OverlayConfig): OverlayConfig => ({
  ...overlays,
  // Cardinal marks are intentionally hidden in the web UI; drop older persisted
  // selections so users do not get a stuck invisible toggle.
  layers: overlays.layers.filter((layer) => layer !== "cardinals"),
});

const numberParam = (
  params: URLSearchParams,
  key: string,
  fallback: number,
  min: number,
  max: number,
): number => {
  const raw = params.get(key);
  if (raw === null) return fallback;
  const value = Number(raw);
  return Number.isFinite(value) ? Math.max(min, Math.min(max, value)) : fallback;
};

function loadAtmosphereFromUrl(): AtmosphereConfig | null {
  if (typeof window === "undefined") return null;
  const params = new URLSearchParams(window.location.search);
  if (
    !params.has("atmosphere") &&
    !params.has("atmospherePreset") &&
    !params.has("turbidity") &&
    !params.has("observerAltitudeM") &&
    !params.has("ozoneDu") &&
    !params.has("visibilityKm")
  ) {
    return null;
  }

  const enabled = params.get("atmosphere") !== "off";
  const presetParam = params.get("atmospherePreset");
  return {
    ...DEFAULT_ATMOSPHERE_CONFIG,
    enabled,
    preset: isAtmospherePreset(presetParam) ? presetParam : DEFAULT_ATMOSPHERE_CONFIG.preset,
    turbidity: numberParam(params, "turbidity", DEFAULT_ATMOSPHERE_CONFIG.turbidity, 1.7, 10),
    observerAltitudeM: numberParam(
      params,
      "observerAltitudeM",
      DEFAULT_ATMOSPHERE_CONFIG.observerAltitudeM,
      0,
      9000,
    ),
    ozoneDu: numberParam(params, "ozoneDu", DEFAULT_ATMOSPHERE_CONFIG.ozoneDu, 0, 600),
    visibilityKm: numberParam(
      params,
      "visibilityKm",
      DEFAULT_ATMOSPHERE_CONFIG.visibilityKm,
      1,
      200,
    ),
  };
}

function saveAtmosphereToUrl(atmosphere: AtmosphereConfig): void {
  if (typeof window === "undefined") return;
  const url = new URL(window.location.href);
  url.searchParams.set("atmosphere", atmosphere.enabled ? "on" : "off");
  url.searchParams.set("atmospherePreset", atmosphere.preset);
  url.searchParams.set("turbidity", atmosphere.turbidity.toFixed(1));
  url.searchParams.set("observerAltitudeM", String(Math.round(atmosphere.observerAltitudeM)));
  url.searchParams.set("ozoneDu", String(Math.round(atmosphere.ozoneDu)));
  url.searchParams.set("visibilityKm", atmosphere.visibilityKm.toFixed(0));
  window.history.replaceState(null, "", url);
}

// Read once at module load.
const PERSISTED = loadConfig();
const URL_ATMOSPHERE = loadAtmosphereFromUrl();

export function App() {
  const [observer, setObserver] = useState<Observer>(PERSISTED?.observer ?? DEFAULT_OBSERVER);
  const [view, setView] = useState<View>(PERSISTED?.view ?? DEFAULT_VIEW);
  const [overlays, setOverlays] = useState<OverlayConfig>(
    PERSISTED?.overlays ? normalizeWebOverlays(PERSISTED.overlays) : DEFAULT_OVERLAY_CONFIG,
  );
  const [atmosphere, setAtmosphere] = useState<AtmosphereConfig>(
    URL_ATMOSPHERE ?? PERSISTED?.atmosphere ?? DEFAULT_ATMOSPHERE_CONFIG,
  );
  const [timeMs, setTimeMs] = useState<number>(() => Date.now());
  const lastTickRef = useRef<number>(performance.now());

  // Persist observer + view + overlays + atmosphere whenever they change. We debounce
  // because the view updates on every mouse/touch frame during a drag, and
  // localStorage.setItem is synchronous; without the debounce we'd write ~60
  // times a second. Time is intentionally not persisted: a stale timestamp on
  // next load would silently mislead the user.
  useEffect(() => {
    const handle = setTimeout(() => saveConfig({ observer, view, overlays, atmosphere }), 250);
    return () => clearTimeout(handle);
  }, [observer, view, overlays, atmosphere]);

  useEffect(() => {
    saveAtmosphereToUrl(atmosphere);
  }, [atmosphere]);

  // Clock always ticks. When the user picks a custom moment via the quick time
  // popup we simply rebase `timeMs`; the same loop keeps advancing from there.
  useEffect(() => {
    let raf = 0;
    const step = (now: number) => {
      const elapsed = now - lastTickRef.current;
      lastTickRef.current = now;
      setTimeMs((t) => t + elapsed);
      raf = requestAnimationFrame(step);
    };
    lastTickRef.current = performance.now();
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, []);

  const useGeolocation = () => {
    if (!navigator.geolocation) return;
    navigator.geolocation.getCurrentPosition((pos) => {
      setObserver({
        latitudeDeg: pos.coords.latitude,
        longitudeDeg: pos.coords.longitude,
      });
    });
  };

  return (
    <>
      <StarCanvas
        observer={observer}
        view={view}
        timeMs={timeMs}
        overlays={overlays}
        atmosphere={atmosphere}
        onDrag={(daz, dalt) =>
          setView((v) => ({
            ...v,
            azimuthDeg: wrapAzimuth(v.azimuthDeg + daz),
            altitudeDeg: clampAltitude(v.altitudeDeg + dalt),
          }))
        }
        onWheel={(factor) =>
          setView((v) => ({ ...v, fovDeg: clampFov(v.fovDeg * factor) }))
        }
      />
      <StatusBar
        observer={observer}
        view={view}
        timeMs={timeMs}
        overlays={overlays}
        atmosphere={atmosphere}
        onSetObserver={setObserver}
        onSetTime={setTimeMs}
        onSetOverlays={setOverlays}
        onSetAtmosphere={setAtmosphere}
        onUseGeolocation={useGeolocation}
      />
    </>
  );
}
