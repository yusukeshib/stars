use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use astronomy::Observer;
use clap::Parser;
use renderer::{
    Atmosphere, AuroraLayer, Camera, CometLayer, LightPollution, LocalView, MeteorLayer,
    OutputColourSpace, OverlayConfig, Renderer, SatelliteLayer, StarInstance,
    DEFAULT_SCREEN_LIMITING_MAGNITUDE,
};
use stars_host_common::{
    atmosphere_from_args, aurora_from_args, curated_comet_layer, curated_satellite_layer,
    eyepiece_from_args, light_pollution_from_args, load_session, load_star_instances_from_file,
    overlay_config_from_args, parse_time_to_time_scales, resolve_goto_query,
    resolve_light_pollution, scene_from_preset, scene_preset_infos, scintillation_from_args,
    viewpoint_from_args, AtmosphereOverrides, AtmospherePresetArg, AuroraSeasonArg,
    CatalogSnapshot, CorrectionSnapshot, ExternalViewpointOverrides, EyepieceOverrides,
    LightPollutionOverrides, OpticalDesign, OutputColourspaceArg, OverlayArg, ProjectionArg,
    ScenePresetArg, ScintillationOverrides, SessionScene, ViewpointArg,
};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const INITIAL_WINDOW_WIDTH: u32 = 1280;
const INITIAL_WINDOW_HEIGHT: u32 = 720;
/// Convert high-resolution trackpad pixel deltas into approximately the same
/// units as line-wheel deltas before applying the zoom exponential.
const PIXEL_SCROLL_TO_LINE_DELTA: f32 = 0.01;
/// Exponential zoom sensitivity per scroll unit. A small value keeps wheel and
/// trackpad zoom smooth rather than jumping between FoV clamp limits.
const ZOOM_SCROLL_SENSITIVITY: f32 = 0.05;
const CLOCK_SPEED_REALTIME: f64 = 1.0;
const CLOCK_SPEED_MINUTE_PER_SECOND: f64 = 60.0;
const CLOCK_SPEED_HOUR_PER_SECOND: f64 = 3_600.0;
const CLOCK_SPEED_DAY_PER_SECOND: f64 = 86_400.0;

/// V-45: cycle through the telescope optical designs for the `O` keybind
/// (apo refractor → achromat refractor → Newtonian → SCT → …), preserving
/// representative parameters for each.
fn next_optical_design(current: OpticalDesign) -> OpticalDesign {
    match current {
        OpticalDesign::Refractor {
            achromat: false, ..
        } => OpticalDesign::Refractor {
            achromat: true,
            focal_ratio: 10.0,
        },
        OpticalDesign::Refractor { achromat: true, .. } => {
            OpticalDesign::Newtonian { spider_vanes: 4 }
        }
        OpticalDesign::Newtonian { .. } => OpticalDesign::SchmidtCassegrain {
            obstruction_pct: 34.0,
        },
        OpticalDesign::SchmidtCassegrain { .. } => OpticalDesign::Refractor {
            achromat: false,
            focal_ratio: 7.0,
        },
    }
}

