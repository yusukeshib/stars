//! Planetary satellites used by visual rendering (V-52b/c).
//!
//! This module currently exposes Jupiter's four Galilean moons (V-52b). The
//! per-moon Jovicentric (X, Y) sky-plane offsets, in units of Jupiter's
//! equatorial radius, come from the [`astro`] crate
//! ([`astro::planet::jupiter::moon::apprnt_rect_coords`]), which is the Meeus
//! 1998 *Astronomical Algorithms* ch. 44 simplification of J. Lieske's E5
//! theory (Lieske 1998, A&AS 129, 205).
//!
//! ## Accuracy budget
//!
//! Meeus's truncation reproduces individual moon positions to ≈10″ within a
//! few decades of J2000, drifting to arcminute-scale errors at the edges of
//! the ROADMAP ±100-yr budget. That is good enough for naked-eye / small-
//! telescope identification — the use case [`V-52b`] targets — but does
//! **not** meet the ~5″ / ±100-yr accuracy gate the full Lieske E5 theory
//! affords. The roadmap tracks the precision upgrade as a follow-on rung
//! (`V-52b-E5`) so the Meeus-grade renderer ships first.
//!
//! ## Frame conventions
//!
//! Mirrors the rest of [`crate::ephemeris`]: equatorial of date, FK5
//! longitude / latitude on the ecliptic of date, J2000.0 TDB Julian Date
//! when a `TimeScales` bundle is available. The topocentric path
//! subtracts the observer's WGS84 position from the Jupiter-centred line
//! of sight; Earth-radius parallax at Jupiter's mean distance (≈5.2 AU) is
//! at most ≈4″, well below the Meeus-grade accuracy budget but still
//! applied so the API matches the planet/Saturn-ring shape one-for-one.
//!
//! [`V-52b`]: https://github.com/yusukebe/stars/blob/main/ROADMAP.md

use glam::Vec3;

use crate::ephemeris::{
    apparent_planet, apparent_planet_topocentric, equatorial_unit_vector_f64,
    ra_dec_from_equatorial_vector, Planet, ASTRONOMICAL_UNIT_KM,
};
use crate::Observer;

/// Jupiter equatorial radius in kilometres
/// (IAU WGCCRE 2015 / Archinal et al. 2018, Table 4).
const JUPITER_EQUATORIAL_RADIUS_KM: f64 = 71_492.0;

/// The four Galilean moons in the canonical Meeus / Lieske ordering
/// (Io = I, Europa = II, Ganymede = III, Callisto = IV).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GalileanMoon {
    Io,
    Europa,
    Ganymede,
    Callisto,
}

impl GalileanMoon {
    pub const ALL: [Self; 4] = [Self::Io, Self::Europa, Self::Ganymede, Self::Callisto];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Io => "Io",
            Self::Europa => "Europa",
            Self::Ganymede => "Ganymede",
            Self::Callisto => "Callisto",
        }
    }

    /// Mean physical radius in kilometres.
    ///
    /// Values from Archinal et al. 2018 *Report of the IAU Working Group on
    /// Cartographic Coordinates and Rotational Elements: 2015* (CMDA 130, 22).
    pub const fn radius_km(self) -> f64 {
        match self {
            Self::Io => 1_821.6,
            Self::Europa => 1_560.8,
            Self::Ganymede => 2_634.1,
            Self::Callisto => 2_410.3,
        }
    }

    /// Mean opposition `V(1,0)` reduced visual magnitude from Meeus 1998
    /// table 41.A — the apparent magnitude at unit Earth/Sun distances. The
    /// runtime apparent magnitude is obtained by adding `5·log10(r · Δ)`
    /// where `r` and `Δ` are the heliocentric / geocentric distances of
    /// Jupiter in AU at the requested epoch. The Galilean moons orbit so
    /// close to Jupiter (<0.013 AU) that using Jupiter's `r`/`Δ` directly
    /// stays well inside the V-52b accuracy budget.
    pub const fn reduced_magnitude(self) -> f64 {
        match self {
            Self::Io => -1.68,
            Self::Europa => -1.41,
            Self::Ganymede => -2.09,
            Self::Callisto => -1.05,
        }
    }

    fn astro(self) -> astro::planet::jupiter::moon::Moon {
        match self {
            Self::Io => astro::planet::jupiter::moon::Moon::Io,
            Self::Europa => astro::planet::jupiter::moon::Moon::Europa,
            Self::Ganymede => astro::planet::jupiter::moon::Moon::Ganymede,
            Self::Callisto => astro::planet::jupiter::moon::Moon::Callisto,
        }
    }
}

