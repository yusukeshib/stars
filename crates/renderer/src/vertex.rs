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

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
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
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
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

/// Convert a star's apparent magnitude into renderer parameters.
///
/// Returns `(size_px, brightness)`. Size scales sub-linearly with linear flux so
/// even faint stars stay above one pixel; brightness scales linearly with flux
/// (clamped) so the shader can attenuate the center intensity, giving 1st-mag
/// stars a perceptibly brighter core than 5th-mag stars.
pub fn magnitude_to_size(mag: f32) -> (f32, f32) {
    // Linear flux relative to magnitude 0: 2.5 mag dimmer = 10× less light.
    let flux = 10.0_f32.powf(-mag * 0.4);
    // Bigger billboards so the Gaussian halo has room; min 2px keeps faint
    // stars from disappearing into pixel snapping.
    let size = (6.0 * flux.powf(0.4)).clamp(2.0, 22.0);
    // Compressed peak intensity: a power < 1 tames the 1000× flux gap between
    // mag 6 and mag −1 while still giving 1st-mag stars a clearly brighter core.
    let brightness = (flux.powf(0.35) * 1.4).clamp(0.35, 2.5);
    (size, brightness)
}
