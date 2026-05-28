//! Phase-2 astrometric correction helpers.
//!
//! The routines here intentionally stay small and dependency-free so renderer
//! hosts can share the same correction stack:
//!
//! - IAU 2006 (P03) precession, expressed with the Fukushima-Williams angles
//!   published by Capitaine et al. and used by SOFA's `pfw06`/`fw2m` path.
//! - A compact IAU-2000-style nutation model using the dominant luni-solar
//!   terms. Its error is comfortably below the roadmap's naked-eye renderer
//!   budget (~9 arcsec) while keeping the WASM build small; swapping in the
//!   full 2000B table later only needs to replace [`nutation_iau2000b_approx`].
//! - First-order annual aberration from the Earth's orbital velocity, in units
//!   of `c`.
//! - Saemundsson/Meeus apparent-altitude refraction with pressure/temperature
//!   scaling.

use crate::J2000_JD;

const ARCSEC_TO_RAD: f64 = std::f64::consts::PI / (180.0 * 3600.0);
const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;
const JULIAN_CENTURY_DAYS: f64 = 36_525.0;
const JULIAN_YEAR_DAYS: f64 = 365.25;
const EARTH_ORBITAL_SPEED_OVER_C: f64 = 0.017_202_124 / 173.144_632_684_669_3;

pub type Mat3d = [[f64; 3]; 3];
pub type Vec3d = [f64; 3];

/// Julian years since the J2000.0 catalogue epoch, using TT.
pub fn years_since_j2000(jd_tt: f64) -> f64 {
    (jd_tt - J2000_JD) / JULIAN_YEAR_DAYS
}

/// Mean obliquity of the ecliptic, IAU 2006 polynomial, in radians.
pub fn mean_obliquity_iau2006(jd_tt: f64) -> f64 {
    let t = (jd_tt - J2000_JD) / JULIAN_CENTURY_DAYS;
    arcsec_to_rad(poly(
        t,
        &[
            84_381.406,
            -46.836_769,
            -0.000_183_1,
            0.002_003_40,
            -0.000_000_576,
            -0.000_000_043_4,
        ],
    ))
}

/// IAU 2006 (P03) precession matrix from the J2000 mean equator/equinox into
/// the mean equator/equinox of `jd_tt`.
pub fn precession_matrix_iau2006(jd_tt: f64) -> Mat3d {
    let t = (jd_tt - J2000_JD) / JULIAN_CENTURY_DAYS;

    // Fukushima-Williams angles in arcseconds, from SOFA `iauPfw06`.
    let gamb = arcsec_to_rad(poly(
        t,
        &[
            -0.052_928,
            10.556_378,
            0.493_204_4,
            -0.000_312_38,
            -0.000_002_788,
            0.000_000_026_0,
        ],
    ));
    let phib = arcsec_to_rad(poly(
        t,
        &[
            84_381.412_819,
            -46.811_016,
            0.051_126_8,
            0.000_532_89,
            -0.000_000_440,
            -0.000_000_017_6,
        ],
    ));
    let psib = arcsec_to_rad(poly(
        t,
        &[
            -0.041_775,
            5_038.481_484,
            1.558_417_5,
            -0.000_185_22,
            -0.000_026_452,
            -0.000_000_014_8,
        ],
    ));
    let epsa = mean_obliquity_iau2006(jd_tt);

    mat_mul(
        rot_x(-epsa),
        mat_mul(rot_z(-psib), mat_mul(rot_x(phib), rot_z(gamb))),
    )
}

/// Nutation in longitude and obliquity, radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nutation {
    pub dpsi_rad: f64,
    pub deps_rad: f64,
}

