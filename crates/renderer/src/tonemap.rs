//! HDR scene buffer + adaptive tone-reproduction pass.
//!
//! Three-stage pipeline:
//!
//! 1. **Scene → HDR** — star, overlay and skyglow passes accumulate into
//!    an `Rgba16Float` colour attachment so Pogson's-law radiances and
//!    Spencer-PSF tails survive past the [0, 1] range a UNORM target would
//!    clip them to.
//! 2. **Luminance reduction → 1×1** — a fullscreen pass samples the HDR
//!    texture on a stratified grid, writes the log-average luminance to a
//!    1×1 `R32Float` target. This is the *scene adaptation luminance*
//!    `L_a` per Reinhard 2002 §3.2 (the photographic-exposure-metering
//!    convention) / Ferwerda 1996 §3 (the visual-adaptation convention).
//! 3. **Tonemap → final swapchain** — a fullscreen pass reads the HDR
//!    scene + the 1×1 adaptation target + the camera uniform (for the
//!    HDR-flux-to-cd/m² conversion), picks a photographic key by the
//!    Ferwerda 1996 / CIE 191:2010 mesopic regime, then applies the
//!    Reinhard 2002 §3.3 keyed operator (Eq. 4) with a soft white-point
//!    knee. The result lands on the host's sRGB swapchain.
//!
//! See `shaders/tonemap.wgsl` and `shaders/luminance.wgsl` for the per-
//! shader references and the full derivation. The Ferwerda 1996 rod/cone
//! TVI functions are implemented in `astronomy::photometry`; they motivate
//! the photopic-vs-scotopic key split here, but the *per-fragment* rod/cone
//! pathway separation (V'(λ)-weighted scotopic chroma) is scoped for the
//! Pattanaik 1998 multiscale upgrade in ROADMAP Phase 1'.

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

/// Format of the 1×1 scene-adaptation-luminance target.
///
/// `R32Float` because we store `log(luma + ε)` which can be very negative
/// (~-16 for blank sky regions); the extra dynamic range of 32-bit float
/// versus 16-bit avoids the latter's quantisation at the low end of the
/// scotopic regime.
const ADAPTATION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

