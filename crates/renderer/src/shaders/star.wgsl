struct CameraUniform {
    view_proj: mat4x4<f32>,
    // Inverse of view_proj. Unused by the star pass (kept here so the
    // struct layout matches the Rust-side `CameraUniform`); used by
    // shaders/skyglow.wgsl to recover per-pixel ray directions.
    inv_view_proj: mat4x4<f32>,
    eq_to_local: mat4x4<f32>,
    view_proj_local: mat4x4<f32>,
    j2000_to_date: mat4x4<f32>,
    // [Earth velocity x/c, y/c, z/c, years since J2000.0 TT].
    aberration_pm: vec4<f32>,
    // [pressure_hpa, temperature_c, unused, unused].
    refraction_params: vec4<f32>,
    // [viewport_width, viewport_height, pixel_solid_angle_sr, magnitude_zeropoint].
    viewport_pixel_sr_zeropoint: vec4<f32>,
    // Local zenith expressed in J2000 equatorial coordinates. Dotted with
    // the unit-vector star position yields sin(altitude) directly for
    // extinction; `eq_to_local` carries the full horizontal-frame rotation
    // for atmospheric refraction.
    zenith_eq: vec4<f32>,
    // Per-channel extinction coefficients (mag per airmass). All zero
    // disables extinction.
    extinction_k_rgb: vec4<f32>,
    // Apparent Sun direction in equatorial coordinates. `w` is angular radius.
    sun_eq_radius: vec4<f32>,
    // [linke_turbidity_eff, observer_altitude_m, solar_illuminance_lux,
    // scattering_enabled]. Effective turbidity is derived from Ångström β (V-37)
    // so the daylight and stellar paths share one (β, α, DU) state.
    atmosphere_params: vec4<f32>,
    // D65-like top-of-atmosphere solar RGB; `w` currently unused.
    solar_rgb: vec4<f32>,
    // Unified spectral-extinction state shared with the daylight pass (V-37):
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
    // Planet uniform block. The star shader does not consume these; they are
    // declared here only because WGSL uniform blocks must mirror the host
    // struct in order to read fields past them (V-24 scintillation_params).
    planet_eq_radius_pad: array<vec4<f32>, 7>,
    planet_rgb_magnitude_pad: array<vec4<f32>, 7>,
    planet_params_pad: vec4<f32>,
    // V-52a Saturn ring uniform block. The star pass does not draw the ring;
    // declared only to keep the WGSL view of `CameraUniform` aligned with the
    // host struct so fields past it (Hošek-Wilkie coefficients, scintillation,
    // solar eclipse, occluder array) read from the right offsets.
    saturn_ring_pole_sinb_pad: vec4<f32>,
    saturn_ring_state_pad: vec4<f32>,
    // V-52b Galilean-moon uniform block. The star pass does not draw the
    // moons (they go through the skyglow pass's point-light pipeline);
    // declared here so the WGSL view of `CameraUniform` keeps reading
    // fields past it from the right offsets.
    galilean_eq_radius_pad: array<vec4<f32>, 4>,
    galilean_rgb_magnitude_pad: array<vec4<f32>, 4>,
    galilean_params_pad: vec4<f32>,
    // V-52c Titan uniform block. Same reasoning as the Galilean pad — the
    // star pass does not draw Titan (it ships in the skyglow point-light
    // pipeline alongside the Galilean moons); declared here so the WGSL
    // view of `CameraUniform` keeps reading fields past it from the right
    // offsets.
    titan_eq_radius_pad: vec4<f32>,
    titan_rgb_magnitude_pad: vec4<f32>,
    titan_params_pad: vec4<f32>,
    // Hošek-Wilkie coefficient block (V-38). Same reason as the planet pad.
    hw_coeffs_pad: array<vec4<f32>, 9>,
    hw_radiance_pad: vec4<f32>,
    // V-24 scintillation: [sigma_sq_zenith, corner_hz_zenith,
    // seed_as_f32, time_seconds_mod_day]. `sigma_sq_zenith == 0` disables
    // the per-star modulation in the vertex shader.
    scintillation_params: vec4<f32>,
    // V-51c solar-eclipse state. Unused by the star pass (the Sun
    // doesn't occult catalog stars); kept here so the struct layout
    // matches the Rust-side `CameraUniform`.
    solar_eclipse_state_pad: vec4<f32>,
    // V-51b/d analytic-mask occluder array. Same layout as in
    // `shaders/skyglow.wgsl`: two `vec4` rows per entry, packed by
    // `CameraUniform::occluders` and counted by
    // `occluder_params.x`. The star pass iterates entries whose
    // target code is `OCCLUDER_TARGET_STARS` (-1) and discards
    // sprites whose direction falls inside the front disk
    // (V-51d lunar occultation of catalog stars).
    occluders: array<vec4<f32>, 32>,
    occluder_params: vec4<f32>,
};

