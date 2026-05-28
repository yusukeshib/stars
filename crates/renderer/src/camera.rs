use astronomy::{
    active_occluders, apparent_galilean_moons_topocentric, apparent_planets_topocentric,
    apparent_saturn_ring_topocentric, apparent_titan_topocentric, earth_velocity_over_c_j2000,
    equation_of_equinoxes, equatorial_to_horizontal_matrix, illuminants, lmst_radians,
    precession_nutation_matrix, solar_eclipse_state, years_since_j2000, GalileanMoon, Observer,
    Planet, SolarEclipseKind, SunMoonApparent, MAX_OCCLUDERS,
};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

use crate::vertex::{limiting_magnitude_to_zeropoint, NAKED_EYE_LIMITING_MAGNITUDE};

/// Sky-to-screen projection used by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkyProjection {
    /// Existing rectilinear camera with the configured vertical field of view.
    #[default]
    Perspective,
    /// Equal-area all-sky ellipse; useful for Milky Way / galaxy-scale structure.
    Mollweide,
    /// Classical all-sky compromise projection used by many star atlases.
    Aitoff,
    /// Equal-area all-sky projection with less edge stretching than Aitoff.
    Hammer,
}

impl SkyProjection {
    pub const ALL: &'static [Self] = &[
        Self::Perspective,
        Self::Mollweide,
        Self::Aitoff,
        Self::Hammer,
    ];

    pub const fn as_kebab_str(self) -> &'static str {
        match self {
            Self::Perspective => "perspective",
            Self::Mollweide => "mollweide",
            Self::Aitoff => "aitoff",
            Self::Hammer => "hammer",
        }
    }

    pub fn from_kebab_str(s: &str) -> Option<Self> {
        Some(match s {
            "perspective" => Self::Perspective,
            "mollweide" => Self::Mollweide,
            "aitoff" => Self::Aitoff,
            "hammer" => Self::Hammer,
            _ => return None,
        })
    }

    const fn shader_mode(self) -> f32 {
        match self {
            Self::Perspective => 0.0,
            Self::Mollweide => 1.0,
            Self::Aitoff => 2.0,
            Self::Hammer => 3.0,
        }
    }

    const fn is_full_sky(self) -> bool {
        !matches!(self, Self::Perspective)
    }
}

/// Where the camera is located. The default is the Earth-centred celestial
/// sphere; external Phase-4 modes move the camera into a parsec-scale IAU
/// galactic Cartesian frame and project catalogue stars using their HYG
/// distances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkyViewpoint {
    /// Observer-centred sky dome with atmosphere, refraction, ephemerides, and
    /// the regular horizontal/equatorial overlay stack.
    #[default]
    Earth,
    /// Preset external top-down map of the local Milky Way disc from the north
    /// galactic pole. The Sun is at the origin, +X points to l=0°, +Y to
    /// l=90°, and +Z to the north galactic pole.
    GalacticNorth,
    /// User-supplied parsec-scale origin/orientation in the same IAU galactic
    /// Cartesian frame as [`Self::GalacticNorth`].
    CustomExternal,
}

impl SkyViewpoint {
    pub const ALL: &'static [Self] = &[Self::Earth, Self::GalacticNorth, Self::CustomExternal];

    pub const fn as_kebab_str(self) -> &'static str {
        match self {
            Self::Earth => "earth",
            Self::GalacticNorth => "galactic-north",
            Self::CustomExternal => "custom-external",
        }
    }

    pub fn from_kebab_str(s: &str) -> Option<Self> {
        Some(match s {
            "earth" => Self::Earth,
            "galactic-north" => Self::GalacticNorth,
            "custom-external" => Self::CustomExternal,
            _ => return None,
        })
    }

    pub(crate) const fn shader_mode(self) -> f32 {
        match self {
            Self::Earth => 0.0,
            Self::GalacticNorth | Self::CustomExternal => 1.0,
        }
    }

    pub(crate) const fn is_external(self) -> bool {
        !matches!(self, Self::Earth)
    }
}

/// Height of the default external galactic camera above the Sun in parsecs. With
/// the regular 60° default FoV this shows a ∼35 kpc-wide neighbourhood, enough
/// for the HYG local stars plus the analytic Milky Way disc context.
const GALACTIC_CAMERA_HEIGHT_PC: f32 = 30_000.0;
const EXTERNAL_SCENE_RADIUS_PC: f32 = 80_000.0;

/// User-configurable external viewpoint in IAU galactic Cartesian parsecs.
/// The Sun is `(0, 0, 0)`, `+X` points toward galactic longitude `l=0°`, `+Y`
/// toward `l=90°`, and `+Z` toward the north galactic pole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExternalViewpoint {
    pub origin_pc: [f32; 3],
    pub target_pc: [f32; 3],
    pub up: [f32; 3],
}

impl ExternalViewpoint {
    pub const GALACTIC_NORTH: Self = Self {
        origin_pc: [0.0, 0.0, GALACTIC_CAMERA_HEIGHT_PC],
        target_pc: [0.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
    };

    pub const fn new(origin_pc: [f32; 3], target_pc: [f32; 3], up: [f32; 3]) -> Self {
        Self {
            origin_pc,
            target_pc,
            up,
        }
    }
}

impl Default for ExternalViewpoint {
    fn default() -> Self {
        Self::GALACTIC_NORTH
    }
}

/// GPU-side camera + atmosphere state. WGSL layout requires `vec3` fields to
/// be 16-byte aligned, so the per-channel extinction coefficients and the
/// equatorial "zenith" direction are stored as `vec4` with an unused `w`
/// component. This keeps the Rust struct byte-for-byte identical to the
/// shader's view of it.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub(crate) struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    /// Inverse of `view_proj`. Lets a fullscreen pass reconstruct the
    /// world-space ray direction for each pixel from its clip-space
    /// coordinate — used by the skyglow pass to sample the
    /// surface-brightness model in galactic coordinates.
    pub inv_view_proj: [[f32; 4]; 4],
    /// Rotation from equatorial coordinates into local ENU. The star shader
    /// uses this to apply atmospheric refraction in the observer's horizontal
    /// frame before projecting with [`Self::view_proj_local`].
    pub eq_to_local: [[f32; 4]; 4],
    /// View-projection matrix for local ENU geometry. Refracted star positions
    /// are local apparent directions, not unrefracted equatorial directions.
    pub view_proj_local: [[f32; 4]; 4],
    /// IAU 2006 precession + compact IAU-2000 nutation matrix from J2000
    /// equatorial vectors into the true equator/equinox of date.
    pub j2000_to_date: [[f32; 4]; 4],
    /// `[Earth velocity x/c, y/c, z/c, years since J2000.0 TT]` for annual
    /// aberration and catalogue proper motion in the star shader.
    pub aberration_pm: [f32; 4],
    /// `[pressure_hpa, temperature_c, unused, unused]` for Saemundsson
    /// atmospheric refraction scaling.
    pub refraction_params: [f32; 4],
    /// `[viewport_width, viewport_height, pixel_solid_angle_sr, magnitude_zeropoint]`.
    /// Packed into one `vec4` for WGSL 16-byte alignment.
    ///
    /// * `pixel_solid_angle_sr` lets a surface-brightness pass convert a
    ///   per-arcsec² flux into the per-pixel HDR contribution the rest of
    ///   the pipeline expects.
    /// * `magnitude_zeropoint` is the apparent magnitude at which the
    ///   renderer's brightness scale is 1.0 (see
    ///   [`limiting_magnitude_to_zeropoint`] / `vertex::magnitude_to_render_params`).
    ///   Sharing it lets the skyglow pass produce HDR values on the same
    ///   scale as the star pass.
    pub viewport_pixel_sr_zeropoint: [f32; 4],
    /// Observer's local "up" expressed in J2000 equatorial coordinates.
    /// The shader uses `sin(alt) = dot(star_pos, zenith_eq)` to derive
    /// per-star altitude without re-uploading the rotation matrix.
    /// `w` is unused (alignment padding).
    pub zenith_eq: [f32; 4],
    /// Per-channel atmospheric extinction coefficients (mag per airmass).
    /// Set to `[0, 0, 0, 0]` to disable extinction. `w` is unused.
    pub extinction_k_rgb: [f32; 4],
    /// Apparent Sun direction in equatorial coordinates. `w` is the apparent
    /// solar angular radius in radians.
    pub sun_eq_radius: [f32; 4],
    /// Atmosphere controls for the sunlit-scattering shader:
    /// `[linke_turbidity_eff, observer_altitude_m, solar_illuminance_lux,
    /// scattering_enabled]`. `linke_turbidity_eff` is derived from `(β, α)`
    /// per V-37 so the daylight model (Hošek-Wilkie 2012, V-38) and the
    /// stellar extinction path share one (β, α, DU) state.
    pub atmosphere_params: [f32; 4],
    /// Top-of-atmosphere solar RGB illuminant, normalised around D65. `w` is
    /// currently unused.
    pub solar_rgb: [f32; 4],
    /// Unified spectral-extinction state shared by the stellar and daylight
    /// paths: `[ozone_du, aerosol_beta, aerosol_alpha, unused]` (V-37).
    pub atmosphere_optics: [f32; 4],
    /// Apparent Moon direction in equatorial coordinates. `w` is approximate
    /// moonlight illuminance in lux before local horizon/airmass attenuation.
    pub moon_eq_illuminance: [f32; 4],
    /// Lunar disk inputs: `[angular_radius_rad, illuminated_fraction,
    /// phase_angle_rad, earth_shadow_fraction]`.
    pub moon_disk: [f32; 4],
    /// Projection controls: `[mode, map_scale_x, map_scale_y, full_sky_flag]`.
    /// `mode = 0` preserves the perspective matrix path; `1..=3` select
    /// Mollweide, Aitoff, and Hammer all-sky maps. The scale terms fit the
    /// natural 2:1 map ellipse into the current framebuffer without stretching.
    pub projection_params: [f32; 4],
    /// Viewpoint controls: `[mode, external_eye_x_pc, external_eye_y_pc,
    /// external_eye_z_pc]`. `mode = 0` is the Earth-centred sky dome; `1` is
    /// an external parsec-scale camera in IAU galactic Cartesian coordinates.
    pub viewpoint_params: [f32; 4],
    /// Planet directions in equatorial coordinates. `w` is angular radius.
    pub planet_eq_radius: PlanetEqRadiusUniform,
    /// Planet display colour in linear RGB. `w` is apparent visual magnitude.
    pub planet_rgb_magnitude: PlanetRgbMagnitudeUniform,
    /// `[planet_count, planets_enabled, unused, unused]`.
    pub planet_params: [f32; 4],
    /// V-52a Saturn ring orientation. `xyz` = unit vector along Saturn's north
    /// ring pole, expressed in the same J2000-equatorial frame the shader
    /// receives `planet_eq_radius` in. `w` = `sin B`, the signed sub-Earth
    /// Saturnicentric latitude; the magnitude controls the ring ellipse's
    /// vertical compression and the sign selects which face the body occults.
    pub saturn_ring_pole_sinb: [f32; 4],
    /// V-52a Saturn ring photometric controls. Layout:
    /// `[sin_b_sun, enabled, reserved, reserved]`.
    /// * `sin_b_sun` is `sin B'`, the signed sub-Sun Saturnicentric latitude;
    ///   shares a sign with `sin B` whenever the lit face is the visible one.
    /// * `enabled` is `1.0` when the ring pass should fire (Saturn above the
    ///   horizon and planets globally on), `0.0` otherwise.
    pub saturn_ring_state: [f32; 4],
    /// V-52b Galilean-moon directions in J2000 equatorial coordinates.
    /// `xyz` is the unit direction toward each moon (refracted near the
    /// horizon when atmosphere is enabled, identical to the planet path);
    /// `w` is the moon's physical angular radius in radians. The Galilean
    /// moons are sub-pixel at every naked-eye / small-eyepiece FoV so the
    /// shader actually renders them as point sources, but the angular
    /// radius is kept here to mirror the planet uniform shape and to let a
    /// future telescope-grade renderer resolve them.
    pub galilean_eq_radius: GalileanEqRadiusUniform,
    /// V-52b Galilean-moon display colour in linear RGB. `w` is apparent
    /// visual magnitude (Meeus 1998 ch. 44 reduced magnitude + `5·log10(rΔ)`).
    pub galilean_rgb_magnitude: GalileanRgbMagnitudeUniform,
    /// V-52b Galilean-moon control header:
    /// `[count, enabled, reserved, reserved]`. `count` is `GALILEAN_UNIFORM_COUNT`
    /// as an `f32`; `enabled` is `1.0` when Jupiter is above the horizon and
    /// planets are globally on, `0.0` otherwise.
    pub galilean_params: [f32; 4],
    /// V-52c Titan direction in J2000 equatorial coordinates.
    /// `xyz` is the unit direction toward Titan (refracted near the
    /// horizon when atmosphere is enabled, identical to the planet path);
    /// `w` is Titan's physical angular radius in radians. Titan is
    /// sub-arcsecond at every Earth-Saturn geometry so the shader renders
    /// it as a point source, but the angular radius is kept here to
    /// mirror the Galilean uniform shape and to let a future
    /// telescope-grade renderer resolve the disk.
    pub titan_eq_radius: [f32; 4],
    /// V-52c Titan display colour in linear RGB. `w` is apparent visual
    /// magnitude (Karkoschka 1998 `V(1, 0)` reduced magnitude +
    /// `5·log10(r · Δ)` with Saturn's distances).
    pub titan_rgb_magnitude: [f32; 4],
    /// V-52c Titan control header:
    /// `[count, enabled, reserved, reserved]`. `count` is
    /// `TITAN_UNIFORM_COUNT` (always `1.0`) as an `f32`; `enabled` is
    /// `1.0` when Saturn is above the horizon and planets are globally
    /// on, `0.0` otherwise. The shape mirrors `galilean_params` so the
    /// shader can iterate this and future Saturnian-moon blocks
    /// uniformly.
    pub titan_params: [f32; 4],
    /// Hošek-Wilkie 2012 RGB sky-dome coefficients pre-cooked on the host
    /// each frame (V-38). Nine `vec4`s; row `i` holds the per-channel `i`-th
    /// analytic coefficient (A..I) as `(R, G, B, _)`. Unused when
    /// `atmosphere_params.w != 1.0`.
    pub hw_coeffs: HosekWilkieCoefficientsUniform,
    /// Per-channel Hošek-Wilkie master radiance scales as `(R, G, B, _)`.
    /// Unused when `atmosphere_params.w != 1.0`.
    pub hw_radiance: [f32; 4],
    /// V-24 scintillation state: `[sigma_sq_zenith, corner_hz_zenith,
    /// seed_as_f32, time_seconds_mod_day]`. `sigma_sq_zenith == 0` disables
    /// the per-star modulation in the shader. `seed_as_f32` is a `bitcast`
    /// of the host-side `u32` seed; the shader recovers it with the same
    /// `bitcast<u32>` so the noise field is deterministic across hosts.
    /// `time_seconds_mod_day` is `fract(jd_ut1) * 86400`, so two renders of
    /// the same session at the same simulated UT1 produce identical pixels.
    pub scintillation_params: [f32; 4],
    /// V-51c solar-eclipse state for the analytic Moon-on-Sun occluder
    /// path. Layout:
    /// `[kind_code, obscuration_fraction, totality_weight, partial_weight]`.
    /// * `kind_code` is [`SolarEclipseKind::shader_code`]:
    ///   0 = none, 1 = partial, 2 = annular, 3 = total.
    /// * `obscuration_fraction` in `[0, 1]` is the fraction of the solar
    ///   disk currently hidden by the Moon; the shader uses it to scale
    ///   the daylight scattering radiance (Koomen 1952 falloff).
    /// * `totality_weight` is a smoothstep on `obscuration` that climbs
    ///   from 0 at second contact to 1 well inside totality; the shader
    ///   reads it to gate the Baumbach 1937 corona term.
    /// * `partial_weight` mirrors the same smoothstep but covers the
    ///   partial / annular phase, so the daylight Koomen falloff fades
    ///   in continuously around C2 / C3 instead of stepping.
    pub solar_eclipse_state: [f32; 4],
    /// V-51b analytic-mask occluder array. Each active occluder occupies
    /// two consecutive `vec4` rows (`MAX_OCCLUDERS * 2` slots):
    ///
    /// * row `2i`   `front_dir_radius`: xyz = front-disk equatorial
    ///   direction (unit), w = angular radius in radians.
    /// * row `2i+1` `target_kind`: x = [`OccluderTarget::shader_code`]
    ///   (Sun = 0, Moon = 1, Planet(`k`) = 2+`k`, Stars = -1),
    ///   y = [`OccultationKind::shader_code`], z = obscuration fraction,
    ///   w = reserved (0).
    ///
    /// The shader iterates `0..occluder_count.x` and applies one
    /// `disk_mask` subtract per entry whose target matches the back disk
    /// being shaded. Padded entries stay zero; the iteration bound
    /// guarantees they are never sampled.
    pub occluders: [[f32; 4]; MAX_OCCLUDERS * 2],
    /// V-51b active-occluder header: `x` = count
    /// (`<= MAX_OCCLUDERS` as an `f32`), `yzw` reserved.
    pub occluder_params: [f32; 4],
}

