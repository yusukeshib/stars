use std::f64::consts::TAU;

pub const J2000_JD: f64 = 2_451_545.0;
pub const UNIX_EPOCH_JD: f64 = 2_440_587.5;
pub const SECONDS_PER_DAY: f64 = 86_400.0;
const TT_MINUS_TAI_SECONDS: f64 = 32.184;

/// One row in the UTC leap-second table.
///
/// `jd_utc_effective` is the UTC Julian Date at which the listed
/// `tai_minus_utc_seconds` first applies. The table follows IERS Bulletin C / USNO
/// leap-second history; the last entry is 2017-01-01, when TAI−UTC became 37 s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeapSecond {
    pub jd_utc_effective: f64,
    pub tai_minus_utc_seconds: f64,
}

/// Built-in TAI−UTC table, valid for UTC dates from 1972-01-01 onward.
///
/// The pre-1972 UTC frequency-steering era is intentionally not modelled; for
/// earlier dates we clamp to the 1972-01-01 offset (10 s) rather than pretending
/// to know historical UT2/UTC details that are outside the renderer's scope.
pub const LEAP_SECONDS: &[LeapSecond] = &[
    LeapSecond {
        jd_utc_effective: 2_441_317.5,
        tai_minus_utc_seconds: 10.0,
    }, // 1972-01-01
    LeapSecond {
        jd_utc_effective: 2_441_499.5,
        tai_minus_utc_seconds: 11.0,
    }, // 1972-07-01
    LeapSecond {
        jd_utc_effective: 2_441_683.5,
        tai_minus_utc_seconds: 12.0,
    }, // 1973-01-01
    LeapSecond {
        jd_utc_effective: 2_442_048.5,
        tai_minus_utc_seconds: 13.0,
    }, // 1974-01-01
    LeapSecond {
        jd_utc_effective: 2_442_413.5,
        tai_minus_utc_seconds: 14.0,
    }, // 1975-01-01
    LeapSecond {
        jd_utc_effective: 2_442_778.5,
        tai_minus_utc_seconds: 15.0,
    }, // 1976-01-01
    LeapSecond {
        jd_utc_effective: 2_443_144.5,
        tai_minus_utc_seconds: 16.0,
    }, // 1977-01-01
    LeapSecond {
        jd_utc_effective: 2_443_509.5,
        tai_minus_utc_seconds: 17.0,
    }, // 1978-01-01
    LeapSecond {
        jd_utc_effective: 2_443_874.5,
        tai_minus_utc_seconds: 18.0,
    }, // 1979-01-01
    LeapSecond {
        jd_utc_effective: 2_444_239.5,
        tai_minus_utc_seconds: 19.0,
    }, // 1980-01-01
    LeapSecond {
        jd_utc_effective: 2_444_786.5,
        tai_minus_utc_seconds: 20.0,
    }, // 1981-07-01
    LeapSecond {
        jd_utc_effective: 2_445_151.5,
        tai_minus_utc_seconds: 21.0,
    }, // 1982-07-01
    LeapSecond {
        jd_utc_effective: 2_445_516.5,
        tai_minus_utc_seconds: 22.0,
    }, // 1983-07-01
    LeapSecond {
        jd_utc_effective: 2_446_247.5,
        tai_minus_utc_seconds: 23.0,
    }, // 1985-07-01
    LeapSecond {
        jd_utc_effective: 2_447_161.5,
        tai_minus_utc_seconds: 24.0,
    }, // 1988-01-01
    LeapSecond {
        jd_utc_effective: 2_447_892.5,
        tai_minus_utc_seconds: 25.0,
    }, // 1990-01-01
    LeapSecond {
        jd_utc_effective: 2_448_257.5,
        tai_minus_utc_seconds: 26.0,
    }, // 1991-01-01
    LeapSecond {
        jd_utc_effective: 2_448_804.5,
        tai_minus_utc_seconds: 27.0,
    }, // 1992-07-01
    LeapSecond {
        jd_utc_effective: 2_449_169.5,
        tai_minus_utc_seconds: 28.0,
    }, // 1993-07-01
    LeapSecond {
        jd_utc_effective: 2_449_534.5,
        tai_minus_utc_seconds: 29.0,
    }, // 1994-07-01
    LeapSecond {
        jd_utc_effective: 2_450_083.5,
        tai_minus_utc_seconds: 30.0,
    }, // 1996-01-01
    LeapSecond {
        jd_utc_effective: 2_450_630.5,
        tai_minus_utc_seconds: 31.0,
    }, // 1997-07-01
    LeapSecond {
        jd_utc_effective: 2_451_179.5,
        tai_minus_utc_seconds: 32.0,
    }, // 1999-01-01
    LeapSecond {
        jd_utc_effective: 2_453_736.5,
        tai_minus_utc_seconds: 33.0,
    }, // 2006-01-01
    LeapSecond {
        jd_utc_effective: 2_454_832.5,
        tai_minus_utc_seconds: 34.0,
    }, // 2009-01-01
    LeapSecond {
        jd_utc_effective: 2_456_109.5,
        tai_minus_utc_seconds: 35.0,
    }, // 2012-07-01
    LeapSecond {
        jd_utc_effective: 2_457_204.5,
        tai_minus_utc_seconds: 36.0,
    }, // 2015-07-01
    LeapSecond {
        jd_utc_effective: 2_457_754.5,
        tai_minus_utc_seconds: 37.0,
    }, // 2017-01-01
];

