/// Geographic observer state. Latitude/longitude in radians, Julian Date in UT.
#[derive(Debug, Clone, Copy)]
pub struct Observer {
    pub latitude_rad: f64,
    pub longitude_rad: f64,
    pub julian_date: f64,
}

impl Observer {
    pub fn from_degrees(lat_deg: f64, lng_deg: f64, julian_date: f64) -> Self {
        Self {
            latitude_rad: lat_deg.to_radians(),
            longitude_rad: lng_deg.to_radians(),
            julian_date,
        }
    }
}