/// Lower bound of the V-51c totality smoothstep on obscuration.
///
/// The Baumbach 1937 corona term in `shaders/skyglow.wgsl` is gated by
/// `totality_weight`. Below ~97 % obscuration there is still a bright
/// crescent on the solar limb whose stray light would wash out the
/// inner corona, so the smoothstep starts at this floor.
const TOTALITY_ENVELOPE_LOW: f32 = 0.97;
/// Upper bound of the same smoothstep — deep totality
/// (Moon-larger-than-Sun core). `0.998` lifts the gate fully a few
/// arcseconds inside C2 / before C3, matching the timescale on which the
/// corona becomes naked-eye visible in the Mazatlán 2024-04-08
/// validation render.
const TOTALITY_ENVELOPE_HIGH: f32 = 0.998;

pub(crate) const HW_COEFFS_PER_CHANNEL: usize = 9;
pub(crate) type HosekWilkieCoefficientsUniform = [[f32; 4]; HW_COEFFS_PER_CHANNEL];

pub(crate) const PLANET_UNIFORM_COUNT: usize = 7;
pub(crate) type PlanetEqRadiusUniform = [[f32; 4]; PLANET_UNIFORM_COUNT];
pub(crate) type PlanetRgbMagnitudeUniform = [[f32; 4]; PLANET_UNIFORM_COUNT];

/// V-52b: number of Galilean moons rendered as point sources next to
/// Jupiter. Mirrored by the `galilean_eq_radius` array length in
/// `shaders/skyglow.wgsl` and `shaders/star.wgsl`.
pub(crate) const GALILEAN_UNIFORM_COUNT: usize = 4;
pub(crate) type GalileanEqRadiusUniform = [[f32; 4]; GALILEAN_UNIFORM_COUNT];
pub(crate) type GalileanRgbMagnitudeUniform = [[f32; 4]; GALILEAN_UNIFORM_COUNT];

/// V-52c: number of Saturnian moons rendered as point sources next to
/// Saturn. Only Titan (V ≈ 8.4 at opposition) ships in V-52c; the other
/// Meeus-supported Saturnian moons (Mimas / Enceladus / Tethys / Dione /
/// Rhea / Hyperion / Iapetus) are deferred to a follow-on rung because
/// their `V` magnitudes fall outside the renderer's default limiting
/// magnitude in most scene presets. The constant exists so the uniform
/// layout — a `vec4` direction-radius slot, a `vec4` rgb-magnitude slot,
/// and a `vec4` control header — can grow to include the other moons
/// without a uniform-block reshuffle.
pub(crate) const TITAN_UNIFORM_COUNT: usize = 1;

/// Cached renderer-facing planet uniforms. Computing VSOP87 planet states is
/// orders of magnitude more expensive than rebuilding the camera matrices, so
/// `Renderer` reuses this between coarse ephemeris refreshes while still
/// updating Sun/Moon, refraction, stars, and camera orientation every frame.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PlanetUniforms {
    pub eq_radius: PlanetEqRadiusUniform,
    pub rgb_magnitude: PlanetRgbMagnitudeUniform,
    pub params: [f32; 4],
    /// V-52a Saturn ring uniform block; mirrors
    /// [`CameraUniform::saturn_ring_pole_sinb`] / [`CameraUniform::saturn_ring_state`].
    pub saturn_ring_pole_sinb: [f32; 4],
    pub saturn_ring_state: [f32; 4],
    /// V-52b Galilean-moon uniform block; mirrors
    /// [`CameraUniform::galilean_eq_radius`] / [`CameraUniform::galilean_rgb_magnitude`]
    /// / [`CameraUniform::galilean_params`].
    pub galilean_eq_radius: GalileanEqRadiusUniform,
    pub galilean_rgb_magnitude: GalileanRgbMagnitudeUniform,
    pub galilean_params: [f32; 4],
    /// V-52c Titan uniform block; mirrors
    /// [`CameraUniform::titan_eq_radius`] / [`CameraUniform::titan_rgb_magnitude`]
    /// / [`CameraUniform::titan_params`].
    pub titan_eq_radius: [f32; 4],
    pub titan_rgb_magnitude: [f32; 4],
    pub titan_params: [f32; 4],
}

impl PlanetUniforms {
    pub const fn disabled() -> Self {
        Self {
            eq_radius: [[0.0; 4]; PLANET_UNIFORM_COUNT],
            rgb_magnitude: [[0.0; 4]; PLANET_UNIFORM_COUNT],
            params: [PLANET_UNIFORM_COUNT as f32, 0.0, 0.0, 0.0],
            saturn_ring_pole_sinb: [0.0; 4],
            saturn_ring_state: [0.0; 4],
            galilean_eq_radius: [[0.0; 4]; GALILEAN_UNIFORM_COUNT],
            galilean_rgb_magnitude: [[0.0; 4]; GALILEAN_UNIFORM_COUNT],
            galilean_params: [GALILEAN_UNIFORM_COUNT as f32, 0.0, 0.0, 0.0],
            titan_eq_radius: [0.0; 4],
            titan_rgb_magnitude: [0.0; 4],
            titan_params: [TITAN_UNIFORM_COUNT as f32, 0.0, 0.0, 0.0],
        }
    }
}

/// Named atmosphere presets shared by CLI, native viewer, and web hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtmospherePreset {
    ClearRural,
    HazyUrban,
    HighAltitude,
}

impl AtmospherePreset {
    pub const ALL: &'static [Self] = &[Self::ClearRural, Self::HazyUrban, Self::HighAltitude];

    pub const fn as_kebab_str(self) -> &'static str {
        match self {
            Self::ClearRural => "clear-rural",
            Self::HazyUrban => "hazy-urban",
            Self::HighAltitude => "high-altitude",
        }
    }

    pub fn from_kebab_str(s: &str) -> Option<Self> {
        match s {
            "clear-rural" => Some(Self::ClearRural),
            "hazy-urban" => Some(Self::HazyUrban),
            "high-altitude" => Some(Self::HighAltitude),
            _ => None,
        }
    }
}

/// Observer-local atmosphere state that the renderer applies to the star and
/// sky-background pipelines.
///
/// The canonical optical state is `(aerosol_beta, aerosol_alpha, ozone_du,
/// observer_altitude_m)` per V-37: Ångström aerosol turbidity (β = AOD at
/// 550 nm, α = wavelength exponent), total ozone column in Dobson units, and
/// observer elevation above sea level. Stellar atmospheric extinction and
/// daylight scattering both read this state through
/// [`astronomy::atmosphere`], so the two paths cannot disagree about how
/// reddened a given sky should be.
#[derive(Debug, Clone, Copy)]
pub struct Atmosphere {
    /// Ångström aerosol optical depth at 550 nm. Clean continental sites are
    /// ≈ 0.05; mid-quality observatories ≈ 0.10; hazy urban skies ≥ 0.30.
    /// This drives both stellar k(λ) and the daylight Mie aerosol term.
    pub aerosol_beta: f32,
    /// Ångström wavelength exponent. Continental aerosols sit near 1.3;
    /// coarser maritime / dust aerosols are around 0.8–1.0.
    pub aerosol_alpha: f32,
    /// Observer altitude above sea level in metres. Rayleigh and aerosol
    /// terms thin exponentially with the standard 8 km scale height.
    pub observer_altitude_m: f32,
    /// Total ozone column in Dobson units. Around 300 DU is a mid-latitude
    /// clear-sky default; larger values deepen blue zeniths and red sunsets
    /// through the Chappuis band.
    pub ozone_du: f32,
    /// Surface pressure in hPa for apparent-altitude refraction. Standard
    /// atmosphere is 1010 hPa; lower pressure reduces the horizon lift.
    pub pressure_hpa: f32,
    /// Air temperature in °C for apparent-altitude refraction. Saemundsson's
    /// correction scales as 283 K / (273 + T).
    pub temperature_c: f32,
    /// Ground albedo seen by the daylight sky model (V-38). Hošek-Wilkie
    /// uses this to lift the zenith when the ground is bright (snow,
    /// sand) or darken it when the ground is dark (forest, ocean).
    /// Default 0.10 ≈ mixed-vegetation continental terrain.
    pub surface_albedo: f32,
    /// Whether direct solar scattering is enabled. `Atmosphere::OFF` disables
    /// both extinction and daylight/twilight scattering.
    pub sunlit_scattering: bool,
}

