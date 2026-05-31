import {
  createContext,
  useContext,
  useEffect,
  useMemo,
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

const LOCALES = ["en", "ja"] as const;
export type Locale = (typeof LOCALES)[number];
export const DEFAULT_LOCALE: Locale = "en";

const isLocale = (s: unknown): s is Locale =>
  typeof s === "string" && (LOCALES as readonly string[]).includes(s);

/// Resolve the locale once at module load from the browser environment.
///
/// Priority:
///   1. `?lang=` URL parameter (so shared session URLs can pin a language).
///   2. `navigator.language` / `navigator.languages` prefix match.
///   3. `DEFAULT_LOCALE` ("en").
///
/// There is no in-app language switcher: the UI follows the browser. Users
/// who want to override pass `?lang=ja` (or `?lang=en`) in the URL, or change
/// their browser's preferred language.
function detectInitialLocale(): Locale {
  if (typeof window === "undefined") return DEFAULT_LOCALE;
  const params = new URLSearchParams(window.location.search);
  const urlLang = params.get("lang");
  if (isLocale(urlLang)) return urlLang;
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
  "status.dragLatTitle": "Drag left/right to change latitude by 0.1° per step",
  "status.dragLngTitle": "Drag left/right to change longitude by 0.1° per step",
  "status.dragAzTitle": "Drag left/right to change azimuth by 1° per step",
  "status.dragAltTitle": "Drag left/right to change altitude by 0.5° per step",
  "status.dragFovTitle": "Drag right to zoom in, left to zoom out",
  "status.cycleProjection": "Click to cycle screen projection",

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
  "time.now": "Now",
  "time.minus1h": "−1h",
  "time.plus1h": "+1h",
  "time.helper":
    "The clock keeps ticking after each quick change. Drag the date in the status bar left/right by one day, or the time by 10 minutes.",

  // Settings popover
  "settings.title": "Settings",
  "settings.tabsLabel": "Settings sections",
  "settings.tab.sky": "Sky",
  "settings.tab.view": "View",
  "settings.tab.environment": "Environment",
  "settings.tab.session": "Session",

  "card.solarSystem.title": "Solar system",
  "card.solarSystem.description": "Draw the major bodies alongside the stars.",

  // Settings — View & objects card
  "card.view.title": "View & objects",
  "card.view.description":
    "Choose the map projection and the solar-system bodies drawn with the stars.",
  "card.view.mercuryToNeptune": "Mercury → Neptune",
  "card.view.satellites": "Artificial satellites (ISS / Starlink)",
  "card.view.satelliteExposure": "Satellite streak exposure (s)",
  "card.view.meteors": "Meteor showers (V-47)",
  "card.view.meteorSeed": "Meteor seed",
  "card.view.meteorRateScale": "Meteor rate scale",
  "card.aurora.title": "Aurora",
  "card.aurora.description": "Statistically-expected auroral oval for a Kp index (V-48).",
  "card.view.aurora": "Aurora display",
  "card.view.auroraKp": "Geomagnetic Kp (0–9)",
  "card.view.auroraSeason": "Season",
  "card.view.comets": "Comets (Halley / Hale-Bopp / C2023 A3)",
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
  "card.telescope.design": "Optical design",
  "card.telescope.design.apo-refractor": "Apo refractor (clean Airy)",
  "card.telescope.design.achromat-refractor": "Achromat refractor (colour fringe)",
  "card.telescope.design.newtonian": "Newtonian (spikes)",
  "card.telescope.design.schmidt-cassegrain": "Schmidt-Cassegrain",
  "card.telescope.spiderVanes": "Spider vanes",
  "card.telescope.otaRotation": "OTA rotation (°)",
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
  "card.atmosphere.aerosolBeta": "Aerosol β (550 nm AOD)",
  "card.atmosphere.aerosolAlpha": "Aerosol α (Ångström exponent)",
  "card.atmosphere.altitude": "Observer altitude (m)",
  "card.atmosphere.ozone": "Ozone column (DU)",

  "card.atmosphere.pressure": "Pressure (hPa)",
  "card.atmosphere.temperature": "Temperature (°C)",
  "card.atmosphere.surfaceAlbedo": "Surface albedo",

  // Settings — Session card
  "card.session.title": "Session",
  "card.session.description":
    "Share this exact location, time, projection, and display setup. JSON sessions are schema-versioned and preserve time scales plus catalog/correction metadata.",
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
  "card.planning.recommended": "Tonight's recommended",
  "card.planning.exportIcal": "Export .ics",
  "card.planning.favourite": "Favourite",
  "card.planning.score": "Visibility score (0–100)",
  "card.planning.maxAltitude": "Max altitude tonight",
  "card.planning.moonImpact": "Moon sky-brightness impact (ΔV mag/arcsec²)",

  // Overlay groups (web UI only)
  "overlayGroup.referenceGeometry.title": "Reference geometry",
  "overlayGroup.referenceGeometry.description":
    "Horizon, coordinate grids, and great circles for orientation.",
  "overlayGroup.deepSky.title": "Deep-sky (Messier)",
  "overlayGroup.deepSky.description":
    "Diamond markers and labels for the 110 Messier objects, filtered by the magnitude slider below.",
  "overlayGroup.deepSkyDensity.title": "Deep-sky density",
  "overlayGroup.deepSkyDensity.description":
    "Hide Messier objects fainter than this V magnitude. 7.0 keeps the dark-sky naked-eye showpieces (M31, M42, M44, M45, M13); raise to 99 to show everything.",
  "overlayGroup.deepSkyMagnitudeLimit": "Magnitude limit",
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
  "overlay.deep-sky-objects": "Messier deep-sky markers",
  "overlay.deep-sky-labels": "Messier labels (M1, M31, ...)",
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

  // V-50 output colour management.
  "card.colourManagement.title": "Colour management",
  "card.colourManagement.description":
    "Choose the output colour space the renderer encodes and the page is tagged with.",
  "card.colourManagement.outputSpace": "Output colour space",
  "card.colourManagement.helper":
    "sRGB is the default working space. Display-P3 and Rec.2020 remap the gamut for wide-gamut screens; the canvas stays sRGB-tagged so colours fall back to sRGB on displays that do not support wide gamut.",
  "colourspace.srgb": "sRGB (IEC 61966-2-1)",
  "colourspace.display-p3": "Display-P3 (wide gamut)",
  "colourspace.rec2020": "Rec.2020 (ultra-wide gamut)",

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
  "status.dragLatTitle": "左右にドラッグで緯度を 0.1° ずつ変更",
  "status.dragLngTitle": "左右にドラッグで経度を 0.1° ずつ変更",
  "status.dragAzTitle": "左右にドラッグで方位を 1° ずつ変更",
  "status.dragAltTitle": "左右にドラッグで高度を 0.5° ずつ変更",
  "status.dragFovTitle": "右ドラッグでズームイン、左ドラッグでズームアウト",
  "status.cycleProjection": "クリックで画面投影を切り替え",

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
  "time.now": "現在",
  "time.minus1h": "−1時間",
  "time.plus1h": "+1時間",
  "time.helper":
    "クイック操作後も時計は進み続けます。ステータスバーの日付を左右にドラッグで 1 日ずつ、時刻を左右にドラッグで 10 分ずつ変更できます。",

  "settings.title": "設定",
  "settings.tabsLabel": "設定セクション",
  "settings.tab.sky": "天体",
  "settings.tab.view": "視点",
  "settings.tab.environment": "環境",
  "settings.tab.session": "セッション",

  "card.solarSystem.title": "太陽系天体",
  "card.solarSystem.description": "主要な太陽系天体を星と一緒に描画します。",

  "card.view.title": "視点と天体",
  "card.view.description": "地図投影と、星と一緒に描画する太陽系天体を選択します。",
  "card.view.mercuryToNeptune": "水星 → 海王星",
  "card.view.satellites": "人工衛星（ISS / Starlink）",
  "card.view.satelliteExposure": "衛星の軌跡露出（秒）",
  "card.view.meteors": "流星群（V-47）",
  "card.view.meteorSeed": "流星シード",
  "card.view.meteorRateScale": "流星レート倍率",
  "card.aurora.title": "オーロラ",
  "card.aurora.description": "Kp 指数から統計的に期待されるオーロラオーバル（V-48）。",
  "card.view.aurora": "オーロラ表示",
  "card.view.auroraKp": "地磁気 Kp（0〜9）",
  "card.view.auroraSeason": "季節",
  "card.view.comets": "彗星（ハレー / ヘール・ボップ / C2023 A3）",
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
  "card.telescope.design": "光学設計",
  "card.telescope.design.apo-refractor": "アポ屈折 (クリーンなエアリー)",
  "card.telescope.design.achromat-refractor": "アクロマート屈折 (色収差)",
  "card.telescope.design.newtonian": "ニュートン (光条)",
  "card.telescope.design.schmidt-cassegrain": "シュミットカセグレン",
  "card.telescope.spiderVanes": "スパイダー枚数",
  "card.telescope.otaRotation": "鏡筒回転 (°)",
  "card.telescope.summary":
    "{mag}× · 実視野 {trueField}° · プレートスケール {plateScale}″/mm · 射出瞳 {exitPupil} mm",

  "card.overlays.title": "オーバーレイ",
  "card.overlays.description":
    "参照線とラベルは用途別にグループ化されており、有効化したい項目を見つけやすくなっています。",

  "card.atmosphere.title": "大気と減光",
  "card.atmosphere.description": "空の色、もや、大気差、現地の大気条件をモデル化します。",
  "card.atmosphere.enable": "大気 / 減光",
  "card.atmosphere.preset": "プリセット",
  "card.atmosphere.aerosolBeta": "エアロゾル β (550 nm AOD)",
  "card.atmosphere.aerosolAlpha": "エアロゾル α (オングストローム指数)",
  "card.atmosphere.altitude": "観測地点標高 (m)",
  "card.atmosphere.ozone": "オゾン量 (DU)",

  "card.atmosphere.pressure": "気圧 (hPa)",
  "card.atmosphere.temperature": "気温 (°C)",
  "card.atmosphere.surfaceAlbedo": "地表アルベド",

  "card.session.title": "セッション",
  "card.session.description":
    "現在の位置、時刻、投影、表示設定をそのまま共有します。JSON セッションは schema-versioned で、時刻系・カタログ・補正のメタデータも保持します。",
  "card.session.copyJson": "JSON をコピー",
  "card.session.loadJson": "JSON を読み込み",
  "card.session.helper": "空をドラッグで視点移動 · スクロールでズーム",
  "card.session.invalidJson": "セッション JSON が不正です。",

  "card.planning.title": "観測計画",
  "card.planning.description": "今夜の主要天体の出 / 南中 / 入りと薄明時間帯を表示します。",
  "card.planning.rise": "出",
  "card.planning.transit": "南中",
  "card.planning.set": "入り",
  "card.planning.recommended": "今夜のおすすめ",
  "card.planning.exportIcal": ".ics 出力",
  "card.planning.favourite": "お気に入り",
  "card.planning.score": "可視スコア（0–100）",
  "card.planning.maxAltitude": "今夜の最高高度",
  "card.planning.moonImpact": "月光による空の明るさへの影響（ΔV mag/arcsec²）",

  "overlayGroup.referenceGeometry.title": "参照幾何",
  "overlayGroup.referenceGeometry.description":
    "方位確認のための地平線、座標グリッド、大円を表示します。",
  "overlayGroup.deepSky.title": "深宇宙天体 (メシエ)",
  "overlayGroup.deepSky.description":
    "110 個のメシエ天体をダイヤモンド型マーカーとラベルで表示します。下の明るさスライダーで描画対象を絞り込めます。",
  "overlayGroup.deepSkyDensity.title": "深宇宙天体の描画密度",
  "overlayGroup.deepSkyDensity.description":
    "この V 等級より暗いメシエ天体を隠します。7.0 は暗い空で胉眼に見える見頃し (M31, M42, M44, M45, M13) を残します。99 まで上げるとすべて表示されます。",
  "overlayGroup.deepSkyMagnitudeLimit": "等級の上限",
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
  "overlay.deep-sky-objects": "メシエ深宇宙天体マーカー",
  "overlay.deep-sky-labels": "メシエラベル (M1, M31, …)",
  "overlay.star-labels": "明るい星のラベル",
  "overlay.planet-labels": "太陽 / 月 / 惑星のラベル",
  "overlay.constellation-labels": "星座名",
  "overlay.cardinal-labels": "方位ラベル (N/E/S/W)",
  "overlay.degree-labels": "角度ラベル",

  "projection.perspective": "透視",
  "projection.mollweide": "モルワイデ (全天)",
  "projection.aitoff": "エイトフ (全天)",
  "projection.hammer": "ハンマー (全天)",

  // V-50 出力カラーマネジメント。
  "card.colourManagement.title": "カラーマネジメント",
  "card.colourManagement.description":
    "レンダラーがエンコードし、ページにタグ付けする出力色空間を選択します。",
  "card.colourManagement.outputSpace": "出力色空間",
  "card.colourManagement.helper":
    "sRGB が既定の作業色空間です。Display-P3 と Rec.2020 は広色域画面向けにガモットを変換します。キャンバスは sRGB タグのままなので、広色域非対応のディスプレイでは sRGB にフォールバックします。",
  "colourspace.srgb": "sRGB (IEC 61966-2-1)",
  "colourspace.display-p3": "Display-P3 (広色域)",
  "colourspace.rec2020": "Rec.2020 (超広色域)",

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
  t: Translator;
};

/// Re-evaluated once at module load. Components only see this through the
/// context so SSR / tests can override by mounting their own provider.
const INITIAL_LOCALE = detectInitialLocale();

const I18nContext = createContext<I18nContextValue>({
  locale: INITIAL_LOCALE,
  t: makeTranslator(INITIAL_LOCALE),
});

export function I18nProvider({ children }: { children: ReactNode }) {
  // Mirror onto <html lang="…"> so assistive tech and CSS lang selectors see
  // the right language. The static `lang="en"` in index.html is the boot-time
  // fallback for the brief moment before React mounts.
  useEffect(() => {
    if (typeof document !== "undefined") {
      document.documentElement.lang = INITIAL_LOCALE;
    }
  }, []);

  const value = useMemo<I18nContextValue>(
    () => ({ locale: INITIAL_LOCALE, t: makeTranslator(INITIAL_LOCALE) }),
    [],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

/// Returns the active translator. The locale is fixed for the lifetime of
/// the page — there is no in-app switcher; refresh after changing
/// `?lang=` or the browser preference.
export function useT(): Translator {
  return useContext(I18nContext).t;
}

export function useLocale(): Locale {
  return useContext(I18nContext).locale;
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
