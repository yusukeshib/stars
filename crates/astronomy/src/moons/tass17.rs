//! TASS1.7 theory of Titan — scaffolding.
//!
//! This module is the substitution point for the `V-52c-TASS17` precision
//! upgrade tracked in `ROADMAP.md`. The public API
//! ([`titan_offset`] + [`TitanOffset`]) is what `moons.rs` calls from
//! `titan_from_saturn`. Today it delegates to the Meeus 1998 ch. 45
//! truncation already wired through the [`astro`] crate so the renderer
//! keeps producing the same Meeus-grade Titan position `V-52c` shipped.
//! The follow-up PR replaces the body of [`titan_offset`] with the full
//! TASS1.7 trigonometric series (loaded from the coefficient tables
//! transcribed in `crates/astronomy/data/tass17/`) without changing the
//! call sites.
//!
//! ## Why scaffolding rather than the full TASS1.7 series in this PR
//!
//! The Vienne & Duriez 1995 paper publishes the full TASS theory of the
//! eight major Saturnian satellites with roughly 100–200 trigonometric
//! coefficients per moon spread across `p` (semi-major-axis radial
//! perturbation), `λ − λ̄` (mean-longitude perturbation), `z` (eccentricity
//! / pericentre complex), and `ζ` (inclination / node complex). The
//! Titan-only subset still carries ~150 published terms across the four
//! series, with integer index pairs encoding which of the secular angles
//! enter each phase. Faithfully transcribing those tables without typo
//! regression requires its own validation matrix — the explicit reason
//! the roadmap split `V-52c` (Meeus-grade engine plumbing) from
//! `V-52c-TASS17` (precision upgrade).
//!
//! This PR ships:
//! 1. The module structure ([`TitanOffset`], [`titan_offset`],
//!    [`TitanSeriesShape`]) the follow-up swaps the coefficients into;
//! 2. A pinned JPL Horizons reference fixture
//!    (`data/horizons_titan.csv`) at three epochs spanning ±100 years —
//!    the bar the precision upgrade has to clear (~5″);
//! 3. A test gate documenting the **current** Meeus-grade error budget
//!    against that fixture so the follow-up tightens a single tolerance
//!    constant instead of inventing a new test harness.
//!
//! ## References
//!
//! - Vienne, A. & Duriez, L. 1995, A&A 297, 588 — TASS1.7 theory of the
//!   eight major Saturnian satellites (the target). Titan is satellite
//!   index 6 in the TASS internal numbering.
//! - Vienne, A. & Duriez, L. 1991, A&A 246, 619 — TASS predecessor
//!   (TASS1.6); the index conventions Titan inherits.
//! - Meeus, J. 1998, *Astronomical Algorithms*, 2nd ed., ch. 45 — the
//!   low-precision truncation actually exercised today via the
//!   [`astro`] crate.
//! - Vienne, A. 2009, *Vienne / Duriez TASS1.7 Fortran reference*
//!   (`TASS17.f`, distributed by the IMCCE) — the canonical coefficient
//!   tables the precision upgrade will transcribe.

/// Titan's planetocentric sky-plane offset, expressed in the same
/// orthonormal `(east, north)` basis the renderer consumes.
///
/// Units: Saturn's equatorial radius (IAU WGCCRE 2015, 60 268 km). The
/// caller scales by `SATURN_EQUATORIAL_RADIUS_KM` to get a physical
/// kilometre offset and combines it with the planet's line-of-sight
/// direction to recover Titan's apparent right ascension / declination.
///
/// The sign convention deliberately matches the Galilean
/// [`super::lieske_e5::JovicentricOffset`] type: east is the direction of
/// increasing right ascension, north the direction of increasing
/// declination. (Meeus's published `(X, Y)` for the Saturnian satellites
/// is east-negative / north-positive in units of `R_S`; the conversion is
/// folded into this type.)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TitanOffset {
    /// Sky-plane offset along the local east direction, in `R_S`.
    /// Positive values mean Titan is east of (greater RA than) Saturn.
    pub east_radii: f64,
    /// Sky-plane offset along the local north direction, in `R_S`.
    /// Positive values mean Titan is north of (greater Dec than) Saturn.
    pub north_radii: f64,
}

/// Apparent planetocentric sky-plane offset of Titan at a TDB Julian Date.
///
/// Today this is the Meeus 1998 ch. 45 truncation routed through the
/// [`astro`] crate. The `V-52c-TASS17` precision upgrade replaces this
/// body with the full TASS1.7 trigonometric series.
pub fn titan_offset(julian_date: f64) -> TitanOffset {
    // TODO(V-52c-TASS17): replace the call below with the full TASS1.7
    // series evaluator. The expected shape is:
    //
    // 1. Compute the secular arguments at `t = julian_date - T_REF` (TASS
    //    uses `T_REF = 2_444_240.0` JD = 1980-Jan-04.5 TT — see
    //    Vienne & Duriez 1995 §2).
    // 2. Evaluate the four trigonometric sums for Titan (TASS internal
    //    index 6): `p` (semi-major-axis perturbation), `λ − λ̄`
    //    (longitude), the complex `z` (eccentricity / pericentre), and
    //    the complex `ζ` (inclination / node).
    // 3. Convert (p, λ, z, ζ) to ecliptic-of-date (x, y, z) using the
    //    Laplace-plane elements `(a₀, n₀, e₀, …)` Vienne & Duriez fit.
    // 4. Rotate by the Laplace-plane → J2000 ICRS matrix and project onto
    //    the observer's sky-plane east / north.
    //
    // Until that lands, fall back to the Meeus truncation — same
    // numerical result the renderer has shipped since V-52c.
    let (x_west_radii, y_north_radii, _z_radii) = astro::planet::saturn::moon::apprnt_rect_coords(
        julian_date,
        &astro::planet::saturn::moon::Moon::Titan,
    );
    TitanOffset {
        east_radii: -x_west_radii,
        north_radii: y_north_radii,
    }
}

