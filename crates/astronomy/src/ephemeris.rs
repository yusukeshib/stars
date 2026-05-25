//! Low-precision solar-system ephemerides used by visual rendering.
//!
//! Phase 2 will eventually replace / extend this with VSOP87 (Sun) and
//! ELP2000 (Moon). This module starts the pipeline with a compact,
//! well-documented apparent-Sun model accurate enough to drive daylight,
//! sunset, and twilight sky-colour rendering.

use glam::Vec3;

use crate::{lmst_radians, Observer};

const J2000_JD: f64 = 2_451_545.0;
const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;
const ARCSEC_TO_RAD: f64 = DEG_TO_RAD / 3600.0;
const ASTRONOMICAL_UNIT_KM: f64 = 149_597_870.7;
const EARTH_EQUATORIAL_RADIUS_KM: f64 = 6_378.14;
const EARTH_FLATTENING: f64 = 1.0 / 298.257_223_563;
const SOLAR_RADIUS_KM: f64 = 695_700.0;
const LUNAR_RADIUS_KM: f64 = 1_737.4;

/// Apparent geocentric Sun state for rendering.
#[derive(Debug, Clone, Copy)]
pub struct SunApparent {
    /// Apparent right ascension in radians, in the low-precision equatorial
    /// frame used by the Phase-2-start renderer (date-of-observation terms,
    /// without the final IAU precession/nutation stack).
    pub right_ascension_rad: f64,
    /// Apparent declination in radians, in the same low-precision equatorial
    /// frame as [`Self::right_ascension_rad`].
    pub declination_rad: f64,
    /// Apparent ecliptic longitude in radians.
    pub ecliptic_longitude_rad: f64,
    /// Earth-Sun distance in astronomical units.
    pub distance_au: f64,
    /// Apparent angular radius of the solar disk in radians.
    pub angular_radius_rad: f64,
}

impl SunApparent {
    /// Unit vector from Earth toward the apparent Sun in equatorial coordinates.
    pub fn direction_equatorial(self) -> Vec3 {
        equatorial_unit_vector(self.right_ascension_rad, self.declination_rad)
    }
}

/// Apparent geocentric Moon state for rendering.
#[derive(Debug, Clone, Copy)]
pub struct MoonApparent {
    /// Apparent right ascension in radians, in the low-precision equatorial
    /// frame used by the Phase-2-start renderer (date-of-observation terms,
    /// without the final IAU precession/nutation stack).
    pub right_ascension_rad: f64,
    /// Apparent declination in radians, in the same low-precision equatorial
    /// frame as [`Self::right_ascension_rad`].
    pub declination_rad: f64,
    /// Approximate geocentric distance in kilometres.
    pub distance_km: f64,
    /// Apparent angular radius of the lunar disk in radians.
    pub angular_radius_rad: f64,
    /// Illuminated fraction, 0 = new Moon, 1 = full Moon.
    pub illuminated_fraction: f64,
}

impl MoonApparent {
    /// Unit vector from Earth toward the apparent Moon in equatorial coordinates.
    pub fn direction_equatorial(self) -> Vec3 {
        equatorial_unit_vector(self.right_ascension_rad, self.declination_rad)
    }
}

fn equatorial_unit_vector(right_ascension_rad: f64, declination_rad: f64) -> Vec3 {
    let [x, y, z] = equatorial_unit_vector_f64(right_ascension_rad, declination_rad);
    Vec3::new(x as f32, y as f32, z as f32)
}

fn equatorial_unit_vector_f64(right_ascension_rad: f64, declination_rad: f64) -> [f64; 3] {
    let (sin_ra, cos_ra) = right_ascension_rad.sin_cos();
    let (sin_dec, cos_dec) = declination_rad.sin_cos();
    [cos_dec * cos_ra, cos_dec * sin_ra, sin_dec]
}

fn ra_dec_from_equatorial_vector(v: [f64; 3]) -> (f64, f64, f64) {
    let distance = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let right_ascension_rad = v[1].atan2(v[0]).rem_euclid(std::f64::consts::TAU);
    let declination_rad = (v[2] / distance).clamp(-1.0, 1.0).asin();
    (right_ascension_rad, declination_rad, distance)
}