/// Julian Dates for the time scales used by the astronomy pipeline.
///
/// UTC is civil input time; UT1 drives Earth rotation / sidereal time; TAI is
/// atomic time; TT is the terrestrial dynamical scale; TDB is the barycentric
/// dynamical scale expected by solar-system ephemerides. `dut1_seconds` is the
/// externally supplied UT1−UTC correction. When no Bulletin A value is known the
/// constructors default it to zero, preserving the old UT1≈UTC behaviour while
/// keeping the approximation explicit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeScales {
    pub jd_utc: f64,
    pub jd_ut1: f64,
    pub jd_tai: f64,
    pub jd_tt: f64,
    pub jd_tdb: f64,
    pub tai_minus_utc_seconds: f64,
    pub dut1_seconds: f64,
}

impl TimeScales {
    pub fn from_unix_seconds(unix_seconds: f64) -> Self {
        Self::from_unix_seconds_with_dut1(unix_seconds, 0.0)
    }

    pub fn from_unix_seconds_with_dut1(unix_seconds: f64, dut1_seconds: f64) -> Self {
        Self::from_utc_julian_date_with_dut1(
            julian_date_from_unix_seconds(unix_seconds),
            dut1_seconds,
        )
    }

    pub fn from_utc_julian_date(jd_utc: f64) -> Self {
        Self::from_utc_julian_date_with_dut1(jd_utc, 0.0)
    }

    pub fn from_utc_julian_date_with_dut1(jd_utc: f64, dut1_seconds: f64) -> Self {
        let tai_minus_utc_seconds = tai_minus_utc_seconds_at_jd_utc(jd_utc);
        let jd_ut1 = jd_utc + dut1_seconds / SECONDS_PER_DAY;
        let jd_tai = jd_utc + tai_minus_utc_seconds / SECONDS_PER_DAY;
        let jd_tt = jd_tai + TT_MINUS_TAI_SECONDS / SECONDS_PER_DAY;
        let jd_tdb = approximate_tdb_from_tt(jd_tt);
        Self {
            jd_utc,
            jd_ut1,
            jd_tai,
            jd_tt,
            jd_tdb,
            tai_minus_utc_seconds,
            dut1_seconds,
        }
    }
}

/// Julian Date for a UTC instant given as Unix/POSIX seconds (may be fractional).
pub fn julian_date_from_unix_seconds(unix_seconds: f64) -> f64 {
    UNIX_EPOCH_JD + unix_seconds / SECONDS_PER_DAY
}

pub fn tai_minus_utc_seconds_at_jd_utc(jd_utc: f64) -> f64 {
    LEAP_SECONDS
        .iter()
        .rev()
        .find(|entry| jd_utc >= entry.jd_utc_effective)
        .or_else(|| LEAP_SECONDS.first())
        .map(|entry| entry.tai_minus_utc_seconds)
        .unwrap_or(0.0)
}

