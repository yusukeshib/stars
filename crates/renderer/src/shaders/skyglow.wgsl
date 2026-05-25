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
    // D65-like top-of-atmosphere solar RGB; `w` currently unused.
    solar_rgb: vec4<f32>,
    // Apparent Moon direction in equatorial coordinates. `w` is approximate
    // moonlight illuminance in lux before local horizon/airmass attenuation.
    moon_eq_illuminance: vec4<f32>,
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
const LN10: f32 = 2.30258509299;
const EYE_PSF_SOLID_ANGLE_SR: f32 = 8.461594994075e-8;

// Dark-sky surface brightness is written directly on the renderer's
// physical brightness scale (no perceptual fudge). The Ferwerda 1996
// adaptive tone-reproduction operator in `shaders/tonemap.wgsl` takes
// care of mapping the dark-sky adaptation regime onto the display, so
// the diffuse glow ends up visible against a genuinely-dark sky without
// any constant boost being needed in this shader. The separate sunlit
// scattering path below intentionally uses a star-atlas exposure
// compression so the atmosphere layer does not erase the catalogue.
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

fn sun_altitude_rad() -> f32 {
    let sun_dir = normalize(camera.sun_eq_radius.xyz);
    return asin(clamp(dot(sun_dir, camera.zenith_eq.xyz), -1.0, 1.0));
}

fn dark_sky_visibility() -> f32 {
    if camera.atmosphere_params.w <= 0.0 {
        return 1.0;
    }
    // Astronomical twilight (-18°) is where the sky is conventionally dark;
    // civil twilight (-6°) is bright enough that the diffuse dark-sky terms
    // should no longer be visible. Blend smoothly through nautical twilight.
    return 1.0 - smoothstep01(-18.0 * DEG_TO_RAD, -6.0 * DEG_TO_RAD, sun_altitude_rad());
}

fn hdr_flux_from_cd_m2(luminance_cd_m2: vec3<f32>, zeropoint: f32) -> vec3<f32> {
    let zp_illum = exp(-0.4 * (zeropoint + 13.99) * LN10);
    let zp_luminance = zp_illum / EYE_PSF_SOLID_ANGLE_SR;
    return luminance_cd_m2 / max(zp_luminance, 1e-20);
}

fn perez_distribution(theta: f32, gamma: f32, coeffs: vec4<f32>, e: f32) -> f32 {
    let cos_theta = max(cos(theta), 0.01);
    let cos_gamma = cos(gamma);
    return (1.0 + coeffs.x * exp(coeffs.y / cos_theta))
        * (1.0 + coeffs.z * exp(coeffs.w * gamma) + e * cos_gamma * cos_gamma);
}

fn xyy_to_linear_rgb(xyy: vec3<f32>) -> vec3<f32> {
    let x = clamp(xyy.x, 1e-4, 0.9);
    let y = clamp(xyy.y, 1e-4, 0.9);
    let Y = max(xyy.z, 0.0);
    let X = x * Y / y;
    let Z = max(0.0, (1.0 - x - y) * Y / y);
    let rgb = vec3<f32>(
        3.2406 * X - 1.5372 * Y - 0.4986 * Z,
        -0.9689 * X + 1.8758 * Y + 0.0415 * Z,
        0.0557 * X - 0.2040 * Y + 1.0570 * Z,
    );
    return max(rgb, vec3<f32>(0.0));
}

fn preetham_sky_luminance_rgb(ray_dir: vec3<f32>, sin_alt: f32) -> vec3<f32> {
    let sun_dir = normalize(camera.sun_eq_radius.xyz);
    let sun_alt = sun_altitude_rad();
    let twilight = smoothstep01(-18.0 * DEG_TO_RAD, 0.0, sun_alt);
    if twilight <= 0.0 {
        return vec3<f32>(0.0);
    }

    let T = clamp(camera.atmosphere_params.x, 1.7, 10.0);
    let theta = acos(clamp(sin_alt, 0.0, 1.0));
    // Preetham's closed-form fit is defined for the Sun above the horizon.
    // For civil/nautical twilight, clamp the solar zenith to just above the
    // horizon and let the explicit twilight weight carry it to zero at -18°.
    let theta_s = clamp(PI * 0.5 - max(sun_alt, 0.5 * DEG_TO_RAD), 0.0, PI * 0.5 - 0.01);
    let gamma = acos(clamp(dot(ray_dir, sun_dir), -1.0, 1.0));

    // Preetham, Shirley & Smits 1999 analytic daylight model: zenith
    // luminance/chromaticity plus Perez all-weather angular distributions.
    let chi = (4.0 / 9.0 - T / 120.0) * (PI - 2.0 * theta_s);
    let Yz = max(0.0, ((4.0453 * T - 4.9710) * tan(chi) - 0.2155 * T + 2.4192) * 1000.0);

    let ts2 = theta_s * theta_s;
    let ts3 = ts2 * theta_s;
    let T2 = T * T;
    let xz = (0.00165 * ts3 - 0.00374 * ts2 + 0.00208 * theta_s) * T2
        + (-0.02902 * ts3 + 0.06377 * ts2 - 0.03202 * theta_s + 0.00394) * T
        + (0.11693 * ts3 - 0.21196 * ts2 + 0.06052 * theta_s + 0.25885);
    let yz = (0.00275 * ts3 - 0.00610 * ts2 + 0.00317 * theta_s) * T2
        + (-0.04214 * ts3 + 0.08970 * ts2 - 0.04153 * theta_s + 0.00516) * T
        + (0.15346 * ts3 - 0.26756 * ts2 + 0.06670 * theta_s + 0.26688);

    let coeff_y = vec4<f32>(0.1787 * T - 1.4630, -0.3554 * T + 0.4275, -0.0227 * T + 5.3251, 0.1206 * T - 2.5771);
    let coeff_x = vec4<f32>(-0.0193 * T - 0.2592, -0.0665 * T + 0.0008, -0.0004 * T + 0.2125, -0.0641 * T - 0.8989);
    let coeff_ch_y = vec4<f32>(-0.0167 * T - 0.2608, -0.0950 * T + 0.0092, -0.0079 * T + 0.2102, -0.0441 * T - 1.6537);

    let denom_y = max(perez_distribution(0.0, theta_s, coeff_y, -0.0670 * T + 0.3703), 1e-4);
    let denom_x = max(perez_distribution(0.0, theta_s, coeff_x, -0.0033 * T + 0.0452), 1e-4);
    let denom_ch_y = max(perez_distribution(0.0, theta_s, coeff_ch_y, -0.0109 * T + 0.0529), 1e-4);

    let Y = Yz * perez_distribution(theta, gamma, coeff_y, -0.0670 * T + 0.3703) / denom_y;
    let x = xz * perez_distribution(theta, gamma, coeff_x, -0.0033 * T + 0.0452) / denom_x;
    let y = yz * perez_distribution(theta, gamma, coeff_ch_y, -0.0109 * T + 0.0529) / denom_ch_y;

    // Preetham assumes mean solar irradiance at sea level; retain the small
    // Earth-Sun distance modulation from the ephemeris/illuminant pipeline.
    let solar_scale = max(camera.atmosphere_params.z, 0.0) / 127000.0;
    return xyy_to_linear_rgb(vec3<f32>(x, y, Y * twilight * solar_scale));
}

