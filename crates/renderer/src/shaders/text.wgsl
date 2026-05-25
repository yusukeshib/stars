// LDR text overlay shader. Vertex positions are already screen-space pixels;
// the shader converts them to clip coordinates and samples the built-in bitmap
// font atlas for coverage.

struct TextUniform {
    viewport: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> text_uniform: TextUniform;

@group(0) @binding(1)
var font_atlas: texture_2d<f32>;

@group(0) @binding(2)
var font_sampler: sampler;

struct VertexInput {
    @location(0) position_px: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let viewport = max(text_uniform.viewport.xy, vec2<f32>(1.0, 1.0));
    let ndc = vec2<f32>(
        input.position_px.x / viewport.x * 2.0 - 1.0,
        1.0 - input.position_px.y / viewport.y * 2.0,
    );
    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = input.uv;
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let coverage = textureSample(font_atlas, font_sampler, input.uv).r;
    return vec4<f32>(input.color.rgb, input.color.a * coverage);
}
