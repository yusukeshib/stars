// Tonemap pass.
//
// Reads the HDR scene texture written by the star + overlay passes, applies a
// global tone-reproduction operator, and writes the result to the final
// (sRGB) framebuffer for presentation.
//
// We need this because the scene is now rendered in linear floating-point
// (`Rgba16Float`): bright stars saturate well past 1.0 in linear flux (Pogson's
// law guarantees that), and the diffuse contributions accumulated from many
// faint PSF tails would be crushed to black by a naive `clamp`. A
// luminance-aware tone curve is the right way to map the HDR scene into the
// limited dynamic range of the display while preserving both the brightest and
// the faintest light.
//
// Operator: a luminance-preserving Reinhard variant.
//
//   L  = dot(rgb, [0.2126, 0.7152, 0.0722])     // Rec. 709 luma
//   L' = L / (1 + L)
//   rgb_out = rgb * (L' / L)
//
// Choice of Reinhard over ACES Filmic is deliberate: ACES darkens the low end
// (its toe pulls 0.01-ish values down to ~0.004), which would visibly dim the
// faint-star regime we are specifically trying to preserve. Plain Reinhard is
// near-identity for small inputs (L' ≈ L for L ≪ 1) and asymptotically maps
// the highlights into [0, 1) without ever clipping, which matches the
// physics-of-an-imaging-system metaphor the rest of the pipeline uses.
//
// References:
//   * Reinhard, E., Stark, M., Shirley, P., & Ferwerda, J. 2002,
//     "Photographic Tone Reproduction for Digital Images", SIGGRAPH '02.
//   * ITU-R Recommendation BT.709 for the luminance coefficients.

@group(0) @binding(0) var hdr_texture: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Fullscreen triangle covering the viewport via gl_VertexIndex = 0..3.
// No vertex buffer needed.
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    // Three vertices forming a triangle that covers the full clip-space
    // rectangle [-1, 1]^2 (the standard "big triangle" trick — one less
    // primitive than a quad, no diagonal seam to worry about).
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

    // Rec. 709 luma — same coefficients used downstream for sRGB encoding,
    // so the tone curve operates on the perceptually relevant scalar.
    let luma = dot(hdr, vec3<f32>(0.2126, 0.7152, 0.0722));

    // Below this we are dominated by float noise; bail to avoid 0/0.
    let safe_luma = max(luma, 1e-6);
    let mapped_luma = safe_luma / (1.0 + safe_luma);

    // Preserve chroma: scale the input by the luma compression ratio.
    let rgb = hdr * (mapped_luma / safe_luma);

    return vec4<f32>(rgb, 1.0);
}
