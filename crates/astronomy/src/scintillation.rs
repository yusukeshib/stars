//! Naked-eye atmospheric scintillation (V-24).
//!
//! Weak-turbulence intensity-variance model for a small (≤ 7 mm) human pupil
//! looking at a point source through the Earth's atmosphere. The renderer
//! consumes the σ² returned here to modulate per-star flux with a band-limited
//! noise field driven by [`temporal_corner_hz`].
//!
//! References
//! ----------
//! * Young, A. T. 1967, AJ 72, 747 — "Photometric error analysis. VIII.
//!   Scintillation of stars". Sea-level σ² scaling with airmass, aperture,
//!   and integrated Cn² column.
//! * Dravins, D., Lindegren, L., Mezey, E., Young, A. T. 1997, PASP 109, 173
//!   (Part I, statistical distributions and temporal properties).
//! * Dravins, D. et al. 1997, PASP 109, 725 (Part II).
//! * Dravins, D. et al. 1998, PASP 110, 610 (Part III — colour scintillation).
//! * Roddier, F. 1981, *Progress in Optics* 19, 281 (Cn² integral form).
//!
//! Model
//! -----
//! The renormalised intensity variance for a circular pupil in the
//! weak-turbulence limit (Young 1967 Eq. 10, recast for an unaided eye in
//! Dravins 1997 Part I §2) is
//!
//! ```text
//!     σ_I² ≈ 10.66 · sec(z)^3 · D_pupil^(-7/3) · exp(−2 h_obs / H_turb) · Σ_C
//! ```
//!
//! where `Σ_C` is the line-of-sight-integrated Cn² profile collapsed to one
//! calibrated column scalar. We chose `H_turb = 4000 m`: most of the
//! scintillation-relevant Cn² in standard Hufnagel-Valley atmospheres sits in
//! the surface boundary layer + ~3 km tropospheric layer, so the *effective*
//! scintillation scale height is several times smaller than the pressure
//! scale height (8 km) that V-37 / V-38 use for stellar reddening. Tying
//! both to 8 km would underpredict the observed amateur-site drop between
//! sea-level and ~4 km observatories (Dravins 1997 Part II Fig. 6).
//!
//! `c_n2_scale` is the user-facing knob: `1.0` reproduces the Dravins 1997
//! amateur-site σ ≈ 4 % at the zenith with a 7 mm pupil at sea level;
//! values < 1 produce a calmer sky, > 1 a more turbulent one. The bare
//! `Σ_C` constant in the formula is hidden inside [`CALIBRATION`] so the
//! engine API is in dimensionally meaningful, easy-to-remember numbers.
//!
//! Temporal spectrum (Dravins 1997 Part I §4) is a low-pass with corner
//! frequency that scales as `1 / √sec(z)` because the dominant Fresnel scale
//! `√(λ · h_turb · sec z)` grows with airmass while the wind crossing time
//! scales linearly with that scale. We anchor the zenith corner at 25 Hz
//! (the centre of the 10–30 Hz "naked-eye twinkle" band reported by
//! Dravins) and let the airmass scaling handle the rest.

const K_YOUNG: f64 = 10.66;
/// Effective scale height (m) of the scintillation-relevant Cn² column for
/// the surface-layer-dominated boundary turbulence that drives naked-eye
/// twinkling. See module docs for why this is shorter than the pressure
/// scale height used by V-37 / V-38.
pub const SCINTILLATION_SCALE_HEIGHT_M: f64 = 4000.0;
/// Default human dark-adapted pupil diameter (mm). The PSF pipeline already
/// assumes the same observer model (V-17 Spencer); reusing it here keeps the
/// two physical effects on the same eye geometry.
pub const DEFAULT_PUPIL_MM: f64 = 7.0;
/// Zenith temporal corner frequency at sea level (Hz). Centre of the
/// 10–30 Hz naked-eye band measured by Dravins 1997 Part I.
pub const CORNER_HZ_ZENITH: f64 = 25.0;
/// Default `c_n2_scale` — produces σ ≈ 4 % at the zenith for an unaided
/// 7 mm pupil at sea level.
pub const DEFAULT_CN2_SCALE: f64 = 1.0;
/// Documented zenith σ that [`DEFAULT_CN2_SCALE`] is calibrated against.
/// Dravins 1997 Part I Table 1 amateur-site median.
pub const STANDARD_AMATEUR_SIGMA_ZENITH: f64 = 0.04;

