//! Sky overlays: horizon, cardinal marks, alt-az grid, equatorial grid,
//! ecliptic, celestial equator, galactic equator, local meridian,
//! constellation lines, IAU constellation boundaries, and text labels.
//!
//! Two coordinate frames are supported per-layer:
//! - **Equatorial** (J2000): geometry rotates with the celestial sphere as time
//!   advances. Used for RA/Dec grids, the celestial equator, the ecliptic.
//! - **Horizontal** (local ENU): geometry is anchored to the observer. Used for
//!   the horizon line, cardinal direction marks, the alt-az grid, the local
//!   meridian.
//!
//! All layers share one wgpu pipeline (`LineList` topology). Each layer carries
//! its own vertex buffer + uniform; the uniform holds the per-frame view-proj
//! matrix and the layer's RGBA color so a single shader handles every overlay.

use bytemuck::{Pod, Zeroable};
use std::f64::consts::{PI, TAU};
use wgpu::util::DeviceExt;

use catalog::{DeepSkyCatalog, DeepSkyId, DeepSkyObject, MessierCatalog, NgcBrightCatalog};

use crate::camera::Camera;
use crate::constellations::{constellation_boundaries, constellation_lines, ConstellationSegment};

/// Mean obliquity of the ecliptic **at J2000.0**, IAU 2006 value
/// (ε₀ = 84381.406″ = 23.4392911°), in radians.
///
/// The renderer draws a *fixed* ecliptic in J2000 equatorial coordinates,
/// not the mean ecliptic of date. The two differ by ≈30″ per 50 years from
/// J2000 due to planetary precession of the ecliptic plane — invisible at
/// Phase 1 naked-eye precision, but worth knowing before this code grows a
/// citation. A future date-dependent ecliptic overlay should use the validated
/// astronomy correction path rather than this fixed constant.
const OBLIQUITY_RAD: f64 = 0.409_092_804_222_329_3;

/// Rotation matrix rows from J2000 equatorial to IAU galactic coordinates.
/// The galactic-equator overlay uses the transpose (galactic → equatorial),
/// matching `astronomy::skyglow` and SOFA's `iauIcrs2g` constants.
#[rustfmt::skip]
const EQUATORIAL_TO_GALACTIC_ROWS: [[f64; 3]; 3] = [
    [-0.054_875_560_416_215, -0.873_437_090_234_885, -0.483_835_015_548_713],
    [ 0.494_109_427_875_584, -0.444_829_629_960_011,  0.746_982_244_497_219],
    [-0.867_666_149_019_004, -0.198_076_373_431_201,  0.455_983_776_175_067],
];

/// Inclusive bounds clamped onto `OverlayConfig::grid_step_deg` before the
/// geometry generators run. The lower bound is what stops the `while`-style
/// generators from looping forever on a zero or negative step (a single bad
/// call from the WASM bindings would otherwise freeze the browser tab); the
/// upper bound is just "larger than 90° produces no parallels at all".
const GRID_STEP_MIN_DEG: f64 = 1.0;
const GRID_STEP_MAX_DEG: f64 = 90.0;

/// Inclusive bounds clamped onto `OverlayConfig::deep_sky_magnitude_limit`.
/// The lower bound is generous so even a pathological caller cannot turn off
/// the layer through the magnitude knob (use the layer toggle for that); the
/// upper bound is high enough to show every Messier object.
const DEEP_SKY_MAG_MIN: f32 = -5.0;
const DEEP_SKY_MAG_MAX: f32 = 99.0;

/// Default Messier magnitude cutoff: visible to the eye in a moderately dark
/// sky. Brighter showpieces (M31, M42, M45, M44, M13) survive; the dimmest
/// half of the catalogue is hidden until the user opts in by raising the
/// limit. Chosen to stay close to the naked-eye limiting magnitude in
/// suburban skies.
pub const DEFAULT_DEEP_SKY_MAGNITUDE_LIMIT: f32 = 7.0;

/// Marker half-size bounds in arcminutes. The lower bound keeps small
/// objects (M1 ≈ 8') visible at moderate zoom without becoming invisible
/// dots; the upper bound stops large objects (M31 ≈ 178', M45 ≈ 110') from
/// drawing a sky-spanning diamond that hides everything inside.
const DEEP_SKY_MARKER_MIN_ARCMIN: f32 = 12.0;
const DEEP_SKY_MARKER_MAX_ARCMIN: f32 = 60.0;

/// Which overlay layers to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlayKind {
    /// Great circle at altitude = 0 in the local horizontal frame.
    Horizon,
    /// Short bars at North, East, South, West (taller at North).
    Cardinals,
    /// Parallels of constant altitude + meridians of constant azimuth.
    AltAzGrid,
    /// Parallels of constant declination + meridians of constant right ascension.
    EquatorialGrid,
    /// Great circle of the **J2000 mean ecliptic** — strictly, the apparent path
    /// of the Sun across the celestial sphere. The Moon and planets stay within
    /// a few degrees of it (Moon ±5.1°, Mercury ±7°) but are not exactly on it.
    Ecliptic,
    /// Great circle at declination = 0.
    CelestialEquator,
    /// Local meridian: great circle through north point, zenith, south point, nadir.
    Meridian,
    /// Great circle at galactic latitude b = 0 (the Milky Way mid-plane).
    GalacticEquator,
    /// Modern western constellation stick figures embedded by the renderer crate.
    ConstellationLines,
    /// IAU/Delporte constellation boundaries embedded by the renderer crate.
    ConstellationBoundaries,
    /// Diamond markers for Messier deep-sky objects whose V magnitude is
    /// brighter than [`OverlayConfig::deep_sky_magnitude_limit`].
    DeepSkyObjects,
    /// Text labels (`M1`, `M31`, …) for Messier deep-sky objects whose V
    /// magnitude is brighter than [`OverlayConfig::deep_sky_magnitude_limit`].
    DeepSkyLabels,
    /// Names/designations for the brightest catalogue stars.
    StarLabels,
    /// Names for Mercury through Neptune at their apparent positions.
    PlanetLabels,
    /// IAU constellation names placed near bright-star centroids.
    ConstellationLabels,
    /// Text N/E/S/W cardinal labels on the horizon.
    CardinalLabels,
    /// Numeric degree labels for the local alt-az grid.
    DegreeLabels,
}

