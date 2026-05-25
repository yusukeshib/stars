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
    let alt_deg = true_altitude_rad.to_degrees();
    if !alt_deg.is_finite() || !(-1.0..=89.9).contains(&alt_deg) {
        return true_altitude_rad;
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
    true_altitude_rad + (r_arcmin / 60.0).to_radians()
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
}
