use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use astronomy::Observer;
use clap::Parser;
use renderer::{LocalView, SkyViewpoint, DEFAULT_SCREEN_LIMITING_MAGNITUDE};
use stars_host_common::{
    atmosphere_from_args, eyepiece_from_args, hyg_catalog_snapshot, light_pollution_from_args,
    load_session, overlay_config_from_args, parse_time_to_time_scales,
    render_scene_from_catalog_path, resolve_goto_query, save_session, scene_from_preset,
    scene_preset_infos, scintillation_from_args, viewpoint_from_args, AtmosphereOverrides,
    AtmospherePresetArg, CorrectionSnapshot, ExternalViewpointOverrides, EyepieceOverrides,
    LightPollutionOverrides, OverlayArg, ProjectionArg, RenderOptions, ScenePresetArg,
    ScintillationOverrides, SessionScene, StarSession, ViewpointArg,
};

/// Render the night sky as seen from a given observer to a PNG.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Observer latitude in decimal degrees (north positive). Required unless --session is supplied.
    #[arg(long, allow_hyphen_values = true)]
    lat: Option<f64>,

    /// Observer longitude in decimal degrees (east positive). Required unless --session is supplied.
    #[arg(long, allow_hyphen_values = true)]
    lng: Option<f64>,

    /// Load a schema-versioned JSON session. Scene fields in the session drive
    /// the render; output size/path and catalog fallback still come from CLI flags.
    #[arg(long)]
    session: Option<PathBuf>,

    /// Use a built-in deterministic validation/demo scene. Ignored when --session is supplied.
    #[arg(long, value_enum)]
    preset: Option<ScenePresetArg>,

    /// List built-in deterministic validation/demo scene presets and exit.
    #[arg(long)]
    list_presets: bool,

    /// Write the effective scene to a schema-versioned JSON session file.
    #[arg(long)]
    write_session: Option<PathBuf>,

    /// Exit after writing --write-session. Useful for exporting preset JSON without rendering.
    #[arg(long, requires = "write_session")]
    write_session_only: bool,

    /// Time as RFC3339 (e.g. 2026-04-26T12:00:00Z). Defaults to "now".
    #[arg(long)]
    time: Option<String>,

    /// Azimuth of the camera, degrees from North toward East.
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    azimuth: f64,

    /// Altitude of the camera, degrees above the horizon.
    #[arg(long, default_value_t = 30.0, allow_hyphen_values = true)]
    altitude: f64,

    /// Vertical field of view, degrees. Ignored by all-sky projections.
    #[arg(long, default_value_t = 60.0)]
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

    /// Output image width.
    #[arg(long, default_value_t = 1280)]
    width: u32,

    /// Output image height.
    #[arg(long, default_value_t = 720)]
    height: u32,

    /// Output PNG path.
    #[arg(short, long, default_value = "stars.png")]
    output: PathBuf,

    /// Path to the HYG-format star catalog CSV.
    #[arg(long, default_value = "crates/catalog/data/hyg_v42.csv")]
    catalog: PathBuf,

    /// Overlay layers to draw. Comma-separated list, or pass --no-overlays to disable all.
    ///
    /// Possible values: horizon, cardinals, alt-az-grid, equatorial-grid,
    /// ecliptic, celestial-equator, meridian, galactic-equator,
    /// constellation-lines, constellation-boundaries, star-labels,
    /// planet-labels, constellation-labels, cardinal-labels, degree-labels.
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

    /// Limiting apparent magnitude of the simulated observer. Stars fainter
    /// than this fade through the shader's discard cutoff. Increasing this
    /// uniformly scales every star's linear flux ("more sensitive observer"
    /// / "longer exposure") without breaking Pogson's law.
    ///
    /// 6.0 = strict dark-adapted naked eye; 7.5 ≈ binocular visual limit and
    /// is a good default for indoor screens.
    #[arg(long, default_value_t = DEFAULT_SCREEN_LIMITING_MAGNITUDE)]
    limiting_magnitude: f32,

    /// Disable atmospheric extinction and sunlit sky scattering. With the
    /// default atmosphere on, stars near the horizon dim/redden and daylight
    /// or twilight sky colour is driven by the Sun position. This flag turns
    /// the atmosphere off, so every star renders at catalogue magnitude and
    /// the sky background contains only non-atmospheric components.
    #[arg(long)]
    no_extinction: bool,

    /// Atmosphere preset used as the base for extinction and sky colour.
    #[arg(long, default_value_t = AtmospherePresetArg::ClearRural)]
    atmosphere_preset: AtmospherePresetArg,

    /// Ångström aerosol optical depth at 550 nm (β). Drives both stellar
    /// k(λ) and the daylight Mie aerosol term through the unified V-37 state.
    /// Clean continental sites ≈ 0.05; mid-quality observatories ≈ 0.10;
    /// hazy urban skies ≥ 0.30.
    #[arg(long)]
    aerosol_beta: Option<f32>,

    /// Ångström wavelength exponent (α). Continental aerosols ≈ 1.3; coarser
    /// maritime / dust aerosols 0.8–1.0.
    #[arg(long)]
    aerosol_alpha: Option<f32>,

    /// Override observer altitude above sea level in metres. Rayleigh and
    /// aerosol extinction thin exponentially with the standard 8 km scale
    /// height.
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
    /// Defaults to a per-preset value (clear-rural ≈ 0.10, hazy-urban
    /// ≈ 0.13, high-altitude ≈ 0.30). Snow / desert ground brightens the
    /// zenith via Hošek-Wilkie's ground-albedo coupling.
    #[arg(long)]
    surface_albedo: Option<f32>,

    /// V-39 light-pollution Bortle class (1..=9). Class 1 = rural dark sky
    /// (default), Class 9 = inner-city. Mutually exclusive with `--sqm`;
    /// `--bortle` wins if both are passed. Adds a sodium / LED-tinted
    /// Garstang-scaled artificial sky-glow to the dark-sky composition
    /// before extinction.
    #[arg(long)]
    bortle: Option<u8>,

    /// V-39 light-pollution manual zenith SQM reading in V mag/arcsec².
    /// Useful when the local Bortle class is unknown but a SQM-L
    /// measurement is on hand. Clamped to `16.0..=22.0`.
    #[arg(long)]
    sqm: Option<f32>,

    /// V-39 light-pollution observer (lat, lng) for the Falchi 2016 World
    /// Atlas sample. Currently a `TODO(V-39-Atlas)` placeholder that falls
    /// back to Bortle 1 + a log line; the schema is laid down so the
    /// loader can ship in a follow-up PR without breaking sessions.
    #[arg(long, num_args = 2, value_names = ["LAT", "LNG"], allow_hyphen_values = true)]
    light_pollution_atlas: Option<Vec<f32>>,

    /// Disable V-39 artificial light pollution: forces the Bortle 1 / dark
    /// sky floor regardless of `--bortle` / `--sqm`. Matches the existing
    /// `--no-extinction` / `--no-scintillation` flag-style.
    #[arg(long)]
    no_light_pollution: bool,

    /// Disable Mercury-through-Neptune rendering.
    #[arg(long)]
    no_planets: bool,

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

    /// Disable the diffuse-sky (integrated starlight + diffuse galactic
    /// light) skyglow pass. With the default (skyglow on), the sky
    /// background includes the analytic Leinert et al. 1998 model so the
    /// Milky Way band is visible against the dark sky.
    #[arg(long)]
    no_skyglow: bool,

    /// Disable atmospheric scintillation (V-24). With the default model on,
    /// each star's RGB flux is modulated by a deterministic band-limited
    /// noise whose variance scales as sec(z)³ and damps with observer
    /// altitude (Young 1967 / Dravins 1997-98).
    #[arg(long)]
    no_scintillation: bool,

    /// Override the dimensionless Cn² column scale. `1.0` reproduces the
    /// Dravins 1997 amateur-site σ ≈ 4 % at the zenith for a 7 mm pupil at
    /// sea level. Use < 1 for a calmer sky, > 1 for a more turbulent one.
    #[arg(long)]
    scintillation_scale: Option<f32>,

    /// Override the scintillation noise seed for deterministic replays.
    #[arg(long)]
    scintillation_seed: Option<u32>,

    /// V-56 GoTo: centre the view on a named object resolved through the
    /// catalog search index. Accepts proper names, Bayer/Flamsteed, HD/HIP/HR,
    /// Messier/NGC/IC ids, planets, the Sun/Moon, and Japanese aliases
    /// (e.g. "Vega", "Alp CMa", "M31", "NGC 869", "Saturn", "土星"). Overrides
    /// --azimuth / --altitude and prints an info summary before rendering.
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
        scene_from_preset(preset, &args.catalog, args.limiting_magnitude)?
    } else {
        let lat = args
            .lat
            .context("--lat is required unless --session is supplied")?;
        let lng = args
            .lng
            .context("--lng is required unless --session is supplied")?;
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
        let (viewpoint, external_viewpoint) = viewpoint_from_args(
            args.viewpoint,
            ExternalViewpointOverrides {
                origin_pc: vec3_arg(&args.external_origin_pc),
                target_pc: vec3_arg(&args.external_target_pc),
                up: vec3_arg(&args.external_up),
            },
        );
        SessionScene {
            latitude_deg: lat,
            longitude_deg: lng,
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
                },
            ),
            catalog: hyg_catalog_snapshot(&args.catalog, args.limiting_magnitude),
            corrections: CorrectionSnapshot::for_scene(atmosphere),
        }
    };

    // V-56 GoTo: resolve a named target and centre the view on it. Applied
    // after the scene is built so it works with --session / --preset too.
    if let Some(query) = &args.goto {
        let observer =
            Observer::from_degrees_with_time(scene.latitude_deg, scene.longitude_deg, scene.time);
        let target = resolve_goto_query(query, observer)?;
        if scene.viewpoint != SkyViewpoint::Earth {
            log::warn!(
                "--goto centres the local alt-az view, which is ignored by the \
                 current non-Earth viewpoint; rendering the target's info only"
            );
        }
        scene.view = target.local_view(scene.view.fov_y_rad);
        println!("GoTo {}", target.info_summary());
        log::info!("GoTo target: {}", target.info_summary());
    }

    log::info!(
        "Observer lat={} lng={} jd_utc={} jd_ut1={} jd_tdb={}",
        scene.latitude_deg,
        scene.longitude_deg,
        scene.time.jd_utc,
        scene.time.jd_ut1,
        scene.time.jd_tdb
    );

    if scene.eyepiece.enabled {
        log::info!(
            "Eyepiece simulation: {:.2}x, {:.3}° true FOV, {:.2} arcsec/mm plate scale, {:.2} mm exit pupil",
            scene.eyepiece.magnification(),
            scene.eyepiece.true_field_deg(),
            scene.eyepiece.plate_scale_arcsec_per_mm(),
            scene.eyepiece.exit_pupil_mm()
        );
    }

    if let Some(path) = &args.write_session {
        let session = StarSession::from_scene(env!("CARGO_PKG_VERSION"), "stars-cli", &scene);
        save_session(path, &session)?;
        log::info!("Wrote session JSON to {}", path.display());
    } else if args.write_session_only {
        bail!("--write-session-only requires --write-session");
    }
    if args.write_session_only {
        return Ok(());
    }

    let pixels = pollster::block_on(render_scene_from_catalog_path(
        &scene,
        &args.catalog,
        RenderOptions {
            width: args.width,
            height: args.height,
            skyglow_enabled: !args.no_skyglow,
        },
    ))?;

    let img = image::RgbaImage::from_raw(args.width, args.height, pixels)
        .context("Pixel buffer size mismatch")?;
    img.save(&args.output)
        .with_context(|| format!("Failed to write {}", args.output.display()))?;

    log::info!("Wrote {}", args.output.display());
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