/// Shape of the TASS1.7 trigonometric series for Titan, recorded for the
/// `V-52c-TASS17` follow-up so the table loader can size its buffers and
/// the test harness can sanity-check transcribed coefficient files
/// against Vienne & Duriez's published counts.
///
/// Counts taken from Vienne / Duriez's reference TASS17 Fortran
/// implementation (IMCCE distribution, `TASS17.f`, Titan = satellite 6):
///
/// ```text
///   long6: 23 terms        ! λ − λ̄  (mean longitude perturbation)
///   rmu6:   9 terms        ! p      (semi-major-axis radial term)
///   z6:    44 terms        ! z      (eccentricity / pericentre complex)
///   zeta6: 31 terms        ! ζ      (inclination / node complex)
/// ```
///
/// The `z` and `ζ` series are complex-valued; the count above is the
/// number of complex coefficient pairs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TitanSeriesShape {
    /// Number of mean-longitude (`λ − λ̄`) perturbation terms.
    pub longitude_terms: usize,
    /// Number of semi-major-axis (`p`) radial-perturbation terms.
    pub radial_terms: usize,
    /// Number of `z` (eccentricity / pericentre) complex terms.
    pub z_terms: usize,
    /// Number of `ζ` (inclination / node) complex terms.
    pub zeta_terms: usize,
}

impl TitanSeriesShape {
    /// Pinned Titan TASS1.7 series shape (Vienne / Duriez `TASS17.f`,
    /// satellite index 6). The future coefficient parser must assert
    /// against these numbers when it loads a transcribed table.
    pub const TITAN: Self = Self {
        longitude_terms: 23,
        radial_terms: 9,
        z_terms: 44,
        zeta_terms: 31,
    };

    /// Total trigonometric term count for Titan.
    pub const fn total_terms(self) -> usize {
        self.longitude_terms + self.radial_terms + self.z_terms + self.zeta_terms
    }
}

/// TASS1.7 reference epoch for the `t = JD - T_REF` argument shared by
/// every trigonometric term: `T_REF = 2_444_240.0` JD = 1980-Jan-04.5 TT.
/// (Vienne & Duriez 1995, §2.)
///
/// Exposed publicly so the `V-52c-TASS17` follow-up can wire the
/// coefficient evaluator without duplicating the constant.
pub const T_REF_JD: f64 = 2_444_240.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_shape_matches_tass17_titan_counts() {
        // Pinned Titan (λ, p, z, ζ) term counts. Sourced from Vienne /
        // Duriez's reference `TASS17.f` Fortran (IMCCE distribution).
        // The `V-52c-TASS17` coefficient transcription must reproduce
        // these numbers exactly — failing this test means the upstream
        // table shape has drifted from the historical record.
        let shape = TitanSeriesShape::TITAN;
        assert_eq!(shape.longitude_terms, 23);
        assert_eq!(shape.radial_terms, 9);
        assert_eq!(shape.z_terms, 44);
        assert_eq!(shape.zeta_terms, 31);
        assert_eq!(shape.total_terms(), 107);
    }

    #[test]
    fn t_ref_is_tass17_1980_jan_04_5() {
        // Hard-coded JD for 1980-Jan-04 12:00 TT = JD 2_444_240.0 (the
        // TASS reference epoch defined in Vienne & Duriez 1995 §2). Pinned
        // here so a future refactor cannot quietly shift it.
        assert!((T_REF_JD - 2_444_240.0).abs() < 1e-9);
    }

    #[test]
    fn titan_offset_returns_finite_values_at_j2000() {
        let offset = titan_offset(crate::J2000_JD);
        assert!(
            offset.east_radii.is_finite() && offset.north_radii.is_finite(),
            "Titan offset has non-finite component: {offset:?}"
        );
        // Sanity bound: Titan stays within ≈25 R_S of Saturn in the
        // sky-plane projection (its actual semi-major axis is ≈20.3 R_S).
        // Values an order of magnitude larger would indicate a sign /
        // scale regression in the delegate.
        let r = (offset.east_radii.powi(2) + offset.north_radii.powi(2)).sqrt();
        assert!(
            r < 30.0,
            "Titan offset magnitude = {r} R_S is implausibly large"
        );
    }
}
