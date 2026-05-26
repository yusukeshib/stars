import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

/// Web-frontend i18n.
///
/// Intentionally dependency-free: this app only needs two locales, simple
/// `{token}` interpolation, and ~150 strings. Pulling in a full library
/// (i18next, react-intl) would add tens of KB to the WASM-heavy bundle for
/// no real win at this scale.
///
/// The wasm renderer still emits English strings for things like
/// `"Civil twilight"` and `"Mercury"` (see `crates/astronomy/src/planning.rs`).
/// Those canonical English strings are mapped to translation keys at the call
/// site via `translateWasmBody` / `translateWasmTwilight` so the renderer
/// stays locale-agnostic.

export const LOCALES = ["en", "ja"] as const;
export type Locale = (typeof LOCALES)[number];
export const DEFAULT_LOCALE: Locale = "en";

export const LOCALE_LABELS: Record<Locale, string> = {
  en: "English",
  ja: "日本語",
};

const isLocale = (s: unknown): s is Locale =>
  typeof s === "string" && (LOCALES as readonly string[]).includes(s);

const STORAGE_KEY = "stars:locale";

/// Resolve the initial locale at module load.
///
/// Priority:
///   1. `?lang=` URL parameter (so shared session URLs can pin a language).
///   2. `localStorage["stars:locale"]` (so the user's last manual pick wins).
///   3. `navigator.language` / `navigator.languages` prefix match.
///   4. `DEFAULT_LOCALE` ("en").
function detectInitialLocale(): Locale {
  if (typeof window === "undefined") return DEFAULT_LOCALE;
  const params = new URLSearchParams(window.location.search);
  const urlLang = params.get("lang");
  if (isLocale(urlLang)) return urlLang;
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (isLocale(stored)) return stored;
  } catch {
    // localStorage may be unavailable (private mode); fall through.
  }
  const candidates = [navigator.language, ...(navigator.languages ?? [])];
  for (const candidate of candidates) {
    if (!candidate) continue;
    const prefix = candidate.toLowerCase().split(/[-_]/)[0];
    if (isLocale(prefix)) return prefix;
  }
  return DEFAULT_LOCALE;
}

// Flat dictionaries keyed by dotted strings. `en` is the source of truth for
// the key set; `ja` falls back to `en` for any missing key at lookup time.
type Dictionary = Record<string, string>;

