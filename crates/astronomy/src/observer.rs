use crate::TimeScales;

/// Geographic observer state.
///
/// Latitude/longitude are radians. `time` carries the separated UTC, UT1, TAI,
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
        Self {
            latitude_rad: lat_deg.to_radians(),
            longitude_rad: lng_deg.to_radians(),
            julian_date: time.jd_ut1,
            time,
        }
    }
}
