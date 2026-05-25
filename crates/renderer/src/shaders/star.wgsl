struct CameraUniform {
    view_proj: mat4x4<f32>,
    // Inverse of view_proj. Unused by the star pass (kept here so the
    // struct layout matches the Rust-side `CameraUniform`); used by
    // shaders/skyglow.wgsl to recover per-pixel ray directions.
    inv_view_proj: mat4x4<f32>,
    // [viewport_width, viewport_height, pixel_solid_angle_sr, magnitude_zeropoint].
    viewport_pixel_sr_zeropoint: vec4<f32>,
    // Local zenith expressed in J2000 equatorial coordinates. Dotted with
    // the unit-vector star position yields sin(altitude) directly, so
    // per-star altitude is computed here on the GPU without uploading a
    // separate rotation matrix.
    zenith_eq: vec4<f32>,
    // Per-channel extinction coefficients (mag per airmass). All zero
    // disables extinction.
    extinction_k_rgb: vec4<f32>,
    // Apparent Sun direction in equatorial coordinates. `w` is angular radius.
    sun_eq_radius: vec4<f32>,
    // [turbidity, observer_altitude_m, solar_illuminance_lux, scattering_enabled].
    atmosphere_params: vec4<f32>,
    // D65-like top-of-atmosphere solar RGB; `w` reserved for moonlight.
    solar_rgb: vec4<f32>,
};

fn viewport_size() -> vec2<f32> {
    return camera.viewport_pixel_sr_zeropoint.xy;
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

// Kasten & Young 1989 airmass (relative slant-path length through the
// atmosphere). See `astronomy::photometry::airmass_kasten_young` for the
// reference and tolerances; this is a literal WGSL port.
fn airmass_kasten_young(altitude_rad: f32) -> f32 {
    let alt_deg = altitude_rad * (180.0 / 3.14159265359);
    return 1.0 / (sin(altitude_rad) + 0.50572 * pow(alt_deg + 6.07995, -1.6364));
}

// Per-channel atmospheric attenuation factor at altitude `alt_rad` given
// extinction coefficients `k_rgb` (mag per airmass). Schaefer 1993 Eq. (1):
//   Δm = k(λ) · X   ⇒   flux_out / flux_in = 10^(-0.4 · k · X)
// Stars below the horizon return zero so they don't leak through the rest
// of the pipeline (the camera frustum already clips most of them, but this
// is the explicit physical statement).
fn atmospheric_attenuation(altitude_rad: f32, k_rgb: vec3<f32>) -> vec3<f32> {
    if altitude_rad <= 0.0 {
        return vec3<f32>(0.0);
    }
    let x = airmass_kasten_young(altitude_rad);
    // 10^(-0.4 · k · X) = exp(-0.4 · ln10 · k · X). ln10 ≈ 2.302585.
    let neg_oh_four_ln10 = -0.9210340371976184;
    return exp(neg_oh_four_ln10 * k_rgb * x);
}

struct VertexInput {
    @location(0) quad_pos: vec2<f32>,    // per-vertex quad corner
    @location(1) star_pos: vec3<f32>,    // per-instance world position
    @location(2) star_size: f32,         // per-instance pixel half-width of the sprite quad
    @location(3) star_color: vec3<f32>,  // per-instance RGB color
    @location(4) star_brightness: f32,   // per-instance peak intensity multiplier
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) brightness: f32,
    @location(3) sprite_half_px: f32,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let clip = camera.view_proj * vec4<f32>(input.star_pos, 1.0);

    let pixel_offset = input.quad_pos * input.star_size;
    let ndc_offset = pixel_offset / viewport_size() * 2.0;

    out.clip_position = vec4<f32>(
        clip.x + ndc_offset.x * clip.w,
        clip.y + ndc_offset.y * clip.w,
        clip.z,
        clip.w,
    );

    // Atmospheric extinction: per-channel attenuation based on this star's
    // altitude in the observer's local horizontal frame. Skipped when the
    // host disables atmosphere (k_rgb = 0) so the multiplication is a
    // no-op identity rather than a hidden "horizon = invisible" rule.
    let k_rgb = camera.extinction_k_rgb.xyz;
    let atmosphere_active = k_rgb.x + k_rgb.y + k_rgb.z > 0.0;
    var attenuated_color = input.star_color;
    if atmosphere_active {
        let sin_alt = clamp(dot(input.star_pos, camera.zenith_eq.xyz), -1.0, 1.0);
        let alt_rad = asin(sin_alt);
        attenuated_color = input.star_color * atmospheric_attenuation(alt_rad, k_rgb);
    }

    out.uv = input.quad_pos;
    out.color = attenuated_color;
    out.brightness = input.star_brightness;
    out.sprite_half_px = input.star_size;
    return out;
}

