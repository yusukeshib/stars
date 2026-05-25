import { useEffect, useRef, useState } from "react";
import { StarCanvas } from "./components/StarCanvas";
import { StatusBar } from "./components/StatusBar";
import {
  clampAltitude,
  clampFov,
  wrapAzimuth,
  DEFAULT_OVERLAY_CONFIG,
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

// Read once at module load. Used both for initial state and to decide whether
// to auto-prompt for geolocation (we skip it if the user already has a stored
// location, so we don't overwrite their explicit choice).
const PERSISTED = loadConfig();

export function App() {
  const [observer, setObserver] = useState<Observer>(PERSISTED?.observer ?? DEFAULT_OBSERVER);
  const [view, setView] = useState<View>(PERSISTED?.view ?? DEFAULT_VIEW);
  const [overlays, setOverlays] = useState<OverlayConfig>(
    PERSISTED?.overlays ?? DEFAULT_OVERLAY_CONFIG,
  );
  const [timeMs, setTimeMs] = useState<number>(() => Date.now());
  const lastTickRef = useRef<number>(performance.now());

  // Persist observer + view + overlays whenever they change. We debounce
  // because the view updates on every mouse/touch frame during a drag, and
  // localStorage.setItem is synchronous; without the debounce we'd write ~60
  // times a second. Time is intentionally not persisted: a stale timestamp on
  // next load would silently mislead the user.
  useEffect(() => {
    const handle = setTimeout(() => saveConfig({ observer, view, overlays }), 250);
    return () => clearTimeout(handle);
  }, [observer, view, overlays]);

  // Clock always ticks. When the user picks a custom moment via the settings
  // panel we simply rebase `timeMs`; the same loop keeps advancing from there.
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
        onSetObserver={setObserver}
        onSetTime={setTimeMs}
        onSetOverlays={setOverlays}
        onUseGeolocation={useGeolocation}
      />
    </>
  );
}