/// Calibration constant chosen so that `intensity_variance(π/2, 0, 7, 1)`
/// returns σ² = [`STANDARD_AMATEUR_SIGMA_ZENITH`]². Solved offline from the
/// closed form `Σ_C = σ² / (K_YOUNG · D^(-7/3))` and pinned by the
/// `default_scale_matches_amateur_site_sigma` test below so a future
/// constant tweak fails loudly.
const CALIBRATION: f64 = {
    // Equivalent to σ²_target / (K_YOUNG · (DEFAULT_PUPIL_MM * 1e-3)^(-7/3))
    // ≈ 0.0016 / (10.66 · 106745.96) ≈ 1.406e-9, but written as a literal so
    // it survives a future refactor of the helper math.
    1.406_059_4e-9
};

/// `sin(altitude)` floor so the sec(z) factor stays finite when the renderer
/// projects a star that has been refraction-lifted right to the geometric
/// horizon. Equivalent to airmass ≈ 50, which is far past the regime where
/// the weak-turbulence model itself remains valid; the floor is there to
/// keep the σ² value finite, not to claim accuracy.
const MIN_SIN_ALT: f64 = 0.02;

/// Intensity variance (dimensionless) and temporal corner frequency (Hz) for
/// a point source seen by a small pupil through the atmosphere.
///
/// `altitude_rad` is the *apparent* altitude of the star (i.e. after
/// refraction); `h_obs_m` is observer elevation above sea level; `pupil_mm`
/// is the entrance pupil diameter (use [`DEFAULT_PUPIL_MM`] for the naked
/// eye); `c_n2_scale` is the dimensionless Cn² column scale described in the
/// module docs ([`DEFAULT_CN2_SCALE`] = 1.0 reproduces an amateur dark site).
///
/// Returns `(sigma_sq, corner_hz)`. The corner is meaningful only when
/// `sigma_sq > 0`; callers should treat `(0, _)` as "scintillation disabled
/// for this sample" without dividing by `corner_hz`.
pub fn intensity_variance(
    altitude_rad: f64,
    h_obs_m: f64,
    pupil_mm: f64,
    c_n2_scale: f64,
) -> (f64, f64) {
    if !altitude_rad.is_finite() || !h_obs_m.is_finite() || !pupil_mm.is_finite() {
        return (0.0, CORNER_HZ_ZENITH);
    }
    let scale = c_n2_scale.max(0.0);
    if scale == 0.0 {
        return (0.0, CORNER_HZ_ZENITH);
    }
    let pupil_m = pupil_mm.max(0.1) * 1e-3;
    let sin_alt = altitude_rad.sin().max(MIN_SIN_ALT);
    let sec_z = 1.0 / sin_alt;
    let altitude_factor = (-2.0 * h_obs_m.max(0.0) / SCINTILLATION_SCALE_HEIGHT_M).exp();
    let sigma_sq =
        K_YOUNG * sec_z.powi(3) * altitude_factor * pupil_m.powf(-7.0 / 3.0) * CALIBRATION * scale;
    let f_corner = temporal_corner_hz(altitude_rad);
    (sigma_sq, f_corner)
}

