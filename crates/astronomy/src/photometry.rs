//! Physically-grounded photometric helpers for star visualisation.
//!
//! Every transform in this module is anchored to a published standard or
//! peer-reviewed paper. The job is to convert *apparent V-magnitude* (the
//! quantity carried in star catalogues) into the perceptual quantities that
//! determine how a human eye, dark-adapted under a real sky, would see that
//! star — so that downstream rendering decisions (colour, brightness, PSF
//! shape) can be defended against the literature rather than tuned by eye.
//!
//! The pipeline is:
//!
//! ```text
//!   apparent magnitude m_V
//!     → illuminance at the pupil E   [lux]          (Schaefer 1990)
//!     → equivalent point-source luminance L_eq      [cd/m²]
//!           = E / Ω_PSF                              (Ferwerda et al. 1996)
//!     → mesopic chromatic-fidelity weight w ∈ [0,1] (CIE 191:2010)
//!     → perceived colour = lerp(scotopic_grey, photopic_rgb, w)
//!                                                    (Purkinje shift; CIE V'(λ))
//! ```
//!
//! Higher-level pieces of the academic visual pipeline (atmospheric
//! extinction per Schaefer 1993, diffuse sky background per Leinert et
//! al. 1998, eye PSF / glare per Spencer et al. 1995, tone reproduction per
//! Ferwerda et al. 1996 / Pattanaik et al. 1998) sit on top of these
//! primitives and are scoped in ROADMAP Phase 1'.

/// Illuminance at the pupil produced by an unobstructed star of apparent
/// V-band magnitude `m_v`, in lux (lumen per square metre).
///
/// Implements the standard photometric zeropoint:
///
/// ```text
///     E_v = 10^(-0.4 · (m_V + 13.99))   lux
/// ```
///
/// A magnitude-0 star (Vega-class) produces ≈ 2.54 × 10⁻⁶ lux outside the
/// atmosphere, which fixes the constant 13.99. This is the relation that
/// converts the catalogue's dimensionless magnitude into a physical quantity
/// the rest of the visual pipeline can reason about.
///
/// # Reference
///
/// Schaefer, B. E. 1990, *Telescopic limiting magnitudes*, PASP 102, 212–229.
/// Equation (1) and Table 1.
pub fn magnitude_to_illuminance_lux(m_v: f64) -> f64 {
    10.0_f64.powf(-0.4 * (m_v + 13.99))
}

/// Solid angle of the dark-adapted human eye's point-spread function, in
/// steradians.
///
/// Conventionally taken as 1 arcmin² = (π / 10800)² sr ≈ 8.46 × 10⁻⁸ sr.
/// This is the angular area over which a true point source's energy is
/// smeared on the retina by the eye's optics — diffraction at the pupil,
/// residual aberration, the Stiles–Crawford effect.
///
/// We need it to convert *illuminance* (a flux measured at a point) into an
/// *equivalent luminance* (a flux per solid angle, which is the quantity the
/// CIE adaptation models below expect).
///
/// # References
///
/// * Liang, J., & Williams, D. R. 1997, *Aberrations and retinal image
///   quality of the normal human eye*, JOSA A 14, 2873.
/// * Spencer, G., Shirley, P., Zimmerman, K., & Greenberg, D. P. 1995,
///   *Physically-based glare effects for digital images*, SIGGRAPH '95.
pub const EYE_PSF_SOLID_ANGLE_SR: f64 = 8.461_594_994_075_e-8;

/// Equivalent point-source luminance seen by the retina, in cd/m².
///
/// A true point source has no surface brightness; what the retina actually
/// integrates is the irradiance spread across the eye's PSF area. The
/// equivalent luminance is `E / Ω_PSF`, which is the quantity the CIE
/// adaptation models below expect.
///
/// This is the standard manoeuvre used to feed point-source stimuli into
/// luminance-based visual-adaptation pipelines (e.g. Ferwerda et al. 1996,
/// Pattanaik et al. 1998, Spencer et al. 1995 for the related glare
/// model) — those papers do not state the conversion in this one-line
/// form, but they all assume an adapting luminance computed from
/// irradiance over the PSF.
///
/// # References
///
/// * Ferwerda, J. A., Pattanaik, S. N., Shirley, P., & Greenberg, D. P.
///   1996, *A Model of Visual Adaptation for Realistic Image Synthesis*,
///   SIGGRAPH '96.
/// * Spencer, G., Shirley, P., Zimmerman, K., & Greenberg, D. P. 1995,
///   *Physically-based glare effects for digital images*, SIGGRAPH '95.
pub fn illuminance_to_point_source_luminance(e_lux: f64, psf_solid_angle_sr: f64) -> f64 {
    e_lux / psf_solid_angle_sr
}