fn observer_equatorial_position_km(observer: Observer) -> [f64; 3] {
    let lst = lmst_radians(observer.julian_date, observer.longitude_rad);
    let (sin_lat, cos_lat) = observer.latitude_rad.sin_cos();
    let (sin_lst, cos_lst) = lst.sin_cos();
    // Interpret `Observer::latitude_rad` as geodetic latitude and place the
    // observer on the WGS84 ellipsoid at sea level. This matters for lunar
    // parallax: a spherical Earth with geodetic latitude used as geocentric
    // latitude is off by up to ≈11.5 arcmin at mid-latitudes.
    let e2 = EARTH_FLATTENING * (2.0 - EARTH_FLATTENING);
    let prime_vertical_radius = EARTH_EQUATORIAL_RADIUS_KM / (1.0 - e2 * sin_lat * sin_lat).sqrt();
    let rho_equatorial = prime_vertical_radius * cos_lat;
    let rho_polar = prime_vertical_radius * (1.0 - e2) * sin_lat;
    [
        rho_equatorial * cos_lst,
        rho_equatorial * sin_lst,
        rho_polar,
    ]
}

/// Apparent geocentric position of the Sun for a Julian Date.
///
/// This is the compact solar-position algorithm from Jean Meeus,
/// *Astronomical Algorithms*, 2nd ed., ch. 25: geometric mean longitude and
/// anomaly, equation of centre, aberration/nutation correction for apparent
/// longitude, and true obliquity. It is not a VSOP87 replacement, but is more
/// than adequate for the renderer's first sunlit-atmosphere pass: the sky
/// colour changes smoothly with the Sun's altitude and azimuth, and the disk
/// lands within naked-eye visual tolerances.
///
/// Time scale note: the formula is formally in TT. The current project-wide
/// convention is JD(UT1≈UTC); the resulting < 1 minute TT-UTC error moves the
/// Sun by < 0.05 arcmin, far below the precision needed for Phase-2-start
/// visual scattering. The future time-systems roadmap item will thread TT/TDB
/// explicitly.
pub fn apparent_sun(julian_date: f64) -> SunApparent {
    let t = (julian_date - J2000_JD) / 36_525.0;

    // Mean longitude and anomaly of the Sun, degrees.
    let l0 = (280.466_46 + 36_000.769_83 * t + 0.000_303_2 * t * t).rem_euclid(360.0);
    let m = (357.529_11 + 35_999.050_29 * t - 0.000_153_7 * t * t).rem_euclid(360.0);
    let m_rad = m * DEG_TO_RAD;

    // Equation of centre, true longitude, and Earth-Sun distance.
    let c = (1.914602 - 0.004817 * t - 0.000014 * t * t) * m_rad.sin()
        + (0.019993 - 0.000101 * t) * (2.0 * m_rad).sin()
        + 0.000289 * (3.0 * m_rad).sin();
    let true_long = l0 + c;
    let true_anom = m + c;
    let true_anom_rad = true_anom * DEG_TO_RAD;
    let e = 0.016708634 - 0.000042037 * t - 0.0000001267 * t * t;
    let distance_au = (1.000001018 * (1.0 - e * e)) / (1.0 + e * true_anom_rad.cos());

    // Apparent longitude and true obliquity.
    let omega = (125.04 - 1934.136 * t) * DEG_TO_RAD;
    let lambda = (true_long - 0.00569 - 0.00478 * omega.sin()).rem_euclid(360.0) * DEG_TO_RAD;
    let mean_obliquity_arcsec = 21.448 - t * (46.8150 + t * (0.00059 - t * 0.001813));
    let epsilon0 = 23.0 + (26.0 + mean_obliquity_arcsec / 60.0) / 60.0;
    let epsilon = (epsilon0 + 0.00256 * omega.cos()) * DEG_TO_RAD;

    let right_ascension_rad = (epsilon.cos() * lambda.sin())
        .atan2(lambda.cos())
        .rem_euclid(std::f64::consts::TAU);
    let declination_rad = (epsilon.sin() * lambda.sin()).asin();

    // Mean solar semidiameter at 1 AU is 959.63 arcsec (Meeus ch. 25).
    let angular_radius_rad = 959.63 * ARCSEC_TO_RAD / distance_au;

    SunApparent {
        right_ascension_rad,
        declination_rad,
        ecliptic_longitude_rad: lambda,
        distance_au,
        angular_radius_rad,
    }
}

