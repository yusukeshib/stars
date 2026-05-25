//! Screen-space text labels rendered from a tiny built-in bitmap font atlas.
//!
//! The renderer keeps text as an overlay concern: labels are projected from the
//! same equatorial/local unit vectors as line overlays, then rasterized in LDR
//! screen space after tone mapping. This keeps labels readable without feeding
//! UI glyphs into the physical HDR sky pipeline.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec4};
use std::borrow::Cow;
use std::cell::Cell;
use wgpu::util::DeviceExt;

use crate::camera::{Camera, CameraUniform, PlanetUniforms, PLANET_UNIFORM_COUNT};
use crate::overlay::{OverlayConfig, OverlayKind};

include!(concat!(env!("OUT_DIR"), "/label_data.rs"));

const ASCII_FIRST: u8 = 32;
const ASCII_LAST: u8 = 126;
const ATLAS_COLS: u32 = 16;
const CELL_W: u32 = 8;
const CELL_H: u32 = 8;
const ATLAS_ROWS: u32 = ((ASCII_LAST - ASCII_FIRST + 1) as u32).div_ceil(ATLAS_COLS);
const ATLAS_W: u32 = ATLAS_COLS * CELL_W;
const ATLAS_H: u32 = ATLAS_ROWS * CELL_H;
const FONT_W: usize = 5;
const FONT_H: usize = 7;
const GLYPH_ADVANCE_PX: f32 = 6.0;
const FONT_SCALE: f32 = 1.0;
const MAX_TEXT_VERTICES: usize = 96_000;

