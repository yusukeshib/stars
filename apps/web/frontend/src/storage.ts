import type { Observer, View } from "./observer";

const STORAGE_KEY = "stars:config";
const CURRENT_VERSION = 1;

/// Schema for everything that survives a page reload. Bump `version` when the
/// shape changes in an incompatible way; `loadConfig` returns `null` for any
/// version it doesn't recognize so callers fall back to defaults cleanly.
export type PersistedConfig = {
  version: 1;
  observer: Observer;
  view: View;
};

export type PartialPersistedConfig = Partial<Omit<PersistedConfig, "version">>;

/// Best-effort load. Returns `null` if nothing is stored, the JSON is malformed,
/// or the version is unknown. Individual fields are validated so a broken entry
/// can never crash the app.
export function loadConfig(): PartialPersistedConfig | null {
  if (typeof localStorage === "undefined") return null;
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object") return null;
    const obj = parsed as { version?: unknown; observer?: unknown; view?: unknown };
    if (obj.version !== CURRENT_VERSION) return null;
    const out: PartialPersistedConfig = {};
    if (isObserver(obj.observer)) out.observer = obj.observer;
    if (isView(obj.view)) out.view = obj.view;
    return out;
  } catch {
    return null;
  }
}

/// Fire-and-forget save. localStorage exceptions (quota, private mode) are
/// swallowed; persistence is a nicety, not a correctness requirement.
export function saveConfig(config: Omit<PersistedConfig, "version">): void {
  if (typeof localStorage === "undefined") return;
  try {
    const payload: PersistedConfig = { version: CURRENT_VERSION, ...config };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(payload));
  } catch {
    // Ignore: storage may be full, disabled, or unavailable in private mode.
  }
}

function isObserver(v: unknown): v is Observer {
  if (!v || typeof v !== "object") return false;
  const o = v as Partial<Observer>;
  return Number.isFinite(o.latitudeDeg) && Number.isFinite(o.longitudeDeg);
}

function isView(v: unknown): v is View {
  if (!v || typeof v !== "object") return false;
  const o = v as Partial<View>;
  return (
    Number.isFinite(o.azimuthDeg) &&
    Number.isFinite(o.altitudeDeg) &&
    Number.isFinite(o.fovDeg)
  );
}