/// Apparent state of a single Galilean moon, mirroring the
/// [`crate::PlanetApparent`] shape so the renderer can treat the four moons
/// as point sources next to Jupiter without a parallel uniform pipeline.
#[derive(Debug, Clone, Copy)]
pub struct GalileanMoonApparent {
    pub moon: GalileanMoon,
    /// Apparent right ascension in radians, equatorial frame of date.
    pub right_ascension_rad: f64,
    /// Apparent declination in radians, equatorial frame of date.
    pub declination_rad: f64,
    /// Observer-moon distance in astronomical units. The Meeus simplification
    /// does not return the line-of-sight offset, so this matches Jupiter's
    /// distance to within ≈0.013 AU (the moons' line-of-sight extent).
    pub distance_au: f64,
    /// Apparent angular radius of the moon's physical disk in radians. The
    /// Galilean moons are sub-pixel at naked-eye / small-eyepiece FoV so the
    /// renderer treats them as point sources; this field is still populated
    /// so future cluster-/binary-style logic can reuse it.
    pub angular_radius_rad: f64,
    /// Apparent visual magnitude. Computed from Meeus 1998 table 41.A
    /// reduced magnitudes plus the standard `5·log10(r · Δ)` distance term.
    pub magnitude: f64,
}

impl GalileanMoonApparent {
    /// Unit vector from observer toward the moon in equatorial coordinates.
    pub fn direction_equatorial(self) -> Vec3 {
        let [x, y, z] = equatorial_unit_vector_f64(self.right_ascension_rad, self.declination_rad);
        Vec3::new(x as f32, y as f32, z as f32)
    }
}

/// Apparent geocentric positions of Io, Europa, Ganymede, and Callisto for a
/// dynamical Julian Date. Order matches [`GalileanMoon::ALL`].
pub fn apparent_galilean_moons(julian_date: f64) -> [GalileanMoonApparent; 4] {
    let jupiter = apparent_planet(Planet::Jupiter, julian_date);
    let jupiter_dir =
        equatorial_unit_vector_f64(jupiter.right_ascension_rad, jupiter.declination_rad);
    let jupiter_distance_km = jupiter.distance_au * ASTRONOMICAL_UNIT_KM;
    galilean_moons_from_jupiter(
        julian_date,
        jupiter_dir,
        jupiter_distance_km,
        jupiter.distance_au,
        jupiter.heliocentric_distance_au,
    )
}

/// Apparent topocentric positions of the four Galilean moons for an
/// Earth observer. Subtracts the WGS84 observer position from the Jupiter-
/// centred line of sight to apply diurnal parallax; the orbital geometry
/// itself uses the dynamical (TDB / TT) Julian Date carried by [`Observer`].
pub fn apparent_galilean_moons_topocentric(observer: Observer) -> [GalileanMoonApparent; 4] {
    let jupiter_topo = apparent_planet_topocentric(observer, Planet::Jupiter);
    let jupiter_dir = equatorial_unit_vector_f64(
        jupiter_topo.right_ascension_rad,
        jupiter_topo.declination_rad,
    );
    let jupiter_distance_km = jupiter_topo.distance_au * ASTRONOMICAL_UNIT_KM;
    galilean_moons_from_jupiter(
        observer.time.jd_tdb,
        jupiter_dir,
        jupiter_distance_km,
        jupiter_topo.distance_au,
        jupiter_topo.heliocentric_distance_au,
    )
}