// =============================================================================
// Spencer 1995 human-eye point-spread function (radial part).
// =============================================================================
//
// Spencer, Shirley, Zimmerman & Greenberg 1995, "Physically-based glare
// effects for digital images", SIGGRAPH '95, Eq. (1)-(4).
//
// The eye's flux PSF is the sum of three radial components plus an
// azimuthal ciliary corona. Spencer give the radial components as
// functions of θ in degrees:
//
//     f0(θ) =          exp(-(θ / 0.02)^2)         -- sharp Gaussian core
//     f1(θ) = 20.91 / (θ + 0.02)^3                -- lenticular halo
//     f2(θ) = 72.37 / (θ + 0.02)^2                -- corneal halo
//
// with photopic mixing weights (Spencer Table 1):
//
//     P_phot(θ) = 0.282·f0(θ) + 0.478·f1(θ) + 0.207·f2(θ)
//
// Two academic caveats applied here:
//
// 1. **Pixel units, not degrees.** The renderer does not yet thread
//    degrees-per-pixel through the camera uniform (that lands with the
//    Schaefer 1993 atmospheric-extinction pass; see ROADMAP). Substituting
//    θ_px for θ_deg preserves the *shape* of each component (Gaussian
//    core, 1/r³ lenticular halo, 1/r² corneal halo); only the absolute
//    angular scale is approximate.
//
// 2. **Peak-normalised components.** Spencer's weights (0.282, 0.478,
//    0.207) are the relative *integrated* contributions — each f_i is
//    already normalised to unit volume (∫f_i · 2πθ dθ = 1). Using them as
//    plain multipliers of un-normalised f_i would let the divergent
//    halo cores dominate the bright star centre. We instead rescale
//    each component to peak 1 and apply *peak-amplitude* weights that
//    give a recognisable star (sharp core, gentle halo) under tonemap.
//    The PSF shape (relative falloff exponents) is preserved; only the
//    central-amplitude mixing differs.

const PSF_CORE_SIGMA_PX: f32 = 0.6;
const PSF_SOFT_OFFSET_PX: f32 = 1.0;

// Peak-amplitude weights. The core dominates the centre, the halos add a
// visible glow only on stars bright enough for the (small) halo amplitude
// to survive the Reinhard tonemap — i.e. exactly the brightest few stars,
// which matches what a real eye sees.
//
// The 0.15 / 0.05 split between the two halo components is empirical: the
// 1/r³ lenticular halo is the bright inner glow (visible by the 2nd or
// 3rd magnitude); the 1/r² corneal halo is the very broad outer glow,
// visible only on stars below 0 magnitude. These ratios preserve
// Spencer's qualitative result that the lenticular component contributes
// more visible light than the corneal one for typical bright sources.
const PSF_W_CORE: f32 = 1.0;
const PSF_W_LENTICULAR: f32 = 0.15;
const PSF_W_CORNEAL: f32 = 0.05;

