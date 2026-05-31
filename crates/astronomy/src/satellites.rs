//! V-55 artificial-satellite layer: TLE parsing, SGP4 propagation, and
//! topocentric apparent state with Earth-shadow visibility and amateur-grade
//! apparent magnitude.
//!
//! # Pipeline
//!
//! 1. **Propagation.** Two-line elements are parsed and propagated by the
//!    `sgp4` crate (a pure-Rust port of the Vallado et al. 2006 / Spacetrack
//!    Report #3 reference SGP4/SDP4 model), yielding a geocentric **TEME**
//!    (True Equator, Mean Equinox) position in kilometres at the requested
//!    epoch.
//! 2. **Topocentric reduction.** The renderer's quasi-inertial equatorial
//!    frame is related to Earth-fixed coordinates by the same GMST rotation
//!    used by [`crate::ephemeris::observer_equatorial_position_km`], so the
//!    TEME position and the WGS84 observer position share one frame to
//!    sub-arcminute (precession/nutation between TEME and the catalog J2000
//!    equinox is ignored — negligible for naked-eye satellite tracks). The
//!    topocentric vector is `r_sat − r_obs`; its right ascension / declination
//!    and the observer's local sidereal time give altitude / azimuth.
//! 3. **Visibility.** A satellite is naked-eye-visible only when sunlit. The
//!    Earth-shadow test uses a conical umbra / penumbra built from the apparent
//!    Sun direction and distance (shares the umbra-cone idea with `V-36`).
//! 4. **Magnitude.** Apparent magnitude follows the McCants / QuickSat
//!    "standard magnitude" convention: the intrinsic magnitude is the visual
//!    magnitude at 1000 km range and 50 % illuminated phase, scaled by range
//!    and a Lambertian-like phase fraction.
//!
//! # References
//! - Vallado, D. A. et al. 2006, AIAA 2006-6753, *Revisiting Spacetrack
//!   Report #3*.
//! - Hoots, F. R., Roehrich, R. L. 1980, Spacetrack Report #3.
//! - CelesTrak (celestrak.org) — current TLE feeds.
//! - McCants, M. (mmccants.org) — satellite intrinsic-magnitude convention.

use std::fmt;

use crate::ephemeris::{
    equatorial_unit_vector_f64, observer_equatorial_position_km, ra_dec_from_equatorial_vector,
};
use crate::time::TimeScales;
use crate::{apparent_sun, equatorial_to_horizontal, lmst_radians, AltAz, Observer, J2000_JD};

/// WGS84 mean Earth radius in kilometres, used for the shadow cone.
const EARTH_RADIUS_KM: f64 = 6_378.137;
/// Nominal solar radius in kilometres, IAU 2015 Resolution B3.
const SOLAR_RADIUS_KM: f64 = 695_700.0;
/// Astronomical Unit in kilometres, IAU 2012 Resolution B2.
const ASTRONOMICAL_UNIT_KM: f64 = 149_597_870.7;
/// Days in a Julian year, the unit returned by `sgp4::Elements::epoch`.
const JULIAN_YEAR_DAYS: f64 = 365.25;
/// SI seconds per day, for finite-difference angular velocity.
const SECONDS_PER_DAY: f64 = 86_400.0;
/// Reference range (km) at which the McCants standard magnitude is defined.
const STD_MAGNITUDE_RANGE_KM: f64 = 1_000.0;
/// Photometric offset so `apparent_magnitude` equals the standard magnitude at
/// 1000 km range and 50 % illuminated phase: `2.5·log10(1000² / 0.5) ≈ 15.75`.
const STD_MAGNITUDE_OFFSET: f64 = 15.75;
/// Illumination fraction below which a satellite counts as eclipsed (not
/// naked-eye-visible). Above this it has emerged from the deep penumbra.
const SUNLIT_THRESHOLD: f64 = 0.15;

/// Intrinsic magnitude assigned to a satellite with no curated entry. A
/// mid-range value so unknown objects render at a plausible faint brightness
/// rather than vanishing or saturating.
pub const DEFAULT_STD_MAGNITUDE: f64 = 8.0;

/// Hand-curated intrinsic ("standard") visual magnitudes for the satellites in
/// the shipped snapshot, keyed by NORAD catalog id. The standard magnitude is
/// the visual magnitude at 1000 km range and 50 % illuminated phase, following
/// the McCants / QuickSat convention (mmccants.org). Values are amateur-grade
/// representative magnitudes; this is deliberately a small hand table rather
/// than a bulk import of McCants' MCNAMES file (see DATA_SOURCES.md).
pub const CURATED_STD_MAGNITUDES: &[(u64, f64)] = &[
    (25544, -1.8), // ISS (ZARYA) — brightest regular naked-eye satellite
    (20580, 2.0),  // HST (Hubble Space Telescope)
    (43013, 6.0),  // NOAA 20 (JPSS-1), polar LEO
    (44714, 5.5),  // STARLINK-1008
    (41866, 11.0), // GOES 16 — geostationary, faint
];

