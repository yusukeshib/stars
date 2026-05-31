import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

/// L-24 accessibility test harness.
///
/// A deliberately minimal vitest config kept separate from `vite.config.ts`:
/// the app build pulls in `vite-plugin-wasm` + a `stars-web` alias to the
/// generated `./pkg` directory, which does not exist until `wasm-pack`
/// runs. The a11y tests only mount dependency-free presentational components
/// (they never statically import `stars-web`), so the test build needs just
/// React + a jsdom DOM and avoids requiring the WASM artifact in CI.
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: false,
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["src/test/setup.ts"],
  },
});