const PLANET_LABELS: [&str; PLANET_UNIFORM_COUNT] = [
    "Mercury", "Venus", "Mars", "Jupiter", "Saturn", "Uranus", "Neptune",
];

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TextVertex {
    position_px: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

impl TextVertex {
    const OFFSET_POSITION: u64 = std::mem::offset_of!(Self, position_px) as u64;
    const OFFSET_UV: u64 = std::mem::offset_of!(Self, uv) as u64;
    const OFFSET_COLOR: u64 = std::mem::offset_of!(Self, color) as u64;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct TextUniform {
    viewport: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
enum LabelFrame {
    Equatorial,
    Local,
}

#[derive(Debug, Clone, Copy)]
struct ProjectedRect {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl ProjectedRect {
    fn overlaps(self, other: Self) -> bool {
        self.left < other.right
            && self.right > other.left
            && self.top < other.bottom
            && self.bottom > other.top
    }
}

#[derive(Debug, Clone, Copy)]
enum LabelPlacement {
    /// Anchor point is the center of the text rectangle.
    Centered,
    /// Anchor point is the labelled object; text starts just to its right.
    LeftAlignedToAnchor,
}

#[derive(Debug, Clone)]
struct LabelCandidate<'a> {
    frame: LabelFrame,
    position: [f32; 3],
    text: Cow<'a, str>,
    color: [f32; 4],
    priority: f32,
    placement: LabelPlacement,
}

#[derive(Debug, Clone, Copy)]
struct TextConfig {
    stars: bool,
    /// Solar-system body labels: Sun, Moon, and Mercury through Neptune.
    planets: bool,
    constellations: bool,
    cardinals: bool,
    degrees: bool,
    grid_step_deg: f64,
    opacity: f32,
}

impl TextConfig {
    fn from_overlay_config(config: &OverlayConfig) -> Self {
        Self {
            stars: config.layers.contains(&OverlayKind::StarLabels),
            planets: config.layers.contains(&OverlayKind::PlanetLabels),
            constellations: config.layers.contains(&OverlayKind::ConstellationLabels),
            cardinals: config.layers.contains(&OverlayKind::CardinalLabels),
            degrees: config.layers.contains(&OverlayKind::DegreeLabels),
            grid_step_deg: config.grid_step_deg.clamp(1.0, 90.0),
            // Text is a legibility layer, not radiance or translucent geometry.
            // Keep labels fully opaque even when the line-overlay opacity slider
            // is lowered.
            opacity: 1.0,
        }
    }
}

impl Default for TextConfig {
    fn default() -> Self {
        Self {
            stars: false,
            planets: false,
            constellations: false,
            cardinals: false,
            degrees: false,
            grid_step_deg: 15.0,
            opacity: 0.6,
        }
    }
}

pub(crate) struct TextRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    atlas_texture: wgpu::Texture,
    atlas_pixels: Vec<u8>,
    atlas_uploaded: Cell<bool>,
    vertex_count: Cell<u32>,
    config: TextConfig,
}

impl TextRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let atlas_pixels = build_font_atlas();
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Text Font Atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_W,
                height: ATLAS_H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Text Font Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text Uniform Buffer"),
            contents: bytemuck::bytes_of(&TextUniform {
                viewport: [1.0, 1.0, 0.0, 0.0],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Text Vertex Buffer"),
            size: (MAX_TEXT_VERTICES * std::mem::size_of::<TextVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(std::mem::size_of::<TextUniform>() as u64)
                                .unwrap(),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Text Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/text.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: TextVertex::OFFSET_POSITION,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: TextVertex::OFFSET_UV,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: TextVertex::OFFSET_COLOR,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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

        Self {
            pipeline,
            bind_group,
            uniform_buffer,
            vertex_buffer,
            atlas_texture,
            atlas_pixels,
            atlas_uploaded: Cell::new(false),
            vertex_count: Cell::new(0),
            config: TextConfig::default(),
        }
    }

    pub fn set_config(&mut self, config: &OverlayConfig) {
        self.config = TextConfig::from_overlay_config(config);
    }

    pub fn update_camera(
        &self,
        queue: &wgpu::Queue,
        camera: &Camera,
        camera_uniform: &CameraUniform,
        planet_uniforms: &PlanetUniforms,
        width: u32,
        height: u32,
    ) {
        if !self.atlas_uploaded.get() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &self.atlas_pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(ATLAS_W),
                    rows_per_image: Some(ATLAS_H),
                },
                wgpu::Extent3d {
                    width: ATLAS_W,
                    height: ATLAS_H,
                    depth_or_array_layers: 1,
                },
            );
            self.atlas_uploaded.set(true);
        }

        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&TextUniform {
                viewport: [width as f32, height as f32, 0.0, 0.0],
            }),
        );

        let mut vertices = Vec::new();
        if self.config.opacity <= 0.0 || width == 0 || height == 0 {
            self.vertex_count.set(0);
            return;
        }

        let mut candidates = Vec::new();
        self.collect_candidates(&mut candidates, camera_uniform, planet_uniforms);
        candidates.sort_by(|a, b| a.priority.total_cmp(&b.priority));

        let vp_eq = camera.overlay_matrix_equatorial();
        let vp_local = camera.overlay_matrix_local();
        let projection_params = camera.overlay_projection_params();
        let viewport = [width as f32, height as f32];
        let mut placed = Vec::new();
        for candidate in candidates {
            if vertices.len() + text_vertex_count(&candidate.text) > MAX_TEXT_VERTICES {
                break;
            }
            let vp = match candidate.frame {
                LabelFrame::Equatorial => vp_eq,
                LabelFrame::Local => vp_local,
            };
            let Some(anchor) = project(candidate.position, vp, projection_params, viewport) else {
                continue;
            };
            let size = text_size(&candidate.text);
            let rect = text_rect(anchor, size, candidate.placement);
            if rect.right < 0.0
                || rect.left > viewport[0]
                || rect.bottom < 0.0
                || rect.top > viewport[1]
            {
                continue;
            }
            if placed.iter().any(|other| rect.overlaps(*other)) {
                continue;
            }
            placed.push(rect);
            append_text(
                &mut vertices,
                &candidate.text,
                rect.left,
                rect.top,
                candidate.color,
            );
        }

        if vertices.is_empty() {
            self.vertex_count.set(0);
            return;
        }
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        self.vertex_count.set(vertices.len() as u32);
    }

    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        let count = self.vertex_count.get();
        if count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..count, 0..1);
    }

    fn collect_candidates<'a>(
        &'a self,
        out: &mut Vec<LabelCandidate<'a>>,
        camera_uniform: &'a CameraUniform,
        planet_uniforms: &'a PlanetUniforms,
    ) {
        let alpha = self.config.opacity;
        if self.config.planets {
            let sun = camera_uniform.sun_eq_radius;
            if sun[3] > 0.0 {
                out.push(LabelCandidate {
                    frame: LabelFrame::Equatorial,
                    position: [sun[0], sun[1], sun[2]],
                    text: Cow::Borrowed("Sun"),
                    color: [1.0, 0.86, 0.34, alpha],
                    priority: -260.0,
                    placement: LabelPlacement::LeftAlignedToAnchor,
                });
            }
            let moon = camera_uniform.moon_eq_illuminance;
            if camera_uniform.moon_disk[0] > 0.0 {
                out.push(LabelCandidate {
                    frame: LabelFrame::Equatorial,
                    position: [moon[0], moon[1], moon[2]],
                    text: Cow::Borrowed("Moon"),
                    color: [0.86, 0.88, 0.92, alpha],
                    priority: -240.0,
                    placement: LabelPlacement::LeftAlignedToAnchor,
                });
            }
        }
        if self.config.planets && planet_uniforms.params[1] > 0.5 {
            for (idx, label) in PLANET_LABELS.iter().enumerate() {
                let p = planet_uniforms.eq_radius[idx];
                if p[3] <= 0.0 {
                    continue;
                }
                out.push(LabelCandidate {
                    frame: LabelFrame::Equatorial,
                    position: [p[0], p[1], p[2]],
                    text: Cow::Borrowed(label),
                    color: [1.0, 0.92, 0.55, alpha],
                    priority: -200.0 + planet_uniforms.rgb_magnitude[idx][3],
                    placement: LabelPlacement::LeftAlignedToAnchor,
                });
            }
        }
        if self.config.cardinals {
            for (az, label) in [
                (0.0_f32, "N"),
                (std::f32::consts::FRAC_PI_2, "E"),
                (std::f32::consts::PI, "S"),
                (3.0 * std::f32::consts::FRAC_PI_2, "W"),
            ] {
                out.push(LabelCandidate {
                    frame: LabelFrame::Local,
                    position: [az.sin(), az.cos(), 0.06],
                    text: Cow::Borrowed(label),
                    color: [1.0, 0.86, 0.42, alpha],
                    priority: -150.0,
                    placement: LabelPlacement::Centered,
                });
            }
        }
        if self.config.stars {
            for label in STAR_LABELS {
                out.push(LabelCandidate {
                    frame: LabelFrame::Equatorial,
                    position: label.position,
                    text: Cow::Borrowed(label.text),
                    color: [0.86, 0.93, 1.0, alpha],
                    priority: label.magnitude,
                    placement: LabelPlacement::LeftAlignedToAnchor,
                });
            }
        }
        if self.config.constellations {
            for label in CONSTELLATION_LABELS {
                out.push(LabelCandidate {
                    frame: LabelFrame::Equatorial,
                    position: label.position,
                    text: Cow::Borrowed(label.text),
                    color: [0.55, 0.73, 1.0, alpha * 0.85],
                    priority: 50.0,
                    placement: LabelPlacement::Centered,
                });
            }
        }
        if self.config.degrees {
            let step = self.config.grid_step_deg.max(5.0).round() as i32;
            let mut az = step;
            while az < 360 {
                if az % 90 != 0 {
                    let rad = (az as f32).to_radians();
                    out.push(LabelCandidate {
                        frame: LabelFrame::Local,
                        position: [rad.sin(), rad.cos(), 0.025],
                        text: Cow::Owned(degree_label(az)),
                        color: [0.62, 0.78, 1.0, alpha * 0.8],
                        priority: 100.0 + az as f32 / 360.0,
                        placement: LabelPlacement::Centered,
                    });
                }
                az += step;
            }
            let mut alt = step;
            while alt < 90 {
                for sign in [1.0_f32, -1.0] {
                    let alt_rad = (alt as f32 * sign).to_radians();
                    let (s, c) = alt_rad.sin_cos();
                    out.push(LabelCandidate {
                        frame: LabelFrame::Local,
                        position: [c, 0.0, s],
                        text: Cow::Owned(degree_label((alt as f32 * sign) as i32)),
                        color: [0.62, 0.78, 1.0, alpha * 0.8],
                        priority: 110.0 + alt as f32 / 90.0,
                        placement: LabelPlacement::Centered,
                    });
                }
                alt += step;
            }
        }
    }
}

