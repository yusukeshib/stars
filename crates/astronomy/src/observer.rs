use crate::TimeScales;

/// Southernmost valid geodetic/astronomical observer latitude, in degrees.
const MIN_LATITUDE_DEG: f64 = -90.0;
/// Northernmost valid geodetic/astronomical observer latitude, in degrees.
const MAX_LATITUDE_DEG: f64 = 90.0;
/// Full longitude wrap, in degrees.
const FULL_TURN_DEG: f64 = 360.0;

/// Geographic observer state.
///
/// Latitude/longitude are radians. Constructors clamp latitude to the physical
/// pole-to-pole range, wrap finite longitude onto one turn, and replace
/// non-finite angles with 0° so host input cannot poison downstream trig with
/// NaNs. `time` carries the separated UTC, UT1, TAI,
/// TT, and TDB Julian Dates. The legacy `julian_date` field is kept as the UT1
/// Julian Date for existing renderer code that only needs Earth rotation; new
/// ephemeris code should use `time.jd_tdb`/`time.jd_tt` explicitly.
#[derive(Debug, Clone, Copy)]
pub struct Observer {
    pub latitude_rad: f64,
    pub longitude_rad: f64,
    pub julian_date: f64,
    pub time: TimeScales,
}

impl Observer {
    pub fn from_degrees(lat_deg: f64, lng_deg: f64, jd_utc: f64) -> Self {
        Self::from_degrees_with_time(lat_deg, lng_deg, TimeScales::from_utc_julian_date(jd_utc))
    }

    pub fn from_degrees_with_time(lat_deg: f64, lng_deg: f64, time: TimeScales) -> Self {
        let lat_deg = sanitize_latitude_deg(lat_deg);
        let lng_deg = sanitize_longitude_deg(lng_deg);
        Self {
            latitude_rad: lat_deg.to_radians(),
            longitude_rad: lng_deg.to_radians(),
            julian_date: time.jd_ut1,
            time,
        }
    }
}

fn sanitize_latitude_deg(lat_deg: f64) -> f64 {
    if lat_deg.is_finite() {
        lat_deg.clamp(MIN_LATITUDE_DEG, MAX_LATITUDE_DEG)
    } else {
        0.0
    }
}

fn sanitize_longitude_deg(lng_deg: f64) -> f64 {
    if lng_deg.is_finite() {
        lng_deg.rem_euclid(FULL_TURN_DEG)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::J2000_JD;

    #[test]
    fn constructor_clamps_latitude_to_physical_domain() {
        let north = Observer::from_degrees(120.0, 0.0, J2000_JD);
        let south = Observer::from_degrees(-120.0, 0.0, J2000_JD);
        assert_eq!(north.latitude_rad, MAX_LATITUDE_DEG.to_radians());
        assert_eq!(south.latitude_rad, MIN_LATITUDE_DEG.to_radians());
    }

    #[test]
    fn constructor_wraps_longitude_and_rejects_non_finite_angles() {
        let wrapped = Observer::from_degrees(0.0, -10.0, J2000_JD);
        assert!((wrapped.longitude_rad - 350_f64.to_radians()).abs() < 1e-12);

        let sanitized = Observer::from_degrees(f64::NAN, f64::INFINITY, J2000_JD);
        assert_eq!(sanitized.latitude_rad, 0.0);
        assert_eq!(sanitized.longitude_rad, 0.0);
    }
}
