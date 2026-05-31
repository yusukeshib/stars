import { useEffect, useRef, useState } from "react";
import { StarCanvas } from "./components/StarCanvas";
import { SearchPanel } from "./components/SearchPanel";
import { StatusBar } from "./components/StatusBar";
import {
  clampAltitude,
  clampFov,
  wrapAzimuth,
  sanitizedOpticalDesign,
  DEFAULT_ATMOSPHERE_CONFIG,
  DEFAULT_SCINTILLATION_CONFIG,
  DEFAULT_EYEPIECE_CONFIG,
  DEFAULT_METEORS_CONFIG,
  DEFAULT_OVERLAY_CONFIG,
  DEFAULT_PLANETS_CONFIG,
  DEFAULT_SATELLITES_CONFIG,
  DEFAULT_AURORA_CONFIG,
  DEFAULT_PROJECTION_CONFIG,
  DEFAULT_OUTPUT_COLOURSPACE,
  MIN_FOV_DEG,
  MAX_FOV_DEG,
  isAtmospherePreset,
  isAuroraSeason,
  isOverlayLayer,
  isSkyProjection,
  isSkyViewpoint,
  type AtmosphereConfig,
  type AuroraConfig,
  type ScintillationConfig,
  type EyepieceConfig,
  type ExternalViewpointConfig,
  type Observer,
  type OutputColourspace,
  type OverlayConfig,
  type PlanetsConfig,
  type MeteorsConfig,
  type PlanningTable,
  type ProjectionConfig,
  type RecommendedPlan,
  type SatellitesConfig,
  type View,
} from "./observer";
import { loadConfig, saveConfig } from "./storage";
import { parseStarSessionJson, starSessionJson, type SessionState } from "./session";
import { useT } from "./i18n";

const DEFAULT_OBSERVER: Observer = {
  latitudeDeg: 35.68, // Tokyo as a sensible default
  longitudeDeg: 139.69,
};

