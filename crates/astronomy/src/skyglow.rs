//! Diffuse night-sky surface-brightness model.
//!
//! At a dark site the visible night sky is *not* black between the resolved
//! stars: a diffuse glow remains, made up of (in decreasing order of
//! contribution to the Milky Way band):
//!
//! * **Integrated starlight (ISL)** — the unresolved population of stars
//!   too faint for any naked-eye catalogue. Strongly concentrated along
//!   the galactic plane; **this is the dominant Milky Way ingredient.**
//! * **Diffuse galactic light (DGL)** — interstellar dust scattering the
//!   integrated starlight. Tracks ISL with ~20–40% relative amplitude.
//! * Zodiacal light, airglow, integrated extragalactic light — smaller,
//!   broader components; deferred to a follow-up PR.
//!
//! This module implements an analytic V-band ISL+DGL model in galactic
//! coordinates, fit to the published surface-brightness profiles in:
//!
//!   Leinert, Ch., Bowyer, S., Haikala, L. K., et al. 1998,
//!   *The 1997 reference of diffuse night sky brightness*,
//!   A&AS 127, 1–99, §6 (Integrated starlight) and §8 (DGL).
//!
//! The fit is an approximation of the Leinert tables, not a digitisation
//! of them — adequate for naked-eye visualisation, **not** for radiometric
//! analysis. The doc comment on [`isl_mag_per_arcsec2`] lists the
//! literature reference points it is calibrated against.
//!
//! ## Coordinate convention
//!
//! Galactic coordinates `(l, b)` use the IAU 1958 system at the J2000
//! equinox. `l` is galactic longitude (0 = direction to the Galactic
//! Center), `b` is galactic latitude (positive = north galactic
//! hemisphere). Both in radians.

use glam::{Mat3, Vec3};

/// Rotation matrix from J2000 equatorial unit vectors to galactic
/// coordinates (IAU 1958, refined by Murray 1989 / ESA SP-1200 1997).
///
/// Apply as `M · v_eq` to get `(x_g, y_g, z_g)` such that
/// `z_g = sin(b)`, `(x_g, y_g) = cos(b)·(cos l, sin l)`.
///
/// The constants are the standard high-precision values used by SOFA's
/// `iauIcrs2g`; SOFA is the IAU's reference implementation, so digitising
/// them here keeps us bit-compatible with the literature.
#[rustfmt::skip]
const EQUATORIAL_TO_GALACTIC_ROWS: [[f64; 3]; 3] = [
    [-0.054_875_560_416_215, -0.873_437_090_234_885, -0.483_835_015_548_713],
    [ 0.494_109_427_875_584, -0.444_829_629_960_011,  0.746_982_244_497_219],
    [-0.867_666_149_019_004, -0.198_076_373_431_201,  0.455_983_776_175_067],
];

/// Convert a unit vector in J2000 equatorial coordinates to galactic
/// longitude/latitude `(l, b)`, both in radians.
///
/// `l ∈ [0, 2π)`, `b ∈ [-π/2, π/2]`.
pub fn equatorial_to_galactic(v_eq: Vec3) -> (f64, f64) {
    let v_g_x = EQUATORIAL_TO_GALACTIC_ROWS[0][0] * v_eq.x as f64
        + EQUATORIAL_TO_GALACTIC_ROWS[0][1] * v_eq.y as f64
        + EQUATORIAL_TO_GALACTIC_ROWS[0][2] * v_eq.z as f64;
    let v_g_y = EQUATORIAL_TO_GALACTIC_ROWS[1][0] * v_eq.x as f64
        + EQUATORIAL_TO_GALACTIC_ROWS[1][1] * v_eq.y as f64
        + EQUATORIAL_TO_GALACTIC_ROWS[1][2] * v_eq.z as f64;
    let v_g_z = EQUATORIAL_TO_GALACTIC_ROWS[2][0] * v_eq.x as f64
        + EQUATORIAL_TO_GALACTIC_ROWS[2][1] * v_eq.y as f64
        + EQUATORIAL_TO_GALACTIC_ROWS[2][2] * v_eq.z as f64;
    let b = v_g_z.clamp(-1.0, 1.0).asin();
    let l = v_g_y.atan2(v_g_x).rem_euclid(std::f64::consts::TAU);
    (l, b)
}

