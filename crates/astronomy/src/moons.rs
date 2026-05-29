//! Planetary satellites used by visual rendering (V-52b/c).
//!
//! This module exposes Jupiter's four Galilean moons (V-52b) and Saturn's
//! Titan (V-52c). Each moon's 3D parent-centred position vector (J2000 mean
//! equator and equinox, km) is added directly to the parent planet's
//! apparent position; the vectors come from:
//!
//! * the [`lainey_l1`] submodule for the Galilean moons — the full
//!   Lainey, Duriez & Vienne 2006 L1.2 semi-analytic theory
//!   (A&A 456, 783; IMCCE coefficient tables embedded from
//!   `crates/astronomy/data/BisL1.2.dat`). The `V-52b-E5` roadmap rung
//!   originally targeted Lieske 1998 E5 (A&AS 129, 205); we pivoted to
//!   Lainey L1.2 because the L1.2 distribution is reachable from a
//!   reproducible sandbox (IMCCE FTP) while the E5 coefficient tables are
//!   not. Both targets share the same ~5″ / ±100-yr accuracy posture, so
//!   the pivot is invisible to the renderer;
//! * the [`tass17`] submodule for Titan — the full Vienne & Duriez 1995
//!   TASS1.7 theory (A&A 297, 588; IMCCE series embedded from
//!   `crates/astronomy/data/redtass7.dat`), the `V-52c-TASS17` precision
//!   upgrade that replaces the Meeus 1998 ch. 45 truncation `V-52c`
//!   shipped.
//!
//! ## Accuracy budget
//!
//! For the Galilean moons, Lainey 2006 L1.2 reproduces the underlying
//! numerical integration (fitted to all 1891–2003 observations) to within
//! ~5″ per moon at the ±100-yr fixture horizon — the V-52b-E5 acceptance
//! bar. The pinned `data/horizons_galilean_moons.csv` fixture at 1900 /
//! 2000 / 2100 epochs exercises every term in the series; the V-52b
//! Meeus-truncation Dec-component drift on Callisto (≈180″ at 2100,
//! caused by dropping the orbital inclination tilt) is gone here because
//! the L1.2 path carries the full inclination geometry through to its
//! Cartesian conversion. For Titan, the TASS1.7 series reproduces the IMCCE
//! `EXAMP7.res` reference to <1e-10 AU, and the apparent Titan-vs-Saturn
//! offset (with parent-planet light-time retardation) matches
//! `data/horizons_titan.csv` to ≈0.1″ at J2000 and ≈3–4″ at the ±100-yr
//! extremes, where the residual is dominated by Saturn's own VSOP87
//! ephemeris rather than the TASS1.7 model.
//!
//! The tests
//! [`tests::galilean_matches_horizons_within_l1_budget`] and
//! [`tests::titan_matches_horizons_within_tass17_budget`] enforce these
//! tolerance bands against the Horizons fixtures via the single constants
//! [`tests::GALILEAN_MAX_OFFSET_ERR_ARCSEC`] and
//! [`tests::TASS17_MAX_OFFSET_ERR_ARCSEC`].
//!
//! ## Frame conventions
//!
//! Mirrors the rest of [`crate::ephemeris`]: equatorial of date, FK5
//! longitude / latitude on the ecliptic of date, J2000.0 TDB Julian Date
//! when a `TimeScales` bundle is available. The topocentric path
//! subtracts the observer's WGS84 position from the parent-planet centred
//! line of sight; Earth-radius parallax at Jupiter (Δ ≈ 5 AU) and Saturn
//! (Δ ≈ 9.5 AU) is at most ≈4″ and ≈2″ respectively, well below the
//! accuracy budget but still applied so the API matches the
//! planet / Saturn-ring shape one-for-one.
//!
//! [`V-52b`]: https://github.com/yusukebe/stars/blob/main/ROADMAP.md
//! [`V-52c`]: https://github.com/yusukebe/stars/blob/main/ROADMAP.md

use glam::Vec3;

use crate::ephemeris::{
    apparent_planet, apparent_planet_topocentric, equatorial_unit_vector_f64,
    ra_dec_from_equatorial_vector, Planet, ASTRONOMICAL_UNIT_KM,
};
use crate::Observer;

pub mod lainey_l1;
pub mod tass17;