fn vec3_arg(values: &Option<Vec<f32>>) -> Option<[f32; 3]> {
    let [x, y, z] = values.as_deref()? else {
        return None;
    };
    Some([*x, *y, *z])
}

#[cfg(test)]
mod tests {
    use super::*;
    use astronomy::TimeScales;

    fn parse(args: &[&str]) -> Args {
        Args::try_parse_from(args).expect("args parse")
    }

    #[test]
    fn goto_flag_defaults_to_none() {
        let args = parse(&["stars", "--lat", "35.68", "--lng", "139.69"]);
        assert!(args.goto.is_none());
    }

    #[test]
    fn goto_flag_parses_value() {
        let args = parse(&[
            "stars", "--lat", "35.68", "--lng", "139.69", "--goto", "Vega",
        ]);
        assert_eq!(args.goto.as_deref(), Some("Vega"));
    }

    #[test]
    fn goto_flag_accepts_multiword_designation() {
        let args = parse(&[
            "stars", "--lat", "35.68", "--lng", "139.69", "--goto", "Alp CMa",
        ]);
        assert_eq!(args.goto.as_deref(), Some("Alp CMa"));
    }

    /// End-to-end wiring: the parsed `--goto` value resolves through the shared
    /// resolver and yields a finite, centred local view — the same path
    /// `main` takes before rendering.
    #[test]
    fn goto_value_resolves_and_centres_view() {
        let args = parse(&[
            "stars", "--lat", "35.68", "--lng", "139.69", "--goto", "Vega",
        ]);
        let query = args.goto.expect("goto present");
        let observer = Observer::from_degrees_with_time(
            35.68,
            139.69,
            TimeScales::from_utc_julian_date(2_461_157.0),
        );
        let target = resolve_goto_query(&query, observer).expect("Vega resolves");
        let view = target.local_view(60.0_f32.to_radians());
        assert!(view.altitude_rad.is_finite() && view.azimuth_rad.is_finite());
        assert!(target.info_summary().contains("Vega"));
    }
}
