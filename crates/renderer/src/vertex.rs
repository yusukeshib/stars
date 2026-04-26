use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct QuadVertex {
    pub position: [f32; 2],
}

impl QuadVertex {
    pub const VERTICES: &[Self] = &[
        Self {
            position: [-1.0, -1.0],
        },
        Self {
            position: [1.0, -1.0],
        },
        Self {
            position: [1.0, 1.0],
        },
        Self {
            position: [-1.0, 1.0],
        },
    ];

    pub const INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

    pub(crate) fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2,
            }],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct StarInstance {
    pub position: [f32; 3],
    pub size: f32,
    pub color: [f32; 3],
    pub brightness: f32,
}

impl StarInstance {
    pub(crate) fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 28,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

/// Per-star rendering parameters derived from apparent magnitude.
#[derive(Debug, Clone, Copy)]
pub struct RenderParams {
    /// Falloff radius of the star's billboard, in screen-space pixels. The
    /// billboard quad spans ±radius_px around the star's projected center; the
    /// shader's Gaussian core fades to zero near this edge.
    pub radius_px: f32,
    /// Multiplier on the shader's center intensity. >1 saturates additively.
    pub brightness: f32,
}

/// Convert a star's apparent magnitude into renderer parameters.
///
/// Radius scales sub-linearly with linear flux so even faint stars stay above
/// one pixel; brightness scales as flux^0.35 so 1st-mag stars have a clearly
/// brighter core than 5th-mag without the eye-watering 1000× raw flux gap.
pub fn magnitude_to_render_params(mag: f32) -> RenderParams {
    // Linear flux relative to magnitude 0: 2.5 mag dimmer = 10× less light.
    let flux = 10.0_f32.powf(-mag * 0.4);
    let radius_px = (6.0 * flux.powf(0.4)).clamp(2.0, 22.0);
    let brightness = (flux.powf(0.35) * 1.4).clamp(0.35, 2.5);
    RenderParams {
        radius_px,
        brightness,
    }
}
