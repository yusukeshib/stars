// Diffuse night-sky surface brightness pass.
//
// Renders integrated starlight, diffuse galactic light, zodiacal light,
// airglow, and an analytic interstellar-dust screen as a
// fullscreen pass over the HDR scene buffer, evaluated per fragment from
// the camera-ray direction. Subsequent star + overlay passes additively
// overlay on top of this background.
//
// Model: the ISL/DGL fit from `astronomy::skyglow::isl_mag_per_arcsec2`,
// plus Leinert-inspired zodiacal light, airglow, and an SFD98-style analytic
// dust screen. Components are summed in linear flux (S10) units before
// conversion to the renderer's physical HDR scale.
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
    // Apparent Sun direction in equatorial coordinates. `w` is angular radius.
    sun_eq_radius: vec4<f32>,
    // [turbidity, observer_altitude_m, solar_illuminance_lux, scattering_enabled].
    atmosphere_params: vec4<f32>,
    // D65-like top-of-atmosphere solar RGB; `w` reserved for moonlight.
    solar_rgb: vec4<f32>,
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
const PI: f32 = 3.14159265359;
const DEG_TO_RAD: f32 = 0.017453292519943295;

// Skyglow surface brightness is written directly on the renderer's
// physical brightness scale (no perceptual fudge). The Ferwerda 1996
// adaptive tone-reproduction operator in `shaders/tonemap.wgsl` takes
// care of mapping the dark-sky adaptation regime onto the display, so
// the diffuse glow ends up visible against a genuinely-dark sky without
// any constant boost being needed in this shader.
const PERCEPTUAL_BOOST_MAGS: f32 = 0.0;

fn s10_to_mag(s10: f32) -> f32 {
    return S10_TO_MAG_ARCSEC2_OFFSET - 2.5 * log(max(s10, 1e-12)) / log(10.0);
}

fn mag_to_s10(mu: f32) -> f32 {
    return exp(log(10.0) * ((S10_TO_MAG_ARCSEC2_OFFSET - mu) / 2.5));
}

fn zodiacal_light_s10(beta_rad: f32, sun_rel_lon_rad: f32) -> f32 {
    _ = sun_rel_lon_rad;
    // Leinert §5-inspired compact table fit. Until Phase 2 provides the real
    // solar longitude, keep only the broad ecliptic dust band; a fake fixed
    // gegenschein renders as an obvious white disk and is visually misleading.
    let beta = abs(beta_rad * RAD_TO_DEG);
    let plane = 55.0 * exp(-pow(beta / 14.0, 2.0));
    return 18.0 + plane;
}

fn dust_transmission(l_rad: f32, b_rad: f32) -> f32 {
    // Schlegel-Finkbeiner-Davis 1998-inspired analytic E(B−V) screen.
    let l_deg0 = l_rad * RAD_TO_DEG;
    let l_deg = select(l_deg0, l_deg0 - 360.0, l_deg0 > 180.0);
    let b_abs = abs(b_rad * RAD_TO_DEG);
    let ebv = 0.015 + 0.12 * exp(-(b_abs / 8.0)) + 0.08 * exp(-pow(l_deg / 45.0, 2.0)) * exp(-(b_abs / 5.0));
    return exp(NEG_OH_FOUR_LN10 * 3.1 * ebv);
}

fn smoothstep01(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn rayleigh_phase(cos_theta: f32) -> f32 {
    return 3.0 / (16.0 * PI) * (1.0 + cos_theta * cos_theta);
}

fn hg_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denom = pow(max(1.0 + g2 - 2.0 * g * cos_theta, 1e-3), 1.5);
    return (1.0 - g2) / (4.0 * PI * denom);
}