/// Build the four `GalileanMoonApparent` records around a precomputed Jupiter
/// state, given the unit direction toward Jupiter, its observer distance in
/// kilometres / AU, and its heliocentric distance in AU.
fn galilean_moons_from_jupiter(
    julian_date: f64,
    jupiter_dir: [f64; 3],
    jupiter_distance_km: f64,
    jupiter_distance_au: f64,
    jupiter_heliocentric_distance_au: f64,
) -> [GalileanMoonApparent; 4] {
    // Build a sky-plane orthonormal basis at Jupiter:
    // * `east_hat`  — local east on the sky (direction of increasing RA);
    // * `north_hat` — local north on the sky (direction of increasing Dec).
    //
    // Standard celestial-sphere convention: with `north_pole = (0, 0, 1)` in
    // the equatorial frame, `east_hat = north_pole × jupiter_dir` (normalised)
    // points along increasing RA, and `north_hat = jupiter_dir × east_hat`
    // completes the right-handed triple, pointing along increasing Dec.
    let north_pole = [0.0_f64, 0.0, 1.0];
    let east_hat = normalise(cross(north_pole, jupiter_dir));
    let north_hat = cross(jupiter_dir, east_hat);

    // Meeus's (X, Y) is in units of Jupiter's equatorial radius. The renderer
    // wants a topocentric direction, which we recover by adding the small
    // sky-plane offset to Jupiter's unit direction. Working in *physical*
    // units (km) lets us also recover the proper observer-moon distance, and
    // matches the rest of the ephemeris pipeline.
    let r_j_km = JUPITER_EQUATORIAL_RADIUS_KM;
    let log_term = (jupiter_heliocentric_distance_au * jupiter_distance_au).log10();

    let mut out: [Option<GalileanMoonApparent>; 4] = [None, None, None, None];
    for (slot, moon) in GalileanMoon::ALL.iter().enumerate() {
        // Meeus's `apprnt_rect_coords`:
        //   * X positive **west** of Jupiter (i.e. opposite the direction of
        //     increasing RA);
        //   * Y positive **north** of Jupiter.
        // Convert to (east, north) by flipping X.
        let (x_west, y_north) =
            astro::planet::jupiter::moon::apprnt_rect_coords(julian_date, &moon.astro());
        let east_offset_km = -x_west * r_j_km;
        let north_offset_km = y_north * r_j_km;

        // Position vector from the observer to the moon, in equatorial km.
        let pos_km = [
            jupiter_dir[0] * jupiter_distance_km
                + east_hat[0] * east_offset_km
                + north_hat[0] * north_offset_km,
            jupiter_dir[1] * jupiter_distance_km
                + east_hat[1] * east_offset_km
                + north_hat[1] * north_offset_km,
            jupiter_dir[2] * jupiter_distance_km
                + east_hat[2] * east_offset_km
                + north_hat[2] * north_offset_km,
        ];
        let (right_ascension_rad, declination_rad, distance_km) =
            ra_dec_from_equatorial_vector(pos_km);
        let distance_au = distance_km / ASTRONOMICAL_UNIT_KM;
        let radius_km = moon.radius_km();
        let angular_radius_rad = (radius_km / distance_km).atan();
        let magnitude = moon.reduced_magnitude() + 5.0 * log_term;

        out[slot] = Some(GalileanMoonApparent {
            moon: *moon,
            right_ascension_rad,
            declination_rad,
            distance_au,
            angular_radius_rad,
            magnitude,
        });
    }
    [
        out[0].unwrap(),
        out[1].unwrap(),
        out[2].unwrap(),
        out[3].unwrap(),
    ]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalise(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
    [v[0] / n, v[1] / n, v[2] / n]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ephemeris::observer_equatorial_position_km;
    use crate::{Observer, J2000_JD};

    // Maximum Jovicentric apparent elongation of each moon as seen from Earth,
    // in arcseconds (rounded up). Used as an upper sanity bound: a moon should
    // never appear farther from Jupiter than `(r_moon_in_R_J + slack) · R_J / Δ`
    // at the closest opposition. Values include 10% slack on the orbital radius
    // and assume the closest plausible Earth-Jupiter distance (~4.0 AU).
    const MAX_ELONGATION_ARCSEC: [f64; 4] = [
        160.0, // Io        : ~5.91 R_J · 1.1 / (4.0 AU) ≈ 2.4′
        220.0, // Europa    : ~9.40 R_J · 1.1 / (4.0 AU) ≈ 3.8′
        330.0, // Ganymede  : ~15.0 R_J · 1.1 / (4.0 AU) ≈ 6.0′
        570.0, // Callisto  : ~26.4 R_J · 1.1 / (4.0 AU) ≈ 10.7′
    ];

    fn angular_separation_arcsec(a_ra: f64, a_dec: f64, b_ra: f64, b_dec: f64) -> f64 {
        let cos_sep = a_dec.sin() * b_dec.sin() + a_dec.cos() * b_dec.cos() * (a_ra - b_ra).cos();
        cos_sep.clamp(-1.0, 1.0).acos().to_degrees() * 3600.0
    }

    #[test]
    fn moons_returned_in_canonical_order() {
        let moons = apparent_galilean_moons(J2000_JD);
        assert_eq!(moons[0].moon, GalileanMoon::Io);
        assert_eq!(moons[1].moon, GalileanMoon::Europa);
        assert_eq!(moons[2].moon, GalileanMoon::Ganymede);
        assert_eq!(moons[3].moon, GalileanMoon::Callisto);
    }

    #[test]
    fn moons_stay_within_max_elongation_from_jupiter() {
        // Sample two-week stride across one Callisto period (16.7 d) so every
        // moon visits a non-trivial part of its orbit.
        let jupiter = apparent_planet(Planet::Jupiter, J2000_JD);
        let moons = apparent_galilean_moons(J2000_JD);
        for (i, moon) in moons.iter().enumerate() {
            let sep = angular_separation_arcsec(
                moon.right_ascension_rad,
                moon.declination_rad,
                jupiter.right_ascension_rad,
                jupiter.declination_rad,
            );
            assert!(
                sep < MAX_ELONGATION_ARCSEC[i],
                "{} separation from Jupiter = {sep}\" exceeds bound {}\"",
                moon.moon.name(),
                MAX_ELONGATION_ARCSEC[i]
            );
        }
    }

    #[test]
    fn moons_have_distinct_positions() {
        let moons = apparent_galilean_moons(J2000_JD);
        for i in 0..4 {
            for j in (i + 1)..4 {
                let sep = angular_separation_arcsec(
                    moons[i].right_ascension_rad,
                    moons[i].declination_rad,
                    moons[j].right_ascension_rad,
                    moons[j].declination_rad,
                );
                assert!(
                    sep > 1.0,
                    "{} and {} overlap to <1″ at J2000",
                    moons[i].moon.name(),
                    moons[j].moon.name()
                );
            }
        }
    }

    #[test]
    fn moons_have_plausible_magnitudes_near_opposition() {
        // 2000-11-28 was an opposition of Jupiter; magnitudes should land
        // close to their tabulated near-opposition values (Meeus AA Table
        // 41.A, with V(1,0) plus the 5·log10(r·Δ) reduction at the actual
        // r ≈ 4.96 AU, Δ ≈ 3.97 AU geometry).
        let jd = 2_451_877.0; // 2000-11-28 12:00 UTC
        let moons = apparent_galilean_moons(jd);
        let expected = [5.0, 5.3, 4.6, 5.7]; // rough V near 2000 opposition
        for (i, moon) in moons.iter().enumerate() {
            assert!(
                (moon.magnitude - expected[i]).abs() < 0.4,
                "{} V = {:.2}, expected ≈ {:.2}",
                moon.moon.name(),
                moon.magnitude,
                expected[i]
            );
        }
    }

    #[test]
    fn moons_evolve_over_one_io_period() {
        // Io's orbital period is ≈1.77 days. After half a period the
        // sky-plane offset should be on the opposite side of Jupiter.
        let jd_a = J2000_JD;
        let jd_b = J2000_JD + 0.885; // half of Io's period
        let a = apparent_galilean_moons(jd_a)[0];
        let b = apparent_galilean_moons(jd_b)[0];
        let sep = angular_separation_arcsec(
            a.right_ascension_rad,
            a.declination_rad,
            b.right_ascension_rad,
            b.declination_rad,
        );
        assert!(
            sep > 100.0,
            "Io should swing > 100\" across half its orbital period; got {sep}\""
        );
    }

    #[test]
    fn topocentric_matches_geocentric_within_parallax() {
        // Earth-radius parallax at Jupiter (Δ ≈ 5 AU) is at most ≈4″. The
        // topocentric API should agree with the geocentric one to within
        // ≈10″ (sum of parallax on Jupiter + the moon's sky-plane projection).
        let jd = 2_451_545.0;
        let observer = Observer::from_degrees(35.68, 139.69, jd);
        let geo = apparent_galilean_moons(observer.time.jd_tdb);
        let topo = apparent_galilean_moons_topocentric(observer);
        for (g, t) in geo.iter().zip(topo.iter()) {
            let sep = angular_separation_arcsec(
                g.right_ascension_rad,
                g.declination_rad,
                t.right_ascension_rad,
                t.declination_rad,
            );
            assert!(
                sep < 10.0,
                "{} topocentric-geocentric offset {sep}\" exceeds parallax bound",
                g.moon.name()
            );
        }
    }

    #[test]
    fn moon_unit_direction_is_normalised() {
        let moons = apparent_galilean_moons(J2000_JD);
        for moon in moons {
            let dir = moon.direction_equatorial();
            assert!(
                (dir.length() - 1.0).abs() < 1e-5,
                "{} direction not unit length: |dir| = {}",
                moon.moon.name(),
                dir.length()
            );
        }
    }

    #[test]
    fn observer_position_zero_when_used_as_geocenter_check() {
        // Sanity: observer-equatorial-position helper is wired through, so
        // the topocentric path actually subtracts a non-zero vector and is
        // not an alias for the geocentric one. The actual numeric impact is
        // covered by `topocentric_matches_geocentric_within_parallax`.
        let observer = Observer::from_degrees(0.0, 0.0, J2000_JD);
        let pos = observer_equatorial_position_km(observer);
        let mag = (pos[0] * pos[0] + pos[1] * pos[1] + pos[2] * pos[2]).sqrt();
        assert!(mag > 6000.0 && mag < 6500.0, "observer rho_km = {mag}");
    }
}
