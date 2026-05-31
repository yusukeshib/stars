use std::cell::RefCell;
use std::rc::Rc;

use astronomy::{
    apparent_moon_topocentric, apparent_planet_topocentric, apparent_sun_topocentric,
    equatorial_to_horizontal, evening_plan, icalendar_for_targets, jd_utc_to_unix_ms, lmst_radians,
    planning_targets_from_bodies, rank_targets, rise_transit_set, Observer, Planet, PlanningBody,
    ScoredTarget, TimeScales,
};
use catalog::load_embedded;
use catalog::{
    simbad_query_url, vizier_query_url, DeepSkyCatalog, DeepSkyId, DeepSkyObject, MessierCatalog,
    NgcBrightCatalog, StarIdentifiers,
};
use catalog::search::{
    named_star, search as catalog_search, SearchId, SearchKind, SearchMatch, SOLAR_SYSTEM_BODIES,
};
use renderer::{
    build_star_instance, Atmosphere, AtmospherePreset, Camera, ExternalViewpoint,
    EyepieceSimulation, LightPollution, LocalView, MeteorLayer, OpticalDesign, OutputColourSpace,
    OverlayConfig, OverlayKind, Renderer, SatelliteLayer, Scintillation, SkyProjection,
    SkyViewpoint, StarInstance,
    DEFAULT_SCREEN_LIMITING_MAGNITUDE,
};

/// V-55 curated, manifest-pinned artificial-satellite TLE snapshot, embedded
/// at build time (provenance: `data/manifest.toml` id
/// `celestrak-tle-curated-2026-05`). Shared verbatim with the native hosts'
/// `crates/common/data/satellites/curated_tle.txt`.
const CURATED_TLE_TEXT: &str =
    include_str!("../../../crates/common/data/satellites/curated_tle.txt");
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Slightly past strict naked-eye to compensate for typical monitor viewing
/// conditions (the on-screen dynamic range is much smaller than a dark-adapted
/// observer's). See `renderer::magnitude_to_render_params` for the model.
const LIMITING_MAGNITUDE: f32 = DEFAULT_SCREEN_LIMITING_MAGNITUDE;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).ok();
}

fn push_num(out: &mut String, value: f64) {
    if value.is_finite() {
        out.push_str(&format!("{value:.3}"));
    } else {
        out.push_str("null");
    }
}

fn push_opt_jd_ms(out: &mut String, jd_utc: Option<f64>) {
    match jd_utc {
        Some(jd) => push_num(out, jd_utc_to_unix_ms(jd)),
        None => out.push_str("null"),
    }
}

fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn push_search_match(out: &mut String, hit: &SearchMatch) {
    out.push('{');
    out.push_str("\"id\":");
    push_json_string(out, &hit.id.encode());
    out.push_str(",\"kind\":");
    push_json_string(out, hit.kind.label());
    out.push_str(",\"display\":");
    push_json_string(out, &hit.display);
    out.push_str(",\"aka\":");
    push_json_string(out, &hit.aka);
    out.push_str(",\"score\":");
    out.push_str(&hit.score.to_string());
    out.push_str(",\"raRad\":");
    push_num(out, hit.right_ascension_rad);
    out.push_str(",\"decRad\":");
    push_num(out, hit.declination_rad);
    out.push_str(",\"magnitude\":");
    match hit.magnitude {
        Some(m) => push_num(out, m as f64),
        None => out.push_str("null"),
    }
    out.push('}');
}