/// Low-amplitude TT→TDB approximation in days.
///
/// This conventional two-term expression is sufficient for the current VSOP87 /
/// lunar visual ephemerides: TDB−TT stays below 2 ms. The full relativistic time
/// ephemeris belongs with the later DE440 / publication-grade Phase 3 work.
pub fn approximate_tdb_from_tt(jd_tt: f64) -> f64 {
    let mean_anomaly_rad = (357.53 + 0.985_600_3 * (jd_tt - J2000_JD)).to_radians();
    jd_tt
        + (0.001_657 * mean_anomaly_rad.sin() + 0.000_022 * (2.0 * mean_anomaly_rad).sin())
            / SECONDS_PER_DAY
}

/// Greenwich Mean Sidereal Time in radians for the given Julian Date in UT1.
///
/// Uses the polynomial from the IAU 1982 model (good to a few arcseconds for our use).
pub fn gmst_radians(jd_ut1: f64) -> f64 {
    let d = jd_ut1 - J2000_JD;
    let t = d / 36_525.0;
    // GMST in seconds of time at 0h UT plus the rotation since:
    let gmst_seconds =
        67_310.548_41 + (876_600.0 * 3600.0 + 8_640_184.812_866) * t + 0.093_104 * t * t
            - 6.2e-6 * t * t * t;
    let frac = gmst_seconds.rem_euclid(SECONDS_PER_DAY) / SECONDS_PER_DAY;
    (frac * TAU).rem_euclid(TAU)
}

/// Local Mean Sidereal Time at the given longitude (east-positive radians), for UT1.
pub fn lmst_radians(jd_ut1: f64, longitude_east_rad: f64) -> f64 {
    (gmst_radians(jd_ut1) + longitude_east_rad).rem_euclid(TAU)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_julian_date() {
        assert!((julian_date_from_unix_seconds(0.0) - UNIX_EPOCH_JD).abs() < 1e-9);
    }

    #[test]
    fn j2000_julian_date() {
        let jd = julian_date_from_unix_seconds(946_728_000.0);
        assert!((jd - J2000_JD).abs() < 1e-6, "jd={jd}");
    }

    #[test]
    fn leap_second_table_offsets_are_applied() {
        assert_eq!(tai_minus_utc_seconds_at_jd_utc(2_451_545.0), 32.0);
        assert_eq!(tai_minus_utc_seconds_at_jd_utc(2_457_753.5), 36.0);
        assert_eq!(tai_minus_utc_seconds_at_jd_utc(2_457_754.5), 37.0);
    }

    #[test]
    fn time_scales_separate_utc_ut1_tai_tt_tdb() {
        let time = TimeScales::from_utc_julian_date_with_dut1(2_451_545.0, -0.355);
        assert!((time.jd_ut1 - (time.jd_utc - 0.355 / SECONDS_PER_DAY)).abs() < 1e-12);
        assert!((time.jd_tai - (time.jd_utc + 32.0 / SECONDS_PER_DAY)).abs() < 1e-12);
        assert!((time.jd_tt - (time.jd_tai + 32.184 / SECONDS_PER_DAY)).abs() < 1e-12);
        assert!(((time.jd_tdb - time.jd_tt) * SECONDS_PER_DAY).abs() < 0.002);
    }

    #[test]
    fn gmst_at_j2000_known_value() {
        let gmst = gmst_radians(J2000_JD);
        let expected = 4.894_961_212_735_793;
        assert!(
            (gmst - expected).abs() < 1e-4,
            "gmst={gmst}, expected≈{expected}"
        );
    }

    #[test]
    fn lmst_offsets_by_longitude() {
        let jd = J2000_JD;
        let lmst_g = lmst_radians(jd, 0.0);
        assert!((lmst_g - gmst_radians(jd)).abs() < 1e-12);

        let lmst_e = lmst_radians(jd, std::f64::consts::FRAC_PI_2);
        let diff = (lmst_e - lmst_g).rem_euclid(TAU);
        assert!((diff - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    }
}