/// L-09: trigger a client-side download of the iCalendar plan produced by the
/// WASM bridge. No-op if the bridge is not ready yet or returns no events.
function exportIcal(ics: string | undefined): void {
  if (!ics || !ics.includes("BEGIN:VEVENT")) return;
  const blob = new Blob([ics], { type: "text/calendar;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = "stars-observation-plan.ics";
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

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

type UrlSession = Partial<SessionState>;

function loadAtmosphereFromUrl(params?: URLSearchParams): AtmosphereConfig | null {
  if (typeof window === "undefined") return null;
  params ??= new URLSearchParams(window.location.search);
  if (
    !params.has("atmosphere") &&
    !params.has("atmospherePreset") &&
    !params.has("aerosolBeta") &&
    !params.has("aerosolAlpha") &&
    !params.has("observerAltitudeM") &&
    !params.has("ozoneDu") &&
    !params.has("pressureHpa") &&
    !params.has("temperatureC") &&
    !params.has("surfaceAlbedo")
  ) {
    return null;
  }

  const enabled = params.get("atmosphere") !== "off";
  const presetParam = params.get("atmospherePreset");
  return {
    ...DEFAULT_ATMOSPHERE_CONFIG,
    enabled,
    preset: isAtmospherePreset(presetParam) ? presetParam : DEFAULT_ATMOSPHERE_CONFIG.preset,
    aerosolBeta: numberParam(params, "aerosolBeta", DEFAULT_ATMOSPHERE_CONFIG.aerosolBeta, 0, 2),
    aerosolAlpha: numberParam(params, "aerosolAlpha", DEFAULT_ATMOSPHERE_CONFIG.aerosolAlpha, 0, 4),
    observerAltitudeM: numberParam(
      params,
      "observerAltitudeM",
      DEFAULT_ATMOSPHERE_CONFIG.observerAltitudeM,
      0,
      9000,
    ),
    ozoneDu: numberParam(params, "ozoneDu", DEFAULT_ATMOSPHERE_CONFIG.ozoneDu, 0, 600),
    pressureHpa: numberParam(params, "pressureHpa", DEFAULT_ATMOSPHERE_CONFIG.pressureHpa, 0, 1100),
    temperatureC: numberParam(
      params,
      "temperatureC",
      DEFAULT_ATMOSPHERE_CONFIG.temperatureC,
      -80,
      60,
    ),
    surfaceAlbedo: numberParam(
      params,
      "surfaceAlbedo",
      DEFAULT_ATMOSPHERE_CONFIG.surfaceAlbedo,
      0,
      1,
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
    "deepSkyMag",
    "planets",
    "satellites",
    "satExposure",
    "aurora",
    "auroraKp",
    "auroraSeason",
    "projection",
    "viewpoint",
    "originPc",
    "targetPc",
    "up",
    "atmosphere",
    "atmospherePreset",
    "aerosolBeta",
    "aerosolAlpha",
    "observerAltitudeM",
    "ozoneDu",
    "pressureHpa",
    "temperatureC",
    "surfaceAlbedo",
    "eyepiece",
    "otaApertureMm",
    "otaFocalMm",
    "eyepieceFocalMm",
    "eyepieceAfovDeg",
    "eyepieceFieldStopMm",
  ];
  if (!sessionKeys.some((key) => params.has(key))) return null;
  const observer: Observer = {
    latitudeDeg: numberParam(params, "lat", DEFAULT_OBSERVER.latitudeDeg, -90, 90),
    longitudeDeg: numberParam(params, "lng", DEFAULT_OBSERVER.longitudeDeg, -180, 180),
  };
  const view: View = {
    azimuthDeg: numberParam(params, "az", DEFAULT_VIEW.azimuthDeg, 0, 360),
    altitudeDeg: numberParam(params, "alt", DEFAULT_VIEW.altitudeDeg, -89.5, 89.5),
    fovDeg: numberParam(params, "fov", DEFAULT_VIEW.fovDeg, MIN_FOV_DEG, MAX_FOV_DEG),
  };
  const overlayLayers = (params.get("overlays") ?? "")
    .split(",")
    .filter(isOverlayLayer)
    .filter((layer) => layer !== "cardinals");
  const overlays: OverlayConfig = {
    layers: overlayLayers.length > 0 ? overlayLayers : DEFAULT_OVERLAY_CONFIG.layers,
    gridStepDeg: numberParam(params, "grid", DEFAULT_OVERLAY_CONFIG.gridStepDeg, 1, 90),
    opacity: numberParam(params, "overlayOpacity", DEFAULT_OVERLAY_CONFIG.opacity, 0, 1),
    deepSkyMagnitudeLimit: numberParam(
      params,
      "deepSkyMag",
      DEFAULT_OVERLAY_CONFIG.deepSkyMagnitudeLimit,
      -5,
      99,
    ),
  };
  const jd = Number(params.get("jd"));
  const eyepieceParam = params.get("eyepiece");
  const hasEyepieceParam =
    eyepieceParam !== null ||
    params.has("otaApertureMm") ||
    params.has("otaFocalMm") ||
    params.has("eyepieceFocalMm") ||
    params.has("eyepieceAfovDeg") ||
    params.has("eyepieceFieldStopMm");
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
    satellites: {
      enabled: params.get("satellites") === "on",
      exposureSeconds: numberParam(params, "satExposure", DEFAULT_SATELLITES_CONFIG.exposureSeconds, 0, 600),
    },
    aurora: {
      enabled: params.get("aurora") === "on",
      kp: numberParam(params, "auroraKp", DEFAULT_AURORA_CONFIG.kp, 0, 9),
      season: (() => {
        const s = params.get("auroraSeason");
        return isAuroraSeason(s) ? s : DEFAULT_AURORA_CONFIG.season;
      })(),
    },
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
    eyepiece: hasEyepieceParam
      ? {
          enabled: eyepieceParam === null ? true : eyepieceParam !== "off",
          apertureMm: numberParam(params, "otaApertureMm", DEFAULT_EYEPIECE_CONFIG.apertureMm, 10, 2000),
          focalLengthMm: numberParam(params, "otaFocalMm", DEFAULT_EYEPIECE_CONFIG.focalLengthMm, 50, 20000),
          eyepieceFocalLengthMm: numberParam(params, "eyepieceFocalMm", DEFAULT_EYEPIECE_CONFIG.eyepieceFocalLengthMm, 1, 100),
          apparentFovDeg: numberParam(params, "eyepieceAfovDeg", DEFAULT_EYEPIECE_CONFIG.apparentFovDeg, 1, 120),
          fieldStopMm: numberParam(params, "eyepieceFieldStopMm", DEFAULT_EYEPIECE_CONFIG.fieldStopMm, 0, 120),
          opticalDesign: sanitizedOpticalDesign(params.get("telescopeDesign")),
          spiderVanes: numberParam(params, "spiderVanes", DEFAULT_EYEPIECE_CONFIG.spiderVanes, 1, 8),
          otaRotationDeg: numberParam(params, "otaRotationDeg", DEFAULT_EYEPIECE_CONFIG.otaRotationDeg, 0, 360),
        }
      : undefined,
    timeMs: Number.isFinite(jd) ? (jd - 2440587.5) * 86400000 : undefined,
  };
}

function sessionUrl({ observer, view, overlays, atmosphere, planets, satellites, aurora, projection, eyepiece, timeMs }: {
  observer: Observer;
  view: View;
  overlays: OverlayConfig;
  atmosphere: AtmosphereConfig;
  planets: PlanetsConfig;
  satellites: SatellitesConfig;
  aurora: AuroraConfig;
  projection: ProjectionConfig;
  eyepiece: EyepieceConfig;
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
  url.searchParams.set("deepSkyMag", overlays.deepSkyMagnitudeLimit.toFixed(1));
  url.searchParams.set("planets", planets.enabled ? "on" : "off");
  url.searchParams.set("satellites", satellites.enabled ? "on" : "off");
  url.searchParams.set("satExposure", satellites.exposureSeconds.toFixed(1));
  url.searchParams.set("aurora", aurora.enabled ? "on" : "off");
  url.searchParams.set("auroraKp", aurora.kp.toFixed(1));
  url.searchParams.set("auroraSeason", aurora.season);
  url.searchParams.set("projection", projection.projection);
  url.searchParams.set("viewpoint", projection.viewpoint);
  url.searchParams.set("originPc", vec3SearchParam(projection.external.originPc));
  url.searchParams.set("targetPc", vec3SearchParam(projection.external.targetPc));
  url.searchParams.set("up", vec3SearchParam(projection.external.up));
  url.searchParams.set("atmosphere", atmosphere.enabled ? "on" : "off");
  url.searchParams.set("atmospherePreset", atmosphere.preset);
  url.searchParams.set("aerosolBeta", atmosphere.aerosolBeta.toFixed(3));
  url.searchParams.set("aerosolAlpha", atmosphere.aerosolAlpha.toFixed(2));
  url.searchParams.set("observerAltitudeM", String(Math.round(atmosphere.observerAltitudeM)));
  url.searchParams.set("ozoneDu", String(Math.round(atmosphere.ozoneDu)));
  url.searchParams.set("pressureHpa", String(Math.round(atmosphere.pressureHpa)));
  url.searchParams.set("temperatureC", atmosphere.temperatureC.toFixed(0));
  url.searchParams.set("surfaceAlbedo", atmosphere.surfaceAlbedo.toFixed(2));
  url.searchParams.set("eyepiece", eyepiece.enabled ? "on" : "off");
  url.searchParams.set("otaApertureMm", eyepiece.apertureMm.toFixed(0));
  url.searchParams.set("otaFocalMm", eyepiece.focalLengthMm.toFixed(0));
  url.searchParams.set("eyepieceFocalMm", eyepiece.eyepieceFocalLengthMm.toFixed(1));
  url.searchParams.set("eyepieceAfovDeg", eyepiece.apparentFovDeg.toFixed(1));
  url.searchParams.set("eyepieceFieldStopMm", eyepiece.fieldStopMm.toFixed(1));
  url.searchParams.set("telescopeDesign", eyepiece.opticalDesign);
  url.searchParams.set("spiderVanes", String(eyepiece.spiderVanes));
  url.searchParams.set("otaRotationDeg", eyepiece.otaRotationDeg.toFixed(0));
  return url.toString();
}

// Read once at module load.
const PERSISTED = loadConfig();
const URL_SESSION = loadSessionFromUrl();
const URL_ATMOSPHERE = URL_SESSION?.atmosphere ?? (typeof window !== "undefined" ? loadAtmosphereFromUrl() : null);

export function App() {
  const t = useT();
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
  const [scintillation, setScintillation] = useState<ScintillationConfig>(
    URL_SESSION?.scintillation ?? PERSISTED?.scintillation ?? DEFAULT_SCINTILLATION_CONFIG,
  );
  const [planets, setPlanets] = useState<PlanetsConfig>(
    URL_SESSION?.planets ?? PERSISTED?.planets ?? DEFAULT_PLANETS_CONFIG,
  );
  const [satellites, setSatellites] = useState<SatellitesConfig>(
    URL_SESSION?.satellites ?? PERSISTED?.satellites ?? DEFAULT_SATELLITES_CONFIG,
  );
  const [meteors, setMeteors] = useState<MeteorsConfig>(
    URL_SESSION?.meteors ?? PERSISTED?.meteors ?? DEFAULT_METEORS_CONFIG,
  );
  const [aurora, setAurora] = useState<AuroraConfig>(
    URL_SESSION?.aurora ?? PERSISTED?.aurora ?? DEFAULT_AURORA_CONFIG,
  );
  const [projection, setProjection] = useState<ProjectionConfig>(
    URL_SESSION?.projection ?? PERSISTED?.projection ?? DEFAULT_PROJECTION_CONFIG,
  );
  const [eyepiece, setEyepiece] = useState<EyepieceConfig>(
    URL_SESSION?.eyepiece ?? PERSISTED?.eyepiece ?? DEFAULT_EYEPIECE_CONFIG,
  );
  const [outputColourspace, setOutputColourspace] = useState<OutputColourspace>(
    URL_SESSION?.outputColourspace ?? PERSISTED?.outputColourspace ?? DEFAULT_OUTPUT_COLOURSPACE,
  );
  const [timeMs, setTimeMs] = useState<number>(() => URL_SESSION?.timeMs ?? Date.now());
  const [sunAltitudeDeg, setSunAltitudeDeg] = useState<number | null>(null);
  const [planning, setPlanning] = useState<PlanningTable | null>(null);
  const [recommended, setRecommended] = useState<RecommendedPlan | null>(null);
  const lastTickRef = useRef<number>(performance.now());
  // V-56 search/GoTo: the WASM `StarView` is owned by `StarCanvas`. We hold
  // a stable proxy in a ref so the search panel can call into it without
  // forcing a re-render of the renderer on every search-state change.
  const searchApiRef = useRef<
    | {
        lookup: (query: string, limit: number) => string;
        goto: (id: string) => string;
        planningIcal: () => string;
      }
    | null
  >(null);

  // Persist observer + view + overlays + atmosphere + planets + projection + eyepiece whenever they change. We debounce
  // because the view updates on every mouse/touch frame during a drag, and
  // localStorage.setItem is synchronous; without the debounce we'd write ~60
  // times a second. Time is intentionally not persisted: a stale timestamp on
  // next load would silently mislead the user.
  useEffect(() => {
    const handle = setTimeout(
      () => saveConfig({ observer, view, overlays, atmosphere, scintillation, planets, satellites, meteors, aurora, projection, eyepiece, outputColourspace }),
      250,
    );
    return () => clearTimeout(handle);
  }, [observer, view, overlays, atmosphere, scintillation, planets, satellites, meteors, aurora, projection, eyepiece, outputColourspace]);

  // Mirror the current session into the address bar so the user can copy the
  // URL at any time without going through the explicit "Copy URL" action.
  // Debounced for the same reason as the persistence effect (scrubber drags
  // hit ~60 Hz). `timeMs` is intentionally absent from the dependency list:
  // including it would re-trigger this effect every animation frame as the
  // clock ticks, churning the address bar. The closure still reads the live
  // `timeMs` when the debounce fires, so the URL captures the moment at
  // which the user last touched any parameter.
  useEffect(() => {
    if (typeof window === "undefined") return;
    const handle = setTimeout(() => {
      const url = sessionUrl({ observer, view, overlays, atmosphere, planets, satellites, aurora, projection, eyepiece, timeMs });
      window.history.replaceState(null, "", url);
    }, 250);
    return () => clearTimeout(handle);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- timeMs deliberately excluded; see comment above.
  }, [observer, view, overlays, atmosphere, scintillation, planets, satellites, aurora, projection, eyepiece]);

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

  const currentSessionState = (): SessionState => ({
    observer,
    view,
    overlays,
    atmosphere,
    scintillation,
    planets,
    satellites,
    meteors,
    aurora,
    projection,
    eyepiece,
    outputColourspace,
    timeMs,
  });

  const applySessionState = (session: SessionState) => {
    setObserver(session.observer);
    setView(session.view);
    setOverlays(normalizeWebOverlays(session.overlays));
    setAtmosphere(session.atmosphere);
    setScintillation(session.scintillation);
    setPlanets(session.planets);
    setSatellites(session.satellites);
    setMeteors(session.meteors);
    setAurora(session.aurora);
    setProjection(session.projection);
    setEyepiece(session.eyepiece);
    setOutputColourspace(session.outputColourspace);
    setTimeMs(session.timeMs);
    lastTickRef.current = performance.now();
  };

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
        scintillation={scintillation}
        planets={planets}
        satellites={satellites}
        meteors={meteors}
        aurora={aurora}
        projection={projection}
        eyepiece={eyepiece}
        outputColourspace={outputColourspace}
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
        onRecommended={setRecommended}
        onSearchReady={(api) => {
          searchApiRef.current = api;
        }}
      />
      <SearchPanel
        onLookup={(query, limit) => searchApiRef.current?.lookup(query, limit) ?? "{\"matches\":[]}"}
        onGoto={(id) => searchApiRef.current?.goto(id) ?? "null"}
        onApplyView={(azRad, altRad) => {
          // V-56 GoTo: snap-and-recentre. A smooth interpolation is the
          // host's job in a follow-up; this slice keeps the wiring linear.
          const azDeg = ((azRad * 180) / Math.PI + 360) % 360;
          const altDeg = (altRad * 180) / Math.PI;
          setView((v) => ({
            ...v,
            azimuthDeg: wrapAzimuth(azDeg),
            altitudeDeg: clampAltitude(altDeg),
          }));
        }}
      />
      <StatusBar
        observer={observer}
        view={view}
        timeMs={timeMs}
        sunAltitudeDeg={sunAltitudeDeg}
        overlays={overlays}
        atmosphere={atmosphere}
        planets={planets}
        satellites={satellites}
        meteors={meteors}
        aurora={aurora}
        projection={projection}
        eyepiece={eyepiece}
        planning={planning}
        recommended={recommended}
        onExportIcal={() => exportIcal(searchApiRef.current?.planningIcal())}
        onSetObserver={setObserver}
        onSetTime={setTimeMs}
        onSetOverlays={setOverlays}
        onSetAtmosphere={setAtmosphere}
        onSetPlanets={setPlanets}
        onSetSatellites={setSatellites}
        onSetMeteors={setMeteors}
        onSetAurora={setAurora}
        onSetProjection={setProjection}
        onSetEyepiece={setEyepiece}
        outputColourspace={outputColourspace}
        onSetOutputColourspace={setOutputColourspace}
        onSetView={setView}
        onCopySessionJson={async () => {
          const json = starSessionJson(currentSessionState());
          let copied = false;
          try {
            if (navigator.clipboard?.writeText) {
              await navigator.clipboard.writeText(json);
              copied = true;
            }
          } catch {
            copied = false;
          }
          if (!copied) {
            const blob = new Blob([json], { type: "application/json" });
            const url = URL.createObjectURL(blob);
            const link = document.createElement("a");
            link.href = url;
            link.download = "stars-session.json";
            link.click();
            URL.revokeObjectURL(url);
          }
        }}
        onImportSessionJson={(raw) => {
          try {
            applySessionState(parseStarSessionJson(raw));
          } catch (error) {
            // Session-parser errors carry developer-facing messages from
            // `session.ts` (e.g. "Unsupported session schemaVersion"). They
            // stay in English; only the generic fallback is translated.
            window.alert(
              error instanceof Error ? error.message : t("card.session.invalidJson"),
            );
          }
        }}
        onUseGeolocation={useGeolocation}
      />
    </>
  );
}
