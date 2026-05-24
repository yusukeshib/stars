// Overlay line shader. One pipeline draws every overlay layer; the per-layer
// uniform supplies the right view-projection (equatorial or local) and color.

struct OverlayUniform {
    view_proj: mat4x4<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> overlay: OverlayUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = overlay.view_proj * vec4<f32>(input.position, 1.0);
    return out;
}

@fragment
fn fs_main(_in: VertexOutput) -> @location(0) vec4<f32> {
    return overlay.color;
}