/// Apparent topocentric Sun state for an observer on Earth.
///
/// The solar parallax is only ≈8.8 arcsec, but applying it here keeps the
/// renderer's Sun/Moon plumbing consistently observer-local before the future
/// VSOP87 upgrade. The underlying geocentric solar longitude remains the
/// compact Meeus value from [`apparent_sun`].
pub fn apparent_sun_topocentric(observer: Observer) -> SunApparent {
    let geo = apparent_sun(observer.julian_date);
    let dir = equatorial_unit_vector_f64(geo.right_ascension_rad, geo.declination_rad);
    let distance_km = geo.distance_au * ASTRONOMICAL_UNIT_KM;
    let observer_km = observer_equatorial_position_km(observer);
    let topo = [
        dir[0] * distance_km - observer_km[0],
        dir[1] * distance_km - observer_km[1],
        dir[2] * distance_km - observer_km[2],
    ];
    let (right_ascension_rad, declination_rad, topocentric_distance_km) =
        ra_dec_from_equatorial_vector(topo);

    SunApparent {
        right_ascension_rad,
        declination_rad,
        ecliptic_longitude_rad: geo.ecliptic_longitude_rad,
        distance_au: topocentric_distance_km / ASTRONOMICAL_UNIT_KM,
        angular_radius_rad: (SOLAR_RADIUS_KM / topocentric_distance_km).asin(),
    }
}

/// Approximate geocentric Moon position for a Julian Date.
///
/// This is the compact low-precision lunar orbit from Paul Schlyter's
/// reduction of the classical elements (node, inclination, argument of
/// perigee, eccentricity, mean anomaly), with the result rotated from
/// ecliptic to equatorial coordinates. It is intentionally a Phase-2-start
/// visual model: good enough to place and phase the Moon for rendering and
/// twilight/moonlight plumbing, but not the final ELP2000 implementation
/// required by the roadmap's arcsecond-grade ephemeris target.
pub fn apparent_moon(julian_date: f64) -> MoonApparent {
    // Days since 2000 Jan 0.0, matching the epoch used by the compact element
    // set. Angles are degrees until explicitly converted.
    let d = julian_date - 2_451_543.5;
    let n = (125.1228 - 0.052_953_808_3 * d).rem_euclid(360.0) * DEG_TO_RAD;
    let i = 5.1454 * DEG_TO_RAD;
    let w = (318.0634 + 0.164_357_322_3 * d).rem_euclid(360.0) * DEG_TO_RAD;
    let a_earth_radii = 60.2666;
    let e = 0.054_900;
    let m = (115.3654 + 13.064_992_950_9 * d).rem_euclid(360.0) * DEG_TO_RAD;

    // One Newton step is enough for this visual orbit because lunar e is small.
    let mut eccentric_anomaly = m + e * m.sin() * (1.0 + e * m.cos());
    eccentric_anomaly = eccentric_anomaly
        - (eccentric_anomaly - e * eccentric_anomaly.sin() - m)
            / (1.0 - e * eccentric_anomaly.cos());

    let xv = a_earth_radii * (eccentric_anomaly.cos() - e);
    let yv = a_earth_radii * (1.0 - e * e).sqrt() * eccentric_anomaly.sin();
    let true_anomaly = yv.atan2(xv);
    let distance_earth_radii = (xv * xv + yv * yv).sqrt();

    let (sin_n, cos_n) = n.sin_cos();
    let (sin_i, cos_i) = i.sin_cos();
    let (sin_vw, cos_vw) = (true_anomaly + w).sin_cos();
    let x_ecl = distance_earth_radii * (cos_n * cos_vw - sin_n * sin_vw * cos_i);
    let y_ecl = distance_earth_radii * (sin_n * cos_vw + cos_n * sin_vw * cos_i);
    let z_ecl = distance_earth_radii * (sin_vw * sin_i);

    // Mean obliquity is sufficient for this low-precision Moon path.
    let t = (julian_date - J2000_JD) / 36_525.0;
    let mean_obliquity_arcsec = 21.448 - t * (46.8150 + t * (0.00059 - t * 0.001813));
    let epsilon = (23.0 + (26.0 + mean_obliquity_arcsec / 60.0) / 60.0) * DEG_TO_RAD;
    let y_eq = y_ecl * epsilon.cos() - z_ecl * epsilon.sin();
    let z_eq = y_ecl * epsilon.sin() + z_ecl * epsilon.cos();
    let x_eq = x_ecl;

    let right_ascension_rad = y_eq.atan2(x_eq).rem_euclid(std::f64::consts::TAU);
    let declination_rad = z_eq.atan2((x_eq * x_eq + y_eq * y_eq).sqrt());
    let distance_km = distance_earth_radii * EARTH_EQUATORIAL_RADIUS_KM;
    let angular_radius_rad = (LUNAR_RADIUS_KM / distance_km).asin();

    let moon_dir = equatorial_unit_vector(right_ascension_rad, declination_rad);
    let sun_dir = apparent_sun(julian_date).direction_equatorial();
    let elongation = (moon_dir.dot(sun_dir) as f64).clamp(-1.0, 1.0).acos();
    let illuminated_fraction = 0.5 * (1.0 - elongation.cos());

    MoonApparent {
        right_ascension_rad,
        declination_rad,
        distance_km,
        angular_radius_rad,
        illuminated_fraction,
    }
}