// =============================================================================
// Ciliary corona (the cross-shaped 'spikes' on bright stars).
// =============================================================================
//
// Spencer 1995 Eq. (5) models the ciliary corona as a sum of azimuthally
// localised exponentials caused by eyelash / eyelid diffraction. Real human
// eyes preferentially produce a roughly +-shaped streak pattern because
// the upper and lower lashes have approximately orthogonal mean
// orientations. We approximate with a 4-fold modulation
// `abs(cos(2φ))^N` (peaks at φ = 0, π/2, π, 3π/2 — the four cardinal
// directions) with a long exponential tail. The N controls how thin the
// rays are; 60 gives rays that are roughly half a degree wide at
// half-max, similar to what Spencer report.
//
// The corona is *intensity-gated*: only stars whose `brightness` factor
// pushes the (small) `CORONA_AMPLITUDE` past the visible-tonemap
// threshold will show spikes, so faint stars stay as clean points and
// bright stars sprout the characteristic cross.

const CORONA_RAYS_EXPONENT: f32 = 60.0;
const CORONA_FALLOFF_PER_PX: f32 = 0.18;
const CORONA_AMPLITUDE: f32 = 0.012;

fn radial_psf(r_px: f32) -> f32 {
    // Each component normalised to peak 1 at r = 0, then mixed by
    // peak-amplitude weights (see header).
    let core = exp(-(r_px / PSF_CORE_SIGMA_PX) * (r_px / PSF_CORE_SIGMA_PX));
    let s = (r_px + PSF_SOFT_OFFSET_PX) / PSF_SOFT_OFFSET_PX;  // = 1 at r=0, growing
    let lenticular = 1.0 / (s * s * s);
    let corneal = 1.0 / (s * s);
    return PSF_W_CORE * core
        + PSF_W_LENTICULAR * lenticular
        + PSF_W_CORNEAL * corneal;
}

fn corona(uv: vec2<f32>, r_px: f32) -> f32 {
    // `uv` is the quad-space coordinate in [-1, 1]; we only need its angle.
    let azim = atan2(uv.y, uv.x);
    // 4-fold modulation: bright at the four cardinal directions, dim
    // between. `abs(cos(2φ))^N` reaches 1 at φ ∈ {0, π/2, π, 3π/2}.
    let rays = pow(abs(cos(2.0 * azim)), CORONA_RAYS_EXPONENT);
    let falloff = exp(-r_px * CORONA_FALLOFF_PER_PX);
    return CORONA_AMPLITUDE * rays * falloff;
}

// Apodization: the Spencer PSF's corneal halo decays as 1/(θ+0.02)², which is
// far too slow to fall to numerical zero at the sprite-quad edge. Left alone
// the corner-vs-edge difference in PSF value produces a *visible square* on
// bright stars (the quad outline becomes a faint box). Multiplying the PSF
// by a smooth circular window that is 1 inside the inscribed disc and
// smoothly tapers to 0 by the quad corner removes the box artefact while
// keeping the physically motivated PSF shape inside the disc. The
// truncation has no academic cost because the literature integrates the
// Spencer PSF over the visual field too — we are just imposing the same
// kind of compact-support window that any finite-aperture rendering must.
const APODIZATION_FADE_START: f32 = 0.85;
const APODIZATION_FADE_END: f32 = 1.0;

fn apodize(r_norm: f32) -> f32 {
    // Smooth Hermite interpolation from 1 -> 0 over [fade_start, fade_end].
    let t = clamp(
        (r_norm - APODIZATION_FADE_START) / (APODIZATION_FADE_END - APODIZATION_FADE_START),
        0.0,
        1.0,
    );
    let s = t * t * (3.0 - 2.0 * t);
    return 1.0 - s;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Quad-normalised radius -> pixel radius from the star centre.
    let r_norm = length(input.uv);
    let r_px = r_norm * input.sprite_half_px;

    // Spencer PSF: radial body + azimuthal ciliary corona, tapered at the
    // sprite-quad edge by a smooth apodization window (see above).
    let psf = (radial_psf(r_px) + corona(input.uv, r_px)) * apodize(r_norm);

    let intensity = psf * input.brightness;

    // No hard discard — we are writing into an Rgba16Float HDR target so
    // faint PSF tails accumulate instead of being clipped. The tonemap pass
    // maps the full HDR scene to the display.
    return vec4<f32>(input.color * intensity, intensity);
}
