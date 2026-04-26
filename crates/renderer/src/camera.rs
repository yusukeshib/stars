use astronomy::{equatorial_to_horizontal_matrix, lmst_radians, Observer};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub viewport_size: [f32; 2],
    pub _pad: [f32; 2],
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
    fn equatorial_to_horizontal(&self) -> Mat4 {
        let lst = lmst_radians(self.observer.julian_date, self.observer.longitude_rad);
        equatorial_to_horizontal_matrix(self.observer.latitude_rad, lst)
    }

    /// Forward direction (in local ENU) the camera is looking at.
    fn forward_local(&self) -> Vec3 {
        let (sa, ca) = self.view.azimuth_rad.sin_cos();
        let (sp, cp) = self.view.altitude_rad.sin_cos();
        Vec3::new(sa * cp, ca * cp, sp)
    }

    pub fn view_matrix(&self) -> Mat4 {
        // look_at_rh derives screen-right from (forward × up); using local zenith as
        // "up" keeps the horizon level on screen. Altitude is clamped elsewhere to
        // avoid gimbal lock when looking straight up/down.
        let view_in_local = Mat4::look_at_rh(Vec3::ZERO, self.forward_local(), Vec3::Z);
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

    /// Drag-style interactive rotation: `daz` scrolls azimuth (East-positive),
    /// `dalt` raises altitude. Altitude is clamped just shy of ±π/2 to avoid gimbal lock.
    pub fn rotate_view(&mut self, daz: f32, dalt: f32) {
        self.view.azimuth_rad = (self.view.azimuth_rad + daz).rem_euclid(std::f32::consts::TAU);
        self.view.altitude_rad = (self.view.altitude_rad + dalt).clamp(-ALT_LIMIT, ALT_LIMIT);
    }

    /// Multiplicative FOV zoom. `factor < 1` zooms in (narrower FOV).
    pub fn zoom_fov(&mut self, factor: f32) {
        self.view.fov_y_rad =
            (self.view.fov_y_rad * factor).clamp(5.0_f32.to_radians(), 120.0_f32.to_radians());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observer() -> Observer {
        Observer::from_degrees(35.0, 139.0, 2_460_000.5)
    }

    #[test]
    fn view_matrix_projects_north_horizon_to_forward() {
        // Camera looking north at horizon: a star sitting in the local +Y (north)
        // direction (after equatorial→horizontal) should project to clip −Z.
        let view = LocalView {
            azimuth_rad: 0.0,
            altitude_rad: 0.0,
            fov_y_rad: std::f32::consts::FRAC_PI_4,
        };
        let cam = Camera::new(observer(), view, 16.0 / 9.0);
        // The forward vector in local ENU is exactly the look-at target.
        let m = cam.view_matrix();
        // Stars at the equatorial direction that maps to local +Y should land on -Z in view.
        let local_north = cam.equatorial_to_horizontal().transpose() * Vec3::Y.extend(0.0);
        let v = m * local_north;
        assert!(
            v.z < 0.0,
            "north star should be in front of camera, got z={}",
            v.z
        );
    }

    #[test]
    fn altitude_clamps() {
        let mut cam = Camera::new(observer(), LocalView::default(), 1.0);
        cam.rotate_view(0.0, 100.0);
        assert!(cam.view.altitude_rad <= ALT_LIMIT + 1e-6);
        cam.rotate_view(0.0, -200.0);
        assert!(cam.view.altitude_rad >= -ALT_LIMIT - 1e-6);
    }

    #[test]
    fn azimuth_wraps_to_zero_two_pi() {
        let mut cam = Camera::new(observer(), LocalView::default(), 1.0);
        cam.rotate_view(-0.5, 0.0);
        assert!((0.0..std::f32::consts::TAU).contains(&cam.view.azimuth_rad));
    }
}
