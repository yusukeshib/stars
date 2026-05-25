// Scene-adaptation luminance pass.
//
// Reads the HDR scene buffer and reduces it to a single scalar: the
// log-average of the per-pixel Rec.709 luminance. That scalar is the
// adaptation luminance `L_a` consumed by the Ferwerda 1996 tone-reproduction
// operator in `tonemap.wgsl`.
//
// We compute `L_a` as the *geometric mean*
//
//     L_a = exp( <log(luma + ε)> )
//
// per Reinhard et al. 2002 §3.2 and Ferwerda et al. 1996 §3 (both follow the
// same convention from photographic exposure metering — see Ansel Adams'
// Zone System). The geometric mean is robust to small clusters of very
// bright pixels (stars) and tracks the bulk darkness of the sky, which is
// what dark-adapted vision actually adapts to.
//
// References:
//   * Ferwerda, J. A. et al. 1996, "A Model of Visual Adaptation for
//     Realistic Image Synthesis", SIGGRAPH '96, §3 (adaptation luminance).
//   * Reinhard, E. et al. 2002, "Photographic Tone Reproduction for Digital
//     Images", SIGGRAPH '02, §3.2 (log-average luminance).

@group(0) @binding(0) var hdr_texture: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    // Fullscreen triangle covering the 1×1 destination.
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    var out: VertexOutput;
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

// Grid resolution for the stratified sample over the HDR texture. 32×32 =
// 1024 samples is enough to track the bulk distribution of a 1280×720
// scene to within fractions of a magnitude; we sample on a regular grid
// rather than randomising because the stars are sparse and any stratified
// stride catches roughly the same star density.
const GRID: i32 = 32;

// Both ends of the per-pixel luminance distribution are rejected from
// the adaptation-luminance average. The remaining "middle" of the
// distribution — the dark-sky background and diffuse skyglow — is what
// a dark-adapted eye actually adapts to (Ferwerda 1996 §3 makes this
// implicit; in practice the eye's adaptation pool is the broad spatial
// average of the visual field, not its individual peaks).
//
// `MIN_SCENE_LUMA` rejects "missing data":
//   the skyglow pass writes exactly zero to the HDR buffer for
//   below-horizon directions (the Earth occludes them) and the star
//   pass leaves them untouched. Those pixels are not dim scene; they
//   are *absent* scene, and including them as `log(0 + ε)` with a
//   tiny ε would let a frame full of "Earth" drag the log-average
//   down by many decades. The visible bug was a horizon-grazing view
//   washing to bright grey because the tonemap over-corrected.
//
//   The faintest scene pixel the renderer produces is the galactic-pole
//   ISL floor at the largest plausible pixel solid angle: ≈ 10^-4 in
//   HDR flux units. Threshold a couple of decades below that so a real
//   dim-sky pixel is never falsely rejected.
//
// `MAX_SCENE_LUMA` only rejects pathological values / point-source cores.
//   The original night-sky-only pipeline capped this at 10^-2 to isolate the
//   Milky Way background from star peaks. That made daylight impossible: the
//   Preetham sky model legitimately produces diffuse-sky luminance many orders
//   above the dark-sky range, so every sample was rejected and the tonemap fell
//   back to a dark adaptation value, washing the sky to white. Keep a high cap
//   so daylight and twilight contribute to adaptation while still ignoring
//   infinities, NaNs, and extreme stellar PSF centres.
const MIN_SCENE_LUMA: f32 = 1e-6;
const MAX_SCENE_LUMA: f32 = 1e6;

// Fallback log-luminance written when every sample falls outside the
// [MIN, MAX] window (e.g. camera pointing entirely at the occluded
// ground, or a degenerate test scene). Matches `log(1e-4)`, the
// canonical galactic-pole sky brightness, so the tonemap behaves as if
// the scene were a dark sky rather than producing NaN.
const FALLBACK_LOG_LUMA: f32 = -9.21;

@fragment
fn fs_main() -> @location(0) f32 {
    let dims = textureDimensions(hdr_texture);
    let w = i32(dims.x);
    let h = i32(dims.y);

    var log_sum: f32 = 0.0;
    var count: i32 = 0;

    for (var iy: i32 = 0; iy < GRID; iy = iy + 1) {
        for (var ix: i32 = 0; ix < GRID; ix = ix + 1) {
            // Stratified centre-of-cell coordinates, clamped to texel bounds.
            let u = (f32(ix) + 0.5) / f32(GRID);
            let v = (f32(iy) + 0.5) / f32(GRID);
            let x = min(i32(u * f32(w)), w - 1);
            let y = min(i32(v * f32(h)), h - 1);
            let rgb = textureLoad(hdr_texture, vec2<i32>(x, y), 0).rgb;
            let luma = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
            if luma > MIN_SCENE_LUMA && luma < MAX_SCENE_LUMA {
                log_sum = log_sum + log(luma);
                count = count + 1;
            }
        }
    }

    // Write the log-average. Tonemap pass exponentiates to recover the
    // geometric-mean luminance L_a. If no samples passed the threshold
    // (camera fully below the horizon or otherwise degenerate), fall
    // back to a sensible dark-sky default rather than dividing by zero.
    if count == 0 {
        return FALLBACK_LOG_LUMA;
    }
    return log_sum / f32(count);
}