/// Look up the curated standard magnitude for `norad_id`, falling back to
/// [`DEFAULT_STD_MAGNITUDE`].
pub fn std_magnitude_for(norad_id: u64) -> f64 {
    CURATED_STD_MAGNITUDES
        .iter()
        .find(|(id, _)| *id == norad_id)
        .map(|(_, m)| *m)
        .unwrap_or(DEFAULT_STD_MAGNITUDE)
}

/// A parsed two-line element set plus its intrinsic magnitude.
#[derive(Debug, Clone)]
pub struct Tle {
    pub name: String,
    pub line1: String,
    pub line2: String,
    /// Intrinsic ("standard") visual magnitude — see [`CURATED_STD_MAGNITUDES`].
    pub std_magnitude: f64,
}

/// Errors from constructing a [`Satellite`] from a TLE.
#[derive(Debug)]
pub enum SatelliteError {
    Parse(String),
    Propagate(String),
}

impl fmt::Display for SatelliteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SatelliteError::Parse(m) => write!(f, "TLE parse error: {m}"),
            SatelliteError::Propagate(m) => write!(f, "SGP4 propagation error: {m}"),
        }
    }
}

impl std::error::Error for SatelliteError {}

/// Parse a CelesTrak-style TLE text blob into [`Tle`] records.
///
/// Lines beginning with `#` and blank lines are ignored. Each satellite is an
/// optional name line followed by line 1 (`1 …`) and line 2 (`2 …`). The
/// intrinsic magnitude is resolved from [`CURATED_STD_MAGNITUDES`] using the
/// NORAD id parsed from line 1.
pub fn parse_tle_set(text: &str) -> Vec<Tle> {
    let mut out = Vec::new();
    let mut pending_name: Option<String> = None;
    let mut line1: Option<String> = None;

    for raw in text.lines() {
        let line = raw.trim_end();
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("1 ") {
            // Reconstruct the full line so checksum-bearing columns survive.
            let _ = rest;
            line1 = Some(line.to_string());
        } else if trimmed.starts_with("2 ") {
            if let Some(l1) = line1.take() {
                let l2 = line.to_string();
                let norad_id = parse_norad_id(&l1).unwrap_or(0);
                let name = pending_name
                    .take()
                    .unwrap_or_else(|| format!("NORAD {norad_id}"));
                out.push(Tle {
                    name,
                    line1: l1,
                    line2: l2,
                    std_magnitude: std_magnitude_for(norad_id),
                });
            }
            pending_name = None;
        } else {
            // A name line precedes the next "1 …" block.
            pending_name = Some(trimmed.to_string());
            line1 = None;
        }
    }
    out
}

/// Parse the NORAD catalog id from columns 3–7 of a TLE line.
fn parse_norad_id(tle_line: &str) -> Option<u64> {
    let bytes = tle_line.as_bytes();
    if bytes.len() < 7 {
        return None;
    }
    tle_line[2..7].trim().parse::<u64>().ok()
}

/// Internal reduction result: `(alt/az, range_km, ra, dec, teme_km, obs_km)`.
type Topocentric = (AltAz, f64, f64, f64, [f64; 3], [f64; 3]);

/// An SGP4 propagator bound to one satellite's elements.
pub struct Satellite {
    pub name: String,
    pub norad_id: u64,
    pub std_magnitude: f64,
    epoch_jd_utc: f64,
    constants: sgp4::Constants,
}

/// Apparent topocentric state of a satellite for an observer at one instant.
#[derive(Debug, Clone)]
pub struct SatelliteApparent {
    pub name: String,
    pub norad_id: u64,
    /// Topocentric right ascension (radians).
    pub right_ascension_rad: f64,
    /// Topocentric declination (radians).
    pub declination_rad: f64,
    /// Apparent altitude above the geometric horizon (radians).
    pub altitude_rad: f64,
    /// Apparent azimuth, measured from North toward East (radians).
    pub azimuth_rad: f64,
    /// Slant range from observer to satellite (kilometres).
    pub range_km: f64,
    /// True when the satellite is above the geometric horizon.
    pub above_horizon: bool,
    /// Fraction of sunlight reaching the satellite: 1 = full sun, 0 = deep
    /// umbra, linearly ramped across the penumbra.
    pub illumination: f64,
    /// True when the satellite is sunlit enough to be naked-eye-visible
    /// ([`SUNLIT_THRESHOLD`]).
    pub sunlit: bool,
    /// Apparent visual magnitude (McCants standard-magnitude convention),
    /// `f64::INFINITY` when eclipsed.
    pub apparent_magnitude: f64,
    /// Apparent angular rate across the sky (radians per second), for streak
    /// rendering when frame integration is enabled.
    pub angular_velocity_rad_per_s: f64,
    /// Geocentric TEME position (kilometres).
    pub eci_position_km: [f64; 3],
}