/// CIE 191:2010 mesopic transition range — lower (pure-scotopic) bound, cd/m².
pub const MESOPIC_LOWER_CD_M2: f64 = 0.005;
/// CIE 191:2010 mesopic transition range — upper (pure-photopic) bound, cd/m².
pub const MESOPIC_UPPER_CD_M2: f64 = 5.0;

/// Chromatic-fidelity weight `w ∈ [0, 1]` for a stimulus at the given
/// adapting luminance, under the CIE 191:2010 mesopic photometry framework.
///
/// * `w = 1` above 5 cd/m² — pure photopic, cone-dominated, full colour.
/// * `w = 0` below 0.005 cd/m² — pure scotopic, rod-only, achromatic.
/// * In between: a log-linear blend on adapting luminance.
///
/// Stars seen against a dark sky map onto this curve via their per-star
/// equivalent luminance (see [`illuminance_to_point_source_luminance`]):
/// only the brightest stars land in the photopic regime where cone colour
/// vision is intact; the rest desaturate progressively, matching the
/// well-known observation that *only bright stars look coloured to a
/// dark-adapted observer*.
///
/// # Reference
///
/// CIE 191:2010, *Recommended System for Mesopic Photometry Based on Visual
/// Performance*, §3 (MES2 model, transition range 0.005 – 5 cd/m²).
///
/// # Approximation
///
/// The standard's `m` parameter is defined implicitly via an iterative
/// scheme that mixes photopic *and* scotopic luminance. This helper takes a
/// single adapting luminance and uses a log-linear blend between the two
/// endpoints. It matches the standard at the endpoints and is monotonic in
/// between; adequate for visualisation, **not** a substitute for the MES2
/// iteration in metrology contexts.
pub fn mesopic_chromatic_weight(adapting_luminance_cd_m2: f64) -> f64 {
    if adapting_luminance_cd_m2 <= MESOPIC_LOWER_CD_M2 {
        0.0
    } else if adapting_luminance_cd_m2 >= MESOPIC_UPPER_CD_M2 {
        1.0
    } else {
        (adapting_luminance_cd_m2.log10() - MESOPIC_LOWER_CD_M2.log10())
            / (MESOPIC_UPPER_CD_M2.log10() - MESOPIC_LOWER_CD_M2.log10())
    }
}

/// Approximate scotopic grey level of an sRGB triple — the achromatic
/// signal a rod-only visual system would produce from this light.
///
/// Without per-star spectra we cannot integrate against the CIE 1951
/// V'(λ) scotopic luminous efficiency directly, so this approximation
/// substitutes channel weights that reproduce two qualitative features of
/// V'(λ): the peak shifts to ~507 nm (Purkinje shift), and rods are
/// essentially insensitive to long-wavelength (red) light.
///
/// The chosen weights `(0.00, 0.40, 0.60)` sum to 1 (so a neutral white
/// point stays neutral under the blend) and put rod sensitivity strongly
/// on the blue / green channels, matching V'(λ) qualitatively.
///
/// # References
///
/// * CIE 1951, *V'(λ) scotopic luminous efficiency function*.
/// * Bowmaker, J. K. & Dartnall, H. J. A. 1980, *Visual pigments of rods
///   and cones in a human retina*, J. Physiol. 298, 501–511.
pub fn scotopic_grey(rgb: [f32; 3]) -> f32 {
    0.00 * rgb[0] + 0.40 * rgb[1] + 0.60 * rgb[2]
}

