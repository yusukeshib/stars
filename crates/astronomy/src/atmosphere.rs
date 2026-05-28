//! Reference atmosphere radiance helpers for the Phase 2 visual pipeline.
//!
//! Runtime rendering happens in `crates/renderer/src/shaders/skyglow.wgsl`, but
//! the scalar pieces below give the Rust side pinned, documented values for the
//! daylight / twilight pipeline: the Hošek-Wilkie 2012 daytime sky-dome model
//! (V-38, see [`hosek_wilkie`]), a continuous solar-depression twilight curve
//! tied to clear-site visual sky brightness, and the unified spectral
//! extinction model (V-37) shared with the stellar reddening path.

pub mod hosek_wilkie;

const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;

/// Clear-site zenith twilight brightness in cd/m².
///
/// This is the luminance form of [`crate::skyglow::twilight_zenith_mag_per_arcsec2`]:
/// solar depression controls an Earth-shadow optical-depth proxy, continuous
/// from civil through astronomical twilight. It deliberately returns `None` for
/// daylight and for fully dark sky so callers compose it with daylight and
/// Phase 1' dark-sky radiance instead of using hard-coded colour fades.
pub fn twilight_zenith_luminance_cd_m2(solar_altitude_rad: f64) -> Option<f64> {
    let mu = crate::skyglow::twilight_zenith_mag_per_arcsec2(solar_altitude_rad)?;
    // V-band mag/arcsec² to luminance: L[cd/m²] = 10.8e4 * 10^(-0.4 μ).
    // The 10.8e4 factor is the standard visual conversion for astronomical
    // surface brightness and keeps the twilight reference in absolute units.
    Some(108_000.0 * 10.0_f64.powf(-0.4 * mu))
}

// =============================================================================
// V-27: anti-solar twilight structure (Belt of Venus + Earth shadow band)
//
// The unidirectional civil-twilight luminance curve
// `twilight_zenith_luminance_cd_m2` is symmetric about the zenith, so it does
// not capture the directional structure observed opposite the Sun during civil
// twilight: a pink "anti-twilight arch" (Belt of Venus, ~5-15° altitude) above
// a darker blue-grey "Earth shadow" band (-2° to +2° altitude). The functions
// below provide compact analytic fits to that field, returning per-channel
// (R, G, B) radiance multipliers relative to the zenith twilight reference.
// They are designed to be cheap to evaluate both in Rust (for unit tests and
// the documented model) and in WGSL (for the renderer twilight composition).
//
// References:
//   * Hulburt, E. O. 1953, JOSA 43, 113.
//   * Lee, R. L. Jr., Hernández-Andrés, J. 2003, Appl. Opt. 42, 445.
//   * Adams, C. N., Plass, G. N., Kattawar, G. W. 1974, J. Atmos. Sci. 31,
//     1662.
// =============================================================================

/// Solar depression range (in degrees) where the Belt of Venus is visible.
///
/// Lee & Hernández-Andrés 2003 §3 measure the arch peaking in radiance and
/// purity for solar depressions of roughly 1°-6° (civil twilight), fading
/// into the sky background once nautical twilight begins.
pub const BELT_OF_VENUS_DEPRESSION_RANGE_DEG: (f64, f64) = (0.0, 6.5);

