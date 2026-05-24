import { useEffect, useRef, useState } from "react";
import { StarCanvas } from "./components/StarCanvas";
import { StatusBar } from "./components/StatusBar";
import { GearButton } from "./components/GearButton";
import { SettingsPanel } from "./components/SettingsPanel";
import {
  clampAltitude,
  clampFov,
  wrapAzimuth,
  type Observer,
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
  const [timeMs, setTimeMs] = useState<number>(() => Date.now());
  const [settingsOpen, setSettingsOpen] = useState(false);

  // Persist observer + view whenever they change. Time is intentionally not
  // persisted: a stale timestamp on next load would silently mislead the user.
  useEffect(() => {
    saveConfig({ observer, view });
  }, [observer, view]);

  // Clock always ticks. When the user picks a custom moment via the settings
  // panel we simply rebase `timeMs`; the same loop keeps advancing from there.
  const lastTickRef = useRef<number>(performance.now());
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

  // Auto-geolocation only fires for first-time visitors. If the user has a
  // persisted observer we respect their explicit choice and stay quiet.
  useEffect(() => {
    if (PERSISTED?.observer) return;
    if (!navigator.geolocation) return;
    navigator.geolocation.getCurrentPosition(
      (pos) => {
        setObserver({
          latitudeDeg: pos.coords.latitude,
          longitudeDeg: pos.coords.longitude,
        });
      },
      () => {},
      { timeout: 5000 },
    );
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
      <StatusBar observer={observer} view={view} timeMs={timeMs} />
      <GearButton onClick={() => setSettingsOpen(true)} />
      {settingsOpen && (
        <SettingsPanel
          observer={observer}
          timeMs={timeMs}
          onClose={() => setSettingsOpen(false)}
          onSetObserver={setObserver}
          onSetTime={setTimeMs}
          onUseGeolocation={useGeolocation}
        />
      )}
    </>
  );
}