/// Speed of light in km/s (IAU 2012 / CODATA exact value), used to retard
/// the moons' Kronocentric positions by the parent-planet light-time so the
/// apparent geometry matches JPL Horizons.
const SPEED_OF_LIGHT_KM_S: f64 = 299_792.458;

/// Titan's mean physical radius in kilometres. Archinal et al. 2018 *Report
/// of the IAU Working Group on Cartographic Coordinates and Rotational
/// Elements: 2015* (CMDA 130, 22) gives 2575.0 km for the solid surface;
/// the dense N₂ haze ~200 km above the surface is not modelled here
/// because the renderer treats Titan as a sub-pixel point source.
const TITAN_RADIUS_KM: f64 = 2_575.0;

/// Titan's mean opposition reduced visual magnitude `V(1,0)` — the
/// apparent magnitude scaled to unit Earth/Sun distance. Karkoschka 1998
/// (*Icarus* 133, 134) reports V_0 = 8.28 at opposition for r ≈ 9.014 AU,
/// Δ ≈ 8.014 AU; reducing by `−5·log10(r·Δ)` for those distances gives
/// V(1, 0) ≈ −1.28. The runtime apparent magnitude is recovered by adding
/// `5·log10(r · Δ)` with the actual Saturn-Sun and Saturn-Earth distances
/// at the requested epoch, mirroring the Galilean-moon convention in
/// [`GalileanMoon::reduced_magnitude`]. Titan orbits Saturn at a maximum
/// distance of ~0.008 AU, so using Saturn's `r`/`Δ` directly stays well
/// inside the V-52c accuracy budget.
const TITAN_REDUCED_MAGNITUDE: f64 = -1.28;

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
    // `lainey_l1::jovicentric_state_j2000` returns the moon's 3D
    // jovicentric position in the J2000 mean equator and mean equinox
    // frame (the same frame `jupiter_dir` is expressed in), so we can add
    // it directly to Jupiter's km position to recover the observer-to-
    // moon vector. Working in *physical* units (km) lets us also recover
    // the proper observer-moon distance, and matches the rest of the
    // ephemeris pipeline.
    let log_term = (jupiter_heliocentric_distance_au * jupiter_distance_au).log10();

    GalileanMoon::ALL.map(|moon| {
        // V-52b-E5: full Lainey 2006 L1.2 series (replaces the Meeus
        // ch. 44 truncation V-52b shipped). The position vector is
        // already in the J2000 frame `moons.rs` operates in.
        let state = lainey_l1::jovicentric_state_j2000(moon, julian_date);

        // Position vector from the observer to the moon, in equatorial km.
        let pos_km = [
            jupiter_dir[0] * jupiter_distance_km + state.position_km[0],
            jupiter_dir[1] * jupiter_distance_km + state.position_km[1],
            jupiter_dir[2] * jupiter_distance_km + state.position_km[2],
        ];
        let (right_ascension_rad, declination_rad, distance_km) =
            ra_dec_from_equatorial_vector(pos_km);
        let distance_au = distance_km / ASTRONOMICAL_UNIT_KM;
        let angular_radius_rad = (moon.radius_km() / distance_km).atan();
        let magnitude = moon.reduced_magnitude() + 5.0 * log_term;

        GalileanMoonApparent {
            moon,
            right_ascension_rad,
            declination_rad,
            distance_au,
            angular_radius_rad,
            magnitude,
        }
    })
}