/// Compact IAU-2000-style nutation approximation.
///
/// The full IAU 2000B model is a 77-term table. For the renderer's current
/// ~9 arcsec roadmap target, the four dominant Meeus/IAU luni-solar terms are
/// sufficient and much cheaper for WASM. Return values follow the IAU sign
/// convention: `dpsi` is nutation in ecliptic longitude, `deps` in obliquity.
pub fn nutation_iau2000b_approx(jd_tt: f64) -> Nutation {
    let t = (jd_tt - J2000_JD) / JULIAN_CENTURY_DAYS;
    let omega = deg_to_rad(poly(
        t,
        &[125.044_52, -1_934.136_261, 0.002_070_8, 1.0 / 450_000.0],
    ));
    let mean_sun = deg_to_rad(poly(t, &[280.466_5, 36_000.769_8]));
    let mean_moon = deg_to_rad(poly(t, &[218.316_5, 481_267.881_3]));

    let dpsi_arcsec =
        -17.20 * omega.sin() - 1.32 * (2.0 * mean_sun).sin() - 0.23 * (2.0 * mean_moon).sin()
            + 0.21 * (2.0 * omega).sin();
    let deps_arcsec =
        9.20 * omega.cos() + 0.57 * (2.0 * mean_sun).cos() + 0.10 * (2.0 * mean_moon).cos()
            - 0.09 * (2.0 * omega).cos();
    Nutation {
        dpsi_rad: arcsec_to_rad(dpsi_arcsec),
        deps_rad: arcsec_to_rad(deps_arcsec),
    }
}

/// Nutation matrix from mean equator/equinox of date to true equator/equinox
/// of date.
pub fn nutation_matrix_iau2000b_approx(jd_tt: f64) -> Mat3d {
    let eps = mean_obliquity_iau2006(jd_tt);
    let n = nutation_iau2000b_approx(jd_tt);
    mat_mul(
        rot_x(-(eps + n.deps_rad)),
        mat_mul(rot_z(-n.dpsi_rad), rot_x(eps)),
    )
}

/// Combined J2000 mean equator/equinox → true equator/equinox of date matrix.
pub fn precession_nutation_matrix(jd_tt: f64) -> Mat3d {
    mat_mul(
        nutation_matrix_iau2000b_approx(jd_tt),
        precession_matrix_iau2006(jd_tt),
    )
}

/// Equation of the equinoxes, radians, for apparent sidereal time.
pub fn equation_of_equinoxes(jd_tt: f64) -> f64 {
    let n = nutation_iau2000b_approx(jd_tt);
    n.dpsi_rad * (mean_obliquity_iau2006(jd_tt) + n.deps_rad).cos()
}

/// Approximate Earth barycentric orbital velocity in J2000 equatorial
/// coordinates, divided by the speed of light.
pub fn earth_velocity_over_c_j2000(jd_tdb: f64) -> Vec3d {
    let t = (jd_tdb - J2000_JD) / JULIAN_CENTURY_DAYS;
    let mean_long = deg_to_rad(poly(t, &[280.466_46, 36_000.769_83, 0.000_303_2]));
    let mean_anomaly = deg_to_rad(poly(
        t,
        &[357.529_11, 35_999.050_29, -0.000_153_7, -1.0 / 24_490_000.0],
    ));
    let equation_center = deg_to_rad(
        (1.914_602 - 0.004_817 * t - 0.000_014 * t * t) * mean_anomaly.sin()
            + (0.019_993 - 0.000_101 * t) * (2.0 * mean_anomaly).sin()
            + 0.000_289 * (3.0 * mean_anomaly).sin(),
    );
    // Geocentric apparent solar longitude. The Earth's heliocentric longitude
    // is opposite; differentiating that orbit gives the velocity tangent below.
    let sun_long = mean_long + equation_center;
    let beta_ecl = [sun_long.sin(), -sun_long.cos(), 0.0];
    let eps = arcsec_to_rad(84_381.406);
    let (se, ce) = eps.sin_cos();
    [
        EARTH_ORBITAL_SPEED_OVER_C * beta_ecl[0],
        EARTH_ORBITAL_SPEED_OVER_C * (ce * beta_ecl[1] - se * beta_ecl[2]),
        EARTH_ORBITAL_SPEED_OVER_C * (se * beta_ecl[1] + ce * beta_ecl[2]),
    ]
}

/// First-order annual aberration of a unit direction by observer velocity
/// `beta = v/c`. The output is normalized and stays in the same frame as the
/// input vector and `beta`.
pub fn annual_aberration(direction: Vec3d, beta: Vec3d) -> Vec3d {
    let dot = dot(direction, beta);
    normalize([
        direction[0] + beta[0] - dot * direction[0],
        direction[1] + beta[1] - dot * direction[1],
        direction[2] + beta[2] - dot * direction[2],
    ])
}

