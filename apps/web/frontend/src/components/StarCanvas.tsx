import { useEffect, useRef } from "react";
import { toRad, type Observer, type OverlayConfig, type View } from "../observer";

type StarViewHandle = {
  set_observer: (lat: number, lng: number, timeUnixMs: number) => void;
  set_view: (az: number, alt: number, fov: number) => void;
  set_overlays: (layers: string[], gridStepDeg: number, opacity: number) => void;
  resize: (w: number, h: number) => void;
  render_frame: () => void;
};

type Props = {
  observer: Observer;
  view: View;
  /** Unix milliseconds for the rendered moment. */
  timeMs: number;
  overlays: OverlayConfig;
  onDrag: (deltaAzDeg: number, deltaAltDeg: number) => void;
  onWheel: (zoomFactor: number) => void;
};

export function StarCanvas({ observer, view, timeMs, overlays, onDrag, onWheel }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const handleRef = useRef<StarViewHandle | null>(null);
  const dragState = useRef<{ x: number; y: number; pointerId: number } | null>(null);

  // Mirror props in refs so the long-lived render loop sees fresh values without
  // having to be torn down and rebuilt on every prop change.
  const observerRef = useRef(observer);
  const viewRef = useRef(view);
  const timeRef = useRef(timeMs);
  const overlaysRef = useRef(overlays);
  observerRef.current = observer;
  viewRef.current = view;
  timeRef.current = timeMs;
  overlaysRef.current = overlays;

  // Push overlays to wasm whenever the config changes. Geometry is rebuilt on
  // the GPU side, so we don't want to do it every frame -- a useEffect keyed
  // on the config is the right granularity. The ref above protects against the
  // race where the user toggles overlays before wasm finishes booting: the
  // boot effect reads the latest value from the ref, and this effect re-fires
  // on any subsequent change.
  useEffect(() => {
    handleRef.current?.set_overlays(overlays.layers, overlays.gridStepDeg, overlays.opacity);
  }, [overlays]);

  // Boot wasm + start the render loop. Only ever runs once.
  useEffect(() => {
    let cancelled = false;
    let raf = 0;

    (async () => {
      const wasm = await import("stars-web");
      await wasm.default();
      if (cancelled) return;
      const handle = (await wasm.StarView.create("star-canvas")) as StarViewHandle;
      handleRef.current = handle;
      // Apply whatever overlay state is current right now -- could be the
      // initial defaults or something the user toggled during the wasm boot.
      const ov = overlaysRef.current;
      handle.set_overlays(ov.layers, ov.gridStepDeg, ov.opacity);

      const tick = () => {
        if (cancelled) return;
        const o = observerRef.current;
        const v = viewRef.current;
        handle.set_observer(o.latitudeDeg, o.longitudeDeg, timeRef.current);
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
        // Drag distance in degrees scales with the current FOV so the feel stays
        // constant whether the user is zoomed wide or tight.
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
        onWheel(Math.exp(e.deltaY * 0.001));
      }}
    />
  );
}
