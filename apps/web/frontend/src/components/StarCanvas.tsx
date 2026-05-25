import { useEffect, useRef } from "react";
import type { StarView } from "stars-web";
import { toRad, type AtmosphereConfig, type Observer, type OverlayConfig, type PlanetsConfig, type PlanningTable, type View } from "../observer";

type Props = {
  observer: Observer;
  view: View;
  /** Unix milliseconds for the rendered moment. */
  timeMs: number;
  overlays: OverlayConfig;
  atmosphere: AtmosphereConfig;
  planets: PlanetsConfig;
  onDrag: (deltaAzDeg: number, deltaAltDeg: number) => void;
  onWheel: (zoomFactor: number) => void;
  onSunAltitude: (sunAltitudeDeg: number) => void;
  onPlanning: (planning: PlanningTable) => void;
};

type PointerPoint = { x: number; y: number };

const distance = (a: PointerPoint, b: PointerPoint) => Math.hypot(a.x - b.x, a.y - b.y);

function firstTwoPointers(points: Map<number, PointerPoint>): [PointerPoint, PointerPoint] | null {
  const iterator = points.values();
  const first = iterator.next();
  const second = iterator.next();
  if (first.done || second.done) return null;
  return [first.value, second.value];
}

export function StarCanvas({
  observer,
  view,
  timeMs,
  overlays,
  atmosphere,
  planets,
  onDrag,
  onWheel,
  onSunAltitude,
  onPlanning,
}: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const handleRef = useRef<StarView | null>(null);
  const activePointers = useRef<Map<number, PointerPoint>>(new Map());
  const dragState = useRef<{ x: number; y: number; pointerId: number } | null>(null);
  const pinchDistance = useRef<number | null>(null);

  // Mirror props in refs so the long-lived render loop sees fresh values without
  // having to be torn down and rebuilt on every prop change.
  const observerRef = useRef(observer);
  const viewRef = useRef(view);
  const timeRef = useRef(timeMs);
  const overlaysRef = useRef(overlays);
  const atmosphereRef = useRef(atmosphere);
  const planetsRef = useRef(planets);
  observerRef.current = observer;
  viewRef.current = view;
  timeRef.current = timeMs;
  overlaysRef.current = overlays;
  atmosphereRef.current = atmosphere;
  planetsRef.current = planets;

  // Push overlays to wasm whenever the config changes. Geometry is rebuilt on
  // the GPU side, so we don't want to do it every frame -- a useEffect keyed
  // on the config is the right granularity. The ref above protects against the
  // race where the user toggles overlays before wasm finishes booting: the
  // boot effect reads the latest value from the ref, and this effect re-fires
  // on any subsequent change.
  useEffect(() => {
    handleRef.current?.set_overlays(overlays.layers, overlays.gridStepDeg, overlays.opacity);
  }, [overlays]);

  useEffect(() => {
    handleRef.current?.set_planets_enabled(planets.enabled);
  }, [planets]);

  useEffect(() => {
    handleRef.current?.set_atmosphere_config(
      atmosphere.enabled,
      atmosphere.preset,
      atmosphere.turbidity,
      atmosphere.observerAltitudeM,
      atmosphere.ozoneDu,
      atmosphere.visibilityKm,
      atmosphere.pressureHpa,
      atmosphere.temperatureC,
    );
  }, [atmosphere]);

  // Boot wasm + start the render loop. Only ever runs once.
  useEffect(() => {
    let cancelled = false;
    let raf = 0;

    (async () => {
      const wasm = await import("stars-web");
      await wasm.default();
      if (cancelled) return;
      const handle = await wasm.StarView.create("star-canvas");
      handleRef.current = handle;
      // Apply whatever overlay state is current right now -- could be the
      // initial defaults or something the user toggled during the wasm boot.
      const ov = overlaysRef.current;
      handle.set_overlays(ov.layers, ov.gridStepDeg, ov.opacity);
      const at = atmosphereRef.current;
      handle.set_atmosphere_config(
        at.enabled,
        at.preset,
        at.turbidity,
        at.observerAltitudeM,
        at.ozoneDu,
        at.visibilityKm,
        at.pressureHpa,
        at.temperatureC,
      );
      handle.set_planets_enabled(planetsRef.current.enabled);

      let lastSunAltitudePublish = 0;
      let lastPlanningPublish = -Infinity;
      let lastPlanningKey = "";
      const tick = (now: number) => {
        if (cancelled) return;
        const o = observerRef.current;
        const v = viewRef.current;
        handle.set_observer(o.latitudeDeg, o.longitudeDeg, timeRef.current);
        if (now - lastSunAltitudePublish > 1000) {
          lastSunAltitudePublish = now;
          const sunAltitudeDeg = handle.sun_altitude_deg();
          if (Number.isFinite(sunAltitudeDeg)) onSunAltitude(sunAltitudeDeg);
        }
        const planningKey = `${o.latitudeDeg.toFixed(3)},${o.longitudeDeg.toFixed(3)},${Math.floor(timeRef.current / 60000)}`;
        if (planningKey !== lastPlanningKey || now - lastPlanningPublish > 30_000) {
          lastPlanningKey = planningKey;
          lastPlanningPublish = now;
          try {
            onPlanning(JSON.parse(handle.planning_table_json()) as PlanningTable);
          } catch {
            // Keep rendering if a development wasm build returns malformed planning data.
          }
        }
        handle.set_view(toRad(v.azimuthDeg), toRad(v.altitudeDeg), toRad(v.fovDeg));
        handle.render_frame();
        raf = requestAnimationFrame(tick);
      };
      raf = requestAnimationFrame(tick);
    })();

    return () => {
      cancelled = true;
      cancelAnimationFrame(raf);
    };
  }, []);

  // Match canvas backing-store size to its CSS size at the device pixel ratio.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const onResize = () => {
      const dpr = window.devicePixelRatio;
      const w = Math.max(1, Math.floor(canvas.clientWidth * dpr));
      const h = Math.max(1, Math.floor(canvas.clientHeight * dpr));
      canvas.width = w;
      canvas.height = h;
      handleRef.current?.resize(w, h);
    };
    onResize();

    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  return (
    <canvas
      id="star-canvas"
      ref={canvasRef}
      style={{
        width: "100%",
        height: "100%",
        display: "block",
        touchAction: "none",
        cursor: "grab",
      }}
      onPointerDown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId);
        activePointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });

        if (activePointers.current.size === 1) {
          dragState.current = { x: e.clientX, y: e.clientY, pointerId: e.pointerId };
          pinchDistance.current = null;
          return;
        }

        const pinchPoints = firstTwoPointers(activePointers.current);
        dragState.current = null;
        pinchDistance.current = pinchPoints ? distance(...pinchPoints) : null;
      }}
      onPointerMove={(e) => {
        if (!activePointers.current.has(e.pointerId)) return;
        activePointers.current.set(e.pointerId, { x: e.clientX, y: e.clientY });

        if (activePointers.current.size >= 2) {
          const pinchPoints = firstTwoPointers(activePointers.current);
          if (!pinchPoints) return;

          const nextDistance = distance(...pinchPoints);
          const previousDistance = pinchDistance.current;
          pinchDistance.current = nextDistance;
          dragState.current = null;

          if (previousDistance !== null && nextDistance > 0) {
            // Pinch out => larger touch distance => smaller FOV (zoom in).
            onWheel(previousDistance / nextDistance);
          }
          return;
        }

        const d = dragState.current;
        if (!d || d.pointerId !== e.pointerId) return;
        const dx = e.clientX - d.x;
        const dy = e.clientY - d.y;
        d.x = e.clientX;
        d.y = e.clientY;
        // Drag distance in degrees scales with the current FOV so the feel stays
        // constant whether the user is zoomed wide or tight.
        const scale = view.fovDeg / canvasRef.current!.clientHeight;
        onDrag(-dx * scale, dy * scale);
      }}
      onPointerUp={(e) => {
        if (e.currentTarget.hasPointerCapture(e.pointerId)) {
          e.currentTarget.releasePointerCapture(e.pointerId);
        }
        activePointers.current.delete(e.pointerId);
        pinchDistance.current = null;

        if (dragState.current?.pointerId === e.pointerId) dragState.current = null;
        if (activePointers.current.size === 1) {
          const [[pointerId, point]] = activePointers.current;
          dragState.current = { ...point, pointerId };
        }
      }}
      onPointerCancel={(e) => {
        if (e.currentTarget.hasPointerCapture(e.pointerId)) {
          e.currentTarget.releasePointerCapture(e.pointerId);
        }
        activePointers.current.delete(e.pointerId);
        pinchDistance.current = null;

        if (dragState.current?.pointerId === e.pointerId) dragState.current = null;
        if (activePointers.current.size === 1) {
          const [[pointerId, point]] = activePointers.current;
          dragState.current = { ...point, pointerId };
        }
      }}
      onWheel={(e) => {
        // Trackpad pinch / wheel: positive deltaY = zoom out.
        onWheel(Math.exp(e.deltaY * 0.001));
      }}
    />
  );
}