/// Resolved apparent position used by `goto_object`. Keeps the JSON
/// emission code in one place.
struct GotoRecord {
    id: String,
    kind: SearchKind,
    display: String,
    aka: String,
    right_ascension_rad: f64,
    declination_rad: f64,
    magnitude: Option<f64>,
    distance: Option<(f64, &'static str)>,
    planning: Option<PlanningBody>,
    /// L-19 SIMBAD lookup URL. `None` for solar-system bodies.
    simbad_url: Option<String>,
    /// L-19 VizieR cone-search URL. `None` for solar-system bodies.
    vizier_url: Option<String>,
}

fn resolve(id: SearchId, observer: Observer) -> Option<GotoRecord> {
    match id {
        SearchId::NamedStar(idx) => {
            let star = named_star(SearchId::NamedStar(idx))?;
            let ids = StarIdentifiers {
                hip: star.hip,
                hd: star.hd,
                hr: star.hr,
                proper_name: star.proper.clone(),
                catalog_designation: None,
                right_ascension_rad: star.right_ascension_rad,
                declination_rad: star.declination_rad,
            };
            Some(GotoRecord {
                id: SearchId::NamedStar(idx).encode(),
                kind: SearchKind::Star,
                display: star.display(),
                aka: star
                    .bayer
                    .as_deref()
                    .zip(star.constellation.as_deref())
                    .map(|(b, c)| format!("{b} {c}"))
                    .unwrap_or_default(),
                right_ascension_rad: star.right_ascension_rad,
                declination_rad: star.declination_rad,
                magnitude: Some(star.magnitude as f64),
                distance: if star.distance_pc > 0.0 {
                    Some((star.distance_pc as f64, "pc"))
                } else {
                    None
                },
                planning: None,
                simbad_url: Some(simbad_query_url(&ids)),
                vizier_url: Some(vizier_query_url(&ids)),
            })
        }
        SearchId::Messier(n) => {
            let object = MessierCatalog
                .objects(99.0)
                .into_iter()
                .find(|o| o.id == DeepSkyId::Messier(n))?;
            Some(deepsky_goto(id, object))
        }
        SearchId::Ngc(n) => {
            let object = NgcBrightCatalog
                .objects(99.0)
                .into_iter()
                .find(|o| o.id == DeepSkyId::Ngc(n))?;
            Some(deepsky_goto(id, object))
        }
        SearchId::Ic(n) => {
            let object = NgcBrightCatalog
                .objects(99.0)
                .into_iter()
                .find(|o| o.id == DeepSkyId::Ic(n))?;
            Some(deepsky_goto(id, object))
        }
        SearchId::SolarSystem(name) => {
            let body = SOLAR_SYSTEM_BODIES
                .iter()
                .find(|b| b.canonical == name)?;
            let (ra, dec, magnitude, distance, planning): (
                f64,
                f64,
                Option<f64>,
                (f64, &'static str),
                PlanningBody,
            ) = match name {
                "sun" => {
                    let sun = apparent_sun_topocentric(observer);
                    (
                        sun.right_ascension_rad,
                        sun.declination_rad,
                        Some(-26.74_f64),
                        (sun.distance_au, "AU"),
                        PlanningBody::Sun,
                    )
                }
                "moon" => {
                    let moon = apparent_moon_topocentric(observer);
                    (
                        moon.right_ascension_rad,
                        moon.declination_rad,
                        None,
                        (moon.distance_km, "km"),
                        PlanningBody::Moon,
                    )
                }
                other => {
                    let planet = match other {
                        "mercury" => Planet::Mercury,
                        "venus" => Planet::Venus,
                        "mars" => Planet::Mars,
                        "jupiter" => Planet::Jupiter,
                        "saturn" => Planet::Saturn,
                        "uranus" => Planet::Uranus,
                        "neptune" => Planet::Neptune,
                        _ => return None,
                    };
                    let p = apparent_planet_topocentric(observer, planet);
                    (
                        p.right_ascension_rad,
                        p.declination_rad,
                        Some(p.magnitude),
                        (p.distance_au, "AU"),
                        PlanningBody::Planet(planet),
                    )
                }
            };
            Some(GotoRecord {
                id: SearchId::SolarSystem(body.canonical).encode(),
                kind: SearchKind::SolarSystem,
                display: body.display_en.to_string(),
                aka: body.display_ja.to_string(),
                right_ascension_rad: ra,
                declination_rad: dec,
                magnitude,
                distance: Some(distance),
                planning: Some(planning),
                // Solar-system bodies are not in the CDS stellar archives.
                simbad_url: None,
                vizier_url: None,
            })
        }
    }
}

fn deepsky_goto(id: SearchId, object: DeepSkyObject) -> GotoRecord {
    let position = object.position;
    let x = position[0] as f64;
    let y = position[1] as f64;
    let z = position[2] as f64;
    let len = (x * x + y * y + z * z).sqrt().max(1e-12);
    let ra = y.atan2(x).rem_euclid(std::f64::consts::TAU);
    let dec = (z / len).clamp(-1.0, 1.0).asin();
    let kind = match id {
        SearchId::Messier(_) => SearchKind::Messier,
        SearchId::Ic(_) => SearchKind::Ic,
        _ => SearchKind::Ngc,
    };
    let label = match object.id {
        DeepSkyId::Messier(n) => format!("M{n}"),
        DeepSkyId::Ngc(n) => format!("NGC {n}"),
        DeepSkyId::Ic(n) => format!("IC {n}"),
    };
    // SIMBAD resolves designations with a space between catalogue and number.
    let designation = match object.id {
        DeepSkyId::Messier(n) => format!("M {n}"),
        DeepSkyId::Ngc(n) => format!("NGC {n}"),
        DeepSkyId::Ic(n) => format!("IC {n}"),
    };
    let ids = StarIdentifiers {
        catalog_designation: Some(designation),
        right_ascension_rad: ra,
        declination_rad: dec,
        ..Default::default()
    };
    GotoRecord {
        id: id.encode(),
        kind,
        display: label.clone(),
        aka: label,
        right_ascension_rad: ra,
        declination_rad: dec,
        simbad_url: Some(simbad_query_url(&ids)),
        vizier_url: Some(vizier_query_url(&ids)),
        magnitude: if object.magnitude < 90.0 {
            Some(object.magnitude as f64)
        } else {
            None
        },
        distance: None,
        planning: None,
    }
}

fn push_goto_record(out: &mut String, record: &GotoRecord, observer: Observer) {
    let lst = lmst_radians(observer.time.jd_ut1, observer.longitude_rad);
    let altaz = equatorial_to_horizontal(
        record.right_ascension_rad,
        record.declination_rad,
        lst,
        observer.latitude_rad,
    );
    out.push('{');
    out.push_str("\"id\":");
    push_json_string(out, &record.id);
    out.push_str(",\"kind\":");
    push_json_string(out, record.kind.label());
    out.push_str(",\"display\":");
    push_json_string(out, &record.display);
    out.push_str(",\"aka\":");
    push_json_string(out, &record.aka);
    out.push_str(",\"raRad\":");
    push_num(out, record.right_ascension_rad);
    out.push_str(",\"decRad\":");
    push_num(out, record.declination_rad);
    out.push_str(",\"azimuthRad\":");
    push_num(out, altaz.azimuth);
    out.push_str(",\"altitudeRad\":");
    push_num(out, altaz.altitude);
    out.push_str(",\"magnitude\":");
    match record.magnitude {
        Some(m) => push_num(out, m),
        None => out.push_str("null"),
    }
    out.push_str(",\"distance\":");
    match &record.distance {
        Some((value, unit)) => {
            out.push_str("{\"value\":");
            push_num(out, *value);
            out.push_str(",\"unit\":");
            push_json_string(out, unit);
            out.push('}');
        }
        None => out.push_str("null"),
    }
    // L-19: CDS deep links (stars / deep-sky only; null for solar-system).
    out.push_str(",\"simbadUrl\":");
    match &record.simbad_url {
        Some(url) => push_json_string(out, url),
        None => out.push_str("null"),
    }
    out.push_str(",\"vizierUrl\":");
    match &record.vizier_url {
        Some(url) => push_json_string(out, url),
        None => out.push_str("null"),
    }
    // Rise / transit / set, if the body is in the planning table.
    out.push_str(",\"riseSetMs\":");
    if let Some(body) = record.planning {
        let (start, end) = (observer.time.jd_utc - 0.25, observer.time.jd_utc + 1.25);
        let rts = rise_transit_set(observer, body, start, end);
        out.push('{');
        out.push_str("\"rise\":");
        push_opt_jd_ms(out, rts.rise_jd_utc);
        out.push_str(",\"transit\":");
        push_opt_jd_ms(out, rts.transit_jd_utc);
        out.push_str(",\"set\":");
        push_opt_jd_ms(out, rts.set_jd_utc);
        out.push('}');
    } else {
        out.push_str("null");
    }
    out.push('}');
}

struct RenderState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    camera: Camera,
}

#[wasm_bindgen]
pub struct StarView {
    state: Rc<RefCell<RenderState>>,
}

#[wasm_bindgen]
impl StarView {
    /// Create a renderer attached to the canvas with the given DOM id.
    pub async fn create(canvas_id: String) -> Result<StarView, JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let canvas = document
            .get_element_by_id(&canvas_id)
            .ok_or("Canvas not found")?
            .dyn_into::<web_sys::HtmlCanvasElement>()?;

