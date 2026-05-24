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

// Small offset added inside the log so completely-black pixels (the HDR
// clear, below-horizon directions) contribute a sensible -∞ floor instead
// of -∞ poisoning the average. 1e-7 is roughly 6 magnitudes below the
// faintest contribution we expect from a star catalogue + skyglow model,
// so it acts as the "noise floor" without biasing the mean.
const LUMA_EPS: f32 = 1e-7;

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
            log_sum = log_sum + log(luma + LUMA_EPS);
            count = count + 1;
        }
    }

    // Write the log-average. Tonemap pass exponentiates to recover the
    // geometric-mean luminance L_a.
    return log_sum / f32(count);
}