/// Interactive desktop viewer for the night sky.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Observer latitude in decimal degrees (north positive).
    #[arg(long, allow_hyphen_values = true, default_value_t = 35.68)]
    lat: f64,

    /// Observer longitude in decimal degrees (east positive).
    #[arg(long, allow_hyphen_values = true, default_value_t = 139.69)]
    lng: f64,

    /// Load the initial scene from a schema-versioned JSON session.
    #[arg(long)]
    session: Option<PathBuf>,

    /// Use a built-in deterministic validation/demo scene. Ignored when --session is supplied.
    #[arg(long, value_enum)]
    preset: Option<ScenePresetArg>,

    /// List built-in deterministic validation/demo scene presets and exit.
    #[arg(long)]
    list_presets: bool,

    /// Initial time as RFC3339. Defaults to "now". The clock advances in real time.
    #[arg(long)]
    time: Option<String>,

    /// Initial azimuth in degrees from North toward East.
    #[arg(long, default_value_t = 180.0, allow_hyphen_values = true)]
    azimuth: f64,

    /// Initial altitude in degrees above the horizon.
    #[arg(long, default_value_t = 30.0, allow_hyphen_values = true)]
    altitude: f64,

    /// Initial vertical field of view, degrees. Ignored by all-sky projections.
    #[arg(long, default_value_t = 70.0)]
    fov: f64,

    /// Screen projection: perspective, mollweide, aitoff, or hammer.
    #[arg(long, default_value_t = ProjectionArg::Perspective)]
    projection: ProjectionArg,

    /// Camera location: earth, galactic-north, or custom-external. Custom
    /// external coordinates use IAU galactic Cartesian parsecs: Sun at origin,
    /// +X to l=0°, +Y to l=90°, +Z to the north galactic pole.
    #[arg(long, default_value_t = ViewpointArg::Earth)]
    viewpoint: ViewpointArg,

    /// Custom external camera origin in parsecs: X Y Z in the IAU galactic
    /// Cartesian frame. Supplying this selects --viewpoint custom-external.
    #[arg(long, num_args = 3, value_names = ["X", "Y", "Z"], allow_hyphen_values = true)]
    external_origin_pc: Option<Vec<f32>>,

    /// Custom external camera target in parsecs: X Y Z in the IAU galactic
    /// Cartesian frame.
    #[arg(long, num_args = 3, value_names = ["X", "Y", "Z"], allow_hyphen_values = true)]
    external_target_pc: Option<Vec<f32>>,

    /// Custom external camera up vector in the IAU galactic Cartesian frame.
    #[arg(long, num_args = 3, value_names = ["X", "Y", "Z"], allow_hyphen_values = true)]
    external_up: Option<Vec<f32>>,

    /// Path to the HYG-format star catalog CSV.
    #[arg(long, default_value = "crates/catalog/data/hyg_v42.csv")]
    catalog: PathBuf,

    /// Overlay layers to draw. Comma-separated; pass --no-overlays to disable all.
    #[arg(
        long,
        value_delimiter = ',',
        default_values_t = vec![OverlayArg::Horizon, OverlayArg::Cardinals, OverlayArg::CardinalLabels],
    )]
    overlays: Vec<OverlayArg>,

    /// Disable all overlays (overrides --overlays).
    #[arg(long)]
    no_overlays: bool,

    /// Spacing between alt-az / RA-Dec grid lines, in degrees.
    #[arg(long, default_value_t = 15.0)]
    grid_step_deg: f64,

    /// Opacity of overlay lines (0..=1). Text labels remain fully opaque.
    #[arg(long, default_value_t = 0.6)]
    overlay_opacity: f32,

    /// Maximum V magnitude shown by the Messier deep-sky overlay. Lower
    /// values show only the brightest objects (default 7.0 ≈ dark-sky
    /// naked-eye limit); raise to 99.0 to show everything. Has no effect
    /// unless `deep-sky-objects` or `deep-sky-labels` is enabled via
    /// `--overlays`.
    #[arg(long, default_value_t = renderer::DEFAULT_DEEP_SKY_MAGNITUDE_LIMIT)]
    deep_sky_magnitude_limit: f32,

    /// Disable atmospheric extinction and sunlit sky scattering.
    #[arg(long)]
    no_extinction: bool,

    /// Atmosphere preset used as the base for extinction and sky colour.
    #[arg(long, default_value_t = AtmospherePresetArg::ClearRural)]
    atmosphere_preset: AtmospherePresetArg,

    /// Ångström aerosol optical depth at 550 nm (β). Drives both stellar
    /// k(λ) and daylight Mie scattering through the unified V-37 state.
    #[arg(long)]
    aerosol_beta: Option<f32>,

    /// Ångström wavelength exponent (α). Continental aerosols ≈ 1.3.
    #[arg(long)]
    aerosol_alpha: Option<f32>,

    /// Override observer altitude above sea level in metres.
    #[arg(long)]
    observer_altitude_m: Option<f32>,

    /// Override total ozone column in Dobson units for sunset/twilight colour.
    #[arg(long)]
    ozone_du: Option<f32>,

    /// Override surface pressure in hPa for atmospheric refraction.
    #[arg(long)]
    pressure_hpa: Option<f32>,

    /// Override air temperature in °C for atmospheric refraction.
    #[arg(long, allow_hyphen_values = true)]
    temperature_c: Option<f32>,

    /// Override ground albedo seen by the daylight sky model (V-38).
    #[arg(long)]
    surface_albedo: Option<f32>,

    /// V-50 output colour management: the primaries the viewer presents and
    /// stores in saved sessions. `srgb` (default), `display-p3`, or `rec2020`.
    /// When omitted, a `--session` / `--preset` scene keeps its stored value.
    #[arg(long, value_enum)]
    output_colourspace: Option<OutputColourspaceArg>,

    /// V-39 light-pollution Bortle class (1..=9). Class 1 = rural dark sky
    /// (default), Class 9 = inner-city. Mutually exclusive with `--sqm`.
    #[arg(long)]
    bortle: Option<u8>,

    /// V-39 light-pollution manual zenith SQM reading in V mag/arcsec².
    #[arg(long)]
    sqm: Option<f32>,

    /// V-39 light-pollution observer (lat, lng) for the Falchi 2016 atlas
    /// (placeholder; falls back to Bortle 1 until V-39-Atlas ships).
    #[arg(long, num_args = 2, value_names = ["LAT", "LNG"], allow_hyphen_values = true)]
    light_pollution_atlas: Option<Vec<f32>>,

    /// Disable V-39 artificial light pollution: forces the Bortle 1 / dark
    /// sky floor regardless of other flags.
    #[arg(long)]
    no_light_pollution: bool,

    /// Disable Mercury-through-Neptune rendering.
    #[arg(long)]
    no_planets: bool,

    /// Enable the V-55 artificial-satellite layer (TLE / SGP4) using the
    /// curated CelesTrak snapshot. Toggle at runtime with the `L` key.
    #[arg(long)]
    satellites: bool,

    /// Frame-integration exposure (seconds) for satellite motion streaks.
    #[arg(long, default_value_t = 0.0)]
    satellite_exposure_seconds: f32,

    /// Enable the V-47 meteor-shower layer (deterministic shower + sporadic
    /// stream). Toggle at runtime with the `M` key.
    #[arg(long)]
    meteors: bool,

    /// Deterministic meteor-stream seed.
    #[arg(long, default_value_t = 1)]
    meteor_seed: u64,

    /// Multiplier on the modelled meteor rate (1.0 = physical expectation).
    #[arg(long, default_value_t = 1.0)]
    meteor_rate_scale: f32,

    /// Meteor integration window (seconds) for the long-exposure still.
    #[arg(long, default_value_t = 120.0)]
    meteor_window_seconds: f32,

    /// Enable the V-48 aurora layer (auroral-oval arc for the supplied Kp).
    /// Toggle at runtime with the `A` key. Off by default.
    #[arg(long)]
    aurora: bool,

    /// Planetary Kp index (0..9) driving the aurora oval. Implies `--aurora`.
    #[arg(long)]
    aurora_kp: Option<f32>,

    /// Season for the aurora oval shift / dark-sky visibility weight.
    #[arg(long, value_enum)]
    aurora_season: Option<AuroraSeasonArg>,

    /// Enable the V-49 comet layer (curated JPL SBDB elements). Toggle at
    /// runtime with the `C` key.
    #[arg(long)]
    comets: bool,

    /// Enable telescope eyepiece simulation. Supplying any telescope/eyepiece
    /// parameter also enables this mode.
    #[arg(long)]
    eyepiece: bool,

    /// Telescope / OTA clear aperture in millimetres, used for exit-pupil reporting.
    #[arg(long)]
    telescope_aperture_mm: Option<f32>,

    /// Telescope / OTA focal length in millimetres. Sets plate scale and true FOV.
    #[arg(long)]
    telescope_focal_length_mm: Option<f32>,

    /// Eyepiece focal length in millimetres.
    #[arg(long)]
    eyepiece_focal_length_mm: Option<f32>,

    /// Eyepiece apparent field of view in degrees, used when field stop is zero.
    #[arg(long)]
    eyepiece_apparent_fov_deg: Option<f32>,

    /// Eyepiece field-stop diameter in millimetres. Set 0 to derive true FOV from AFOV.
    #[arg(long)]
    eyepiece_field_stop_mm: Option<f32>,

    /// V-45 telescope optical design: `apo-refractor`, `achromat-refractor`,
    /// `newtonian`, or `schmidt-cassegrain`. Press `O` to cycle at runtime.
    #[arg(long, value_name = "DESIGN")]
    telescope_design: Option<String>,

    /// V-45 number of Newtonian spider vanes (implies a Newtonian design).
    #[arg(long)]
    spider_vanes: Option<u8>,

    /// V-45 OTA roll about the optical axis in degrees (`[` / `]` to rotate).
    #[arg(long)]
    ota_rotation_deg: Option<f32>,

    /// Disable atmospheric scintillation (V-24).
    #[arg(long)]
    no_scintillation: bool,

    /// Override the dimensionless Cn² column scale for scintillation.
    #[arg(long)]
    scintillation_scale: Option<f32>,

    /// Override the scintillation noise seed for deterministic replays.
    #[arg(long)]
    scintillation_seed: Option<u32>,

    /// V-56 GoTo: centre the initial view on a named object resolved through
    /// the catalog search index (e.g. "Vega", "M31", "Saturn", "土星"). At
    /// runtime press `/` to open an interactive search prompt in the title
    /// bar, type a query, and press Enter to slew + show the info panel.
    #[arg(long, value_name = "NAME")]
    goto: Option<String>,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    if args.list_presets {
        print_scene_presets();
        return Ok(());
    }

    let mut scene = if let Some(session_path) = &args.session {
        load_session(session_path)?.to_scene()?
    } else if let Some(preset) = args.preset {
        scene_from_preset(preset, &args.catalog, DEFAULT_SCREEN_LIMITING_MAGNITUDE)?
    } else {
        let time = parse_time_to_time_scales(args.time.as_deref())?;
        let overlays = overlay_config_from_args(
            args.no_overlays,
            &args.overlays,
            args.grid_step_deg,
            args.overlay_opacity,
            args.deep_sky_magnitude_limit,
        );
        let atmosphere = atmosphere_from_args(
            args.no_extinction,
            args.atmosphere_preset,
            AtmosphereOverrides {
                aerosol_beta: args.aerosol_beta,
                aerosol_alpha: args.aerosol_alpha,
                observer_altitude_m: args.observer_altitude_m,
                ozone_du: args.ozone_du,
                pressure_hpa: args.pressure_hpa,
                temperature_c: args.temperature_c,
                surface_albedo: args.surface_albedo,
            },
        );
        let (viewpoint, external_viewpoint) = viewpoint_from_args(
            args.viewpoint,
            ExternalViewpointOverrides {
                origin_pc: vec3_arg(&args.external_origin_pc),
                target_pc: vec3_arg(&args.external_target_pc),
                up: vec3_arg(&args.external_up),
            },
        );
        let scintillation = scintillation_from_args(
            args.no_scintillation || args.no_extinction,
            ScintillationOverrides {
                c_n2_scale: args.scintillation_scale,
                seed: args.scintillation_seed,
            },
        );
        let light_pollution = light_pollution_from_args(
            args.no_light_pollution,
            LightPollutionOverrides {
                bortle: args.bortle,
                sqm_mag_per_arcsec2: args.sqm,
                atlas_lat_lng_deg: args.light_pollution_atlas.as_deref().and_then(|v| match v {
                    [lat, lng] => Some((*lat, *lng)),
                    _ => None,
                }),
            },
        );
        SessionScene {
            latitude_deg: args.lat,
            longitude_deg: args.lng,
            time,
            view: LocalView {
                azimuth_rad: (args.azimuth as f32).to_radians(),
                altitude_rad: (args.altitude as f32).to_radians(),
                fov_y_rad: (args.fov as f32).to_radians(),
            },
            overlays,
            atmosphere_preset: args.atmosphere_preset.into(),
            atmosphere,
            light_pollution,
            scintillation,
            planets_enabled: !args.no_planets,
            satellites: curated_satellite_layer(args.satellites, args.satellite_exposure_seconds),
            meteors: MeteorLayer {
                enabled: args.meteors,
                seed: args.meteor_seed,
                rate_scale: args.meteor_rate_scale,
                window_seconds: args.meteor_window_seconds,
            },
            aurora: aurora_from_args(
                args.aurora || args.aurora_kp.is_some(),
                args.aurora_kp.unwrap_or(0.0),
                args.aurora_season.unwrap_or_default(),
            ),
            comets: curated_comet_layer(args.comets),
            projection: args.projection.into(),
            viewpoint,
            external_viewpoint,
            eyepiece: eyepiece_from_args(
                args.eyepiece,
                EyepieceOverrides {
                    aperture_mm: args.telescope_aperture_mm,
                    focal_length_mm: args.telescope_focal_length_mm,
                    eyepiece_focal_length_mm: args.eyepiece_focal_length_mm,
                    apparent_fov_deg: args.eyepiece_apparent_fov_deg,
                    field_stop_mm: args.eyepiece_field_stop_mm,
                    optical_design: args
                        .telescope_design
                        .as_deref()
                        .and_then(OpticalDesign::from_kebab_str)
                        .or_else(|| {
                            args.spider_vanes
                                .map(|v| OpticalDesign::Newtonian { spider_vanes: v })
                        }),
                    ota_rotation_deg: args.ota_rotation_deg,
                },
            ),
            catalog: catalog_snapshot(&args.catalog, DEFAULT_SCREEN_LIMITING_MAGNITUDE),
            corrections: CorrectionSnapshot::for_scene(atmosphere),
            output_colourspace: OutputColourSpace::default(),
        }
    };

    // V-50: explicit flag overrides the scene's stored colour space.
    if let Some(cs) = args.output_colourspace {
        scene.output_colourspace = cs.into();
    }

    // V-48: aurora flags override the scene's stored layer.
    if args.aurora || args.aurora_kp.is_some() || args.aurora_season.is_some() {
        scene.aurora = aurora_from_args(
            true,
            args.aurora_kp.unwrap_or(scene.aurora.kp),
            args.aurora_season
                .unwrap_or_else(|| scene.aurora.season.into()),
        );
    }

    let catalog_path = scene
        .catalog
        .path
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| args.catalog.clone());
    let instances = load_star_instances_from_file(&catalog_path, scene.catalog.limiting_magnitude)?;
    log::info!("Loaded {} stars", instances.len());

    if scene.eyepiece.enabled {
        log::info!(
            "Eyepiece simulation: {:.2}x, {:.3}° true FOV, {:.2} arcsec/mm plate scale, {:.2} mm exit pupil",
            scene.eyepiece.magnification(),
            scene.eyepiece.true_field_deg(),
            scene.eyepiece.plate_scale_arcsec_per_mm(),
            scene.eyepiece.exit_pupil_mm()
        );
    }

    let event_loop = EventLoop::new()?;
    let mut app = App::new(
        instances,
        scene.latitude_deg,
        scene.longitude_deg,
        scene.time.jd_utc,
        scene.view,
        scene.overlays,
        scene.catalog.limiting_magnitude,
        scene.atmosphere,
        scene.scintillation,
        // V-39-Atlas: sample the Falchi 2016 grid for the `Atlas2016` variant
        // when one is configured; the session still records the (lat, lng).
        resolve_light_pollution(scene.light_pollution),
        scene.planets_enabled,
        scene.satellites.clone(),
        scene.meteors.clone(),
        scene.aurora,
        scene.comets.clone(),
        scene.projection,
        scene.viewpoint,
        scene.external_viewpoint,
        scene.eyepiece,
        scene.output_colourspace,
        args.goto.clone(),
    );
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn print_scene_presets() {
    for info in scene_preset_infos() {
        println!(
            "{:<22} {} — {}",
            info.id.as_kebab_str(),
            info.title,
            info.validation_focus
        );
    }
}