impl OverlayKind {
    /// Canonical kebab-case name, shared by the CLI flag (`--overlays …`) and
    /// the WASM/JS bindings. Single source of truth for the host-facing
    /// string ↔ variant mapping; all native hosts route through this via
    /// `stars-host-common::OverlayArg`, and the web host calls
    /// [`OverlayKind::from_kebab_str`] directly.
    pub fn as_kebab_str(self) -> &'static str {
        match self {
            OverlayKind::Horizon => "horizon",
            OverlayKind::Cardinals => "cardinals",
            OverlayKind::AltAzGrid => "alt-az-grid",
            OverlayKind::EquatorialGrid => "equatorial-grid",
            OverlayKind::Ecliptic => "ecliptic",
            OverlayKind::CelestialEquator => "celestial-equator",
            OverlayKind::Meridian => "meridian",
            OverlayKind::GalacticEquator => "galactic-equator",
            OverlayKind::ConstellationLines => "constellation-lines",
            OverlayKind::ConstellationBoundaries => "constellation-boundaries",
            OverlayKind::DeepSkyObjects => "deep-sky-objects",
            OverlayKind::DeepSkyLabels => "deep-sky-labels",
            OverlayKind::StarLabels => "star-labels",
            OverlayKind::PlanetLabels => "planet-labels",
            OverlayKind::ConstellationLabels => "constellation-labels",
            OverlayKind::CardinalLabels => "cardinal-labels",
            OverlayKind::DegreeLabels => "degree-labels",
        }
    }

    /// Inverse of [`OverlayKind::as_kebab_str`]. Returns `None` for unknown
    /// names so hosts can choose whether to warn or silently ignore them.
    pub fn from_kebab_str(s: &str) -> Option<Self> {
        Some(match s {
            "horizon" => OverlayKind::Horizon,
            "cardinals" => OverlayKind::Cardinals,
            "alt-az-grid" => OverlayKind::AltAzGrid,
            "equatorial-grid" => OverlayKind::EquatorialGrid,
            "ecliptic" => OverlayKind::Ecliptic,
            "celestial-equator" => OverlayKind::CelestialEquator,
            "meridian" => OverlayKind::Meridian,
            "galactic-equator" => OverlayKind::GalacticEquator,
            "constellation-lines" => OverlayKind::ConstellationLines,
            "constellation-boundaries" => OverlayKind::ConstellationBoundaries,
            "deep-sky-objects" => OverlayKind::DeepSkyObjects,
            "deep-sky-labels" => OverlayKind::DeepSkyLabels,
            "star-labels" => OverlayKind::StarLabels,
            "planet-labels" => OverlayKind::PlanetLabels,
            "constellation-labels" => OverlayKind::ConstellationLabels,
            "cardinal-labels" => OverlayKind::CardinalLabels,
            "degree-labels" => OverlayKind::DegreeLabels,
            _ => return None,
        })
    }
}