fn degree_label(deg: i32) -> String {
    format!("{deg} DEG")
}

fn project(
    position: [f32; 3],
    view_proj: Mat4,
    projection_params: [f32; 4],
    viewport: [f32; 2],
) -> Option<[f32; 2]> {
    let ndc = if projection_params[3] > 0.5 {
        let view_dir =
            (view_proj * Vec4::new(position[0], position[1], position[2], 0.0)).truncate();
        all_sky_project_from_view_dir(view_dir, projection_params)
    } else {
        let clip = view_proj * Vec4::new(position[0], position[1], position[2], 1.0);
        if clip.w <= 0.0 || !clip.w.is_finite() {
            return None;
        }
        clip.truncate() / clip.w
    };
    if ndc.z < -1.0 || ndc.z > 1.0 || ndc.x < -1.2 || ndc.x > 1.2 || ndc.y < -1.2 || ndc.y > 1.2 {
        return None;
    }
    Some([
        (ndc.x * 0.5 + 0.5) * viewport[0],
        (0.5 - ndc.y * 0.5) * viewport[1],
    ])
}

fn all_sky_project_from_view_dir(view_dir: glam::Vec3, projection_params: [f32; 4]) -> glam::Vec3 {
    let d = view_dir.normalize_or_zero();
    let lon = d.x.atan2(-d.z);
    let lat = d.y.clamp(-1.0, 1.0).asin();
    let mut p = mollweide_project(lon, lat);
    if projection_params[0] >= 2.5 {
        p = hammer_project(lon, lat);
    } else if projection_params[0] >= 1.5 {
        p = aitoff_project(lon, lat);
    }
    glam::Vec3::new(
        p[0] * projection_params[1],
        p[1] * projection_params[2],
        0.5,
    )
}

