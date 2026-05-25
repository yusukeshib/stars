use astronomy::photometry::DEFAULT_EXTINCTION_K_RGB;
use astronomy::{
    apparent_planets_topocentric, earth_velocity_over_c_j2000, equation_of_equinoxes,
    equatorial_to_horizontal_matrix, illuminants, lmst_radians, precession_nutation_matrix,
    years_since_j2000, Observer, Planet, SunMoonApparent,
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
    /// `[turbidity, observer_altitude_m, solar_illuminance_lux, enabled]`.
    pub atmosphere_params: [f32; 4],
    /// Top-of-atmosphere solar RGB illuminant, normalised around D65. `w` is
    /// currently unused.
    pub solar_rgb: [f32; 4],
    /// Additional atmospheric optics controls: `[ozone_du, visibility_km,
    /// unused, unused]`.
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
    /// Planet directions in equatorial coordinates. `w` is angular radius.
    pub planet_eq_radius: PlanetEqRadiusUniform,
    /// Planet display colour in linear RGB. `w` is apparent visual magnitude.
    pub planet_rgb_magnitude: PlanetRgbMagnitudeUniform,
    /// `[planet_count, planets_enabled, unused, unused]`.
    pub planet_params: [f32; 4],
}

pub(crate) const PLANET_UNIFORM_COUNT: usize = 7;
pub(crate) type PlanetEqRadiusUniform = [[f32; 4]; PLANET_UNIFORM_COUNT];
pub(crate) type PlanetRgbMagnitudeUniform = [[f32; 4]; PLANET_UNIFORM_COUNT];

/// Cached renderer-facing planet uniforms. Computing VSOP87 planet states is
/// orders of magnitude more expensive than rebuilding the camera matrices, so
/// `Renderer` reuses this between coarse ephemeris refreshes while still
/// updating Sun/Moon, refraction, stars, and camera orientation every frame.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PlanetUniforms {
    pub eq_radius: PlanetEqRadiusUniform,
    pub rgb_magnitude: PlanetRgbMagnitudeUniform,
    pub params: [f32; 4],
}