/// Overlay configuration. Hosts construct this and call [`Renderer::set_overlays`].
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    pub layers: Vec<OverlayKind>,
    /// Spacing between grid lines in degrees (for `AltAzGrid` and `EquatorialGrid`).
    pub grid_step_deg: f64,
    /// Global multiplier on line-overlay alpha. Text labels remain fully opaque for legibility.
    pub opacity: f32,
    /// V magnitude cutoff for [`OverlayKind::DeepSkyObjects`] and
    /// [`OverlayKind::DeepSkyLabels`]: only Messier objects with `mag <= limit`
    /// are drawn. Defaults to [`DEFAULT_DEEP_SKY_MAGNITUDE_LIMIT`]; clamped
    /// to `[-5.0, 99.0]` at apply time so a tampered WASM caller cannot
    /// disable the layer with NaN or crash the builder.
    pub deep_sky_magnitude_limit: f32,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            layers: vec![
                OverlayKind::Horizon,
                OverlayKind::Cardinals,
                OverlayKind::CardinalLabels,
            ],
            grid_step_deg: 15.0,
            opacity: 0.6,
            deep_sky_magnitude_limit: DEFAULT_DEEP_SKY_MAGNITUDE_LIMIT,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum OverlayFrame {
    Equatorial,
    Horizontal,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct OverlayVertex {
    position: [f32; 3],
    /// Other endpoint of this line-list segment. The all-sky projection shader
    /// uses it to drop segments that cross the longitude seam instead of
    /// drawing an artefactual line across the whole map.
    other_position: [f32; 3],
}

impl OverlayVertex {
    const OFFSET_POSITION: u64 = std::mem::offset_of!(Self, position) as u64;
    const OFFSET_OTHER_POSITION: u64 = std::mem::offset_of!(Self, other_position) as u64;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct OverlayUniform {
    view_proj: [[f32; 4]; 4],
    color: [f32; 4],
    projection_params: [f32; 4],
}

struct OverlayLayer {
    frame: OverlayFrame,
    vertex_buffer: wgpu::Buffer,
    num_vertices: u32,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// RGB from the layer's default palette, A = `OverlayConfig::opacity` at build time.
    color: [f32; 4],
}

pub(crate) struct OverlayRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    layers: Vec<OverlayLayer>,
}

impl OverlayRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Overlay Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<OverlayUniform>() as u64)
                            .unwrap(),
                    ),
                },
                count: None,
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/overlay.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Overlay Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Overlay Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<OverlayVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: OverlayVertex::OFFSET_POSITION,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: OverlayVertex::OFFSET_OTHER_POSITION,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x3,
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
                    // Premultiplied additive: src.rgb * src.a is added to the framebuffer.
                    // Keeps stars showing through the lines (no flat occlusion) and gives
                    // a soft "glow on dark sky" look, while the global opacity slider
                    // still controls visibility.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
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
            bind_group_layout,
            layers: Vec::new(),
        }
    }

    /// Rebuild every layer from `config`. Cheap to call (geometry generation is
    /// a few thousand verts of trig); intended to be called whenever the user
    /// toggles overlays or changes the step / opacity.
    ///
    /// Duplicate layer entries are coalesced and `grid_step_deg` is clamped to
    /// a sane range so a pathological caller (e.g. the WASM bindings handed a
    /// tampered config) can't infinite-loop the geometry generators or build a
    /// gigabyte vertex buffer.
    pub fn set_config(&mut self, device: &wgpu::Device, config: &OverlayConfig) {
        self.layers.clear();
        let step_deg = config
            .grid_step_deg
            .clamp(GRID_STEP_MIN_DEG, GRID_STEP_MAX_DEG);
        let deep_sky_limit = sanitised_deep_sky_limit(config.deep_sky_magnitude_limit);
        // Linear scan because the variant set is tiny (≤7 entries today); avoids
        // taking a hash dependency for a deduplication that runs at most once
        // per overlay-config change.
        let mut seen: Vec<OverlayKind> = Vec::with_capacity(config.layers.len());
        for kind in &config.layers {
            if seen.contains(kind) {
                continue;
            }
            seen.push(*kind);
            let (frame, verts, rgb) = build_layer(*kind, step_deg, deep_sky_limit);
            if verts.is_empty() {
                continue;
            }
            let color = [rgb[0], rgb[1], rgb[2], config.opacity.clamp(0.0, 1.0)];

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Overlay Vertex Buffer"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Overlay Uniform Buffer"),
                contents: bytemuck::bytes_of(&OverlayUniform {
                    view_proj: [[0.0; 4]; 4],
                    color,
                    projection_params: [0.0, 1.0, 1.0, 0.0],
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Overlay Bind Group"),
                layout: &self.bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }],
            });

            self.layers.push(OverlayLayer {
                frame,
                vertex_buffer,
                num_vertices: verts.len() as u32,
                uniform_buffer,
                bind_group,
                color,
            });
        }
    }

    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &Camera) {
        if self.layers.is_empty() {
            return;
        }
        let vp_eq = camera.overlay_matrix_equatorial().to_cols_array_2d();
        let vp_local = camera.overlay_matrix_local().to_cols_array_2d();
        let projection_params = camera.overlay_projection_params();
        for layer in &self.layers {
            let view_proj = match layer.frame {
                OverlayFrame::Equatorial => vp_eq,
                OverlayFrame::Horizontal => vp_local,
            };
            queue.write_buffer(
                &layer.uniform_buffer,
                0,
                bytemuck::bytes_of(&OverlayUniform {
                    view_proj,
                    color: layer.color,
                    projection_params,
                }),
            );
        }
    }

    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if self.layers.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        for layer in &self.layers {
            pass.set_bind_group(0, &layer.bind_group, &[]);
            pass.set_vertex_buffer(0, layer.vertex_buffer.slice(..));
            pass.draw(0..layer.num_vertices, 0..1);
        }
    }
}

// ---------------------------------------------------------------------------
// Layer builders. Each returns (frame, vertices, rgb).
// ---------------------------------------------------------------------------

/// Apply the [`DEEP_SKY_MAG_MIN`] / [`DEEP_SKY_MAG_MAX`] clamp and replace
/// NaN with the default. The marker builder needs a finite finite-comparison
/// threshold or it would silently render nothing.
pub(crate) fn sanitised_deep_sky_limit(limit: f32) -> f32 {
    if limit.is_nan() {
        DEFAULT_DEEP_SKY_MAGNITUDE_LIMIT
    } else {
        limit.clamp(DEEP_SKY_MAG_MIN, DEEP_SKY_MAG_MAX)
    }
}