fn sunlit_scattering_radiance(ray_dir: vec3<f32>, sin_alt: f32, pixel_sr: f32, zeropoint: f32) -> vec3<f32> {
    if camera.atmosphere_params.w <= 0.0 || sin_alt <= 0.0 {
        return vec3<f32>(0.0);
    }

    let sun_dir = normalize(camera.sun_eq_radius.xyz);
    let sun_sin_alt = dot(sun_dir, camera.zenith_eq.xyz);
    let sun_alt = asin(clamp(sun_sin_alt, -1.0, 1.0));
    let twilight = smoothstep01(-18.0 * DEG_TO_RAD, 0.0, sun_alt);
    if twilight <= 0.0 {
        return vec3<f32>(0.0);
    }

    let turbidity = max(camera.atmosphere_params.x, 0.0);
    let altitude_m = max(camera.atmosphere_params.y, 0.0);
    let solar_lux = max(camera.atmosphere_params.z, 0.0);
    let altitude_scale = exp(-altitude_m / 8000.0);

    let view_alt = asin(clamp(sin_alt, 0.0, 1.0));
    let view_airmass = min(airmass_kasten_young(max(view_alt, 0.5 * DEG_TO_RAD)), 40.0);
    let sun_airmass = min(airmass_kasten_young(max(sun_alt, 0.5 * DEG_TO_RAD)), 40.0);

    let cos_theta = clamp(dot(ray_dir, sun_dir), -1.0, 1.0);
    let rayleigh_rgb = vec3<f32>(0.20, 0.45, 1.00);
    let mie_rgb = vec3<f32>(1.00, 0.96, 0.88);
    let rayleigh = rayleigh_rgb * rayleigh_phase(cos_theta) * altitude_scale;
    let mie = mie_rgb * hg_phase(cos_theta, 0.76) * turbidity * 0.18 * altitude_scale;

    // Extinguish the solar beam by the incoming slant path. Blue falls off
    // fastest, producing warmer low-Sun illumination; the view path adds
    // horizon haze without requiring a full multiple-scattering solve.
    let incoming = exp(NEG_OH_FOUR_LN10 * camera.extinction_k_rgb.xyz * sun_airmass);
    let view_haze = 1.0 - exp(-0.025 * (1.0 + turbidity) * view_airmass * altitude_scale);
    let solar_scale = solar_lux / 127000.0;

    // Convert the sky patch from an illuminance-like relative scale into the
    // renderer's magnitude zeropoint units. The constant is intentionally
    // conservative: the adaptive tonemap handles daylight brightness while
    // preserving twilight gradients instead of clipping the HDR target.
    let flux_scale = exp(NEG_OH_FOUR_LN10 * (-26.74 - zeropoint)) * pixel_sr * 2.0e-5;
    return camera.solar_rgb.xyz * incoming * (rayleigh + mie) * view_haze * twilight * solar_scale * flux_scale;
}

fn diffuse_sky_mag_per_arcsec2(l_rad: f32, b_rad: f32, beta_rad: f32, sun_rel_lon_rad: f32) -> f32 {
    let isl = mag_to_s10(isl_mag_per_arcsec2(l_rad, b_rad)) * dust_transmission(l_rad, b_rad);
    let zl = zodiacal_light_s10(beta_rad, sun_rel_lon_rad);
    let airglow = 145.0;
    return s10_to_mag(isl + zl + airglow);
}

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
    // Approximate ecliptic latitude directly from J2000 equatorial y/z using
    // ε=23.4392911°. The longitude value is reserved for Phase 2's real solar
    // ephemeris; the current zodiacal component intentionally avoids drawing a
    // fake fixed gegenschein blob.
    let x_ecl = ray_dir.x;
    let y_ecl = 0.917482062 * ray_dir.y + 0.397777156 * ray_dir.z;
    let z_ecl = -0.397777156 * ray_dir.y + 0.917482062 * ray_dir.z;
    let beta = asin(clamp(z_ecl, -1.0, 1.0));
    let lambda = atan2(y_ecl, x_ecl);
    let sun = normalize(camera.sun_eq_radius.xyz);
    let sun_x_ecl = sun.x;
    let sun_y_ecl = 0.917482062 * sun.y + 0.397777156 * sun.z;
    let sun_lambda = atan2(sun_y_ecl, sun_x_ecl);
    let sun_rel_lon = lambda - sun_lambda;
    let mu = diffuse_sky_mag_per_arcsec2(l_rad, b_rad, beta, sun_rel_lon) - PERCEPTUAL_BOOST_MAGS;
    let flux_per_arcsec2 = exp(NEG_OH_FOUR_LN10 * (mu - zeropoint));
    let flux_per_pixel = flux_per_arcsec2 * pixel_arcsec2;

    // Atmospheric extinction (same Schaefer 1993 / Kasten-Young 1989
    // pipeline as the star pass): per-channel `10^(-0.4 · k · X)`,
    // below-horizon → zero.
    let k_rgb = camera.extinction_k_rgb.xyz;
    let atmosphere_active = k_rgb.x + k_rgb.y + k_rgb.z > 0.0;
    let sin_alt = clamp(dot(ray_dir, camera.zenith_eq.xyz), -1.0, 1.0);
    var attenuation = vec3<f32>(1.0);
    if atmosphere_active {
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

    let night_radiance = tint * flux_per_pixel * attenuation;
    let day_radiance = sunlit_scattering_radiance(ray_dir, sin_alt, pixel_sr, zeropoint);
    return vec4<f32>(night_radiance + day_radiance, 1.0);
}