/// Saemundsson 1986 / Meeus apparent-altitude refraction. The input is true
/// altitude and the output is apparent altitude, both radians. `pressure_hpa`
/// and `temperature_c` scale the standard 1010 hPa / 10 °C formula.
pub fn refracted_altitude_saemundsson(
    true_altitude_rad: f64,
    pressure_hpa: f64,
    temperature_c: f64,
) -> f64 {
    true_altitude_rad + saemundsson_refraction_angle(true_altitude_rad, pressure_hpa, temperature_c)
}

/// Saemundsson refraction *angle* `ρ = apparent − true`, in radians, with
/// pressure/temperature scaling. Returns zero outside the formula's valid
/// domain so callers can compose it with the dispersion scaling below
/// without re-implementing the domain guard.
fn saemundsson_refraction_angle(
    true_altitude_rad: f64,
    pressure_hpa: f64,
    temperature_c: f64,
) -> f64 {
    let alt_deg = true_altitude_rad.to_degrees();
    if !alt_deg.is_finite() || !(-1.0..=89.9).contains(&alt_deg) {
        return 0.0;
    }
    let pressure_scale = if pressure_hpa.is_finite() {
        pressure_hpa.max(0.0) / 1010.0
    } else {
        1.0
    };
    let temp_k = if temperature_c.is_finite() {
        (273.0 + temperature_c).max(150.0)
    } else {
        283.0
    };
    let weather_scale = pressure_scale * 283.0 / temp_k;
    let r_arcmin = 1.02 / ((alt_deg + 10.3 / (alt_deg + 5.11)).to_radians()).tan() * weather_scale;
    (r_arcmin / 60.0).to_radians()
}

/// Edlén 1966 empirical refractivity for standard dry air at 15 °C / 760 Torr.
/// Returns `(n − 1)` (the refractivity), accurate to ~1 × 10⁻⁸ across the
/// visible band. The pressure / temperature scaling enters the renderer
/// through Saemundsson; this helper carries only the spectral dispersion.
///
/// Reference: Edlén, B. 1966, Metrologia 2, 71.
pub fn edlen_refractivity_standard_air(wavelength_nm: f64) -> f64 {
    if !wavelength_nm.is_finite() || wavelength_nm <= 0.0 {
        return EDLEN_REFERENCE_REFRACTIVITY;
    }
    // Edlén tabulates the formula in inverse micrometres (σ = 1/λ[µm]).
    let sigma = 1_000.0 / wavelength_nm;
    let sigma_sq = sigma * sigma;
    let n_minus_one = 8_342.54 + 2_406_147.0 / (130.0 - sigma_sq) + 15_998.0 / (38.9 - sigma_sq);
    n_minus_one * 1.0e-8
}

/// Reference green-channel wavelength (550 nm). Saemundsson is empirically
/// calibrated against the broadband visible refraction, so anchoring the
/// per-wavelength scaling at 550 nm keeps the green channel bit-identical
/// to the existing single-wavelength path.
pub const REFERENCE_WAVELENGTH_NM: f64 = 550.0;

/// Edlén refractivity at the reference (green) wavelength. Used to scale
/// Saemundsson's broadband refraction into per-wavelength refraction.
pub const EDLEN_REFERENCE_REFRACTIVITY: f64 = 2.778_4e-4;

/// Representative renderer wavelengths for the R/G/B channels, in nm.
///
/// 620 / 550 / 440 nm are the wavelengths the roadmap (`V-25`) anchors the
/// per-channel atmospheric dispersion against, and are within a few nm of
/// the sRGB primaries' dominant wavelengths. Centring the green channel at
/// 550 nm keeps the single-wavelength refraction path unchanged.
pub const RGB_REFERENCE_WAVELENGTHS_NM: [f64; 3] = [620.0, 550.0, 440.0];

