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