impl PlanetUniforms {
    pub const fn disabled() -> Self {
        Self {
            eq_radius: [[0.0; 4]; PLANET_UNIFORM_COUNT],
            rgb_magnitude: [[0.0; 4]; PLANET_UNIFORM_COUNT],
            params: [PLANET_UNIFORM_COUNT as f32, 0.0, 0.0, 0.0],
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
#[derive(Debug, Clone, Copy)]
pub struct Atmosphere {
    /// Per-channel extinction coefficients `[k_R, k_G, k_B]` in magnitudes
    /// per unit airmass. The shader applies
    /// `extinction_factor = 10^(-0.4 · k · X)` independently to each RGB
    /// channel, where `X` is the Kasten-Young 1989 airmass at the star's
    /// altitude.
    pub extinction_k_rgb: [f32; 3],
    /// Aerosol / haze control for sunlit sky colour. Values around 2–3 are
    /// clear rural skies; larger values whiten and brighten the horizon via
    /// the Mie component.
    pub turbidity: f32,
    /// Observer altitude above sea level in metres. The first-order shader
    /// uses this to thin the optical depth exponentially with scale height.
    pub observer_altitude_m: f32,
    /// Total ozone column in Dobson units. Around 300 DU is a mid-latitude
    /// clear-sky default; larger values suppress orange/red sunset light more
    /// strongly through a Chappuis-band approximation in the shader.
    pub ozone_du: f32,
    /// Meteorological visibility in kilometres. This controls Mie/aerosol haze
    /// independently from the named turbidity presets.
    pub visibility_km: f32,
    /// Surface pressure in hPa for apparent-altitude refraction. Standard
    /// atmosphere is 1010 hPa; lower pressure reduces the horizon lift.
    pub pressure_hpa: f32,
    /// Air temperature in °C for apparent-altitude refraction. Saemundsson's
    /// correction scales as 283 K / (273 + T).
    pub temperature_c: f32,
    /// Whether direct solar scattering is enabled. `Atmosphere::OFF` disables
    /// both extinction and daylight/twilight scattering.
    pub sunlit_scattering: bool,
}

impl Atmosphere {
    /// Clean sea-level dark site — the default model.
    /// See [`astronomy::photometry::DEFAULT_EXTINCTION_K_RGB`].
    pub const CLEAR_RURAL: Self = Self {
        extinction_k_rgb: [
            DEFAULT_EXTINCTION_K_RGB[0] as f32,
            DEFAULT_EXTINCTION_K_RGB[1] as f32,
            DEFAULT_EXTINCTION_K_RGB[2] as f32,
        ],
        turbidity: 2.5,
        observer_altitude_m: 0.0,
        ozone_du: 300.0,
        visibility_km: 50.0,
        pressure_hpa: 1010.0,
        temperature_c: 10.0,
        sunlit_scattering: true,
    };

    pub const HAZY_URBAN: Self = Self {
        extinction_k_rgb: [0.18, 0.28, 0.45],
        turbidity: 5.0,
        observer_altitude_m: 0.0,
        ozone_du: 325.0,
        visibility_km: 12.0,
        pressure_hpa: 1010.0,
        temperature_c: 15.0,
        sunlit_scattering: true,
    };

    pub const HIGH_ALTITUDE: Self = Self {
        extinction_k_rgb: [0.06, 0.10, 0.18],
        turbidity: 2.0,
        observer_altitude_m: 2500.0,
        ozone_du: 275.0,
        visibility_km: 80.0,
        pressure_hpa: 750.0,
        temperature_c: 0.0,
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
        extinction_k_rgb: [0.0, 0.0, 0.0],
        turbidity: 0.0,
        observer_altitude_m: 0.0,
        ozone_du: 0.0,
        visibility_km: 0.0,
        pressure_hpa: 0.0,
        temperature_c: 10.0,
        sunlit_scattering: false,
    };
}

impl Default for Atmosphere {
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

/// How close to the zenith / nadir the camera is allowed to tilt before the
/// clamp engages, in radians. The clamp exists because `Mat4::look_at_rh`
/// uses `forward × up` to build the right-axis: when `forward` aligns with
/// our world `up = +Z`, the cross product collapses and the view matrix
/// degenerates. The 0.01 rad (≈0.57°) gap keeps `|forward × up| ≳ 0.01`, which
/// glam normalises without precision loss while staying invisible at
/// reasonable FoVs. If you ever want to *look* at the zenith, switch to a
/// gimbal-lock-free representation (quaternion) rather than widening this.
const ALT_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.01;

/// Narrowest supported vertical field of view. Below ≈5° the current single-
/// precision view-projection matrices and fixed-size point-spread sprites make
/// the renderer behave more like a telescope simulator, which Phase 1 is not.
const MIN_FOV_Y_RAD: f32 = 5.0 * std::f32::consts::PI / 180.0;
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
    /// Whether Mercury through Neptune are rendered as apparent solar-system
    /// disks/points from the VSOP87 ephemeris.
    pub planets_enabled: bool,
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
            planets_enabled: true,
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
        self.view.clamped()
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

    fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.effective_view().fov_y_rad, self.aspect, 0.01, 10.0)
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

    /// Matrix stored in `CameraUniform::view_proj`.
    ///
    /// Perspective mode keeps the historical view-projection matrix. All-sky
    /// projections are non-linear, so the shader needs the pre-projection view
    /// matrix and performs the spherical map itself.
    fn shader_view_proj(&self) -> Mat4 {
        if self.projection.is_full_sky() {
            self.view_matrix()
        } else {
            self.view_proj()
        }
    }

    fn shader_view_proj_local(&self) -> Mat4 {
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
        self.projection_matrix() * self.view_matrix()
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
        self.projection_params()
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

        PlanetUniforms {
            eq_radius,
            rgb_magnitude,
            params: [PLANET_UNIFORM_COUNT as f32, 1.0, 0.0, 0.0],
        }
    }

    pub(crate) fn uniform_with_planets(
        &self,
        width: u32,
        height: u32,
        planet_uniforms: &PlanetUniforms,
    ) -> CameraUniform {
        let zenith = self.zenith_in_equatorial();
        let k = self
            .atmosphere
            .extinction_k_rgb
            .map(|k| if k.is_finite() { k.max(0.0) } else { 0.0 });
        let turbidity = if self.atmosphere.turbidity.is_finite() {
            self.atmosphere.turbidity.max(0.0)
        } else {
            Atmosphere::DEFAULT.turbidity
        };
        let observer_altitude_m = if self.atmosphere.observer_altitude_m.is_finite() {
            self.atmosphere.observer_altitude_m.max(0.0)
        } else {
            Atmosphere::DEFAULT.observer_altitude_m
        };
        let ozone_du = if self.atmosphere.ozone_du.is_finite() {
            self.atmosphere.ozone_du.clamp(0.0, 600.0)
        } else {
            Atmosphere::DEFAULT.ozone_du
        };
        let visibility_km = if self.atmosphere.visibility_km.is_finite() {
            self.atmosphere.visibility_km.clamp(1.0, 200.0)
        } else {
            Atmosphere::DEFAULT.visibility_km
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
        let scattering_enabled = if self.atmosphere.sunlit_scattering {
            1.0
        } else {
            0.0
        };
        let view_proj = self.shader_view_proj();
        let inv_view_proj = self.shader_inv_view_proj();
        let eq_to_local = self.equatorial_to_horizontal();
        let view_proj_local = self.shader_view_proj_local();
        let earth_velocity = earth_velocity_over_c_j2000(self.observer.time.jd_tdb);
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
                turbidity,
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
            atmosphere_optics: [ozone_du, visibility_km, 0.0, 0.0],
            moon_eq_illuminance: [moon_dir.x, moon_dir.y, moon_dir.z, moon_lux],
            moon_disk: [
                moon.angular_radius_rad as f32,
                moon.illuminated_fraction as f32,
                moon.phase_angle_rad as f32,
                moon.earth_shadow_fraction as f32,
            ],
            projection_params: self.projection_params(),
            planet_eq_radius: planet_uniforms.eq_radius,
            planet_rgb_magnitude: planet_uniforms.rgb_magnitude,
            planet_params: planet_uniforms.params,
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

    fn observer_at(lat_deg: f64) -> Observer {
        // Use J2000 so the pole/zenith geometry tests are not asserting a
        // particular modern precession offset.
        Observer::from_degrees(lat_deg, 0.0, astronomy::J2000_JD)
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

    /// Default `Atmosphere` carries the Hardie 1962 sea-level coefficients;
    /// `Atmosphere::OFF` zeros them out. Pin both so changes in defaults are
    /// loud.
    #[test]
    fn atmosphere_defaults_and_off_are_pinned() {
        let d = Atmosphere::default();
        assert_eq!(d.extinction_k_rgb, [0.10, 0.16, 0.30]);
        assert_eq!(d.pressure_hpa, 1010.0);
        assert_eq!(d.temperature_c, 10.0);
        assert!(d.extinction_k_rgb[0] < d.extinction_k_rgb[2]);
        let off = Atmosphere::OFF;
        assert_eq!(off.extinction_k_rgb, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn atmosphere_uniform_rejects_non_finite_host_values() {
        let mut cam = Camera::new(observer_at(35.0), LocalView::default(), 1.0);
        cam.atmosphere.extinction_k_rgb = [f32::NAN, -1.0, 0.3];
        cam.atmosphere.turbidity = f32::NAN;
        cam.atmosphere.observer_altitude_m = f32::NAN;
        cam.atmosphere.ozone_du = f32::NAN;
        cam.atmosphere.visibility_km = f32::NAN;
        cam.atmosphere.pressure_hpa = f32::NAN;
        cam.atmosphere.temperature_c = f32::NAN;
        let planet_uniforms = cam.planet_uniforms();
        let uniform = cam.uniform_with_planets(800, 600, &planet_uniforms);
        assert_eq!(uniform.extinction_k_rgb, [0.0, 0.0, 0.3, 0.0]);
        assert_eq!(uniform.atmosphere_params[0], Atmosphere::DEFAULT.turbidity);
        assert_eq!(
            uniform.atmosphere_params[1],
            Atmosphere::DEFAULT.observer_altitude_m
        );
        assert_eq!(uniform.atmosphere_optics[0], Atmosphere::DEFAULT.ozone_du);
        assert_eq!(
            uniform.atmosphere_optics[1],
            Atmosphere::DEFAULT.visibility_km
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