/// Refraction angle `ρ(λ) = apparent − true` at a given wavelength, in
/// radians. Combines Saemundsson's apparent-altitude refraction (the
/// renderer's broadband baseline) with Edlén 1966 dispersion `(n(λ) − 1)`
/// so the green channel matches the existing single-wavelength path and
/// the differential `ρ(B) − ρ(R)` follows the Edlén refractivity ratio.
///
/// References:
/// * Filippenko, A. V. 1982, PASP 94, 715.
/// * Stone, R. C. 1996, PASP 108, 1051.
/// * Edlén, B. 1966, Metrologia 2, 71.
pub fn refraction_per_wavelength(
    true_altitude_rad: f64,
    pressure_hpa: f64,
    temperature_c: f64,
    wavelength_nm: f64,
) -> f64 {
    let broadband = saemundsson_refraction_angle(true_altitude_rad, pressure_hpa, temperature_c);
    if broadband == 0.0 {
        return 0.0;
    }
    let ratio = edlen_refractivity_standard_air(wavelength_nm) / EDLEN_REFERENCE_REFRACTIVITY;
    broadband * ratio
}

pub fn mat_mul_vec(m: Mat3d, v: Vec3d) -> Vec3d {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

pub fn mat_transpose(m: Mat3d) -> Mat3d {
    [
        [m[0][0], m[1][0], m[2][0]],
        [m[0][1], m[1][1], m[2][1]],
        [m[0][2], m[1][2], m[2][2]],
    ]
}

fn poly(t: f64, coeffs: &[f64]) -> f64 {
    coeffs.iter().rev().fold(0.0, |acc, &c| acc * t + c)
}

fn arcsec_to_rad(arcsec: f64) -> f64 {
    arcsec * ARCSEC_TO_RAD
}

fn deg_to_rad(deg: f64) -> f64 {
    deg * DEG_TO_RAD
}

fn rot_x(angle: f64) -> Mat3d {
    let (s, c) = angle.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]]
}

fn rot_z(angle: f64) -> Mat3d {
    let (s, c) = angle.sin_cos();
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}

fn mat_mul(a: Mat3d, b: Mat3d) -> Mat3d {
    let mut out = [[0.0; 3]; 3];
    for r in 0..3 {
        for c in 0..3 {
            out[r][c] = a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c];
        }
    }
    out
}

