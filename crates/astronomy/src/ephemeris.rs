//! Solar-system ephemerides used by visual rendering.
//!
//! The Phase 2 atmosphere stack needs observer-local Sun and Moon directions,
//! angular radii, lunar phase, and physically scaled illuminants. This module
//! uses the `astro` crate's VSOP87 Earth/Sun and ELP2000-style lunar series as
//! the geocentric source, then applies the renderer's WGS84 topocentric
//! parallax correction for disk placement and sky-light directionality.

use glam::Vec3;

use crate::{lmst_radians, Observer};

/// J2000.0 epoch, JD(TT) 2451545.0. The project currently passes JD(UT1≈UTC)
/// through most APIs; see the function-level notes below where formulae are
/// formally TT/TDB-based.
const J2000_JD: f64 = 2_451_545.0;
const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;
/// Astronomical Unit in kilometres, IAU 2012 Resolution B2 exact definition.
const ASTRONOMICAL_UNIT_KM: f64 = 149_597_870.7;
/// WGS84 semi-major axis in kilometres (NGA.STND.0036 / EPSG:7030). Used with
/// geodetic observer latitude for first-order topocentric parallax.
const EARTH_EQUATORIAL_RADIUS_KM: f64 = 6_378.137;
/// WGS84 flattening, exact reciprocal 298.257223563.
const EARTH_FLATTENING: f64 = 1.0 / 298.257_223_563;
/// Nominal solar radius in kilometres, IAU 2015 Resolution B3.
const SOLAR_RADIUS_KM: f64 = 695_700.0;
/// Mean lunar radius in kilometres, IAU/IAG Working Group cartographic value.
const LUNAR_RADIUS_KM: f64 = 1_737.4;

/// Apparent geocentric Sun state for rendering.
#[derive(Debug, Clone, Copy)]
pub struct SunApparent {
    /// Apparent right ascension in radians, in the equatorial frame of date used
    /// by the VSOP87/FK5 solar series.
    pub right_ascension_rad: f64,
    /// Apparent declination in radians, in the same frame as
    /// [`Self::right_ascension_rad`].
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
    /// Apparent right ascension in radians, in the equatorial frame of date used
    /// by the ELP2000 lunar series.
    pub right_ascension_rad: f64,
    /// Apparent declination in radians, in the same frame as
    /// [`Self::right_ascension_rad`].
    pub declination_rad: f64,
    /// Approximate geocentric distance in kilometres.
    pub distance_km: f64,
    /// Apparent angular radius of the lunar disk in radians.
    pub angular_radius_rad: f64,
    /// Illuminated fraction, 0 = new Moon, 1 = full Moon.
    pub illuminated_fraction: f64,
    /// Sun-Moon phase angle in radians, measured at the Moon: 0 = full Moon,
    /// π = new Moon. This is the angle used by lunar brightness laws and disk
    /// shading, distinct from the geocentric elongation seen by the observer.
    pub phase_angle_rad: f64,
}

/// Renderer-facing apparent disk inputs for the two atmosphere illuminants.
#[derive(Debug, Clone, Copy)]
pub struct SunMoonApparent {
    pub sun: SunApparent,
    pub moon: MoonApparent,
}

