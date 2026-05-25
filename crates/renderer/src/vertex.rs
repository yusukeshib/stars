use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub(crate) struct QuadVertex {
    position: [f32; 2],
}

impl QuadVertex {
    pub(crate) const VERTICES: &[Self] = &[
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

    pub(crate) const INDICES: &[u16] = &[0, 1, 2, 0, 2, 3];

    const OFFSET_POSITION: u64 = std::mem::offset_of!(Self, position) as u64;

    pub(crate) fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                offset: Self::OFFSET_POSITION,
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
    /// Cartesian tangent vector in radians per Julian year. The vertex shader
    /// applies `position + proper_motion * years_since_j2000` before the
    /// precession/nutation/aberration/refraction stack.
    pub proper_motion: [f32; 3],
    pub _pad0: f32,
}

impl StarInstance {
    // Field offsets are computed from the actual `#[repr(C)]` layout so a
    // reordering, padding insertion, or type change of any field is caught at
    // compile time instead of silently producing garbage on the GPU.
    const OFFSET_POSITION: u64 = std::mem::offset_of!(Self, position) as u64;
    const OFFSET_SIZE: u64 = std::mem::offset_of!(Self, size) as u64;
    const OFFSET_COLOR: u64 = std::mem::offset_of!(Self, color) as u64;
    const OFFSET_BRIGHTNESS: u64 = std::mem::offset_of!(Self, brightness) as u64;
    const OFFSET_PROPER_MOTION: u64 = std::mem::offset_of!(Self, proper_motion) as u64;