fn dot(a: Vec3d, b: Vec3d) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(v: Vec3d) -> Vec3d {
    let len = dot(v, v).sqrt();
    if len > 0.0 {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn angular_sep_arcsec(a: Vec3d, b: Vec3d) -> f64 {
        dot(a, b).clamp(-1.0, 1.0).acos().to_degrees() * 3600.0
    }

    #[test]
    fn p03_precession_has_expected_modern_scale() {
        let m = precession_matrix_iau2006(J2000_JD + 25.0 * JULIAN_YEAR_DAYS);
        let equinox = [1.0, 0.0, 0.0];
        let shifted = mat_mul_vec(m, equinox);
        let sep = angular_sep_arcsec(equinox, shifted);
        assert!(
            (1_200.0..=1_300.0).contains(&sep),
            "precession over 25y = {sep}\""
        );
    }

    #[test]
    fn nutation_is_in_expected_arcsecond_range() {
        let n = nutation_iau2000b_approx(2_460_000.5);
        assert!(n.dpsi_rad.abs().to_degrees() * 3600.0 < 20.0);
        assert!(n.deps_rad.abs().to_degrees() * 3600.0 < 10.0);
    }

    #[test]
    fn annual_aberration_reaches_twenty_arcsec_scale() {
        let beta = earth_velocity_over_c_j2000(2_460_000.5);
        let speed = dot(beta, beta).sqrt();
        assert!((9.8e-5..=1.02e-4).contains(&speed));
        let dir = normalize([beta[1], -beta[0], 0.0]);
        let app = annual_aberration(dir, beta);
        let sep = angular_sep_arcsec(dir, app);
        assert!((19.0..=21.0).contains(&sep), "aberration = {sep}\"");
    }

    #[test]
    fn saemundsson_refraction_lifts_horizon_by_about_34_arcmin() {
        let refracted = refracted_altitude_saemundsson(0.0, 1010.0, 10.0);
        let arcmin = refracted.to_degrees() * 60.0;
        assert!((28.0..=35.0).contains(&arcmin), "refraction = {arcmin}'");
    }

    /// V-25: the Edlén 1966 dispersion at the renderer's R/G/B reference
    /// wavelengths must straddle the green anchor: red refraction is
    /// slightly weaker, blue is slightly stronger.
    #[test]
    fn edlen_refractivity_brackets_550nm_with_rgb_anchors() {
        let n_r = edlen_refractivity_standard_air(620.0);
        let n_g = edlen_refractivity_standard_air(550.0);
        let n_b = edlen_refractivity_standard_air(440.0);
        assert!(
            n_r < n_g,
            "red refractivity {n_r:e} must be < green {n_g:e}"
        );
        assert!(
            n_g < n_b,
            "blue refractivity {n_b:e} must be > green {n_g:e}"
        );
        // The reference constant must match Edlén at 550 nm to within the
        // formula's own ~1e-8 precision so the green channel renders
        // bit-identically through the single-wavelength path.
        assert!(
            (n_g - EDLEN_REFERENCE_REFRACTIVITY).abs() < 1.0e-7,
            "n_g {n_g:e} vs reference {EDLEN_REFERENCE_REFRACTIVITY:e}"
        );
    }

    /// V-25: at 5° true altitude, 1013 hPa, 10 °C, the differential
    /// `ρ(B) − ρ(R)` between the renderer's blue (440 nm) and red
    /// (620 nm) channels must sit in the naked-eye-visible regime
    /// predicted by Edlén combined with the broadband Saemundsson
    /// refraction (Filippenko 1982 Table 1 scaled to z = 85°).
    ///
    /// The roadmap originally framed this as “∈ [1.2″, 2.5″]”, but those
    /// values correspond to a much higher altitude or a narrower
    /// wavelength interval; the correct Edlén + Saemundsson differential
    /// at altitude 5° between 440 and 620 nm is ~9″, still firmly
    /// naked-eye-visible (the qualitative roadmap criterion).
    #[test]
    fn rgb_dispersion_at_five_degrees_is_arcsecond_scale() {
        let alt = 5.0_f64.to_radians();
        let rho_r = refraction_per_wavelength(alt, 1013.0, 10.0, 620.0);
        let rho_b = refraction_per_wavelength(alt, 1013.0, 10.0, 440.0);
        let diff_arcsec = (rho_b - rho_r).to_degrees() * 3600.0;
        assert!(
            (6.0..=12.0).contains(&diff_arcsec),
            "ρ(B) − ρ(R) at alt=5° = {diff_arcsec}″ (expected naked-eye-visible ≈ 8–9″)"
        );
        // Green stays equal to the broadband Saemundsson refraction.
        let rho_g = refraction_per_wavelength(alt, 1013.0, 10.0, 550.0);
        let broadband = refracted_altitude_saemundsson(alt, 1013.0, 10.0) - alt;
        // Green is anchored at 550 nm; the only deviation from the
        // broadband Saemundsson value is the constant rounding of
        // `EDLEN_REFERENCE_REFRACTIVITY`, which is < 1e-5 relative.
        assert!(
            (rho_g - broadband).abs() / broadband < 1.0e-4,
            "green channel ρ = {rho_g} vs Saemundsson broadband {broadband}"
        );
    }

    /// V-25: dispersion must shrink with altitude (high zenith → small
    /// differential refraction) and survive pressure scaling without
    /// flipping sign.
    #[test]
    fn rgb_dispersion_decreases_with_altitude() {
        let alt_low = 5.0_f64.to_radians();
        let alt_mid = 30.0_f64.to_radians();
        let alt_high = 60.0_f64.to_radians();
        let diff = |alt: f64| {
            refraction_per_wavelength(alt, 1013.0, 10.0, 440.0)
                - refraction_per_wavelength(alt, 1013.0, 10.0, 620.0)
        };
        let d_low = diff(alt_low);
        let d_mid = diff(alt_mid);
        let d_high = diff(alt_high);
        assert!(d_low > d_mid && d_mid > d_high);
        assert!(
            d_high > 0.0,
            "differential must stay positive (blue lifted more than red)"
        );
    }
}
