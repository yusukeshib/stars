import { useEffect, useRef } from "react";

export function StarCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const wasm = await import("stars-web");
      if (cancelled) return;
      await wasm.start_renderer("star-canvas");
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <canvas
      id="star-canvas"
      ref={canvasRef}
      style={{ width: "100vw", height: "100vh", display: "block" }}
    />
  );
}
