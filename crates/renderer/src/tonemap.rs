//! HDR scene buffer + tone-reproduction pass.
//!
//! The star and overlay passes write to a floating-point colour attachment
//! (`Rgba16Float`) so Pogson's-law radiances and accumulated PSF tails are
//! preserved past the [0, 1] range a UNORM target would clip them to. This
//! module owns that intermediate texture and the fullscreen pass that maps
//! it onto the final sRGB framebuffer via a luminance-preserving Reinhard
//! curve.
//!
//! See `shaders/tonemap.wgsl` for the operator and the references.

/// Format of the intermediate scene buffer.
///
/// `Rgba16Float` is the smallest wgpu format guaranteed by
/// `wgpu::Features::empty()` to support both `RENDER_ATTACHMENT` *and*
/// `TEXTURE_BINDING` with linear filtering across native and WebGPU.
/// 16-bit float gives ~3 decades of usable dynamic range above 1.0 before
/// running out of mantissa, which more than covers the brightest star
/// (Sirius peaks at ~60× the limiting-magnitude reference) plus PSF
/// accumulation headroom.
pub(crate) const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Resources for the tone-reproduction post-process pass.
pub(crate) struct Tonemap {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The HDR scene texture. Created in [`Tonemap::new`] and recreated by
    /// [`Tonemap::resize`] whenever the framebuffer changes size.
    hdr_texture: wgpu::Texture,
    hdr_view: wgpu::TextureView,
    /// Bind group pointing at `hdr_view` + `sampler`. Recreated together
    /// with the texture so the pass always samples the live attachment.
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl Tonemap {
    pub fn new(
        device: &wgpu::Device,
        final_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Tonemap Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Tonemap Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/tonemap.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Tonemap Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Tonemap Pipeline"),
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
                    format: final_format,
                    // Opaque blit — the tonemap pass fully replaces the
                    // framebuffer contents (we cleared to black in the
                    // scene pass already).
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Tonemap Sampler"),
            // We sample 1:1 (texel-aligned) in normal use; linear filtering
            // is only insurance against fractional viewport sizes.
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let (hdr_texture, hdr_view, bind_group) =
            create_hdr_target(device, &bind_group_layout, &sampler, width, height);

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            hdr_texture,
            hdr_view,
            bind_group,
            width,
            height,
        }
    }

    /// Recreate the HDR target at the new size. No-op when the size is
    /// unchanged so this is safe to call from per-frame paths.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }
        let (hdr_texture, hdr_view, bind_group) = create_hdr_target(
            device,
            &self.bind_group_layout,
            &self.sampler,
            width,
            height,
        );
        self.hdr_texture = hdr_texture;
        self.hdr_view = hdr_view;
        self.bind_group = bind_group;
        self.width = width;
        self.height = height;
    }

    /// View to bind as the colour attachment for the scene (stars + overlays).
    pub fn scene_view(&self) -> &wgpu::TextureView {
        &self.hdr_view
    }

    /// Run the tonemap pass: samples the HDR scene, writes to `final_view`.
    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, final_view: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Tonemap Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: final_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    // We fully cover the framebuffer; the load op is just
                    // formality. Use Clear(0) to make the intent explicit
                    // and so a malformed scene texture can't bleed through.
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn create_hdr_target(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
    // Guard against zero-sized framebuffers: wgpu validates strictly and a
    // resize-to-0 from a minimised window or pre-layout WASM canvas would
    // panic the device. Clamp to 1×1 — the next non-trivial resize replaces
    // it before the tonemap pass actually samples anything.
    let safe_width = width.max(1);
    let safe_height = height.max(1);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("HDR Scene Texture"),
        size: wgpu::Extent3d {
            width: safe_width,
            height: safe_height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Tonemap Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    (texture, view, bind_group)
}