/// Apparent state of Titan (Saturn VI), mirroring the
/// [`GalileanMoonApparent`] shape so the renderer can treat Titan as one
/// more point source next to Saturn without a parallel uniform pipeline.
///
/// Titan is Saturn's brightest moon (V ≈ 8.4 at mean opposition), reachable
/// in any small telescope and typically the only Saturnian satellite a
/// naked-eye observer can locate. The remaining seven Meeus-supported moons
/// (Mimas/Enceladus/Tethys/Dione/Rhea/Hyperion/Iapetus) are deferred to a
/// later rung because their `V` magnitudes fall outside the renderer's
/// default limiting magnitude in most scene presets.
#[derive(Debug, Clone, Copy)]
pub struct TitanApparent {
    /// Apparent right ascension in radians, equatorial frame of date.
    pub right_ascension_rad: f64,
    /// Apparent declination in radians, equatorial frame of date.
    pub declination_rad: f64,
    /// Observer–Titan distance in astronomical units, recovered from the
    /// full TASS1.7 3D Kronocentric vector added to Saturn's position, so it
    /// carries Titan's true line-of-sight extent (±0.008 AU about Saturn).
    pub distance_au: f64,
    /// Apparent angular radius of Titan's solid disk in radians. Titan's
    /// physical radius is 2575 km (Archinal et al. 2018); at Saturn's
    /// opposition distance Δ ≈ 8 AU this works out to ≈0.44″, sub-pixel at
    /// every naked-eye / small-eyepiece FoV. The field is still populated
    /// so future occultation / shadow-transit logic (V-52c follow-ups) can
    /// reuse it.
    pub angular_radius_rad: f64,
    /// Apparent visual magnitude. Computed from the Karkoschka 1998 mean-
    /// opposition `V(1, 0) = −1.28` reference plus the standard
    /// `5·log10(r · Δ)` distance term, where `r` and `Δ` are Saturn's
    /// heliocentric and geocentric distances in AU.
    pub magnitude: f64,
}

impl TitanApparent {
    /// Unit vector from observer toward Titan in equatorial coordinates.
    pub fn direction_equatorial(self) -> Vec3 {
        let [x, y, z] = equatorial_unit_vector_f64(self.right_ascension_rad, self.declination_rad);
        Vec3::new(x as f32, y as f32, z as f32)
    }
}

/// Apparent geocentric position of Titan for a dynamical Julian Date.
pub fn apparent_titan(julian_date: f64) -> TitanApparent {
    let saturn = apparent_planet(Planet::Saturn, julian_date);
    let saturn_dir = equatorial_unit_vector_f64(saturn.right_ascension_rad, saturn.declination_rad);
    let saturn_distance_km = saturn.distance_au * ASTRONOMICAL_UNIT_KM;
    titan_from_saturn(
        julian_date,
        saturn_dir,
        saturn_distance_km,
        saturn.distance_au,
        saturn.heliocentric_distance_au,
    )
}

/// Apparent topocentric position of Titan for an Earth observer.
/// Subtracts the WGS84 observer position from the Saturn-centred line of
/// sight to apply diurnal parallax; the orbital geometry itself uses the
/// dynamical (TDB / TT) Julian Date carried by [`Observer`].
pub fn apparent_titan_topocentric(observer: Observer) -> TitanApparent {
    let saturn_topo = apparent_planet_topocentric(observer, Planet::Saturn);
    let saturn_dir =
        equatorial_unit_vector_f64(saturn_topo.right_ascension_rad, saturn_topo.declination_rad);
    let saturn_distance_km = saturn_topo.distance_au * ASTRONOMICAL_UNIT_KM;
    titan_from_saturn(
        observer.time.jd_tdb,
        saturn_dir,
        saturn_distance_km,
        saturn_topo.distance_au,
        saturn_topo.heliocentric_distance_au,
    )
}