/// Blend a photopic sRGB colour toward its mesopic perceived appearance.
///
/// At `chromatic_weight = 1` the colour is unchanged (full cone response).
/// At `chromatic_weight = 0` it collapses to the [`scotopic_grey`] level
/// (rod-only, achromatic, Purkinje-shifted). Intermediate values linearly
/// interpolate.
///
/// The combination of [`mesopic_chromatic_weight`] (which depends on the
/// star's luminance, hence its magnitude) with this function gives the
/// per-star "perceived colour" used by the renderer.
pub fn apply_mesopic_desaturation(rgb: [f32; 3], chromatic_weight: f32) -> [f32; 3] {
    let w = chromatic_weight.clamp(0.0, 1.0);
    let grey = scotopic_grey(rgb);
    [
        w * rgb[0] + (1.0 - w) * grey,
        w * rgb[1] + (1.0 - w) * grey,
        w * rgb[2] + (1.0 - w) * grey,
    ]
}

// =============================================================================
// Ferwerda 1996 visual-adaptation TVI functions.
// =============================================================================
//
// Ferwerda, J. A., Pattanaik, S. N., Shirley, P., & Greenberg, D. P. 1996,
// *A Model of Visual Adaptation for Realistic Image Synthesis*, SIGGRAPH '96.
//
// The TVI (threshold-versus-intensity) function gives the minimum
// detectable luminance increment a human observer can perceive against a
// uniform background at adaptation luminance `L_a`. Cones (photopic) and
// rods (scotopic) have different TVI curves; both are piecewise functions
// of `log10(L_a)` derived from the classical Blackwell 1946 / Hecht 1934
// psychophysics data.
//
// In a tone-reproduction pipeline the TVI ratio `T(L_display) / T(L_scene)`
// is the *scale factor* that must be applied to scene luminance so a
// just-detectable difference in the scene remains just-detectable on the
// display. That is what lets a dark-adapted night-sky scene render with
// the Milky Way visible against the sky background, rather than being
// crushed to black by a naive luminance-preserving operator.

/// Cone (photopic) TVI — Ferwerda 1996 Eq. (1).
///
/// Input is `log10(adapting luminance / (cd/m²))`; output is
/// `log10(threshold luminance / (cd/m²))`.
///
/// Piecewise:
/// * `log L_a ≤ -2.6`  →  `log T_p = -0.72` (dark-detection plateau)
/// * `log L_a ≥ 1.9`   →  `log T_p = log L_a - 1.255` (Weber's-law regime)
/// * in between: smooth transition, Ferwerda Eq. (1).
pub fn cone_tvi_log10(log_la: f64) -> f64 {
    if log_la <= -2.6 {
        -0.72
    } else if log_la >= 1.9 {
        log_la - 1.255
    } else {
        let inner = 0.249 * log_la + 0.65;
        inner.powf(2.7) - 0.72
    }
}

/// Rod (scotopic) TVI — Ferwerda 1996 Eq. (2).
///
/// Input is `log10(adapting luminance / (cd/m²))`; output is
/// `log10(threshold luminance / (cd/m²))`.
///
/// Piecewise:
/// * `log L_a ≤ -3.94` →  `log T_s = -2.86` (absolute-threshold plateau)
/// * `log L_a ≥ -1.44` →  `log T_s = log L_a - 0.395` (rod saturation
///   upper end, beyond which rods bleach)
/// * in between: smooth transition, Ferwerda Eq. (2).
pub fn rod_tvi_log10(log_la: f64) -> f64 {
    if log_la <= -3.94 {
        -2.86
    } else if log_la >= -1.44 {
        log_la - 0.395
    } else {
        let inner = 0.405 * log_la + 1.6;
        inner.powf(2.18) - 2.86
    }
}

