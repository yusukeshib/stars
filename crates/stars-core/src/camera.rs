use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub viewport_size: [f32; 2],
    pub _pad: [f32; 2],
}

pub struct Camera {
    pub azimuth: f32,
    pub elevation: f32,
    pub distance: f32,
    pub aspect: f32,
    pub fov_y: f32,
}

impl Camera {
    pub fn new(aspect: f32) -> Self {
        Self {
            azimuth: 0.0,
            elevation: 0.0,
            distance: 3.0,
            aspect,
            fov_y: std::f32::consts::FRAC_PI_4,
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        let eye = Vec3::new(
            self.distance * self.elevation.cos() * self.azimuth.cos(),
            self.distance * self.elevation.cos() * self.azimuth.sin(),
            self.distance * self.elevation.sin(),
        );
        Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Z)
    }

    pub fn projection_matrix(&self) -> Mat4 {
        Mat4::perspective_rh(self.fov_y, self.aspect, 0.1, 100.0)
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

    pub fn rotate(&mut self, dx: f32, dy: f32) {
        self.azimuth += dx;
        self.elevation = (self.elevation + dy).clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta * 0.1).clamp(0.5, 20.0);
    }
}