fn catalog_snapshot(path: &Path, limiting_magnitude: f32) -> CatalogSnapshot {
    CatalogSnapshot {
        backend: "hyg-csv".to_string(),
        source: "HYG".to_string(),
        version: Some("4.2".to_string()),
        path: Some(path.display().to_string()),
        hash: None,
        limiting_magnitude,
    }
}

fn vec3_arg(values: &Option<Vec<f32>>) -> Option<[f32; 3]> {
    let [x, y, z] = values.as_deref()? else {
        return None;
    };
    Some([*x, *y, *z])
}

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    camera: Camera,
    size: PhysicalSize<u32>,
}

struct App {
    gpu: Option<GpuState>,
    window: Option<Arc<Window>>,
    stars: Vec<StarInstance>,
    lat: f64,
    lng: f64,
    initial_view: LocalView,
    overlays: OverlayConfig,
    limiting_magnitude: f32,
    atmosphere: Atmosphere,
    scintillation: renderer::Scintillation,
    light_pollution: LightPollution,
    planets_enabled: bool,
    /// V-55 artificial-satellite layer (curated TLEs are always loaded so the
    /// `L` key can toggle the layer on/off at runtime).
    satellites: SatelliteLayer,
    /// V-47 meteor-shower layer (the `M` key toggles it at runtime).
    meteors: MeteorLayer,
    /// V-48 aurora layer (toggle with the `A` key).
    aurora: AuroraLayer,
    /// V-49 comet layer (curated elements are always loaded so the `C` key can
    /// toggle the layer on/off at runtime).
    comets: CometLayer,
    projection: renderer::SkyProjection,
    viewpoint: renderer::SkyViewpoint,
    external_viewpoint: renderer::ExternalViewpoint,
    eyepiece: renderer::EyepieceSimulation,
    output_colourspace: OutputColourSpace,
    sky_clock: SkyClock,
    mouse_pressed: bool,
    last_mouse: Option<(f64, f64)>,
    /// V-56 GoTo query supplied via `--goto`, applied once the camera exists.
    pending_goto: Option<String>,
    /// V-56 interactive search: `true` while the title-bar prompt is open.
    search_mode: bool,
    /// In-progress search query typed into the prompt.
    search_query: String,
    /// Info summary of the most recent GoTo target, shown in the title bar.
    goto_info: Option<String>,
}