/// Convert a linear-flux value on the renderer's magnitude-zeropoint scale
/// to an absolute luminance in cd/m².
///
/// The renderer's brightness scale is anchored so that 1.0 corresponds to
/// a point source of apparent magnitude `zeropoint`. Combining the
/// Schaefer 1990 zeropoint (illuminance per magnitude) with the canonical
/// eye-PSF solid angle gives the luminance one such source produces
/// across the PSF:
///
/// ```text
///     L [cd/m²] = flux · 10^(-0.4 · (zeropoint + 13.99)) / Ω_PSF
/// ```
///
/// where `Ω_PSF = 8.46e-8 sr` (1 arcmin²). Required by [`cone_tvi_log10`]
/// and [`rod_tvi_log10`], which take absolute cd/m².
pub fn hdr_flux_to_luminance_cd_m2(flux: f64, zeropoint: f64) -> f64 {
    let zeropoint_illuminance_lux = 10.0_f64.powf(-0.4 * (zeropoint + 13.99));
    let zeropoint_luminance_cd_m2 = zeropoint_illuminance_lux / EYE_PSF_SOLID_ANGLE_SR;
    flux * zeropoint_luminance_cd_m2
}

// =============================================================================
// Atmospheric extinction (Schaefer 1993; Kasten & Young 1989; Hardie 1962).
// =============================================================================

/// Airmass `X` along the line of sight to a star at altitude `alt`, using the
/// Kasten & Young (1989) refraction-aware formula.
///
/// Airmass is the slant-path length of the line of sight through the
/// atmosphere expressed in units of zenith path length: `X = 1` at the
/// zenith, growing toward the horizon. The simple secant approximation
/// `X ≈ sec(z)` diverges at the horizon and overestimates by several units
/// below ≈10° altitude; the Kasten-Young formula matches measured
/// extinction down to ~0° with sub-percent error.
///
/// ```text
///     X = 1 / [ sin(alt) + 0.50572 · (alt_deg + 6.07995)^(-1.6364) ]
/// ```
///
/// Reference values:
///   * zenith (alt = 90°)  →  X = 1.0   exactly
///   * alt = 30°           →  X ≈ 1.99   (matches sec 60° = 2.0)
///   * alt = 10°           →  X ≈ 5.55   (sec 80° = 5.76, divergent)
///   * horizon (alt = 0)   →  X ≈ 37.92  (sec 90° is undefined)
///
/// # Reference
///
/// Kasten, F. & Young, A. T. 1989, *Revised optical air mass tables and
/// approximation formula*, Applied Optics 28, 4735–4738. Eq. (3) and
/// Table 1.
pub fn airmass_kasten_young(altitude_rad: f64) -> f64 {
    // Below the horizon there is no defensible slant path; callers that
    // want a continuous knob for below-horizon stars should clamp the
    // altitude on their side. We return +∞ here to make accidental
    // below-horizon evaluation loud rather than silently "finite-ish".
    if altitude_rad <= 0.0 {
        return f64::INFINITY;
    }
    let alt_deg = altitude_rad.to_degrees();
    let sin_alt = altitude_rad.sin();
    1.0 / (sin_alt + 0.50572 * (alt_deg + 6.07995).powf(-1.6364))
}

/// Default V-band-broken-into-RGB extinction coefficients for a clean
/// sea-level observatory site, in magnitudes per unit airmass.
///
/// Rayleigh scattering scales as λ⁻⁴, so the blue channel is dimmed and
/// reddened by an order of magnitude more than the red channel. The values
/// here are the standard atmospheric extinction at a mid-quality dark site
/// (cf. Hardie 1962 Table I; Schaefer 1993 §3 for the dependence on
/// wavelength, altitude above sea level, and seasonal aerosol load).
///
/// Tuple order is `[R, G, B]`. To disable extinction, pass `[0.0; 3]`.
///
/// # References
///
/// * Hardie, R. H. 1962, *Photoelectric Reductions*, in *Astronomical
///   Techniques* (ed. W. A. Hiltner), University of Chicago Press, ch. 8.
/// * Schaefer, B. E. 1993, *Astronomy and the limits of vision*, Vistas in
///   Astronomy 36, 311. §3 (atmospheric extinction breakdown).
pub const DEFAULT_EXTINCTION_K_RGB: [f64; 3] = [0.10, 0.16, 0.30];