impl Atmosphere {
    /// Clean sea-level dark site — the default model.
    ///
    /// `β = 0.10` matches Hardie 1962's mid-quality observatory V-band
    /// extinction within 0.03 mag/airmass; `α = 1.30` is the AERONET
    /// continental-aerosol mean.
    /// Default ground albedo for clear-rural / continental presets.
    /// AERONET / MODIS mid-latitude broadband albedo sits near 0.10
    /// across forest, cropland, and mixed grassland (Liang 2000 §4).
    pub const DEFAULT_SURFACE_ALBEDO: f32 = 0.10;

    pub const CLEAR_RURAL: Self = Self {
        aerosol_beta: 0.10,
        aerosol_alpha: 1.30,
        observer_altitude_m: 0.0,
        ozone_du: 300.0,
        pressure_hpa: 1010.0,
        temperature_c: 10.0,
        surface_albedo: Self::DEFAULT_SURFACE_ALBEDO,
        sunlit_scattering: true,
    };

    pub const HAZY_URBAN: Self = Self {
        aerosol_beta: 0.35,
        aerosol_alpha: 1.10,
        observer_altitude_m: 0.0,
        ozone_du: 325.0,
        pressure_hpa: 1010.0,
        temperature_c: 15.0,
        // Urban broadband albedo (asphalt + concrete + rooftops) sits
        // around 0.13 in Akbari & Levinson 2008.
        surface_albedo: 0.13,
        sunlit_scattering: true,
    };

    pub const HIGH_ALTITUDE: Self = Self {
        aerosol_beta: 0.04,
        aerosol_alpha: 1.30,
        observer_altitude_m: 2500.0,
        ozone_du: 275.0,
        pressure_hpa: 750.0,
        temperature_c: 0.0,
        // Snow / bare-rock observatory sites are brighter than 0.10;
        // 0.30 is a representative seasonal-average for Mauna-Kea-class
        // alpine sites (Sicart et al. 2001).
        surface_albedo: 0.30,
        sunlit_scattering: true,
    };

    pub const DEFAULT: Self = Self::CLEAR_RURAL;

    pub const fn from_preset(preset: AtmospherePreset) -> Self {
        match preset {
            AtmospherePreset::ClearRural => Self::CLEAR_RURAL,
            AtmospherePreset::HazyUrban => Self::HAZY_URBAN,
            AtmospherePreset::HighAltitude => Self::HIGH_ALTITUDE,
        }
    }

    /// No atmosphere — every star renders at its catalogue magnitude
    /// regardless of altitude, and no daylight/twilight scattering is added.
    /// Useful for debugging or for views from outside the Earth's atmosphere.
    pub const OFF: Self = Self {
        aerosol_beta: 0.0,
        aerosol_alpha: 1.30,
        observer_altitude_m: 0.0,
        ozone_du: 0.0,
        pressure_hpa: 0.0,
        temperature_c: 10.0,
        surface_albedo: 0.0,
        sunlit_scattering: false,
    };

    /// Broadband R/G/B extinction coefficients (mag per airmass) derived
    /// from the canonical (β, α, DU, h) state.
    ///
    /// Returns `[0; 3]` when the atmosphere is disabled so the renderer
    /// passes through unattenuated star fluxes.
    pub fn extinction_k_rgb(&self) -> [f32; 3] {
        if !self.sunlit_scattering {
            return [0.0; 3];
        }
        // Fully qualified to avoid the inherent-method name shadowing the
        // free function in bare-name lookup.
        let k = astronomy::atmosphere::extinction_k_rgb(
            self.observer_altitude_m as f64,
            self.aerosol_beta as f64,
            self.aerosol_alpha as f64,
            self.ozone_du as f64,
        );
        [k[0] as f32, k[1] as f32, k[2] as f32]
    }
}

impl Default for Atmosphere {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Per-frame atmospheric scintillation state (V-24).
///
/// The renderer reads this together with [`Atmosphere::observer_altitude_m`]
/// to derive a per-star intensity variance + temporal corner frequency, then
/// modulates the star shader's RGB output by a deterministic band-limited
/// noise field driven by `Observer::time.jd_ut1` so that the same JSON
/// session re-renders bit-for-bit identical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scintillation {
    /// Master enable. `false` matches a host that wants stars to render as
    /// constant point sources (debugging, external galactic viewpoint).
    pub enabled: bool,
    /// Dimensionless Cn² column scale. `1.0` reproduces the Dravins 1997
    /// amateur-site σ ≈ 4 % at the zenith for a 7 mm pupil at sea level;
    /// see [`astronomy::scintillation`] for the calibration.
    pub c_n2_scale: f32,
    /// Noise seed. Part of the session schema so two renders of the same
    /// session are bit-identical. `0` is allowed; the shader xors with a
    /// fixed constant before hashing.
    pub seed: u32,
}

impl Scintillation {
    /// Default: enabled, calibrated against the amateur-site median.
    pub const DEFAULT: Self = Self {
        enabled: true,
        c_n2_scale: astronomy::scintillation::DEFAULT_CN2_SCALE as f32,
        seed: 0x5C_15_71_07,
    };

    /// Zero σ², used for the external galactic viewpoint and for hosts that
    /// want every frame to be bit-deterministic without the time-varying
    /// modulation.
    pub const OFF: Self = Self {
        enabled: false,
        c_n2_scale: 0.0,
        seed: 0,
    };
}

impl Default for Scintillation {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Camera orientation expressed in the observer's local horizontal frame.
///
/// `azimuth_rad` is measured from North toward East. `altitude_rad` is measured
/// above the horizon (zero = horizon, +π/2 = zenith).
#[derive(Debug, Clone, Copy)]
pub struct LocalView {
    pub azimuth_rad: f32,
    pub altitude_rad: f32,
    pub fov_y_rad: f32,
}

impl Default for LocalView {
    fn default() -> Self {
        Self {
            azimuth_rad: 0.0,
            altitude_rad: 0.0,
            fov_y_rad: std::f32::consts::FRAC_PI_3,
        }
    }
}

/// Telescope optical train used to derive an eyepiece true field of view.
///
/// The model intentionally stays geometric: plate scale is `206264.806 /
/// focal_length_mm` arcsec/mm, magnification is OTA focal length divided by
/// eyepiece focal length, and the true field is either the eyepiece field-stop
/// angle or the apparent-field / magnification approximation when no physical
/// field stop is provided.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EyepieceSimulation {
    /// Whether the optical train should override [`LocalView::fov_y_rad`].
    pub enabled: bool,
    /// OTA clear aperture in millimetres. Used for exit-pupil reporting.
    pub aperture_mm: f32,
    /// OTA focal length in millimetres. Sets plate scale and true field.
    pub focal_length_mm: f32,
    /// Eyepiece focal length in millimetres.
    pub eyepiece_focal_length_mm: f32,
    /// Eyepiece apparent field of view in degrees, used when `field_stop_mm`
    /// is zero or non-finite.
    pub apparent_fov_deg: f32,
    /// Eyepiece field-stop diameter in millimetres. Values `<= 0` select the
    /// apparent-field / magnification estimate.
    pub field_stop_mm: f32,
}

impl EyepieceSimulation {
    pub const OFF: Self = Self {
        enabled: false,
        aperture_mm: 200.0,
        focal_length_mm: 2000.0,
        eyepiece_focal_length_mm: 25.0,
        apparent_fov_deg: 50.0,
        field_stop_mm: 21.0,
    };

    pub const DEFAULT_ENABLED: Self = Self {
        enabled: true,
        ..Self::OFF
    };

    fn finite_positive(value: f32, fallback: f32) -> f32 {
        if value.is_finite() && value > 0.0 {
            value
        } else {
            fallback
        }
    }

    fn sanitized(self) -> Self {
        Self {
            enabled: self.enabled,
            aperture_mm: Self::finite_positive(self.aperture_mm, Self::OFF.aperture_mm),
            focal_length_mm: Self::finite_positive(self.focal_length_mm, Self::OFF.focal_length_mm),
            eyepiece_focal_length_mm: Self::finite_positive(
                self.eyepiece_focal_length_mm,
                Self::OFF.eyepiece_focal_length_mm,
            ),
            apparent_fov_deg: Self::finite_positive(
                self.apparent_fov_deg,
                Self::OFF.apparent_fov_deg,
            ),
            field_stop_mm: if self.field_stop_mm.is_finite() {
                self.field_stop_mm.max(0.0)
            } else {
                Self::OFF.field_stop_mm
            },
        }
    }

    /// Plate scale at the OTA focal plane in arcseconds per millimetre.
    pub fn plate_scale_arcsec_per_mm(self) -> f32 {
        206_264.81 / self.sanitized().focal_length_mm
    }

    /// Eyepiece magnification (`OTA focal length / eyepiece focal length`).
    pub fn magnification(self) -> f32 {
        let s = self.sanitized();
        s.focal_length_mm / s.eyepiece_focal_length_mm
    }

    /// Exit-pupil diameter in millimetres.
    pub fn exit_pupil_mm(self) -> f32 {
        self.sanitized().aperture_mm / self.magnification()
    }

    /// Geometric true field of view in radians.
    pub fn true_field_rad(self) -> f32 {
        let s = self.sanitized();
        let field_rad = if s.field_stop_mm > 0.0 {
            2.0 * (s.field_stop_mm / (2.0 * s.focal_length_mm)).atan()
        } else {
            s.apparent_fov_deg.to_radians() / s.magnification()
        };
        field_rad.clamp(MIN_FOV_Y_RAD, MAX_FOV_Y_RAD)
    }

    /// Geometric true field of view in degrees.
    pub fn true_field_deg(self) -> f32 {
        self.true_field_rad().to_degrees()
    }
}

impl Default for EyepieceSimulation {
    fn default() -> Self {
        Self::OFF
    }
}

/// How close to the zenith / nadir the camera is allowed to tilt before the
/// clamp engages, in radians. The clamp exists because `Mat4::look_at_rh`
/// uses `forward × up` to build the right-axis: when `forward` aligns with
/// our world `up = +Z`, the cross product collapses and the view matrix
/// degenerates. The 0.01 rad (≈0.57°) gap keeps `|forward × up| ≳ 0.01`, which
/// glam normalises without precision loss while staying invisible at
/// reasonable FoVs. If you ever want to *look* at the zenith, switch to a
/// gimbal-lock-free representation (quaternion) rather than widening this.
const ALT_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.01;

/// Narrowest supported vertical field of view. Phase 4's eyepiece simulator
/// intentionally enters sub-degree fields, so the clamp is now set by
/// practical matrix / input sanity rather than naked-eye ergonomics.
const MIN_FOV_Y_RAD: f32 = 0.05 * std::f32::consts::PI / 180.0;
/// Widest supported vertical field of view. Larger values are better served by
/// a full-sky projection (ROADMAP Phase 4) rather than a perspective camera.
const MAX_FOV_Y_RAD: f32 = 120.0 * std::f32::consts::PI / 180.0;

fn full_sky_map_scale(aspect: f32) -> [f32; 2] {
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        1.0
    };
    if aspect >= 2.0 {
        [2.0 / aspect, 1.0]
    } else {
        [1.0, aspect / 2.0]
    }
}

fn mat3d_to_mat4(m: [[f64; 3]; 3]) -> Mat4 {
    Mat4::from_cols_array(&[
        m[0][0] as f32,
        m[1][0] as f32,
        m[2][0] as f32,
        0.0,
        m[0][1] as f32,
        m[1][1] as f32,
        m[2][1] as f32,
        0.0,
        m[0][2] as f32,
        m[1][2] as f32,
        m[2][2] as f32,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ])
}