/// Resources for the tone-reproduction post-process pass.
pub(crate) struct Tonemap {
    luminance_pipeline: wgpu::RenderPipeline,
    luminance_bind_group_layout: wgpu::BindGroupLayout,
    tonemap_pipeline: wgpu::RenderPipeline,
    tonemap_bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// The HDR scene texture. Created in [`Tonemap::new`] and recreated by
    /// [`Tonemap::resize`] whenever the framebuffer changes size.
    #[allow(dead_code)]
    hdr_texture: wgpu::Texture,
    hdr_view: wgpu::TextureView,
    /// 1×1 R32Float target holding the log-average luminance of the HDR
    /// scene, computed by the luminance-reduction pass.
    #[allow(dead_code)]
    adaptation_texture: wgpu::Texture,
    adaptation_view: wgpu::TextureView,
    luminance_bind_group: wgpu::BindGroup,
    tonemap_bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

impl Tonemap {
    /// Build the tone-reproduction stage.
    ///
    /// `camera_buffer` is the per-frame camera uniform owned by
    /// [`crate::renderer::Renderer`]; the tonemap pass needs to sample
    /// `magnitude_zeropoint` from it to perform the HDR-flux → cd/m²
    /// conversion that drives the Ferwerda mesopic-regime split. The
    /// pass keeps an internal bind group pointing at that buffer; if the
    /// buffer is ever recreated the renderer must rebuild the `Tonemap`.
    pub fn new(
        device: &wgpu::Device,
        final_format: wgpu::TextureFormat,
        camera_buffer: &wgpu::Buffer,
        width: u32,
        height: u32,
    ) -> Self {
        // ---------------------------------------------------------------
        // Luminance reduction pipeline: HDR -> 1×1 log-luma.
        // ---------------------------------------------------------------
        let luminance_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Luminance Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                }],
            });

        let luminance_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Luminance Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/luminance.wgsl").into()),
        });

        let luminance_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Luminance Pipeline Layout"),
                bind_group_layouts: &[Some(&luminance_bind_group_layout)],
                immediate_size: 0,
            });

        let luminance_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Luminance Pipeline"),
            layout: Some(&luminance_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &luminance_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &luminance_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: ADAPTATION_FORMAT,
                    // R32Float is not a blendable format on wgpu; the
                    // pass writes the single fragment value directly.
                    blend: None,
                    write_mask: wgpu::ColorWrites::RED,
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

        // ---------------------------------------------------------------
        // Tonemap pipeline: HDR + adaptation + camera -> swapchain.
        // ---------------------------------------------------------------
        let tonemap_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            // R32Float is not filterable on all backends;
                            // we sample it with textureLoad anyway.
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: Some(
                                std::num::NonZeroU64::new(std::mem::size_of::<
                                    crate::camera::CameraUniform,
                                >()
                                    as u64)
                                .unwrap(),
                            ),
                        },
                        count: None,
                    },
                ],
            });

        let tonemap_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Tonemap Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/tonemap.wgsl").into()),
        });

        let tonemap_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Tonemap Pipeline Layout"),
                bind_group_layouts: &[Some(&tonemap_bind_group_layout)],
                immediate_size: 0,
            });

        let tonemap_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Tonemap Pipeline"),
            layout: Some(&tonemap_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &tonemap_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &tonemap_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: final_format,
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
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let (hdr_texture, hdr_view, adaptation_texture, adaptation_view, luminance_bind_group) =
            create_render_targets(device, &luminance_bind_group_layout, width, height);

        let tonemap_bind_group = create_tonemap_bind_group(
            device,
            &tonemap_bind_group_layout,
            &hdr_view,
            &sampler,
            &adaptation_view,
            camera_buffer,
        );

        Self {
            luminance_pipeline,
            luminance_bind_group_layout,
            tonemap_pipeline,
            tonemap_bind_group_layout,
            sampler,
            hdr_texture,
            hdr_view,
            adaptation_texture,
            adaptation_view,
            luminance_bind_group,
            tonemap_bind_group,
            width,
            height,
        }
    }

    /// Recreate the HDR target at the new size. No-op when the size is
    /// unchanged so this is safe to call from per-frame paths. The
    /// `camera_buffer` argument is the same buffer passed to
    /// [`Tonemap::new`]; we re-bind it because the bind group is rebuilt
    /// in lock-step with the resized scene texture.
    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        camera_buffer: &wgpu::Buffer,
        width: u32,
        height: u32,
    ) {
        if width == self.width && height == self.height {
            return;
        }
        let (hdr_texture, hdr_view, adaptation_texture, adaptation_view, luminance_bind_group) =
            create_render_targets(device, &self.luminance_bind_group_layout, width, height);
        self.tonemap_bind_group = create_tonemap_bind_group(
            device,
            &self.tonemap_bind_group_layout,
            &hdr_view,
            &self.sampler,
            &adaptation_view,
            camera_buffer,
        );
        self.hdr_texture = hdr_texture;
        self.hdr_view = hdr_view;
        self.adaptation_texture = adaptation_texture;
        self.adaptation_view = adaptation_view;
        self.luminance_bind_group = luminance_bind_group;
        self.width = width;
        self.height = height;
    }

    /// View to bind as the colour attachment for the scene (skyglow +
    /// stars + overlays).
    pub fn scene_view(&self) -> &wgpu::TextureView {
        &self.hdr_view
    }

    /// Run the luminance-reduction pass (HDR → 1×1 log-average), then the
    /// tone-map pass (HDR + adaptation → `final_view`).
    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, final_view: &wgpu::TextureView) {
        // Pass A: log-average luminance into the 1×1 adaptation target.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Luminance Reduction Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.adaptation_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
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
            pass.set_pipeline(&self.luminance_pipeline);
            pass.set_bind_group(0, &self.luminance_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        // Pass B: tone-map HDR scene onto the final framebuffer.
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Tonemap Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: final_view,
                resolve_target: None,
                ops: wgpu::Operations {
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
        pass.set_pipeline(&self.tonemap_pipeline);
        pass.set_bind_group(0, &self.tonemap_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn create_render_targets(
    device: &wgpu::Device,
    luminance_bind_group_layout: &wgpu::BindGroupLayout,
    width: u32,
    height: u32,
) -> (
    wgpu::Texture,
    wgpu::TextureView,
    wgpu::Texture,
    wgpu::TextureView,
    wgpu::BindGroup,
) {
    // Guard against zero-sized framebuffers: wgpu validates strictly and a
    // resize-to-0 from a minimised window or pre-layout WASM canvas would
    // panic the device. Clamp to 1×1 — the next non-trivial resize replaces
    // it before the tonemap pass actually samples anything.
    let safe_width = width.max(1);
    let safe_height = height.max(1);
    let hdr_texture = device.create_texture(&wgpu::TextureDescriptor {
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
    let hdr_view = hdr_texture.create_view(&wgpu::TextureViewDescriptor::default());

    let adaptation_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Adaptation Luminance Texture"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: ADAPTATION_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let adaptation_view = adaptation_texture.create_view(&wgpu::TextureViewDescriptor::default());

    let luminance_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Luminance Bind Group"),
        layout: luminance_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&hdr_view),
        }],
    });

    (
        hdr_texture,
        hdr_view,
        adaptation_texture,
        adaptation_view,
        luminance_bind_group,
    )
}

fn create_tonemap_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    hdr_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    adaptation_view: &wgpu::TextureView,
    camera_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Tonemap Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(hdr_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(adaptation_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: camera_buffer.as_entire_binding(),
            },
        ],
    })
}
