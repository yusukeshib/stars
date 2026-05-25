import { useEffect, useRef, useState } from "react";
import { StarCanvas } from "./components/StarCanvas";
import { StatusBar } from "./components/StatusBar";
import {
  clampAltitude,
  clampFov,
  wrapAzimuth,
  DEFAULT_ATMOSPHERE_CONFIG,
  DEFAULT_OVERLAY_CONFIG,
  DEFAULT_PLANETS_CONFIG,
  DEFAULT_PROJECTION_CONFIG,
  isAtmospherePreset,
  isOverlayLayer,
  isSkyProjection,
  isSkyViewpoint,
  type AtmosphereConfig,
  type ExternalViewpointConfig,
  type Observer,
  type OverlayConfig,
  type PlanetsConfig,
  type PlanningTable,
  type ProjectionConfig,
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

const numberParam = (
  params: URLSearchParams,
  key: string,
  fallback: number,
  min: number,
  max: number,
): number => {
  const raw = params.get(key);
  if (raw === null) return fallback;
  if (raw.trim() === "") return fallback;
  const value = Number(raw);
  return Number.isFinite(value) ? Math.max(min, Math.min(max, value)) : fallback;
};

const vec3Param = (
  params: URLSearchParams,
  key: string,
  fallback: { x: number; y: number; z: number },
  min: number,
  max: number,
): { x: number; y: number; z: number } => {
  const parts = (params.get(key) ?? "").split(",").map((part) => Number(part));
  if (parts.length !== 3 || parts.some((value) => !Number.isFinite(value))) return fallback;
  const clamp = (value: number) => Math.max(min, Math.min(max, value));
  return { x: clamp(parts[0]), y: clamp(parts[1]), z: clamp(parts[2]) };
};

const vec3SearchParam = ({ x, y, z }: { x: number; y: number; z: number }): string =>
  `${x.toFixed(1)},${y.toFixed(1)},${z.toFixed(1)}`;

type UrlSession = {
  observer?: Observer;
  view?: View;
  overlays?: OverlayConfig;
  atmosphere?: AtmosphereConfig;
  planets?: PlanetsConfig;
  projection?: ProjectionConfig;
  timeMs?: number;
};

function loadAtmosphereFromUrl(params?: URLSearchParams): AtmosphereConfig | null {
  if (typeof window === "undefined") return null;
  params ??= new URLSearchParams(window.location.search);
  if (
    !params.has("atmosphere") &&
    !params.has("atmospherePreset") &&
    !params.has("turbidity") &&
    !params.has("observerAltitudeM") &&
    !params.has("ozoneDu") &&
    !params.has("visibilityKm") &&
    !params.has("pressureHpa") &&
    !params.has("temperatureC")
  ) {
    return null;
  }

  const enabled = params.get("atmosphere") !== "off";
  const presetParam = params.get("atmospherePreset");
  return {
    ...DEFAULT_ATMOSPHERE_CONFIG,
    enabled,
    preset: isAtmospherePreset(presetParam) ? presetParam : DEFAULT_ATMOSPHERE_CONFIG.preset,
    turbidity: numberParam(params, "turbidity", DEFAULT_ATMOSPHERE_CONFIG.turbidity, 1.7, 10),
    observerAltitudeM: numberParam(
      params,
      "observerAltitudeM",
      DEFAULT_ATMOSPHERE_CONFIG.observerAltitudeM,
      0,
      9000,
    ),
    ozoneDu: numberParam(params, "ozoneDu", DEFAULT_ATMOSPHERE_CONFIG.ozoneDu, 0, 600),
    visibilityKm: numberParam(
      params,
      "visibilityKm",
      DEFAULT_ATMOSPHERE_CONFIG.visibilityKm,
      1,
      200,
    ),
    pressureHpa: numberParam(params, "pressureHpa", DEFAULT_ATMOSPHERE_CONFIG.pressureHpa, 0, 1100),
    temperatureC: numberParam(
      params,
      "temperatureC",
      DEFAULT_ATMOSPHERE_CONFIG.temperatureC,
      -80,
      60,
    ),
  };
}

function loadSessionFromUrl(): UrlSession | null {
  if (typeof window === "undefined") return null;
  const params = new URLSearchParams(window.location.search);
  const sessionKeys = [
    "lat",
    "lng",
    "jd",
    "az",
    "alt",
    "fov",
    "overlays",
    "grid",
    "overlayOpacity",
    "planets",
    "projection",
    "viewpoint",
    "originPc",
    "targetPc",
    "up",
    "atmosphere",
    "atmospherePreset",
    "turbidity",
    "observerAltitudeM",
    "ozoneDu",
    "visibilityKm",
    "pressureHpa",
    "temperatureC",
  ];
  if (!sessionKeys.some((key) => params.has(key))) return null;
  const observer: Observer = {
    latitudeDeg: numberParam(params, "lat", DEFAULT_OBSERVER.latitudeDeg, -90, 90),
    longitudeDeg: numberParam(params, "lng", DEFAULT_OBSERVER.longitudeDeg, -180, 180),
  };
  const view: View = {
    azimuthDeg: numberParam(params, "az", DEFAULT_VIEW.azimuthDeg, 0, 360),
    altitudeDeg: numberParam(params, "alt", DEFAULT_VIEW.altitudeDeg, -89.5, 89.5),
    fovDeg: numberParam(params, "fov", DEFAULT_VIEW.fovDeg, 5, 120),
  };
  const overlayLayers = (params.get("overlays") ?? "")
    .split(",")
    .filter(isOverlayLayer)
    .filter((layer) => layer !== "cardinals");
  const overlays: OverlayConfig = {
    layers: overlayLayers.length > 0 ? overlayLayers : DEFAULT_OVERLAY_CONFIG.layers,
    gridStepDeg: numberParam(params, "grid", DEFAULT_OVERLAY_CONFIG.gridStepDeg, 1, 90),
    opacity: numberParam(params, "overlayOpacity", DEFAULT_OVERLAY_CONFIG.opacity, 0, 1),
  };
  const jd = Number(params.get("jd"));
  const projectionParam = params.get("projection");
  const viewpointParam = params.get("viewpoint");
  const hasExternalViewpointParam = params.has("originPc") || params.has("targetPc") || params.has("up");
  const external: ExternalViewpointConfig = {
    originPc: vec3Param(params, "originPc", DEFAULT_PROJECTION_CONFIG.external.originPc, -1_000_000, 1_000_000),
    targetPc: vec3Param(params, "targetPc", DEFAULT_PROJECTION_CONFIG.external.targetPc, -1_000_000, 1_000_000),
    up: vec3Param(params, "up", DEFAULT_PROJECTION_CONFIG.external.up, -10, 10),
  };
  return {
    observer,
    view,
    overlays,
    atmosphere: loadAtmosphereFromUrl(params) ?? undefined,
    planets: { enabled: params.get("planets") !== "off" },
    projection: {
      projection: isSkyProjection(projectionParam)
        ? projectionParam
        : DEFAULT_PROJECTION_CONFIG.projection,
      viewpoint: isSkyViewpoint(viewpointParam)
        ? viewpointParam
        : hasExternalViewpointParam
          ? "custom-external"
          : DEFAULT_PROJECTION_CONFIG.viewpoint,
      external,
    },
    timeMs: Number.isFinite(jd) ? (jd - 2440587.5) * 86400000 : undefined,
  };
}

function sessionUrl({ observer, view, overlays, atmosphere, planets, projection, timeMs }: {
  observer: Observer;
  view: View;
  overlays: OverlayConfig;
  atmosphere: AtmosphereConfig;
  planets: PlanetsConfig;
  projection: ProjectionConfig;
  timeMs: number;
}): string {
  const url = new URL(window.location.href);
  url.search = "";
  url.searchParams.set("lat", observer.latitudeDeg.toFixed(5));
  url.searchParams.set("lng", observer.longitudeDeg.toFixed(5));
  url.searchParams.set("jd", (timeMs / 86400000 + 2440587.5).toFixed(7));
  url.searchParams.set("az", view.azimuthDeg.toFixed(2));
  url.searchParams.set("alt", view.altitudeDeg.toFixed(2));
  url.searchParams.set("fov", view.fovDeg.toFixed(2));
  url.searchParams.set("overlays", overlays.layers.join(","));
  url.searchParams.set("grid", overlays.gridStepDeg.toFixed(0));
  url.searchParams.set("overlayOpacity", overlays.opacity.toFixed(2));
  url.searchParams.set("planets", planets.enabled ? "on" : "off");
  url.searchParams.set("projection", projection.projection);
  url.searchParams.set("viewpoint", projection.viewpoint);
  url.searchParams.set("originPc", vec3SearchParam(projection.external.originPc));
  url.searchParams.set("targetPc", vec3SearchParam(projection.external.targetPc));
  url.searchParams.set("up", vec3SearchParam(projection.external.up));
  url.searchParams.set("atmosphere", atmosphere.enabled ? "on" : "off");
  url.searchParams.set("atmospherePreset", atmosphere.preset);
  url.searchParams.set("turbidity", atmosphere.turbidity.toFixed(1));
  url.searchParams.set("observerAltitudeM", String(Math.round(atmosphere.observerAltitudeM)));
  url.searchParams.set("ozoneDu", String(Math.round(atmosphere.ozoneDu)));
  url.searchParams.set("visibilityKm", atmosphere.visibilityKm.toFixed(0));
  url.searchParams.set("pressureHpa", String(Math.round(atmosphere.pressureHpa)));
  url.searchParams.set("temperatureC", atmosphere.temperatureC.toFixed(0));
  return url.toString();
}

// Read once at module load.
const PERSISTED = loadConfig();
const URL_SESSION = loadSessionFromUrl();
const URL_ATMOSPHERE = URL_SESSION?.atmosphere ?? (typeof window !== "undefined" ? loadAtmosphereFromUrl() : null);

export function App() {
  const [observer, setObserver] = useState<Observer>(URL_SESSION?.observer ?? PERSISTED?.observer ?? DEFAULT_OBSERVER);
  const [view, setView] = useState<View>(URL_SESSION?.view ?? PERSISTED?.view ?? DEFAULT_VIEW);
  const [overlays, setOverlays] = useState<OverlayConfig>(
    URL_SESSION?.overlays
      ? normalizeWebOverlays(URL_SESSION.overlays)
      : PERSISTED?.overlays
        ? normalizeWebOverlays(PERSISTED.overlays)
        : DEFAULT_OVERLAY_CONFIG,
  );
  const [atmosphere, setAtmosphere] = useState<AtmosphereConfig>(
    URL_ATMOSPHERE ?? PERSISTED?.atmosphere ?? DEFAULT_ATMOSPHERE_CONFIG,
  );
  const [planets, setPlanets] = useState<PlanetsConfig>(
    URL_SESSION?.planets ?? PERSISTED?.planets ?? DEFAULT_PLANETS_CONFIG,
  );
  const [projection, setProjection] = useState<ProjectionConfig>(
    URL_SESSION?.projection ?? PERSISTED?.projection ?? DEFAULT_PROJECTION_CONFIG,
  );
  const [timeMs, setTimeMs] = useState<number>(() => URL_SESSION?.timeMs ?? Date.now());
  const [sunAltitudeDeg, setSunAltitudeDeg] = useState<number | null>(null);
  const [planning, setPlanning] = useState<PlanningTable | null>(null);
  const lastTickRef = useRef<number>(performance.now());

  // Persist observer + view + overlays + atmosphere + planets + projection whenever they change. We debounce
  // because the view updates on every mouse/touch frame during a drag, and
  // localStorage.setItem is synchronous; without the debounce we'd write ~60
  // times a second. Time is intentionally not persisted: a stale timestamp on
  // next load would silently mislead the user.
  useEffect(() => {
    const handle = setTimeout(() => saveConfig({ observer, view, overlays, atmosphere, planets, projection }), 250);
    return () => clearTimeout(handle);
  }, [observer, view, overlays, atmosphere, planets, projection]);

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
        planets={planets}
        projection={projection}
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
        onSunAltitude={setSunAltitudeDeg}
        onPlanning={setPlanning}
      />
      <StatusBar
        observer={observer}
        view={view}
        timeMs={timeMs}
        sunAltitudeDeg={sunAltitudeDeg}
        overlays={overlays}
        atmosphere={atmosphere}
        planets={planets}
        projection={projection}
        planning={planning}
        onSetObserver={setObserver}
        onSetTime={setTimeMs}
        onSetOverlays={setOverlays}
        onSetAtmosphere={setAtmosphere}
        onSetPlanets={setPlanets}
        onSetProjection={setProjection}
        onCopySessionUrl={async () => {
          const url = sessionUrl({ observer, view, overlays, atmosphere, planets, projection, timeMs });
          try {
            if (navigator.clipboard?.writeText) await navigator.clipboard.writeText(url);
          } catch {
            // Clipboard permission is best-effort; updating the address bar still shares the session.
          }
          window.history.replaceState(null, "", url);
        }}
        onUseGeolocation={useGeolocation}
      />
    </>
  );
}