fn refracted_local_direction(local: Vec3, pressure_hpa: f32, temperature_c: f32) -> Vec3 {
    let true_alt = local.z.clamp(-1.0, 1.0).asin();
    let alt_deg = true_alt.to_degrees();
    if !alt_deg.is_finite() || !(-1.0..=89.9).contains(&alt_deg) {
        return local.normalize_or_zero();
    }
    let pressure_scale = if pressure_hpa.is_finite() {
        pressure_hpa.max(0.0) / 1010.0
    } else {
        1.0
    };
    let temp_k = if temperature_c.is_finite() {
        (273.0 + temperature_c).max(150.0)
    } else {
        283.0
    };
    let weather_scale = pressure_scale * 283.0 / temp_k;
    let r_arcmin = 1.02 / ((alt_deg + 10.3 / (alt_deg + 5.11)).to_radians()).tan() * weather_scale;
    let apparent_alt = true_alt + (r_arcmin / 60.0).to_radians();
    let az = local.x.atan2(local.y);
    let cos_alt = apparent_alt.cos();
    Vec3::new(az.sin() * cos_alt, az.cos() * cos_alt, apparent_alt.sin()).normalize_or_zero()
}

fn apparent_disk_direction_j2000(
    direction_date: Vec3,
    refract: bool,
    pressure_hpa: f32,
    temperature_c: f32,
    date_to_local: Mat4,
    local_to_date: Mat4,
    date_to_j2000: Mat4,
) -> Vec3 {
    let direction_date = if refract {
        let local = (date_to_local * direction_date.extend(0.0)).truncate();
        let refracted = refracted_local_direction(local, pressure_hpa, temperature_c);
        (local_to_date * refracted.extend(0.0)).truncate()
    } else {
        direction_date
    };
    (date_to_j2000 * direction_date.extend(0.0))
        .truncate()
        .normalize_or_zero()
}

fn finite_vec3(value: [f32; 3], fallback: [f32; 3]) -> Vec3 {
    if value.iter().all(|v| v.is_finite()) {
        Vec3::from_array(value)
    } else {
        Vec3::from_array(fallback)
    }
}

fn planet_linear_rgb(planet: Planet) -> [f32; 3] {
    match planet {
        Planet::Mercury => [0.86, 0.78, 0.68],
        Planet::Venus => [1.00, 0.91, 0.70],
        Planet::Mars => [1.00, 0.54, 0.34],
        Planet::Jupiter => [0.95, 0.82, 0.62],
        Planet::Saturn => [0.92, 0.80, 0.55],
        Planet::Uranus => [0.64, 0.88, 1.00],
        Planet::Neptune => [0.45, 0.62, 1.00],
    }
}

/// V-52b Galilean-moon display colour in linear RGB. Surface colours follow
/// the standard amateur-imaging tints (Io: sulfur yellow; Europa: water-ice
/// white; Ganymede: tan-grey; Callisto: dark grey-tan), softened toward the
/// Sun-illumination colour so the moons sit next to Jupiter without an
/// implausible chroma jump in the eyepiece.
fn galilean_linear_rgb(moon: GalileanMoon) -> [f32; 3] {
    match moon {
        GalileanMoon::Io => [1.00, 0.86, 0.55],
        GalileanMoon::Europa => [0.95, 0.93, 0.88],
        GalileanMoon::Ganymede => [0.90, 0.84, 0.74],
        GalileanMoon::Callisto => [0.78, 0.74, 0.68],
    }
}

/// V-52c Titan display colour in linear RGB. Titan's dense N₂-CH₄ haze
/// gives it a pronounced orange-brown tint in the eyepiece (Karkoschka
/// 1998, Cassini/ISS photometry). The colour here is softened toward the
/// Sun-illumination chroma the same way the Galilean tints are softened
/// against Jupiter, so Titan sits next to Saturn without an implausible
/// brightness jump.
fn titan_linear_rgb() -> [f32; 3] {
    [0.96, 0.78, 0.50]
}

impl LocalView {
    /// Return a finite, renderer-safe view.
    ///
    /// Hosts may construct `LocalView` directly (CLI flags, WASM bindings,
    /// tests), so the renderer cannot rely on only the interactive helpers to
    /// clamp it. This keeps `look_at_rh` away from the zenith/nadir gimbal-lock
    /// singularity and keeps `perspective_rh` away from zero/NaN FOVs.
    pub fn clamped(self) -> Self {
        let default = Self::default();
        let azimuth_rad = if self.azimuth_rad.is_finite() {
            self.azimuth_rad.rem_euclid(std::f32::consts::TAU)
        } else {
            default.azimuth_rad
        };
        let altitude_rad = if self.altitude_rad.is_finite() {
            self.altitude_rad.clamp(-ALT_LIMIT, ALT_LIMIT)
        } else {
            default.altitude_rad
        };
        let fov_y_rad = if self.fov_y_rad.is_finite() {
            self.fov_y_rad.clamp(MIN_FOV_Y_RAD, MAX_FOV_Y_RAD)
        } else {
            default.fov_y_rad
        };
        Self {
            azimuth_rad,
            altitude_rad,
            fov_y_rad,
        }
    }
}

pub struct Camera {
    pub observer: Observer,
    pub view: LocalView,
    pub aspect: f32,
    pub atmosphere: Atmosphere,
    /// Projection model. Perspective uses [`LocalView::fov_y_rad`]; the
    /// all-sky projections ignore FoV but keep azimuth/altitude as the map
    /// centre so users can rotate the seam away from the structure of interest.
    pub projection: SkyProjection,
    /// Camera location. [`SkyViewpoint::Earth`] keeps the historical sky-dome
    /// path; external modes render a parsec-scale Milky Way disc map from
    /// outside Earth.
    pub viewpoint: SkyViewpoint,
    /// Origin/orientation used when [`SkyViewpoint::CustomExternal`] is active.
    /// Values are IAU galactic Cartesian parsecs; see [`ExternalViewpoint`].
    pub external_viewpoint: ExternalViewpoint,
    /// Whether Mercury through Neptune are rendered as apparent solar-system
    /// disks/points from the VSOP87 ephemeris.
    pub planets_enabled: bool,
    /// Telescope/eyepiece optical train. When enabled for the Earth-centred
    /// perspective view, its true field of view overrides [`LocalView::fov_y_rad`]
    /// while keeping the same azimuth/altitude pointing.
    pub eyepiece: EyepieceSimulation,
    /// Atmospheric scintillation (V-24). Disabled automatically when the
    /// renderer is rendering an external galactic viewpoint.
    pub scintillation: Scintillation,
    /// Faintest magnitude the simulated observer should be able to detect.
    /// Anchors the linear-flux brightness scale used by both the star pass
    /// and the skyglow surface-brightness pass; see
    /// `vertex::magnitude_to_render_params` for the formula. Hosts that
    /// want to render a more-or-less-sensitive observer should set this
    /// alongside the field of the same name they pass to
    /// `build_star_instance`.
    pub limiting_magnitude: f32,
}

impl Camera {
    pub fn new(observer: Observer, view: LocalView, aspect: f32) -> Self {
        Self {
            observer,
            view: view.clamped(),
            aspect,
            atmosphere: Atmosphere::default(),
            projection: SkyProjection::default(),
            viewpoint: SkyViewpoint::default(),
            external_viewpoint: ExternalViewpoint::default(),
            planets_enabled: true,
            eyepiece: EyepieceSimulation::default(),
            scintillation: Scintillation::default(),
            limiting_magnitude: NAKED_EYE_LIMITING_MAGNITUDE,
        }
    }

    /// Rotation that maps a true equator/equinox-of-date direction into the
    /// observer's local ENU frame. Nutation contributes the equation of the
    /// equinoxes, so local apparent sidereal time is used here rather than the
    /// mean sidereal angle used by the Phase-1 J2000-only pipeline.
    fn equatorial_to_horizontal(&self) -> Mat4 {
        let gast = lmst_radians(self.observer.time.jd_ut1, self.observer.longitude_rad)
            + equation_of_equinoxes(self.observer.time.jd_tt);
        equatorial_to_horizontal_matrix(self.observer.latitude_rad, gast)
    }

    fn j2000_to_date(&self) -> Mat4 {
        mat3d_to_mat4(precession_nutation_matrix(self.observer.time.jd_tt))
    }

    fn effective_view(&self) -> LocalView {
        let mut view = self.view.clamped();
        if self.eyepiece.enabled && !self.viewpoint.is_external() && !self.projection.is_full_sky()
        {
            view.fov_y_rad = self.eyepiece.true_field_rad();
        }
        view
    }

    /// Vertical field of view actually used for perspective rendering.
    ///
    /// Interactive hosts use this to keep drag sensitivity matched to the
    /// visible field when eyepiece simulation overrides the free FoV slider.
    pub fn effective_fov_y_rad(&self) -> f32 {
        self.effective_view().fov_y_rad
    }

    /// Forward direction (in local ENU) the camera is looking at.
    fn forward_local(&self) -> Vec3 {
        let view = self.effective_view();
        let (sa, ca) = view.azimuth_rad.sin_cos();
        let (sp, cp) = view.altitude_rad.sin_cos();
        Vec3::new(sa * cp, ca * cp, sp)
    }

    /// View matrix in the observer's local ENU frame (no equatorial→horizontal rotation).
    /// Use this for geometry that is naturally expressed in local coordinates
    /// (horizon line, alt-az grid, cardinal direction markers).
    fn view_matrix_local(&self) -> Mat4 {
        // look_at_rh derives screen-right from (forward × up); using local zenith as
        // "up" keeps the horizon level on screen. `forward_local` uses a clamped
        // view so host-supplied ±90° altitudes cannot hit the gimbal-lock singularity.
        Mat4::look_at_rh(Vec3::ZERO, self.forward_local(), Vec3::Z)
    }

    /// View matrix in J2000 equatorial coordinates (includes the equatorial→horizontal
    /// rotation). Use this for star positions, RA/Dec grids, ecliptic, celestial equator.
    fn view_matrix(&self) -> Mat4 {
        self.view_matrix_local() * self.equatorial_to_horizontal() * self.j2000_to_date()
    }

    /// View matrix for the external Milky Way map. Coordinates are parsecs in
    /// the IAU galactic frame: the Sun is at the origin, +X points to l=0°,
    /// +Y to l=90°, and +Z to the north galactic pole.
    fn external_view_matrix(&self) -> Mat4 {
        let viewpoint = self.active_external_viewpoint();
        let eye = finite_vec3(
            viewpoint.origin_pc,
            ExternalViewpoint::GALACTIC_NORTH.origin_pc,
        );
        let mut target = finite_vec3(
            viewpoint.target_pc,
            ExternalViewpoint::GALACTIC_NORTH.target_pc,
        );
        if (target - eye).length_squared() < 1.0e-6 {
            target = if eye.length_squared() > 1.0e-6 {
                Vec3::ZERO
            } else {
                Vec3::Z
            };
        }
        let forward = (target - eye).normalize();
        let mut up = finite_vec3(viewpoint.up, ExternalViewpoint::GALACTIC_NORTH.up);
        if up.length_squared() < 1.0e-6 || forward.cross(up).length_squared() < 1.0e-6 {
            up = if forward.cross(Vec3::Y).length_squared() > 1.0e-6 {
                Vec3::Y
            } else {
                Vec3::Z
            };
        }
        Mat4::look_at_rh(eye, target, up.normalize())
    }

    fn projection_matrix(&self) -> Mat4 {
        let far = if self.viewpoint.is_external() {
            let viewpoint = self.active_external_viewpoint();
            let origin = finite_vec3(
                viewpoint.origin_pc,
                ExternalViewpoint::GALACTIC_NORTH.origin_pc,
            );
            let target = finite_vec3(
                viewpoint.target_pc,
                ExternalViewpoint::GALACTIC_NORTH.target_pc,
            );
            (origin.length().max((origin - target).length()) + EXTERNAL_SCENE_RADIUS_PC)
                .max(GALACTIC_CAMERA_HEIGHT_PC * 3.0)
        } else {
            10.0
        };
        Mat4::perspective_rh(self.effective_view().fov_y_rad, self.aspect, 0.01, far)
    }

    fn active_external_viewpoint(&self) -> ExternalViewpoint {
        match self.viewpoint {
            SkyViewpoint::Earth | SkyViewpoint::GalacticNorth => ExternalViewpoint::GALACTIC_NORTH,
            SkyViewpoint::CustomExternal => self.external_viewpoint,
        }
    }

    fn external_eye_pc(&self) -> Vec3 {
        finite_vec3(
            self.active_external_viewpoint().origin_pc,
            ExternalViewpoint::GALACTIC_NORTH.origin_pc,
        )
    }

    /// NDC half-extents that fit a natural 2:1 all-sky map ellipse into the
    /// current viewport without stretching it.
    fn full_sky_map_scale(&self) -> [f32; 2] {
        full_sky_map_scale(self.aspect)
    }