// Mirrors `astronomy::occultation::OccluderTarget::Stars.shader_code()`.
const OCCLUDER_TARGET_STARS: i32 = -1;
const MAX_STAR_OCCLUDERS: u32 = 16u;

fn viewport_size() -> vec2<f32> {
    return camera.viewport_pixel_sr_zeropoint.xy;
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

const PI: f32 = 3.14159265359;
const HALF_PI: f32 = 1.57079632679;

// IAU 1958 / J2000 equatorial → galactic rotation. Columns are the
// equatorial basis vectors expressed in galactic coordinates, so
// `EQ_TO_GAL * v_eq` yields `(x toward l=0, y toward l=90, z=N galactic pole)`.
const EQ_TO_GAL = mat3x3<f32>(
    vec3<f32>(-0.054875560, 0.494109427, -0.867666149),
    vec3<f32>(-0.873437090, -0.444829629, -0.198076373),
    vec3<f32>(-0.483835015, 0.746982244, 0.455983776),
);

fn all_sky_lon_lat_from_view_dir(view_dir: vec3<f32>) -> vec2<f32> {
    let d = normalize(view_dir);
    return vec2<f32>(atan2(d.x, -d.z), asin(clamp(d.y, -1.0, 1.0)));
}

fn mollweide_project(lon: f32, lat: f32) -> vec2<f32> {
    // Solve 2θ + sin(2θ) = π sin φ with a fixed Newton iteration. θ stays in
    // [-π/2, π/2]; the polar cases converge safely under the clamp.
    var theta = lat;
    for (var i = 0; i < 6; i = i + 1) {
        let f = 2.0 * theta + sin(2.0 * theta) - PI * sin(lat);
        let fp = 2.0 + 2.0 * cos(2.0 * theta);
        theta = theta - f / max(fp, 1e-4);
        theta = clamp(theta, -HALF_PI, HALF_PI);
    }
    return vec2<f32>((lon / PI) * cos(theta), sin(theta));
}

fn aitoff_project(lon: f32, lat: f32) -> vec2<f32> {
    let half_lon = 0.5 * lon;
    let alpha = acos(clamp(cos(lat) * cos(half_lon), -1.0, 1.0));
    let sinc = select(sin(alpha) / max(alpha, 1e-6), 1.0, abs(alpha) < 1e-6);
    return vec2<f32>(2.0 * cos(lat) * sin(half_lon) / (PI * sinc), sin(lat) / (HALF_PI * sinc));
}

fn hammer_project(lon: f32, lat: f32) -> vec2<f32> {
    let half_lon = 0.5 * lon;
    let denom = sqrt(max(1.0 + cos(lat) * cos(half_lon), 1e-6));
    return vec2<f32>(cos(lat) * sin(half_lon) / denom, sin(lat) / denom);
}

fn all_sky_project_from_view_dir(view_dir: vec3<f32>) -> vec4<f32> {
    let lon_lat = all_sky_lon_lat_from_view_dir(view_dir);
    let lon = lon_lat.x;
    let lat = lon_lat.y;
    let mode = camera.projection_params.x;
    var p = mollweide_project(lon, lat);
    if mode >= 2.5 {
        p = hammer_project(lon, lat);
    } else if mode >= 1.5 {
        p = aitoff_project(lon, lat);
    }
    let scaled = p * camera.projection_params.yz;
    return vec4<f32>(scaled, 0.5, 1.0);
}

fn project_equatorial_direction(direction: vec3<f32>, local_direction: vec3<f32>, atmosphere_active: bool) -> vec4<f32> {
    if camera.projection_params.w > 0.5 {
        let view_dir = select(
            (camera.view_proj * vec4<f32>(direction, 0.0)).xyz,
            (camera.view_proj_local * vec4<f32>(local_direction, 0.0)).xyz,
            atmosphere_active,
        );
        return all_sky_project_from_view_dir(view_dir);
    }
    return select(
        camera.view_proj * vec4<f32>(direction, 1.0),
        camera.view_proj_local * vec4<f32>(local_direction, 1.0),
        atmosphere_active,
    );
}

// Kasten & Young 1989 airmass (relative slant-path length through the
// atmosphere). See `astronomy::photometry::airmass_kasten_young` for the
// reference and tolerances; this is a literal WGSL port.
fn airmass_kasten_young(altitude_rad: f32) -> f32 {
    let alt_deg = altitude_rad * (180.0 / 3.14159265359);
    return 1.0 / (sin(altitude_rad) + 0.50572 * pow(alt_deg + 6.07995, -1.6364));
}

// Per-channel atmospheric attenuation factor at altitude `alt_rad` given
// extinction coefficients `k_rgb` (mag per airmass). Schaefer 1993 Eq. (1):
//   Δm = k(λ) · X   ⇒   flux_out / flux_in = 10^(-0.4 · k · X)
// Stars below the horizon return zero so they don't leak through the rest
// of the pipeline (the camera frustum already clips most of them, but this
// is the explicit physical statement).
fn atmospheric_attenuation(altitude_rad: f32, k_rgb: vec3<f32>) -> vec3<f32> {
    if altitude_rad <= 0.0 {
        return vec3<f32>(0.0);
    }
    let x = airmass_kasten_young(altitude_rad);
    // 10^(-0.4 · k · X) = exp(-0.4 · ln10 · k · X). ln10 ≈ 2.302585.
    let neg_oh_four_ln10 = -0.9210340371976184;
    return exp(neg_oh_four_ln10 * k_rgb * x);
}

fn refracted_altitude_rad(true_altitude_rad: f32) -> f32 {
    // Saemundsson 1986 / Meeus ch. 16 apparent refraction for standard
    // pressure/temperature. Returns apparent altitude from true altitude.
    // The expression is well-behaved down to about -1°, below which a simple
    // stellar renderer should keep objects hidden rather than invent ducting.
    let alt_deg = true_altitude_rad * (180.0 / 3.14159265359);
    if alt_deg < -1.0 || alt_deg > 89.9 {
        return true_altitude_rad;
    }
    let pressure_scale = max(camera.refraction_params.x, 0.0) / 1010.0;
    let temp_k = max(273.0 + camera.refraction_params.y, 150.0);
    let weather_scale = pressure_scale * 283.0 / temp_k;
    let r_arcmin = 1.02 / tan((alt_deg + 10.3 / (alt_deg + 5.11)) * (3.14159265359 / 180.0)) * weather_scale;
    return true_altitude_rad + (r_arcmin / 60.0) * (3.14159265359 / 180.0);
}

fn apply_annual_aberration(eq_j2000_dir: vec3<f32>) -> vec3<f32> {
    let beta = camera.aberration_pm.xyz;
    let dot_beta = dot(eq_j2000_dir, beta);
    return normalize(eq_j2000_dir + beta - dot_beta * eq_j2000_dir);
}

fn corrected_j2000_direction(position: vec3<f32>, proper_motion: vec3<f32>) -> vec3<f32> {
    let years = camera.aberration_pm.w;
    let moved = normalize(position + proper_motion * years);
    return apply_annual_aberration(moved);
}

fn refract_equatorial_direction(eq_date_dir: vec3<f32>) -> vec3<f32> {
    let local = (camera.eq_to_local * vec4<f32>(eq_date_dir, 0.0)).xyz;
    let true_alt = asin(clamp(local.z, -1.0, 1.0));
    let apparent_alt = refracted_altitude_rad(true_alt);
    let az = atan2(local.x, local.y);
    let cos_alt = cos(apparent_alt);
    return vec3<f32>(sin(az) * cos_alt, cos(az) * cos_alt, sin(apparent_alt));
}

struct VertexInput {
    @builtin(instance_index) instance_idx: u32,
    @location(0) quad_pos: vec2<f32>,    // per-vertex quad corner
    @location(1) star_pos: vec3<f32>,    // per-instance world position
    @location(2) star_size: f32,         // per-instance pixel half-width of the sprite quad
    @location(3) star_color: vec3<f32>,  // per-instance RGB color
    @location(4) star_brightness: f32,   // per-instance peak intensity multiplier
    @location(5) proper_motion: vec3<f32>, // per-instance Cartesian radians/year tangent
    @location(6) distance_pc: f32,         // per-instance heliocentric distance in parsecs
};

// =============================================================================
// V-24 atmospheric scintillation.
// =============================================================================
//
// Per-star band-limited noise field. Three independent samplings (sampled at
// slightly offset times for the RGB phase shift of Dravins 1998 §3) modulate
// the post-extinction linear flux by `(1 + σ · n)`. The noise itself is a
// 1-D smoothed-hash field driven by `time_seconds_mod_day` from the host so
// that two renders of the same session at the same simulated UT1 produce
// identical pixels (the seed travels in the session schema).

fn pcg_hash(s: u32) -> u32 {
    var state = s * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn unit_signed(s: u32) -> f32 {
    // Map a 32-bit hash uniformly into [-1, 1).
    return f32(s) * (2.0 / 4294967296.0) - 1.0;
}

// Smoothed-hash noise: piecewise-cubic interpolation between per-bin uniform
// samples. Bin width is `1 / freq_hz`, so the returned signal is band-limited
// to roughly that corner.
fn scint_noise(star_id: u32, seed: u32, t_seconds: f32, freq_hz: f32) -> f32 {
    let t = t_seconds * max(freq_hz, 0.1);
    let bin = floor(t);
    let frac = t - bin;
    let s = frac * frac * (3.0 - 2.0 * frac);
    let bin_u = bitcast<u32>(bin);
    let key0 = pcg_hash(star_id ^ seed ^ bin_u);
    let key1 = pcg_hash(star_id ^ seed ^ (bin_u + 1u));
    let a = unit_signed(key0);
    let b = unit_signed(key1);
    return mix(a, b, s);
}

fn scintillation_modulation(star_id: u32, altitude_rad: f32) -> vec3<f32> {
    let sigma_sq_zenith = camera.scintillation_params.x;
    if sigma_sq_zenith <= 0.0 || altitude_rad <= 0.0 {
        return vec3<f32>(1.0);
    }
    let corner_hz = camera.scintillation_params.y;
    let seed = bitcast<u32>(camera.scintillation_params.z);
    let t_seconds = camera.scintillation_params.w;

    // Airmass scaling from σ² ∝ sec(z)^3 (Young 1967). The Kasten-Young
    // airmass helper above is the renderer's canonical sec(z) curve; using
    // it here keeps the scintillation gating consistent with extinction.
    let x = airmass_kasten_young(altitude_rad);
    let sigma_sq = sigma_sq_zenith * x * x * x;
    // Clamp to a sane range — the weak-turbulence model itself breaks down
    // for σ ≳ 1 anyway, so saturating here is more defensible than
    // letting the multiplier go negative on a very low-altitude star.
    let sigma = sqrt(min(sigma_sq, 0.81));

    // Per-channel time offset gives the Dravins 1998 colour scintillation:
    // the RGB samples share most of the noise field but differ by a fraction
    // of the bin width, producing a faint flicker in chromaticity on top of
    // the dominant common-mode intensity variation.
    let dt_chrom = 0.10 / max(corner_hz, 1.0);
    let n_r = scint_noise(star_id, seed, t_seconds - dt_chrom, corner_hz);
    let n_g = scint_noise(star_id, seed, t_seconds,           corner_hz);
    let n_b = scint_noise(star_id, seed, t_seconds + dt_chrom, corner_hz);
    // Clamp the multiplier so it never goes negative even when σ → 0.9.
    return max(vec3<f32>(0.0), vec3<f32>(1.0) + sigma * vec3<f32>(n_r, n_g, n_b));
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) brightness: f32,
    @location(3) sprite_half_px: f32,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let k_rgb = camera.extinction_k_rgb.xyz;
    let atmosphere_active = (camera.viewpoint_params.x < 0.5) && (k_rgb.x + k_rgb.y + k_rgb.z > 0.0);
    let corrected_j2000 = corrected_j2000_direction(input.star_pos, input.proper_motion);

    // V-51d: lunar occultation of catalog stars. Iterate the active
    // analytic-mask occluder list for any entry targeting stars (the
    // Moon's apparent disk is the only producer today). If this star's
    // direction lies inside one of those front disks, collapse the
    // sprite to a degenerate point at the camera plane so the
    // fragment stage never lights it. Off-occultation frames stay
    // bit-identical because the producer emits one inert front disk
    // and the `cos(angle) > cos(radius)` test trivially fails for any
    // star outside the lunar disc.
    //
    // The producer side runs in the Earth-centred path only (the
    // external galactic viewpoint skips `active_occluders`), so this
    // gate is correct for both viewpoints: external renders see an
    // all-zero occluder list and short-circuit on `count == 0`.
    var occluded: bool = false;
    let occluder_count = u32(max(camera.occluder_params.x, 0.0));
    if occluder_count > 0u && camera.viewpoint_params.x < 0.5 {
        for (var i: u32 = 0u; i < MAX_STAR_OCCLUDERS; i = i + 1u) {
            if i >= occluder_count {
                break;
            }
            let dr = camera.occluders[i * 2u];
            let tk = camera.occluders[i * 2u + 1u];
            if i32(tk.x) != OCCLUDER_TARGET_STARS {
                continue;
            }
            let front_dir = normalize(dr.xyz);
            let cos_sep = clamp(dot(corrected_j2000, front_dir), -1.0, 1.0);
            // `cos(sep) > cos(radius)` <=> sep < radius. Use the closed
            // form (no acos) so the check is one dot product plus one
            // comparison per active occluder.
            if cos_sep > cos(max(dr.w, 0.0)) {
                occluded = true;
                break;
            }
        }
    }
    var clip: vec4<f32>;
    if camera.viewpoint_params.x > 0.5 {
        let distance_pc = max(input.distance_pc, 0.0);
        let galactic_position_pc = (EQ_TO_GAL * corrected_j2000) * distance_pc;
        clip = camera.view_proj * vec4<f32>(galactic_position_pc, 1.0);
    } else {
        let corrected_date = (camera.j2000_to_date * vec4<f32>(corrected_j2000, 0.0)).xyz;
        let local_or_refracted = refract_equatorial_direction(corrected_date);
        clip = project_equatorial_direction(corrected_j2000, local_or_refracted, atmosphere_active);
    }

    let pixel_offset = input.quad_pos * input.star_size;
    let ndc_offset = pixel_offset / viewport_size() * 2.0;

    out.clip_position = vec4<f32>(
        clip.x + ndc_offset.x * clip.w,
        clip.y + ndc_offset.y * clip.w,
        clip.z,
        clip.w,
    );

    // Atmospheric extinction: per-channel attenuation based on this star's
    // altitude in the observer's local horizontal frame. Skipped when the
    // host disables atmosphere (k_rgb = 0) so the multiplication is a
    // no-op identity rather than a hidden "horizon = invisible" rule.
    var attenuated_color = input.star_color;
    if atmosphere_active {
        let sin_alt = clamp(dot(corrected_j2000, camera.zenith_eq.xyz), -1.0, 1.0);
        let alt_rad = asin(sin_alt);
        attenuated_color = input.star_color * atmospheric_attenuation(alt_rad, k_rgb);
        // V-24: modulate the post-extinction colour by a per-star
        // band-limited noise. The Earth-frame altitude derived above is
        // exactly the airmass input scintillation needs, so we reuse it.
        attenuated_color = attenuated_color
            * scintillation_modulation(input.instance_idx, alt_rad);
    }

    if occluded {
        // V-51d: pack the sprite into a degenerate quad behind the
        // camera so the rasterizer culls it. Setting the clip vector
        // to `(0, 0, -w, -w)` puts every vertex of the quad outside
        // the `[-w, w]` clip cube; the GPU drops the primitive before
        // the fragment stage runs, matching the V-51 "no measurable
        // fps regression" contract (per-star cost is one dot + one
        // branch).
        out.clip_position = vec4<f32>(0.0, 0.0, -1.0, -1.0);
        out.uv = vec2<f32>(0.0, 0.0);
        out.color = vec3<f32>(0.0);
        out.brightness = 0.0;
        out.sprite_half_px = 0.0;
        return out;
    }

    out.uv = input.quad_pos;
    out.color = attenuated_color;
    // Do not hard-gate catalog stars by solar altitude. The sunlit sky model
    // and adaptive tonemap already determine whether a star has enough
    // contrast to be visible; forcing brightness to zero made all stars vanish
    // whenever the atmospheric pass was active.
    out.brightness = input.star_brightness;
    out.sprite_half_px = input.star_size;
    return out;
}

// =============================================================================
// Spencer 1995 human-eye point-spread function (radial part).
// =============================================================================
//
// Spencer, Shirley, Zimmerman & Greenberg 1995, "Physically-based glare
// effects for digital images", SIGGRAPH '95, Eq. (1)-(4).
//
// The eye's flux PSF is the sum of three radial components plus an
// azimuthal ciliary corona. Spencer give the radial components as
// functions of θ in degrees:
//
//     f0(θ) =          exp(-(θ / 0.02)^2)         -- sharp Gaussian core
//     f1(θ) = 20.91 / (θ + 0.02)^3                -- lenticular halo
//     f2(θ) = 72.37 / (θ + 0.02)^2                -- corneal halo
//
// with photopic mixing weights (Spencer Table 1):
//
//     P_phot(θ) = 0.282·f0(θ) + 0.478·f1(θ) + 0.207·f2(θ)
//
// Two academic caveats applied here:
//
// 1. **Pixel units, not degrees.** The renderer does not yet thread
//    degrees-per-pixel through the camera uniform (that lands with the
//    Schaefer 1993 atmospheric-extinction pass; see ROADMAP). Substituting
//    θ_px for θ_deg preserves the *shape* of each component (Gaussian
//    core, 1/r³ lenticular halo, 1/r² corneal halo); only the absolute
//    angular scale is approximate.
//
// 2. **Peak-normalised components.** Spencer's weights (0.282, 0.478,
//    0.207) are the relative *integrated* contributions — each f_i is
//    already normalised to unit volume (∫f_i · 2πθ dθ = 1). Using them as
//    plain multipliers of un-normalised f_i would let the divergent
//    halo cores dominate the bright star centre. We instead rescale
//    each component to peak 1 and apply *peak-amplitude* weights that
//    give a recognisable star (sharp core, gentle halo) under tonemap.
//    The PSF shape (relative falloff exponents) is preserved; only the
//    central-amplitude mixing differs.

const PSF_CORE_SIGMA_PX: f32 = 0.6;
const PSF_SOFT_OFFSET_PX: f32 = 1.0;

// Peak-amplitude weights. The core dominates the centre, the halos add a
// visible glow only on stars bright enough for the (small) halo amplitude
// to survive the Reinhard tonemap — i.e. exactly the brightest few stars,
// which matches what a real eye sees.
//
// The 0.15 / 0.05 split between the two halo components is empirical: the
// 1/r³ lenticular halo is the bright inner glow (visible by the 2nd or
// 3rd magnitude); the 1/r² corneal halo is the very broad outer glow,
// visible only on stars below 0 magnitude. These ratios preserve
// Spencer's qualitative result that the lenticular component contributes
// more visible light than the corneal one for typical bright sources.
const PSF_W_CORE: f32 = 1.0;
const PSF_W_LENTICULAR: f32 = 0.15;
const PSF_W_CORNEAL: f32 = 0.05;

// =============================================================================
// Ciliary corona (the cross-shaped 'spikes' on bright stars).
// =============================================================================
//
// Spencer 1995 Eq. (5) models the ciliary corona as a sum of azimuthally
// localised exponentials caused by eyelash / eyelid diffraction. Real human
// eyes preferentially produce a roughly +-shaped streak pattern because
// the upper and lower lashes have approximately orthogonal mean
// orientations. We approximate with a 4-fold modulation
// `abs(cos(2φ))^N` (peaks at φ = 0, π/2, π, 3π/2 — the four cardinal
// directions) with a long exponential tail. The N controls how thin the
// rays are; 60 gives rays that are roughly half a degree wide at
// half-max, similar to what Spencer report.
//
// The corona is *intensity-gated*: only stars whose `brightness` factor
// pushes the (small) `CORONA_AMPLITUDE` past the visible-tonemap
// threshold will show spikes, so faint stars stay as clean points and
// bright stars sprout the characteristic cross.

const CORONA_RAYS_EXPONENT: f32 = 60.0;
const CORONA_FALLOFF_PER_PX: f32 = 0.18;
const CORONA_AMPLITUDE: f32 = 0.012;

fn radial_psf(r_px: f32) -> f32 {
    // Each component normalised to peak 1 at r = 0, then mixed by
    // peak-amplitude weights (see header).
    let core = exp(-(r_px / PSF_CORE_SIGMA_PX) * (r_px / PSF_CORE_SIGMA_PX));
    let s = (r_px + PSF_SOFT_OFFSET_PX) / PSF_SOFT_OFFSET_PX;  // = 1 at r=0, growing
    let lenticular = 1.0 / (s * s * s);
    let corneal = 1.0 / (s * s);
    return PSF_W_CORE * core
        + PSF_W_LENTICULAR * lenticular
        + PSF_W_CORNEAL * corneal;
}

fn corona(uv: vec2<f32>, r_px: f32) -> f32 {
    // `uv` is the quad-space coordinate in [-1, 1]; we only need its angle.
    let azim = atan2(uv.y, uv.x);
    // 4-fold modulation: bright at the four cardinal directions, dim
    // between. `abs(cos(2φ))^N` reaches 1 at φ ∈ {0, π/2, π, 3π/2}.
    let rays = pow(abs(cos(2.0 * azim)), CORONA_RAYS_EXPONENT);
    let falloff = exp(-r_px * CORONA_FALLOFF_PER_PX);
    return CORONA_AMPLITUDE * rays * falloff;
}

// Apodization: the Spencer PSF's corneal halo decays as 1/(θ+0.02)², which is
// far too slow to fall to numerical zero at the sprite-quad edge. Left alone
// the corner-vs-edge difference in PSF value produces a *visible square* on
// bright stars (the quad outline becomes a faint box). Multiplying the PSF
// by a smooth circular window that is 1 inside the inscribed disc and
// smoothly tapers to 0 by the quad corner removes the box artefact while
// keeping the physically motivated PSF shape inside the disc. The
// truncation has no academic cost because the literature integrates the
// Spencer PSF over the visual field too — we are just imposing the same
// kind of compact-support window that any finite-aperture rendering must.
const APODIZATION_FADE_START: f32 = 0.85;
const APODIZATION_FADE_END: f32 = 1.0;

fn apodize(r_norm: f32) -> f32 {
    // Smooth Hermite interpolation from 1 -> 0 over [fade_start, fade_end].
    let t = clamp(
        (r_norm - APODIZATION_FADE_START) / (APODIZATION_FADE_END - APODIZATION_FADE_START),
        0.0,
        1.0,
    );
    let s = t * t * (3.0 - 2.0 * t);
    return 1.0 - s;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Quad-normalised radius -> pixel radius from the star centre.
    let r_norm = length(input.uv);
    let r_px = r_norm * input.sprite_half_px;

    // Spencer PSF: radial body + azimuthal ciliary corona, tapered at the
    // sprite-quad edge by a smooth apodization window (see above).
    let psf = (radial_psf(r_px) + corona(input.uv, r_px)) * apodize(r_norm);

    let intensity = psf * input.brightness;

    // No hard discard — we are writing into an Rgba16Float HDR target so
    // faint PSF tails accumulate instead of being clipped. The tonemap pass
    // maps the full HDR scene to the display.
    return vec4<f32>(input.color * intensity, intensity);
}
