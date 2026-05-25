import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

export default defineConfig({
  base: process.env.VITE_BASE_PATH ?? "/",
  plugins: [react(), wasm(), topLevelAwait()],
  resolve: {
    alias: {
      // `wasm-pack` writes this package into ./pkg. Keep it out of
      // package.json so `bun install` succeeds before the generated files exist.
      "stars-web": new URL("./pkg/stars_web.js", import.meta.url).pathname,
    },
  },
  build: {
    target: "esnext",
  },
  optimizeDeps: {
    exclude: ["stars-web"],
  },
});