    fn projection_params(&self) -> [f32; 4] {
        let [sx, sy] = self.full_sky_map_scale();
        [
            self.projection.shader_mode(),
            sx,
            sy,
            if self.projection.is_full_sky() {
                1.0
            } else {
                0.0
            },
        ]
    }

    fn viewpoint_params(&self) -> [f32; 4] {
        let eye = self.external_eye_pc();
        [self.viewpoint.shader_mode(), eye.x, eye.y, eye.z]
    }

    /// Matrix stored in `CameraUniform::view_proj`.
    ///
    /// Perspective mode keeps the historical view-projection matrix. All-sky
    /// projections are non-linear, so the shader needs the pre-projection view
    /// matrix and performs the spherical map itself.
    fn shader_view_proj(&self) -> Mat4 {
        if self.viewpoint.is_external() {
            return self.view_proj();
        }
        if self.projection.is_full_sky() {
            self.view_matrix()
        } else {
            self.view_proj()
        }
    }

    fn shader_view_proj_local(&self) -> Mat4 {
        if self.viewpoint.is_external() {
            return self.view_proj();
        }
        if self.projection.is_full_sky() {
            self.view_matrix_local()
        } else {
            self.view_proj_local()
        }
    }

    fn shader_inv_view_proj(&self) -> Mat4 {
        self.shader_view_proj().inverse()
    }

    /// View-projection for J2000 equatorial-frame geometry. Alias kept for backward compat.
    pub fn view_proj(&self) -> Mat4 {
        if self.viewpoint.is_external() {
            self.projection_matrix() * self.external_view_matrix()
        } else {
            self.projection_matrix() * self.view_matrix()
        }
    }

    /// View-projection for local ENU-frame geometry.
    pub(crate) fn view_proj_local(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix_local()
    }

    pub(crate) fn overlay_matrix_equatorial(&self) -> Mat4 {
        self.shader_view_proj()
    }

    pub(crate) fn overlay_matrix_local(&self) -> Mat4 {
        self.shader_view_proj_local()
    }

    pub(crate) fn overlay_projection_params(&self) -> [f32; 4] {
        if self.viewpoint.is_external() {
            // Earth-local reference overlays do not have a meaningful position
            // in the parsec-scale galactic map. A negative mode tells overlay
            // and label projectors to skip their geometry for this viewpoint.
            [-1.0, 0.0, 0.0, -1.0]
        } else {
            self.projection_params()
        }
    }

    /// Local zenith direction expressed in J2000 equatorial coordinates.
    ///
    /// The local up vector is `(0, 0, 1)` in ENU; the transpose of the
    /// equatorial→ENU matrix maps it back to equatorial. We expose this so
    /// the shader can compute each star's altitude on the GPU without
    /// re-deriving the matrix per-instance.
    fn zenith_in_equatorial(&self) -> Vec3 {
        // ENU→Eq is the inverse of Eq→ENU. The matrix is orthonormal so
        // the inverse is the transpose. Multiplying the transpose by
        // (0, 0, 1) yields the third *column* of the transpose, which is
        // the third *row* of the original Eq→ENU matrix — i.e. the "Up"
        // basis vector expressed in equatorial coords.
        let eq_to_enu = self.equatorial_to_horizontal() * self.j2000_to_date();
        let m = eq_to_enu.to_cols_array_2d();
        // Third row of `m`: take z-component of each column basis vector.
        Vec3::new(m[0][2], m[1][2], m[2][2])
    }

    /// Approximate solid angle subtended by one pixel of the framebuffer,
    /// in steradians. Assumes a square pixel and small-angle behaviour at
    /// the centre of the viewport (the value is constant across the frame
    /// in this approximation, which is good enough for naked-eye-scale
    /// FoVs; wide-FoV edge fall-off would need a per-fragment computation,
    /// scoped for a future PR).
    fn pixel_solid_angle_sr(&self, width_pixels: u32, height_pixels: u32) -> f32 {
        if self.viewpoint.is_external() {
            let pixel_size_rad = self.effective_view().fov_y_rad / height_pixels.max(1) as f32;
            return pixel_size_rad * pixel_size_rad;
        }
        if self.projection.is_full_sky() {
            let [sx, sy] = self.full_sky_map_scale();
            let width = width_pixels.max(1) as f32;
            let height = height_pixels.max(1) as f32;
            // The all-sky maps cover 4π sr inside an ellipse with half-axes
            // sx·W/2 and sy·H/2 pixels, so the average pixel solid angle is
            // 4π / (π·sx·W/2·sy·H/2).
            16.0 / (sx.max(1e-6) * sy.max(1e-6) * width * height)
        } else {
            let pixel_size_rad = self.effective_view().fov_y_rad / height_pixels.max(1) as f32;
            pixel_size_rad * pixel_size_rad
        }
    }

    pub(crate) fn planet_uniforms(&self) -> PlanetUniforms {
        if !self.planets_enabled {
            return PlanetUniforms::disabled();
        }
        let pressure_hpa = if self.atmosphere.pressure_hpa.is_finite() {
            self.atmosphere.pressure_hpa.clamp(0.0, 1100.0)
        } else {
            Atmosphere::DEFAULT.pressure_hpa
        };
        let temperature_c = if self.atmosphere.temperature_c.is_finite() {
            self.atmosphere.temperature_c.clamp(-80.0, 60.0)
        } else {
            Atmosphere::DEFAULT.temperature_c
        };
        let j2000_to_date = self.j2000_to_date();
        let date_to_j2000 = j2000_to_date.transpose();
        let date_to_local = self.equatorial_to_horizontal();
        let local_to_date = date_to_local.transpose();

        let mut eq_radius = [[0.0; 4]; PLANET_UNIFORM_COUNT];
        let mut rgb_magnitude = [[0.0; 4]; PLANET_UNIFORM_COUNT];
        for (idx, planet) in apparent_planets_topocentric(self.observer)
            .iter()
            .enumerate()
        {
            let dir = apparent_disk_direction_j2000(
                planet.direction_equatorial(),
                self.atmosphere.sunlit_scattering,
                pressure_hpa,
                temperature_c,
                date_to_local,
                local_to_date,
                date_to_j2000,
            );
            let rgb = planet_linear_rgb(planet.planet);
            eq_radius[idx] = [dir.x, dir.y, dir.z, planet.angular_radius_rad as f32];
            rgb_magnitude[idx] = [rgb[0], rgb[1], rgb[2], planet.magnitude as f32];
        }

        // V-52a Saturn ring orientation. Rotate the ring pole from the
        // equatorial-of-date frame `apparent_saturn_ring` returns into the same
        // J2000 frame the shader sees `planet_eq_radius` in. Refraction is not
        // re-applied to the pole: it is a small (~30′) almost-radial shift at
        // the horizon, leaving the ring pole's orientation relative to Saturn's
        // centre invariant at the V-52a accuracy budget.
        let ring = apparent_saturn_ring_topocentric(self.observer);
        let pole_j2000 = (date_to_j2000 * ring.ring_pole_eq.extend(0.0))
            .truncate()
            .normalize_or_zero();
        let saturn_ring_pole_sinb = [
            pole_j2000.x,
            pole_j2000.y,
            pole_j2000.z,
            ring.sub_earth_lat_rad.sin() as f32,
        ];
        let saturn_ring_state = [ring.sub_sun_lat_rad.sin() as f32, 1.0, 0.0, 0.0];

        // V-52b Galilean moons. Same `apparent_disk_direction_j2000` pipeline
        // as the planets so refraction near the horizon stays consistent with
        // Jupiter, and so the shader-side direction lands in the J2000 frame
        // the camera uniform already publishes.
        let mut galilean_eq_radius = [[0.0; 4]; GALILEAN_UNIFORM_COUNT];
        let mut galilean_rgb_magnitude = [[0.0; 4]; GALILEAN_UNIFORM_COUNT];
        for (idx, moon) in apparent_galilean_moons_topocentric(self.observer)
            .iter()
            .enumerate()
        {
            let dir = apparent_disk_direction_j2000(
                moon.direction_equatorial(),
                self.atmosphere.sunlit_scattering,
                pressure_hpa,
                temperature_c,
                date_to_local,
                local_to_date,
                date_to_j2000,
            );
            let rgb = galilean_linear_rgb(moon.moon);
            galilean_eq_radius[idx] = [dir.x, dir.y, dir.z, moon.angular_radius_rad as f32];
            galilean_rgb_magnitude[idx] = [rgb[0], rgb[1], rgb[2], moon.magnitude as f32];
        }
        let galilean_params = [GALILEAN_UNIFORM_COUNT as f32, 1.0, 0.0, 0.0];

        // V-52c Titan. Same `apparent_disk_direction_j2000` pipeline as the
        // planets / Galilean moons so refraction near the horizon stays
        // consistent with Saturn, and so the shader-side direction lands in
        // the J2000 frame the camera uniform already publishes.
        let titan = apparent_titan_topocentric(self.observer);
        let titan_dir = apparent_disk_direction_j2000(
            titan.direction_equatorial(),
            self.atmosphere.sunlit_scattering,
            pressure_hpa,
            temperature_c,
            date_to_local,
            local_to_date,
            date_to_j2000,
        );
        let titan_rgb = titan_linear_rgb();
        let titan_eq_radius = [
            titan_dir.x,
            titan_dir.y,
            titan_dir.z,
            titan.angular_radius_rad as f32,
        ];
        let titan_rgb_magnitude = [
            titan_rgb[0],
            titan_rgb[1],
            titan_rgb[2],
            titan.magnitude as f32,
        ];
        let titan_params = [TITAN_UNIFORM_COUNT as f32, 1.0, 0.0, 0.0];

        PlanetUniforms {
            eq_radius,
            rgb_magnitude,
            params: [PLANET_UNIFORM_COUNT as f32, 1.0, 0.0, 0.0],
            saturn_ring_pole_sinb,
            saturn_ring_state,
            galilean_eq_radius,
            galilean_rgb_magnitude,
            galilean_params,
            titan_eq_radius,
            titan_rgb_magnitude,
            titan_params,
        }
    }