fn mollweide_project(lon: f32, lat: f32) -> [f32; 2] {
    let mut theta = lat;
    for _ in 0..6 {
        let f = 2.0 * theta + (2.0 * theta).sin() - std::f32::consts::PI * lat.sin();
        let fp = 2.0 + 2.0 * (2.0 * theta).cos();
        theta = (theta - f / fp.max(1.0e-4))
            .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    }
    [(lon / std::f32::consts::PI) * theta.cos(), theta.sin()]
}

fn aitoff_project(lon: f32, lat: f32) -> [f32; 2] {
    let half_lon = 0.5 * lon;
    let alpha = (lat.cos() * half_lon.cos()).clamp(-1.0, 1.0).acos();
    let sinc = if alpha.abs() < 1.0e-6 {
        1.0
    } else {
        alpha.sin() / alpha.max(1.0e-6)
    };
    [
        2.0 * lat.cos() * half_lon.sin() / (std::f32::consts::PI * sinc),
        lat.sin() / (std::f32::consts::FRAC_PI_2 * sinc),
    ]
}

fn hammer_project(lon: f32, lat: f32) -> [f32; 2] {
    let half_lon = 0.5 * lon;
    let denom = (1.0 + lat.cos() * half_lon.cos()).max(1.0e-6).sqrt();
    [lat.cos() * half_lon.sin() / denom, lat.sin() / denom]
}