impl SunMoonApparent {
    pub fn for_observer(observer: Observer) -> Self {
        Self {
            sun: apparent_sun_topocentric(observer),
            moon: apparent_moon_topocentric(observer),
        }
    }
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

fn mean_obliquity_rad(julian_date: f64) -> f64 {
    let t = (julian_date - J2000_JD) / 36_525.0;
    let mean_obliquity_arcsec = 21.448 - t * (46.8150 + t * (0.00059 - t * 0.001813));
    (23.0 + (26.0 + mean_obliquity_arcsec / 60.0) / 60.0) * DEG_TO_RAD
}

fn ecliptic_to_equatorial_vector(
    longitude_rad: f64,
    latitude_rad: f64,
    radius: f64,
    obliquity_rad: f64,
) -> [f64; 3] {
    let (sin_lon, cos_lon) = longitude_rad.sin_cos();
    let (sin_lat, cos_lat) = latitude_rad.sin_cos();
    let (sin_eps, cos_eps) = obliquity_rad.sin_cos();
    let x_ecl = radius * cos_lat * cos_lon;
    let y_ecl = radius * cos_lat * sin_lon;
    let z_ecl = radius * sin_lat;
    [
        x_ecl,
        y_ecl * cos_eps - z_ecl * sin_eps,
        y_ecl * sin_eps + z_ecl * cos_eps,
    ]
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
/// The geocentric ecliptic longitude/distance come from the `astro` crate's
/// VSOP87 Earth solution (`astro::sun::geocent_ecl_pos`) with the FK5 correction
/// applied. The project still threads JD(UT1≈UTC) until the Time Systems row
/// lands; callers that already have TT/TDB can pass that Julian date here.
pub fn apparent_sun(julian_date: f64) -> SunApparent {
    let (sun_ecl, distance_au) = astro::sun::geocent_ecl_pos(julian_date);
    let (lambda, beta) = astro::sun::ecl_coords_to_FK5(julian_date, sun_ecl.long, sun_ecl.lat);
    let eq =
        ecliptic_to_equatorial_vector(lambda, beta, distance_au, mean_obliquity_rad(julian_date));
    let (right_ascension_rad, declination_rad, _) = ra_dec_from_equatorial_vector(eq);

    SunApparent {
        right_ascension_rad,
        declination_rad,
        ecliptic_longitude_rad: lambda.rem_euclid(std::f64::consts::TAU),
        distance_au,
        angular_radius_rad: (SOLAR_RADIUS_KM / (distance_au * ASTRONOMICAL_UNIT_KM)).asin(),
    }
}

/// Apparent topocentric Sun state for an observer on Earth.
///
/// The solar parallax is only ≈8.8 arcsec, but applying it keeps the renderer's
/// Sun/Moon plumbing consistently observer-local. The underlying geocentric
/// solar longitude remains the VSOP87/FK5 value from [`apparent_sun`].
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

/// Apparent geocentric Moon position for a Julian Date.
///
/// The geocentric ecliptic longitude/latitude/distance come from the `astro`
/// crate lunar series (`astro::lunar::geocent_ecl_pos`), which implements the
/// principal ELP2000 terms documented by Meeus. The output is rotated to the
/// mean equatorial frame of date for the renderer; topocentric parallax is
/// applied by [`apparent_moon_topocentric`].
pub fn apparent_moon(julian_date: f64) -> MoonApparent {
    let (moon_ecl, distance_km) = astro::lunar::geocent_ecl_pos(julian_date);
    let eq = ecliptic_to_equatorial_vector(
        moon_ecl.long,
        moon_ecl.lat,
        distance_km,
        mean_obliquity_rad(julian_date),
    );
    let (right_ascension_rad, declination_rad, _) = ra_dec_from_equatorial_vector(eq);
    let angular_radius_rad = (LUNAR_RADIUS_KM / distance_km).asin();

    let moon_dir = equatorial_unit_vector(right_ascension_rad, declination_rad);
    let sun_dir = apparent_sun(julian_date).direction_equatorial();
    let elongation = (moon_dir.dot(sun_dir) as f64).clamp(-1.0, 1.0).acos();
    let phase_angle_rad = (std::f64::consts::PI - elongation).clamp(0.0, std::f64::consts::PI);
    let illuminated_fraction = 0.5 * (1.0 + phase_angle_rad.cos());

    MoonApparent {
        right_ascension_rad,
        declination_rad,
        distance_km,
        angular_radius_rad,
        illuminated_fraction,
        phase_angle_rad,
    }
}

/// Apparent topocentric Moon state for an observer on Earth.
///
/// This subtracts the observer's WGS84 geocentric position from the ELP2000
/// geocentric Moon vector. The correction reaches about one degree near the
/// horizon, which is large enough to matter for moonlit-sky directionality,
/// disk rendering, and rise/set timing.
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
        phase_angle_rad: geo.phase_angle_rad,
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