impl Satellite {
    /// Build a propagator from a parsed TLE.
    pub fn from_tle(tle: &Tle) -> Result<Self, SatelliteError> {
        let elements = sgp4::Elements::from_tle(None, tle.line1.as_bytes(), tle.line2.as_bytes())
            .map_err(|e| SatelliteError::Parse(e.to_string()))?;
        let constants = sgp4::Constants::from_elements(&elements)
            .map_err(|e| SatelliteError::Propagate(e.to_string()))?;
        // `epoch()` is Julian years since J2000 (UTC); recover the UTC Julian
        // Date so we can express the requested instant as minutes-since-epoch.
        let epoch_jd_utc = J2000_JD + elements.epoch() * JULIAN_YEAR_DAYS;
        Ok(Self {
            name: tle.name.clone(),
            norad_id: elements.norad_id,
            std_magnitude: tle.std_magnitude,
            epoch_jd_utc,
            constants,
        })
    }

    /// Geocentric TEME position (kilometres) at the given UTC Julian Date, or
    /// `None` if the propagator reports a decay/error.
    pub fn teme_position_km(&self, jd_utc: f64) -> Option<[f64; 3]> {
        let minutes = (jd_utc - self.epoch_jd_utc) * (SECONDS_PER_DAY / 60.0);
        self.constants
            .propagate(sgp4::MinutesSinceEpoch(minutes))
            .ok()
            .map(|p| p.position)
    }

    /// Compute the apparent topocentric state for `observer`, or `None` if the
    /// SGP4 propagation fails (e.g. a decayed orbit).
    pub fn apparent(&self, observer: Observer) -> Option<SatelliteApparent> {
        let (altaz, range_km, ra, dec, teme, obs_eq) = self.topocentric(observer)?;

        // Finite-difference angular velocity over one second of motion.
        let dt_days = 1.0 / SECONDS_PER_DAY;
        let obs2 = advance_observer(observer, dt_days);
        let angular_velocity_rad_per_s = match self.topocentric(obs2) {
            Some((altaz2, _, _, _, _, _)) => angular_separation(altaz, altaz2),
            None => 0.0,
        };

        // Earth-shadow visibility from the apparent Sun.
        let sun = apparent_sun(observer.time.jd_tt);
        let sun_unit = equatorial_unit_vector_f64(sun.right_ascension_rad, sun.declination_rad);
        let sun_distance_km = sun.distance_au * ASTRONOMICAL_UNIT_KM;
        let illumination = earth_shadow_illumination(teme, sun_unit, sun_distance_km);
        let sunlit = illumination > SUNLIT_THRESHOLD;

        // Phase angle (Sun–satellite–observer) for the magnitude phase term.
        let sun_geo_km = [
            sun_unit[0] * sun_distance_km,
            sun_unit[1] * sun_distance_km,
            sun_unit[2] * sun_distance_km,
        ];
        let to_sun = sub(sun_geo_km, teme);
        let to_obs = sub(obs_eq, teme);
        let phase_angle = angle_between(to_sun, to_obs);
        let apparent_magnitude = if sunlit {
            apparent_magnitude(self.std_magnitude, range_km, phase_angle)
        } else {
            f64::INFINITY
        };

        Some(SatelliteApparent {
            name: self.name.clone(),
            norad_id: self.norad_id,
            right_ascension_rad: ra,
            declination_rad: dec,
            altitude_rad: altaz.altitude,
            azimuth_rad: altaz.azimuth,
            range_km,
            above_horizon: altaz.altitude > 0.0,
            illumination,
            sunlit,
            apparent_magnitude,
            angular_velocity_rad_per_s,
            eci_position_km: teme,
        })
    }

