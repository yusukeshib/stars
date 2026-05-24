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
//! primitives and are scoped in ROADMAP.md Phase 2.5.

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