/// Low-pass corner frequency of the scintillation temporal spectrum at
/// `altitude_rad`. See module docs.
pub fn temporal_corner_hz(altitude_rad: f64) -> f64 {
    if !altitude_rad.is_finite() {
        return CORNER_HZ_ZENITH;
    }
    let sin_alt = altitude_rad.sin().max(MIN_SIN_ALT);
    let sec_z = 1.0 / sin_alt;
    CORNER_HZ_ZENITH / sec_z.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CALIBRATION is pinned so the default scale yields the Dravins
    /// amateur-site σ_zenith ≈ 4 % at 7 mm pupil, sea level. If you change
    /// the constant this test fails loudly.
    #[test]
    fn default_scale_matches_amateur_site_sigma() {
        let (sigma_sq, _) = intensity_variance(
            std::f64::consts::FRAC_PI_2,
            0.0,
            DEFAULT_PUPIL_MM,
            DEFAULT_CN2_SCALE,
        );
        let sigma = sigma_sq.sqrt();
        assert!(
            (sigma - STANDARD_AMATEUR_SIGMA_ZENITH).abs() < 5.0e-4,
            "σ_zenith = {sigma}, expected ≈ {STANDARD_AMATEUR_SIGMA_ZENITH}"
        );
    }

    /// Spec V-24 unit test: σ²(airmass≈5) > 10 × σ²(airmass=1).
    #[test]
    fn airmass_scaling_amplifies_low_altitude_scintillation() {
        let alt_zenith = std::f64::consts::FRAC_PI_2; // airmass = 1
        let alt_low = 0.2_f64.asin(); // sin = 0.2, airmass = 5
        let (sigma_sq_z, _) =
            intensity_variance(alt_zenith, 0.0, DEFAULT_PUPIL_MM, DEFAULT_CN2_SCALE);
        let (sigma_sq_l, _) = intensity_variance(alt_low, 0.0, DEFAULT_PUPIL_MM, DEFAULT_CN2_SCALE);
        assert!(
            sigma_sq_l > 10.0 * sigma_sq_z,
            "σ²(airmass≈5)={sigma_sq_l} should be > 10× σ²(zenith)={sigma_sq_z}"
        );
    }

    /// Spec V-24 unit test: σ²(4 km observer) < σ²(sea level) by > 5×.
    #[test]
    fn observer_altitude_damps_scintillation() {
        let alt_zenith = std::f64::consts::FRAC_PI_2;
        let (sigma_sq_sea, _) =
            intensity_variance(alt_zenith, 0.0, DEFAULT_PUPIL_MM, DEFAULT_CN2_SCALE);
        let (sigma_sq_high, _) =
            intensity_variance(alt_zenith, 4000.0, DEFAULT_PUPIL_MM, DEFAULT_CN2_SCALE);
        assert!(
            sigma_sq_sea > 5.0 * sigma_sq_high,
            "σ²(sea)={sigma_sq_sea} should be > 5× σ²(4 km)={sigma_sq_high}"
        );
    }

    #[test]
    fn larger_pupil_reduces_variance() {
        // Aperture averaging: D^(-7/3) scaling means a 200 mm telescope sees
        // far smaller σ² than the 7 mm naked eye at the same site.
        let alt = std::f64::consts::FRAC_PI_2;
        let (eye, _) = intensity_variance(alt, 0.0, 7.0, DEFAULT_CN2_SCALE);
        let (scope, _) = intensity_variance(alt, 0.0, 200.0, DEFAULT_CN2_SCALE);
        assert!(scope < eye / 100.0);
    }

    #[test]
    fn corner_frequency_falls_with_airmass() {
        let zenith = temporal_corner_hz(std::f64::consts::FRAC_PI_2);
        let low = temporal_corner_hz(0.2_f64.asin()); // sin = 0.2, airmass = 5
        assert!((zenith - CORNER_HZ_ZENITH).abs() < 1e-9);
        assert!(low < zenith);
        // sqrt(5) ≈ 2.236
        assert!((zenith / low - 5.0_f64.sqrt()).abs() < 1e-6);
    }

    #[test]
    fn disabled_scale_returns_zero_variance() {
        let (s, _) = intensity_variance(std::f64::consts::FRAC_PI_2, 0.0, DEFAULT_PUPIL_MM, 0.0);
        assert_eq!(s, 0.0);
    }

    #[test]
    fn nan_inputs_are_safe() {
        let (s, f) = intensity_variance(f64::NAN, 0.0, DEFAULT_PUPIL_MM, DEFAULT_CN2_SCALE);
        assert_eq!(s, 0.0);
        assert!(f.is_finite());
    }
}