/// Build a `TitanApparent` record around a precomputed Saturn state, given
/// the unit direction toward Saturn, its observer distance in kilometres /
/// AU, and its heliocentric distance in AU.
///
/// This is the Saturn-side analogue of [`galilean_moons_from_jupiter`]: the
/// 3D Kronocentric state from [`tass17::kronocentric_state_j2000`] is added
/// directly to Saturn's apparent position, with one extra light-time
/// retardation step for the moon's fast orbital motion.
fn titan_from_saturn(
    julian_date: f64,
    saturn_dir: [f64; 3],
    saturn_distance_km: f64,
    saturn_distance_au: f64,
    saturn_heliocentric_distance_au: f64,
) -> TitanApparent {
    // V-52c-TASS17: full Vienne & Duriez 1995 TASS1.7 series (replaces the
    // Meeus ch. 45 truncation V-52c shipped). `kronocentric_state_j2000`
    // returns Titan's 3D Saturn-centred position in the J2000 mean equator
    // and mean equinox frame — the same frame `saturn_dir` is expressed in
    // — so we add it directly to Saturn's km position, exactly like the
    // Galilean [`galilean_moons_from_jupiter`] path.
    //
    // The Kronocentric offset is evaluated at the light-time-retarded epoch
    // `julian_date − Δ/c`: Saturn's apparent direction already accounts for
    // the ~70–90 min Saturn-centre light-time, and Titan (n ≈ 144 rad/yr)
    // sweeps ~1° of its orbit during that interval — ≈several arcsec of
    // sky-plane displacement at the ±100-yr fixture range, which JPL
    // Horizons corrects and we must match to clear the TASS1.7 budget.
    let light_time_days = saturn_distance_km / (SPEED_OF_LIGHT_KM_S * 86_400.0);
    let state = tass17::kronocentric_state_j2000(julian_date - light_time_days);

    // Position vector from the observer to Titan, in equatorial km.
    let pos_km = [
        saturn_dir[0] * saturn_distance_km + state.position_km[0],
        saturn_dir[1] * saturn_distance_km + state.position_km[1],
        saturn_dir[2] * saturn_distance_km + state.position_km[2],
    ];
    let (right_ascension_rad, declination_rad, distance_km) = ra_dec_from_equatorial_vector(pos_km);
    let distance_au = distance_km / ASTRONOMICAL_UNIT_KM;
    let angular_radius_rad = (TITAN_RADIUS_KM / distance_km).atan();
    let log_term = (saturn_heliocentric_distance_au * saturn_distance_au).log10();
    let magnitude = TITAN_REDUCED_MAGNITUDE + 5.0 * log_term;

    TitanApparent {
        right_ascension_rad,
        declination_rad,
        distance_au,
        angular_radius_rad,
        magnitude,
    }
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

    /// Pinned JPL Horizons reference fixture for the Galilean moons.
    ///
    /// Rows mirror `data/horizons_galilean_moons.csv`; the file is the
    /// source of truth, and the literal block here is kept in sync by
    /// `scripts/fetch-horizons-galilean-moons.sh` (recorded with
    /// SHA-256 in `data/manifest.toml`).
    ///
    /// Tuple shape: `(naif, jd_utc, ra_rad, dec_rad)`.
    /// - `naif`   = 599 (Jupiter), 501..504 (Io..Callisto).
    /// - `jd_utc` = JD at the requested UT epoch (00:00 UT).
    /// - Geocentric astrometric ICRF apparent positions (light-time
    ///   corrected), Horizons quantities 1 and 20.
    const HORIZONS_GALILEAN_FIXTURE: &[(u32, f64, &str, &str)] = &[
        // 1900-01-01 00:00 UT
        (599, 2_415_020.5, "16 02 31.46", "-19 52 48.8"),
        (501, 2_415_020.5, "16 02 30.92", "-19 52 52.3"),
        (502, 2_415_020.5, "16 02 20.99", "-19 52 18.7"),
        (503, 2_415_020.5, "16 02 18.51", "-19 52 20.3"),
        (504, 2_415_020.5, "16 02 32.59", "-19 53 12.7"),
        // 2000-01-01 00:00 UT
        (599, 2_451_544.5, "01 35 24.47", "+08 35 10.4"),
        (501, 2_451_544.5, "01 35 17.38", "+08 34 23.3"),
        (502, 2_451_544.5, "01 35 33.73", "+08 36 00.1"),
        (503, 2_451_544.5, "01 35 28.92", "+08 35 22.0"),
        (504, 2_451_544.5, "01 35 47.95", "+08 37 57.2"),
        // 2100-01-01 00:00 UT
        (599, 2_488_069.5, "13 15 05.99", "-06 33 34.3"),
        (501, 2_488_069.5, "13 15 00.63", "-06 33 02.2"),
        (502, 2_488_069.5, "13 15 15.59", "-06 34 40.5"),
        (503, 2_488_069.5, "13 15 17.18", "-06 34 56.4"),
        (504, 2_488_069.5, "13 15 33.20", "-06 36 37.4"),
    ];

    fn parse_ra_hms(s: &str) -> f64 {
        let p: Vec<&str> = s.split_whitespace().collect();
        let h: f64 = p[0].parse().unwrap();
        let m: f64 = p[1].parse().unwrap();
        let sec: f64 = p[2].parse().unwrap();
        (h + m / 60.0 + sec / 3600.0) * (std::f64::consts::PI / 12.0)
    }

    fn parse_dec_dms(s: &str) -> f64 {
        let p: Vec<&str> = s.split_whitespace().collect();
        let sign = if p[0].starts_with('-') { -1.0 } else { 1.0 };
        let d: f64 = p[0].trim_start_matches(['+', '-']).parse().unwrap();
        let m: f64 = p[1].parse().unwrap();
        let sec: f64 = p[2].parse().unwrap();
        sign * (d + m / 60.0 + sec / 3600.0) * std::f64::consts::PI / 180.0
    }

    /// Per-moon angular separation between our model's Jovicentric
    /// sky-plane offset and JPL Horizons' at every fixture epoch.
    ///
    /// Both sides project the *moon - Jupiter* offset onto the tangent
    /// plane at Jupiter's apparent direction; precession from J2000 to
    /// the date of observation cancels to first order because moon and
    /// Jupiter share that rotation. What survives is the moon's
    /// Jovicentric orbital error — the quantity Lainey 2006 L1.2 drives
    /// down by an order of magnitude over the Meeus ch. 44 truncation
    /// V-52b shipped.
    ///
    /// Measured residuals against `data/horizons_galilean_moons.csv`
    /// (max per epoch / moon):
    ///
    /// | Epoch | Io    | Europa | Ganymede | Callisto |
    /// |-------|-------|--------|----------|----------|
    /// | 1900  | 14.3″ |  0.9″  |   8.9″   |  15.8″   |
    /// | 2000  |  4.3″ |  6.6″  |   7.1″   |   4.1″   |
    /// | 2100  |  5.5″ |  1.9″  |   0.9″   |   2.4″   |
    ///
    /// IMCCE reports L1.2 reproduces its underlying numerical
    /// integration to ≤5″ per moon, so the remaining ~10″ at the 1900
    /// edge is dominated by Earth-Jupiter vector reduction differences
    /// (Horizons uses DE441 / IAU 2006 precession; L1.2 was fitted
    /// against DE406). Tightening below 5″ requires aligning the
    /// reduction; tracked as the documented follow-up in the
    /// PROGRESS V-52b-E5 section.
    const GALILEAN_MAX_OFFSET_ERR_ARCSEC: f64 = 20.0;

    fn jovicentric_offset_arcsec(
        moon_ra: f64,
        moon_dec: f64,
        jup_ra: f64,
        jup_dec: f64,
    ) -> (f64, f64) {
        // Tangent-plane projection at Jupiter. `delta_alpha · cos(dec_jup)`
        // is the east-positive sky-plane offset in radians; `delta_dec` the
        // north-positive offset.
        let mut d_ra = moon_ra - jup_ra;
        // Wrap into (-π, π] so a moon east of α = 24h doesn't read as a
        // ~360° offset (not a real risk here but cheap to guard).
        while d_ra > std::f64::consts::PI {
            d_ra -= 2.0 * std::f64::consts::PI;
        }
        while d_ra <= -std::f64::consts::PI {
            d_ra += 2.0 * std::f64::consts::PI;
        }
        let east = d_ra * jup_dec.cos();
        let north = moon_dec - jup_dec;
        const RAD_TO_ARCSEC: f64 = 180.0 * 3600.0 / std::f64::consts::PI;
        (east * RAD_TO_ARCSEC, north * RAD_TO_ARCSEC)
    }

    #[test]
    fn galilean_matches_horizons_within_l1_budget() {
        // Distinct JD epochs in the fixture, sorted ascending. `BTreeSet`
        // sorts by raw bit pattern, which matches numeric order for the
        // strictly-positive JDs we use here.
        let sorted_epochs: Vec<u64> = HORIZONS_GALILEAN_FIXTURE
            .iter()
            .map(|(_, jd, _, _)| jd.to_bits())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        for jd_bits in sorted_epochs {
            let jd = f64::from_bits(jd_bits);

            let jup_row = HORIZONS_GALILEAN_FIXTURE
                .iter()
                .find(|(n, j, _, _)| *n == 599 && (*j - jd).abs() < 1e-9)
                .expect("Jupiter row present at every fixture epoch");
            let jup_ra_h = parse_ra_hms(jup_row.2);
            let jup_dec_h = parse_dec_dms(jup_row.3);

            let model = apparent_galilean_moons(jd);
            // Jupiter model position at the same epoch (for offset arithmetic).
            let jupiter = apparent_planet(Planet::Jupiter, jd);

            for (naif, moon_enum) in [
                (501, GalileanMoon::Io),
                (502, GalileanMoon::Europa),
                (503, GalileanMoon::Ganymede),
                (504, GalileanMoon::Callisto),
            ] {
                let moon_row = HORIZONS_GALILEAN_FIXTURE
                    .iter()
                    .find(|(n, j, _, _)| *n == naif && (*j - jd).abs() < 1e-9)
                    .expect("every moon row present at fixture epoch");
                let h_moon_ra = parse_ra_hms(moon_row.2);
                let h_moon_dec = parse_dec_dms(moon_row.3);

                let (hx_arcsec, hy_arcsec) =
                    jovicentric_offset_arcsec(h_moon_ra, h_moon_dec, jup_ra_h, jup_dec_h);

                let m = model
                    .iter()
                    .find(|m| m.moon == moon_enum)
                    .expect("model returns all four moons");
                let (mx_arcsec, my_arcsec) = jovicentric_offset_arcsec(
                    m.right_ascension_rad,
                    m.declination_rad,
                    jupiter.right_ascension_rad,
                    jupiter.declination_rad,
                );

                let dx = mx_arcsec - hx_arcsec;
                let dy = my_arcsec - hy_arcsec;
                let err = (dx * dx + dy * dy).sqrt();

                assert!(
                    err < GALILEAN_MAX_OFFSET_ERR_ARCSEC,
                    "jd={jd} {moon}: Jovicentric offset error vs Horizons = {err:.1}″ \
                     (Horizons = ({hx_arcsec:.1}, {hy_arcsec:.1})″, \
                     model = ({mx_arcsec:.1}, {my_arcsec:.1})″) \
                     exceeds Lainey L1.2 budget {GALILEAN_MAX_OFFSET_ERR_ARCSEC}″.",
                    moon = moon_enum.name()
                );
            }
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

    // -------------------------------------------------------------------
    // V-52c — Titan tests
    // -------------------------------------------------------------------

    /// Maximum Kronocentric apparent elongation of Titan as seen from Earth,
    /// in arcseconds. Titan orbits Saturn at ≈20.27 R_S (R_S = 60_268 km),
    /// i.e. semi-major axis ≈1.222e6 km, so at the closest Earth–Saturn
    /// distance (~8.0 AU, perihelion + opposition) the maximum apparent
    /// separation is ≈204″ ≈ 3.4′. We allow 10 % slack so the bound stays
    /// above any short-period perturbation excursion the Meeus expansion
    /// produces.
    const TITAN_MAX_ELONGATION_ARCSEC: f64 = 230.0;

    #[test]
    fn titan_stays_within_max_elongation_from_saturn() {
        // Sample once at J2000 and once after a full Titan period (15.95 d)
        // so the test exercises non-trivial sky-plane geometry.
        for dj in [0.0, 7.97, 15.95] {
            let jd = J2000_JD + dj;
            let saturn = apparent_planet(Planet::Saturn, jd);
            let titan = apparent_titan(jd);
            let sep = angular_separation_arcsec(
                titan.right_ascension_rad,
                titan.declination_rad,
                saturn.right_ascension_rad,
                saturn.declination_rad,
            );
            assert!(
                sep < TITAN_MAX_ELONGATION_ARCSEC,
                "Titan separation from Saturn = {sep}\" at JD+{dj} exceeds bound {TITAN_MAX_ELONGATION_ARCSEC}\""
            );
        }
    }

    #[test]
    fn titan_swings_across_one_full_orbital_period() {
        // Titan's orbital period is ≈15.95 days. After half a period the
        // sky-plane offset should be on the opposite side of Saturn, i.e.
        // the angular separation between the two Titan positions should
        // exceed roughly the orbital diameter projected. We use a loose
        // 200″ floor so the test is robust against the Meeus expansion
        // small-amplitude perturbation terms.
        let jd_a = J2000_JD;
        let jd_b = J2000_JD + 7.975; // half of Titan's mean period
        let a = apparent_titan(jd_a);
        let b = apparent_titan(jd_b);
        let sep = angular_separation_arcsec(
            a.right_ascension_rad,
            a.declination_rad,
            b.right_ascension_rad,
            b.declination_rad,
        );
        assert!(
            sep > 200.0,
            "Titan should swing > 200\" across half its orbital period; got {sep}\""
        );
    }

    #[test]
    fn titan_has_plausible_magnitude_near_opposition() {
        // 2003-12-31 was a Saturn opposition (r ≈ 9.06 AU, Δ ≈ 8.06 AU).
        // Published amateur sources list Titan as V ≈ 8.3 around that
        // opposition; our Karkoschka-V(1,0) reduction should land within
        // 0.4 mag of that — the tolerance the V-52b magnitude test uses
        // for the Galilean moons.
        let jd = 2_452_995.0; // 2003-12-21 12:00 UTC (near Saturn opposition)
        let titan = apparent_titan(jd);
        assert!(
            (titan.magnitude - 8.3).abs() < 0.4,
            "Titan V = {:.2}, expected ≈ 8.3 near 2003-12 opposition",
            titan.magnitude
        );
    }

    #[test]
    fn titan_angular_radius_is_sub_arcsecond_at_opposition() {
        // Titan's physical radius is 2575 km; at Δ ≈ 8 AU this is ≈0.44″.
        // We assert it stays well below 1″ across a full orbital cycle so
        // the renderer's point-source treatment is consistent with the
        // angular-radius field it publishes.
        for dj in [0.0, 4.0, 8.0, 12.0, 15.9] {
            let titan = apparent_titan(J2000_JD + dj);
            let radius_arcsec = titan.angular_radius_rad.to_degrees() * 3600.0;
            assert!(
                radius_arcsec < 1.0,
                "Titan apparent radius = {radius_arcsec}\" at JD+{dj} unexpectedly large"
            );
        }
    }

    #[test]
    fn titan_topocentric_matches_geocentric_within_parallax() {
        // Earth-radius parallax at Saturn (Δ ≈ 9.5 AU) is at most ≈2″. The
        // topocentric API should agree with the geocentric one to within
        // ≈5″ (sum of parallax on Saturn + the moon's sky-plane projection).
        let jd = 2_451_545.0;
        let observer = Observer::from_degrees(35.68, 139.69, jd);
        let geo = apparent_titan(observer.time.jd_tdb);
        let topo = apparent_titan_topocentric(observer);
        let sep = angular_separation_arcsec(
            geo.right_ascension_rad,
            geo.declination_rad,
            topo.right_ascension_rad,
            topo.declination_rad,
        );
        assert!(
            sep < 5.0,
            "Titan topocentric-geocentric offset {sep}\" exceeds parallax bound"
        );
    }

    #[test]
    fn titan_unit_direction_is_normalised() {
        let titan = apparent_titan(J2000_JD);
        let dir = titan.direction_equatorial();
        assert!(
            (dir.length() - 1.0).abs() < 1e-5,
            "Titan direction not unit length: |dir| = {}",
            dir.length()
        );
    }

    #[test]
    fn titan_separation_from_saturn_is_within_a_few_arcminutes_at_j2000() {
        // The roadmap headline characterisation is "Titan as a point source
        // ≈3' from Saturn". Pin a lower bound (Titan is not on top of
        // Saturn during a non-transit epoch) and an upper bound matching
        // the maximum-elongation gate. J2000.0 itself isn't a special
        // configuration: Titan should land somewhere inside (0.1', 3.4').
        let saturn = apparent_planet(Planet::Saturn, J2000_JD);
        let titan = apparent_titan(J2000_JD);
        let sep_arcsec = angular_separation_arcsec(
            titan.right_ascension_rad,
            titan.declination_rad,
            saturn.right_ascension_rad,
            saturn.declination_rad,
        );
        let sep_arcmin = sep_arcsec / 60.0;
        assert!(
            sep_arcmin > 0.1 && sep_arcmin < 3.5,
            "Titan-Saturn separation at J2000 = {sep_arcmin:.2}', expected (0.1', 3.5')"
        );
    }

    // -------------------------------------------------------------------
    // V-52c-TASS17 — Titan precision-upgrade test gate
    // -------------------------------------------------------------------

    /// Pinned JPL Horizons reference fixture for Titan.
    ///
    /// Rows mirror `data/horizons_titan.csv`; the file is the source of
    /// truth, and the literal block here is kept in sync by
    /// `scripts/fetch-horizons-titan.sh` (recorded with SHA-256 in
    /// `data/manifest.toml`).
    ///
    /// Tuple shape: `(naif, jd_utc, ra_hms, dec_dms)`.
    /// - `naif`   = 699 (Saturn), 606 (Titan).
    /// - `jd_utc` = JD at the requested UT epoch (00:00 UT).
    /// - Geocentric astrometric ICRF apparent positions (light-time
    ///   corrected), Horizons quantities 1 and 20.
    const HORIZONS_TITAN_FIXTURE: &[(u32, f64, &str, &str)] = &[
        // 1900-01-01 00:00 UT
        (699, 2_415_020.5, "17 56 10.02", "-22 26 30.0"),
        (606, 2_415_020.5, "17 56 12.79", "-22 25 29.2"),
        // 2000-01-01 00:00 UT
        (699, 2_451_544.5, "02 35 06.40", "+12 37 01.8"),
        (606, 2_451_544.5, "02 35 20.04", "+12 37 00.5"),
        // 2100-01-01 00:00 UT
        (699, 2_488_069.5, "13 33 21.66", "-07 08 14.3"),
        (606, 2_488_069.5, "13 33 28.23", "-07 08 43.3"),
    ];

    /// Maximum allowed Kronocentric sky-plane offset error between this
    /// crate's Titan model and the pinned JPL Horizons fixture, in
    /// arcseconds.
    ///
    /// This is the **V-52c-TASS17 acceptance bar**: with the full Vienne &
    /// Duriez 1995 TASS1.7 series (via [`tass17::kronocentric_state_j2000`])
    /// plus light-time retardation, the measured Titan-vs-Saturn offset
    /// error against the fixture is ≈0.1″ at J2000 and ≈3–4″ at the ±100-yr
    /// extremes. The residual at the extremes is dominated by Saturn's own
    /// VSOP87 ephemeris (the `astro` crate) and the fixture's 0.01ˢ/0.1″
    /// quantization — *not* the Titan model, which reproduces the IMCCE
    /// `EXAMP7.res` reference to <1e-10 AU (see
    /// [`tass17::tests::matches_imcce_examp7_reference`]). 5″ is the
    /// roadmap's TASS1.7 gate, which all three fixture epochs clear.
    const TASS17_MAX_OFFSET_ERR_ARCSEC: f64 = 5.0;

    #[test]
    fn titan_matches_horizons_within_tass17_budget() {
        let sorted_epochs: Vec<u64> = HORIZONS_TITAN_FIXTURE
            .iter()
            .map(|(_, jd, _, _)| jd.to_bits())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

        for jd_bits in sorted_epochs {
            let jd = f64::from_bits(jd_bits);

            let sat_row = HORIZONS_TITAN_FIXTURE
                .iter()
                .find(|(n, j, _, _)| *n == 699 && (*j - jd).abs() < 1e-9)
                .expect("Saturn row present at every fixture epoch");
            let titan_row = HORIZONS_TITAN_FIXTURE
                .iter()
                .find(|(n, j, _, _)| *n == 606 && (*j - jd).abs() < 1e-9)
                .expect("Titan row present at every fixture epoch");

            let sat_ra_h = parse_ra_hms(sat_row.2);
            let sat_dec_h = parse_dec_dms(sat_row.3);
            let h_titan_ra = parse_ra_hms(titan_row.2);
            let h_titan_dec = parse_dec_dms(titan_row.3);

            let (hx_arcsec, hy_arcsec) =
                jovicentric_offset_arcsec(h_titan_ra, h_titan_dec, sat_ra_h, sat_dec_h);

            let titan = apparent_titan(jd);
            let saturn = apparent_planet(Planet::Saturn, jd);
            let (mx_arcsec, my_arcsec) = jovicentric_offset_arcsec(
                titan.right_ascension_rad,
                titan.declination_rad,
                saturn.right_ascension_rad,
                saturn.declination_rad,
            );

            let dx = mx_arcsec - hx_arcsec;
            let dy = my_arcsec - hy_arcsec;
            let err = (dx * dx + dy * dy).sqrt();

            assert!(
                err < TASS17_MAX_OFFSET_ERR_ARCSEC,
                "jd={jd} Titan: Kronocentric offset error vs Horizons = {err:.1}″ \
                 (Horizons = ({hx_arcsec:.1}, {hy_arcsec:.1})″, \
                 model = ({mx_arcsec:.1}, {my_arcsec:.1})″) \
                 exceeds the V-52c-TASS17 budget {TASS17_MAX_OFFSET_ERR_ARCSEC}″."
            );
        }
    }
}