/// Tracks the rendered moment in JD, advancing real time at a configurable speed.
struct SkyClock {
    /// JD that corresponds to `epoch`.
    anchor_jd: f64,
    /// Real-time instant the anchor was set.
    epoch: Instant,
    /// Multiplier on real-time elapsed seconds. 1.0 = real time, 60.0 = a minute per second.
    speed: f64,
    /// When paused, the JD frozen at the moment of pausing.
    paused_at: Option<f64>,
}

impl SkyClock {
    fn new(start_jd: f64) -> Self {
        Self {
            anchor_jd: start_jd,
            epoch: Instant::now(),
            speed: CLOCK_SPEED_REALTIME,
            paused_at: None,
        }
    }

    fn current_jd(&self) -> f64 {
        if let Some(jd) = self.paused_at {
            jd
        } else {
            self.anchor_jd + self.epoch.elapsed().as_secs_f64() * self.speed / 86_400.0
        }
    }

    fn set_speed(&mut self, speed: f64) {
        self.anchor_jd = self.current_jd();
        self.epoch = Instant::now();
        self.speed = speed;
    }

    fn toggle_pause(&mut self) {
        match self.paused_at {
            Some(jd) => {
                self.anchor_jd = jd;
                self.epoch = Instant::now();
                self.paused_at = None;
            }
            None => self.paused_at = Some(self.current_jd()),
        }
    }
}

