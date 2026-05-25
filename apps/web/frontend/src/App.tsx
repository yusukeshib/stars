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

function loadAtmosphereFromUrl(): AtmosphereConfig | null {
  if (typeof window === "undefined") return null;
  const params = new URLSearchParams(window.location.search);
  if (
    !params.has("atmosphere") &&
    !params.has("atmospherePreset") &&
    !params.has("turbidity") &&
    !params.has("observerAltitudeM")
  ) {
    return null;
  }

  const enabled = params.get("atmosphere") !== "off";
  const presetParam = params.get("atmospherePreset");
  const turbidity = Number(params.get("turbidity"));
  const observerAltitudeM = Number(params.get("observerAltitudeM"));
  return {
    ...DEFAULT_ATMOSPHERE_CONFIG,
    enabled,
    preset: isAtmospherePreset(presetParam) ? presetParam : DEFAULT_ATMOSPHERE_CONFIG.preset,
    turbidity: Number.isFinite(turbidity)
      ? Math.max(1.7, Math.min(10, turbidity))
      : DEFAULT_ATMOSPHERE_CONFIG.turbidity,
    observerAltitudeM: Number.isFinite(observerAltitudeM)
      ? Math.max(0, Math.min(9000, observerAltitudeM))
      : DEFAULT_ATMOSPHERE_CONFIG.observerAltitudeM,
  };
}

function saveAtmosphereToUrl(atmosphere: AtmosphereConfig): void {
  if (typeof window === "undefined") return;
  const url = new URL(window.location.href);
  url.searchParams.set("atmosphere", atmosphere.enabled ? "on" : "off");
  url.searchParams.set("atmospherePreset", atmosphere.preset);
  url.searchParams.set("turbidity", atmosphere.turbidity.toFixed(1));
  url.searchParams.set("observerAltitudeM", String(Math.round(atmosphere.observerAltitudeM)));
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
