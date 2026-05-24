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
    /// units. The mapping from apparent magnitude to this value is set by
    /// `magnitude_to_render_params`; the zeropoint is derived from the
    /// caller's chosen `limiting_magnitude` so that a star at exactly that
    /// magnitude lands on the shader's discard cutoff.
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

/// Billboard half-width, expressed as a multiple of `PSF_SIGMA_PX`.
///
/// Must be large enough that the brightest star we ever render has its
/// Gaussian tail fall below the shader's discard cutoff at the quad edge,
/// across the full range of `limiting_magnitude` values we support. The peak
/// linear intensity of Sirius (m ≈ −1.46) at `limiting_magnitude = 9` is
/// ≈61, so the edge PSF must drop below 0.004 / 61 ≈ 6.5e−5, requiring at
/// least ≈4.4σ. 5σ leaves the edge at exp(−12.5) ≈ 3.7e−6, i.e. headroom
/// for ~`limiting_magnitude ≤ 10.5` before any bright-star quad would clip.
const PSF_QUAD_HALF_WIDTH_SIGMAS: f32 = 5.0;

/// Peak-intensity threshold below which the shader discards a star pixel.
///
/// Mirrors the literal `intensity < 0.004` in `shaders/star.wgsl::fs_main`.
/// Exposed here so the magnitude↔brightness mapping can pin the chosen
/// limiting magnitude exactly onto the shader's cutoff.
pub const SHADER_INTENSITY_CUTOFF: f32 = 0.004;

/// Conventional naked-eye limiting magnitude for a dark-adapted observer under
/// a pristine sky. The value the literature uses when stating "you can see
/// down to magnitude six".
pub const NAKED_EYE_LIMITING_MAGNITUDE: f32 = 6.0;

/// Convert a star's apparent magnitude into renderer parameters.
///
/// Brightness follows Pogson's law exactly:
///
/// ```text
///     L = 10^(-0.4 · (m − m_ref))
/// ```
///
/// so a 5-magnitude difference always corresponds to a factor of 100 in
/// linear flux, exactly as on the sky. The only knob is `limiting_magnitude`:
/// the faintest star the *observer* can still register. The zeropoint
/// `m_ref` is chosen so that a star at exactly the limiting magnitude lands
/// on the shader's discard cutoff, so increasing `limiting_magnitude`
/// uniformly scales the whole linear-flux scene ("longer exposure" / "more
/// sensitive observer") without breaking Pogson's law or introducing any
/// per-star compression.
///
/// For reference observers:
/// * `NAKED_EYE_LIMITING_MAGNITUDE` (6.0) — strict dark-adapted naked eye.
/// * ≈7.5 — typical visual limit through good binoculars / matches the depth
///   of the HYG catalog, useful as a default on indoor screens whose dynamic
///   range can't reproduce a dark-sky scene faithfully.
pub fn magnitude_to_render_params(mag: f32, limiting_magnitude: f32) -> RenderParams {
    // Solve `10^(-0.4 * (limiting_mag - zeropoint)) = SHADER_INTENSITY_CUTOFF`
    // for `zeropoint` so that the user's requested limiting magnitude lands
    // exactly on the shader's discard threshold.
    let zeropoint = limiting_magnitude + SHADER_INTENSITY_CUTOFF.log10() / 0.4;
    let brightness = 10.0_f32.powf(-0.4 * (mag - zeropoint));
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
        // Hard-coded in `shaders/star.wgsl::fs_main`. Update both together.
        const SHADER_COEFF: f32 = 12.5;
        assert!(
            (coeff - SHADER_COEFF).abs() < 1e-5,
            "PSF Gaussian coefficient drift: CPU constants imply {coeff}, shader has {SHADER_COEFF}"
        );
    }

    /// Pogson's law: a 5-magnitude difference is exactly a factor of 100 in
    /// linear flux, regardless of the observer model. This is the central
    /// guarantee the renderer makes.
    #[test]
    fn brightness_follows_pogson_law() {
        for &lim in &[6.0_f32, 7.5, 9.0] {
            let b0 = magnitude_to_render_params(0.0, lim).brightness;
            let b5 = magnitude_to_render_params(5.0, lim).brightness;
            let ratio = b0 / b5;
            assert!(
                (ratio - 100.0).abs() < 1e-3,
                "5-mag flux ratio at limiting_mag={lim} is {ratio}, expected 100"
            );
        }
    }

    /// Apparent radius on screen must not encode magnitude — stars are point
    /// sources, and any size variation would be a chart-style exaggeration we
    /// are explicitly opting out of.
    #[test]
    fn radius_is_independent_of_magnitude() {
        let lim = NAKED_EYE_LIMITING_MAGNITUDE;
        let r_bright = magnitude_to_render_params(-1.5, lim).radius_px;
        let r_mid = magnitude_to_render_params(2.0, lim).radius_px;
        let r_faint = magnitude_to_render_params(5.5, lim).radius_px;
        assert_eq!(r_bright, r_mid);
        assert_eq!(r_mid, r_faint);
    }

    /// A star at exactly the user-chosen limiting magnitude must land on the
    /// shader's discard cutoff. This is what gives `limiting_magnitude` its
    /// physical meaning: the faintest star that survives rendering.
    #[test]
    fn limiting_magnitude_lands_on_shader_cutoff() {
        for &lim in &[6.0_f32, 7.5, 9.0] {
            let peak = magnitude_to_render_params(lim, lim).brightness;
            let rel_err = (peak - SHADER_INTENSITY_CUTOFF).abs() / SHADER_INTENSITY_CUTOFF;
            assert!(
                rel_err < 1e-4,
                "limiting_mag={lim} peaks at {peak}, expected {SHADER_INTENSITY_CUTOFF}"
            );
        }
    }

    /// The PSF quad (sized in `PSF_QUAD_HALF_WIDTH_SIGMAS` units of sigma)
    /// must contain the Gaussian tail below the shader's discard threshold
    /// even for the brightest star we expect to see (≈ Sirius, m = -1.46).
    /// If this fails, bright-star quads would show a hard edge.
    ///
    /// Worst case for tail leakage is the most sensitive observer model we
    /// support: more exposure ⇒ brighter Sirius ⇒ hotter edge pixels.
    #[test]
    fn quad_edge_falls_below_cutoff_for_brightest_stars() {
        // Sirius: brightest fixed star, m ≈ -1.46.
        // Test the deepest observer we expect to expose; if this passes the
        // strict naked-eye case (smaller brightness) passes trivially.
        let p = magnitude_to_render_params(-1.46, 9.0);
        // Gaussian value at the quad corner (|uv| = 1) with the shader's coeff:
        let edge_psf = (-12.5_f32).exp();
        let edge_intensity = p.brightness * edge_psf;
        assert!(
            edge_intensity < SHADER_INTENSITY_CUTOFF,
            "Sirius-class star leaks past quad edge: edge intensity {edge_intensity} ≥ cutoff {SHADER_INTENSITY_CUTOFF}"
        );
    }
}
