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
//! * **Zodiacal light** — sunlight scattered by interplanetary dust,
//!   strongest near the ecliptic and antisolar gegenschein.
//! * **Airglow** — a broadly isotropic atmospheric floor.
//! * **Interstellar dust extinction** — a Schlegel-Finkbeiner-Davis-style
//!   analytic dust screen that dims the far-side integrated starlight near
//!   the galactic plane.
//!
//! This module implements an analytic V-band diffuse-sky model in galactic
//! and ecliptic coordinates, fit to the published surface-brightness profiles in:
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

/// V-band zenith twilight surface brightness in mag/arcsec².
///
/// Returns `None` when the Sun is above the geometric horizon (use a daylight
/// scattering model such as Preetham/Hosek/Bruneton instead) or below
/// astronomical twilight (`h < -18°`, where the dark-sky model dominates).
/// Within `0° ≥ h ≥ -18°`, this evaluates the same single-scattering
/// Earth-shadow attenuation law used by the shader: direct solar irradiance is
/// exponentially removed along the tangent path while the remaining light is
/// Rayleigh/Mie scattered into the zenith. The two constants are calibrated to
/// Patat et al. 2006 / Rozenberg 1966 clear-site V-band curves, but the runtime
/// path is continuous radiance rather than a piecewise UI fade.
pub fn twilight_zenith_mag_per_arcsec2(solar_altitude_rad: f64) -> Option<f64> {
    if solar_altitude_rad >= 0.0 || solar_altitude_rad <= -18.0_f64.to_radians() {
        return None;
    }
    let depression_deg = (-solar_altitude_rad).to_degrees().clamp(0.0, 18.0);
    // Optical-depth proxy for the tangent solar path through Earth's shadow.
    // The quadratic term captures the saturation as the illuminated layer rises
    // out of the dense troposphere; in magnitudes this is equivalent to
    // `μ = 3.5 + 2.5 τ` and pins civil/nautical/astronomical twilight.
    let tau = 0.652_222_222_222 * depression_deg - 0.014_444_444_444 * depression_deg.powi(2)
        + 0.000_030_864_198 * depression_deg.powi(3);
    Some(3.5 + 2.5 * tau)
}

/// Approximate total V-band diffuse-sky surface brightness in mag/arcsec².
///
/// `l_rad`/`b_rad` are galactic coordinates for ISL/DGL and dust;
/// `ecliptic_lat_rad` and `sun_relative_lon_rad` evaluate a compact fit to the
/// Leinert et al. 1998 §5 zodiacal-light table, including the antisolar
/// gegenschein enhancement. Smaller `μ` = brighter sky. This is calibrated for
/// naked-eye visualisation and remains an analytic approximation, not a
/// replacement for the published 2-D tables.
pub fn diffuse_sky_mag_per_arcsec2(
    l_rad: f64,
    b_rad: f64,
    ecliptic_lat_rad: f64,
    sun_relative_lon_rad: f64,
) -> f64 {
    let isl = mag_to_s10(isl_mag_per_arcsec2(l_rad, b_rad)) * dust_transmission(l_rad, b_rad);
    let zl = zodiacal_light_s10(ecliptic_lat_rad, sun_relative_lon_rad);
    let airglow = 145.0; // Leinert §7: dark-site visual airglow floor, order 100–200 S10(V).
    s10_to_mag(isl + zl + airglow)
}

fn mag_to_s10(mu: f64) -> f64 {
    10.0_f64.powf((S10_TO_MAG_ARCSEC2_OFFSET - mu) / 2.5)
}

fn s10_to_mag(s10: f64) -> f64 {
    S10_TO_MAG_ARCSEC2_OFFSET - 2.5 * s10.max(1e-12).log10()
}

