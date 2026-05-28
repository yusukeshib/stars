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
    eq_to_local: mat4x4<f32>,
    view_proj_local: mat4x4<f32>,
    j2000_to_date: mat4x4<f32>,
    // [Earth velocity x/c, y/c, z/c, years since J2000.0 TT].
    aberration_pm: vec4<f32>,
    // [pressure_hpa, temperature_c, unused, unused].
    refraction_params: vec4<f32>,
    // [viewport_width, viewport_height, pixel_solid_angle_sr, magnitude_zeropoint]
    viewport_pixel_sr_zeropoint: vec4<f32>,
    zenith_eq: vec4<f32>,
    extinction_k_rgb: vec4<f32>,
    // Apparent Sun direction in equatorial coordinates. `w` is angular radius.
    sun_eq_radius: vec4<f32>,
    // [linke_turbidity_eff, observer_altitude_m, solar_illuminance_lux,
    // scattering_enabled]. Effective turbidity is derived from Ångström β (V-37)
    // so the daylight and stellar paths share one (β, α, DU) state.
    atmosphere_params: vec4<f32>,
    // D65-like top-of-atmosphere solar RGB; `w` currently unused.
    solar_rgb: vec4<f32>,
    // Unified spectral-extinction state shared with the star pass (V-37):
    // [ozone_du, aerosol_beta, aerosol_alpha, unused].
    atmosphere_optics: vec4<f32>,
    // Apparent Moon direction in equatorial coordinates. `w` is approximate
    // moonlight illuminance in lux before local horizon/airmass attenuation.
    moon_eq_illuminance: vec4<f32>,
    // [angular_radius_rad, illuminated_fraction, phase_angle_rad, earth_shadow_fraction].
    moon_disk: vec4<f32>,
    // [projection_mode, full_sky_scale_x, full_sky_scale_y, full_sky_flag].
    // mode: 0=perspective, 1=Mollweide, 2=Aitoff, 3=Hammer.
    projection_params: vec4<f32>,
    // [viewpoint_mode, external_eye_x_pc, external_eye_y_pc, external_eye_z_pc].
    // mode: 0=Earth-centred sky dome, 1=external IAU-galactic parsec-scale map.
    viewpoint_params: vec4<f32>,
    // Mercury through Neptune: xyz = direction, w = angular radius.
    planet_eq_radius: array<vec4<f32>, 7>,
    // xyz = display colour, w = apparent visual magnitude.
    planet_rgb_magnitude: array<vec4<f32>, 7>,
    // [planet_count, planets_enabled, unused, unused].
    planet_params: vec4<f32>,
    // V-52a Saturn ring orientation: xyz = unit vector along Saturn's north
    // ring pole in the same J2000-equatorial frame as `planet_eq_radius`,
    // w = signed `sin B` (sub-Earth Saturnicentric latitude).
    saturn_ring_pole_sinb: vec4<f32>,
    // V-52a Saturn ring photometric state:
    // [sin_b_sun, enabled, reserved, reserved].
    saturn_ring_state: vec4<f32>,
    // V-52b Galilean moons (Io, Europa, Ganymede, Callisto):
    // xyz = J2000-equatorial unit direction, w = angular radius in radians.
    galilean_eq_radius: array<vec4<f32>, 4>,
    // V-52b Galilean moons: xyz = display colour (linear RGB),
    // w = apparent visual magnitude.
    galilean_rgb_magnitude: array<vec4<f32>, 4>,
    // V-52b Galilean moons header: [count, enabled, reserved, reserved].
    galilean_params: vec4<f32>,
    // V-52c Titan: xyz = J2000-equatorial unit direction,
    // w = angular radius in radians (sub-arcsecond, sub-pixel at every FoV).
    titan_eq_radius: vec4<f32>,
    // V-52c Titan: xyz = display colour (linear RGB),
    // w = apparent visual magnitude.
    titan_rgb_magnitude: vec4<f32>,
    // V-52c Titan header: [count, enabled, reserved, reserved].
    // count is 1 (Titan is the only Saturnian moon V-52c ships); enabled
    // is 1.0 when Saturn is above the horizon and planets are globally on.
    titan_params: vec4<f32>,
    // Hošek-Wilkie 2012 RGB sky-dome coefficients (V-38). Nine vec4s; row i
    // holds the per-channel i-th analytic coefficient (A..I) as (R, G, B, _).
    // Pre-cooked on the CPU each frame from (turbidity, albedo, sun_elev).
    // All-zero when atmosphere_params.w == 0 (atmosphere off).
    hw_coeffs: array<vec4<f32>, 9>,
    // Per-channel HW master radiance scales (R, G, B, _). Same lifecycle as
    // hw_coeffs.
    hw_radiance: vec4<f32>,
    // V-24 scintillation tail (sigma_sq_zenith, corner_hz, seed, time_s).
    // The skyglow pass does not consume it; declared only to keep the WGSL
    // view of CameraUniform aligned with the host struct.
    scintillation_params: vec4<f32>,
    // V-51c solar-eclipse state: [kind_code, obscuration, totality_weight,
    // partial_weight]. kind_code mirrors `SolarEclipseKind::shader_code`:
    // 0=none, 1=partial, 2=annular, 3=total. The renderer reads it to
    // apply the Koomen 1952 daylight falloff during obscuration and gate
    // the Baumbach 1937 corona term during totality. The analytic subtract
    // mask itself lives in the occluder array below (V-51b).
    solar_eclipse_state: vec4<f32>,
    // V-51b analytic-mask occluder array. Two vec4 rows per entry: row
    // (2i)   = (front_dir.xyz, front_radius_rad), row (2i+1) =
    // (target_code, kind_code, obscuration, 0). Target codes mirror
    // `OccluderTarget::shader_code`: 0 = Sun, 1 = Moon, 2..=8 =
    // planet[0..=6], -1 = stars (CPU-only). Iteration is gated by
    // `occluder_params.x` so the WGSL loop never reads padded rows.
    occluders: array<vec4<f32>, 32>,
    // V-51b active-occluder header: x = count, yzw reserved.
    occluder_params: vec4<f32>,
};

// Stable shader codes for `OccluderTarget` (mirrors Rust).
const OCCLUDER_TARGET_SUN: i32 = 0;
const OCCLUDER_TARGET_MOON: i32 = 1;
// V-51d planet codes: `Planet(i)` packs as `2 + i` for i in 0..=6
// (Mercury..Neptune), matching `Planet::ALL` and the renderer's
// `planet_eq_radius[i]` ordering.
const OCCLUDER_TARGET_PLANET_BASE: i32 = 2;
const MAX_OCCLUDERS: u32 = 16u;

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
const HALF_PI: f32 = 1.57079632679;
const DEG_TO_RAD: f32 = 0.017453292519943295;
const LN10: f32 = 2.30258509299;
// Mean obliquity of the ecliptic at J2000.0, IAU 2006 value
// ε₀ = 84381.406″ = 23.4392911°. These are sin/cos(ε₀), used only for
// converting J2000 equatorial camera rays into the fixed J2000 ecliptic frame
// of the zodiacal-light fit; obliquity-of-date is ROADMAP Phase 2.
const OBLIQUITY_COS_J2000: f32 = 0.917482062;
const OBLIQUITY_SIN_J2000: f32 = 0.397777156;
const EYE_PSF_SOLID_ANGLE_SR: f32 = 8.461594994075e-8;

// Dark-sky surface brightness is written directly on the renderer's physical
// brightness scale (no perceptual fudge). The Ferwerda 1996 / Reinhard 2002
// tone-reproduction operator maps that radiance to display space; sunlit and
// twilight sky terms below also stay in the same HDR scale rather than using
// visibility gates or star-atlas exposure cheats.
const PERCEPTUAL_BOOST_MAGS: f32 = 0.0;