/// Per-channel radiance multiplier for the Belt of Venus (anti-twilight arch).
///
/// All angles are radians. `sun_alt_rad` is negative during twilight,
/// `relative_az_rad` is the view azimuth relative to the Sun (0 = toward Sun,
/// π = anti-solar), and `view_alt_rad` is the view altitude above the horizon.
/// Returns a multiplicative `[R, G, B]` tint to apply on top of the zenith
/// twilight reference radiance, so the integrated colour shifts to a warm pink
/// only along the anti-solar arch band, and is `[1, 1, 1]` elsewhere.
///
/// The fit follows Lee & Hernández-Andrés 2003 Fig. 6/7: peak chromaticity
/// purity near anti-solar (cos Δaz = -1), peak altitude around 8-10° during
/// civil twilight, and a roughly Gaussian falloff in both altitude and
/// azimuth. The physical driver is Hulburt 1953: the anti-solar slant column
/// has Rayleigh-stripped its blue, so the remaining single-scattered
/// sunlight is red-biased.
pub fn antitwilight_arch_radiance(
    sun_alt_rad: f64,
    relative_az_rad: f64,
    view_alt_rad: f64,
) -> [f64; 3] {
    let depression_deg = -sun_alt_rad / DEG_TO_RAD;
    let (d_lo, d_hi) = BELT_OF_VENUS_DEPRESSION_RANGE_DEG;
    if depression_deg <= d_lo || depression_deg >= d_hi {
        return [1.0, 1.0, 1.0];
    }
    if view_alt_rad <= 0.0 {
        return [1.0, 1.0, 1.0];
    }

    // Anti-solar weight: 1 at relative_az = π, 0 toward the Sun. The half-power
    // half-width is ≈ 45° in Lee & Hernández-Andrés 2003 Fig. 7.
    let antisolar = 0.5 * (1.0 - relative_az_rad.cos());
    let az_weight = antisolar.powi(2);

    // Altitude profile: Gaussian peaked at ≈ 8° altitude with σ ≈ 5°.
    let alt_deg = view_alt_rad / DEG_TO_RAD;
    let alt_peak = 8.0;
    let alt_sigma = 5.0;
    let alt_weight = (-((alt_deg - alt_peak).powi(2)) / (2.0 * alt_sigma * alt_sigma)).exp();

    // Solar-depression envelope: smooth ramp up over 0-1°, peak ≈ 2-4°, fade
    // out by 6°. Approximated with a centred quadratic over (d_lo, d_hi).
    let t = (depression_deg - d_lo) / (d_hi - d_lo);
    let depression_weight = (4.0 * t * (1.0 - t)).clamp(0.0, 1.0);

    // Peak pink amplitude: the arch is ≈ 20-30 % brighter in red and ≈ 10-15 %
    // dimmer in blue relative to the local zenith twilight reference at peak
    // chromaticity (Lee & Hernández-Andrés 2003 §4).
    let amp = az_weight * alt_weight * depression_weight;
    let r = 1.0 + 0.28 * amp;
    let g = 1.0 + 0.04 * amp;
    let b = 1.0 - 0.18 * amp;
    [r, g, b]
}

/// Per-channel radiance multiplier for the Earth-shadow band below the arch.
///
/// Same coordinate conventions as [`antitwilight_arch_radiance`]. Returns a
/// multiplicative `[R, G, B]` tint applied to the zenith twilight reference;
/// outside the band the multiplier is `[1, 1, 1]`. Inside the band
/// (anti-solar half-sky, -2°…+2° altitude, civil twilight) the multiplier
/// drops below 1 and tints cool blue-grey: the line of sight intersects
/// Earth's umbra/penumbra, so the scattering source is dominated by faint
/// multiply-scattered light with a blue-grey colour cast.
pub fn earth_shadow_band_radiance(
    sun_alt_rad: f64,
    relative_az_rad: f64,
    view_alt_rad: f64,
) -> [f64; 3] {
    let depression_deg = -sun_alt_rad / DEG_TO_RAD;
    let (d_lo, d_hi) = BELT_OF_VENUS_DEPRESSION_RANGE_DEG;
    if depression_deg <= d_lo || depression_deg >= d_hi {
        return [1.0, 1.0, 1.0];
    }

    let antisolar = 0.5 * (1.0 - relative_az_rad.cos());
    let az_weight = antisolar.powi(2);

    // Band altitude profile: peaked at the horizon (0°) with σ ≈ 2°, so the
    // dimming covers -2°…+2° altitude as documented in the V-27 spec.
    let alt_deg = view_alt_rad / DEG_TO_RAD;
    let alt_sigma = 2.0;
    let alt_weight = (-(alt_deg.powi(2)) / (2.0 * alt_sigma * alt_sigma)).exp();

    let t = (depression_deg - d_lo) / (d_hi - d_lo);
    let depression_weight = (4.0 * t * (1.0 - t)).clamp(0.0, 1.0);

    // Peak shadow darkening: ≈ 35 % radiance loss with a slight cool tint, in
    // line with Lee & Hernández-Andrés 2003 §4 Earth-shadow measurements.
    let amp = az_weight * alt_weight * depression_weight;
    let r = 1.0 - 0.40 * amp;
    let g = 1.0 - 0.32 * amp;
    let b = 1.0 - 0.22 * amp;
    [r.max(0.0), g.max(0.0), b.max(0.0)]
}

/// Smooth physical-domain selector for daylight/twilight/dark-sky composition.
///
/// Returns `(daylight, twilight, dark)` weights from solar altitude. The weights
/// are labels for model validity domains, not exposure gates: radiance remains
/// additive and each term is still computed in physical units.
pub fn solar_depression_domain_weights(solar_altitude_rad: f64) -> (f64, f64, f64) {
    let h = solar_altitude_rad / DEG_TO_RAD;
    let daylight = smoothstep(-0.5, 0.5, h);
    let dark = 1.0 - smoothstep(-18.5, -17.5, h);
    let twilight = (1.0 - daylight - dark).clamp(0.0, 1.0);
    (daylight, twilight, dark)
}