impl App {
    #[allow(clippy::too_many_arguments)]
    fn new(
        stars: Vec<StarInstance>,
        lat: f64,
        lng: f64,
        start_jd: f64,
        initial_view: LocalView,
        overlays: OverlayConfig,
        limiting_magnitude: f32,
        atmosphere: Atmosphere,
        scintillation: renderer::Scintillation,
        light_pollution: LightPollution,
        planets_enabled: bool,
        satellites: SatelliteLayer,
        meteors: MeteorLayer,
        aurora: AuroraLayer,
        comets: CometLayer,
        projection: renderer::SkyProjection,
        viewpoint: renderer::SkyViewpoint,
        external_viewpoint: renderer::ExternalViewpoint,
        eyepiece: renderer::EyepieceSimulation,
        output_colourspace: OutputColourSpace,
        pending_goto: Option<String>,
    ) -> Self {
        // Always keep the curated TLEs available so the runtime toggle works
        // even when the layer starts disabled.
        let satellites = SatelliteLayer {
            tles: curated_satellite_layer(true, satellites.exposure_seconds).tles,
            ..satellites
        };
        // Likewise keep the curated comet elements loaded so the `C` key works
        // even when the layer starts disabled.
        let comets = CometLayer {
            comets: curated_comet_layer(true).comets,
            ..comets
        };
        Self {
            gpu: None,
            window: None,
            stars,
            lat,
            lng,
            initial_view,
            overlays,
            limiting_magnitude,
            atmosphere,
            scintillation,
            light_pollution,
            planets_enabled,
            satellites,
            meteors,
            aurora,
            comets,
            projection,
            viewpoint,
            external_viewpoint,
            eyepiece,
            output_colourspace,
            sky_clock: SkyClock::new(start_jd),
            mouse_pressed: false,
            last_mouse: None,
            pending_goto,
            search_mode: false,
            search_query: String::new(),
            goto_info: None,
        }
    }
}

