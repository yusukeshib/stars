// Tonemap pass — Reinhard 2002 §3.3 photographic operator with a key
// derived from the Ferwerda 1996 / CIE 191:2010 mesopic adaptation regime.
//
// The pipeline:
//
//   1. Read the scene adaptation luminance `L_a` (geometric mean per-pixel
//      luminance) from the 1×1 reduction target.
//   2. Convert `L_a` from the renderer's HDR flux units into absolute
//      cd/m² using the Schaefer 1990 zero-point + 1-arcmin² eye PSF —
//      so the mesopic-regime test below operates on the same physical
//      luminance the psychophysics standards (CIE 191:2010, Ferwerda 1996)
//      use.
//   3. Pick the *photographic key* `a`: where the scene's adaptation
//      patch should land on the display, expressed as a fraction of
//      display white. Bright scenes use Ansel Adams' classical
//      Zone V (`a ≈ 0.18`); dark, dark-adapted scenes use a much lower
//      key so the visible-night-sky feel survives (`a ≈ 0.03`). The blend
//      between the two regimes follows the CIE 191:2010 mesopic
//      photopic-fraction curve, which is the same weight used per-star
//      by `astronomy::photometry::mesopic_chromatic_weight`. The choice
//      of `a` is the place where the Ferwerda 1996 adaptation argument
//      enters: at threshold the TVI ratio determines the just-detectable
//      scaling, and that scaling is what the key encodes.
//   4. Apply Reinhard 2002 Eq. (4) — the extended photographic operator
//      with a white-point burn-out at `L_white` — per channel.
//
// References:
//   * Reinhard, E., Stark, M., Shirley, P., & Ferwerda, J. 2002,
//     "Photographic Tone Reproduction for Digital Images", SIGGRAPH '02,
//     §3.2 (log-average key) and §3.3 (Eq. 4 extended operator).
//   * Ferwerda, J. A. et al. 1996, "A Model of Visual Adaptation for
//     Realistic Image Synthesis", SIGGRAPH '96, §3 (TVI-based threshold
//     reproduction; cone TVI Eq. 1, rod TVI Eq. 2).
//   * CIE 191:2010, "Recommended System for Mesopic Photometry Based on
//     Visual Performance" (mesopic blend range 0.005–5 cd/m²).
//   * Schaefer, B. E. 1990, "Telescopic limiting magnitudes",
//     PASP 102, 212 (HDR-flux ↔ cd/m² conversion).
//   * Adams, A. 1948, *The Negative*, ch. 4 (Zone V = middle gray =
//     0.18 reflectance, the photopic key).

struct CameraUniform {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    // [viewport_w, viewport_h, pixel_solid_angle_sr, magnitude_zeropoint]
    viewport_pixel_sr_zeropoint: vec4<f32>,
    zenith_eq: vec4<f32>,
    extinction_k_rgb: vec4<f32>,
};

@group(0) @binding(0) var hdr_texture: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;
@group(0) @binding(2) var adaptation_texture: texture_2d<f32>;
@group(0) @binding(3) var<uniform> camera: CameraUniform;

const LN10: f32 = 2.30258509299;
const EYE_PSF_SOLID_ANGLE_SR: f32 = 8.461594994075e-8;

// CIE 191:2010 mesopic transition bounds, mirroring
// `astronomy::photometry::{MESOPIC_LOWER_CD_M2, MESOPIC_UPPER_CD_M2}`.
const MESOPIC_LOWER_CD_M2: f32 = 0.005;
const MESOPIC_UPPER_CD_M2: f32 = 5.0;

// Photographic keys for the two adaptation regimes (Reinhard 2002 §3.2):
//   * 0.18 — Ansel Adams' Zone V, the photopic / well-lit default.
//   * 0.008 — deep "low key", appropriate for night-adapted dark scenes.
//     The bulk of our scene is genuinely-dark sky (≪ 0.001 cd/m²) with a
//     few extreme point-source peaks (Sirius at ~30 cd/m²), giving an
//     enormous dynamic range and a very dim log-average. A standard
//     Zone-V / Reinhard "low key" of 0.045 places that log-average at
//     visible mid-grey, which destroys the dark-sky feel. 0.008 keeps
//     the bulk of the sky below display perceptual threshold while the
//     keyed Reinhard knee still lifts the Milky Way + faint stars into
//     visible range. The value is empirical; a follow-up Pattanaik 1998
//     multiscale adaptation model would derive it analytically.
const KEY_PHOTOPIC: f32 = 0.18;
const KEY_SCOTOPIC: f32 = 0.008;

// Display white-point in HDR-key-relative units. The Reinhard Eq. (4)
// formula
//    x' = x · (1 + x / L_white²) / (1 + x)
// asymptotes to 1.0 for `x → L_white`, so anything brighter than
// `L_white` in keyed-HDR units burns to display white. We set this high
// enough that only the genuinely-saturating stars (Sirius peak) reach
// it; the rest live in the soft Reinhard knee.
const L_WHITE: f32 = 4.0;

fn hdr_flux_to_cd_m2(flux: f32, zeropoint: f32) -> f32 {
    let zp_illum = exp(-0.4 * (zeropoint + 13.99) * LN10);
    let zp_luminance = zp_illum / EYE_PSF_SOLID_ANGLE_SR;
    return flux * zp_luminance;
}

fn mesopic_photopic_weight(l_cd_m2: f32) -> f32 {
    if l_cd_m2 <= MESOPIC_LOWER_CD_M2 {
        return 0.0;
    } else if l_cd_m2 >= MESOPIC_UPPER_CD_M2 {
        return 1.0;
    } else {
        let lo = log(MESOPIC_LOWER_CD_M2);
        let hi = log(MESOPIC_UPPER_CD_M2);
        return (log(l_cd_m2) - lo) / (hi - lo);
    }
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    var out: VertexOutput;
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let hdr = textureSample(hdr_texture, hdr_sampler, input.uv).rgb;

    // Step 1: read the scene's log-average HDR luminance, exponentiate
    // to get the geometric mean adaptation luminance `L_a` (HDR units).
    let log_la_flux = textureLoad(adaptation_texture, vec2<i32>(0, 0), 0).r;
    let la_flux = exp(log_la_flux);

    // Step 2: convert to absolute cd/m² for the mesopic-regime test.
    let zeropoint = camera.viewport_pixel_sr_zeropoint.w;
    let la_cd_m2 = hdr_flux_to_cd_m2(la_flux, zeropoint);

    // Step 3: photographic key by adaptation regime.
    let w_phot = mesopic_photopic_weight(la_cd_m2);
    let key = mix(KEY_SCOTOPIC, KEY_PHOTOPIC, w_phot);

    // Step 4: Reinhard 2002 Eq. (4) extended operator per channel.
    //   scaled = (key / L_a) · L
    //   L'    = scaled · (1 + scaled / L_white²) / (1 + scaled)
    let scaled = (key / max(la_flux, 1e-20)) * hdr;
    let lw2 = L_WHITE * L_WHITE;
    let rgb = scaled * (vec3<f32>(1.0) + scaled / vec3<f32>(lw2)) / (vec3<f32>(1.0) + scaled);

    return vec4<f32>(rgb, 1.0);
}