/// 3×3 matrix form of the equatorial→galactic rotation, for callers that
/// want to bake the transform into a larger matrix pipeline (e.g. a GPU
/// shader uniform).
pub fn equatorial_to_galactic_matrix() -> Mat3 {
    let r = EQUATORIAL_TO_GALACTIC_ROWS;
    // glam Mat3::from_cols stores columns; transpose rows → columns.
    Mat3::from_cols_array(&[
        r[0][0] as f32,
        r[1][0] as f32,
        r[2][0] as f32,
        r[0][1] as f32,
        r[1][1] as f32,
        r[2][1] as f32,
        r[0][2] as f32,
        r[1][2] as f32,
        r[2][2] as f32,
    ])
}

/// V-band surface brightness in mag/arcsec² → linear surface flux in the
/// "magnitude-zero point" radiometric units used by the renderer.
///
/// ```text
///     F = 10^(-0.4 · (mu - m_ref))
/// ```
///
/// where `m_ref` is the zero-point apparent magnitude (a point source of
/// magnitude `m_ref` has unit flux). The output is dimensionless flux
/// **per arcsec²**; multiply by the pixel solid angle (in arcsec²) to
/// get the per-pixel HDR contribution the renderer expects.
pub fn surface_brightness_to_linear_flux(mu_mag_per_arcsec2: f64, m_ref: f64) -> f64 {
    10.0_f64.powf(-0.4 * (mu_mag_per_arcsec2 - m_ref))
}

// =============================================================================
// Analytic Integrated-Starlight (+DGL) model
// =============================================================================
//
// Empirical V-band surface brightness `μ(l, b)` in mag/arcsec², fit to the
// 1-D profiles published in Leinert et al. 1998 §6 (galactic-latitude
// dependence) and §8 (longitude dependence + diffuse galactic light
// contribution). The qualitative structure is:
//
//   * Galactic poles (|b| ≈ 90°): isotropic floor at μ_floor ≈ 23.5 mag/arcsec².
//   * Galactic plane (b = 0°): thin disk peaks brightly near the bulge
//     (μ ≈ 21 at l = 0°) and fades toward the anti-centre (μ ≈ 22 at
//     l = 180°).
//   * Thin-disk thickness in latitude: σ_b ≈ 4° (Gaussian).
//   * A *thick-disk* component (σ ≈ 30°) keeps the sky a few tenths of a
//     magnitude brighter off-plane near the bulge than at the galactic
//     pole, matching Leinert §6's smooth fall-off with |b|.
//
// We sum the components in linear flux (S10 units) before converting back
// to magnitudes, which keeps the photometric addition correct.

const POLE_FLUX_S10: f64 = 50.0; // ~23.5 mag/arcsec², galactic-pole floor
const THIN_DISK_UNIFORM_S10: f64 = 60.0; // "baseline" thin-disk brightness at b = 0
const THIN_DISK_BULGE_S10: f64 = 400.0; // extra central enhancement on the thin disk
const THICK_DISK_S10: f64 = 50.0; // broad component, keeps |b| < 45° above floor
const SIGMA_B_THIN_DEG: f64 = 4.0; // thin disk Gaussian σ in galactic latitude
const SIGMA_B_THICK_DEG: f64 = 30.0; // thick disk Gaussian σ in galactic latitude
const SIGMA_L_BULGE_DEG: f64 = 60.0; // bulge Gaussian σ in galactic longitude

/// Conversion: 1 S10 unit = 27.78 V-mag per arcsec².
///
/// One S10 unit is the surface brightness of a 10th-magnitude star spread
/// uniformly over one square degree. Since `(mag/arcsec²) = mag(star) +
/// 2.5·log10(arcsec² per square degree) = 10 + 2.5·log10(3600²) ≈ 27.78`,
/// a surface with `F` S10 units shines at
/// `μ = 27.78 - 2.5·log10(F)` mag/arcsec².
const S10_TO_MAG_ARCSEC2_OFFSET: f64 = 27.78;