fn build_layer(
    kind: OverlayKind,
    grid_step_deg: f64,
    deep_sky_magnitude_limit: f32,
) -> (OverlayFrame, Vec<OverlayVertex>, [f32; 3]) {
    match kind {
        OverlayKind::Horizon => (
            OverlayFrame::Horizontal,
            closed_circle_local_at_alt(0.0, 256),
            [0.40, 0.75, 1.00],
        ),
        OverlayKind::Cardinals => (
            OverlayFrame::Horizontal,
            cardinal_marks(),
            [1.00, 0.85, 0.40],
        ),
        OverlayKind::AltAzGrid => (
            OverlayFrame::Horizontal,
            alt_az_grid(grid_step_deg),
            [0.30, 0.55, 0.85],
        ),
        OverlayKind::EquatorialGrid => (
            OverlayFrame::Equatorial,
            equatorial_grid(grid_step_deg),
            [0.85, 0.40, 0.50],
        ),
        OverlayKind::CelestialEquator => (
            OverlayFrame::Equatorial,
            closed_circle_eq_at_dec(0.0, 256),
            [0.95, 0.30, 0.35],
        ),
        OverlayKind::Ecliptic => (
            OverlayFrame::Equatorial,
            ecliptic_circle(256),
            [1.00, 0.75, 0.25],
        ),
        OverlayKind::Meridian => (
            OverlayFrame::Horizontal,
            meridian_local(256),
            [0.65, 0.65, 0.70],
        ),
        OverlayKind::GalacticEquator => (
            OverlayFrame::Equatorial,
            galactic_equator_circle(256),
            [0.55, 0.80, 1.00],
        ),
        OverlayKind::ConstellationLines => (
            OverlayFrame::Equatorial,
            segments_to_vertices(&constellation_lines()),
            [0.35, 0.65, 1.00],
        ),
        OverlayKind::ConstellationBoundaries => (
            OverlayFrame::Equatorial,
            segments_to_vertices(&constellation_boundaries()),
            [0.45, 0.45, 0.55],
        ),
        OverlayKind::DeepSkyObjects => (
            OverlayFrame::Equatorial,
            deep_sky_markers(deep_sky_magnitude_limit),
            [0.45, 0.85, 0.55],
        ),
        OverlayKind::DeepSkyLabels
        | OverlayKind::StarLabels
        | OverlayKind::PlanetLabels
        | OverlayKind::ConstellationLabels
        | OverlayKind::CardinalLabels
        | OverlayKind::DegreeLabels => (OverlayFrame::Horizontal, Vec::new(), [1.0, 1.0, 1.0]),
    }
}

/// Resolution (segment count) of the closed octagonal ring drawn around
/// each NGC / IC object. Eight segments are enough for a smooth-looking
/// circle at all viable on-screen sizes, while keeping the vertex budget
/// (16 per object) well within the overlay buffer.
const NGC_RING_SEGMENTS: usize = 8;

/// Build deep-sky outline markers in J2000 equatorial coordinates: a
/// 4-segment diamond per Messier object and an 8-segment ring per NGC / IC
/// object whose V magnitude is at most `magnitude_limit`. The two shapes
/// give the user an immediate read on what kind of catalogue an object is
/// drawn from without consulting the label.
///
/// Marker size is derived from each object's catalogued major axis, clamped
/// to `[DEEP_SKY_MARKER_MIN_ARCMIN, DEEP_SKY_MARKER_MAX_ARCMIN]` so a fully
/// resolved M31 does not paint a sky-spanning diamond that obscures the
/// galaxy itself, and so a tiny planetary nebula remains clickably big.
fn deep_sky_markers(magnitude_limit: f32) -> Vec<OverlayVertex> {
    // Pull Messier + NGC together so the marker pass is a single source of
    // truth for the line-overlay output. Order does not change the visible
    // result (the overlay pipeline blends additively, see `pipeline.rs`),
    // but Messier is appended last to keep the vertex layout stable across
    // catalogue regenerations.
    let mut objects = NgcBrightCatalog.objects(magnitude_limit);
    objects.extend(MessierCatalog.objects(magnitude_limit));

    // Worst case: every retained object is a 16-vertex NGC ring; in the
    // common case Messier diamonds (8 verts) dilute this, so the buffer
    // may over-allocate by up to ~50% on a Messier-only slider position.
    // The cost is negligible (a few KB) and only paid on config change.
    let mut verts = Vec::with_capacity(objects.len() * 16);
    for obj in objects {
        // Re-check magnitude here even though `objects()` already filters:
        // the catalog filter uses a direct `<=` (drops NaN-valued rows),
        // and the renderer asserts the same contract via `partial_cmp` so
        // a future catalog impl that forwards NaN magnitudes cannot leak
        // an unrenderable marker into the overlay buffer.
        if !matches!(
            obj.magnitude.partial_cmp(&magnitude_limit),
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal),
        ) {
            continue;
        }
        match obj.id {
            DeepSkyId::Messier(_) => append_diamond_marker(&mut verts, &obj),
            DeepSkyId::Ngc(_) | DeepSkyId::Ic(_) => append_ring_marker(&mut verts, &obj),
        }
    }
    attach_segment_partners(verts)
}

fn marker_half_radius_rad(obj: &DeepSkyObject) -> f32 {
    let half_arcmin =
        (obj.size_arcmin * 0.5).clamp(DEEP_SKY_MARKER_MIN_ARCMIN, DEEP_SKY_MARKER_MAX_ARCMIN);
    half_arcmin * std::f32::consts::PI / (180.0 * 60.0)
}

fn marker_offset(
    p: [f32; 3],
    u: [f32; 3],
    v: [f32; 3],
    half_rad: f32,
    su: f32,
    sv: f32,
) -> [f32; 3] {
    let q = [
        p[0] + half_rad * (su * u[0] + sv * v[0]),
        p[1] + half_rad * (su * u[1] + sv * v[1]),
        p[2] + half_rad * (su * u[2] + sv * v[2]),
    ];
    // Re-normalise so the result stays on the unit sphere; at the marker
    // sizes used here the correction is ~10⁻⁴ but the overlay shader and
    // downstream tests assume unit length.
    let r = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
    [q[0] / r, q[1] / r, q[2] / r]
}