const en: Dictionary = {
  // Locale switcher
  "locale.label": "Language",

  // Status bar — chips
  "status.location": "Location",
  "status.time": "Time",
  "status.az": "Az",
  "status.alt": "Alt",
  "status.fov": "FOV",
  "status.fovFullSky": "full sky",
  "status.fovEyepiece": "{value}° eyepiece",
  "status.projection": "Projection",
  "status.viewpoint": "Viewpoint",
  "status.sky": "Sky",
  "status.settings": "Settings",
  "status.dragDateTitle": "Drag left/right to change the date by one day per step",
  "status.dragClockTitle": "Drag left/right to change the time by 10 minutes per step",

  // Twilight summary line in the status bar
  "twilight.initializing": "Sky model initializing",
  "twilight.daylight": "Daylight (Sun {alt}°)",
  "twilight.civil": "Civil twilight (Sun {alt}°)",
  "twilight.nautical": "Nautical twilight (Sun {alt}°)",
  "twilight.astronomical": "Astronomical twilight (Sun {alt}°)",
  "twilight.night": "Night (Sun {alt}°)",

  // Popover chrome
  "popover.close": "Close {title} quick controls",
  "popover.dialogLabel": "{title} quick controls",

  // Location popover
  "location.title": "Location",
  "location.latitude": "Latitude",
  "location.longitude": "Longitude",
  "location.addressLabel": "Address / place lookup",
  "location.addressPlaceholder": "Tokyo Tower, Paris, Mauna Kea…",
  "location.find": "Find",
  "location.finding": "Finding…",
  "location.searching": "Searching address…",
  "location.enterAddress": "Enter an address or place name.",
  "location.noMatch": "No matching place found.",
  "location.lookupFailed": "Address lookup failed. Check your connection and try again.",
  "location.updated": "Location updated from address.",
  "location.useMyLocation": "Use my location",

  // Time popover
  "time.title": "Time",
  "time.localDateTime": "Local date/time",
  "time.datePicker": "Date picker",
  "time.now": "Now",
  "time.minus1h": "−1h",
  "time.plus1h": "+1h",
  "time.helper":
    "The clock keeps ticking after each quick change. Drag the date in the status bar left/right by one day, or the time by 10 minutes.",

  // Settings popover
  "settings.title": "Settings",

  // Settings — View & objects card
  "card.view.title": "View & objects",
  "card.view.description":
    "Choose the map projection and the solar-system bodies drawn with the stars.",
  "card.view.mercuryToNeptune": "Mercury → Neptune",
  "card.view.viewpoint": "Viewpoint",
  "card.view.screenProjection": "Screen projection",
  "card.view.helper":
    "Earth full-sky maps ignore FOV but still rotate with azimuth/altitude. External viewpoints use a perspective parsec-scale camera in IAU galactic Cartesian coordinates (Sun at 0,0,0; +X l=0°; +Y l=90°; +Z north galactic pole) and hide Earth-local overlays.",
  "card.view.originPc": "Origin pc (X, Y, Z)",
  "card.view.targetPc": "Target pc (X, Y, Z)",
  "card.view.up": "Up vector (X, Y, Z)",

  // Settings — Telescope card
  "card.telescope.title": "Telescope eyepiece",
  "card.telescope.description":
    "Derive plate scale and true field of view from an OTA + eyepiece pair. Active only for Earth perspective views.",
  "card.telescope.enable": "Enable eyepiece simulation",
  "card.telescope.aperture": "OTA aperture (mm)",
  "card.telescope.focal": "OTA focal length (mm)",
  "card.telescope.eyepieceFocal": "Eyepiece focal length (mm)",
  "card.telescope.afov": "Apparent field (°)",
  "card.telescope.fieldStop": "Field stop (mm, 0 = AFOV estimate)",
  "card.telescope.summary":
    "{mag}× · {trueField}° true field · {plateScale}″/mm plate scale · {exitPupil} mm exit pupil",

  // Settings — Overlays card
  "card.overlays.title": "Overlays",
  "card.overlays.description":
    "Reference lines and labels are grouped by purpose so it is easier to find what to turn on.",

  // Settings — Atmosphere card
  "card.atmosphere.title": "Atmosphere & extinction",
  "card.atmosphere.description":
    "Model sky colour, haze, refraction, and local air conditions.",
  "card.atmosphere.enable": "Atmosphere / extinction",
  "card.atmosphere.preset": "Preset",
  "card.atmosphere.turbidity": "Turbidity",
  "card.atmosphere.altitude": "Observer altitude (m)",
  "card.atmosphere.ozone": "Ozone column (DU)",
  "card.atmosphere.visibility": "Visibility (km)",
  "card.atmosphere.pressure": "Pressure (hPa)",
  "card.atmosphere.temperature": "Temperature (°C)",

  // Settings — Session card
  "card.session.title": "Session",
  "card.session.description":
    "Share this exact location, time, projection, and display setup. JSON sessions are schema-versioned and preserve time scales plus catalog/correction metadata.",
  "card.session.copyUrl": "Copy session URL",
  "card.session.copyJson": "Copy JSON",
  "card.session.loadJson": "Load JSON",
  "card.session.helper": "Drag the sky to look around · scroll to zoom",
  "card.session.invalidJson": "Invalid session JSON.",

  // Settings — Planning card
  "card.planning.title": "Planning",
  "card.planning.description":
    "Tonight's rise, transit, set, and twilight windows for major objects.",
  "card.planning.rise": "Rise",
  "card.planning.transit": "Transit",
  "card.planning.set": "Set",

  // Overlay groups (web UI only)
  "overlayGroup.referenceGeometry.title": "Reference geometry",
  "overlayGroup.referenceGeometry.description":
    "Horizon, coordinate grids, and great circles for orientation.",
  "overlayGroup.constellations.title": "Constellations",
  "overlayGroup.constellations.description":
    "Western stick figures and IAU boundary outlines.",
  "overlayGroup.labels.title": "Labels",
  "overlayGroup.labels.description": "Names and degree markers drawn over the sky.",
  "overlayGroup.lineStyling.title": "Line styling",
  "overlayGroup.gridStep": "Grid step",
  "overlayGroup.lineOpacity": "Line opacity",

  // Overlay layer labels (kept in sync with `OVERLAY_LAYERS` in observer.ts).
  "overlay.horizon": "Horizon",
  "overlay.cardinals": "Cardinal marks (N/E/S/W)",
  "overlay.alt-az-grid": "Alt-Az grid (observer)",
  "overlay.equatorial-grid": "Equatorial grid (J2000)",
  "overlay.ecliptic": "Ecliptic",
  "overlay.celestial-equator": "Celestial equator",
  "overlay.meridian": "Local meridian",
  "overlay.galactic-equator": "Galactic equator",
  "overlay.constellation-lines": "Constellation lines",
  "overlay.constellation-boundaries": "Constellation boundaries (IAU)",
  "overlay.star-labels": "Bright star labels",
  "overlay.planet-labels": "Sun/Moon/planet labels",
  "overlay.constellation-labels": "Constellation names",
  "overlay.cardinal-labels": "Cardinal labels (N/E/S/W)",
  "overlay.degree-labels": "Degree labels",

  // Screen projections.
  "projection.perspective": "Perspective",
  "projection.mollweide": "Mollweide (full sky)",
  "projection.aitoff": "Aitoff (full sky)",
  "projection.hammer": "Hammer (full sky)",

  // Camera viewpoints.
  "viewpoint.earth": "Earth-centred sky",
  "viewpoint.galactic-north": "Milky Way from above",
  "viewpoint.custom-external": "Custom external camera",

  // Atmosphere presets.
  "atmospherePreset.clear-rural": "Clear rural",
  "atmospherePreset.hazy-urban": "Hazy urban",
  "atmospherePreset.high-altitude": "High altitude",

  // Planning body names emitted by the wasm bridge (matches PlanningBody::name).
  "body.Sun": "Sun",
  "body.Moon": "Moon",
  "body.Mercury": "Mercury",
  "body.Venus": "Venus",
  "body.Earth": "Earth",
  "body.Mars": "Mars",
  "body.Jupiter": "Jupiter",
  "body.Saturn": "Saturn",
  "body.Uranus": "Uranus",
  "body.Neptune": "Neptune",

  // Planning twilight band names emitted by the wasm bridge
  // (matches TwilightBand::label).
  "planningBand.Daylight": "Daylight",
  "planningBand.Civil twilight": "Civil twilight",
  "planningBand.Nautical twilight": "Nautical twilight",
  "planningBand.Astronomical twilight": "Astronomical twilight",
  "planningBand.Night": "Night",
};