    pub(crate) fn uniform_with_planets(
        &self,
        width: u32,
        height: u32,
        planet_uniforms: &PlanetUniforms,
    ) -> CameraUniform {
        let zenith = self.zenith_in_equatorial();
        let observer_altitude_m = if self.atmosphere.observer_altitude_m.is_finite() {
            self.atmosphere.observer_altitude_m.max(0.0)
        } else {
            Atmosphere::DEFAULT.observer_altitude_m
        };
        let aerosol_beta = if self.atmosphere.aerosol_beta.is_finite() {
            self.atmosphere.aerosol_beta.clamp(0.0, 2.0)
        } else {
            Atmosphere::DEFAULT.aerosol_beta
        };
        let aerosol_alpha = if self.atmosphere.aerosol_alpha.is_finite() {
            self.atmosphere.aerosol_alpha.clamp(0.0, 4.0)
        } else {
            Atmosphere::DEFAULT.aerosol_alpha
        };
        let ozone_du = if self.atmosphere.ozone_du.is_finite() {
            self.atmosphere.ozone_du.clamp(0.0, 600.0)
        } else {
            Atmosphere::DEFAULT.ozone_du
        };
        let k = if self.atmosphere.sunlit_scattering {
            let k64 = astronomy::atmosphere::extinction_k_rgb(
                observer_altitude_m as f64,
                aerosol_beta as f64,
                aerosol_alpha as f64,
                ozone_du as f64,
            );
            [k64[0] as f32, k64[1] as f32, k64[2] as f32]
        } else {
            [0.0; 3]
        };
        let turbidity_eff = if self.atmosphere.sunlit_scattering {
            astronomy::atmosphere::linke_turbidity_from_aerosol(aerosol_beta as f64) as f32
        } else {
            0.0
        };
        let pressure_hpa = if self.atmosphere.pressure_hpa.is_finite() {
            self.atmosphere.pressure_hpa.clamp(0.0, 1100.0)
        } else {
            Atmosphere::DEFAULT.pressure_hpa
        };
        let temperature_c = if self.atmosphere.temperature_c.is_finite() {
            self.atmosphere.temperature_c.clamp(-80.0, 60.0)
        } else {
            Atmosphere::DEFAULT.temperature_c
        };
        let j2000_to_date = self.j2000_to_date();
        let date_to_j2000 = j2000_to_date.transpose();
        let date_to_local = self.equatorial_to_horizontal();
        let local_to_date = date_to_local.transpose();
        let disks = SunMoonApparent::for_observer(self.observer);
        let sun = disks.sun;
        let sun_dir_date = sun.direction_equatorial();
        let sun_dir = apparent_disk_direction_j2000(
            sun_dir_date,
            self.atmosphere.sunlit_scattering,
            pressure_hpa,
            temperature_c,
            date_to_local,
            local_to_date,
            date_to_j2000,
        );
        let solar_lux = illuminants::solar_illuminance_lux(sun.distance_au) as f32;
        let solar_rgb = illuminants::SOLAR_LINEAR_RGB;
        let moon = disks.moon;
        let moon_dir_date = moon.direction_equatorial();
        let moon_dir = apparent_disk_direction_j2000(
            moon_dir_date,
            self.atmosphere.sunlit_scattering,
            pressure_hpa,
            temperature_c,
            date_to_local,
            local_to_date,
            date_to_j2000,
        );
        let moon_lux = illuminants::lunar_illuminance_lux(
            moon.illuminated_fraction,
            moon.distance_km,
            moon.phase_angle_rad,
        ) as f32;
        // V-38: pre-cook the Hošek-Wilkie nine-parameter (A..I) coefficients
        // and per-channel radiance scales for this frame's (turbidity,
        // albedo, sun elevation) configuration. `cook` returns the all-zero
        // sentinel for sun below the horizon so the shader can stay
        // branch-free.
        let scattering_enabled = if self.atmosphere.sunlit_scattering {
            1.0_f32
        } else {
            0.0
        };
        let surface_albedo = if self.atmosphere.surface_albedo.is_finite() {
            self.atmosphere.surface_albedo.clamp(0.0, 1.0)
        } else {
            Atmosphere::DEFAULT_SURFACE_ALBEDO
        };
        let (hw_coeffs, hw_radiance) = if self.atmosphere.sunlit_scattering {
            // Apparent (refraction-corrected) solar altitude in the
            // observer's local ENU frame. The shader gates the HW daylight
            // branch on the same refracted `sun_altitude_rad()` it reads
            // from `sun_eq_radius`, so cooking the coefficients against the
            // geometric altitude here would leave a multi-arcminute band
            // where the host emits the zero sentinel but the shader still
            // evaluates the HW branch — the dark frame the user saw right
            // after sunset.
            let sun_enu = date_to_local.transform_vector3(j2000_to_date.transform_vector3(sun_dir));
            let sun_alt = (sun_enu.z as f64).clamp(-1.0, 1.0).asin();
            let params = astronomy::atmosphere::hosek_wilkie::cook(
                turbidity_eff as f64,
                surface_albedo as f64,
                sun_alt,
            );
            let mut coeffs = [[0.0_f32; 4]; HW_COEFFS_PER_CHANNEL];
            for (coeff_idx, slot) in coeffs.iter_mut().enumerate() {
                *slot = [
                    params.coeffs[0][coeff_idx] as f32,
                    params.coeffs[1][coeff_idx] as f32,
                    params.coeffs[2][coeff_idx] as f32,
                    0.0,
                ];
            }
            let radiance = [
                params.radiances[0] as f32,
                params.radiances[1] as f32,
                params.radiances[2] as f32,
                0.0,
            ];
            (coeffs, radiance)
        } else {
            ([[0.0; 4]; HW_COEFFS_PER_CHANNEL], [0.0; 4])
        };
        let view_proj = self.shader_view_proj();
        let inv_view_proj = self.shader_inv_view_proj();
        let eq_to_local = self.equatorial_to_horizontal();
        let view_proj_local = self.shader_view_proj_local();
        let earth_velocity = earth_velocity_over_c_j2000(self.observer.time.jd_tdb);
        // V-24 scintillation: derive zenith σ² + corner from astronomy,
        // disable for the external galactic viewpoint, and drive the noise
        // phase from `Observer.time.jd_ut1` so re-renders of the same
        // session are bit-identical. The shader divides this by airmass
        // per-star to get the altitude-dependent σ².
        let scintillation_params = if self.scintillation.enabled
            && self.atmosphere.sunlit_scattering
            && !self.viewpoint.is_external()
            && self.scintillation.c_n2_scale.is_finite()
            && self.scintillation.c_n2_scale > 0.0
        {
            let (sigma_sq_zenith, corner_hz) = astronomy::scintillation::intensity_variance(
                std::f64::consts::FRAC_PI_2,
                observer_altitude_m as f64,
                astronomy::scintillation::DEFAULT_PUPIL_MM,
                self.scintillation.c_n2_scale as f64,
            );
            // Wrap to a UT1 day so the f32 phase keeps sub-frame precision
            // (86400 s ≈ 17 bits of mantissa, leaving ~7 bits for sub-second
            // resolution — plenty for a ~25 Hz noise field).
            let day_fraction = self.observer.time.jd_ut1.rem_euclid(1.0);
            let t_seconds = (day_fraction * 86_400.0) as f32;
            let seed_bits = self.scintillation.seed;
            [
                sigma_sq_zenith as f32,
                corner_hz as f32,
                f32::from_bits(seed_bits),
                t_seconds,
            ]
        } else {
            [
                0.0,
                astronomy::scintillation::CORNER_HZ_ZENITH as f32,
                0.0,
                0.0,
            ]
        };
        // V-51c: classify the Moon-occults-Sun apparent-disk geometry for
        // this frame. The renderer disables the analytic-mask path on
        // external galactic viewpoints (no atmosphere, no apparent disks)
        // and on `Atmosphere::OFF` (which already turns daylight off, so
        // there is nothing to darken). The Koomen 1952 daylight falloff is
        // applied later inside the skyglow shader via this uniform.
        // V-51b analytic-mask occluder uniform. Populated alongside the
        // V-51c Sun-specific photometric falloff so the two paths cannot
        // drift: the same predicate (`sunlit_scattering && !external`)
        // gates both. Off-eclipse and on external viewpoints the list is
        // empty and the shader short-circuits on `count == 0`.
        //
        // `active_occluders` returns date-of-epoch equatorial directions
        // without atmospheric refraction; the renderer's star, Sun, and
        // Moon disks live in J2000 equatorial after a Saemundsson
        // refraction pass, so we map each front-disk direction through
        // the same `apparent_disk_direction_j2000` pipeline. Skipping
        // this step misaligns the analytic mask by the refraction lift
        // (~0.5° near the horizon, ~tens of arcsec near zenith) and
        // breaks bit-parity with the V-51c golden frame.
        let mut occluders_uniform = [[0.0_f32; 4]; MAX_OCCLUDERS * 2];
        let mut occluder_count: u32 = 0;
        if self.atmosphere.sunlit_scattering && !self.viewpoint.is_external() {
            let list = active_occluders(self.observer);
            for (i, occ) in list.as_slice().iter().enumerate() {
                let dir_date = Vec3::new(
                    occ.front_dir_eq[0] as f32,
                    occ.front_dir_eq[1] as f32,
                    occ.front_dir_eq[2] as f32,
                );
                let dir_j2000 = apparent_disk_direction_j2000(
                    dir_date,
                    self.atmosphere.sunlit_scattering,
                    pressure_hpa,
                    temperature_c,
                    date_to_local,
                    local_to_date,
                    date_to_j2000,
                );
                occluders_uniform[2 * i] = [
                    dir_j2000.x,
                    dir_j2000.y,
                    dir_j2000.z,
                    occ.front_radius_rad as f32,
                ];
                occluders_uniform[2 * i + 1] = [
                    occ.target.shader_code() as f32,
                    occ.kind.shader_code(),
                    occ.obscuration as f32,
                    0.0,
                ];
            }
            occluder_count = list.len() as u32;
        }
        let occluder_params_uniform = [occluder_count as f32, 0.0, 0.0, 0.0];

        let solar_eclipse_state_uniform =
            if self.atmosphere.sunlit_scattering && !self.viewpoint.is_external() {
                let state = solar_eclipse_state(self.observer);
                let totality_weight = if matches!(state.kind, SolarEclipseKind::Total) {
                    // smoothstep(TOTALITY_ENVELOPE_LOW, TOTALITY_ENVELOPE_HIGH, obs):
                    // the totality envelope only turns the corona on inside the
                    // Moon-larger-than-Sun core, not during deep partial phases
                    // that still leave a bright crescent.
                    let span = TOTALITY_ENVELOPE_HIGH - TOTALITY_ENVELOPE_LOW;
                    let t = ((state.obscuration - TOTALITY_ENVELOPE_LOW) / span).clamp(0.0, 1.0);
                    t * t * (3.0 - 2.0 * t)
                } else {
                    0.0
                };
                let partial_weight = if matches!(state.kind, SolarEclipseKind::None) {
                    0.0
                } else {
                    state.obscuration
                };
                [
                    state.kind.shader_code(),
                    state.obscuration,
                    totality_weight,
                    partial_weight,
                ]
            } else {
                [0.0, 0.0, 0.0, 0.0]
            };
        CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            eq_to_local: eq_to_local.to_cols_array_2d(),
            view_proj_local: view_proj_local.to_cols_array_2d(),
            j2000_to_date: j2000_to_date.to_cols_array_2d(),
            aberration_pm: [
                earth_velocity[0] as f32,
                earth_velocity[1] as f32,
                earth_velocity[2] as f32,
                years_since_j2000(self.observer.time.jd_tt) as f32,
            ],
            refraction_params: [pressure_hpa, temperature_c, 0.0, 0.0],
            viewport_pixel_sr_zeropoint: [
                width as f32,
                height as f32,
                self.pixel_solid_angle_sr(width, height),
                limiting_magnitude_to_zeropoint(self.limiting_magnitude),
            ],
            zenith_eq: [zenith.x, zenith.y, zenith.z, 0.0],
            extinction_k_rgb: [k[0], k[1], k[2], 0.0],
            sun_eq_radius: [
                sun_dir.x,
                sun_dir.y,
                sun_dir.z,
                sun.angular_radius_rad as f32,
            ],
            atmosphere_params: [
                turbidity_eff,
                observer_altitude_m,
                solar_lux,
                scattering_enabled,
            ],
            solar_rgb: [
                solar_rgb[0] as f32,
                solar_rgb[1] as f32,
                solar_rgb[2] as f32,
                0.0,
            ],
            atmosphere_optics: [ozone_du, aerosol_beta, aerosol_alpha, 0.0],
            moon_eq_illuminance: [moon_dir.x, moon_dir.y, moon_dir.z, moon_lux],
            moon_disk: [
                moon.angular_radius_rad as f32,
                moon.illuminated_fraction as f32,
                moon.phase_angle_rad as f32,
                moon.earth_shadow_fraction as f32,
            ],
            projection_params: self.projection_params(),
            viewpoint_params: self.viewpoint_params(),
            planet_eq_radius: planet_uniforms.eq_radius,
            planet_rgb_magnitude: planet_uniforms.rgb_magnitude,
            planet_params: planet_uniforms.params,
            saturn_ring_pole_sinb: planet_uniforms.saturn_ring_pole_sinb,
            saturn_ring_state: planet_uniforms.saturn_ring_state,
            galilean_eq_radius: planet_uniforms.galilean_eq_radius,
            galilean_rgb_magnitude: planet_uniforms.galilean_rgb_magnitude,
            galilean_params: planet_uniforms.galilean_params,
            titan_eq_radius: planet_uniforms.titan_eq_radius,
            titan_rgb_magnitude: planet_uniforms.titan_rgb_magnitude,
            titan_params: planet_uniforms.titan_params,
            hw_coeffs,
            hw_radiance,
            scintillation_params,
            solar_eclipse_state: solar_eclipse_state_uniform,
            occluders: occluders_uniform,
            occluder_params: occluder_params_uniform,
        }
    }

    /// Drag-style interactive rotation: `daz` scrolls azimuth (East-positive),
    /// `dalt` raises altitude. Altitude is clamped just shy of ±π/2 to avoid gimbal lock.
    pub fn rotate_view(&mut self, daz: f32, dalt: f32) {
        let view = self.effective_view();
        self.view.azimuth_rad = (view.azimuth_rad + daz).rem_euclid(std::f32::consts::TAU);
        self.view.altitude_rad = (view.altitude_rad + dalt).clamp(-ALT_LIMIT, ALT_LIMIT);
        self.view.fov_y_rad = view.fov_y_rad;
    }

    /// Multiplicative FOV zoom. `factor < 1` zooms in (narrower FOV).
    pub fn zoom_fov(&mut self, factor: f32) {
        let view = self.effective_view();
        let factor = if factor.is_finite() { factor } else { 1.0 };
        self.view = LocalView {
            fov_y_rad: (view.fov_y_rad * factor).clamp(MIN_FOV_Y_RAD, MAX_FOV_Y_RAD),
            ..view
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V-52a: the Saturn ring band-radius constants live in two places (the
    /// host-side `astronomy::SaturnRingApparent::BAND_RADII_R_S` array and the
    /// WGSL `SATURN_RING_*_R_S` declarations in `shaders/skyglow.wgsl`). They
    /// must agree at `f32` precision; this test re-parses the shader source so
    /// the two cannot silently drift.
    #[test]
    fn saturn_ring_band_constants_agree_with_shader() {
        let shader = include_str!("shaders/skyglow.wgsl");
        let names = [
            "SATURN_RING_C_INNER_R_S",
            "SATURN_RING_B_INNER_R_S",
            "SATURN_RING_B_OUTER_R_S",
            "SATURN_RING_A_INNER_R_S",
            "SATURN_RING_A_OUTER_R_S",
        ];
        for (idx, name) in names.iter().enumerate() {
            let needle = format!("const {name}: f32 = ");
            let start = shader
                .find(&needle)
                .unwrap_or_else(|| panic!("shader missing constant {name}"));
            let after = &shader[start + needle.len()..];
            let end = after.find(';').expect("const declaration terminator");
            let lit = after[..end].trim();
            let parsed: f32 = lit
                .parse()
                .unwrap_or_else(|error| panic!("could not parse {name} = {lit}: {error}"));
            let host_f32 = astronomy::SaturnRingApparent::BAND_RADII_R_S[idx] as f32;
            assert_eq!(
                parsed.to_bits(),
                host_f32.to_bits(),
                "{name}: shader literal {parsed} (0x{:08x}) != host f32 {host_f32} (0x{:08x})",
                parsed.to_bits(),
                host_f32.to_bits(),
            );
        }
    }

    fn observer_at(lat_deg: f64) -> Observer {
        // Use J2000 so the pole/zenith geometry tests are not asserting a
        // particular modern precession offset.
        Observer::from_degrees(lat_deg, 0.0, astronomy::J2000_JD)
    }

    #[test]
    fn titan_uniform_matches_apparent_titan_at_j2000() {
        // V-52c: the Titan uniform slot should publish a non-zero direction
        // and a magnitude consistent with the standalone astronomy backend
        // at the same instant. We compare the host-side `apparent_titan`
        // result (the same backend the renderer consumes through
        // `apparent_titan_topocentric`) to the uniform slot's `xyz`. The
        // separation must stay within ≈10″ — Earth-radius parallax on
        // Saturn (~2″) plus refraction (we hand the renderer atmosphere
        // off, but the apparent_disk_direction_j2000 chain still rotates
        // through equinox-of-date) is well inside that bound.
        let observer = observer_at(35.0);
        let mut cam = Camera::new(observer, LocalView::default(), 1.0);
        cam.atmosphere = Atmosphere::OFF;
        let pu = cam.planet_uniforms();

        // Slot 0 is the only Titan slot; count = 1, enabled = 1.
        assert_eq!(pu.titan_params[0], TITAN_UNIFORM_COUNT as f32);
        assert_eq!(pu.titan_params[1], 1.0);

        // Direction is a unit vector.
        let dir = Vec3::new(
            pu.titan_eq_radius[0],
            pu.titan_eq_radius[1],
            pu.titan_eq_radius[2],
        );
        assert!((dir.length() - 1.0).abs() < 1e-4, "|dir|={}", dir.length());

        // Magnitude is in the Karkoschka 1998 family (V ≈ 7.8 .. 9.0 over the
        // full Earth–Saturn distance range). At J2000 Saturn is near 3-4 AU
        // from Earth so we expect a magnitude in the upper-8s.
        let mag = pu.titan_rgb_magnitude[3];
        assert!(
            (7.5..9.5).contains(&mag),
            "Titan magnitude in uniform = {mag}, expected 7.5..9.5"
        );

        // The angular-radius field is sub-arcsecond (Titan is sub-pixel
        // at every FoV the renderer currently exposes).
        let radius_arcsec = (pu.titan_eq_radius[3] as f64).to_degrees() * 3600.0;
        assert!(
            radius_arcsec < 1.0,
            "Titan apparent radius in uniform = {radius_arcsec}\" too large"
        );

        // RGB is the documented amber-haze tint (positive, < 1).
        for &c in &pu.titan_rgb_magnitude[..3] {
            assert!(c > 0.0 && c <= 1.0, "Titan RGB channel out of range: {c}");
        }
    }

    #[test]
    fn titan_uniform_disabled_when_planets_off() {
        // V-52c: `PlanetUniforms::disabled()` is what the host passes when
        // the user toggles planets off. In that state the Titan-enabled
        // gate must read zero so the shader skips the moon entirely.
        let pu = PlanetUniforms::disabled();
        assert_eq!(pu.titan_params[0], TITAN_UNIFORM_COUNT as f32);
        assert_eq!(pu.titan_params[1], 0.0);
        assert_eq!(pu.titan_eq_radius, [0.0; 4]);
        assert_eq!(pu.titan_rgb_magnitude, [0.0; 4]);
    }

    #[test]
    fn celestial_pole_projects_above_camera_at_latitude() {
        // The North Celestial Pole (equatorial vector (0,0,1)) sits in the local
        // ENU frame at altitude = observer's latitude, due north. With the camera
        // at azimuth=0, altitude=0 (horizon, looking north) it should project to
        // view-space y > 0 (above center) and z < 0 (in front).
        let lat_deg = 35.0_f64;
        let view = LocalView {
            azimuth_rad: 0.0,
            altitude_rad: 0.0,
            fov_y_rad: std::f32::consts::FRAC_PI_4,
        };
        let cam = Camera::new(observer_at(lat_deg), view, 1.0);
        let pole_eq = Vec3::new(0.0, 0.0, 1.0);

        let view_pos = cam.view_matrix() * pole_eq.extend(0.0);
        assert!(
            view_pos.z < 0.0,
            "pole should be in front, got z={}",
            view_pos.z
        );
        assert!(
            view_pos.y > 0.0,
            "pole should be above center, got y={}",
            view_pos.y
        );

        // The angle above the forward axis should equal the observer's latitude.
        let angle_rad = (view_pos.y / -view_pos.z).atan() as f64;
        assert!(
            (angle_rad - lat_deg.to_radians()).abs() < 1e-4,
            "expected pole at altitude={lat_deg}°, got {}°",
            angle_rad.to_degrees()
        );
    }

    #[test]
    fn altitude_clamps() {
        let mut cam = Camera::new(observer_at(0.0), LocalView::default(), 1.0);
        cam.rotate_view(0.0, 100.0);
        assert!(cam.view.altitude_rad <= ALT_LIMIT + 1e-6);
        cam.rotate_view(0.0, -200.0);
        assert!(cam.view.altitude_rad >= -ALT_LIMIT - 1e-6);
    }

    #[test]
    fn initial_view_is_clamped() {
        let cam = Camera::new(
            observer_at(0.0),
            LocalView {
                azimuth_rad: -0.5,
                altitude_rad: 100.0,
                fov_y_rad: 0.0,
            },
            1.0,
        );
        assert!((0.0..std::f32::consts::TAU).contains(&cam.view.azimuth_rad));
        assert!(cam.view.altitude_rad <= ALT_LIMIT);
        assert_eq!(cam.view.fov_y_rad, MIN_FOV_Y_RAD);
    }

    #[test]
    fn eyepiece_optics_pin_plate_scale_and_true_field() {
        let sim = EyepieceSimulation {
            enabled: true,
            aperture_mm: 200.0,
            focal_length_mm: 2000.0,
            eyepiece_focal_length_mm: 25.0,
            apparent_fov_deg: 50.0,
            field_stop_mm: 21.0,
        };
        assert!((sim.plate_scale_arcsec_per_mm() - 103.1324).abs() < 1e-3);
        assert!((sim.magnification() - 80.0).abs() < 1e-6);
        assert!((sim.exit_pupil_mm() - 2.5).abs() < 1e-6);
        assert!((sim.true_field_deg() - 0.6016).abs() < 1e-3);

        let afov_only = EyepieceSimulation {
            field_stop_mm: 0.0,
            ..sim
        };
        assert!((afov_only.true_field_deg() - 0.625).abs() < 1e-3);
    }

    #[test]
    fn eyepiece_overrides_perspective_fov_only() {
        let mut cam = Camera::new(observer_at(0.0), LocalView::default(), 1.0);
        cam.eyepiece = EyepieceSimulation::DEFAULT_ENABLED;
        assert!((cam.effective_view().fov_y_rad.to_degrees() - 0.6016).abs() < 1e-3);

        cam.projection = SkyProjection::Mollweide;
        assert_eq!(
            cam.effective_view().fov_y_rad,
            LocalView::default().fov_y_rad
        );

        cam.projection = SkyProjection::Perspective;
        cam.viewpoint = SkyViewpoint::GalacticNorth;
        assert_eq!(
            cam.effective_view().fov_y_rad,
            LocalView::default().fov_y_rad
        );
    }

    /// The third row of the equatorial→ENU matrix — which `zenith_in_equatorial`
    /// returns — must be the local "Up" basis vector expressed in equatorial
    /// coordinates. At the North Pole the local Up coincides with the
    /// equatorial +z; at the Equator looking along LST=0 the local Up sits in
    /// the equatorial xy plane. Pin both so a refactor of `equatorial_to_horizontal_matrix`
    /// can't silently break the shader-side altitude derivation.
    #[test]
    fn zenith_in_equatorial_matches_observer_latitude() {
        // North pole: local up = equatorial +z.
        let cam_pole = Camera::new(observer_at(90.0), LocalView::default(), 1.0);
        let z_pole = cam_pole.zenith_in_equatorial();
        assert!((z_pole.x).abs() < 1e-4);
        assert!((z_pole.y).abs() < 1e-4);
        assert!((z_pole.z - 1.0).abs() < 1e-4);

        // Equator: local up lies in the equatorial xy plane (z = 0).
        let cam_eq = Camera::new(observer_at(0.0), LocalView::default(), 1.0);
        let z_eq = cam_eq.zenith_in_equatorial();
        assert!(
            z_eq.z.abs() < 1e-4,
            "equator zenith should have eq-z = 0, got {z_eq:?}"
        );
        // Length must be ~1 (orthonormal rotation).
        assert!(
            (z_eq.length() - 1.0).abs() < 1e-4,
            "zenith vector not unit length: {z_eq:?}"
        );
    }

    /// Default `Atmosphere` carries the Hardie 1962 mid-quality (β, α, DU)
    /// state; `Atmosphere::OFF` zeros the aerosol/ozone budget. Pin both so
    /// drift in defaults is loud.
    #[test]
    fn atmosphere_defaults_and_off_are_pinned() {
        let d = Atmosphere::default();
        assert_eq!(d.aerosol_beta, 0.10);
        assert_eq!(d.aerosol_alpha, 1.30);
        assert_eq!(d.ozone_du, 300.0);
        assert_eq!(d.pressure_hpa, 1010.0);
        assert_eq!(d.temperature_c, 10.0);
        let k = d.extinction_k_rgb();
        assert!(
            k[0] < k[1] && k[1] < k[2],
            "derived k_RGB must be monotone red→blue: {k:?}"
        );
        let off = Atmosphere::OFF;
        assert_eq!(off.extinction_k_rgb(), [0.0; 3]);
    }

    #[test]
    fn atmosphere_uniform_rejects_non_finite_host_values() {
        let mut cam = Camera::new(observer_at(35.0), LocalView::default(), 1.0);
        cam.atmosphere.aerosol_beta = f32::NAN;
        cam.atmosphere.aerosol_alpha = f32::NAN;
        cam.atmosphere.observer_altitude_m = f32::NAN;
        cam.atmosphere.ozone_du = f32::NAN;
        cam.atmosphere.pressure_hpa = f32::NAN;
        cam.atmosphere.temperature_c = f32::NAN;
        let planet_uniforms = cam.planet_uniforms();
        let uniform = cam.uniform_with_planets(800, 600, &planet_uniforms);
        // k_RGB falls back to the DEFAULT (β, α, DU, h) state, not zero,
        // because the per-field guards substitute DEFAULT values.
        let expected_k = Atmosphere::DEFAULT.extinction_k_rgb();
        assert!((uniform.extinction_k_rgb[0] - expected_k[0]).abs() < 1e-6);
        assert!((uniform.extinction_k_rgb[1] - expected_k[1]).abs() < 1e-6);
        assert!((uniform.extinction_k_rgb[2] - expected_k[2]).abs() < 1e-6);
        let expected_t = astronomy::atmosphere::linke_turbidity_from_aerosol(
            Atmosphere::DEFAULT.aerosol_beta as f64,
        ) as f32;
        assert!((uniform.atmosphere_params[0] - expected_t).abs() < 1e-6);
        assert_eq!(
            uniform.atmosphere_params[1],
            Atmosphere::DEFAULT.observer_altitude_m
        );
        assert_eq!(uniform.atmosphere_optics[0], Atmosphere::DEFAULT.ozone_du);
        assert_eq!(
            uniform.atmosphere_optics[1],
            Atmosphere::DEFAULT.aerosol_beta
        );
        assert_eq!(
            uniform.atmosphere_optics[2],
            Atmosphere::DEFAULT.aerosol_alpha
        );
        assert_eq!(
            uniform.refraction_params[0],
            Atmosphere::DEFAULT.pressure_hpa
        );
        assert_eq!(
            uniform.refraction_params[1],
            Atmosphere::DEFAULT.temperature_c
        );
    }

    /// V-24: scintillation uniform must be deterministic across hosts for a
    /// pinned (observer, time, seed) tuple, and must zero itself when the
    /// camera is on an external galactic viewpoint or has scintillation off.
    #[test]
    fn scintillation_uniform_is_deterministic_and_gated() {
        let mut cam = Camera::new(observer_at(35.68), LocalView::default(), 1.0);
        cam.scintillation = Scintillation {
            enabled: true,
            c_n2_scale: 1.0,
            seed: 0xDEAD_BEEF,
        };
        let a = cam.uniform_with_planets(800, 600, &PlanetUniforms::disabled());
        let b = cam.uniform_with_planets(800, 600, &PlanetUniforms::disabled());
        assert_eq!(
            a.scintillation_params, b.scintillation_params,
            "same (observer, time, seed) must produce identical scintillation params"
        );
        assert!(
            a.scintillation_params[0] > 0.0,
            "σ²_zenith must be positive when enabled"
        );
        assert!(
            a.scintillation_params[1] > 0.0,
            "corner_hz must be positive when enabled"
        );
        // Seed round-trip: shader recovers the host's u32 via bitcast.
        assert_eq!(a.scintillation_params[2].to_bits(), 0xDEAD_BEEF);
        // t_seconds is in the wrapped [0, 86400) window so f32 keeps subsecond precision.
        assert!(a.scintillation_params[3] >= 0.0 && a.scintillation_params[3] < 86_400.0);

        // External galactic viewpoint must zero σ² so off-Earth scenes stay
        // deterministic and free of inadvertent stellar twinkle.
        cam.viewpoint = SkyViewpoint::GalacticNorth;
        let external = cam.uniform_with_planets(800, 600, &PlanetUniforms::disabled());
        assert_eq!(external.scintillation_params[0], 0.0);

        // Atmosphere::OFF must also disable scintillation.
        cam.viewpoint = SkyViewpoint::Earth;
        cam.atmosphere = Atmosphere::OFF;
        let no_atmosphere = cam.uniform_with_planets(800, 600, &PlanetUniforms::disabled());
        assert_eq!(no_atmosphere.scintillation_params[0], 0.0);

        // Explicit disable.
        cam.atmosphere = Atmosphere::DEFAULT;
        cam.scintillation = Scintillation::OFF;
        let off = cam.uniform_with_planets(800, 600, &PlanetUniforms::disabled());
        assert_eq!(off.scintillation_params[0], 0.0);
    }

    /// V-51b: the analytic-mask uniform must contain a Sun-targeted
    /// occluder during the Mazatlán 2024-04-08 totality preset, its
    /// direction must equal `moon_eq_illuminance.xyz` (bit-identical
    /// J2000-and-refracted), and its radius must equal `moon_disk.x`.
    /// Anything else would let the analytic mask drift away from the
    /// V-51c photometric falloff and break the golden frame committed
    /// in `docs/assets/validation/solar-eclipse.png`. V-51d adds a
    /// second Stars-target entry (Moon-on-Stars cull) at every frame;
    /// the Sun entry sits in slot 0 because the producer emits it
    /// first.
    #[test]
    fn occluder_uniform_matches_moon_state_at_mazatlan_peak() {
        // 2024-04-08T18:13:00Z, the SolarEclipse scene-preset epoch.
        let jd_utc = astronomy::julian_date_from_unix_seconds(1_712_599_980.0);
        let observer = Observer::from_degrees(23.219, -106.420, jd_utc);
        let cam = Camera::new(observer, LocalView::default(), 1.0);
        let uniform = cam.uniform_with_planets(800, 600, &PlanetUniforms::disabled());

        // V-51c Moon-on-Sun + V-51d Moon-on-Stars cull entry are both
        // emitted at totality (the Stars entry is unconditional).
        assert_eq!(
            uniform.occluder_params[0], 2.0,
            "Moon-on-Sun + Moon-on-Stars entries must both be active at greatest eclipse"
        );
        // Sun entry is emitted first by `active_occluders`, so it lives
        // in slot 0; the V-51d Stars cull entry is in slot 1.
        let dir = &uniform.occluders[0];
        let target_kind = &uniform.occluders[1];
        let moon_dir = &uniform.moon_eq_illuminance;
        let moon_radius = uniform.moon_disk[0];
        // Bit-identical to the J2000-and-refracted Moon direction stored
        // in `moon_eq_illuminance.xyz`. Any per-pixel f32 drift here would
        // misalign the analytic mask against the Moon disk source term.
        assert_eq!(dir[0], moon_dir[0]);
        assert_eq!(dir[1], moon_dir[1]);
        assert_eq!(dir[2], moon_dir[2]);
        assert_eq!(dir[3], moon_radius);
        // target = Sun (0), kind = Partial (1) at this epoch — deep but
        // not geometrically Total (the classifier needs r_moon ≥ r_sun).
        assert_eq!(target_kind[0], 0.0);
        assert_eq!(target_kind[1], 1.0);
        // Obscuration must mirror `solar_eclipse_state.y` exactly.
        assert!((target_kind[2] - uniform.solar_eclipse_state[1]).abs() < 1e-6);
        // V-51d Stars cull entry sits in slot 1 with the same Moon
        // front-disk geometry as the Sun entry (one shared producer).
        let stars_dir = &uniform.occluders[2];
        let stars_kind = &uniform.occluders[3];
        assert_eq!(stars_dir[0], moon_dir[0]);
        assert_eq!(stars_dir[1], moon_dir[1]);
        assert_eq!(stars_dir[2], moon_dir[2]);
        assert_eq!(stars_dir[3], moon_radius);
        assert_eq!(stars_kind[0], -1.0, "Stars target code must be -1");
        // Padded entries stay zero so the shader's loop never reads junk.
        for i in 2..MAX_OCCLUDERS {
            assert_eq!(uniform.occluders[i * 2], [0.0; 4]);
            assert_eq!(uniform.occluders[i * 2 + 1], [0.0; 4]);
        }
    }

    /// V-51b: external galactic viewpoints and `Atmosphere::OFF` zero the
    /// occluder list, mirroring the same gating the V-51c
    /// `solar_eclipse_state` uniform uses. Without this, the analytic
    /// mask would still try to subtract a (now meaningless) front disk
    /// from a Sun source term that the renderer is no longer drawing.
    #[test]
    fn occluder_uniform_zeros_on_external_or_atmosphere_off() {
        let jd_utc = astronomy::julian_date_from_unix_seconds(1_712_599_980.0);
        let observer = Observer::from_degrees(23.219, -106.420, jd_utc);

        let mut cam = Camera::new(observer, LocalView::default(), 1.0);
        cam.atmosphere = Atmosphere::OFF;
        let off = cam.uniform_with_planets(800, 600, &PlanetUniforms::disabled());
        assert_eq!(off.occluder_params[0], 0.0);

        let mut cam = Camera::new(observer, LocalView::default(), 1.0);
        cam.viewpoint = SkyViewpoint::GalacticNorth;
        let external = cam.uniform_with_planets(800, 600, &PlanetUniforms::disabled());
        assert_eq!(external.occluder_params[0], 0.0);
    }

    /// V-51d: at an off-eclipse epoch the producer emits only the
    /// Moon-on-Stars cull entry (always present), and no Sun / planet
    /// occluders. The renderer must pack exactly that one entry so the
    /// star vertex shader can iterate it without reading padded rows.
    /// Mirrors `astronomy::planning::active_occluders_off_eclipse_emits_only_moon_on_stars`.
    #[test]
    fn occluder_uniform_off_eclipse_emits_only_moon_on_stars() {
        let jd_utc = astronomy::julian_date_from_unix_seconds(1_751_328_000.0); // 2025-07-01T00:00Z
        let observer = Observer::from_degrees(35.68, 139.69, jd_utc);
        let cam = Camera::new(observer, LocalView::default(), 1.0);
        let uniform = cam.uniform_with_planets(800, 600, &PlanetUniforms::disabled());
        assert_eq!(uniform.occluder_params[0], 1.0);
        // Slot 0 carries the Stars-target cull entry (target code = -1).
        assert_eq!(uniform.occluders[1][0], -1.0);
        // Front radius must equal the Moon apparent semidiameter.
        let moon_radius = uniform.moon_disk[0];
        assert!((uniform.occluders[0][3] - moon_radius).abs() < 1.0e-6);
    }

    #[test]
    fn azimuth_wraps_to_zero_two_pi() {
        let mut cam = Camera::new(observer_at(0.0), LocalView::default(), 1.0);
        cam.rotate_view(-0.5, 0.0);
        assert!((0.0..std::f32::consts::TAU).contains(&cam.view.azimuth_rad));
    }

    #[test]
    fn sky_projection_kebab_round_trips() {
        for projection in SkyProjection::ALL {
            let s = projection.as_kebab_str();
            assert_eq!(SkyProjection::from_kebab_str(s), Some(*projection));
        }
        assert_eq!(SkyProjection::from_kebab_str("unknown"), None);
    }

    #[test]
    fn sky_viewpoint_kebab_round_trips() {
        for viewpoint in SkyViewpoint::ALL {
            let s = viewpoint.as_kebab_str();
            assert_eq!(SkyViewpoint::from_kebab_str(s), Some(*viewpoint));
        }
        assert_eq!(SkyViewpoint::from_kebab_str("unknown"), None);
    }

    #[test]
    fn galactic_viewpoint_uses_external_parsec_camera() {
        let mut cam = Camera::new(observer_at(0.0), LocalView::default(), 1.0);
        cam.viewpoint = SkyViewpoint::GalacticNorth;
        let uniform = cam.uniform_with_planets(800, 600, &PlanetUniforms::disabled());
        assert_eq!(uniform.viewpoint_params[0], 1.0);
        assert_eq!(uniform.viewpoint_params[1], 0.0);
        assert_eq!(uniform.viewpoint_params[2], 0.0);
        assert_eq!(uniform.viewpoint_params[3], GALACTIC_CAMERA_HEIGHT_PC);
        assert_eq!(cam.overlay_projection_params()[0], -1.0);
    }

    #[test]
    fn custom_external_viewpoint_uploads_origin_and_orientation() {
        let mut cam = Camera::new(observer_at(0.0), LocalView::default(), 1.0);
        cam.viewpoint = SkyViewpoint::CustomExternal;
        cam.external_viewpoint =
            ExternalViewpoint::new([8_200.0, -120.0, 500.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let uniform = cam.uniform_with_planets(800, 600, &PlanetUniforms::disabled());
        assert_eq!(uniform.viewpoint_params[0], 1.0);
        assert_eq!(uniform.viewpoint_params[1], 8_200.0);
        assert_eq!(uniform.viewpoint_params[2], -120.0);
        assert_eq!(uniform.viewpoint_params[3], 500.0);
        assert!(cam.view_proj().is_finite());
    }

    #[test]
    fn full_sky_scale_preserves_two_to_one_map_aspect() {
        assert_eq!(full_sky_map_scale(2.0), [1.0, 1.0]);
        assert_eq!(full_sky_map_scale(1.0), [1.0, 0.5]);
        assert_eq!(full_sky_map_scale(4.0), [0.5, 1.0]);
        assert_eq!(full_sky_map_scale(f32::NAN), [1.0, 0.5]);
    }

    #[test]
    fn all_sky_pixel_solid_angle_averages_to_four_pi() {
        let mut cam = Camera::new(observer_at(0.0), LocalView::default(), 2.0);
        cam.projection = SkyProjection::Mollweide;
        let sr = cam.pixel_solid_angle_sr(1000, 500);
        let ellipse_pixels = std::f32::consts::PI * 500.0 * 250.0;
        let total_sr = sr * ellipse_pixels;
        assert!((total_sr - std::f32::consts::TAU * 2.0).abs() < 1e-4);
    }
}