    /// Shared reduction returning `(alt/az, range, ra, dec, teme, observer)`.
    fn topocentric(&self, observer: Observer) -> Option<Topocentric> {
        let teme = self.teme_position_km(observer.time.jd_utc)?;
        let obs_eq = observer_equatorial_position_km(observer);
        let topo = sub(teme, obs_eq);
        let (ra, dec, range_km) = ra_dec_from_equatorial_vector(topo);
        let lst = lmst_radians(observer.time.jd_ut1, observer.longitude_rad);
        let altaz = equatorial_to_horizontal(ra, dec, lst, observer.latitude_rad);
        Some((altaz, range_km, ra, dec, teme, obs_eq))
    }
}

/// Build apparent states for every TLE in `tles`, skipping any that fail to
/// parse or propagate.
pub fn apparent_satellites(tles: &[Tle], observer: Observer) -> Vec<SatelliteApparent> {
    tles.iter()
        .filter_map(|tle| Satellite::from_tle(tle).ok())
        .filter_map(|sat| sat.apparent(observer))
        .collect()
}

/// Fraction of sunlight reaching `sat_geo_km` given the unit Earth→Sun
/// direction `sun_unit` and the Earth–Sun distance. Returns 1 in full sun, 0
/// in the umbra, and a linear ramp across the penumbra. Uses a conical shadow:
/// the umbra tapers behind Earth, the penumbra widens.
fn earth_shadow_illumination(
    sat_geo_km: [f64; 3],
    sun_unit: [f64; 3],
    sun_distance_km: f64,
) -> f64 {
    let d_par = dot(sat_geo_km, sun_unit);
    if d_par >= 0.0 {
        // Satellite is on the sunward side of Earth — always fully lit.
        return 1.0;
    }
    let r2 = dot(sat_geo_km, sat_geo_km);
    let perp = (r2 - d_par * d_par).max(0.0).sqrt();
    let x = -d_par; // distance behind Earth along the anti-solar axis
    let tan_umbra = (SOLAR_RADIUS_KM - EARTH_RADIUS_KM) / sun_distance_km;
    let tan_penumbra = (SOLAR_RADIUS_KM + EARTH_RADIUS_KM) / sun_distance_km;
    let r_umbra = EARTH_RADIUS_KM - x * tan_umbra;
    let r_penumbra = EARTH_RADIUS_KM + x * tan_penumbra;
    if perp <= r_umbra {
        0.0
    } else if perp >= r_penumbra {
        1.0
    } else {
        ((perp - r_umbra) / (r_penumbra - r_umbra)).clamp(0.0, 1.0)
    }
}

/// McCants / QuickSat apparent magnitude: standard magnitude scaled by range
/// and a Lambertian-like phase fraction `(1 + cos φ) / 2`.
fn apparent_magnitude(std_magnitude: f64, range_km: f64, phase_angle_rad: f64) -> f64 {
    let frac_illuminated = ((1.0 + phase_angle_rad.cos()) / 2.0).max(1.0e-3);
    let _ = STD_MAGNITUDE_RANGE_KM;
    std_magnitude - STD_MAGNITUDE_OFFSET + 2.5 * (range_km * range_km / frac_illuminated).log10()
}

/// Return a copy of `observer` advanced by `dt_days` (recomputing time scales).
fn advance_observer(observer: Observer, dt_days: f64) -> Observer {
    let time = TimeScales::from_utc_julian_date_with_dut1(
        observer.time.jd_utc + dt_days,
        observer.time.dut1_seconds,
    );
    Observer {
        latitude_rad: observer.latitude_rad,
        longitude_rad: observer.longitude_rad,
        julian_date: time.jd_ut1,
        time,
    }
}

