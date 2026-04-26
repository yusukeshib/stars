use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use stars_astronomy::{equatorial_to_horizontal_matrix, lmst_radians};

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub viewport_size: [f32; 2],
    pub _pad: [f32; 2],
}

/// Geographic observer state. Latitude/longitude in radians, Julian Date in UT.
#[derive(Debug, Clone, Copy)]
pub struct Observer {
    pub latitude_rad: f64,
    pub longitude_rad: f64,
    pub julian_date: f64,
}

impl Observer {
    pub fn from_degrees(lat_deg: f64, lng_deg: f64, julian_date: f64) -> Self {
        Self {
            latitude_rad: lat_deg.to_radians(),
            longitude_rad: lng_deg.to_radians(),
            julian_date,
        }
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

const ALT_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.01;

pub struct Camera {
    pub observer: Observer,
    pub view: LocalView,
    pub aspect: f32,
}

impl Camera {
    pub fn new(observer: Observer, view: LocalView, aspect: f32) -> Self {
        Self {
            observer,
            view,
            aspect,
        }
    }

    /// Rotation that maps a J2000 equatorial direction into the observer's local ENU frame.
    pub fn equatorial_to_horizontal(&self) -> Mat4 {
        let lst = lmst_radians(self.observer.julian_date, self.observer.longitude_rad);
        equatorial_to_horizontal_matrix(self.observer.latitude_rad, lst)
    }

    /// Forward direction (in local ENU) the camera is looking at.
    pub fn forward_local(&self) -> Vec3 {
        let (sa, ca) = self.view.azimuth_rad.sin_cos();
        let (sp, cp) = self.view.altitude_rad.sin_cos();
        Vec3::new(sa * cp, ca * cp, sp)
    }

    pub fn view_matrix(&self) -> Mat4 {
        let forward = self.forward_local();
        // look_at_rh chooses screen-right perpendicular to (up × forward); using local
        // zenith as "up" keeps the horizon level on screen. Altitude is clamped to avoid
        // gimbal lock when looking straight up/down.
        let view_in_local = Mat4::look_at_rh(Vec3::ZERO, forward, Vec3::Z);
        view_in_local * self.equatorial_to_horizontal()
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.view.fov_y_rad, self.aspect, 0.01, 10.0)
    }

    pub fn view_proj(&self) -> Mat4 {
        self.projection_matrix() * self.view_matrix()
    }

    pub fn uniform(&self, width: u32, height: u32) -> CameraUniform {
        CameraUniform {
            view_proj: self.view_proj().to_cols_array_2d(),
            viewport_size: [width as f32, height as f32],
            _pad: [0.0; 2],
        }
    }

    /// Drag-style interactive rotation: dx scrolls azimuth (East-positive),
    /// dy raises altitude.
    pub fn rotate_view(&mut self, daz: f32, dalt: f32) {
        self.view.azimuth_rad = wrap_tau(self.view.azimuth_rad + daz);
        self.view.altitude_rad = (self.view.altitude_rad + dalt).clamp(-ALT_LIMIT, ALT_LIMIT);
    }

    /// Multiplicative FOV zoom. `factor < 1` zooms in (narrower FOV).
    pub fn zoom_fov(&mut self, factor: f32) {
        let f = (self.view.fov_y_rad * factor).clamp(5.0_f32.to_radians(), 120.0_f32.to_radians());
        self.view.fov_y_rad = f;
    }
}

fn wrap_tau(x: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let mut v = x.rem_euclid(tau);
    if v < 0.0 {
        v += tau;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_north_horizon() {
        let view = LocalView {
            azimuth_rad: 0.0,
            altitude_rad: 0.0,
            fov_y_rad: 1.0,
        };
        let c = Camera::new(
            Observer::from_degrees(35.0, 139.0, 2_460_000.5),
            view,
            16.0 / 9.0,
        );
        let f = c.forward_local();
        assert!(f.x.abs() < 1e-6);
        assert!((f.y - 1.0).abs() < 1e-6);
        assert!(f.z.abs() < 1e-6);
    }

    #[test]
    fn forward_east_horizon() {
        let view = LocalView {
            azimuth_rad: std::f32::consts::FRAC_PI_2,
            altitude_rad: 0.0,
            fov_y_rad: 1.0,
        };
        let c = Camera::new(Observer::from_degrees(0.0, 0.0, 2_460_000.5), view, 1.0);
        let f = c.forward_local();
        assert!((f.x - 1.0).abs() < 1e-6);
        assert!(f.y.abs() < 1e-6);
        assert!(f.z.abs() < 1e-6);
    }

    #[test]
    fn altitude_clamps() {
        let view = LocalView::default();
        let mut c = Camera::new(Observer::from_degrees(0.0, 0.0, 2_460_000.5), view, 1.0);
        c.rotate_view(0.0, 100.0);
        assert!(c.view.altitude_rad <= ALT_LIMIT + 1e-6);
        c.rotate_view(0.0, -200.0);
        assert!(c.view.altitude_rad >= -ALT_LIMIT - 1e-6);
    }
}