const ja: Dictionary = {
  "locale.label": "言語",

  "status.location": "現在地",
  "status.time": "時刻",
  "status.az": "方位",
  "status.alt": "高度",
  "status.fov": "視野",
  "status.fovFullSky": "全天",
  "status.fovEyepiece": "{value}° 接眼レンズ",
  "status.projection": "投影",
  "status.viewpoint": "視点",
  "status.sky": "空",
  "status.settings": "設定",
  "status.dragDateTitle": "左右にドラッグで日付を 1 日ずつ変更",
  "status.dragClockTitle": "左右にドラッグで時刻を 10 分ずつ変更",

  "twilight.initializing": "空モデルを初期化中",
  "twilight.daylight": "昼 (太陽 {alt}°)",
  "twilight.civil": "市民薄明 (太陽 {alt}°)",
  "twilight.nautical": "航海薄明 (太陽 {alt}°)",
  "twilight.astronomical": "天文薄明 (太陽 {alt}°)",
  "twilight.night": "夜 (太陽 {alt}°)",

  "popover.close": "{title}を閉じる",
  "popover.dialogLabel": "{title}",

  "location.title": "現在地",
  "location.latitude": "緯度",
  "location.longitude": "経度",
  "location.addressLabel": "住所・地名で検索",
  "location.addressPlaceholder": "東京タワー、パリ、マウナケア…",
  "location.find": "検索",
  "location.finding": "検索中…",
  "location.searching": "住所を検索中…",
  "location.enterAddress": "住所または地名を入力してください。",
  "location.noMatch": "該当する場所が見つかりませんでした。",
  "location.lookupFailed": "住所検索に失敗しました。接続を確認してから再試行してください。",
  "location.updated": "住所から現在地を更新しました。",
  "location.useMyLocation": "現在地を取得",

  "time.title": "時刻",
  "time.localDateTime": "ローカル日時",
  "time.datePicker": "日付",
  "time.now": "現在",
  "time.minus1h": "−1時間",
  "time.plus1h": "+1時間",
  "time.helper":
    "クイック操作後も時計は進み続けます。ステータスバーの日付を左右にドラッグで 1 日ずつ、時刻を左右にドラッグで 10 分ずつ変更できます。",

  "settings.title": "設定",

  "card.view.title": "視点と天体",
  "card.view.description": "地図投影と、星と一緒に描画する太陽系天体を選択します。",
  "card.view.mercuryToNeptune": "水星 → 海王星",
  "card.view.viewpoint": "視点",
  "card.view.screenProjection": "画面投影",
  "card.view.helper":
    "地球からの全天マップは FOV を無視しますが、方位 / 高度では回転します。外部視点は IAU 銀河直交座標 (太陽を原点とし、+X は l=0°、+Y は l=90°、+Z は北銀極) のパーセクスケール透視カメラを使用し、地球ローカルなオーバーレイは非表示になります。",
  "card.view.originPc": "原点 pc (X, Y, Z)",
  "card.view.targetPc": "注視点 pc (X, Y, Z)",
  "card.view.up": "上方向ベクトル (X, Y, Z)",

  "card.telescope.title": "望遠鏡接眼レンズ",
  "card.telescope.description":
    "OTA と接眼レンズの組み合わせからプレートスケールと実視野を算出します。地球からの透視ビューでのみ有効です。",
  "card.telescope.enable": "接眼レンズシミュレーションを有効化",
  "card.telescope.aperture": "OTA 口径 (mm)",
  "card.telescope.focal": "OTA 焦点距離 (mm)",
  "card.telescope.eyepieceFocal": "接眼レンズ焦点距離 (mm)",
  "card.telescope.afov": "見かけ視野 (°)",
  "card.telescope.fieldStop": "視野絞り (mm, 0 = AFOV からの推定)",
  "card.telescope.summary":
    "{mag}× · 実視野 {trueField}° · プレートスケール {plateScale}″/mm · 射出瞳 {exitPupil} mm",

  "card.overlays.title": "オーバーレイ",
  "card.overlays.description":
    "参照線とラベルは用途別にグループ化されており、有効化したい項目を見つけやすくなっています。",

  "card.atmosphere.title": "大気と減光",
  "card.atmosphere.description": "空の色、もや、大気差、現地の大気条件をモデル化します。",
  "card.atmosphere.enable": "大気 / 減光",
  "card.atmosphere.preset": "プリセット",
  "card.atmosphere.turbidity": "混濁度",
  "card.atmosphere.altitude": "観測地点標高 (m)",
  "card.atmosphere.ozone": "オゾン量 (DU)",
  "card.atmosphere.visibility": "視程 (km)",
  "card.atmosphere.pressure": "気圧 (hPa)",
  "card.atmosphere.temperature": "気温 (°C)",

  "card.session.title": "セッション",
  "card.session.description":
    "現在の位置、時刻、投影、表示設定をそのまま共有します。JSON セッションは schema-versioned で、時刻系・カタログ・補正のメタデータも保持します。",
  "card.session.copyUrl": "セッション URL をコピー",
  "card.session.copyJson": "JSON をコピー",
  "card.session.loadJson": "JSON を読み込み",
  "card.session.helper": "空をドラッグで視点移動 · スクロールでズーム",
  "card.session.invalidJson": "セッション JSON が不正です。",

  "card.planning.title": "観測計画",
  "card.planning.description": "今夜の主要天体の出 / 南中 / 入りと薄明時間帯を表示します。",
  "card.planning.rise": "出",
  "card.planning.transit": "南中",
  "card.planning.set": "入り",

  "overlayGroup.referenceGeometry.title": "参照幾何",
  "overlayGroup.referenceGeometry.description":
    "方位確認のための地平線、座標グリッド、大円を表示します。",
  "overlayGroup.constellations.title": "星座",
  "overlayGroup.constellations.description": "西洋星座の線と IAU 境界を表示します。",
  "overlayGroup.labels.title": "ラベル",
  "overlayGroup.labels.description": "空に重ねて表示する名称・角度のラベルです。",
  "overlayGroup.lineStyling.title": "ラインスタイル",
  "overlayGroup.gridStep": "グリッド間隔",
  "overlayGroup.lineOpacity": "ライン不透明度",

  "overlay.horizon": "地平線",
  "overlay.cardinals": "方位マーク (N/E/S/W)",
  "overlay.alt-az-grid": "水平座標グリッド (観測者)",
  "overlay.equatorial-grid": "赤道座標グリッド (J2000)",
  "overlay.ecliptic": "黄道",
  "overlay.celestial-equator": "天の赤道",
  "overlay.meridian": "子午線",
  "overlay.galactic-equator": "銀河赤道",
  "overlay.constellation-lines": "星座線",
  "overlay.constellation-boundaries": "星座境界 (IAU)",
  "overlay.star-labels": "明るい星のラベル",
  "overlay.planet-labels": "太陽 / 月 / 惑星のラベル",
  "overlay.constellation-labels": "星座名",
  "overlay.cardinal-labels": "方位ラベル (N/E/S/W)",
  "overlay.degree-labels": "角度ラベル",

  "projection.perspective": "透視",
  "projection.mollweide": "モルワイデ (全天)",
  "projection.aitoff": "エイトフ (全天)",
  "projection.hammer": "ハンマー (全天)",

  "viewpoint.earth": "地球中心の天球",
  "viewpoint.galactic-north": "銀河北極からの俯瞰",
  "viewpoint.custom-external": "カスタム外部カメラ",

  "atmospherePreset.clear-rural": "郊外・晴天",
  "atmospherePreset.hazy-urban": "都市・もや",
  "atmospherePreset.high-altitude": "高地",

  "body.Sun": "太陽",
  "body.Moon": "月",
  "body.Mercury": "水星",
  "body.Venus": "金星",
  "body.Earth": "地球",
  "body.Mars": "火星",
  "body.Jupiter": "木星",
  "body.Saturn": "土星",
  "body.Uranus": "天王星",
  "body.Neptune": "海王星",

  "planningBand.Daylight": "昼",
  "planningBand.Civil twilight": "市民薄明",
  "planningBand.Nautical twilight": "航海薄明",
  "planningBand.Astronomical twilight": "天文薄明",
  "planningBand.Night": "夜",
};

