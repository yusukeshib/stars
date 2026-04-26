use std::f64::consts::TAU;

const J2000_JD: f64 = 2_451_545.0;
const UNIX_EPOCH_JD: f64 = 2_440_587.5;

/// Julian Date for a UTC instant given as Unix seconds (may be fractional).
pub fn julian_date_from_unix_seconds(unix_seconds: f64) -> f64 {
    UNIX_EPOCH_JD + unix_seconds / 86_400.0
}

/// Greenwich Mean Sidereal Time in radians for the given Julian Date (UT1 ≈ UTC).
///
/// Uses the polynomial from the IAU 1982 model (good to a few arcseconds for our use).
pub fn gmst_radians(jd_ut: f64) -> f64 {
    let d = jd_ut - J2000_JD;
    let t = d / 36_525.0;
    // GMST in seconds of time at 0h UT plus the rotation since:
    let gmst_seconds =
        67_310.548_41 + (876_600.0 * 3600.0 + 8_640_184.812_866) * t + 0.093_104 * t * t
            - 6.2e-6 * t * t * t;
    let seconds_in_day = 86_400.0;
    let frac = (gmst_seconds.rem_euclid(seconds_in_day)) / seconds_in_day;
    wrap_tau(frac * TAU)
}

/// Local Mean Sidereal Time at the given longitude (east-positive radians).
pub fn lmst_radians(jd_ut: f64, longitude_east_rad: f64) -> f64 {
    wrap_tau(gmst_radians(jd_ut) + longitude_east_rad)
}

fn wrap_tau(x: f64) -> f64 {
    let mut v = x.rem_euclid(TAU);
    if v < 0.0 {
        v += TAU;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_julian_date() {
        // 1970-01-01T00:00:00Z = JD 2440587.5
        assert!((julian_date_from_unix_seconds(0.0) - UNIX_EPOCH_JD).abs() < 1e-9);
    }

    #[test]
    fn j2000_julian_date() {
        // 2000-01-01T12:00:00Z = JD 2451545.0; unix seconds = 946728000
        let jd = julian_date_from_unix_seconds(946_728_000.0);
        assert!((jd - J2000_JD).abs() < 1e-6, "jd={jd}");
    }

    #[test]
    fn gmst_at_j2000_known_value() {
        // At J2000.0, GMST ≈ 18h 41m 50.5s ≈ 4.894961 rad.
        let gmst = gmst_radians(J2000_JD);
        let expected = 4.894_961_212_735_793;
        assert!(
            (gmst - expected).abs() < 1e-4,
            "gmst={gmst}, expected≈{expected}"
        );
    }

    #[test]
    fn lmst_offsets_by_longitude() {
        // At Greenwich, LMST == GMST.
        let jd = J2000_JD;
        let lmst_g = lmst_radians(jd, 0.0);
        assert!((lmst_g - gmst_radians(jd)).abs() < 1e-12);

        // At longitude +90° east, LMST = GMST + π/2.
        let lmst_e = lmst_radians(jd, std::f64::consts::FRAC_PI_2);
        let diff = (lmst_e - lmst_g).rem_euclid(TAU);
        assert!((diff - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }
}