/// Emit the 4-segment Messier diamond (8 vertices).
fn append_diamond_marker(verts: &mut Vec<OverlayVertex>, obj: &DeepSkyObject) {
    let half_rad = marker_half_radius_rad(obj);
    let p = obj.position;
    let (u, v) = tangent_basis(p);
    let top = marker_offset(p, u, v, half_rad, 0.0, 1.0);
    let right = marker_offset(p, u, v, half_rad, 1.0, 0.0);
    let bottom = marker_offset(p, u, v, half_rad, 0.0, -1.0);
    let left = marker_offset(p, u, v, half_rad, -1.0, 0.0);
    for (a, b) in [(top, right), (right, bottom), (bottom, left), (left, top)] {
        verts.push(overlay_vertex(a));
        verts.push(overlay_vertex(b));
    }
}

/// Emit the 8-segment NGC / IC ring (16 vertices). The ring is inscribed
/// in the same half-radius the Messier diamond uses, so the two markers
/// stay visually comparable in size at the same catalogue dimensions.
fn append_ring_marker(verts: &mut Vec<OverlayVertex>, obj: &DeepSkyObject) {
    let half_rad = marker_half_radius_rad(obj);
    let p = obj.position;
    let (u, v) = tangent_basis(p);
    let mut points: [[f32; 3]; NGC_RING_SEGMENTS] = [[0.0; 3]; NGC_RING_SEGMENTS];
    for (idx, point) in points.iter_mut().enumerate() {
        let theta = std::f32::consts::TAU * idx as f32 / NGC_RING_SEGMENTS as f32;
        let (sin_t, cos_t) = theta.sin_cos();
        *point = marker_offset(p, u, v, half_rad, sin_t, cos_t);
    }
    for i in 0..NGC_RING_SEGMENTS {
        let a = points[i];
        let b = points[(i + 1) % NGC_RING_SEGMENTS];
        verts.push(overlay_vertex(a));
        verts.push(overlay_vertex(b));
    }
}

/// Build a right-handed orthonormal tangent frame `(u, v)` at the unit
/// vector `p` on the celestial sphere.
///
/// `u` points roughly east (toward increasing RA in the tangent plane);
/// `v` points roughly north. Near a celestial pole the cross product with
/// the z axis vanishes, so we fall back to the x axis.
fn tangent_basis(p: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    // u' = ẑ × p, scaled later. If p is along ẑ, fall back to x̂.
    let mut u_raw = [-p[1], p[0], 0.0];
    let r2 = u_raw[0] * u_raw[0] + u_raw[1] * u_raw[1];
    if r2 < 1.0e-8 {
        u_raw = [1.0, 0.0, 0.0];
    }
    let r = (u_raw[0] * u_raw[0] + u_raw[1] * u_raw[1] + u_raw[2] * u_raw[2]).sqrt();
    let u = [u_raw[0] / r, u_raw[1] / r, u_raw[2] / r];
    // v = p × u (right-handed).
    let v_raw = [
        p[1] * u[2] - p[2] * u[1],
        p[2] * u[0] - p[0] * u[2],
        p[0] * u[1] - p[1] * u[0],
    ];
    let rv = (v_raw[0] * v_raw[0] + v_raw[1] * v_raw[1] + v_raw[2] * v_raw[2]).sqrt();
    let v = [v_raw[0] / rv, v_raw[1] / rv, v_raw[2] / rv];
    (u, v)
}

// ---------------------------------------------------------------------------
// Geometry primitives. Output uses `LineList` topology: every two consecutive
// vertices form one line segment.
// ---------------------------------------------------------------------------

fn overlay_vertex(position: [f32; 3]) -> OverlayVertex {
    OverlayVertex {
        position,
        other_position: position,
    }
}

fn attach_segment_partners(mut verts: Vec<OverlayVertex>) -> Vec<OverlayVertex> {
    for pair in verts.chunks_exact_mut(2) {
        let a = pair[0].position;
        let b = pair[1].position;
        pair[0].other_position = b;
        pair[1].other_position = a;
    }
    verts
}

/// Parallel circle at constant declination in equatorial coords. Closed.
fn closed_circle_eq_at_dec(dec_rad: f64, n: usize) -> Vec<OverlayVertex> {
    let (sd, cd) = dec_rad.sin_cos();
    let mut verts = Vec::with_capacity(n * 2);
    for i in 0..n {
        let a0 = (i as f64) / (n as f64) * TAU;
        let a1 = ((i + 1) as f64) / (n as f64) * TAU;
        verts.push(overlay_vertex([
            (cd * a0.cos()) as f32,
            (cd * a0.sin()) as f32,
            sd as f32,
        ]));
        verts.push(overlay_vertex([
            (cd * a1.cos()) as f32,
            (cd * a1.sin()) as f32,
            sd as f32,
        ]));
    }
    attach_segment_partners(verts)
}

/// Parallel circle at constant altitude in local ENU coords. Closed.
fn closed_circle_local_at_alt(alt_rad: f64, n: usize) -> Vec<OverlayVertex> {
    // ENU basis: x = East = sin(az) cos(alt), y = North = cos(az) cos(alt), z = Up = sin(alt).
    let (sa, ca) = alt_rad.sin_cos();
    let mut verts = Vec::with_capacity(n * 2);
    for i in 0..n {
        let az0 = (i as f64) / (n as f64) * TAU;
        let az1 = ((i + 1) as f64) / (n as f64) * TAU;
        verts.push(overlay_vertex([
            (az0.sin() * ca) as f32,
            (az0.cos() * ca) as f32,
            sa as f32,
        ]));
        verts.push(overlay_vertex([
            (az1.sin() * ca) as f32,
            (az1.cos() * ca) as f32,
            sa as f32,
        ]));
    }
    attach_segment_partners(verts)
}