fn text_size(text: &str) -> [f32; 2] {
    let chars = text.chars().filter(|c| *c != '\n').count() as f32;
    [
        chars * GLYPH_ADVANCE_PX * FONT_SCALE,
        FONT_H as f32 * FONT_SCALE,
    ]
}

fn text_rect(anchor: [f32; 2], size: [f32; 2], placement: LabelPlacement) -> ProjectedRect {
    const ANCHOR_GAP_PX: f32 = 4.0;
    let (left, top) = match placement {
        LabelPlacement::Centered => (anchor[0] - size[0] * 0.5, anchor[1] - size[1] * 0.5),
        LabelPlacement::LeftAlignedToAnchor => {
            (anchor[0] + ANCHOR_GAP_PX, anchor[1] - size[1] * 0.5)
        }
    };
    ProjectedRect {
        left,
        top,
        right: left + size[0],
        bottom: top + size[1],
    }
}

fn text_vertex_count(text: &str) -> usize {
    text.chars().filter(|c| *c != ' ' && *c != '\n').count() * 6
}

fn append_text(vertices: &mut Vec<TextVertex>, text: &str, x: f32, y: f32, color: [f32; 4]) {
    let mut pen_x = x;
    for ch in text.chars() {
        if ch == ' ' {
            pen_x += GLYPH_ADVANCE_PX * FONT_SCALE;
            continue;
        }
        let ascii = normalize_char(ch);
        let (u0, v0, u1, v1) = glyph_uv(ascii);
        let x0 = pen_x;
        let y0 = y;
        let x1 = pen_x + CELL_W as f32 * FONT_SCALE;
        let y1 = y + CELL_H as f32 * FONT_SCALE;
        vertices.extend_from_slice(&[
            TextVertex {
                position_px: [x0, y0],
                uv: [u0, v0],
                color,
            },
            TextVertex {
                position_px: [x1, y0],
                uv: [u1, v0],
                color,
            },
            TextVertex {
                position_px: [x1, y1],
                uv: [u1, v1],
                color,
            },
            TextVertex {
                position_px: [x0, y0],
                uv: [u0, v0],
                color,
            },
            TextVertex {
                position_px: [x1, y1],
                uv: [u1, v1],
                color,
            },
            TextVertex {
                position_px: [x0, y1],
                uv: [u0, v1],
                color,
            },
        ]);
        pen_x += GLYPH_ADVANCE_PX * FONT_SCALE;
    }
}

fn glyph_uv(ascii: u8) -> (f32, f32, f32, f32) {
    let idx = (ascii.clamp(ASCII_FIRST, ASCII_LAST) - ASCII_FIRST) as u32;
    let col = idx % ATLAS_COLS;
    let row = idx / ATLAS_COLS;
    let u0 = (col * CELL_W) as f32 / ATLAS_W as f32 + 0.5 / ATLAS_W as f32;
    let v0 = (row * CELL_H) as f32 / ATLAS_H as f32 + 0.5 / ATLAS_H as f32;
    let u1 = ((col + 1) * CELL_W) as f32 / ATLAS_W as f32 - 0.5 / ATLAS_W as f32;
    let v1 = ((row + 1) * CELL_H) as f32 / ATLAS_H as f32 - 0.5 / ATLAS_H as f32;
    (u0, v0, u1, v1)
}