/// Window-title text used as the viewer's lightweight, renderer-free info
/// panel. Shows the live search prompt while typing, otherwise the most
/// recent GoTo target summary, falling back to the bare app name.
/// L-19: log the resolved object's CDS deep links (stars / deep-sky only).
/// Logging keeps the link in the metadata stream without a network call,
/// matching the CLI's metadata exposure.
fn log_deep_links(target: &stars_host_common::GotoTarget) {
    if let Some(url) = &target.simbad_url {
        log::info!("SIMBAD {url}");
    }
    if let Some(url) = &target.vizier_url {
        log::info!("VizieR {url}");
    }
}

fn compose_title(search_mode: bool, query: &str, info: Option<&str>) -> String {
    if search_mode {
        format!("Stars — search: {query}█  (Enter = GoTo, Esc = cancel)")
    } else if let Some(info) = info {
        format!("Stars — {info}")
    } else {
        "Stars".to_string()
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("Stars")
            .with_inner_size(PhysicalSize::new(
                INITIAL_WINDOW_WIDTH,
                INITIAL_WINDOW_HEIGHT,
            ));
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                log::error!("failed to create viewer window: {error}");
                event_loop.exit();
                return;
            }
        };
        self.window = Some(window.clone());

        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = match instance.create_surface(window.clone()) {
            Ok(surface) => surface,
            Err(error) => {
                log::error!("failed to create wgpu surface: {error}");
                event_loop.exit();
                return;
            }
        };
        let adapter =
            match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })) {
                Ok(adapter) => adapter,
                Err(error) => {
                    log::error!("no suitable GPU adapter found: {error}");
                    event_loop.exit();
                    return;
                }
            };

        let (device, queue) =
            match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("Stars Viewer Device"),
                ..Default::default()
            })) {
                Ok(device_and_queue) => device_and_queue,
                Err(error) => {
                    log::error!("failed to create wgpu device: {error}");
                    event_loop.exit();
                    return;
                }
            };

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let mut renderer = Renderer::new(&device, format, size.width, size.height, &self.stars);
        renderer.set_overlays(&device, &self.overlays);
        let observer = Observer::from_degrees(self.lat, self.lng, self.sky_clock.current_jd());
        let mut camera = Camera::new(
            observer,
            self.initial_view,
            size.width as f32 / size.height as f32,
        );
        camera.limiting_magnitude = self.limiting_magnitude;
        camera.atmosphere = self.atmosphere;
        camera.scintillation = self.scintillation;
        camera.light_pollution = self.light_pollution;
        camera.planets_enabled = self.planets_enabled;
        camera.satellites = self.satellites.clone();
        camera.meteors = self.meteors.clone();
        camera.aurora = self.aurora;
        camera.comets = self.comets.clone();
        camera.projection = self.projection;
        camera.viewpoint = self.viewpoint;
        camera.external_viewpoint = self.external_viewpoint;
        camera.eyepiece = self.eyepiece;
        camera.output_colourspace = self.output_colourspace;

        // V-56 GoTo supplied via `--goto`: centre the initial view on the
        // resolved target before the first frame.
        if let Some(query) = self.pending_goto.take() {
            match resolve_goto_query(&query, observer) {
                Ok(target) => {
                    camera.view = target.local_view(camera.view.fov_y_rad);
                    let info = target.info_summary();
                    log::info!("GoTo {info}");
                    log_deep_links(&target);
                    self.goto_info = Some(info);
                }
                Err(error) => log::warn!("GoTo: {error}"),
            }
        }
        window.set_title(&compose_title(
            self.search_mode,
            &self.search_query,
            self.goto_info.as_deref(),
        ));
        log::info!("Press '/' to search and GoTo an object (Enter to confirm, Esc to cancel)");

        self.gpu = Some(GpuState {
            surface,
            device,
            queue,
            config,
            renderer,
            camera,
            size,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Snapshot self-state needed inside the gpu match arms before borrowing
        // self.gpu mutably (Rust can't see that lat/lng/sky_clock and gpu don't alias).
        let jd = self.sky_clock.current_jd();
        let lat = self.lat;
        let lng = self.lng;
        let Some(gpu) = &mut self.gpu else { return };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(new_size) if new_size.width > 0 && new_size.height > 0 => {
                gpu.size = new_size;
                gpu.config.width = new_size.width;
                gpu.config.height = new_size.height;
                gpu.surface.configure(&gpu.device, &gpu.config);
                gpu.camera.aspect = new_size.width as f32 / new_size.height as f32;
                // Keep the renderer's HDR target matched to the swapchain.
                gpu.renderer
                    .resize(&gpu.device, new_size.width, new_size.height);
            }

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.mouse_pressed = state == ElementState::Pressed;
                if !self.mouse_pressed {
                    self.last_mouse = None;
                }
            }

            WindowEvent::CursorMoved { position, .. } if self.mouse_pressed => {
                if let Some((lx, ly)) = self.last_mouse {
                    // Pixel drag scaled by current FOV so it feels consistent at any zoom.
                    let scale = gpu.camera.effective_fov_y_rad() / gpu.size.height as f32;
                    let daz = -(position.x - lx) as f32 * scale;
                    let dalt = (position.y - ly) as f32 * scale;
                    gpu.camera.rotate_view(daz, dalt);
                }
                self.last_mouse = Some((position.x, position.y));
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => {
                        p.y as f32 * PIXEL_SCROLL_TO_LINE_DELTA
                    }
                };
                gpu.camera
                    .zoom_fov((-scroll * ZOOM_SCROLL_SENSITIVITY).exp());
            }

            WindowEvent::KeyboardInput {
                event: key_event, ..
            } if key_event.state == ElementState::Pressed => {
                let code = match key_event.physical_key {
                    PhysicalKey::Code(c) => Some(c),
                    _ => None,
                };

                if self.search_mode {
                    // V-56 interactive search prompt: capture text into the
                    // query, resolve on Enter, slew the camera, and surface the
                    // info summary in the title bar.
                    match code {
                        Some(KeyCode::Escape) => {
                            self.search_mode = false;
                            self.search_query.clear();
                        }
                        Some(KeyCode::Enter) | Some(KeyCode::NumpadEnter) => {
                            let query = std::mem::take(&mut self.search_query);
                            self.search_mode = false;
                            let observer = Observer::from_degrees(lat, lng, jd);
                            match resolve_goto_query(&query, observer) {
                                Ok(target) => {
                                    gpu.camera.view = target.local_view(gpu.camera.view.fov_y_rad);
                                    let info = target.info_summary();
                                    log::info!("GoTo {info}");
                                    log_deep_links(&target);
                                    self.goto_info = Some(info);
                                }
                                Err(error) => {
                                    log::warn!("GoTo: {error}");
                                    self.goto_info = Some(format!("not found: {query}"));
                                }
                            }
                        }
                        Some(KeyCode::Backspace) => {
                            self.search_query.pop();
                        }
                        _ => {
                            if let Some(text) = &key_event.text {
                                for ch in text.chars() {
                                    if !ch.is_control() {
                                        self.search_query.push(ch);
                                    }
                                }
                            }
                        }
                    }
                } else {
                    match code {
                        Some(KeyCode::Slash) => {
                            self.search_mode = true;
                            self.search_query.clear();
                        }
                        Some(KeyCode::KeyL) => {
                            // V-55: toggle the artificial-satellite layer.
                            gpu.camera.satellites.enabled = !gpu.camera.satellites.enabled;
                            log::info!(
                                "satellites {}",
                                if gpu.camera.satellites.enabled {
                                    "on"
                                } else {
                                    "off"
                                }
                            );
                        }
                        Some(KeyCode::KeyO) => {
                            // V-45: cycle the telescope optical design (only
                            // visible while eyepiece mode is active).
                            gpu.camera.eyepiece.optical_design =
                                next_optical_design(gpu.camera.eyepiece.optical_design);
                            log::info!(
                                "telescope design: {}",
                                gpu.camera.eyepiece.optical_design.as_kebab_str()
                            );
                        }
                        Some(KeyCode::BracketRight) => {
                            // V-45: roll the OTA +15° (rotates spider spikes).
                            gpu.camera.eyepiece.ota_rotation_deg =
                                (gpu.camera.eyepiece.ota_rotation_deg + 15.0).rem_euclid(360.0);
                        }
                        Some(KeyCode::BracketLeft) => {
                            gpu.camera.eyepiece.ota_rotation_deg =
                                (gpu.camera.eyepiece.ota_rotation_deg - 15.0).rem_euclid(360.0);
                        }
                        Some(KeyCode::KeyM) => {
                            // V-47: toggle the meteor-shower layer.
                            gpu.camera.meteors.enabled = !gpu.camera.meteors.enabled;
                            log::info!(
                                "meteors {}",
                                if gpu.camera.meteors.enabled {
                                    "on"
                                } else {
                                    "off"
                                }
                            );
                        }
                        Some(KeyCode::KeyA) => {
                            // V-48: toggle the aurora layer. Default to Kp=5 if
                            // no activity level was supplied so the toggle is
                            // visibly useful out of the box.
                            gpu.camera.aurora.enabled = !gpu.camera.aurora.enabled;
                            if gpu.camera.aurora.enabled && gpu.camera.aurora.kp <= 0.0 {
                                gpu.camera.aurora.kp = 5.0;
                            }
                            self.aurora = gpu.camera.aurora;
                            log::info!(
                                "aurora {} (Kp {:.1})",
                                if gpu.camera.aurora.enabled {
                                    "on"
                                } else {
                                    "off"
                                },
                                gpu.camera.aurora.kp
                            );
                        }
                        Some(KeyCode::KeyC) => {
                            // V-49: toggle the comet layer.
                            gpu.camera.comets.enabled = !gpu.camera.comets.enabled;
                            log::info!(
                                "comets {}",
                                if gpu.camera.comets.enabled {
                                    "on"
                                } else {
                                    "off"
                                }
                            );
                        }
                        Some(KeyCode::Space) => self.sky_clock.toggle_pause(),
                        Some(KeyCode::Digit1) => self.sky_clock.set_speed(CLOCK_SPEED_REALTIME),
                        Some(KeyCode::Digit2) => {
                            self.sky_clock.set_speed(CLOCK_SPEED_MINUTE_PER_SECOND)
                        }
                        Some(KeyCode::Digit3) => {
                            self.sky_clock.set_speed(CLOCK_SPEED_HOUR_PER_SECOND)
                        }
                        Some(KeyCode::Digit4) => {
                            self.sky_clock.set_speed(CLOCK_SPEED_DAY_PER_SECOND)
                        }
                        Some(KeyCode::Escape) => event_loop.exit(),
                        _ => {}
                    }
                }

                if let Some(window) = &self.window {
                    window.set_title(&compose_title(
                        self.search_mode,
                        &self.search_query,
                        self.goto_info.as_deref(),
                    ));
                }
            }

            WindowEvent::RedrawRequested => {
                gpu.camera.observer = Observer::from_degrees(lat, lng, jd);
                gpu.renderer.update_camera(
                    &gpu.queue,
                    &gpu.camera,
                    gpu.size.width,
                    gpu.size.height,
                );

                let surface_texture = match gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) => t,
                    wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                        gpu.surface.configure(&gpu.device, &gpu.config);
                        t
                    }
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        gpu.surface.configure(&gpu.device, &gpu.config);
                        return;
                    }
                    // A future wgpu release may introduce new variants. Skip
                    // the frame but make the skip visible — silently dropping
                    // frames here masked a real OS-level surface failure once.
                    other => {
                        log::warn!("unexpected surface state: {other:?}; skipping frame");
                        return;
                    }
                };

                let view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    gpu.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Render Encoder"),
                        });

                gpu.renderer.render(&mut encoder, &view);
                gpu.queue.submit(std::iter::once(encoder.finish()));
                surface_texture.present();

                if let Some(window) = &self.window {
                    window.request_redraw();
                } else {
                    log::warn!("redraw completed but viewer window is missing");
                }
            }

            _ => {}
        }
    }
}
