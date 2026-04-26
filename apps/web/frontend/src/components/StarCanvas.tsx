import { useEffect, useRef } from "react";
import {
  julianDateFromUnixSeconds,
  toRad,
  type Observer,
  type View,
} from "../observer";

type StarViewHandle = {
  set_observer: (lat: number, lng: number, jd: number) => void;
  set_view: (az: number, alt: number, fov: number) => void;
  resize: (w: number, h: number) => void;
  render_frame: () => void;
};

type Props = {
  observer: Observer;
  view: View;
  /** Unix milliseconds for the rendered moment. */
  timeMs: number;
  onDrag: (deltaAzDeg: number, deltaAltDeg: number) => void;
  onWheel: (zoomFactor: number) => void;
};

export function StarCanvas({ observer, view, timeMs, onDrag, onWheel }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const handleRef = useRef<StarViewHandle | null>(null);
  // Mirror props in refs so the long-lived render loop sees fresh values.
  const observerRef = useRef(observer);
  const viewRef = useRef(view);
  const timeRef = useRef(timeMs);
  observerRef.current = observer;
  viewRef.current = view;
  timeRef.current = timeMs;

  useEffect(() => {
    let cancelled = false;
    let raf = 0;

    (async () => {
      const wasm = await import("stars-web");
      await wasm.default();
      if (cancelled) return;
      const handle = (await wasm.StarView.create("star-canvas")) as StarViewHandle;
      handleRef.current = handle;

      const tick = () => {
        if (cancelled) return;
        const o = observerRef.current;
        const v = viewRef.current;
        const jd = julianDateFromUnixSeconds(timeRef.current / 1000);
        handle.set_observer(o.latitudeDeg, o.longitudeDeg, jd);
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

  // Pointer drag → orientation. Use refs so we don't re-bind on every render.
  const dragState = useRef<{ x: number; y: number; pointerId: number } | null>(null);

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
        (e.target as Element).setPointerCapture(e.pointerId);
        dragState.current = { x: e.clientX, y: e.clientY, pointerId: e.pointerId };
      }}
      onPointerMove={(e) => {
        const d = dragState.current;
        if (!d || d.pointerId !== e.pointerId) return;
        const dx = e.clientX - d.x;
        const dy = e.clientY - d.y;
        d.x = e.clientX;
        d.y = e.clientY;
        // Convert pixel drag to degrees of rotation, scaled by current FOV so it
        // feels consistent at any zoom level.
        const scale = view.fovDeg / canvasRef.current!.clientHeight;
        onDrag(-dx * scale, dy * scale);
      }}
      onPointerUp={(e) => {
        if (dragState.current?.pointerId === e.pointerId) dragState.current = null;
      }}
      onPointerCancel={(e) => {
        if (dragState.current?.pointerId === e.pointerId) dragState.current = null;
      }}
      onWheel={(e) => {
        // Trackpad pinch / wheel: positive deltaY = zoom out.
        const factor = Math.exp(e.deltaY * 0.001);
        onWheel(factor);
      }}
    />
  );
}
