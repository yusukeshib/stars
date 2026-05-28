//! Lieske 1998 E5 theory of the Galilean satellites — scaffolding.
//!
//! This module is the substitution point for the `V-52b-E5` precision
//! upgrade tracked in `ROADMAP.md`. The public API
//! ([`jovicentric_offset`] + [`JovicentricOffset`]) is what `moons.rs` calls
//! from `apparent_galilean_moons{,_topocentric}`. Today it delegates to the
//! Meeus 1998 ch. 44 truncation already wired through the [`astro`] crate
//! so the renderer keeps producing the same Meeus-grade positions
//! `V-52b` shipped. The follow-up PR replaces the body of
//! [`jovicentric_offset`] with the full Lieske 1998 trigonometric series
//! (loaded from the coefficient tables transcribed in
//! `crates/astronomy/data/lieske_e5/`) without changing the call sites.
//!
//! ## Why scaffolding rather than the full E5 series in this PR
//!
//! The Lieske 1998 paper publishes ~700 trigonometric coefficients plus
//! ~700 argument/rate pairs across the four moons (Io: 10 ξ + 41 V + 7 ζ
//! terms; Europa: 24/66/11; Ganymede: 31/75/13; Callisto: 49/89/18 — the
//! per-moon counts mirror Jay Lieske's reference `galsat` Fortran
//! implementation). Each term carries an integer `kod`-pair encoding
//! which of the 99 secular angles enters its phase, and the Jupiter /
//! Saturn long-period inequality (Lieske 1977 table 3 footnote) injects a
//! 50-day rolling `dG` perturbation on top. Faithfully transcribing those
//! tables without typo regression requires its own validation matrix —
//! the explicit reason the roadmap split `V-52b` (Meeus-grade engine
//! plumbing) from `V-52b-E5` (precision upgrade).
//!
//! This PR ships:
//! 1. The module structure ([`JovicentricOffset`], [`jovicentric_offset`],
//!    [`MoonSeriesShape`]) the follow-up swaps the coefficients into;
//! 2. A pinned JPL Horizons reference fixture
//!    (`data/horizons_galilean_moons.csv`) at three epochs spanning ±100
//!    years — the bar the precision upgrade has to clear (~5″);
//! 3. A test gate documenting the **current** Meeus-grade error budget
//!    against that fixture so the follow-up tightens a single tolerance
//!    constant instead of inventing a new test harness.
//!
//! ## References
//!
//! - Lieske, J. H. 1998, A&AS 129, 205 — E5 theory of the Galilean
//!   satellites (the target).
//! - Lieske, J. H. 1977, A&A 56, 333 — E2 theory; introduces the
//!   `xi / V / zeta` decomposition and the `kod`-encoding the E5 paper
//!   inherits.
//! - Meeus, J. 1998, *Astronomical Algorithms*, 2nd ed., ch. 44 — the
//!   low-precision truncation actually exercised today via the
//!   [`astro`] crate.
//! - Lieske, J. H. 1977, *JPL Engineering Memorandum 314-112*: companion
//!   routines for partials and barycenter-to-Jupiter correction —
//!   relevant to the velocity branch the E5 follow-up will add.

use super::GalileanMoon;

/// Jovicentric sky-plane offset of a single Galilean moon, expressed in
/// the same orthonormal `(east, north)` basis the renderer consumes.
///
/// Units: Jupiter's equatorial radius (IAU WGCCRE 2015, 71 492 km). The
/// caller scales by `JUPITER_EQUATORIAL_RADIUS_KM` to get a physical
/// kilometre offset and combines it with the planet's line-of-sight
/// direction to recover the moon's apparent right ascension / declination.
///
/// The sign convention deliberately matches the rest of the project: east
/// is the direction of increasing right ascension, north the direction of
/// increasing declination. (Meeus's published (X, Y) is east-negative /
/// north-positive in units of `R_J`; the conversion is folded into this
/// type.)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JovicentricOffset {
    /// Sky-plane offset along the local east direction, in `R_J`.
    /// Positive values mean the moon is east of (greater RA than) Jupiter.
    pub east_radii: f64,
    /// Sky-plane offset along the local north direction, in `R_J`.
    /// Positive values mean the moon is north of (greater Dec than) Jupiter.
    pub north_radii: f64,
}

/// Apparent Jovicentric sky-plane offset of `moon` at a TDB Julian Date.
///
/// Today this is the Meeus 1998 ch. 44 truncation routed through the
/// [`astro`] crate. The `V-52b-E5` precision upgrade replaces this body
/// with the full Lieske 1998 trigonometric series.
pub fn jovicentric_offset(moon: GalileanMoon, julian_date: f64) -> JovicentricOffset {
    // TODO(V-52b-E5): replace the call below with the full Lieske 1998
    // series evaluator. The expected shape is:
    //
    // 1. Look up the per-moon (xi, V, zeta) coefficient table (see
    //    `MoonSeriesShape` for the counts).
    // 2. Evaluate the three trigonometric sums at `t = julian_date - T_REF`
    //    (Lieske uses `T_REF = 2_443_000.5` JD = 1976-Aug-10.0 TDB).
    // 3. Apply the long-period Jupiter / Saturn inequality correction
    //    (`dG`, refreshed every 50 days following Lieske 1977 table 3
    //    footnote).
    // 4. Form orbital-plane (x, y, z) from `axis_au[moon] * cos(angle) *
    //    (1 + xi)` etc.
    // 5. Rotate by the `qqdot` matrix (Jupiter's equator → J2000 mean
    //    equator) and project onto the observer's sky-plane east / north.
    //
    // Until that lands, fall back to the Meeus truncation — same
    // numerical result the renderer has shipped since V-52b.
    let (x_west_radii, y_north_radii) =
        astro::planet::jupiter::moon::apprnt_rect_coords(julian_date, &moon.astro());
    JovicentricOffset {
        east_radii: -x_west_radii,
        north_radii: y_north_radii,
    }
}