/// Extinction magnitudes at a given airmass, per RGB channel.
///
/// Implements Schaefer 1993's atmospheric-extinction term:
///
/// ```text
///     Δm(λ) = k(λ) · X
/// ```
///
/// where `X` is airmass (see [`airmass_kasten_young`]) and `k(λ)` is the
/// site's per-wavelength extinction coefficient (mag per airmass). A star
/// at airmass `X` shines fainter than its catalogued apparent magnitude
/// by `Δm` magnitudes in each channel — the blue channel always more than
/// the red, which gives horizon stars their characteristic reddening.
///
/// # Reference
///
/// Schaefer, B. E. 1993, *Astronomy and the limits of vision*, Vistas in
/// Astronomy 36, 311, Eq. (1).
pub fn extinction_magnitudes_rgb(airmass: f64, k_rgb: [f64; 3]) -> [f64; 3] {
    [k_rgb[0] * airmass, k_rgb[1] * airmass, k_rgb[2] * airmass]
}

/// Convenience: full pipeline from magnitude to chromatic-fidelity weight.
///
/// Convolves [`magnitude_to_illuminance_lux`] →
/// [`illuminance_to_point_source_luminance`] →
/// [`mesopic_chromatic_weight`] using the canonical eye PSF solid angle.
///
/// Returns `w ∈ [0, 1]` ready to feed into [`apply_mesopic_desaturation`].
pub fn chromatic_weight_for_magnitude(m_v: f64) -> f64 {
    let e = magnitude_to_illuminance_lux(m_v);
    let l_eq = illuminance_to_point_source_luminance(e, EYE_PSF_SOLID_ANGLE_SR);
    mesopic_chromatic_weight(l_eq)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Schaefer 1990 Table 1: a 0-mag star outside the atmosphere produces
    /// 2.54 × 10⁻⁶ lux. Pin that exactly to catch any drift in the constant.
    #[test]
    fn schaefer_1990_zeropoint() {
        let e = magnitude_to_illuminance_lux(0.0);
        assert!(
            (e - 2.54e-6).abs() < 5e-9,
            "m=0 illuminance {e} lux, expected ≈ 2.54e-6 lux (Schaefer 1990)"
        );
    }

    /// Pogson scaling: a 5-mag difference is exactly a factor of 100 in
    /// illuminance. This is the same invariant the renderer relies on for
    /// `brightness`, restated in physical units.
    #[test]
    fn pogson_in_illuminance() {
        let e0 = magnitude_to_illuminance_lux(0.0);
        let e5 = magnitude_to_illuminance_lux(5.0);
        let ratio = e0 / e5;
        assert!(
            (ratio - 100.0).abs() < 1e-6,
            "5-mag flux ratio is {ratio}, expected 100"
        );
    }

    /// 1 arcmin² is the conventional eye-PSF solid angle. Sanity-check the
    /// constant against its definition (π/10800)².
    #[test]
    fn eye_psf_solid_angle_matches_one_arcmin_squared() {
        let expected = (std::f64::consts::PI / 10800.0).powi(2);
        let rel = (EYE_PSF_SOLID_ANGLE_SR - expected).abs() / expected;
        assert!(
            rel < 1e-9,
            "EYE_PSF_SOLID_ANGLE_SR = {EYE_PSF_SOLID_ANGLE_SR}, expected (π/10800)² = {expected}"
        );
    }

    /// CIE 191:2010 endpoints. Below 0.005 cd/m² is pure scotopic (w=0),
    /// above 5 cd/m² is pure photopic (w=1).
    #[test]
    fn mesopic_endpoints_match_cie_191() {
        assert_eq!(mesopic_chromatic_weight(0.0), 0.0);
        assert_eq!(mesopic_chromatic_weight(MESOPIC_LOWER_CD_M2), 0.0);
        assert_eq!(mesopic_chromatic_weight(MESOPIC_UPPER_CD_M2), 1.0);
        assert_eq!(mesopic_chromatic_weight(1e6), 1.0);
    }

    /// Halfway through the mesopic range on log L should give w ≈ 0.5.
    /// The geometric mean of 0.005 and 5 is √(0.005·5) ≈ 0.158 cd/m².
    #[test]
    fn mesopic_midpoint_is_half() {
        let l_mid = (MESOPIC_LOWER_CD_M2 * MESOPIC_UPPER_CD_M2).sqrt();
        let w = mesopic_chromatic_weight(l_mid);
        assert!(
            (w - 0.5).abs() < 1e-9,
            "midpoint w={w}, expected 0.5 at L_mid={l_mid}"
        );
    }

    /// A magnitude-0 star (Vega) should land fully in photopic — bright stars
    /// retain their colour to a dark-adapted observer.
    #[test]
    fn bright_stars_are_photopic() {
        let w = chromatic_weight_for_magnitude(0.0);
        assert!(
            w >= 1.0 - 1e-9,
            "m=0 star chromatic weight {w}, expected 1.0 (fully photopic)"
        );
    }

    /// A magnitude-6 star (naked-eye limit under a dark sky) should be
    /// strongly desaturated — the well-known "faint stars look grey"
    /// effect.
    #[test]
    fn naked_eye_limit_is_partially_scotopic() {
        let w = chromatic_weight_for_magnitude(6.0);
        // At m=6: E ≈ 1.0e-8 lux ⇒ L_eq ≈ 0.12 cd/m² ⇒ mid-mesopic.
        assert!(
            (0.3..=0.6).contains(&w),
            "m=6 chromatic weight {w}, expected mid-mesopic (~0.3–0.6)"
        );
    }

    /// Chromatic weight must be monotonically non-increasing with magnitude:
    /// fainter ⇒ less chromatic. Spot-check across the visible range.
    #[test]
    fn chromatic_weight_monotone_in_magnitude() {
        let mags = [-1.5_f64, 0.0, 1.5, 3.0, 4.5, 6.0, 7.5];
        let weights: Vec<f64> = mags
            .iter()
            .map(|&m| chromatic_weight_for_magnitude(m))
            .collect();
        for pair in weights.windows(2) {
            assert!(
                pair[0] >= pair[1] - 1e-9,
                "chromatic weight not monotone: {weights:?}"
            );
        }
    }

    /// Desaturation at w=1 is a no-op; at w=0 it collapses to the scotopic
    /// grey scalar replicated in every channel.
    #[test]
    fn desaturation_endpoints() {
        let red = [1.0_f32, 0.3, 0.1];
        let g = scotopic_grey(red);
        assert_eq!(apply_mesopic_desaturation(red, 1.0), red);
        let out = apply_mesopic_desaturation(red, 0.0);
        assert!((out[0] - g).abs() < 1e-6);
        assert!((out[1] - g).abs() < 1e-6);
        assert!((out[2] - g).abs() < 1e-6);
    }

    /// Neutral white must stay neutral white at every chromatic weight —
    /// the channel weights must sum to 1 for this to hold.
    #[test]
    fn neutral_white_is_invariant_under_desaturation() {
        let white = [1.0_f32, 1.0, 1.0];
        for &w in &[0.0_f32, 0.25, 0.5, 0.75, 1.0] {
            let out = apply_mesopic_desaturation(white, w);
            for c in out {
                assert!((c - 1.0).abs() < 1e-6, "white drifted at w={w}: {out:?}");
            }
        }
    }

    /// Kasten-Young 1989 is a curve fit, not an exact identity, so the
    /// formula reads 0.9997 at the zenith (the paper claims < 0.1% error
    /// across the full range). The defining behaviour is that the zenith
    /// is the *minimum* and the value is *near* 1; pin both.
    #[test]
    fn airmass_at_zenith_is_near_one() {
        let x = airmass_kasten_young(std::f64::consts::FRAC_PI_2);
        assert!(
            (x - 1.0).abs() < 1e-3,
            "zenith airmass = {x}, expected within 0.1% of 1.0 (Kasten-Young 1989)"
        );
        // And it must be the minimum: any other altitude is larger.
        for alt_deg in [10.0_f64, 30.0, 60.0, 89.0] {
            let other = airmass_kasten_young(alt_deg.to_radians());
            assert!(
                other > x,
                "X({alt_deg}°) = {other} should exceed zenith X = {x}"
            );
        }
    }

    /// At altitude 30° the Kasten-Young airmass should match the simple
    /// secant approximation to within ~0.01 — the refraction-aware
    /// correction only matters at low altitudes.
    #[test]
    fn airmass_matches_secant_at_high_altitude() {
        let alt = 30.0_f64.to_radians();
        let x = airmass_kasten_young(alt);
        // sec(zenith angle) = sec(60°) = 2.
        assert!(
            (x - 2.0).abs() < 0.01,
            "airmass at 30° = {x}, expected ≈ 2.0"
        );
    }

    /// Near the horizon the airmass diverges far above the secant
    /// approximation. Kasten-Young gives ≈38 at 0°, while sec(89°) is
    /// already 57 — the refraction term keeps the formula finite all the
    /// way to the horizon. Pin the value.
    #[test]
    fn airmass_at_horizon_matches_kasten_young_table() {
        let x = airmass_kasten_young(1e-6);
        // Kasten-Young 1989 Table 1 gives X(z=90°) = 37.92.
        assert!(
            (x - 37.92).abs() < 0.5,
            "horizon airmass = {x}, expected ≈ 37.92 (Kasten-Young 1989)"
        );
    }

    /// Airmass is undefined below the horizon — the slant path no longer
    /// has a defensible physical meaning. Return +∞ so accidental
    /// below-horizon evaluation produces a loud `f64::INFINITY` extinction
    /// rather than a silently finite "sort of dim" number.
    #[test]
    fn airmass_below_horizon_is_infinite() {
        assert_eq!(airmass_kasten_young(0.0), f64::INFINITY);
        assert_eq!(airmass_kasten_young(-10.0_f64.to_radians()), f64::INFINITY);
    }

    /// Default extinction coefficients must be in the canonical order
    /// `k_R < k_G < k_B` (Rayleigh scattering scales as λ⁻⁴), and within
    /// the literature-typical bracket for a clean sea-level dark site.
    /// The exact triple is pinned to catch accidental refactors.
    #[test]
    fn default_extinction_is_blue_heaviest() {
        let [r, g, b] = DEFAULT_EXTINCTION_K_RGB;
        assert!(
            r < g && g < b,
            "k_RGB must be monotone red→blue: {r}, {g}, {b}"
        );
        assert!((0.05..=0.20).contains(&r), "k_R out of typical range: {r}");
        assert!((0.10..=0.25).contains(&g), "k_G out of typical range: {g}");
        assert!((0.20..=0.45).contains(&b), "k_B out of typical range: {b}");
        // Pin the exact published triple so a refactor can't drift it.
        assert_eq!(DEFAULT_EXTINCTION_K_RGB, [0.10, 0.16, 0.30]);
    }

    /// At zenith the extinction is just `k(λ)` per channel (X = 1). This
    /// is the basic sanity check on the multiplication.
    #[test]
    fn extinction_at_zenith_is_k() {
        let dm = extinction_magnitudes_rgb(1.0, DEFAULT_EXTINCTION_K_RGB);
        for (i, &k) in DEFAULT_EXTINCTION_K_RGB.iter().enumerate() {
            assert!((dm[i] - k).abs() < 1e-9);
        }
    }

    /// Cone TVI plateau (Ferwerda Eq. 1, dim end): the photopic threshold
    /// stops decreasing below ~`log L_a = -2.6`, fixed at `log T_p = -0.72`.
    /// Pin both the value and the plateau behaviour.
    #[test]
    fn cone_tvi_dim_plateau() {
        assert!((cone_tvi_log10(-3.0) - (-0.72)).abs() < 1e-12);
        assert!((cone_tvi_log10(-5.0) - (-0.72)).abs() < 1e-12);
        // Just inside the plateau — still pinned at the plateau value.
        assert!((cone_tvi_log10(-2.6) - (-0.72)).abs() < 1e-12);
    }

    /// Cone TVI Weber regime (Ferwerda Eq. 1, bright end): for
    /// `log L_a ≥ 1.9` the threshold tracks adaptation as
    /// `log T_p = log L_a - 1.255` (Weber's law with constant 10^-1.255
    /// ≈ 0.056).
    #[test]
    fn cone_tvi_weber_regime() {
        for la in [2.0_f64, 3.0, 5.0] {
            let expected = la - 1.255;
            assert!(
                (cone_tvi_log10(la) - expected).abs() < 1e-12,
                "cone_tvi(log L_a={la}) = {}, expected {expected}",
                cone_tvi_log10(la)
            );
        }
    }

    /// Cone TVI is monotonically non-decreasing in log adaptation
    /// luminance: brighter adaptation ⇒ higher (or equal) detection
    /// threshold. The piecewise formula must not introduce a dip.
    #[test]
    fn cone_tvi_monotone() {
        let samples: Vec<f64> = (-50..50).map(|i| f64::from(i) / 10.0).collect();
        let mut prev = cone_tvi_log10(samples[0]);
        for &la in &samples[1..] {
            let t = cone_tvi_log10(la);
            assert!(
                t >= prev - 1e-12,
                "cone TVI not monotone: T({la}) = {t} < T(prev) = {prev}"
            );
            prev = t;
        }
    }

    /// Rod TVI plateau / saturation: dim plateau at `log T_s = -2.86`,
    /// upper-end linear regime `log T_s = log L_a - 0.395`.
    #[test]
    fn rod_tvi_plateau_and_saturation() {
        // Dark plateau.
        assert!((rod_tvi_log10(-5.0) - (-2.86)).abs() < 1e-12);
        assert!((rod_tvi_log10(-3.94) - (-2.86)).abs() < 1e-12);
        // Saturation regime.
        for la in [-1.0_f64, 0.0, 1.0] {
            let expected = la - 0.395;
            assert!((rod_tvi_log10(la) - expected).abs() < 1e-12);
        }
    }

    /// Rod TVI must also be monotone in log adaptation luminance.
    #[test]
    fn rod_tvi_monotone() {
        let samples: Vec<f64> = (-50..50).map(|i| f64::from(i) / 10.0).collect();
        let mut prev = rod_tvi_log10(samples[0]);
        for &la in &samples[1..] {
            let t = rod_tvi_log10(la);
            assert!(
                t >= prev - 1e-12,
                "rod TVI not monotone: T({la}) = {t} < T(prev) = {prev}"
            );
            prev = t;
        }
    }

    /// Sanity-check the HDR-flux → cd/m² conversion against Schaefer 1990:
    /// a magnitude-0 point source produces 2.54e-6 lux of illuminance
    /// at the eye; spread over the 1-arcmin² PSF that's ≈30 cd/m² of
    /// equivalent luminance. The renderer's `flux = 1` at
    /// `zeropoint = 0` must reproduce that number.
    #[test]
    fn hdr_flux_to_luminance_matches_schaefer_zeropoint() {
        let l = hdr_flux_to_luminance_cd_m2(1.0, 0.0);
        // 2.54e-6 lux / 8.46e-8 sr ≈ 30 cd/m².
        assert!(
            (l - 30.0).abs() < 1.0,
            "mag-0 luminance = {l} cd/m², expected ≈ 30"
        );
    }

    /// At airmass 2 (altitude ≈30°) the blue channel must dim by 0.6 mag
    /// and the red channel by only 0.2 mag with the default coefficients
    /// — a clear reddening signature even at high altitude.
    #[test]
    fn extinction_reddens_at_airmass_two() {
        let [dm_r, _dm_g, dm_b] = extinction_magnitudes_rgb(2.0, DEFAULT_EXTINCTION_K_RGB);
        assert!((dm_r - 0.20).abs() < 1e-9);
        assert!((dm_b - 0.60).abs() < 1e-9);
        assert!(
            dm_b > dm_r + 0.3,
            "airmass 2 should redden by at least 0.3 mag, got Δm_B - Δm_R = {}",
            dm_b - dm_r
        );
    }

    /// A pure-red stimulus should desaturate strongly under scotopic
    /// conditions: rods are essentially insensitive to long-wavelength
    /// light, so a red star fades to near-black, not to a bright grey.
    #[test]
    fn pure_red_collapses_to_near_black_in_scotopic() {
        let pure_red = [1.0_f32, 0.0, 0.0];
        let out = apply_mesopic_desaturation(pure_red, 0.0);
        for c in out {
            assert!(
                c < 0.05,
                "pure red should be near-black under scotopic, got {out:?}"
            );
        }
    }
}