// Artistic exposure compression for the daytime/twilight atmosphere layer.
// The Preetham model returns physical cd/m² values; if those are converted
// one-to-one into the same HDR buffer as point-source stars, the sky radiance
// is so dominant that the catalogue appears to vanish. This renderer is an
// interactive star atlas rather than a daylight visibility simulator, so keep
// the sunlit sky colour/directionality while compressing its radiance relative
// to stars enough that the star layer remains present for orientation.
const SUNLIT_SKY_STAR_ATLAS_EXPOSURE: f32 = 3.0e-5;

fn sunlit_scattering_radiance(ray_dir: vec3<f32>, sin_alt: f32, zeropoint: f32) -> vec3<f32> {
    if camera.atmosphere_params.w <= 0.0 || sin_alt <= 0.0 {
        return vec3<f32>(0.0);
    }
    return hdr_flux_from_cd_m2(preetham_sky_luminance_rgb(ray_dir, sin_alt), zeropoint)
        * SUNLIT_SKY_STAR_ATLAS_EXPOSURE;
}

fn henyey_greenstein(cos_angle: f32, g: f32) -> f32 {
    let gg = g * g;
    let denom = pow(max(1.0 + gg - 2.0 * g * cos_angle, 1e-3), 1.5);
    return (1.0 - gg) / max(4.0 * PI * denom, 1e-4);
}

fn moonlit_sky_radiance(ray_dir: vec3<f32>, sin_alt: f32, zeropoint: f32) -> vec3<f32> {
    if camera.atmosphere_params.w <= 0.0 || sin_alt <= 0.0 {
        return vec3<f32>(0.0);
    }

    let moon_lux = max(camera.moon_eq_illuminance.w, 0.0);
    if moon_lux <= 0.0 {
        return vec3<f32>(0.0);
    }

    let moon_dir = normalize(camera.moon_eq_illuminance.xyz);
    let moon_sin_alt = dot(moon_dir, camera.zenith_eq.xyz);
    if moon_sin_alt <= 0.0 {
        return vec3<f32>(0.0);
    }

    let moon_alt = asin(clamp(moon_sin_alt, -1.0, 1.0));
    let moon_airmass = airmass_kasten_young(max(moon_alt, 0.5 * DEG_TO_RAD));
    let k_v = max(dot(camera.extinction_k_rgb.xyz, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0);
    let moon_transmission = exp(NEG_OH_FOUR_LN10 * k_v * moon_airmass);

    let cos_gamma = clamp(dot(ray_dir, moon_dir), -1.0, 1.0);
    let rayleigh = 0.75 * (1.0 + cos_gamma * cos_gamma);
    let mie = henyey_greenstein(cos_gamma, 0.72) / max(henyey_greenstein(0.0, 0.72), 1e-4);
    let angular = 0.35 * rayleigh + 0.65 * clamp(mie, 0.0, 8.0);

    // Clear full-moon skies are roughly a few 10^-3 cd/m² away from the lunar
    // aureole. Scale from illuminance (lux) to diffuse sky luminance with a
    // compact empirical factor until the Krisciunas-Schaefer model replaces it.
    let horizon_haze = 1.0 + 0.25 * smoothstep01(0.35, 0.0, sin_alt);
    let moon_alt_weight = smoothstep01(0.0, 10.0 * DEG_TO_RAD, moon_alt);
    let luminance = moon_lux * 0.012 * moon_transmission * moon_alt_weight * angular * horizon_haze;
    let rgb = vec3<f32>(0.78, 0.84, 1.00) * luminance;
    return hdr_flux_from_cd_m2(rgb, zeropoint);
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

    let night_radiance = tint * flux_per_pixel * attenuation * dark_sky_visibility();
    let moon_radiance = moonlit_sky_radiance(ray_dir, sin_alt, zeropoint) * dark_sky_visibility();
    let day_radiance = sunlit_scattering_radiance(ray_dir, sin_alt, zeropoint);
    return vec4<f32>(night_radiance + moon_radiance + day_radiance, 1.0);
}
