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
    @location(4) star_brightness: f32,   // per-instance peak intensity multiplier
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) brightness: f32,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let clip = camera.view_proj * vec4<f32>(input.star_pos, 1.0);

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
    out.brightness = input.star_brightness;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Physical point-spread function: an isotropic Gaussian whose width is
    // identical for every star. Brightness encodes apparent magnitude via
    // Pogson's law (see `vertex::magnitude_to_render_params`); apparent size
    // on screen is *only* the PSF, never a function of the star itself.
    //
    // The coefficient must stay in sync with `PSF_QUAD_HALF_WIDTH_SIGMAS` in
    // `vertex.rs`:
    //   sigma_quad = PSF_SIGMA_PX / radius_px = 1 / PSF_QUAD_HALF_WIDTH_SIGMAS
    //   coeff      = 1 / (2 * sigma_quad^2) = PSF_QUAD_HALF_WIDTH_SIGMAS^2 / 2
    // For half-width = 5 sigma this is 12.5. A unit test in `vertex.rs`
    // (`shader_coefficient_matches_psf_constants`) pins the relationship.
    let r2 = dot(input.uv, input.uv);
    let psf = exp(-r2 * 12.5);
    let intensity = psf * input.brightness;

    // Naked-eye limiting magnitude (~6.0) falls out of this cutoff for the
    // m = 0 zeropoint used in `vertex.rs`; fainter stars discard naturally.
    if intensity < 0.004 {
        discard;
    }

    return vec4<f32>(input.color * intensity, intensity);
}
