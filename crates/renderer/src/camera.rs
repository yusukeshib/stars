use astronomy::photometry::DEFAULT_EXTINCTION_K_RGB;
use astronomy::{
    equatorial_to_horizontal_matrix, illuminants, lmst_radians, Observer, SunMoonApparent,
};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

use crate::vertex::{limiting_magnitude_to_zeropoint, NAKED_EYE_LIMITING_MAGNITUDE};

/// GPU-side camera + atmosphere state. WGSL layout requires `vec3` fields to
/// be 16-byte aligned, so the per-channel extinction coefficients and the
/// equatorial "zenith" direction are stored as `vec4` with an unused `w`
/// component. This keeps the Rust struct byte-for-byte identical to the
/// shader's view of it.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct CameraUniform {
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
    /// phase_angle_rad, unused]`.
    pub moon_disk: [f32; 4],
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
        sunlit_scattering: true,
    };

    pub const HAZY_URBAN: Self = Self {
        extinction_k_rgb: [0.18, 0.28, 0.45],
        turbidity: 5.0,
        observer_altitude_m: 0.0,
        ozone_du: 325.0,
        visibility_km: 12.0,
        sunlit_scattering: true,
    };

    pub const HIGH_ALTITUDE: Self = Self {
        extinction_k_rgb: [0.06, 0.10, 0.18],
        turbidity: 2.0,
        observer_altitude_m: 2500.0,
        ozone_du: 275.0,
        visibility_km: 80.0,
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
/// a full-sky projection (README Phase 4) rather than a perspective camera.
const MAX_FOV_Y_RAD: f32 = 120.0 * std::f32::consts::PI / 180.0;

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
            limiting_magnitude: NAKED_EYE_LIMITING_MAGNITUDE,
        }
    }

    /// Rotation that maps a J2000 equatorial direction into the observer's local ENU frame.
    fn equatorial_to_horizontal(&self) -> Mat4 {
        let lst = lmst_radians(self.observer.time.jd_ut1, self.observer.longitude_rad);
        equatorial_to_horizontal_matrix(self.observer.latitude_rad, lst)
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
    pub fn view_matrix_local(&self) -> Mat4 {
        // look_at_rh derives screen-right from (forward × up); using local zenith as
        // "up" keeps the horizon level on screen. `forward_local` uses a clamped
        // view so host-supplied ±90° altitudes cannot hit the gimbal-lock singularity.
        Mat4::look_at_rh(Vec3::ZERO, self.forward_local(), Vec3::Z)
    }

    /// View matrix in J2000 equatorial coordinates (includes the equatorial→horizontal
    /// rotation). Use this for star positions, RA/Dec grids, ecliptic, celestial equator.
    pub fn view_matrix(&self) -> Mat4 {
        self.view_matrix_local() * self.equatorial_to_horizontal()
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.effective_view().fov_y_rad, self.aspect, 0.01, 10.0)
    }

    /// View-projection for J2000 equatorial-frame geometry. Alias kept for backward compat.
    pub fn view_proj(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    /// View-projection for local ENU-frame geometry.
    pub fn view_proj_local(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix_local()
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
        let eq_to_enu = self.equatorial_to_horizontal();
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
    fn pixel_solid_angle_sr(&self, height_pixels: u32) -> f32 {
        let pixel_size_rad = self.effective_view().fov_y_rad / height_pixels.max(1) as f32;
        pixel_size_rad * pixel_size_rad
    }

    pub fn uniform(&self, width: u32, height: u32) -> CameraUniform {
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
        let disks = SunMoonApparent::for_observer(self.observer);
        let sun = disks.sun;
        let sun_dir = sun.direction_equatorial();
        let solar_lux = illuminants::solar_illuminance_lux(sun.distance_au) as f32;
        let solar_rgb = illuminants::SOLAR_LINEAR_RGB;
        let moon = disks.moon;
        let moon_dir = moon.direction_equatorial();
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
        let view_proj = self.view_proj();
        let inv_view_proj = view_proj.inverse();
        let eq_to_local = self.equatorial_to_horizontal();
        let view_proj_local = self.view_proj_local();
        CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            eq_to_local: eq_to_local.to_cols_array_2d(),
            view_proj_local: view_proj_local.to_cols_array_2d(),
            viewport_pixel_sr_zeropoint: [
                width as f32,
                height as f32,
                self.pixel_solid_angle_sr(height),
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
                0.0,
            ],
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
        // Use a fixed JD; LST cancels out for the celestial pole test below.
        Observer::from_degrees(lat_deg, 0.0, 2_460_000.5)
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
        let uniform = cam.uniform(800, 600);
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
    }

    #[test]
    fn azimuth_wraps_to_zero_two_pi() {
        let mut cam = Camera::new(observer_at(0.0), LocalView::default(), 1.0);
        cam.rotate_view(-0.5, 0.0);
        assert!((0.0..std::f32::consts::TAU).contains(&cam.view.azimuth_rad));
    }
}
