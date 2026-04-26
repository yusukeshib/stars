import { useEffect, useRef, useState } from "react";
import { StarCanvas } from "./components/StarCanvas";
import { Hud } from "./components/Hud";
import {
  clampAltitude,
  clampFov,
  wrapAzimuth,
  type Observer,
  type View,
} from "./observer";

const DEFAULT_OBSERVER: Observer = {
  latitudeDeg: 35.68, // Tokyo as a sensible default
  longitudeDeg: 139.69,
};

const DEFAULT_VIEW: View = {
  azimuthDeg: 180, // facing south
  altitudeDeg: 30,
  fovDeg: 70,
};

export function App() {
  const [observer, setObserver] = useState<Observer>(DEFAULT_OBSERVER);
  const [view, setView] = useState<View>(DEFAULT_VIEW);
  const [paused, setPaused] = useState(false);
  const [timeMs, setTimeMs] = useState<number>(() => Date.now());

  // Tick the clock when not paused.
  const lastTickRef = useRef<number>(performance.now());
  useEffect(() => {
    if (paused) return;
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
  }, [paused]);

  // Try geolocation on mount; silently ignore if denied.
  useEffect(() => {
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
      <Hud
        observer={observer}
        view={view}
        timeMs={timeMs}
        paused={paused}
        onSetObserver={setObserver}
        onSetTime={(ms) => {
          setTimeMs(ms);
          setPaused(true);
        }}
        onSetPaused={setPaused}
        onUseGeolocation={() => {
          if (!navigator.geolocation) return;
          navigator.geolocation.getCurrentPosition((pos) => {
            setObserver({
              latitudeDeg: pos.coords.latitude,
              longitudeDeg: pos.coords.longitude,
            });
          });
        }}
        onResetTime={() => {
          setTimeMs(Date.now());
          setPaused(false);
        }}
      />
    </>
  );
}