fn s10_to_mag(s10: f32) -> f32 {
    return S10_TO_MAG_ARCSEC2_OFFSET - 2.5 * log(max(s10, 1e-12)) / log(10.0);
}

fn mag_to_s10(mu: f32) -> f32 {
    return exp(log(10.0) * ((S10_TO_MAG_ARCSEC2_OFFSET - mu) / 2.5));
}

fn zodiacal_light_s10(beta_rad: f32, sun_rel_lon_rad: f32) -> f32 {
    // Leinert §5-inspired compact zodiacal-light table fit: ecliptic dust band
    // plus the broad antisolar gegenschein once the Sun longitude is known.
    let beta = abs(beta_rad * RAD_TO_DEG);
    let lon = atan2(sin(sun_rel_lon_rad), cos(sun_rel_lon_rad));
    let elongation = acos(clamp(cos(beta_rad) * cos(lon), -1.0, 1.0)) * RAD_TO_DEG;
    let antisolar = abs(atan2(sin(lon - PI), cos(lon - PI))) * RAD_TO_DEG;
    let latitude_band = exp(-pow(beta / 14.0, 2.0));
    let forward_scatter = 1.0 + 1.15 * exp(-pow(elongation / 42.0, 2.0));
    let ecliptic_band = 48.0 * latitude_band * forward_scatter;
    let gegenschein = 32.0 * exp(-pow(antisolar / 18.0, 2.0) - pow(beta / 10.0, 2.0));
    return 18.0 + ecliptic_band + gegenschein;
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

fn hdr_flux_from_cd_m2(luminance_cd_m2: vec3<f32>, zeropoint: f32) -> vec3<f32> {
    let zp_illum = exp(-0.4 * (zeropoint + 13.99) * LN10);
    let zp_luminance = zp_illum / EYE_PSF_SOLID_ANGLE_SR;
    return luminance_cd_m2 / max(zp_luminance, 1e-20);
}

// V-38: Hošek-Wilkie 2012 analytic sky-dome radiance, ported from
// `astronomy::atmosphere::hosek_wilkie::radiance`. The per-frame coefficients
// are cooked on the CPU in `Camera::uniform_with_planets`; this evaluator is
// the angular polynomial they parameterise. Returns per-channel cd/m² (the
// raw W·m⁻²·sr⁻¹ output multiplied by 683 lm/W; see
// `HW_RADIANCE_TO_LUMINANCE_LM_PER_W` in the Rust module).
//
// Reference: Hošek, L. & Wilkie, A. 2012, ACM TOG 31(4),
//   "An Analytic Model for Full Spectral Sky-Dome Radiance", eq. (3).
const HW_RADIANCE_TO_LUMINANCE_LM_PER_W: f32 = 683.0;

fn hosek_wilkie_channel(channel: i32, theta: f32, gamma: f32) -> f32 {
    // Unpack the nine (A..I) coefficients for `channel` (0=R, 1=G, 2=B) from
    // the per-frame uniform. WGSL has no dynamic indexing into a vec3 in this
    // configuration, so unroll the three cases.
    var A: f32; var B: f32; var C: f32; var D: f32; var E: f32;
    var F: f32; var G: f32; var H: f32; var I: f32;
    if channel == 0 {
        A = camera.hw_coeffs[0].x; B = camera.hw_coeffs[1].x;
        C = camera.hw_coeffs[2].x; D = camera.hw_coeffs[3].x;
        E = camera.hw_coeffs[4].x; F = camera.hw_coeffs[5].x;
        G = camera.hw_coeffs[6].x; H = camera.hw_coeffs[7].x;
        I = camera.hw_coeffs[8].x;
    } else if channel == 1 {
        A = camera.hw_coeffs[0].y; B = camera.hw_coeffs[1].y;
        C = camera.hw_coeffs[2].y; D = camera.hw_coeffs[3].y;
        E = camera.hw_coeffs[4].y; F = camera.hw_coeffs[5].y;
        G = camera.hw_coeffs[6].y; H = camera.hw_coeffs[7].y;
        I = camera.hw_coeffs[8].y;
    } else {
        A = camera.hw_coeffs[0].z; B = camera.hw_coeffs[1].z;
        C = camera.hw_coeffs[2].z; D = camera.hw_coeffs[3].z;
        E = camera.hw_coeffs[4].z; F = camera.hw_coeffs[5].z;
        G = camera.hw_coeffs[6].z; H = camera.hw_coeffs[7].z;
        I = camera.hw_coeffs[8].z;
    }

    let cos_gamma = cos(gamma);
    let cos_theta = cos(theta);
    let exp_m = exp(E * gamma);
    let ray_m = cos_gamma * cos_gamma;
    let denom = pow(max(1.0 + I * I - 2.0 * I * cos_gamma, 1e-6), 1.5);
    let mie_m = (1.0 + cos_gamma * cos_gamma) / denom;
    let zenith = sqrt(max(cos_theta, 0.0));

    let term1 = 1.0 + A * exp(B / (cos_theta + 0.01));
    let term2 = C + D * exp_m + F * ray_m + G * mie_m + H * zenith;
    return max(term1 * term2, 0.0);
}

// Half-width (radians, 1°) of the daylight↔twilight smoothstep blend.
// Must match `DAY_NIGHT_BLEND_HALF_WINDOW_RAD` in
// `astronomy::atmosphere::hosek_wilkie`. The host extends `cook` to keep
// producing horizon-grazing coefficients across the same window so this
// fade has something physical to scale.
const HW_DAY_NIGHT_BLEND_HALF_WINDOW_RAD: f32 = 0.017453292519943295;

fn hosek_wilkie_sky_luminance_rgb(ray_dir: vec3<f32>, sin_alt: f32) -> vec3<f32> {
    let sun_dir = normalize(camera.sun_eq_radius.xyz);
    let sun_alt = sun_altitude_rad();
    // Smooth fade across the daylight ↔ twilight handoff. Below the lower
    // edge of the window the host has already zeroed the HW coefficients;
    // this weight handles the upper half of the transition so the sky does
    // not flicker dark when the apparent Sun crosses the horizon.
    let day_weight = smoothstep(
        -HW_DAY_NIGHT_BLEND_HALF_WINDOW_RAD,
        0.0,
        sun_alt,
    );
    if day_weight <= 0.0 {
        return vec3<f32>(0.0);
    }
    let theta = acos(clamp(sin_alt, 0.0, 1.0));
    let gamma = acos(clamp(dot(ray_dir, sun_dir), -1.0, 1.0));

    let r = hosek_wilkie_channel(0, theta, gamma) * camera.hw_radiance.x;
    let g = hosek_wilkie_channel(1, theta, gamma) * camera.hw_radiance.y;
    let b = hosek_wilkie_channel(2, theta, gamma) * camera.hw_radiance.z;

    // Radiometric (W·m⁻²·sr⁻¹) → photometric (cd/m²) per channel, with the
    // small Earth-Sun distance modulation the host already applied to the
    // solar illuminant. Daylight ↔ twilight cross-fade is applied last so
    // both models can additively overlap in the narrow blend window.
    let solar_scale = max(camera.atmosphere_params.z, 0.0) / 127000.0;
    return max(vec3<f32>(r, g, b), vec3<f32>(0.0))
        * HW_RADIANCE_TO_LUMINANCE_LM_PER_W
        * solar_scale
        * day_weight;
}

// Keep sunlit sky radiance on the same physical cd/m² scale as the dark-sky
// and star passes. Daytime should therefore adapt to a bright blue sky rather
// than a dim star-atlas background; stars naturally lose contrast in daylight.
const SUNLIT_SKY_EXPOSURE: f32 = 1.0;

// V-51c Koomen 1952 daylight darkening during a solar eclipse. The total-
// eclipse limit drops to roughly 1e-4 of normal daylight at maximum
// obscuration; partial phases fall off smoothly with the fraction of the
// solar disk hidden. Smoothed across the totality envelope so C2 and C3
// do not produce a step in sky luminance.
//
// Reference: Koomen, M. J., Lock, C., Packer, D. M., Scolnik, R., Tousey,
// R. & Hulburt, E. O. 1952, J. Opt. Soc. Am. 42, 353, "Measurements of
// the Brightness of the Twilight Sky."
fn solar_eclipse_daylight_factor() -> f32 {
    let kind_code = camera.solar_eclipse_state.x;
    if kind_code < 0.5 {
        return 1.0;
    }
    let obscuration = clamp(camera.solar_eclipse_state.y, 0.0, 1.0);
    let totality_weight = clamp(camera.solar_eclipse_state.z, 0.0, 1.0);
    // Linear falloff with obscuration for the partial / annular phase.
    // Annular limit ~ 1 - 0.94 obs because some sky stays lit through
    // the residual annulus; partial uses the same factor.
    let partial = max(1.0 - obscuration, 1.0e-4);
    // Totality limit: ~1e-4 of normal daylight (Koomen et al. 1952).
    let total = 1.0e-4;
    return mix(partial, total, totality_weight);
}

fn sunlit_scattering_radiance(ray_dir: vec3<f32>, sin_alt: f32, zeropoint: f32) -> vec3<f32> {
    // Sky-only fragments below the geometric horizon are still skipped;
    // the daylight ↔ twilight blend lives inside
    // `hosek_wilkie_sky_luminance_rgb` so the apparent-Sun crossing does
    // not produce a dark frame at sunset.
    if camera.atmosphere_params.w <= 0.0 || sin_alt <= 0.0 {
        return vec3<f32>(0.0);
    }
    return hdr_flux_from_cd_m2(hosek_wilkie_sky_luminance_rgb(ray_dir, sin_alt), zeropoint)
        * SUNLIT_SKY_EXPOSURE
        * solar_eclipse_daylight_factor();
}

fn disk_mask(ray_dir: vec3<f32>, center_dir: vec3<f32>, radius_rad: f32, pixel_sr: f32) -> f32 {
    let delta = acos(clamp(dot(ray_dir, center_dir), -1.0, 1.0));
    let aa = max(sqrt(max(pixel_sr, 1e-12)), radius_rad * 0.08);
    return 1.0 - smoothstep01(radius_rad - aa, radius_rad + aa, delta);
}

// V-51b: union mask of every active occluder whose target matches
// `target_code`. Returns 0 outside any front disk, ramping to 1 inside
// (with `disk_mask`'s pixel-scaled antialias band). Used by the Sun and
// Moon disk source terms to subtract the front-disk silhouettes; pixel-
// local with no depth / stencil pass.
fn occluder_subtract_mask(
    ray_dir: vec3<f32>,
    target_code: i32,
    pixel_sr: f32,
) -> f32 {
    let count = u32(max(camera.occluder_params.x, 0.0));
    var mask: f32 = 0.0;
    for (var i: u32 = 0u; i < MAX_OCCLUDERS; i = i + 1u) {
        if i >= count {
            break;
        }
        let dr = camera.occluders[i * 2u];
        let tk = camera.occluders[i * 2u + 1u];
        if i32(tk.x) != target_code {
            continue;
        }
        // Renormalise: the host-side `front_dir_eq` is the f32 Vec3 returned
        // by `direction_equatorial()` and may be slightly non-unit after the
        // f32 → f64 → f32 uniform round-trip. The V-51c path also calls
        // `normalize()` on the Moon direction; matching that here keeps the
        // analytic mask bit-identical to the V-51c golden frame.
        mask = max(mask, disk_mask(ray_dir, normalize(dr.xyz), max(dr.w, 1e-6), pixel_sr));
    }
    return mask;
}

// CPU twin: `crates/renderer/src/lunar_phase.rs::lunar_phase_lambert`.
// Keep the two in sync; the Rust copy has unit tests pinning the
// near-hemisphere sign convention (a previous bug flipped it and rendered
// the complementary phase, e.g. waxing gibbous as waning crescent).
fn lunar_phase_lambert(ray_dir: vec3<f32>, moon_dir: vec3<f32>, sun_dir: vec3<f32>, radius_rad: f32) -> f32 {
    let cos_delta = clamp(dot(ray_dir, moon_dir), -1.0, 1.0);
    let delta = acos(cos_delta);
    if delta >= radius_rad {
        return 0.0;
    }

    let r = clamp(delta / max(radius_rad, 1e-6), 0.0, 1.0);
    var tangent = ray_dir - moon_dir * cos_delta;
    if dot(tangent, tangent) < 1e-10 {
        tangent = normalize(cross(moon_dir, vec3<f32>(0.0, 0.0, 1.0)));
        if dot(tangent, tangent) < 1e-10 {
            tangent = vec3<f32>(1.0, 0.0, 0.0);
        }
    } else {
        tangent = normalize(tangent);
    }

    // The visible (near) hemisphere of the Moon has surface normals pointing
    // back toward the observer, i.e. opposite to `moon_dir` (which is the
    // observer->Moon direction). Reconstructing with +moon_dir would model the
    // far hemisphere and produce a phase complementary to the true one
    // (e.g. a waxing gibbous would render as a thin waning crescent).
    let normal = normalize(-moon_dir * sqrt(max(1.0 - r * r, 0.0)) + tangent * r);
    return clamp(dot(normal, sun_dir), 0.0, 1.0);
}

// V-51c Baumbach 1937 coronal brightness law in units of mean solar disk
// brightness. `r` is the projected radius in solar radii (`r >= 1` is
// outside the solar limb). Coefficients follow Allen 1973 / Astrophysical
// Quantities §14: a r^-2.5 + b r^-7 + c r^-17 with an overall 10^-6
// normalisation so the inner corona is at the canonical ~10^-6 of the
// solar disk. Evaluated only inside a 2° scissor and gated by
// `solar_eclipse_state.z` (totality weight).
fn baumbach_corona(r_solar: f32) -> f32 {
    let r = max(r_solar, 1.0);
    let inv = 1.0 / r;
    let r2 = inv * inv;
    let r25 = r2 * sqrt(inv);
    let r7 = r2 * r2 * r2 * inv;
    let r17 = r7 * r7 * r2 * inv;
    return 1.0e-6 * (2.10 * r25 + 8.65 * r7 + 4.40 * r17);
}

// V-26 dark-side earthshine surface luminance (cd/m²). Mirrors
// `astronomy::illuminants::earthshine_disk_luminance_cd_m2` with canonical
// Bond albedos (Earth 0.30, Moon 0.12) baked in: anchor V = 13.7 mag/arcsec²
// at phase = 60°, Lambertian Earth-from-Moon half-phase, vanishing at full
// Moon and peaking at new Moon. A workspace test pins the two formulas to
// agree on a sweep of phase angles so the GPU value cannot drift from the
// CPU helper.
fn earthshine_disk_luminance_cd_m2(phase_rad: f32) -> f32 {
    let phase = clamp(phase_rad, 0.0, PI);
    let earth_phase = 0.5 * (1.0 - cos(phase));
    // L_anchor = 1.08e5 · 10^(-0.4 · 13.7) cd/m², evaluated as a constant.
    let anchor_cd_m2 = 0.35762161;
    let anchor_earth_phase = 0.25; // 0.5 · (1 − cos 60°).
    return anchor_cd_m2 * (earth_phase / anchor_earth_phase);
}

fn sun_moon_disk_radiance(ray_dir: vec3<f32>, sin_alt: f32, zeropoint: f32, pixel_sr: f32) -> vec3<f32> {
    if camera.atmosphere_params.w <= 0.0 || sin_alt <= 0.0 {
        return vec3<f32>(0.0);
    }

    let sun_dir = normalize(camera.sun_eq_radius.xyz);
    let moon_dir = normalize(camera.moon_eq_illuminance.xyz);
    let sun_radius = max(camera.sun_eq_radius.w, 1e-6);
    let moon_radius = max(camera.moon_disk.x, 1e-6);

    // V-51b: union mask of every active occluder whose back disk is the
    // Sun. With V-51c + V-51e this covers the Moon-on-Sun pair and any
    // Mercury / Venus transit; off-event the loop short-circuits on
    // `count == 0` and the V-51c golden frames stay bit-identical.
    let totality_weight = clamp(camera.solar_eclipse_state.z, 0.0, 1.0);
    // `sun_moon_sep` is reused by the V-51c corona scissor below to mute
    // the Baumbach term inside the lunar disk during totality.
    let sun_moon_sep = acos(clamp(dot(sun_dir, moon_dir), -1.0, 1.0));
    let sun_subtract = occluder_subtract_mask(ray_dir, OCCLUDER_TARGET_SUN, pixel_sr);
    let moon_subtract = occluder_subtract_mask(ray_dir, OCCLUDER_TARGET_MOON, pixel_sr);

    var rgb = vec3<f32>(0.0);
    if dot(sun_dir, camera.zenith_eq.xyz) > 0.0 {
        let sun_solid_angle = PI * sun_radius * sun_radius;
        let sun_luminance = max(camera.atmosphere_params.z, 0.0) / max(sun_solid_angle, 1e-8);
        let sun_disk_term = hdr_flux_from_cd_m2(camera.solar_rgb.xyz * sun_luminance, zeropoint)
            * disk_mask(ray_dir, sun_dir, sun_radius, pixel_sr);
        rgb += sun_disk_term * (1.0 - sun_subtract);

        // V-51c Baumbach corona: evaluate only during totality, inside a
        // ~2° scissor centred on the Sun. The corona radiance scales with
        // the mean solar disk luminance so it stays calibrated against
        // whatever `solar_illuminance_lux` the host supplies.
        if totality_weight > 0.0 {
            let view_sun_sep = acos(clamp(dot(ray_dir, sun_dir), -1.0, 1.0));
            let scissor = 2.0 * DEG_TO_RAD;
            if view_sun_sep < scissor {
                let r_solar = max(view_sun_sep / sun_radius, 1.0);
                let inside_moon = step(view_sun_sep, moon_radius) * step(sun_moon_sep, moon_radius);
                let corona_brightness = baumbach_corona(r_solar);
                // K-corona is slightly cool-white in broadband visible.
                let corona_tint = vec3<f32>(0.90, 0.95, 1.05);
                rgb += hdr_flux_from_cd_m2(
                    corona_tint * sun_luminance * corona_brightness,
                    zeropoint,
                ) * totality_weight * (1.0 - 0.5 * inside_moon);
            }
        }
    }

    if dot(moon_dir, camera.zenith_eq.xyz) > 0.0 {
        let moon_solid_angle = PI * moon_radius * moon_radius;
        let moon_luminance = max(camera.moon_eq_illuminance.w, 0.0) / max(moon_solid_angle, 1e-8);
        let phase = lunar_phase_lambert(ray_dir, moon_dir, sun_dir, moon_radius);
        let earth_shadow = clamp(camera.moon_disk.w, 0.0, 1.0);
        let mask = disk_mask(ray_dir, moon_dir, moon_radius, pixel_sr);
        // Lit-side Lambertian shading (existing path), tinted slightly
        // warmer than D65 to track the lunar regolith spectrum.
        let lit_rgb = vec3<f32>(1.01, 1.0, 0.82) * moon_luminance * phase;
        // V-26 dark-side earthshine. The dark hemisphere of the Moon is
        // lit by reflected sunlight off Earth ("Da Vinci glow"). Lambert
        // shading uses Earth in the anti-Moon direction; this reuses the
        // same near-hemisphere normal reconstruction as the lit term so
        // the dark side glows brightest near disk centre and fades to
        // zero at the limb. The lit and dark contributions add
        // physically: a pixel near the terminator carries the weak
        // Sun-shaded value plus the constant earthshine, exactly as a
        // doubly-illuminated Lambertian surface does.
        let earth_lambert = lunar_phase_lambert(ray_dir, moon_dir, -moon_dir, moon_radius);
        let earthshine = earthshine_disk_luminance_cd_m2(camera.moon_disk.z);
        // Slightly cool tint (D65 → Rayleigh-scattered through Earth's
        // atmosphere → reflected by the lunar regolith): warmer than the
        // moonlit-sky tint but cooler than the lit-side regolith colour.
        let dark_rgb = vec3<f32>(0.78, 0.84, 1.00) * earthshine * earth_lambert;
        // V-26 extinction: apply the same per-channel Schaefer 1993 /
        // Kasten-Young 1989 path attenuation the diffuse sky pass uses
        // to the dark-side term, so a low-altitude crescent attenuates
        // its earthshine in lockstep with the surrounding sky. The lit
        // side keeps its existing un-attenuated path; physical
        // attenuation of the bright crescent is a separate change
        // outside this slice.
        var dark_attenuation = vec3<f32>(1.0);
        if camera.extinction_k_rgb.x + camera.extinction_k_rgb.y + camera.extinction_k_rgb.z > 0.0 {
            let alt_rad = asin(clamp(sin_alt, -1.0, 1.0));
            let air_x = airmass_kasten_young(max(alt_rad, 0.5 * DEG_TO_RAD));
            dark_attenuation = exp(NEG_OH_FOUR_LN10 * camera.extinction_k_rgb.xyz * air_x);
        }
        // V-51b: subtract any front disks targeting the Moon. No
        // producer emits an `OccluderTarget::Moon` entry today (V-51d
        // makes the Moon a front disk, V-51f only pairs planets), so
        // the multiplicand stays 1.0 and the V-51c golden frame is
        // unaffected; the wiring remains in place for future slices
        // that may emit planet-behind-Moon edge cases.
        rgb += hdr_flux_from_cd_m2(
            (lit_rgb + dark_rgb * dark_attenuation) * (1.0 - 0.88 * earth_shadow),
            zeropoint,
        ) * mask * (1.0 - moon_subtract);
    }

    return rgb;
}

fn magnitude_to_flux(magnitude: f32, zeropoint: f32) -> f32 {
    return exp(NEG_OH_FOUR_LN10 * (magnitude - zeropoint));
}

// V-52a Saturn ring constants.
// `Planet::ALL` indexes Saturn at position 4 (Mercury, Venus, Mars, Jupiter,
// Saturn, Uranus, Neptune).
const SATURN_PLANET_INDEX: i32 = 4;
// Ring inner / outer radii in units of Saturn's equatorial radius (Porco
// et al. 2005); these are the f32 truncation of the host-side
// `astronomy::SaturnRingApparent::BAND_RADII_R_S` values
// (`74_510 / 60_268`, `91_980 / 60_268`, `117_580 / 60_268`,
// `122_050 / 60_268`, `136_775 / 60_268`). A workspace test pins the two
// representations against each other at f32 precision so the WGSL constants
// cannot silently drift from the host.
const SATURN_RING_C_INNER_R_S: f32 = 1.2363112;
const SATURN_RING_B_INNER_R_S: f32 = 1.526183;
const SATURN_RING_B_OUTER_R_S: f32 = 1.9509524;
const SATURN_RING_A_INNER_R_S: f32 = 2.0251212;
const SATURN_RING_A_OUTER_R_S: f32 = 2.2694464;
// Per-band V-band brightness ratios relative to the B ring (Dones et al.
// 1993). The B ring anchors at 1.0; ring radiance is later scaled by Saturn's
// per-pixel flux contribution so the band sits naturally next to the body.
const SATURN_BAND_BRIGHTNESS_C: f32 = 0.20;
const SATURN_BAND_BRIGHTNESS_B: f32 = 1.00;
const SATURN_BAND_BRIGHTNESS_CASSINI: f32 = 0.15;
const SATURN_BAND_BRIGHTNESS_A: f32 = 0.50;
// Dark (unlit) face dim factor: rings on the side facing away from the Sun
// still glow faintly from ringshine and forward-scattered Saturnshine, but
// drop sharply in surface brightness.
const SATURN_RING_DARK_FACE_FACTOR: f32 = 0.10;

// Returns the V-52a Saturn ring brightness factor at `ray_dir` relative to
// the B-ring photometric anchor. Zero when Saturn is below the horizon, the
// ring pass is disabled, the pixel falls outside every band, or the pixel sits
// on the far half of the ring inside Saturn's body silhouette.
//
// `body_visual_radius_rad` is the same per-pixel-floored disk radius the body
// uses (`planet_disk_radiance`); the ring tracks it so that at naked-eye FoV,
// where Saturn's true 9″ disk is sub-pixel, the ring scales up with the body
// instead of vanishing.
fn saturn_ring_brightness(ray_dir: vec3<f32>, body_visual_radius_rad: f32) -> f32 {
    if camera.saturn_ring_state.y <= 0.0 {
        return 0.0;
    }
    let saturn_dir = normalize(camera.planet_eq_radius[SATURN_PLANET_INDEX].xyz);
    if dot(saturn_dir, camera.zenith_eq.xyz) <= 0.0 {
        return 0.0;
    }
    let r_planet = max(body_visual_radius_rad, 1e-7);
    let sin_b = camera.saturn_ring_pole_sinb.w;
    let sin_bp = camera.saturn_ring_state.x;

    // Build a 2D "sky-plane" basis at Saturn's centre. `semi_minor_dir` is the
    // direction the ring's projected semi-minor axis points along on the sky
    // (the in-sky-plane projection of the ring pole). `semi_major_dir` is the
    // perpendicular within the sky plane and stays at full ring radius because
    // it lies on the ring's true major axis.
    let pole = normalize(camera.saturn_ring_pole_sinb.xyz);
    let pole_tangent = pole - saturn_dir * dot(pole, saturn_dir);
    let pole_tangent_len = length(pole_tangent);
    let abs_sin_b = abs(sin_b);
    if abs_sin_b < 1e-3 || pole_tangent_len < 1e-4 {
        // Edge-on ring — the projected ellipse has zero area at the V-52a
        // accuracy budget. (`abs_sin_b` is the foreshortening factor below;
        // `pole_tangent_len` only flags the degenerate face-on case where the
        // semi-minor axis direction is undefined.)
        return 0.0;
    }
    let semi_minor_dir = pole_tangent / pole_tangent_len;
    let semi_major_dir = normalize(cross(saturn_dir, semi_minor_dir));

    // Offset of the ray from Saturn's centre on the sky tangent plane.
    let to_ray = ray_dir - saturn_dir * dot(ray_dir, saturn_dir);
    let u = dot(to_ray, semi_major_dir);
    let v = dot(to_ray, semi_minor_dir);

    // Foreshortening: a point at radius `r` in the ring plane projects to a
    // sky-plane displacement of (`r cosφ`, `r sinφ · sin B`). Inverting that
    // gives the true ring-plane coordinates below; `sin B` (not `cos B`) is
    // the squash factor because `B` is the sub-Earth Saturnicentric latitude,
    // i.e. 90° minus the angle between the line of sight and the ring pole.
    let ring_x = u;
    let ring_y = v / abs_sin_b;
    let ring_r = sqrt(ring_x * ring_x + ring_y * ring_y);
    let r_in_R_S = ring_r / r_planet;

    // Band lookup. The Cassini Division is the gap between B-outer and
    // A-inner; everything else outside the four bands is empty.
    var band = 0.0;
    if r_in_R_S >= SATURN_RING_C_INNER_R_S && r_in_R_S < SATURN_RING_B_INNER_R_S {
        band = SATURN_BAND_BRIGHTNESS_C;
    } else if r_in_R_S >= SATURN_RING_B_INNER_R_S && r_in_R_S < SATURN_RING_B_OUTER_R_S {
        band = SATURN_BAND_BRIGHTNESS_B;
    } else if r_in_R_S >= SATURN_RING_B_OUTER_R_S && r_in_R_S < SATURN_RING_A_INNER_R_S {
        band = SATURN_BAND_BRIGHTNESS_CASSINI;
    } else if r_in_R_S >= SATURN_RING_A_INNER_R_S && r_in_R_S < SATURN_RING_A_OUTER_R_S {
        band = SATURN_BAND_BRIGHTNESS_A;
    } else {
        return 0.0;
    }

    // Ring surface brightness falls with the projected-area foreshortening.
    // At edge-on (|sin B| = 0) the ring is invisible; the factor recovers the
    // ring's apparent integrated brightness across the elliptical annulus.
    band *= abs_sin_b;

    // Dark-face dim: the unlit side of the ring (sub-Earth and sub-Sun
    // Saturnicentric latitudes have opposite signs) glows only from ringshine.
    if sin_b * sin_bp < 0.0 {
        band *= SATURN_RING_DARK_FACE_FACTOR;
    }

    // Body shadow on the far half of the ring. "Far half" is the side of the
    // ring plane that points away from the visible pole — i.e., `v` has the
    // opposite sign of `sin B`. A ring pixel inside Saturn's body silhouette
    // on that side is occulted by the opaque body.
    if v * sin_b < 0.0 {
        let sky_offset = sqrt(u * u + v * v);
        if sky_offset < r_planet {
            return 0.0;
        }
    }

    return band;
}

fn planet_disk_radiance(ray_dir: vec3<f32>, sin_alt: f32, zeropoint: f32, pixel_sr: f32) -> vec3<f32> {
    if camera.planet_params.y <= 0.0 || sin_alt <= 0.0 {
        return vec3<f32>(0.0);
    }

    var rgb = vec3<f32>(0.0);
    let pixel_radius = sqrt(max(pixel_sr, 1e-12));
    for (var i = 0; i < 7; i = i + 1) {
        if f32(i) >= camera.planet_params.x {
            break;
        }
        let dir = normalize(camera.planet_eq_radius[i].xyz);
        if dot(dir, camera.zenith_eq.xyz) <= 0.0 {
            continue;
        }
        let angular_radius = max(camera.planet_eq_radius[i].w, 1e-7);
        // True disks are usually sub-pixel in a naked-eye planetarium view.
        // Use a small pixel-scale minimum footprint so the integrated flux is
        // visible and anti-aliased without pretending the physical radius is larger.
        let visual_radius = max(angular_radius, pixel_radius * 1.25);
        let footprint_pixels = max(PI * visual_radius * visual_radius / max(pixel_sr, 1e-12), 1.0);
        // V-51d: subtract any active front-disk occluder pointed at this
        // planet (Moon-on-Planet from `active_occluders`). Off-event the
        // producer emits no entry, the loop short-circuits, and the
        // result is bit-identical to the pre-V-51d render.
        let occluder_target = OCCLUDER_TARGET_PLANET_BASE + i32(i);
        let subtract = occluder_subtract_mask(ray_dir, occluder_target, pixel_sr);
        let flux = magnitude_to_flux(camera.planet_rgb_magnitude[i].w, zeropoint);
        let mask = disk_mask(ray_dir, dir, visual_radius, pixel_sr);
        let body_contribution = camera.planet_rgb_magnitude[i].xyz
            * flux * mask * (1.0 - subtract) / footprint_pixels;
        rgb += body_contribution;

        // V-52a: add the Saturn ring system using the same per-pixel flux
        // anchor as the body. The band factor is on the B-ring scale (1.0 at
        // the brightest band, opening-angle scaled); multiplying by the body
        // per-pixel flux keeps the ring at a physically plausible ratio to
        // Saturn's disk surface brightness without a separate calibration.
        if i == SATURN_PLANET_INDEX {
            let ring_band = saturn_ring_brightness(ray_dir, visual_radius);
            if ring_band > 0.0 {
                rgb += camera.planet_rgb_magnitude[i].xyz
                    * flux * ring_band * (1.0 - subtract) / footprint_pixels;
            }
        }
    }
    return rgb;
}

// V-52b Galilean moons. Renders Io / Europa / Ganymede / Callisto as point
// sources next to Jupiter. Mirrors `planet_disk_radiance` but skips the
// occluder array (no V-51b target codes are reserved for Galilean moons in
// this rung; V-52d will add them when shadow / occultation transits ship)
// and the Saturn-ring tail. Each moon contributes one pixel-footprint flux
// at its catalogued magnitude, attenuated by the standard above-horizon gate.
fn galilean_disk_radiance(ray_dir: vec3<f32>, sin_alt: f32, zeropoint: f32, pixel_sr: f32) -> vec3<f32> {
    if camera.galilean_params.y <= 0.0 || sin_alt <= 0.0 {
        return vec3<f32>(0.0);
    }
    if camera.planet_params.y <= 0.0 {
        // Gate the moons on the global planets toggle so they share one host
        // control with Jupiter itself; if planets are off, the moons also go.
        return vec3<f32>(0.0);
    }

    var rgb = vec3<f32>(0.0);
    let pixel_radius = sqrt(max(pixel_sr, 1e-12));
    for (var i = 0; i < 4; i = i + 1) {
        if f32(i) >= camera.galilean_params.x {
            break;
        }
        let dir = normalize(camera.galilean_eq_radius[i].xyz);
        if dot(dir, camera.zenith_eq.xyz) <= 0.0 {
            continue;
        }
        let angular_radius = max(camera.galilean_eq_radius[i].w, 1e-7);
        // The Galilean moons are < 2" across, well below the per-pixel angle
        // at every naked-eye / small-eyepiece FoV, so the visual radius is the
        // pixel radius. The shader still computes a footprint area so the
        // per-pixel HDR contribution stays scale-invariant the same way the
        // planet path does.
        let visual_radius = max(pixel_radius, angular_radius);
        let footprint_pixels = max((visual_radius * visual_radius) / max(pixel_sr, 1e-12), 1.0);
        let flux = magnitude_to_flux(camera.galilean_rgb_magnitude[i].w, zeropoint);
        let mask = disk_mask(ray_dir, dir, visual_radius, pixel_sr);
        rgb += camera.galilean_rgb_magnitude[i].xyz * flux * mask / footprint_pixels;
    }
    return rgb;
}

// V-52c Titan. Renders Titan as a point source next to Saturn. Mirrors
// `galilean_disk_radiance` but uses the scalar Titan uniform block instead
// of a four-element array — Titan is the only Saturnian moon V-52c ships,
// so iterating over a length-one array would just add cost for no gain.
// The shape stays parallel so the function reads identically to its
// Galilean sibling: above-horizon gate, pixel-footprint flux, no occluder
// subtraction (Saturnian occultation transits are deferred to a follow-on
// rung the same way Galilean ones are).
fn titan_disk_radiance(ray_dir: vec3<f32>, sin_alt: f32, zeropoint: f32, pixel_sr: f32) -> vec3<f32> {
    if camera.titan_params.y <= 0.0 || sin_alt <= 0.0 {
        return vec3<f32>(0.0);
    }
    if camera.planet_params.y <= 0.0 {
        // Gate Titan on the global planets toggle so it shares one host
        // control with Saturn itself; if planets are off, Titan also goes.
        return vec3<f32>(0.0);
    }

    let dir = normalize(camera.titan_eq_radius.xyz);
    if dot(dir, camera.zenith_eq.xyz) <= 0.0 {
        return vec3<f32>(0.0);
    }
    let pixel_radius = sqrt(max(pixel_sr, 1e-12));
    let angular_radius = max(camera.titan_eq_radius.w, 1e-7);
    // Titan's apparent disk is ≈0.4–0.9" across, well below the per-pixel
    // angle at every naked-eye / small-eyepiece FoV, so the visual radius
    // is the pixel radius. Same footprint-area scaling as the planet /
    // Galilean paths to keep the per-pixel HDR contribution scale-
    // invariant under FoV changes.
    let visual_radius = max(pixel_radius, angular_radius);
    let footprint_pixels = max((visual_radius * visual_radius) / max(pixel_sr, 1e-12), 1.0);
    let flux = magnitude_to_flux(camera.titan_rgb_magnitude.w, zeropoint);
    let mask = disk_mask(ray_dir, dir, visual_radius, pixel_sr);
    return camera.titan_rgb_magnitude.xyz * flux * mask / footprint_pixels;
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

fn twilight_sky_radiance(ray_dir: vec3<f32>, sin_alt: f32, zeropoint: f32) -> vec3<f32> {
    if camera.atmosphere_params.w <= 0.0 || sin_alt <= 0.0 {
        return vec3<f32>(0.0);
    }
    let sun_alt = sun_altitude_rad();
    if sun_alt >= 0.0 || sun_alt <= -18.0 * DEG_TO_RAD {
        return vec3<f32>(0.0);
    }

    // Single-scattering twilight approximation: direct solar irradiance is
    // exponentially attenuated along the tangent path through Earth's shadow;
    // the remaining light is Rayleigh + forward Mie scattered into the view.
    // The extinction scale is tied to the same (β, α) aerosol controls as
    // daylight via the unified V-37 state, so civil/nautical/astronomical
    // twilight are continuous radiance states rather than UI fade bands.
    let sun_dir = normalize(camera.sun_eq_radius.xyz);
    let cos_gamma = clamp(dot(ray_dir, sun_dir), -1.0, 1.0);
    let gamma = acos(cos_gamma);
    let depression = -sun_alt;
    let T = clamp(camera.atmosphere_params.x, 1.7, 10.0);
    // V-37: twilight aerosol load tracks Ångström β in lock-step with the
    // stellar k(λ) extinction path. β = 0.10 reproduces the previous
    // visibility ≈ 50 km behaviour at the rural default.
    let beta = clamp(camera.atmosphere_optics.y, 0.0, 2.0);
    let aerosol = clamp(beta / 0.10, 0.25, 5.0) * (T / 2.5);
    let shadow_tau = (0.72 + 0.18 * aerosol) * pow(depression / DEG_TO_RAD, 1.18);
    let shadow_transmission = exp(-shadow_tau);
    let view_airmass = 1.0 / max(sin_alt + 0.06, 0.06);

    let rayleigh_phase = 0.75 * (1.0 + cos_gamma * cos_gamma);
    let mie_phase = henyey_greenstein(cos_gamma, 0.78) / max(henyey_greenstein(0.0, 0.78), 1e-4);
    let scatter_phase = 0.62 * rayleigh_phase + 0.38 * clamp(mie_phase, 0.0, 10.0);
    let optical_depth = exp(-0.09 * view_airmass * (1.0 + 0.6 * aerosol));
    let luminance = max(camera.atmosphere_params.z, 0.0)
        * 2.2e-5
        * shadow_transmission
        * scatter_phase
        * optical_depth;

    // Long slant paths extinguish blue first; ozone Chappuis absorption
    // suppresses green/orange near the horizon, yielding red sunset/civil
    // twilight that cools smoothly toward astronomical twilight.
    let blue_loss = exp(-vec3<f32>(0.45, 0.22, 0.04) * view_airmass * (0.5 + aerosol));
    let ozone = clamp(camera.atmosphere_optics.x, 0.0, 600.0) / 300.0;
    let ozone_transmission = exp(-0.025 * ozone * view_airmass * vec3<f32>(0.25, 0.58, 0.16));
    let rgb_luminance = camera.solar_rgb.xyz * luminance * blue_loss * ozone_transmission;
    return hdr_flux_from_cd_m2(rgb_luminance, zeropoint) * solar_eclipse_daylight_factor();
}

// V-28: airglow is now decomposed into O I 557.7 nm + Na D 589 nm + OH
// Meinel red/IR bands, with a per-line Van Rhijn limb-brightening factor
// and a per-channel chromaticity vector. The Rust-side reference lives in
// `astronomy::skyglow::airglow_components` / `airglow_rgb_s10`. We keep
// `diffuse_sky_mag_per_arcsec2` for the ISL + zodiacal-light path; the
// airglow term is added per channel in the fragment shader (see
// `airglow_radiance_rgb`).
fn diffuse_sky_mag_per_arcsec2(l_rad: f32, b_rad: f32, beta_rad: f32, sun_rel_lon_rad: f32) -> f32 {
    let isl = mag_to_s10(isl_mag_per_arcsec2(l_rad, b_rad)) * dust_transmission(l_rad, b_rad);
    let zl = zodiacal_light_s10(beta_rad, sun_rel_lon_rad);
    return s10_to_mag(isl + zl);
}

// Per-line zenith V-band surface brightness in S10(V) at moderate solar
// activity. Total ≈ 145 S10(V), matching the Leinert §7 dark-site floor.
const AIRGLOW_GREEN_ZENITH_S10: f32 = 80.0;
const AIRGLOW_SODIUM_ZENITH_S10: f32 = 15.0;
const AIRGLOW_OH_ZENITH_S10: f32 = 50.0;

// Van Rhijn coefficient k = (R_earth / (R_earth + H))² per layer height.
// H = 90 km (O I), 92 km (Na D), 87 km (OH).
const AIRGLOW_GREEN_VR_K: f32 = 0.9722189;
const AIRGLOW_SODIUM_VR_K: f32 = 0.9716166;
const AIRGLOW_OH_VR_K: f32 = 0.9730228;

// Per-line linear-sRGB chromaticity vectors, normalised so Rec.709
// luminance Y = 1. Multiplying line V-band S10 by these gives per-channel
// S10 while preserving the V-band luminance budget.
const AIRGLOW_GREEN_RGB: vec3<f32> = vec3<f32>(0.000, 1.398, 0.000);
const AIRGLOW_SODIUM_RGB: vec3<f32> = vec3<f32>(1.229, 1.033, 0.000);
const AIRGLOW_OH_RGB: vec3<f32> = vec3<f32>(2.343, 0.703, 0.000);

fn van_rhijn_factor_wgsl(altitude_rad: f32, k: f32) -> f32 {
    let alt = max(altitude_rad, 0.0);
    let cos_alt = cos(alt);
    let denom = max(1.0 - k * cos_alt * cos_alt, 1e-6);
    return inverseSqrt(denom);
}

// Per-channel airglow surface brightness in S10(V), summed over the three
// emission systems with per-line Van Rhijn limb brightening. Activity is
// fixed at the Leinert moderate-activity reference (1.0); a future uniform
// could drive solar-cycle scaling without changing the colour split.
fn airglow_rgb_s10(altitude_rad: f32) -> vec3<f32> {
    let g = AIRGLOW_GREEN_ZENITH_S10 * van_rhijn_factor_wgsl(altitude_rad, AIRGLOW_GREEN_VR_K);
    let n = AIRGLOW_SODIUM_ZENITH_S10 * van_rhijn_factor_wgsl(altitude_rad, AIRGLOW_SODIUM_VR_K);
    let h = AIRGLOW_OH_ZENITH_S10 * van_rhijn_factor_wgsl(altitude_rad, AIRGLOW_OH_VR_K);
    return g * AIRGLOW_GREEN_RGB + n * AIRGLOW_SODIUM_RGB + h * AIRGLOW_OH_RGB;
}

// Per-pixel airglow radiance in the renderer's HDR units. Each S10(V) unit
// is a linear V-band flux per arcsec², so per-channel flux is linear in S10.
fn airglow_radiance_rgb(altitude_rad: f32, zeropoint: f32, pixel_arcsec2: f32) -> vec3<f32> {
    let s10_rgb = airglow_rgb_s10(altitude_rad);
    // mag = OFFSET − 2.5 log10(s10);  flux = 10^(−0.4 (mag − zp − boost))
    // ⇒ flux = s10 · 10^(0.4 (zp + boost − OFFSET))
    let unit_flux = exp(NEG_OH_FOUR_LN10
        * (S10_TO_MAG_ARCSEC2_OFFSET - PERCEPTUAL_BOOST_MAGS - zeropoint));
    return s10_rgb * unit_flux * pixel_arcsec2;
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

fn view_dir_from_lon_lat(lon: f32, lat: f32) -> vec3<f32> {
    let cos_lat = cos(lat);
    return normalize(vec3<f32>(sin(lon) * cos_lat, sin(lat), -cos(lon) * cos_lat));
}

fn inverse_mollweide(p: vec2<f32>) -> vec4<f32> {
    let y = p.y;
    if abs(y) > 1.0 {
        return vec4<f32>(0.0, 0.0, -1.0, 0.0);
    }
    let theta = asin(clamp(y, -1.0, 1.0));
    let cos_theta = cos(theta);
    if abs(p.x) > cos_theta + 1e-4 {
        return vec4<f32>(0.0, 0.0, -1.0, 0.0);
    }
    let lat = asin(clamp((2.0 * theta + sin(2.0 * theta)) / PI, -1.0, 1.0));
    let lon = PI * p.x / max(cos_theta, 1e-6);
    return vec4<f32>(view_dir_from_lon_lat(lon, lat), 1.0);
}

fn aitoff_project_base(lon: f32, lat: f32) -> vec2<f32> {
    let half_lon = 0.5 * lon;
    let alpha = acos(clamp(cos(lat) * cos(half_lon), -1.0, 1.0));
    let sinc = select(sin(alpha) / max(alpha, 1e-6), 1.0, abs(alpha) < 1e-6);
    return vec2<f32>(2.0 * cos(lat) * sin(half_lon) / (PI * sinc), sin(lat) / (HALF_PI * sinc));
}

fn inverse_aitoff(p: vec2<f32>) -> vec4<f32> {
    // The Aitoff projection has no compact inverse. Newton iteration seeded
    // from the normalized map coordinates is stable for this bounded all-sky
    // domain and runs only in the fullscreen background pass.
    if p.x * p.x + p.y * p.y > 1.0 + 1e-4 {
        return vec4<f32>(0.0, 0.0, -1.0, 0.0);
    }
    var lon = clamp(PI * p.x, -PI, PI);
    var lat = clamp(HALF_PI * p.y, -HALF_PI, HALF_PI);
    for (var i = 0; i < 6; i = i + 1) {
        let f = aitoff_project_base(lon, lat) - p;
        if dot(f, f) < 1e-10 {
            break;
        }
        let eps = 1e-3;
        let dlon = (aitoff_project_base(clamp(lon + eps, -PI, PI), lat) - aitoff_project_base(clamp(lon - eps, -PI, PI), lat)) / (2.0 * eps);
        let dlat = (aitoff_project_base(lon, clamp(lat + eps, -HALF_PI, HALF_PI)) - aitoff_project_base(lon, clamp(lat - eps, -HALF_PI, HALF_PI))) / (2.0 * eps);
        let det = dlon.x * dlat.y - dlon.y * dlat.x;
        if abs(det) < 1e-6 {
            break;
        }
        let delta = vec2<f32>(( f.x * dlat.y - f.y * dlat.x) / det,
                              (-f.x * dlon.y + f.y * dlon.x) / det);
        lon = clamp(lon - delta.x, -PI, PI);
        lat = clamp(lat - delta.y, -HALF_PI, HALF_PI);
    }
    let residual = aitoff_project_base(lon, lat) - p;
    if dot(residual, residual) > 1e-4 {
        return vec4<f32>(0.0, 0.0, -1.0, 0.0);
    }
    return vec4<f32>(view_dir_from_lon_lat(lon, lat), 1.0);
}

fn inverse_hammer(p: vec2<f32>) -> vec4<f32> {
    let x = p.x * 2.0 * sqrt(2.0);
    let y = p.y * sqrt(2.0);
    let under = 1.0 - (x * x) / 16.0 - (y * y) / 4.0;
    if under < -1e-5 {
        return vec4<f32>(0.0, 0.0, -1.0, 0.0);
    }
    let z = sqrt(max(under, 0.0));
    let lon = 2.0 * atan2(z * x, 2.0 * (2.0 * z * z - 1.0));
    let lat = asin(clamp(z * y, -1.0, 1.0));
    return vec4<f32>(view_dir_from_lon_lat(lon, lat), 1.0);
}

fn ray_dir_from_ndc(ndc: vec2<f32>) -> vec4<f32> {
    if camera.projection_params.w <= 0.5 {
        let clip = vec4<f32>(ndc.x, ndc.y, 0.5, 1.0);
        let world = camera.inv_view_proj * clip;
        return vec4<f32>(normalize(world.xyz / world.w), 1.0);
    }

    let p = ndc / max(camera.projection_params.yz, vec2<f32>(1e-6));
    let mode = camera.projection_params.x;
    var view = inverse_mollweide(p);
    if mode >= 2.5 {
        view = inverse_hammer(p);
    } else if mode >= 1.5 {
        view = inverse_aitoff(p);
    }
    if view.w <= 0.0 {
        return view;
    }
    return vec4<f32>(normalize((camera.inv_view_proj * vec4<f32>(view.xyz, 0.0)).xyz), 1.0);
}

fn external_world_ray_from_ndc(ndc: vec2<f32>) -> vec4<f32> {
    let near4 = camera.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
    let far4 = camera.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    let near = near4.xyz / near4.w;
    let far = far4.xyz / far4.w;
    return vec4<f32>(normalize(far - near), 1.0);
}

fn spiral_arm_modulation(phi: f32, r_kpc: f32) -> f32 {
    // Log-spiral arms as a deliberately compact visual context model. It is
    // not a star-count catalogue; it gives the external Phase-4 view enough
    // Milky-Way-disc structure to orient the local HYG stars.
    let pitch = 0.32;
    let arm_phase = phi - log(max(r_kpc, 0.2)) / pitch;
    let four_arms = 0.5 + 0.5 * cos(4.0 * arm_phase);
    return 0.65 + 0.35 * pow(max(four_arms, 0.0), 8.0);
}

fn external_galaxy_disc_radiance(ndc: vec2<f32>, zeropoint: f32) -> vec4<f32> {
    let origin = camera.viewpoint_params.yzw;
    let ray = external_world_ray_from_ndc(ndc).xyz;
    if abs(ray.z) < 1e-5 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let t = -origin.z / ray.z;
    if t <= 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    let hit = origin + ray * t;
    let r_pc = length(hit.xy);
    let r_kpc = r_pc / 1000.0;
    if r_kpc > 35.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    let phi = atan2(hit.y, hit.x);
    let disk = exp(-r_kpc / 3.0) * spiral_arm_modulation(phi, r_kpc);
    let thin_lane = 0.75 + 0.25 * exp(-pow(abs(hit.y) / 900.0, 2.0));
    let bulge = 3.0 * exp(-pow(r_kpc / 1.2, 2.0));
    let local = 0.20 * exp(-pow((r_kpc - 8.2) / 2.5, 2.0));
    let flux = (0.020 * disk * thin_lane + 0.015 * bulge + local * 0.01)
        * exp(-0.4 * LN10 * (7.5 - zeropoint));
    let tint = mix(vec3<f32>(1.0, 0.78, 0.55), vec3<f32>(0.78, 0.84, 1.0), clamp(r_kpc / 18.0, 0.0, 1.0));
    return vec4<f32>(tint * flux, 1.0);
}

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
    let zeropoint = camera.viewport_pixel_sr_zeropoint.w;
    if camera.viewpoint_params.x > 0.5 {
        return external_galaxy_disc_radiance(input.ndc, zeropoint);
    }

    // Reconstruct the camera ray direction in equatorial coordinates. The
    // perspective path unprojects through the inverse view-projection matrix;
    // all-sky maps invert the selected spherical projection first and then
    // rotate that camera-space direction back into equatorial coordinates.
    let ray = ray_dir_from_ndc(input.ndc);
    if ray.w <= 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let ray_dir = ray.xyz;

    // Equatorial → galactic, then (l, b).
    let v_gal = EQ_TO_GAL * ray_dir;
    let z = clamp(v_gal.z, -1.0, 1.0);
    let b_rad = asin(z);
    let l_rad = atan2(v_gal.y, v_gal.x);

    // Surface brightness → per-pixel linear flux on the renderer's
    // brightness scale (where a point source of magnitude `zeropoint`
    // produces unit flux; the same scale used by the star pass).
    let pixel_sr = camera.viewport_pixel_sr_zeropoint.z;
    let pixel_arcsec2 = pixel_sr * ARCSEC2_PER_SR;
    // Convert J2000 equatorial rays into the fixed J2000 ecliptic frame.
    // The Sun direction supplied by the ephemeris lets the zodiacal component
    // evaluate a real sun-relative longitude and antisolar gegenschein rather
    // than a fixed sky-space blob.
    let x_ecl = ray_dir.x;
    let y_ecl = OBLIQUITY_COS_J2000 * ray_dir.y + OBLIQUITY_SIN_J2000 * ray_dir.z;
    let z_ecl = -OBLIQUITY_SIN_J2000 * ray_dir.y + OBLIQUITY_COS_J2000 * ray_dir.z;
    let beta = asin(clamp(z_ecl, -1.0, 1.0));
    let lambda = atan2(y_ecl, x_ecl);
    let sun = normalize(camera.sun_eq_radius.xyz);
    let sun_x_ecl = sun.x;
    let sun_y_ecl = OBLIQUITY_COS_J2000 * sun.y + OBLIQUITY_SIN_J2000 * sun.z;
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
    // mix without per-band surface-brightness data. The catalogue point-source
    // colour path is physically calibrated separately; this diffuse component
    // remains V-band because the Leinert/SFD dark-sky fit is pinned there.
    let tint = vec3<f32>(0.92, 0.94, 1.00);

    // V-28: airglow is added per channel with its own chromaticity. The
    // ISL + zodiacal-light term is the tinted broadband floor; the airglow
    // term contributes the characteristic green/yellow/red emission-line
    // mottle of a dark-site night sky.
    let alt_rad_airglow = asin(sin_alt);
    let airglow_flux = airglow_radiance_rgb(alt_rad_airglow, zeropoint, pixel_arcsec2);
    let night_radiance = (tint * flux_per_pixel + airglow_flux) * attenuation;
    let moon_radiance = moonlit_sky_radiance(ray_dir, sin_alt, zeropoint);
    let twilight_radiance = twilight_sky_radiance(ray_dir, sin_alt, zeropoint);
    let day_radiance = sunlit_scattering_radiance(ray_dir, sin_alt, zeropoint);
    let disk_radiance = sun_moon_disk_radiance(ray_dir, sin_alt, zeropoint, pixel_sr);
    let planet_radiance = planet_disk_radiance(ray_dir, sin_alt, zeropoint, pixel_sr);
    let galilean_radiance = galilean_disk_radiance(ray_dir, sin_alt, zeropoint, pixel_sr);
    let titan_radiance = titan_disk_radiance(ray_dir, sin_alt, zeropoint, pixel_sr);
    return vec4<f32>(night_radiance + moon_radiance + twilight_radiance + day_radiance + disk_radiance + planet_radiance + galilean_radiance + titan_radiance, 1.0);
}