// =============================================================================
// Unified spectral extinction model (V-37)
//
// One canonical (β, α, DU, h_obs) state drives both stellar atmospheric
// extinction and the daylight scattering shader, so the two systems can no
// longer disagree about how reddened a given sky should be.
//
// References:
//   * Schaefer, B. E. 1993, Vistas in Astronomy 36, 311, §3 (extinction
//     decomposed into Rayleigh + aerosol + ozone terms).
//   * Ångström, A. 1929, Geografiska Annaler 11, 156 (aerosol turbidity
//     formula k_a(λ) = β · (λ/550)^(−α)).
//   * Hayes, D. S. & Latham, D. W. 1975, ApJ 197, 593 (anchor extinction at
//     a clean sea-level site, used for the Hardie cross-check).
//   * Iqbal, M. 1983, "An Introduction to Solar Radiation", §6.5
//     (Chappuis-band ozone absorption coefficient k_O3(λ) tabulation).
// =============================================================================

/// Rayleigh + aerosol + ozone extinction at one wavelength, in mag/airmass.
///
/// The total `k(λ)` is the field a callers usually want; the components are
/// returned separately so renderers and tests can audit which physical process
/// dominates at a given wavelength.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtinctionTerms {
    pub rayleigh: f64,
    pub aerosol: f64,
    pub ozone: f64,
}

impl ExtinctionTerms {
    pub fn total(&self) -> f64 {
        self.rayleigh + self.aerosol + self.ozone
    }
}

/// Atmospheric scale height (m) used by the Rayleigh + aerosol depletion
/// with observer altitude. Standard atmosphere value, also used in the
/// renderer skyglow shader.
pub const ATMOSPHERE_SCALE_HEIGHT_M: f64 = 8000.0;

/// Rayleigh extinction at 550 nm and sea level for a standard atmosphere.
///
/// Used as the molecular component anchor in both `extinction_coefficients`
/// and the Preetham turbidity conversion. The value matches Hayes & Latham
/// 1975 (k_R(550) ≈ 0.0855 mag/airmass) used as the canonical clean-site
/// Rayleigh term.
pub const RAYLEIGH_K550_MAG_PER_AIRMASS: f64 = 0.0855;

/// Per-channel representative wavelengths used to integrate the
/// monochromatic extinction model into broadband R/G/B coefficients. These
/// match the wavelengths used by `catalog::color` for the CIE 1931
/// blackbody-to-sRGB integration so stars and atmosphere are evaluated at the
/// same anchors.
pub const RGB_REPRESENTATIVE_WAVELENGTHS_NM: [f64; 3] = [612.0, 549.0, 464.0];

/// Extinction coefficients (mag per unit airmass) at one wavelength.
///
/// `(β, α)` are the Ångström aerosol turbidity parameters at 550 nm; `DU`
/// is the vertical ozone column in Dobson units; `h_obs_m` is the observer's
/// elevation above sea level in metres. Rayleigh and aerosol both follow the
/// standard scale-height exponential; ozone resides in the stratosphere and
/// is **not** thinned with observer altitude.
pub fn extinction_coefficients(
    wavelength_nm: f64,
    h_obs_m: f64,
    beta: f64,
    alpha: f64,
    ozone_du: f64,
) -> ExtinctionTerms {
    let altitude_factor = (-h_obs_m.max(0.0) / ATMOSPHERE_SCALE_HEIGHT_M).exp();
    let lambda_um = wavelength_nm / 1000.0;
    let lambda_ratio = wavelength_nm / 550.0;

    // Rayleigh: k_R(λ) = k_R(550) · (λ/550)^(−4), thinned with altitude.
    let rayleigh = RAYLEIGH_K550_MAG_PER_AIRMASS * lambda_ratio.powf(-4.0) * altitude_factor;

    // Aerosol: Ångström k_a(λ) = β · (λ/550)^(−α), thinned with altitude.
    let aerosol = beta.max(0.0) * lambda_ratio.powf(-alpha) * altitude_factor;

    // Ozone: compact Chappuis-band absorption peaking near 600 nm.
    // k_O3(λ) in cm^-1 · atm peaks at ≈20 · 10^-3 near 600 nm and drops to
    // negligible levels by 450 nm and 750 nm. Converted to mag/airmass via
    // 1 DU = 1e-3 cm · atm and the 1.0857 ln(10)/2.5 factor for magnitudes:
    //     k_O3(λ) [mag/airmass] = 1.0857 · (DU / 1000) · sigma(λ)
    // The Gaussian-style envelope reproduces Iqbal 1983 Table 6.5.2 within a
    // few percent across 480–700 nm.
    let chappuis = (-((lambda_um - 0.60).powi(2)) / 0.020_f64).exp();
    let ozone = 1.0857 * (ozone_du.max(0.0) / 1000.0) * 0.060 * chappuis;

    ExtinctionTerms {
        rayleigh,
        aerosol,
        ozone,
    }
}