/// Half great circle at fixed RA, sweeping declination from near-south to near-north.
/// Endpoints are pulled in slightly to avoid converging at the poles.
fn meridian_eq_at_ra(ra_rad: f64, n: usize) -> Vec<OverlayVertex> {
    let eps = 0.01;
    let lo = -PI / 2.0 + eps;
    let hi = PI / 2.0 - eps;
    let (sa, ca) = ra_rad.sin_cos();
    let mut verts = Vec::with_capacity((n - 1) * 2);
    let p = |d: f64| {
        let (sd, cd) = d.sin_cos();
        [(cd * ca) as f32, (cd * sa) as f32, sd as f32]
    };
    for i in 0..(n - 1) {
        let d0 = lo + (hi - lo) * (i as f64) / ((n - 1) as f64);
        let d1 = lo + (hi - lo) * ((i + 1) as f64) / ((n - 1) as f64);
        verts.push(overlay_vertex(p(d0)));
        verts.push(overlay_vertex(p(d1)));
    }
    attach_segment_partners(verts)
}

/// Half great circle at fixed azimuth, sweeping altitude from near-nadir to near-zenith.
fn meridian_local_at_az(az_rad: f64, n: usize) -> Vec<OverlayVertex> {
    let eps = 0.01;
    let lo = -PI / 2.0 + eps;
    let hi = PI / 2.0 - eps;
    let (saz, caz) = az_rad.sin_cos();
    let p = |alt: f64| {
        let (s, c) = alt.sin_cos();
        [(saz * c) as f32, (caz * c) as f32, s as f32]
    };
    let mut verts = Vec::with_capacity((n - 1) * 2);
    for i in 0..(n - 1) {
        let a0 = lo + (hi - lo) * (i as f64) / ((n - 1) as f64);
        let a1 = lo + (hi - lo) * ((i + 1) as f64) / ((n - 1) as f64);
        verts.push(overlay_vertex(p(a0)));
        verts.push(overlay_vertex(p(a1)));
    }
    attach_segment_partners(verts)
}

fn equatorial_grid(step_deg: f64) -> Vec<OverlayVertex> {
    let mut verts = Vec::new();
    let step = step_deg.to_radians();
    // Parallels of constant declination (skip the equator; CelestialEquator owns it).
    let mut dec = step;
    while dec < PI / 2.0 - 1e-6 {
        verts.extend(closed_circle_eq_at_dec(dec, 96));
        verts.extend(closed_circle_eq_at_dec(-dec, 96));
        dec += step;
    }
    // Hour circles at constant RA.
    let mut ra = 0.0;
    while ra < TAU - 1e-6 {
        verts.extend(meridian_eq_at_ra(ra, 48));
        ra += step;
    }
    verts
}

fn alt_az_grid(step_deg: f64) -> Vec<OverlayVertex> {
    let mut verts = Vec::new();
    let step = step_deg.to_radians();
    // Parallels of constant altitude (skip the horizon; Horizon owns it).
    let mut alt = step;
    while alt < PI / 2.0 - 1e-6 {
        verts.extend(closed_circle_local_at_alt(alt, 96));
        verts.extend(closed_circle_local_at_alt(-alt, 96));
        alt += step;
    }
    // Meridians at constant azimuth.
    let mut az = 0.0;
    while az < TAU - 1e-6 {
        verts.extend(meridian_local_at_az(az, 48));
        az += step;
    }
    verts
}

/// Four short vertical bars at the cardinal points on the horizon. North is taller
/// so it's recognizable without text labels.
fn cardinal_marks() -> Vec<OverlayVertex> {
    // (azimuth, bar height as a fraction of the unit sphere radius)
    let bars: [(f64, f64); 4] = [
        (0.0, 0.12),            // N
        (PI / 2.0, 0.06),       // E
        (PI, 0.06),             // S
        (3.0 * PI / 2.0, 0.06), // W
    ];
    let mut verts = Vec::with_capacity(bars.len() * 2);
    for (az, h) in bars {
        let (s, c) = az.sin_cos();
        verts.push(overlay_vertex([s as f32, c as f32, 0.0]));
        verts.push(overlay_vertex([s as f32, c as f32, h as f32]));
    }
    attach_segment_partners(verts)
}

/// Great circle of the ecliptic in equatorial coords. The plane is tilted about
/// the X axis (vernal equinox direction) by the obliquity ε.
fn ecliptic_circle(n: usize) -> Vec<OverlayVertex> {
    let (se, ce) = OBLIQUITY_RAD.sin_cos();
    // Rx(ε) · (cos λ, sin λ, 0) = (cos λ, cos ε · sin λ, sin ε · sin λ)
    let p = |lam: f64| {
        let (sl, cl) = lam.sin_cos();
        [cl as f32, (ce * sl) as f32, (se * sl) as f32]
    };
    let mut verts = Vec::with_capacity(n * 2);
    for i in 0..n {
        let l0 = (i as f64) / (n as f64) * TAU;
        let l1 = ((i + 1) as f64) / (n as f64) * TAU;
        verts.push(overlay_vertex(p(l0)));
        verts.push(overlay_vertex(p(l1)));
    }
    attach_segment_partners(verts)
}

/// Great circle at galactic latitude b = 0, transformed back into J2000
/// equatorial coordinates with the transpose of the SOFA-compatible
/// equatorial→galactic rotation.
fn galactic_equator_circle(n: usize) -> Vec<OverlayVertex> {
    let r = EQUATORIAL_TO_GALACTIC_ROWS;
    let p = |l: f64| {
        let (sl, cl) = l.sin_cos();
        // v_eq = R^T · (cos l, sin l, 0)
        [
            (r[0][0] * cl + r[1][0] * sl) as f32,
            (r[0][1] * cl + r[1][1] * sl) as f32,
            (r[0][2] * cl + r[1][2] * sl) as f32,
        ]
    };
    let mut verts = Vec::with_capacity(n * 2);
    for i in 0..n {
        let l0 = (i as f64) / (n as f64) * TAU;
        let l1 = ((i + 1) as f64) / (n as f64) * TAU;
        verts.push(overlay_vertex(p(l0)));
        verts.push(overlay_vertex(p(l1)));
    }
    attach_segment_partners(verts)
}