/// Approximate V-band integrated-starlight + diffuse-galactic-light
/// surface brightness in mag/arcsec², at galactic coordinates `(l, b)`
/// in radians.
///
/// Calibrated against Leinert et al. 1998 §6 reference points; the model
/// reproduces the published 1-D profiles to within ~0.5 mag/arcsec²
/// across the visible range. This is adequate for naked-eye Milky Way
/// visualisation; it is **not** a substitute for the published tables in
/// radiometric applications.
///
/// Smaller `μ` = brighter sky.
pub fn isl_mag_per_arcsec2(l_rad: f64, b_rad: f64) -> f64 {
    let l_deg = l_rad.to_degrees();
    let b_deg = b_rad.to_degrees();

    // Wrap longitude to (-180°, 180°] so the bulge Gaussian is symmetric
    // around the galactic centre.
    let l_centered = if l_deg > 180.0 { l_deg - 360.0 } else { l_deg };

    let thin_lat_factor = (-(b_deg * b_deg) / (2.0 * SIGMA_B_THIN_DEG * SIGMA_B_THIN_DEG)).exp();
    let thick_lat_factor = (-(b_deg * b_deg) / (2.0 * SIGMA_B_THICK_DEG * SIGMA_B_THICK_DEG)).exp();
    let bulge_lon_factor =
        (-(l_centered * l_centered) / (2.0 * SIGMA_L_BULGE_DEG * SIGMA_L_BULGE_DEG)).exp();

    let flux_s10 = POLE_FLUX_S10
        + THICK_DISK_S10 * thick_lat_factor
        + (THIN_DISK_UNIFORM_S10 + THIN_DISK_BULGE_S10 * bulge_lon_factor) * thin_lat_factor;

    S10_TO_MAG_ARCSEC2_OFFSET - 2.5 * flux_s10.log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deg(x: f64) -> f64 {
        x.to_radians()
    }

    /// The IAU galactic-rotation matrix is orthonormal: rows are unit
    /// length and mutually orthogonal. Pin this so a transcription error
    /// in the constants would be caught.
    #[test]
    fn galactic_matrix_is_orthonormal() {
        let m = EQUATORIAL_TO_GALACTIC_ROWS;
        for row in &m {
            let n = row[0] * row[0] + row[1] * row[1] + row[2] * row[2];
            assert!(
                (n - 1.0).abs() < 1e-9,
                "row {row:?} not unit length: |.|² = {n}"
            );
        }
        // Pairwise orthogonality.
        for i in 0..3 {
            for j in i + 1..3 {
                let dot = m[i][0] * m[j][0] + m[i][1] * m[j][1] + m[i][2] * m[j][2];
                assert!(
                    dot.abs() < 1e-9,
                    "rows {i} and {j} not orthogonal: dot = {dot}"
                );
            }
        }
    }

    /// The galactic centre (Sagittarius A*) sits at J2000 equatorial
    /// `(α, δ) ≈ (266.405°, -28.936°)` and should map to galactic
    /// `(l, b) ≈ (0°, 0°)` to within rounding of the catalogue position
    /// versus the dynamical centre.
    #[test]
    fn galactic_centre_maps_to_zero_zero() {
        let ra = deg(266.405);
        let dec = deg(-28.936);
        let v = Vec3::new(
            (dec.cos() * ra.cos()) as f32,
            (dec.cos() * ra.sin()) as f32,
            dec.sin() as f32,
        );
        let (l, b) = equatorial_to_galactic(v);
        assert!(
            l.to_degrees() < 0.5 || l.to_degrees() > 359.5,
            "Sgr A* should be near l = 0°, got {}°",
            l.to_degrees()
        );
        assert!(
            b.to_degrees().abs() < 0.5,
            "Sgr A* should be near b = 0°, got {}°",
            b.to_degrees()
        );
    }

    /// The north galactic pole sits at J2000 equatorial
    /// `(α, δ) = (192.85948°, 27.12825°)` (this is *how* the matrix is
    /// defined). It must map to galactic `b = +90°`. Tolerance is loose
    /// (~1 arcmin) because the input vector is built in `f32` so
    /// `asin(≈1)` accumulates the usual square-root precision loss near
    /// the pole — not a defect of the rotation matrix itself.
    #[test]
    fn north_galactic_pole_maps_to_b_plus_ninety() {
        let ra = deg(192.85948);
        let dec = deg(27.12825);
        let v = Vec3::new(
            (dec.cos() * ra.cos()) as f32,
            (dec.cos() * ra.sin()) as f32,
            dec.sin() as f32,
        );
        let (_, b) = equatorial_to_galactic(v);
        assert!(
            (b.to_degrees() - 90.0).abs() < 0.02,
            "NGP should map to b ≈ 90°, got {}°",
            b.to_degrees()
        );
    }

    /// Surface-brightness → linear flux mapping is monotone: a brighter
    /// (numerically smaller) magnitude produces more linear flux. And a
    /// 5-magnitude difference is exactly a factor of 100, as on point
    /// sources (Pogson's law).
    #[test]
    fn surface_brightness_pogson_law() {
        let f21 = surface_brightness_to_linear_flux(21.0, 0.0);
        let f26 = surface_brightness_to_linear_flux(26.0, 0.0);
        let ratio = f21 / f26;
        assert!(
            (ratio - 100.0).abs() < 1e-6,
            "5-mag SB ratio = {ratio}, expected 100"
        );
    }

    /// ISL surface brightness must be brighter (= numerically smaller μ) in
    /// the galactic centre direction than at the pole. This is the
    /// defining property of the Milky Way band.
    #[test]
    fn galactic_centre_brighter_than_pole() {
        let mu_centre = isl_mag_per_arcsec2(0.0, 0.0);
        let mu_pole = isl_mag_per_arcsec2(0.0, std::f64::consts::FRAC_PI_2);
        assert!(
            mu_centre < mu_pole - 2.0,
            "galactic centre μ = {mu_centre}, pole μ = {mu_pole}: centre should be ≥ 2 mag brighter"
        );
    }

    /// ISL at five Leinert 1998 reference points (§6 summary). Tolerance
    /// is the published-spread of the underlying photometry (±0.5 mag/
    /// arcsec²) plus an analytic-fit allowance; tightening this would
    /// require digitising the full 2-D table instead of using a sum of
    /// Gaussians (see ROADMAP).
    #[test]
    fn isl_matches_leinert_reference_points() {
        // (label, l_deg, b_deg, expected μ, tolerance)
        let cases: &[(&str, f64, f64, f64, f64)] = &[
            ("galactic centre", 0.0, 0.0, 21.0, 0.7),
            ("local plane", 90.0, 0.0, 21.7, 0.7),
            ("anti-centre", 180.0, 0.0, 22.2, 0.7),
            ("off-plane near bulge", 0.0, 30.0, 23.0, 0.7),
            ("galactic pole", 0.0, 90.0, 23.5, 0.5),
        ];
        for (label, l, b, expected, tol) in cases {
            let got = isl_mag_per_arcsec2(l.to_radians(), b.to_radians());
            assert!(
                (got - expected).abs() < *tol,
                "{label} ({l}°, {b}°): got μ = {got}, expected {expected} ± {tol}"
            );
        }
    }

    /// The disk drops off rapidly in galactic latitude: at |b| = σ_b ≈ 4°
    /// the disk component is at 1/√e of its peak; at |b| = 30° the disk
    /// is negligible and only the pole floor remains. Pin the latitudinal
    /// fall-off so the disk doesn't accidentally become broad.
    #[test]
    fn disk_falls_off_in_latitude() {
        let mu_b0 = isl_mag_per_arcsec2(deg(90.0), 0.0);
        let mu_b30 = isl_mag_per_arcsec2(deg(90.0), deg(30.0));
        assert!(
            mu_b30 > mu_b0 + 1.0,
            "disk should fade ≥ 1 mag from plane to b=30°: μ(0)={mu_b0}, μ(30°)={mu_b30}"
        );
    }
}