/// Apparent topocentric Moon state for an observer on Earth.
///
/// This subtracts the observer's geocentric position from the compact
/// geocentric Moon vector. The correction reaches about one degree near the
/// horizon, which is large enough to matter for moonlit-sky directionality,
/// disk rendering, and rise/set timing even before the final ELP2000 model.
pub fn apparent_moon_topocentric(observer: Observer) -> MoonApparent {
    let geo = apparent_moon(observer.julian_date);
    let dir = equatorial_unit_vector_f64(geo.right_ascension_rad, geo.declination_rad);
    let observer_km = observer_equatorial_position_km(observer);
    let topo = [
        dir[0] * geo.distance_km - observer_km[0],
        dir[1] * geo.distance_km - observer_km[1],
        dir[2] * geo.distance_km - observer_km[2],
    ];
    let (right_ascension_rad, declination_rad, distance_km) = ra_dec_from_equatorial_vector(topo);

    MoonApparent {
        right_ascension_rad,
        declination_rad,
        distance_km,
        angular_radius_rad: (LUNAR_RADIUS_KM / distance_km).asin(),
        illuminated_fraction: geo.illuminated_fraction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sun_at_j2000_matches_almanac_scale() {
        let sun = apparent_sun(J2000_JD);
        assert!((sun.right_ascension_rad.to_degrees() - 281.28).abs() < 0.2);
        assert!((sun.declination_rad.to_degrees() + 23.03).abs() < 0.2);
        assert!((sun.distance_au - 0.9833).abs() < 0.002);
        assert!((sun.angular_radius_rad.to_degrees() * 60.0 - 16.26).abs() < 0.05);
    }

    #[test]
    fn june_solstice_sun_is_north() {
        // 2024-06-20T20:51Z, near the June solstice.
        let sun = apparent_sun(2_460_482.368_75);
        assert!((sun.declination_rad.to_degrees() - 23.44).abs() < 0.1);
    }

    #[test]
    fn direction_is_unit_length() {
        let sun = apparent_sun(2_460_000.5).direction_equatorial();
        let moon = apparent_moon(2_460_000.5).direction_equatorial();
        assert!((sun.length() - 1.0).abs() < 1e-6);
        assert!((moon.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn moon_visual_quantities_are_in_physical_ranges() {
        let moon = apparent_moon(J2000_JD);
        assert!((350_000.0..=410_000.0).contains(&moon.distance_km));
        let radius_arcmin = moon.angular_radius_rad.to_degrees() * 60.0;
        assert!((14.0..=17.5).contains(&radius_arcmin));
        assert!((0.0..=1.0).contains(&moon.illuminated_fraction));
    }

    fn angular_separation_rad(a_ra: f64, a_dec: f64, b_ra: f64, b_dec: f64) -> f64 {
        let a = equatorial_unit_vector_f64(a_ra, a_dec);
        let b = equatorial_unit_vector_f64(b_ra, b_dec);
        (a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
            .clamp(-1.0, 1.0)
            .acos()
    }

    #[test]
    fn topocentric_moon_applies_diurnal_parallax() {
        let observer = Observer::from_degrees(35.0, 139.0, 2_460_000.5);
        let geo = apparent_moon(observer.julian_date);
        let topo = apparent_moon_topocentric(observer);
        let separation = angular_separation_rad(
            geo.right_ascension_rad,
            geo.declination_rad,
            topo.right_ascension_rad,
            topo.declination_rad,
        )
        .to_degrees();
        assert!(separation > 0.01, "separation too small: {separation}°");
        assert!(separation < 1.2, "separation too large: {separation}°");
        assert_ne!(geo.distance_km, topo.distance_km);
    }

    #[test]
    fn topocentric_sun_parallax_is_small_but_finite() {
        let observer = Observer::from_degrees(35.0, 139.0, 2_460_000.5);
        let geo = apparent_sun(observer.julian_date);
        let topo = apparent_sun_topocentric(observer);
        let separation_arcsec = angular_separation_rad(
            geo.right_ascension_rad,
            geo.declination_rad,
            topo.right_ascension_rad,
            topo.declination_rad,
        )
        .to_degrees()
            * 3600.0;
        assert!(separation_arcsec > 0.01);
        assert!(separation_arcsec < 10.0);
    }
}
