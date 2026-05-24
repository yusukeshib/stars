//! Diffuse night-sky surface-brightness pass.
//!
//! Renders the integrated-starlight (+ diffuse galactic light) glow into
//! the HDR scene buffer before the star and overlay passes draw on top.
//! See `shaders/skyglow.wgsl` for the model and references; the canonical
//! Rust-side implementation of the same analytic fit lives in
//! `astronomy::skyglow`.
//!
//! The pass uses the existing camera bind group (it consumes the camera's
//! `inv_view_proj`, per-pixel solid angle, magnitude zeropoint, extinction
//! coefficients, and zenith direction), so there are no new uniforms to
//! own — only the pipeline itself.

use crate::tonemap::HDR_FORMAT;

/// Resources for the skyglow fullscreen pass.
pub(crate) struct Skyglow {
    pipeline: wgpu::RenderPipeline,
}

impl Skyglow {
    pub fn new(device: &wgpu::Device, camera_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Skyglow Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/skyglow.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Skyglow Pipeline Layout"),
            bind_group_layouts: &[Some(camera_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skyglow Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    // Opaque blit — the skyglow pass writes the diffuse-sky
                    // baseline directly to the HDR buffer. Subsequent star
                    // and overlay passes use additive blending on top.
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self { pipeline }
    }

    /// Run the skyglow pass into `view`. The pass clears its render target
    /// to black via the `LoadOp::Clear` set by the caller's `RenderPass`;
    /// it does not modify any GPU state outside `pass`.
    pub fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, camera_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