/// Broadband R/G/B extinction coefficients for the renderer star pass.
///
/// Evaluates [`extinction_coefficients`] at
/// [`RGB_REPRESENTATIVE_WAVELENGTHS_NM`] and returns the totals in mag per
/// unit airmass. This is the function the renderer calls to build the GPU
/// uniform from the canonical `(β, α, DU, h)` state.
pub fn extinction_k_rgb(h_obs_m: f64, beta: f64, alpha: f64, ozone_du: f64) -> [f64; 3] {
    let [lr, lg, lb] = RGB_REPRESENTATIVE_WAVELENGTHS_NM;
    [
        extinction_coefficients(lr, h_obs_m, beta, alpha, ozone_du).total(),
        extinction_coefficients(lg, h_obs_m, beta, alpha, ozone_du).total(),
        extinction_coefficients(lb, h_obs_m, beta, alpha, ozone_du).total(),
    ]
}

/// Effective Linke turbidity for the given Ångström aerosol depth.
///
/// Linke turbidity is defined as
///   T = (τ_aerosol + τ_molecular) / τ_molecular
/// evaluated at 550 nm, so the (β, α) state determines T directly via
/// `T = 1 + β / RAYLEIGH_K550`. The result is clamped to the practical
/// daylight window [1.7, 10] that both the Hošek-Wilkie 2012 sky-dome
/// dataset and the legacy Preetham 1999 evaluator were originally fit
/// against.
///
/// V-37 ties the daylight scattering term and the stellar k(λ) reddening
/// to a single (β, α, DU) state; V-38 then uses this Linke turbidity
/// directly as the HW input.
pub fn linke_turbidity_from_aerosol(beta: f64) -> f64 {
    let t = 1.0 + beta.max(0.0) / RAYLEIGH_K550_MAG_PER_AIRMASS;
    t.clamp(1.7, 10.0)
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hardie 1962 sea-level standard V-band extinction is ≈ 0.20 mag/airmass
    /// for a mid-quality observatory site (Mauna Kea-class is ≈ 0.13, urban
    /// is ≥ 0.35). With β=0.10 (Hardie-anchored aerosol depth at 550 nm),
    /// α=1.3, DU=300 the unified model must reproduce 0.20 within 0.03
    /// mag/airmass.
    #[test]
    fn unified_extinction_reproduces_hardie_v_band() {
        let terms = extinction_coefficients(549.0, 0.0, 0.10, 1.3, 300.0);
        let k_v = terms.total();
        assert!(
            (k_v - 0.20).abs() < 0.03,
            "k_V at Hardie mid-quality site = {k_v} mag/airmass, expected within 0.03 of 0.20"
        );
        // Decomposition sanity: each component is non-negative and within order.
        assert!(terms.rayleigh > 0.08 && terms.rayleigh < 0.10);
        assert!(terms.aerosol > 0.08 && terms.aerosol < 0.12);
        assert!(terms.ozone > 0.0 && terms.ozone < 0.05);
    }

    #[test]
    fn unified_extinction_is_monotone_in_beta_alpha_ozone() {
        let base = extinction_coefficients(464.0, 0.0, 0.10, 1.3, 300.0).total();
        let more_beta = extinction_coefficients(464.0, 0.0, 0.25, 1.3, 300.0).total();
        let more_alpha = extinction_coefficients(464.0, 0.0, 0.10, 1.6, 300.0).total();
        let more_ozone = extinction_coefficients(549.0, 0.0, 0.10, 1.3, 500.0).total();
        let base_g = extinction_coefficients(549.0, 0.0, 0.10, 1.3, 300.0).total();
        assert!(more_beta > base, "β↑ must raise blue extinction");
        // α↑ steepens the spectrum, so B (464 nm < 550) increases.
        assert!(more_alpha > base, "α↑ must raise blue extinction");
        assert!(more_ozone > base_g, "DU↑ must raise green extinction");
    }

    #[test]
    fn high_altitude_thins_rayleigh_and_aerosol() {
        let sea = extinction_coefficients(549.0, 0.0, 0.10, 1.3, 300.0);
        let alt = extinction_coefficients(549.0, 2500.0, 0.10, 1.3, 300.0);
        // 2500 m → factor exp(-2500/8000) ≈ 0.733 on Rayleigh + aerosol.
        let expected_factor = (-2500_f64 / 8000.0).exp();
        assert!(
            ((alt.rayleigh / sea.rayleigh) - expected_factor).abs() < 1e-9,
            "Rayleigh must follow scale-height drop exactly"
        );
        assert!(
            ((alt.aerosol / sea.aerosol) - expected_factor).abs() < 1e-9,
            "Aerosol must follow scale-height drop exactly"
        );
        // Ozone is stratospheric and must not scale with observer altitude.
        assert_eq!(alt.ozone, sea.ozone);
    }

    #[test]
    fn extinction_k_rgb_is_redder_for_blue() {
        let k = extinction_k_rgb(0.0, 0.10, 1.3, 300.0);
        assert!(
            k[2] > k[1] && k[1] > k[0],
            "k_RGB must satisfy k_B > k_G > k_R (Rayleigh + Ångström sign), got {k:?}"
        );
    }

    #[test]
    fn linke_turbidity_clamps_to_documented_window() {
        assert_eq!(linke_turbidity_from_aerosol(0.0), 1.7);
        // β=0.12 → T = 1 + 1.403 = 2.40
        let t = linke_turbidity_from_aerosol(0.12);
        assert!((t - 2.40).abs() < 0.05);
        // β=1.0 → T well above 10, clamped.
        assert_eq!(linke_turbidity_from_aerosol(1.0), 10.0);
    }

    #[test]
    fn twilight_luminance_is_continuous_and_fades_to_dark_sky() {
        let civil = twilight_zenith_luminance_cd_m2((-6.0_f64).to_radians()).unwrap();
        let nautical = twilight_zenith_luminance_cd_m2((-12.0_f64).to_radians()).unwrap();
        let astronomical = twilight_zenith_luminance_cd_m2((-17.999_f64).to_radians()).unwrap();
        assert!(civil > nautical && nautical > astronomical);
        assert!(civil > 1.0e-1);
        assert!(astronomical < 1.0e-3);
        assert_eq!(twilight_zenith_luminance_cd_m2(1_f64.to_radians()), None);
        assert_eq!(
            twilight_zenith_luminance_cd_m2((-19_f64).to_radians()),
            None
        );
    }

    /// V-27: at sun_alt = -2°, looking anti-solar (relative_az = 180°) at
    /// view_alt = 5° the arch fit must show a positive red excess relative
    /// to the zenith reference (R/G > 1), reproducing the Belt of Venus
    /// chromaticity from Lee & Hernández-Andrés 2003 Fig. 7.
    #[test]
    fn antitwilight_arch_has_red_excess_anti_solar() {
        let sun_alt = (-2.0_f64).to_radians();
        let rel_az = std::f64::consts::PI;
        let view_alt = 5.0_f64.to_radians();
        let arch = antitwilight_arch_radiance(sun_alt, rel_az, view_alt);
        assert!(
            arch[0] > arch[1] && arch[1] > arch[2],
            "anti-solar arch must show R > G > B, got {arch:?}"
        );
        assert!(
            arch[0] > 1.05,
            "anti-solar arch must show a clear red excess, got R = {}",
            arch[0]
        );

        // Toward the Sun (relative_az = 0) the arch must vanish: the fit
        // returns the neutral [1,1,1] reference.
        let toward_sun = antitwilight_arch_radiance(sun_alt, 0.0, view_alt);
        for (channel, value) in toward_sun.iter().enumerate() {
            assert!(
                (value - 1.0).abs() < 1e-6,
                "toward-sun arch must be neutral, channel {channel} = {value}"
            );
        }
    }

    /// V-27: at sun_alt = -2°, the anti-solar Earth-shadow band at
    /// view_alt = 0° must be the dimmest point in the anti-solar half-sky.
    #[test]
    fn earth_shadow_band_is_dimmest_anti_solar() {
        let sun_alt = (-2.0_f64).to_radians();
        let rel_az = std::f64::consts::PI;
        let luminance = |alt_deg: f64| {
            let view_alt = alt_deg.to_radians();
            let m = earth_shadow_band_radiance(sun_alt, rel_az, view_alt);
            // V-band luminance proxy under D65: 0.2126 R + 0.7152 G + 0.0722 B.
            0.2126 * m[0] + 0.7152 * m[1] + 0.0722 * m[2]
        };
        let band = luminance(0.0);
        for alt_deg in [2.5_f64, 5.0, 8.0, 12.0, 20.0, 45.0, 80.0] {
            assert!(
                band < luminance(alt_deg),
                "band at horizon must be dimmer than view_alt = {alt_deg}°"
            );
        }
        // Cool-tint check: the band must drop blue less than red so the
        // residual colour is blue-grey, not warm.
        let m = earth_shadow_band_radiance(sun_alt, rel_az, 0.0);
        assert!(m[2] > m[0], "Earth shadow band must keep B above R: {m:?}");
    }

    /// V-27: outside the civil-twilight depression window both fits must
    /// short-circuit to the neutral [1,1,1] reference so the composition
    /// path falls back to the zenith twilight curve.
    #[test]
    fn antitwilight_and_shadow_neutral_outside_civil_twilight() {
        let rel_az = std::f64::consts::PI;
        let view_alt = 5.0_f64.to_radians();
        for sun_alt_deg in [1.0_f64, -10.0, -20.0] {
            let sun_alt = sun_alt_deg.to_radians();
            let arch = antitwilight_arch_radiance(sun_alt, rel_az, view_alt);
            let band = earth_shadow_band_radiance(sun_alt, rel_az, 0.0);
            for c in 0..3 {
                assert!(
                    (arch[c] - 1.0).abs() < 1e-6,
                    "arch must be neutral at sun_alt = {sun_alt_deg}°"
                );
                assert!(
                    (band[c] - 1.0).abs() < 1e-6,
                    "band must be neutral at sun_alt = {sun_alt_deg}°"
                );
            }
        }
    }

    /// Pinned ROI ratios: two reference points inside the
    /// `civil-twilight-antisolar-tokyo` scene used by the validation
    /// gallery. ROI A samples the Belt of Venus core; ROI B samples the
    /// Earth-shadow band. The combined model multiplier R/G ratio must
    /// stay within the documented Lee & Hernández-Andrés 2003 envelope.
    #[test]
    fn antitwilight_civil_twilight_roi_pixel_ratios_are_pinned() {
        let sun_alt = (-3.0_f64).to_radians();
        let rel_az = std::f64::consts::PI;

        // ROI A: 8° altitude, anti-solar — Belt of Venus core.
        let view_alt_a = 8.0_f64.to_radians();
        let arch_a = antitwilight_arch_radiance(sun_alt, rel_az, view_alt_a);
        let band_a = earth_shadow_band_radiance(sun_alt, rel_az, view_alt_a);
        let r_a = arch_a[0] * band_a[0];
        let g_a = arch_a[1] * band_a[1];
        let ratio_a = r_a / g_a;
        assert!(
            (1.15..=1.35).contains(&ratio_a),
            "belt-of-venus ROI R/G outside 1.15-1.35, got {ratio_a}"
        );

        // ROI B: 0° altitude, anti-solar — Earth shadow band core.
        let view_alt_b = 0.0;
        let arch_b = antitwilight_arch_radiance(sun_alt, rel_az, view_alt_b);
        let band_b = earth_shadow_band_radiance(sun_alt, rel_az, view_alt_b);
        let r_b = arch_b[0] * band_b[0];
        let g_b = arch_b[1] * band_b[1];
        let ratio_b = r_b / g_b;
        assert!(
            (0.85..=1.00).contains(&ratio_b),
            "earth-shadow ROI R/G outside 0.85-1.00, got {ratio_b}"
        );
        assert!(
            ratio_a > ratio_b,
            "belt-of-venus R/G must exceed earth-shadow R/G"
        );
    }

    #[test]
    fn solar_domain_weights_cover_day_twilight_and_night() {
        let (day, twi, dark) = solar_depression_domain_weights(10_f64.to_radians());
        assert!(day > 0.99 && twi < 0.01 && dark < 0.01);
        let (day, twi, dark) = solar_depression_domain_weights((-6_f64).to_radians());
        assert!(twi > 0.99 && day < 0.01 && dark < 0.01);
        let (day, twi, dark) = solar_depression_domain_weights((-20_f64).to_radians());
        assert!(dark > 0.99 && day < 0.01 && twi < 0.01);
    }
}