fn angular_separation(a: AltAz, b: AltAz) -> f64 {
    let cos_sep = a.altitude.sin() * b.altitude.sin()
        + a.altitude.cos() * b.altitude.cos() * (a.azimuth - b.azimuth).cos();
    cos_sep.clamp(-1.0, 1.0).acos()
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn angle_between(a: [f64; 3], b: [f64; 3]) -> f64 {
    let na = dot(a, a).sqrt();
    let nb = dot(b, b).sqrt();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    (dot(a, b) / (na * nb)).clamp(-1.0, 1.0).acos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TimeScales;

    // Classic Spacetrack Report #3 / AIAA 2006-6753 near-Earth verification
    // object (catalog 88888).
    const TEST_L1: &str = "1 88888U          80275.98708465  .00073094  13844-3  66816-4 0    87";
    const TEST_L2: &str = "2 88888  72.8435 115.9689 0086731  52.6988 110.5714 16.05824518  1058";

    fn test_satellite() -> Satellite {
        let tle = Tle {
            name: "TEST 88888".to_string(),
            line1: TEST_L1.to_string(),
            line2: TEST_L2.to_string(),
            std_magnitude: 4.0,
        };
        Satellite::from_tle(&tle).expect("parse test TLE")
    }

    #[test]
    fn sgp4_matches_aiaa_reference_within_sub_km() {
        // AIAA 2006-6753 verification output at epoch (t = 0 min), WGS-72:
        // x = 2328.97048951, y = -5995.22076416, z = 1719.97067261 km.
        let sat = test_satellite();
        let pos = sat.teme_position_km(sat.epoch_jd_utc).unwrap();
        let reference = [2328.97048951, -5995.22076416, 1719.97067261];
        let err = ((pos[0] - reference[0]).powi(2)
            + (pos[1] - reference[1]).powi(2)
            + (pos[2] - reference[2]).powi(2))
        .sqrt();
        // The sgp4 crate uses WGS-84 gravity constants by default; the residual
        // vs. the WGS-72 reference vector is a few tens of metres — well inside
        // the documented sub-km tolerance.
        assert!(err < 0.5, "position error {err} km exceeds 0.5 km");
    }

    #[test]
    fn norad_id_parsed_from_tle() {
        let sat = test_satellite();
        assert_eq!(sat.norad_id, 88888);
    }

    #[test]
    fn curated_std_magnitudes_resolve() {
        assert_eq!(std_magnitude_for(25544), -1.8);
        assert_eq!(std_magnitude_for(999_999), DEFAULT_STD_MAGNITUDE);
    }

    #[test]
    fn parse_tle_set_groups_three_line_blocks() {
        let text = "# comment\nISS (ZARYA)\n1 25544U 98067A   26150.51748228  .00011776  00000+0  21767-3 0  9998\n2 25544  51.6337  27.5746 0007245 114.7080 245.4664 15.49496548569014\n";
        let set = parse_tle_set(text);
        assert_eq!(set.len(), 1);
        assert_eq!(set[0].name, "ISS (ZARYA)");
        assert_eq!(set[0].std_magnitude, -1.8);
        assert!(set[0].line1.starts_with("1 25544"));
        assert!(set[0].line2.starts_with("2 25544"));
    }

    #[test]
    fn shadow_fully_lit_on_sunward_side() {
        // Satellite directly toward the Sun: always lit.
        let sun = [1.0, 0.0, 0.0];
        let sat = [7000.0, 0.0, 0.0];
        let f = earth_shadow_illumination(sat, sun, ASTRONOMICAL_UNIT_KM);
        assert_eq!(f, 1.0);
    }

    #[test]
    fn shadow_umbra_on_anti_solar_axis() {
        // Satellite directly behind Earth on the anti-solar axis, low altitude:
        // inside the umbra → zero illumination.
        let sun = [1.0, 0.0, 0.0];
        let sat = [-7000.0, 0.0, 0.0];
        let f = earth_shadow_illumination(sat, sun, ASTRONOMICAL_UNIT_KM);
        assert_eq!(f, 0.0);
    }

    #[test]
    fn shadow_lit_when_far_off_axis_behind_earth() {
        // Behind Earth but far from the anti-solar axis (well outside the
        // penumbra) → fully lit.
        let sun = [1.0, 0.0, 0.0];
        let sat = [-7000.0, 20000.0, 0.0];
        let f = earth_shadow_illumination(sat, sun, ASTRONOMICAL_UNIT_KM);
        assert_eq!(f, 1.0);
    }

    #[test]
    fn magnitude_equals_standard_at_reference_geometry() {
        // 1000 km range, phase 90° (half illuminated) → apparent == standard.
        let m = apparent_magnitude(4.0, 1000.0, std::f64::consts::FRAC_PI_2);
        assert!((m - 4.0).abs() < 0.02, "m = {m}");
    }

    #[test]
    fn magnitude_dims_with_range() {
        let near = apparent_magnitude(4.0, 1000.0, 0.0);
        let far = apparent_magnitude(4.0, 4000.0, 0.0);
        assert!(far > near, "farther satellite must be fainter");
    }

    #[test]
    fn iss_apparent_state_is_finite_and_consistent() {
        let sat = test_satellite();
        // Observe at the TLE epoch from Tokyo.
        let observer = Observer::from_degrees_with_time(
            35.68,
            139.69,
            TimeScales::from_utc_julian_date(sat.epoch_jd_utc),
        );
        let app = sat.apparent(observer).expect("apparent state");
        assert!(app.range_km > 0.0 && app.range_km.is_finite());
        assert!(app.altitude_rad >= -std::f64::consts::FRAC_PI_2);
        assert!(app.altitude_rad <= std::f64::consts::FRAC_PI_2);
        assert!(app.angular_velocity_rad_per_s >= 0.0);
        assert!((0.0..=1.0).contains(&app.illumination));
    }
}
