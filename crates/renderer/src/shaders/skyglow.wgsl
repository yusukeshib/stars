// Diffuse night-sky surface brightness pass.
//
// Renders the integrated-starlight (+ diffuse galactic light) glow as a
// fullscreen pass over the HDR scene buffer, evaluated per fragment from
// the camera-ray direction. Subsequent star + overlay passes additively
// overlay on top of this background.
//
// Model: an analytic fit to Leinert et al. 1998 §6's published 1-D
// surface-brightness profiles in galactic coordinates. The model has three
// components — isotropic floor, thick disk (broad in galactic latitude),
// thin disk with longitude-dependent bulge enhancement — summed in linear
// flux (S10) units. See `astronomy::skyglow::isl_mag_per_arcsec2` for the
// canonical version and unit tests against the published reference points.
//
// References:
//   * Leinert, Ch. et al. 1998, "The 1997 reference of diffuse night sky
//     brightness", A&AS 127, 1.
//   * Roach, F. E. & Megill, L. R. 1961, "Integrated starlight over the
//     sky", ApJ 133, 228 (the data Leinert §6 summarises).

struct CameraUniform {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    // [viewport_width, viewport_height, pixel_solid_angle_sr, magnitude_zeropoint]
    viewport_pixel_sr_zeropoint: vec4<f32>,
    zenith_eq: vec4<f32>,
    extinction_k_rgb: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

// IAU 1958 / J2000 equatorial → galactic rotation. WGSL `mat3x3` columns are
// the equatorial basis vectors expressed in galactic coordinates (so
// `m * v_eq = v_gal`). Constants copied from `astronomy::skyglow`'s SOFA-
// compatible table; a unit test in `astronomy::skyglow` pins their
// orthonormality.
const EQ_TO_GAL = mat3x3<f32>(
    vec3<f32>(-0.054875560, 0.494109427, -0.867666149),
    vec3<f32>(-0.873437090, -0.444829629, -0.198076373),
    vec3<f32>(-0.483835015, 0.746982244, 0.455983776),
);

// ISL model constants — must stay in lock-step with the same names in
// `astronomy::skyglow`. A test in that module pins the model against
// Leinert reference points; this shader is a literal port of the same
// closed form.
const POLE_FLUX_S10: f32 = 50.0;
const THIN_DISK_UNIFORM_S10: f32 = 60.0;
const THIN_DISK_BULGE_S10: f32 = 400.0;
const THICK_DISK_S10: f32 = 50.0;
const SIGMA_B_THIN_DEG: f32 = 4.0;
const SIGMA_B_THICK_DEG: f32 = 30.0;
const SIGMA_L_BULGE_DEG: f32 = 60.0;
const S10_TO_MAG_ARCSEC2_OFFSET: f32 = 27.78;

const RAD_TO_DEG: f32 = 57.295779513;
const NEG_OH_FOUR_LN10: f32 = -0.9210340371976184; // -0.4 · ln(10)
// 1 sr = (180·3600/π)² arcsec² ≈ 4.2545e10.
const ARCSEC2_PER_SR: f32 = 4.2545e10;

// Skyglow surface brightness is written directly on the renderer's
// physical brightness scale (no perceptual fudge). The Ferwerda 1996
// adaptive tone-reproduction operator in `shaders/tonemap.wgsl` takes
// care of mapping the dark-sky adaptation regime onto the display, so
// the diffuse glow ends up visible against a genuinely-dark sky without
// any constant boost being needed in this shader.
const PERCEPTUAL_BOOST_MAGS: f32 = 0.0;

fn isl_mag_per_arcsec2(l_rad: f32, b_rad: f32) -> f32 {
    let l_deg = l_rad * RAD_TO_DEG;
    let b_deg = b_rad * RAD_TO_DEG;

    var l_centered = l_deg;
    if l_centered > 180.0 {
        l_centered = l_centered - 360.0;
    }

    let thin_lat = exp(-(b_deg * b_deg) / (2.0 * SIGMA_B_THIN_DEG * SIGMA_B_THIN_DEG));
    let thick_lat = exp(-(b_deg * b_deg) / (2.0 * SIGMA_B_THICK_DEG * SIGMA_B_THICK_DEG));
    let bulge_lon = exp(-(l_centered * l_centered) / (2.0 * SIGMA_L_BULGE_DEG * SIGMA_L_BULGE_DEG));

    let flux = POLE_FLUX_S10
        + THICK_DISK_S10 * thick_lat
        + (THIN_DISK_UNIFORM_S10 + THIN_DISK_BULGE_S10 * bulge_lon) * thin_lat;
    return S10_TO_MAG_ARCSEC2_OFFSET - 2.5 * log(flux) / log(10.0);
}

// Kasten-Young 1989 airmass — same port as in star.wgsl.
fn airmass_kasten_young(altitude_rad: f32) -> f32 {
    let alt_deg = altitude_rad * RAD_TO_DEG;
    return 1.0 / (sin(altitude_rad) + 0.50572 * pow(alt_deg + 6.07995, -1.6364));
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

// Fullscreen triangle (big-triangle trick, no vertex buffer).
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    let cx = x * 2.0 - 1.0;
    let cy = 1.0 - y * 2.0;
    var out: VertexOutput;
    out.clip_position = vec4<f32>(cx, cy, 0.0, 1.0);
    out.ndc = vec2<f32>(cx, cy);
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Reconstruct the camera ray direction in equatorial coordinates. The
    // camera looks from the origin (the renderer's view matrix has no
    // translation for an infinite-distance celestial sphere), so a point
    // anywhere along the ray unprojects to the same direction once
    // normalised.
    let clip = vec4<f32>(input.ndc.x, input.ndc.y, 0.5, 1.0);
    let world = camera.inv_view_proj * clip;
    let ray_dir = normalize(world.xyz / world.w);

    // Equatorial → galactic, then (l, b).
    let v_gal = EQ_TO_GAL * ray_dir;
    let z = clamp(v_gal.z, -1.0, 1.0);
    let b_rad = asin(z);
    let l_rad = atan2(v_gal.y, v_gal.x);

    // Surface brightness → per-pixel linear flux on the renderer's
    // brightness scale (where a point source of magnitude `zeropoint`
    // produces unit flux; the same scale used by the star pass).
    let zeropoint = camera.viewport_pixel_sr_zeropoint.w;
    let pixel_sr = camera.viewport_pixel_sr_zeropoint.z;
    let pixel_arcsec2 = pixel_sr * ARCSEC2_PER_SR;
    let mu = isl_mag_per_arcsec2(l_rad, b_rad) - PERCEPTUAL_BOOST_MAGS;
    let flux_per_arcsec2 = exp(NEG_OH_FOUR_LN10 * (mu - zeropoint));
    let flux_per_pixel = flux_per_arcsec2 * pixel_arcsec2;

    // Atmospheric extinction (same Schaefer 1993 / Kasten-Young 1989
    // pipeline as the star pass): per-channel `10^(-0.4 · k · X)`,
    // below-horizon → zero.
    let k_rgb = camera.extinction_k_rgb.xyz;
    let atmosphere_active = k_rgb.x + k_rgb.y + k_rgb.z > 0.0;
    var attenuation = vec3<f32>(1.0);
    if atmosphere_active {
        let sin_alt = clamp(dot(ray_dir, camera.zenith_eq.xyz), -1.0, 1.0);
        let alt_rad = asin(sin_alt);
        if alt_rad <= 0.0 {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
        let x = airmass_kasten_young(alt_rad);
        attenuation = exp(NEG_OH_FOUR_LN10 * k_rgb * x);
    }

    // V-band → RGB tint. The ISL spectrum is dominated by F/G/K main-
    // sequence stars; a slightly cool-white tint approximates the colour
    // mix without per-band surface-brightness data. Replacing this with a
    // proper per-band model (Leinert §6 + Sandage 1976 colours) is scoped
    // for the same future PR that upgrades the catalogue colours to the
    // Ballesteros blackbody pipeline.
    let tint = vec3<f32>(0.92, 0.94, 1.00);

    let radiance = tint * flux_per_pixel * attenuation;
    return vec4<f32>(radiance, 1.0);
}