        let dpr = window.device_pixel_ratio();
        let width = ((canvas.client_width() as f64) * dpr).max(1.0) as u32;
        let height = ((canvas.client_height() as f64) * dpr).max(1.0) as u32;
        canvas.set_width(width);
        canvas.set_height(height);

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| JsValue::from_str(&format!("Surface error: {e}")))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("Adapter error: {e}")))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Stars Device"),
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("Device error: {e}")))?;

        let surface_caps = surface.get_capabilities(&adapter);
        // Prefer an sRGB surface so the hardware applies the linear→sRGB EOTF
        // on present. The star shader emits linear radiance (Pogson's law, see
        // `vertex::magnitude_to_render_params`); writing that straight into a
        // non-sRGB framebuffer crushes mid/faint magnitudes to near-black on
        // the display. Selecting an sRGB format is the correct, lossless way
        // to get perceptually right brightness without altering the physics.
        let format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        log::info!("Surface format: {format:?} (sRGB: {})", format.is_srgb());
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        log::info!("Loading star catalog...");
        let stars = load_embedded();
        log::info!("Loaded {} stars", stars.len());
        let instances: Vec<StarInstance> = stars
            .iter()
            .map(|s| {
                build_star_instance(
                    s.position.into(),
                    s.proper_motion.into(),
                    s.color,
                    s.magnitude,
                    LIMITING_MAGNITUDE,
                    s.distance_pc,
                )
            })
            .collect();

        let renderer = Renderer::new(&device, format, width, height, &instances);
        let mut camera = Camera::new(
            // Defaults; JS will overwrite immediately.
            Observer::from_degrees(0.0, 0.0, 2_451_545.0),
            LocalView::default(),
            width as f32 / height as f32,
        );
        // Same brightness scale as the star pipeline so the skyglow pass
        // composites correctly on top.
        camera.limiting_magnitude = LIMITING_MAGNITUDE;

        Ok(StarView {
            state: Rc::new(RefCell::new(RenderState {
                surface,
                device,
                queue,
                config,
                renderer,
                camera,
            })),
        })
    }

    /// Update the observer. `time_unix_ms` is a JS `Date.now()`-style millisecond
    /// epoch; conversion to Julian Date happens here so the JS side doesn't need
    /// to know the constant.
    pub fn set_observer(&self, lat_deg: f64, lng_deg: f64, time_unix_ms: f64) {
        let time = TimeScales::from_unix_seconds(time_unix_ms / 1000.0);
        self.state.borrow_mut().camera.observer =
            Observer::from_degrees_with_time(lat_deg, lng_deg, time);
    }

    /// Current apparent topocentric Sun altitude, in degrees.
    ///
    /// The HUD uses this for daylight/twilight labels so the user-visible sky
    /// state is derived from the same Rust ephemeris and `TimeScales` convention as
    /// the renderer's daylight, twilight, and disk inputs. Keeping the formula
    /// here avoids a second, drifting JavaScript solar-position implementation.
    pub fn sun_altitude_deg(&self) -> f64 {
        let observer = self.state.borrow().camera.observer;
        let sun = apparent_sun_topocentric(observer);
        let lst = lmst_radians(observer.time.jd_ut1, observer.longitude_rad);
        equatorial_to_horizontal(
            sun.right_ascension_rad,
            sun.declination_rad,
            lst,
            observer.latitude_rad,
        )
        .altitude
        .to_degrees()
    }

    /// Update the active overlay layers. `layers` is a list of kebab-case names
    /// that match the CLI's `--overlays` flag: "horizon", "cardinals",
    /// "alt-az-grid", "equatorial-grid", "ecliptic", "celestial-equator",
    /// "meridian", "galactic-equator", "constellation-lines",
    /// "constellation-boundaries", "deep-sky-objects", "deep-sky-labels",
    /// "star-labels", "planet-labels", "constellation-labels",
    /// "cardinal-labels", and "degree-labels".
    /// Unknown names are ignored with a
    /// warning so the JS layer can evolve without breaking older builds.
    ///
    /// `grid_step_deg`, `opacity`, and `deep_sky_magnitude_limit` are passed
    /// through to the renderer, which applies its own clamps; finite values
    /// outside the renderer's accepted range are silently coerced. Non-finite
    /// values would propagate into the geometry generators and produce NaN
    /// vertices, so we replace them with the renderer's defaults here.
    pub fn set_overlays(
        &self,
        layers: Vec<String>,
        grid_step_deg: f64,
        opacity: f32,
        deep_sky_magnitude_limit: f32,
    ) {
        let kinds: Vec<OverlayKind> = layers
            .iter()
            .filter_map(|name| {
                let parsed = OverlayKind::from_kebab_str(name);
                if parsed.is_none() {
                    log::warn!("unknown overlay name from JS: {name:?}");
                }
                parsed
            })
            .collect();
        let grid_step_deg = if grid_step_deg.is_finite() {
            grid_step_deg
        } else {
            15.0
        };
        let opacity = if opacity.is_finite() { opacity } else { 0.6 };
        // The renderer clamps and NaN-replaces internally; we just forward.
        let s = &mut *self.state.borrow_mut();
        s.renderer.set_overlays(
            &s.device,
            &OverlayConfig {
                layers: kinds,
                grid_step_deg,
                opacity,
                deep_sky_magnitude_limit,
            },
        );
    }

    pub fn set_view(&self, azimuth_rad: f32, altitude_rad: f32, fov_y_rad: f32) {
        self.state.borrow_mut().camera.view = LocalView {
            azimuth_rad,
            altitude_rad,
            fov_y_rad,
        }
        .clamped();
    }

    pub fn set_planets_enabled(&self, enabled: bool) {
        self.state.borrow_mut().camera.planets_enabled = enabled;
    }

    /// V-55: enable / disable the artificial-satellite layer (TLE / SGP4) from
    /// the curated CelesTrak snapshot. `exposure_seconds > 0` renders motion
    /// streaks; `0` renders point sprites.
    pub fn set_satellites(&self, enabled: bool, exposure_seconds: f32) {
        let tles = if enabled {
            astronomy::parse_tle_set(CURATED_TLE_TEXT)
        } else {
            Vec::new()
        };
        self.state.borrow_mut().camera.satellites = SatelliteLayer {
            enabled,
            exposure_seconds: exposure_seconds.max(0.0),
            tles,
        };
    }

    /// V-47: enable / disable the meteor-shower layer and set its deterministic
    /// stream parameters. `rate_scale` multiplies the modelled observed rate;
    /// `window_seconds` is the long-exposure integration window (and the time
    /// bin for deterministic seeding).
    pub fn set_meteors(
        &self,
        enabled: bool,
        seed: f64,
        rate_scale: f32,
        window_seconds: f32,
    ) {
        self.state.borrow_mut().camera.meteors = MeteorLayer {
            enabled,
            seed: seed.max(0.0) as u64,
            rate_scale: rate_scale.max(0.0),
            window_seconds: window_seconds.max(0.0),
        };
    }

    /// Update the telescope eyepiece simulator. When enabled, the renderer
    /// overrides the perspective FoV with the true field from these optics.
    pub fn set_eyepiece_simulation(
        &self,
        enabled: bool,
        aperture_mm: f32,
        focal_length_mm: f32,
        eyepiece_focal_length_mm: f32,
        apparent_fov_deg: f32,
        field_stop_mm: f32,
    ) {
        let mut state = self.state.borrow_mut();
        // Preserve the V-45 telescope-side optics (set via
        // `set_telescope_optics`); this call only updates the geometric train.
        let prev = state.camera.eyepiece;
        state.camera.eyepiece = EyepieceSimulation {
            enabled,
            aperture_mm,
            focal_length_mm,
            eyepiece_focal_length_mm,
            apparent_fov_deg,
            field_stop_mm,
            optical_design: prev.optical_design,
            ota_rotation_deg: prev.ota_rotation_deg,
        };
    }

    /// V-45: update the telescope optical design driving the eyepiece
    /// diffraction artifacts. `design` is one of `apo-refractor`,
    /// `achromat-refractor`, `newtonian`, or `schmidt-cassegrain`; unknown
    /// names fall back to an apochromatic refractor. `spider_vanes` overrides
    /// the Newtonian vane count, and `ota_rotation_deg` rolls the spikes with
    /// the tube.
    pub fn set_telescope_optics(&self, design: String, spider_vanes: u8, ota_rotation_deg: f32) {
        let mut optical_design = OpticalDesign::from_kebab_str(&design).unwrap_or_default();
        if let OpticalDesign::Newtonian { .. } = optical_design {
            optical_design = OpticalDesign::Newtonian { spider_vanes };
        }
        let mut state = self.state.borrow_mut();
        state.camera.eyepiece.optical_design = optical_design;
        state.camera.eyepiece.ota_rotation_deg = ota_rotation_deg;
    }

    /// Select the active screen projection by kebab-case name: `perspective`,
    /// `mollweide`, `aitoff`, or `hammer`. Unknown names fall back to the
    /// perspective camera so older JavaScript cannot leave the renderer in an
    /// invalid state.
    pub fn set_projection(&self, projection: String) {
        self.state.borrow_mut().camera.projection =
            SkyProjection::from_kebab_str(&projection).unwrap_or_default();
    }

    /// Select the camera viewpoint by kebab-case name: `earth`,
    /// `galactic-north`, or `custom-external`. Unknown names fall back to the
    /// Earth-centred sky dome.
    pub fn set_viewpoint(&self, viewpoint: String) {
        self.state.borrow_mut().camera.viewpoint =
            SkyViewpoint::from_kebab_str(&viewpoint).unwrap_or_default();
    }

    /// Update the custom external camera. Coordinates are IAU galactic
    /// Cartesian parsecs: Sun at the origin, +X toward l=0°, +Y toward l=90°,
    /// and +Z toward the north galactic pole.
    pub fn set_external_viewpoint(
        &self,
        origin_x_pc: f32,
        origin_y_pc: f32,
        origin_z_pc: f32,
        target_x_pc: f32,
        target_y_pc: f32,
        target_z_pc: f32,
        up_x: f32,
        up_y: f32,
        up_z: f32,
    ) {
        self.state.borrow_mut().camera.external_viewpoint = ExternalViewpoint::new(
            [origin_x_pc, origin_y_pc, origin_z_pc],
            [target_x_pc, target_y_pc, target_z_pc],
            [up_x, up_y, up_z],
        );
    }

    /// Return the current local-evening rise/transit/set table and twilight
    /// bands as a JSON string. Keeping this in Rust makes the UI use the same
    /// ephemerides/time-scale split as the renderer.
    pub fn planning_table_json(&self) -> String {
        let observer = self.state.borrow().camera.observer;
        let plan = evening_plan(observer);
        let mut s = String::new();
        s.push_str("{\"startMs\":");
        push_num(&mut s, jd_utc_to_unix_ms(plan.start_jd_utc));
        s.push_str(",\"endMs\":");
        push_num(&mut s, jd_utc_to_unix_ms(plan.end_jd_utc));
        s.push_str(",\"rows\":[");
        for (idx, row) in plan.rows.iter().enumerate() {
            if idx > 0 {
                s.push(',');
            }
            s.push_str("{\"name\":\"");
            s.push_str(row.name);
            s.push_str("\",\"riseMs\":");
            push_opt_jd_ms(&mut s, row.rise_jd_utc);
            s.push_str(",\"transitMs\":");
            push_opt_jd_ms(&mut s, row.transit_jd_utc);
            s.push_str(",\"setMs\":");
            push_opt_jd_ms(&mut s, row.set_jd_utc);
            s.push_str(",\"transitAltitudeDeg\":");
            match row.transit_altitude_rad {
                Some(alt) => push_num(&mut s, alt.to_degrees()),
                None => s.push_str("null"),
            }
            s.push('}');
        }
        s.push_str("],\"twilight\":[");
        for (idx, band) in plan.twilight.iter().enumerate() {
            if idx > 0 {
                s.push(',');
            }
            s.push_str("{\"label\":\"");
            s.push_str(band.band.label());
            s.push_str("\",\"startMs\":");
            push_num(&mut s, jd_utc_to_unix_ms(band.start_jd_utc));
            s.push_str(",\"endMs\":");
            push_num(&mut s, jd_utc_to_unix_ms(band.end_jd_utc));
            s.push('}');
        }
        s.push_str("]}");
        s
    }

    /// L-09: rank tonight's solar-system bodies by visibility score and return
    /// the recommended-object list as JSON, including per-target Moon-impact
    /// (Krisciunas-Schaefer 1991) and the observable dark window. The
    /// Moon-free baseline is the current site's light-pollution zenith
    /// brightness, so the score reflects what the observer will actually see.
    pub fn planning_recommended_json(&self) -> String {
        let (observer, dark_v) = {
            let state = self.state.borrow();
            (
                state.camera.observer,
                state.camera.light_pollution.zenith_sqm_mag_per_arcsec2(),
            )
        };
        let targets = planning_targets_from_bodies(observer);
        let ranked = rank_targets(observer, &targets, dark_v);
        let mut s = String::new();
        s.push_str("{\"darkSkyZenithVMag\":");
        push_num(&mut s, dark_v);
        s.push_str(",\"recommended\":[");
        for (idx, entry) in ranked.iter().enumerate() {
            if idx > 0 {
                s.push(',');
            }
            let v = &entry.visibility;
            s.push_str("{\"name\":\"");
            s.push_str(&entry.target.name);
            s.push_str("\",\"score\":");
            push_num(&mut s, v.score);
            s.push_str(",\"maxAltitudeDeg\":");
            push_num(&mut s, v.max_altitude_rad.to_degrees());
            s.push_str(",\"observableDarkHours\":");
            push_num(&mut s, v.observable_dark_hours);
            s.push_str(",\"windowStartMs\":");
            match v.observable_window_jd_utc {
                Some((start, _)) => push_num(&mut s, jd_utc_to_unix_ms(start)),
                None => s.push_str("null"),
            }
            s.push_str(",\"windowEndMs\":");
            match v.observable_window_jd_utc {
                Some((_, end)) => push_num(&mut s, jd_utc_to_unix_ms(end)),
                None => s.push_str("null"),
            }
            s.push_str(",\"moonDeltaVMag\":");
            push_num(&mut s, v.moon.delta_v_mag);
            s.push_str(",\"moonAltitudeDeg\":");
            push_num(&mut s, v.moon.moon_altitude_rad.to_degrees());
            s.push_str(",\"moonIlluminatedFraction\":");
            push_num(&mut s, v.moon.moon_illuminated_fraction);
            s.push('}');
        }
        s.push_str("]}");
        s
    }

    /// L-09: export the recommended targets' observable dark windows as an
    /// RFC 5545 iCalendar document. The frontend offers this as an `.ics`
    /// download so a plan can be dropped straight into a calendar app.
    pub fn planning_ical(&self) -> String {
        let (observer, dark_v) = {
            let state = self.state.borrow();
            (
                state.camera.observer,
                state.camera.light_pollution.zenith_sqm_mag_per_arcsec2(),
            )
        };
        let targets = planning_targets_from_bodies(observer);
        let ranked: Vec<ScoredTarget> = rank_targets(observer, &targets, dark_v);
        icalendar_for_targets(&ranked)
    }

    /// Update atmosphere controls from the web UI. `enabled=false` matches the
    /// native `--no-extinction` flag and disables both extinction and sunlit
    /// scattering. `aerosol_beta` is the Ångström optical depth at 550 nm.
    pub fn set_atmosphere(&self, enabled: bool, aerosol_beta: f32, observer_altitude_m: f32) {
        self.state.borrow_mut().camera.atmosphere = if enabled {
            Atmosphere {
                aerosol_beta,
                observer_altitude_m,
                ..Atmosphere::default()
            }
        } else {
            Atmosphere::OFF
        };
    }

    /// Select one of the renderer's serializable atmosphere presets by kebab
    /// name: `clear-rural`, `hazy-urban`, or `high-altitude`.
    pub fn set_atmosphere_preset(&self, enabled: bool, preset: String) {
        self.state.borrow_mut().camera.atmosphere = if enabled {
            AtmospherePreset::from_kebab_str(&preset)
                .map(Atmosphere::from_preset)
                .unwrap_or_default()
        } else {
            Atmosphere::OFF
        };
    }

    /// Update the complete atmosphere state from the web UI. The preset only
    /// picks defaults; the canonical (β, α, DU, h) state drives both stellar
    /// k(λ) and the daylight scattering shader (V-37).
    pub fn set_atmosphere_config(
        &self,
        enabled: bool,
        preset: String,
        aerosol_beta: f32,
        aerosol_alpha: f32,
        observer_altitude_m: f32,
        ozone_du: f32,
        pressure_hpa: f32,
        temperature_c: f32,
        surface_albedo: f32,
    ) {
        self.state.borrow_mut().camera.atmosphere = if enabled {
            let mut atmosphere = AtmospherePreset::from_kebab_str(&preset)
                .map(Atmosphere::from_preset)
                .unwrap_or_default();
            atmosphere.aerosol_beta = aerosol_beta;
            atmosphere.aerosol_alpha = aerosol_alpha;
            atmosphere.observer_altitude_m = observer_altitude_m;
            atmosphere.ozone_du = ozone_du;
            atmosphere.pressure_hpa = pressure_hpa;
            atmosphere.temperature_c = temperature_c;
            atmosphere.surface_albedo = surface_albedo;
            atmosphere
        } else {
            Atmosphere::OFF
        };
    }

    /// Update V-39 light-pollution controls. `kind` is a kebab-case tag
    /// matching the `SessionLightPollution.kind` field: `bortle`, `sqm`, or
    /// `atlas-2016`. Only the field for the active variant is read;
    /// passing `enabled = false` forces the Bortle-1 / dark-sky floor
    /// regardless of the other parameters. Atlas sampling is deferred to
    /// `V-39-Atlas`; on the web side it currently maps to Bortle 1.
    pub fn set_light_pollution(
        &self,
        enabled: bool,
        kind: String,
        bortle_class: u8,
        sqm_mag_per_arcsec2: f32,
        atlas_latitude_deg: f32,
        atlas_longitude_deg: f32,
    ) {
        let pollution = if !enabled {
            LightPollution::DARK_SKY
        } else {
            match kind.as_str() {
                "bortle" => LightPollution::Bortle(bortle_class),
                "sqm" => LightPollution::Sqm(sqm_mag_per_arcsec2),
                "atlas-2016" => LightPollution::Atlas2016 {
                    latitude_deg: atlas_latitude_deg,
                    longitude_deg: atlas_longitude_deg,
                },
                other => {
                    log::warn!("set_light_pollution: unknown kind \"{other}\", falling back to Bortle 1");
                    LightPollution::DARK_SKY
                }
            }
        };
        self.state.borrow_mut().camera.light_pollution = pollution;
    }

    /// V-56 object search. Returns a JSON-encoded ranked list of matches for
    /// the free-text `query`. `limit = 0` falls back to
    /// [`catalog::search::SEARCH_LIMIT_DEFAULT`]. The host calls this on
    /// debounced input from the search box.
    ///
    /// Each match carries enough data to render the dropdown row without a
    /// follow-up call; selecting one then calls [`goto_object_json`] with the
    /// `id` field to obtain the apparent (alt, az) the host should slew the
    /// camera to.
    pub fn lookup_object(&self, query: String, limit: u32) -> String {
        let hits = catalog_search(&query, limit as usize);
        let mut s = String::new();
        s.push_str("{\"matches\":[");
        for (i, hit) in hits.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            push_search_match(&mut s, hit);
        }
        s.push_str("]}");
        s
    }

    /// V-56 GoTo. Resolves the encoded `SearchId` to an apparent
    /// topocentric (alt, az) for the current observer + clock, returning a
    /// JSON record the host applies to the camera. Unknown ids yield a
    /// `null` payload so the UI can drop the request quietly instead of
    /// crashing.
    pub fn goto_object(&self, id_json: String) -> String {
        let Some(id) = SearchId::parse(&id_json) else {
            return "null".to_string();
        };
        let observer = self.state.borrow().camera.observer;
        let mut s = String::new();
        match resolve(id.clone(), observer) {
            None => s.push_str("null"),
            Some(record) => push_goto_record(&mut s, &record, observer),
        }
        s
    }

    /// Update V-24 scintillation controls. `enabled=false` matches the
    /// native `--no-scintillation` flag and disables the per-star
    /// time-varying flux modulation entirely; the seed travels in the
    /// session schema so deterministic re-renders share noise.
    pub fn set_scintillation(&self, enabled: bool, c_n2_scale: f32, seed: u32) {
        self.state.borrow_mut().camera.scintillation = if enabled {
            Scintillation {
                enabled: true,
                c_n2_scale,
                seed,
            }
        } else {
            Scintillation::OFF
        };
    }

    /// V-50 output colour management. `space` is one of `"srgb"`,
    /// `"display-p3"`, or `"rec2020"`. Unrecognised values fall back to sRGB.
    /// The renderer applies the gamut transform in the tonemap step; the
    /// canvas swap-chain itself stays sRGB-tagged, so wide-gamut primaries are
    /// reproduced on browsers/screens that honour the sRGB-encoded values,
    /// with sRGB as the documented fallback elsewhere.
    pub fn set_output_colourspace(&self, space: String) {
        let cs = OutputColourSpace::from_str_opt(&space).unwrap_or(OutputColourSpace::Srgb);
        self.state.borrow_mut().camera.output_colourspace = cs;
    }

    pub fn resize(&self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let mut s = self.state.borrow_mut();
        s.config.width = width;
        s.config.height = height;
        s.surface.configure(&s.device, &s.config);
        s.camera.aspect = width as f32 / height as f32;
        // Keep the renderer's HDR target matched to the swapchain.
        // Split-borrow: the borrow checker can't see that `device` and
        // `renderer` don't alias when accessed through one `&mut s`.
        let RenderState {
            device, renderer, ..
        } = &mut *s;
        renderer.resize(device, width, height);
    }

    /// Render a single frame using the current observer/view.
    pub fn render_frame(&self) {
        let s = self.state.borrow();
        s.renderer
            .update_camera(&s.queue, &s.camera, s.config.width, s.config.height);

        let surface_texture = match s.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                log::warn!("surface texture was outdated/lost; reconfiguring surface");
                s.surface.configure(&s.device, &s.config);
                return;
            }
            unexpected => {
                log::warn!("skipping frame after unexpected surface texture state: {unexpected:?}");
                return;
            }
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = s
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        s.renderer.render(&mut encoder, &view);
        s.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
    }
}