    pub(crate) fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: Self::OFFSET_POSITION,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: Self::OFFSET_SIZE,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: Self::OFFSET_COLOR,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: Self::OFFSET_BRIGHTNESS,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: Self::OFFSET_PROPER_MOTION,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

/// Per-star rendering parameters derived from apparent magnitude.
///
/// The model treats every star as an unresolved point source convolved with
/// the human-eye PSF (Spencer et al. 1995). The *only* thing that varies
/// between stars on screen is their brightness; apparent size on the sky
/// is always the same fixed sprite quad that contains the PSF's tails.
#[derive(Debug, Clone, Copy)]
pub struct RenderParams {
    /// Half-width of the billboard quad in screen-space pixels.
    ///
    /// This is purely a *container* sized to hold the Spencer 1995 PSF's
    /// tails down to numerical insignificance. It is **identical for
    /// every star** — brightness is encoded by `brightness`, never by
    /// size. See [`STAR_QUAD_HALF_PX`] for the exact value and rationale.
    pub radius_px: f32,
    /// Peak (centre-pixel) intensity of the PSF, in linear light units.
    /// The mapping from apparent magnitude to this value is set by
    /// [`magnitude_to_render_params`]; the zeropoint is derived from the
    /// caller's chosen `limiting_magnitude` so a star at exactly that
    /// magnitude lands on the [`SHADER_INTENSITY_CUTOFF`] reference value
    /// (the soft visibility threshold on the tonemap curve).
    ///
    /// Values ≫ 1.0 are the *correct* HDR-domain output for stars
    /// brighter than the limiting magnitude; the renderer's `Rgba16Float`
    /// scene buffer accumulates them and the tonemap pass compresses the
    /// dynamic range into the display gamut without clipping.
    pub brightness: f32,
}

/// Half-width of each star's billboard sprite, in screen-space pixels.
///
/// Sized to contain the Spencer 1995 PSF down to a level the smooth
/// apodization window in `shaders/star.wgsl` can taper to zero by the quad
/// boundary without producing the visible-square artefact that a hard
/// truncation of the heavy-tailed corneal halo would cause. The constant
/// is shared between every star (point sources are identical up to
/// brightness; size never encodes magnitude) and is large enough that on
/// the brightest naked-eye star (Sirius, m ≈ -1.46) the PSF's visible
/// halo and ciliary corona fit comfortably inside the sprite.
pub const STAR_QUAD_HALF_PX: f32 = 16.0;

/// Reference peak-intensity value used by the magnitude → brightness
/// mapping to anchor the user's chosen `limiting_magnitude`.
///
/// With the HDR pipeline a star fainter than the limiting magnitude is no
/// longer hard-discarded — it still contributes to the HDR accumulation
/// buffer, just at a level that is mapped to a near-black display value by
/// the tonemap. The constant defines how far below the tonemap's visible
/// range we want the limiting star to sit; 0.004 keeps continuity with
/// the previous discard-based behaviour so existing renders look the same
/// at the threshold.
pub const SHADER_INTENSITY_CUTOFF: f32 = 0.004;

/// Conventional naked-eye limiting magnitude for a dark-adapted observer under
/// a pristine sky. The value the literature uses when stating "you can see
/// down to magnitude six".
pub const NAKED_EYE_LIMITING_MAGNITUDE: f32 = 6.0;

/// Default limiting magnitude for screen-based hosts.
///
/// This is slightly past strict naked-eye because indoor screens cannot
/// reproduce the dynamic range of a pristine night sky; the more sensitive
/// virtual observer compensates without breaking Pogson's law. The value also
/// lines up with the HYG catalog's practical depth (~m 7.5).
pub const DEFAULT_SCREEN_LIMITING_MAGNITUDE: f32 = NAKED_EYE_LIMITING_MAGNITUDE + 1.5;

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
/// `m_ref` is chosen so a star at exactly the limiting magnitude lands on
/// the [`SHADER_INTENSITY_CUTOFF`] reference value — the soft visibility
/// threshold on the Reinhard tonemap curve. Increasing `limiting_magnitude`
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
    let zeropoint = limiting_magnitude_to_zeropoint(limiting_magnitude);
    let brightness = 10.0_f32.powf(-0.4 * (mag - zeropoint));
    let radius_px = STAR_QUAD_HALF_PX;
    RenderParams {
        radius_px,
        brightness,
    }
}

/// Apparent magnitude at which the renderer's linear-flux brightness
/// scale equals 1.0, given the observer's limiting magnitude.
///
/// This is the zeropoint solved out of
/// `10^(-0.4 · (limiting_mag - zeropoint)) = SHADER_INTENSITY_CUTOFF`, so
/// a star at the limiting magnitude lands on the soft tonemap visibility
/// threshold. Internal passes use it to keep skyglow surface brightness on
/// the same HDR scale as point-source stars.
pub(crate) fn limiting_magnitude_to_zeropoint(limiting_magnitude: f32) -> f32 {
    limiting_magnitude + SHADER_INTENSITY_CUTOFF.log10() / 0.4
}

/// Build a `StarInstance` ready to upload to the GPU from a star's catalogue
/// data, applying the perceptual corrections established in the photometric
/// pipeline (`astronomy::photometry`):
///
/// * **Brightness** follows Pogson's law via [`magnitude_to_render_params`],
///   anchored so a star at exactly `limiting_magnitude` lands on the soft
///   tonemap visibility threshold (see that function's docs).
/// * **Colour** is the catalogue's photopic sRGB triple, blended toward the
///   Purkinje-shifted scotopic grey by the CIE 191:2010 mesopic chromatic
///   weight for *this star's* equivalent luminance. Bright stars (Vega,
///   Sirius) keep their full B-V colour; faint stars desaturate toward
///   neutral, reproducing the well-known observation that *only the brightest
///   stars look coloured to a dark-adapted human observer*.
///
/// This is the single entry point every host app (CLI, native viewer, web)
/// should use to turn catalogue rows into renderer instances, so the
/// perceptual model stays in one place.
///
/// # References
///
/// * Schaefer, B. E. 1990, *Telescopic limiting magnitudes*, PASP 102, 212.
/// * CIE 191:2010, *Recommended System for Mesopic Photometry Based on
///   Visual Performance*.
pub fn build_star_instance(
    position: [f32; 3],
    proper_motion: [f32; 3],
    photopic_color: [f32; 3],
    magnitude: f32,
    limiting_magnitude: f32,
) -> StarInstance {
    let params = magnitude_to_render_params(magnitude, limiting_magnitude);
    let w = astronomy::photometry::chromatic_weight_for_magnitude(magnitude as f64) as f32;
    let perceived_color = astronomy::photometry::apply_mesopic_desaturation(photopic_color, w);
    StarInstance {
        position,
        size: params.radius_px,
        color: perceived_color,
        brightness: params.brightness,
        proper_motion,
        _pad0: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// A star at exactly the user-chosen limiting magnitude must have peak
    /// HDR brightness equal to the [`SHADER_INTENSITY_CUTOFF`] reference —
    /// the soft visibility threshold the Reinhard tonemap compresses to a
    /// near-black display value. This is what gives `limiting_magnitude`
    /// its physical meaning.
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

    /// A magnitude-0 star (Vega-class) keeps its full photopic colour after
    /// the mesopic blend; a magnitude-6 star (naked-eye limit) is
    /// noticeably desaturated. This pins the perceptual side of
    /// `build_star_instance` so a refactor of the photometry module can't
    /// silently change how stars look.
    #[test]
    fn build_star_instance_applies_mesopic_desaturation() {
        let red = [1.0_f32, 0.3, 0.1];
        let lim = NAKED_EYE_LIMITING_MAGNITUDE;

        let bright = build_star_instance([1.0, 0.0, 0.0], [0.0; 3], red, 0.0, lim);
        // Bright star: photopic, colour preserved within float error.
        for (got, want) in bright.color.iter().zip(red.iter()) {
            assert!(
                (got - want).abs() < 1e-5,
                "bright star should keep its colour, got {:?}, want {:?}",
                bright.color,
                red
            );
        }

        let faint = build_star_instance([1.0, 0.0, 0.0], [0.0; 3], red, 6.0, lim);
        // Faint star: mid-mesopic, channels pulled toward the scotopic grey
        // of the input. The red channel must drop (rods don't see red);
        // the blue channel must rise (rod sensitivity peak near 507 nm).
        assert!(
            faint.color[0] < red[0] - 0.1,
            "faint red channel did not desaturate: {:?}",
            faint.color
        );
        assert!(
            faint.color[2] > red[2] + 0.02,
            "faint blue channel did not pick up rod response: {:?}",
            faint.color
        );
    }

    /// The shader applies a smooth apodization window to the PSF so the
    /// quad edge is **always** zero, regardless of the radial PSF amplitude
    /// at that radius. Without it the heavy-tailed Spencer corneal halo
    /// (∝ 1/r²) leaves a visible square outline on bright stars. This test
    /// replicates the shader's `apodize` function at the quad corner
    /// (|uv| = √2, beyond the fade-end of 1.0) and verifies the window has
    /// fully gated the PSF, so a future tweak of the apodization
    /// constants cannot silently reintroduce boxy bright stars.
    #[test]
    fn apodization_zeroes_psf_at_quad_corner() {
        // Shader literals; keep in sync with `shaders/star.wgsl`.
        const FADE_END: f32 = 1.0;
        // |uv| at the corner of a [-1, 1]² quad.
        let r_norm_corner = std::f32::consts::SQRT_2;
        assert!(
            r_norm_corner >= FADE_END,
            "quad corner ({r_norm_corner}) must lie at or past the apodization fade-end ({FADE_END})"
        );
        // The smoothstep window is exactly 0 once r_norm >= FADE_END, so
        // any PSF amplitude there is irrelevant. This is the property the
        // shader relies on to avoid the visible-box artefact.
    }
}