fn build_font_atlas() -> Vec<u8> {
    let mut pixels = vec![0_u8; (ATLAS_W * ATLAS_H) as usize];
    for ascii in ASCII_FIRST..=ASCII_LAST {
        let idx = (ascii - ASCII_FIRST) as u32;
        let origin_x = (idx % ATLAS_COLS) * CELL_W;
        let origin_y = (idx / ATLAS_COLS) * CELL_H;
        let glyph = glyph_rows(ascii as char);
        for (y, row) in glyph.iter().enumerate() {
            for x in 0..FONT_W {
                if (row >> (FONT_W - 1 - x)) & 1 == 1 {
                    let px = origin_x + x as u32 + 1;
                    let py = origin_y + y as u32;
                    pixels[(py * ATLAS_W + px) as usize] = 255;
                }
            }
        }
    }
    pixels
}

fn normalize_char(ch: char) -> u8 {
    let upper = ch.to_ascii_uppercase();
    if upper.is_ascii() {
        let b = upper as u8;
        if (ASCII_FIRST..=ASCII_LAST).contains(&b) {
            return b;
        }
    }
    b'?'
}

fn glyph_rows(ch: char) -> [u8; FONT_H] {
    match ch.to_ascii_uppercase() {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '/' => [
            0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000,
        ],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        ':' => [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
        _ => [0; FONT_H],
    }
}

#[cfg(test)]
fn collect_static_test_candidates<'a>(out: &mut Vec<LabelCandidate<'a>>) {
    for label in STAR_LABELS {
        out.push(LabelCandidate {
            frame: LabelFrame::Equatorial,
            position: label.position,
            text: Cow::Borrowed(label.text),
            color: [1.0; 4],
            priority: label.magnitude,
            placement: LabelPlacement::LeftAlignedToAnchor,
        });
    }
    for label in CONSTELLATION_LABELS {
        out.push(LabelCandidate {
            frame: LabelFrame::Equatorial,
            position: label.position,
            text: Cow::Borrowed(label.text),
            color: [1.0; 4],
            priority: 50.0,
            placement: LabelPlacement::Centered,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_label_catalogs_are_populated() {
        assert_eq!(STAR_LABELS.len(), 50);
        assert!(CONSTELLATION_LABELS.len() >= 80);
        assert!(STAR_LABELS
            .iter()
            .any(|label| label.text.contains("Sirius")));
        assert!(CONSTELLATION_LABELS
            .iter()
            .any(|label| label.text == "Orion"));
    }

    #[test]
    fn atlas_has_expected_size_and_visible_glyphs() {
        let atlas = build_font_atlas();
        assert_eq!(atlas.len(), (ATLAS_W * ATLAS_H) as usize);
        assert!(atlas.contains(&255));
        assert_ne!(glyph_rows('A'), [0; FONT_H]);
    }

    #[test]
    fn text_sizing_uses_fixed_advance() {
        assert_eq!(text_vertex_count("A B"), 12);
        assert_eq!(text_size("AB")[0], 2.0 * GLYPH_ADVANCE_PX * FONT_SCALE);
    }

    #[test]
    fn object_labels_start_to_the_right_of_anchor() {
        let rect = text_rect(
            [100.0, 50.0],
            [30.0, 10.0],
            LabelPlacement::LeftAlignedToAnchor,
        );
        assert!(rect.left > 100.0);
        assert_eq!(rect.top, 45.0);
        let centered = text_rect([100.0, 50.0], [30.0, 10.0], LabelPlacement::Centered);
        assert_eq!(centered.left, 85.0);
    }

    #[test]
    fn all_sky_label_projection_matches_map_center() {
        for mode in [1.0, 2.0, 3.0] {
            let ndc = all_sky_project_from_view_dir(
                glam::Vec3::new(0.0, 0.0, -1.0),
                [mode, 1.0, 1.0, 1.0],
            );
            assert!(ndc.x.abs() < 1.0e-6);
            assert!(ndc.y.abs() < 1.0e-6);
        }
    }

    #[test]
    fn bright_star_labels_sort_before_constellations() {
        let mut labels = Vec::new();
        collect_static_test_candidates(&mut labels);
        labels.sort_by(|a, b| a.priority.total_cmp(&b.priority));
        assert!(labels.first().unwrap().priority < 5.0);
    }
}