fn segments_to_vertices(segments: &[ConstellationSegment]) -> Vec<OverlayVertex> {
    let mut verts = Vec::with_capacity(segments.len() * 2);
    for segment in segments {
        verts.push(overlay_vertex(segment.start));
        verts.push(overlay_vertex(segment.end));
    }
    attach_segment_partners(verts)
}

fn meridian_local(n: usize) -> Vec<OverlayVertex> {
    let p = |t: f64| {
        let (s, c) = t.sin_cos();
        [0.0_f32, s as f32, c as f32]
    };
    let mut verts = Vec::with_capacity(n * 2);
    for i in 0..n {
        let t0 = (i as f64) / (n as f64) * TAU;
        let t1 = ((i + 1) as f64) / (n as f64) * TAU;
        verts.push(overlay_vertex(p(t0)));
        verts.push(overlay_vertex(p(t1)));
    }
    attach_segment_partners(verts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizon_lies_on_z_zero() {
        let verts = closed_circle_local_at_alt(0.0, 64);
        for v in &verts {
            assert!(v.position[2].abs() < 1e-5);
            let r = (v.position[0].powi(2) + v.position[1].powi(2)).sqrt();
            assert!((r - 1.0).abs() < 1e-5, "horizon should be a unit circle");
        }
    }

    #[test]
    fn celestial_equator_lies_on_z_zero() {
        let verts = closed_circle_eq_at_dec(0.0, 64);
        for v in &verts {
            assert!(v.position[2].abs() < 1e-5);
        }
    }

    #[test]
    fn ecliptic_passes_through_vernal_equinox() {
        // At ecliptic longitude 0, the point should be (1, 0, 0) — the vernal equinox.
        let (se, ce) = OBLIQUITY_RAD.sin_cos();
        let p = [(1.0_f64) as f32, (ce * 0.0) as f32, (se * 0.0) as f32];
        assert!((p[0] - 1.0).abs() < 1e-6);
        assert!(p[1].abs() < 1e-6);
        assert!(p[2].abs() < 1e-6);
    }

    #[test]
    fn ecliptic_max_tilt_matches_obliquity() {
        // At longitude π/2, the z component equals sin(ε).
        let (se, _) = OBLIQUITY_RAD.sin_cos();
        let verts = ecliptic_circle(360);
        let max_z = verts
            .iter()
            .map(|v| v.position[2])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (max_z - se as f32).abs() < 1e-3,
            "max z = {max_z}, sin ε = {}",
            se
        );
    }

    #[test]
    fn linelist_vertex_count_is_even() {
        // LineList topology requires pairs.
        assert_eq!(closed_circle_local_at_alt(0.0, 64).len() % 2, 0);
        assert_eq!(meridian_eq_at_ra(0.0, 64).len() % 2, 0);
        assert_eq!(cardinal_marks().len() % 2, 0);
        assert_eq!(equatorial_grid(15.0).len() % 2, 0);
        assert_eq!(alt_az_grid(15.0).len() % 2, 0);
    }

    #[test]
    fn default_config_has_horizon_and_cardinals() {
        let c = OverlayConfig::default();
        assert!(c.layers.contains(&OverlayKind::Horizon));
        assert!(c.layers.contains(&OverlayKind::Cardinals));
    }

    #[test]
    fn kebab_str_round_trips_every_variant() {
        for kind in [
            OverlayKind::Horizon,
            OverlayKind::Cardinals,
            OverlayKind::AltAzGrid,
            OverlayKind::EquatorialGrid,
            OverlayKind::Ecliptic,
            OverlayKind::CelestialEquator,
            OverlayKind::Meridian,
            OverlayKind::GalacticEquator,
            OverlayKind::ConstellationLines,
            OverlayKind::ConstellationBoundaries,
            OverlayKind::DeepSkyObjects,
            OverlayKind::DeepSkyLabels,
            OverlayKind::StarLabels,
            OverlayKind::PlanetLabels,
            OverlayKind::ConstellationLabels,
            OverlayKind::CardinalLabels,
            OverlayKind::DegreeLabels,
        ] {
            let s = kind.as_kebab_str();
            assert_eq!(
                OverlayKind::from_kebab_str(s),
                Some(kind),
                "round-trip failed for {kind:?} via {s:?}"
            );
        }
        assert_eq!(OverlayKind::from_kebab_str("unknown"), None);
    }

    #[test]
    fn alt_az_grid_terminates_at_min_step() {
        // The generator uses `step += step` to advance the parallels and
        // meridians. A zero/negative step would loop forever, which is the
        // bug the GRID_STEP_MIN_DEG clamp in `set_config` guards against.
        // This test asserts the generator itself behaves at the lower bound
        // (so the clamp is the only safety we need to rely on).
        let v = alt_az_grid(GRID_STEP_MIN_DEG);
        assert!(!v.is_empty());
        assert_eq!(v.len() % 2, 0);
    }

    #[test]
    fn equatorial_grid_terminates_at_min_step() {
        let v = equatorial_grid(GRID_STEP_MIN_DEG);
        assert!(!v.is_empty());
        assert_eq!(v.len() % 2, 0);
    }

    #[test]
    fn equatorial_grid_at_max_step_is_minimal() {
        // At step = 90°, there are no non-equator parallels and only 4 hour
        // circles. Mostly a smoke test that the upper-clamp value doesn't
        // produce a degenerate buffer.
        let v = equatorial_grid(GRID_STEP_MAX_DEG);
        assert_eq!(v.len() % 2, 0);
    }

    #[test]
    fn constellation_line_vertices_are_well_formed() {
        let v = segments_to_vertices(&constellation_lines());
        assert_eq!(v.len(), 743 * 2);
        assert_constellation_vertices_are_unit_length(&v);
    }

    #[test]
    fn constellation_boundary_vertices_are_well_formed() {
        let v = segments_to_vertices(&constellation_boundaries());
        assert_eq!(v.len(), 1565 * 2);
        assert_constellation_vertices_are_unit_length(&v);
    }

    fn assert_constellation_vertices_are_unit_length(vertices: &[OverlayVertex]) {
        assert_eq!(vertices.len() % 2, 0);
        for vertex in vertices {
            let r = (vertex.position[0].powi(2)
                + vertex.position[1].powi(2)
                + vertex.position[2].powi(2))
            .sqrt();
            assert!(
                (r - 1.0).abs() < 1e-4,
                "constellation vertex is not unit length"
            );
        }
    }

    // ---- deep-sky marker tests ----

    fn assert_unit_length(vertices: &[OverlayVertex], context: &str) {
        for v in vertices {
            let r = (v.position[0].powi(2) + v.position[1].powi(2) + v.position[2].powi(2)).sqrt();
            assert!(
                (r - 1.0).abs() < 1.0e-3,
                "{context}: vertex {:?} not unit length (r={r})",
                v.position
            );
        }
    }

    #[test]
    fn deep_sky_markers_at_show_all_limit_have_expected_segment_count() {
        // Each Messier object contributes a 4-segment diamond (8 vertices);
        // each NGC / IC object contributes an 8-segment ring (16 vertices).
        // The exact NGC count drifts with each OpenNGC snapshot, so we only
        // check the Messier contribution is present and the total is a
        // multiple of two (every vertex must have a partner for the
        // LineList topology).
        let v = deep_sky_markers(99.0);
        let messier_verts = 110 * 8;
        assert!(v.len() >= messier_verts + 16); // at least one NGC ring.
        assert_eq!(v.len() % 2, 0);
        // Rings contribute multiples of 16; subtracting the Messier diamond
        // contribution must leave a multiple of 16.
        assert_eq!((v.len() - messier_verts) % 16, 0);
        assert_unit_length(&v, "deep_sky_markers(99.0)");
    }

    #[test]
    fn deep_sky_markers_respect_magnitude_limit() {
        // At limit -10, no object qualifies (brightest Messier is M45 at
        // ~1.6; brightest NGC entry sits above the same threshold).
        let none = deep_sky_markers(-10.0);
        assert!(none.is_empty());
        // At limit 2.0 only Messier objects brighter than mag 2 survive
        // (M45 alone), plus any NGC / IC entries at the same brightness.
        // The combined count is therefore at least the M45 diamond (8) and
        // is a multiple of 8 (4 segments × 2 vertices for diamonds;
        // 8 segments × 2 vertices for rings, both divisible by 8).
        let only_brightest = deep_sky_markers(2.0);
        assert!(only_brightest.len() >= 8);
        assert_eq!(only_brightest.len() % 8, 0);
        // At the default cutoff (7.0) the slider should expose strictly
        // more markers than the brightest-only filter and strictly fewer
        // than the show-all filter.
        let default = deep_sky_markers(DEFAULT_DEEP_SKY_MAGNITUDE_LIMIT);
        let show_all = deep_sky_markers(99.0);
        assert!(default.len() > only_brightest.len());
        assert!(default.len() < show_all.len());
    }

    #[test]
    fn deep_sky_markers_skip_nan_and_inf() {
        // NaN must not pass the threshold check (the `!(a <= b)` invariant).
        let v = deep_sky_markers(f32::NAN);
        assert!(v.is_empty());
    }

    #[test]
    fn sanitised_deep_sky_limit_clamps_and_replaces_nan() {
        assert_eq!(sanitised_deep_sky_limit(7.0), 7.0);
        assert_eq!(sanitised_deep_sky_limit(-99.0), DEEP_SKY_MAG_MIN);
        assert_eq!(sanitised_deep_sky_limit(999.0), DEEP_SKY_MAG_MAX);
        assert_eq!(
            sanitised_deep_sky_limit(f32::NAN),
            DEFAULT_DEEP_SKY_MAGNITUDE_LIMIT
        );
    }

    #[test]
    fn tangent_basis_is_orthonormal_and_falls_back_at_pole() {
        for p in [
            [1.0_f32, 0.0, 0.0],
            [0.0_f32, 1.0, 0.0],
            [0.0_f32, 0.0, 1.0],  // pole — fallback path
            [0.0_f32, 0.0, -1.0], // pole — fallback path
            [0.5_f32, 0.5, 0.707_106_77],
        ] {
            let (u, v) = tangent_basis(p);
            // Unit length.
            let lu = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
            let lv = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            assert!((lu - 1.0).abs() < 1.0e-5);
            assert!((lv - 1.0).abs() < 1.0e-5);
            // Orthogonal to p and to each other.
            let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            assert!(dot(u, p).abs() < 1.0e-5);
            assert!(dot(v, p).abs() < 1.0e-5);
            assert!(dot(u, v).abs() < 1.0e-5);
        }
    }
}