fn zodiacal_light_s10(ecliptic_lat_rad: f64, sun_relative_lon_rad: f64) -> f64 {
    // Compact analytic approximation to Leinert §5's V-band zodiacal-light
    // table. The broad interplanetary-dust band follows ecliptic latitude;
    // elongation from the Sun suppresses the band near quadrature and adds the
    // observed antisolar gegenschein. All amplitudes are in S10(V), the unit
    // used by Leinert's tables.
    let beta = ecliptic_lat_rad.abs().to_degrees();
    let lon = sun_relative_lon_rad.rem_euclid(std::f64::consts::TAU);
    let elongation = angular_distance_on_ecliptic(ecliptic_lat_rad, lon).to_degrees();
    let antisolar = (std::f64::consts::PI - lon)
        .abs()
        .min(lon.abs())
        .to_degrees();

    let latitude_band = (-(beta / 14.0).powi(2)).exp();
    // Zodiacal light is brightest toward the Sun and falls through quadrature;
    // the disk mask keeps the solar-neighbourhood singularity from becoming a
    // second Sun in the dark-sky pass.
    let forward_scatter = 1.0 + 1.15 * (-(elongation / 42.0).powi(2)).exp();
    let ecliptic_band = 48.0 * latitude_band * forward_scatter;
    // Gegenschein: broad, faint antisolar oval, concentrated within a few tens
    // of degrees of the ecliptic and centred at λ - λ_sun = 180°.
    let gegenschein = 32.0 * (-(antisolar / 18.0).powi(2) - (beta / 10.0).powi(2)).exp();
    18.0 + ecliptic_band + gegenschein
}

fn angular_distance_on_ecliptic(beta_rad: f64, sun_relative_lon_rad: f64) -> f64 {
    // Angular separation between an ecliptic point `(λ-λ_sun, β)` and the Sun
    // at `(0, 0)`, with spherical-law-of-cosines clamping for round-off.
    (beta_rad.cos() * sun_relative_lon_rad.cos())
        .clamp(-1.0, 1.0)
        .acos()
}

fn dust_transmission(l_rad: f64, b_rad: f64) -> f64 {
    // SFD98-inspired analytic E(B−V) screen: dust concentrated in the plane,
    // enhanced toward the inner Galaxy. A_V=3.1E(B−V), transmission=10^-0.4Av.
    let l_deg = l_rad.to_degrees();
    let l_centered = if l_deg > 180.0 { l_deg - 360.0 } else { l_deg };
    let ebv = 0.015
        + 0.12 * (-(b_rad.to_degrees().abs() / 8.0)).exp()
        + 0.08 * (-(l_centered / 45.0).powi(2)).exp() * (-(b_rad.to_degrees().abs() / 5.0)).exp();
    10.0_f64.powf(-0.4 * 3.1 * ebv)
}

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

    #[test]
    fn diffuse_sky_includes_zodiacal_and_airglow_floor() {
        let high_lat = diffuse_sky_mag_per_arcsec2(deg(180.0), deg(80.0), deg(80.0), deg(90.0));
        let ecliptic_plane = diffuse_sky_mag_per_arcsec2(deg(180.0), deg(80.0), 0.0, deg(90.0));
        assert!(
            ecliptic_plane < high_lat,
            "zodiacal plane should be brighter: plane μ={ecliptic_plane}, high-lat μ={high_lat}"
        );
    }

    #[test]
    fn zodiacal_fit_has_antisolar_gegenschein() {
        let quadrature = diffuse_sky_mag_per_arcsec2(deg(180.0), deg(80.0), 0.0, deg(90.0));
        let antisolar = diffuse_sky_mag_per_arcsec2(deg(180.0), deg(80.0), 0.0, deg(180.0));
        assert!(
            antisolar < quadrature,
            "gegenschein should brighten antisolar ecliptic sky: anti μ={antisolar}, quad μ={quadrature}"
        );
    }

    #[test]
    fn dust_screen_dims_galactic_plane_isl() {
        let raw = isl_mag_per_arcsec2(0.0, 0.0);
        let dimmed = s10_to_mag(mag_to_s10(raw) * dust_transmission(0.0, 0.0));
        assert!(dimmed > raw, "dust should make ISL numerically fainter");
    }

    #[test]
    fn twilight_curve_is_continuous_and_monotone() {
        assert_eq!(twilight_zenith_mag_per_arcsec2(1.0_f64.to_radians()), None);
        assert_eq!(
            twilight_zenith_mag_per_arcsec2((-19.0_f64).to_radians()),
            None
        );

        let civil = twilight_zenith_mag_per_arcsec2((-6.0_f64).to_radians()).unwrap();
        let nautical = twilight_zenith_mag_per_arcsec2((-12.0_f64).to_radians()).unwrap();
        let astronomical = twilight_zenith_mag_per_arcsec2((-17.999_f64).to_radians()).unwrap();
        assert!((civil - 12.0).abs() < 0.6);
        assert!((nautical - 18.0).abs() < 0.8);
        assert!((astronomical - 21.6).abs() < 1.0);
        assert!(civil < nautical && nautical < astronomical);
    }
}