const DICTIONARIES: Record<Locale, Dictionary> = { en, ja };

export type Translator = (key: string, params?: Record<string, string | number>) => string;

function formatString(template: string, params?: Record<string, string | number>): string {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in params ? String(params[name]) : match,
  );
}

function makeTranslator(locale: Locale): Translator {
  return (key, params) => {
    const value = DICTIONARIES[locale][key] ?? DICTIONARIES[DEFAULT_LOCALE][key] ?? key;
    return formatString(value, params);
  };
}

type I18nContextValue = {
  locale: Locale;
  setLocale: (next: Locale) => void;
  t: Translator;
};

const I18nContext = createContext<I18nContextValue | null>(null);

/// Re-evaluated once at module load. Components only see this through the
/// context so SSR / tests can override by mounting their own provider.
const INITIAL_LOCALE = detectInitialLocale();

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(INITIAL_LOCALE);

  // Mirror onto <html lang="…"> so assistive tech and CSS lang selectors see
  // the right language. The static `lang="en"` in index.html is the boot-time
  // fallback for the brief moment before React mounts.
  useEffect(() => {
    if (typeof document !== "undefined") {
      document.documentElement.lang = locale;
    }
  }, [locale]);

  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next);
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // Persistence is best-effort; runtime switch still works.
    }
  }, []);

  const value = useMemo<I18nContextValue>(
    () => ({ locale, setLocale, t: makeTranslator(locale) }),
    [locale, setLocale],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

function useI18n(): I18nContextValue {
  const value = useContext(I18nContext);
  if (!value) {
    throw new Error("useI18n must be used inside <I18nProvider>");
  }
  return value;
}

/// Returns the active translator. Components that only need text should use
/// this; components that need to render a language switcher use `useLocale` /
/// `useSetLocale` alongside.
export function useT(): Translator {
  return useI18n().t;
}

export function useLocale(): Locale {
  return useI18n().locale;
}

export function useSetLocale(): (next: Locale) => void {
  return useI18n().setLocale;
}

/// Translate a body name emitted by the wasm bridge (e.g. "Mercury"). Falls
/// back to the canonical English string if the renderer ever adds a body the
/// JS side has not been taught about yet.
export function translateWasmBody(t: Translator, name: string): string {
  const key = `body.${name}`;
  const translated = t(key);
  return translated === key ? name : translated;
}

/// Translate a twilight band label emitted by the wasm planning table.
export function translateWasmTwilight(t: Translator, label: string): string {
  const key = `planningBand.${label}`;
  const translated = t(key);
  return translated === key ? label : translated;
}
