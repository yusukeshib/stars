struct CameraUniform {
    view_proj: mat4x4<f32>,
    viewport_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) quad_pos: vec2<f32>,    // per-vertex quad corner
    @location(1) star_pos: vec3<f32>,    // per-instance world position
    @location(2) star_size: f32,         // per-instance pixel size
    @location(3) star_color: vec3<f32>,  // per-instance RGB color
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Project star position to clip space
    let clip = camera.view_proj * vec4<f32>(input.star_pos, 1.0);

    // Offset in screen space for billboard quad
    let pixel_offset = input.quad_pos * input.star_size;
    let ndc_offset = pixel_offset / camera.viewport_size * 2.0;

    out.clip_position = vec4<f32>(
        clip.x + ndc_offset.x * clip.w,
        clip.y + ndc_offset.y * clip.w,
        clip.z,
        clip.w,
    );

    out.uv = input.quad_pos;
    out.color = input.star_color;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let dist = length(input.uv);
    let alpha = smoothstep(1.0, 0.2, dist);

    if alpha < 0.01 {
        discard;
    }

    return vec4<f32>(input.color * alpha, alpha);
}