/// Per-moon shape of the Lieske 1998 trigonometric series, recorded for
/// the `V-52b-E5` follow-up so the table loader can size its buffers and
/// the test harness can sanity-check transcribed coefficient files
/// against Lieske's published counts.
///
/// Counts taken from J. Lieske's `galsat` reference Fortran implementation
/// (Lieske 1998 + JPL Engineering Memorandum 314-112). The same numbers
/// appear in the `lieske_routines.f90` module declarations:
///
/// ```text
///   cxi1(10), cv1(41), cz1(7)    ! Io        (NSAT = 1)
///   cxi2(24), cv2(66), cz2(11)   ! Europa    (NSAT = 2)
///   cxi3(31), cv3(75), cz3(13)   ! Ganymede  (NSAT = 3)
///   cxi4(49), cv4(89), cz4(18)   ! Callisto  (NSAT = 4)
/// ```
///
/// `xi` is the radial perturbation of the semi-major axis, `V` is the
/// longitude perturbation around the mean motion, and `zeta` is the
/// out-of-orbital-plane perturbation (Lieske 1977, eqs. 1-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoonSeriesShape {
    /// Number of `xi` (radial perturbation) terms.
    pub xi_terms: usize,
    /// Number of `V` (longitude perturbation) terms.
    pub v_terms: usize,
    /// Number of `zeta` (out-of-plane perturbation) terms.
    pub zeta_terms: usize,
}

impl MoonSeriesShape {
    /// Lookup table indexed by [`GalileanMoon`]. Mirrors the `galsat`
    /// Fortran declarations exactly so the future coefficient parser can
    /// assert against these numbers when it loads a transcribed table.
    pub const fn for_moon(moon: GalileanMoon) -> Self {
        match moon {
            GalileanMoon::Io => Self {
                xi_terms: 10,
                v_terms: 41,
                zeta_terms: 7,
            },
            GalileanMoon::Europa => Self {
                xi_terms: 24,
                v_terms: 66,
                zeta_terms: 11,
            },
            GalileanMoon::Ganymede => Self {
                xi_terms: 31,
                v_terms: 75,
                zeta_terms: 13,
            },
            GalileanMoon::Callisto => Self {
                xi_terms: 49,
                v_terms: 89,
                zeta_terms: 18,
            },
        }
    }

    /// Total trigonometric term count for this moon.
    pub const fn total_terms(self) -> usize {
        self.xi_terms + self.v_terms + self.zeta_terms
    }
}

/// Lieske 1998 reference epoch for the `t = JD - T_REF` argument shared by
/// every trigonometric term: `T_REF = 2_443_000.5` JD = 1976-Aug-10.0 TDB.
///
/// Exposed publicly so the `V-52b-E5` follow-up can wire the coefficient
/// evaluator without duplicating the constant.
pub const T_REF_JD: f64 = 2_443_000.5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_shape_matches_lieske_galsat_counts() {
        // Pinned per-moon (xi, V, zeta) term counts. Sourced from
        // Lieske 1998 / Jay Lieske's `galsat` reference Fortran. The
        // `V-52b-E5` coefficient transcription must reproduce these
        // numbers exactly — failing this test means the upstream table
        // shape has drifted from the historical record.
        let io = MoonSeriesShape::for_moon(GalileanMoon::Io);
        assert_eq!((io.xi_terms, io.v_terms, io.zeta_terms), (10, 41, 7));
        assert_eq!(io.total_terms(), 58);

        let europa = MoonSeriesShape::for_moon(GalileanMoon::Europa);
        assert_eq!(
            (europa.xi_terms, europa.v_terms, europa.zeta_terms),
            (24, 66, 11)
        );
        assert_eq!(europa.total_terms(), 101);

        let ganymede = MoonSeriesShape::for_moon(GalileanMoon::Ganymede);
        assert_eq!(
            (ganymede.xi_terms, ganymede.v_terms, ganymede.zeta_terms),
            (31, 75, 13)
        );
        assert_eq!(ganymede.total_terms(), 119);

        let callisto = MoonSeriesShape::for_moon(GalileanMoon::Callisto);
        assert_eq!(
            (callisto.xi_terms, callisto.v_terms, callisto.zeta_terms),
            (49, 89, 18)
        );
        assert_eq!(callisto.total_terms(), 156);
    }

    #[test]
    fn t_ref_is_lieske_1976_aug_10() {
        // Hard-coded JD for 1976-Aug-10 12:00 TDB (the "10.5" point) is
        // 2_443_001.0; Lieske's reference epoch is the *previous* midnight
        // (1976-Aug-10.0 TDB = JD 2_443_000.5) — pinned here so a future
        // refactor cannot quietly shift it half a day.
        assert!((T_REF_JD - 2_443_000.5).abs() < 1e-9);
    }

    #[test]
    fn jovicentric_offset_returns_finite_values_at_j2000() {
        for moon in GalileanMoon::ALL {
            let offset = jovicentric_offset(moon, crate::J2000_JD);
            assert!(
                offset.east_radii.is_finite() && offset.north_radii.is_finite(),
                "{} offset has non-finite component: {offset:?}",
                moon.name()
            );
            // Sanity bound: even Callisto stays within ≈30 R_J of Jupiter
            // in the sky-plane projection; values an order of magnitude
            // larger would indicate a sign / scale regression in the
            // delegate.
            let r = (offset.east_radii.powi(2) + offset.north_radii.powi(2)).sqrt();
            assert!(
                r < 30.0,
                "{} offset magnitude = {r} R_J is implausibly large",
                moon.name()
            );
        }
    }
}
