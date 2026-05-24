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
///
/// The model treats every star as an unresolved point source convolved with a
/// fixed point-spread function (PSF). This is the academically defensible
/// view: the *only* thing that varies between stars on screen is their
/// brightness, never their apparent size on the sky.
#[derive(Debug, Clone, Copy)]
pub struct RenderParams {
    /// Half-width of the billboard quad in screen-space pixels.
    ///
    /// This is purely a *container* sized to hold the Gaussian PSF's tails
    /// down to numerical insignificance. It is **identical for every star** —
    /// brightness is encoded by `brightness`, never by size.
    pub radius_px: f32,
    /// Peak (center-pixel) intensity of the Gaussian PSF, in linear light
    /// units relative to a magnitude-`MAG_ZEROPOINT` reference star.
    ///
    /// Values > 1.0 saturate against the additive blend, producing the
    /// naturally larger visible glow of bright stars — which is the physical
    /// behaviour of any imaging system (eye, camera) viewing a point source
    /// brighter than its dynamic range, not an artistic exaggeration.
    pub brightness: f32,
}

/// Standard deviation of the rendered PSF, in screen pixels.
///
/// Stars are unresolved point sources. For naked-eye-scale rendering (≈90°
/// FoV at ≈1280 px wide, i.e. ~4′ / px) the eye's own angular resolution
/// (≈1′) is sub-pixel, and atmospheric seeing is utterly negligible — the
/// physical PSF is effectively a delta function. We use σ just slightly under
/// one pixel so the Gaussian is well anti-aliased without becoming visibly
/// puffy. Bright stars still grow naturally via additive saturation, which is
/// the genuine optical glare of a point source past the dynamic range, not a
/// magnitude→size hack.
///
/// The matching Gaussian coefficient lives in `shaders/star.wgsl` and must be
/// kept in sync with `PSF_QUAD_HALF_WIDTH_SIGMAS` below.
const PSF_SIGMA_PX: f32 = 0.9;

/// Billboard half-width, expressed as a multiple of `PSF_SIGMA_PX`. 4σ leaves
/// the Gaussian at ~3.4e-4 of its peak at the quad edge, comfortably below the
/// shader's `alpha < 0.004` discard threshold.
const PSF_QUAD_HALF_WIDTH_SIGMAS: f32 = 4.0;

/// Reference apparent magnitude that maps to `brightness = 1.0`.
///
/// With the zeropoint at m = 0 the shader's `alpha < 0.004` cutoff lines up
/// with apparent magnitude ≈ 6.0, i.e. the conventional naked-eye limiting
/// magnitude — faint stars fade out exactly where a dark-adapted observer
/// would lose them, with no artificial clamp.
const MAG_ZEROPOINT: f32 = 0.0;

/// Convert a star's apparent magnitude into renderer parameters.
///
/// Brightness follows Pogson's law exactly:
///
/// ```text
///     L = 10^(-0.4 · (m − m_ref))
/// ```
///
/// so a 5-magnitude difference corresponds to a factor of 100 in linear flux,
/// as it does on the sky. No exponent compression, no clamps — the dynamic
/// range you see is the dynamic range the catalog reports.
pub fn magnitude_to_render_params(mag: f32) -> RenderParams {
    let brightness = 10.0_f32.powf(-0.4 * (mag - MAG_ZEROPOINT));
    let radius_px = PSF_SIGMA_PX * PSF_QUAD_HALF_WIDTH_SIGMAS;
    RenderParams {
        radius_px,
        brightness,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shader's Gaussian coefficient is hard-coded; if either constant on
    /// the CPU side changes, the WGSL literal must move in lock-step or the
    /// PSF stops matching its container. This pins the relationship.
    #[test]
    fn shader_coefficient_matches_psf_constants() {
        let sigma_quad = PSF_SIGMA_PX / (PSF_SIGMA_PX * PSF_QUAD_HALF_WIDTH_SIGMAS);
        let coeff = 1.0 / (2.0 * sigma_quad * sigma_quad);
        // Hard-coded `8.0` in `shaders/star.wgsl::fs_main`.
        const SHADER_COEFF: f32 = 8.0;
        assert!(
            (coeff - SHADER_COEFF).abs() < 1e-5,
            "PSF Gaussian coefficient drift: CPU constants imply {coeff}, shader has {SHADER_COEFF}"
        );
    }

    /// Pogson's law: a 5-magnitude difference is exactly a factor of 100 in
    /// linear flux. This is the central guarantee the renderer now makes.
    #[test]
    fn brightness_follows_pogson_law() {
        let b0 = magnitude_to_render_params(0.0).brightness;
        let b5 = magnitude_to_render_params(5.0).brightness;
        let ratio = b0 / b5;
        assert!(
            (ratio - 100.0).abs() < 1e-3,
            "5-mag flux ratio is {ratio}, expected 100"
        );
    }

    /// With `MAG_ZEROPOINT = 0` a magnitude-0 star renders at peak intensity
    /// 1.0 — the reference point against which everything else is measured.
    #[test]
    fn zeropoint_star_has_unit_brightness() {
        let b = magnitude_to_render_params(MAG_ZEROPOINT).brightness;
        assert!(
            (b - 1.0).abs() < 1e-6,
            "m = m_ref should give brightness 1, got {b}"
        );
    }

    /// Apparent radius on screen must not encode magnitude — stars are point
    /// sources, and any size variation would be a chart-style exaggeration we
    /// are explicitly opting out of.
    #[test]
    fn radius_is_independent_of_magnitude() {
        let r_bright = magnitude_to_render_params(-1.5).radius_px;
        let r_mid = magnitude_to_render_params(2.0).radius_px;
        let r_faint = magnitude_to_render_params(5.5).radius_px;
        assert_eq!(r_bright, r_mid);
        assert_eq!(r_mid, r_faint);
    }

    /// The naked-eye limiting magnitude (≈6.0) must coincide with the shader's
    /// alpha discard threshold (0.004). If either drifts, faint stars start
    /// appearing or disappearing in the wrong places.
    #[test]
    fn naked_eye_limit_matches_shader_cutoff() {
        // Shader literal: `intensity < 0.004` in `fs_main`.
        const SHADER_CUTOFF: f32 = 0.004;
        let peak_at_m6 = magnitude_to_render_params(6.0).brightness;
        // m = 6 should land essentially on the cutoff (within float noise).
        assert!(
            (peak_at_m6 - SHADER_CUTOFF).abs() < 5e-4,
            "m = 6 peak intensity {peak_at_m6} does not land on shader cutoff {SHADER_CUTOFF}"
        );
    }

    /// 4-sigma quad must contain the Gaussian tails below the shader's discard
    /// threshold even for the brightest stars we expect to see (≈ Sirius,
    /// m = -1.46). If this fails, bright-star quads would show a hard edge.
    #[test]
    fn quad_edge_falls_below_cutoff_for_brightest_stars() {
        const SHADER_CUTOFF: f32 = 0.004;
        // Sirius: brightest fixed star, m ≈ -1.46.
        let p = magnitude_to_render_params(-1.46);
        // Gaussian value at the quad corner (|uv| = 1) with the shader's coeff:
        let edge_psf = (-8.0_f32).exp();
        let edge_intensity = p.brightness * edge_psf;
        assert!(
            edge_intensity < SHADER_CUTOFF,
            "Sirius-class star leaks past quad edge: edge intensity {edge_intensity} ≥ cutoff {SHADER_CUTOFF}"
        );
    }
}
