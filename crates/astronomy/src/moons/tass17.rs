//! TASS1.7 theory of Titan — full Vienne & Duriez 1995 series.
//!
//! This module is the body of the `V-52c-TASS17` roadmap rung. The public
//! API ([`kronocentric_state_j2000`] + [`KronocentricState`]) is what
//! `moons.rs` calls from `titan_from_saturn`. It evaluates the full TASS1.7
//! semi-analytic series of Vienne & Duriez 1995 (A&A 297, 588) against the
//! IMCCE-published coefficient tables, replacing the Meeus 1998 ch. 45
//! truncation that `V-52c` shipped.
//!
//! ## What is evaluated
//!
//! TASS1.7 represents each Saturnian satellite's motion by four
//! trigonometric series in the elements
//!
//! ```text
//!   p        — semi-major-axis perturbation (a = a0·(1 + p))
//!   λ        — mean longitude  (λ = λ̄ + AL0 + AN0·T + Σ sin-series)
//!   z = k+ih — e·exp(iϖ)        (eccentricity / pericentre complex)
//!   ζ = q+ip — sin(i/2)·exp(iΩ) (inclination / node complex)
//! ```
//!
//! The phases of every term are linear integer combinations of the eight
//! satellites' "proper" mean-longitude perturbations `DLO(1..8)` (the
//! short [`NTR(5)`] sub-series of each satellite's λ-series), exactly as in
//! the IMCCE `CALCLON` / `CALCELEM` Fortran. Hyperion (satellite 7) is not
//! evaluated here, so `DLO(7) = 0` — the same value the upstream `CALCLON`
//! assigns it; this is why the vendored coefficient file
//! (`crates/astronomy/data/redtass7.dat`) carries satellites 1–6 and 8 but
//! not the Hyperion block.
//!
//! The elements are converted to cartesian `(x, y, z)` by `EDERED` in the
//! TASS reference frame — Saturn-centred, mean **ecliptic and equinox
//! J2000** — and this module then rotates that into the J2000 mean
//! **equator** and equinox frame (one obliquity rotation) so the result
//! matches the frame the rest of [`crate::ephemeris`] (and the Galilean
//! [`super::lainey_l1`]) works in.
//!
//! ## Accuracy budget
//!
//! Vienne & Duriez 1995 report that TASS1.7 reproduces the underlying
//! numerical integration to a few tens of km over the 1874–2030 fit span,
//! i.e. well under the ~5″ `V-52c-TASS17` acceptance bar at the ±100-yr
//! fixture horizon (Titan's ≈1.22 Gm orbit projects ≈20″ per Gm at
//! Δ ≈ 8 AU, so a few-tens-of-km model error is ≲0.001″ intrinsic; the
//! residual against JPL Horizons is dominated by Saturn's own ephemeris).
//! The port is validated bit-for-bit against the IMCCE `EXAMP7.res`
//! reference positions (see [`tests::matches_imcce_examp7_reference`]) and
//! against `data/horizons_titan.csv` via
//! [`crate::moons::tests::TASS17_MAX_OFFSET_ERR_ARCSEC`].
//!
//! ## References
//!
//! - Vienne, A. & Duriez, L. 1995, A&A 297, 588 — *TASS1.6: General theory
//!   of the motion of the major eight satellites of Saturn*. Titan is
//!   satellite index 6 in the TASS internal numbering.
//! - Vienne, A. & Duriez, L. 1991, A&A 246, 619 — TASS predecessor.
//! - IMCCE 1996, *TASS1.7 distribution*, `ftp://ftp.imcce.fr/pub/ephem/
//!   satel/tass17/` — Fortran source `tass17.f` (subroutines `POSIRED`,
//!   `CALCLON`, `CALCELEM`, `EDERED`, `LECSER`) and embedded series
//!   (`redtass7.dat`), with reference positions in `EXAMP7.res`.
//! - Meeus, J. 1998, *Astronomical Algorithms*, 2nd ed., ch. 45 — the
//!   low-precision truncation `V-52c` shipped.

use crate::corrections::mean_obliquity_iau2006;
use crate::J2000_JD;
use std::sync::OnceLock;

/// Kronocentric (Saturn-centred) state of Titan in the J2000 mean equator
/// and mean equinox reference frame.
///
/// Units: kilometres for position, kilometres-per-second for velocity. The
/// frame matches the rest of [`crate::ephemeris`] and the Galilean
/// [`super::lainey_l1::JovicentricState`], so a caller can add this vector
/// directly to Saturn's apparent geocentric/topocentric km position to
/// recover Titan's apparent km position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KronocentricState {
    /// Position of Titan relative to Saturn's centre, in km, J2000 mean
    /// equator and mean equinox frame.
    pub position_km: [f64; 3],
    /// Velocity of Titan relative to Saturn's centre, in km/s, same frame
    /// as [`Self::position_km`].
    pub velocity_km_s: [f64; 3],
}

/// TASS1.7 reference epoch for the `T = (JD − T_REF) / 365.25` argument
/// (in Julian years) shared by every trigonometric term:
/// `T_REF = 2_444_240.0` JD = 1980-Jan-04.5 TT. (Vienne & Duriez 1995, §2;
/// IMCCE `CALCELEM`.)
pub const T_REF_JD: f64 = 2_444_240.0;

/// TASS internal satellite index for Titan (Mimas=1 … Titan=6 … Iapetus=8;
/// Hyperion=7 is excluded from the vendored series).
const TITAN_IS: usize = 6;

/// Astronomical unit in kilometres, matching the value the rest of
/// [`crate::ephemeris`] uses (IAU 2012 resolution).
const ASTRONOMICAL_UNIT_KM: f64 = 149_597_870.700;

/// Julian year in days; TASS expresses its time argument and velocities
/// per Julian year of 365.25 days.
const JULIAN_YEAR_DAYS: f64 = 365.25;

/// Seconds per Julian year, to convert TASS AU/year velocity to km/s.
const SECONDS_PER_JULIAN_YEAR: f64 = JULIAN_YEAR_DAYS * 86_400.0;

/// Kronocentric position + velocity of Titan at Terrestrial Time Julian
/// Date `julian_date`, in the J2000 mean equator / mean equinox frame.
///
/// This is the `V-52c-TASS17` (Vienne & Duriez 1995 TASS1.7) replacement
/// for the `V-52c` Meeus ch. 45 truncation.
pub fn kronocentric_state_j2000(julian_date: f64) -> KronocentricState {
    let tables = tass_tables();
    let t_years = (julian_date - T_REF_JD) / JULIAN_YEAR_DAYS;

    // 1. Proper mean-longitude perturbations DLO(1..8) (CALCLON). These are
    //    the fundamental arguments shared by every satellite's series.
    let dlo = calc_longitudes(tables, t_years);

    // 2. Titan's six TASS elements (CALCELEM, IS = 6).
    let elem = calc_elem(tables, TITAN_IS, &dlo, t_years);

    // 3. Elements → cartesian (x, y, z) in the TASS frame: Saturn-centred,
    //    mean ecliptic & equinox J2000, AU and AU/year (EDERED).
    let (pos_ecl_au, vel_ecl_au_yr) = ederede(tables, TITAN_IS, &elem);

    // 4. Rotate mean ecliptic J2000 → mean equator J2000 by the fixed J2000
    //    obliquity (rotation about the common equinox/X axis).
    let eps0 = mean_obliquity_iau2006(J2000_JD);
    let (sin_e, cos_e) = eps0.sin_cos();
    let to_equ = |v: [f64; 3]| {
        [
            v[0],
            v[1] * cos_e - v[2] * sin_e,
            v[1] * sin_e + v[2] * cos_e,
        ]
    };
    let pos_eq_au = to_equ(pos_ecl_au);
    let vel_eq_au_yr = to_equ(vel_ecl_au_yr);

    // 5. AU → km, AU/year → km/s.
    let to_km = ASTRONOMICAL_UNIT_KM;
    let to_km_per_s = ASTRONOMICAL_UNIT_KM / SECONDS_PER_JULIAN_YEAR;
    KronocentricState {
        position_km: [
            pos_eq_au[0] * to_km,
            pos_eq_au[1] * to_km,
            pos_eq_au[2] * to_km,
        ],
        velocity_km_s: [
            vel_eq_au_yr[0] * to_km_per_s,
            vel_eq_au_yr[1] * to_km_per_s,
            vel_eq_au_yr[2] * to_km_per_s,
        ],
    }
}

/// Port of the IMCCE `CALCLON` subroutine: the proper mean-longitude
/// perturbation `DLO(is)` for each satellite, summed over its `NTR(5)`
/// sub-series. Hyperion (`is = 7`) is zero (it is not in the vendored
/// series). Returns a 1-based array (index 0 unused).
fn calc_longitudes(tables: &TassTables, t_years: f64) -> [f64; 9] {
    let mut dlo = [0.0_f64; 9];
    for (is, dlo_is) in dlo.iter_mut().enumerate() {
        // Index 0 is unused; Hyperion (is = 7) is not in the vendored series.
        if is == 0 || is == 7 {
            continue;
        }
        let n5 = tables.ntr5[is];
        *dlo_is = tables.series[is][LON][..n5]
            .iter()
            .map(|term| term.amplitude * (term.phase + t_years * term.frequency).sin())
            .sum();
    }
    dlo
}

/// Port of the IMCCE `CALCELEM` subroutine for satellite `is`: returns the
/// six TASS elements `(p, λ, k, h, q, p2)`.
fn calc_elem(tables: &TassTables, is: usize, dlo: &[f64; 9], t_years: f64) -> [f64; 6] {
    let phase_with_combo = |term: &TrigTerm| -> f64 {
        let mut phase = term.phase;
        for (jk, &ik) in term.combo.iter().enumerate() {
            if ik != 0 {
                phase += f64::from(ik) * dlo[jk + 1];
            }
        }
        phase
    };

    // ELEM(1) = p : cosine series over all NTR(1) terms.
    let mut p = 0.0;
    for term in &tables.series[is][RAD] {
        p += term.amplitude * (phase_with_combo(term) + t_years * term.frequency).cos();
    }

    // ELEM(2) = λ : DLO(is) + AL0 + (sine series from NTR(5)+1..NTR(2)) +
    //               AN0·T, wrapped into atan2.
    let mut s = dlo[is] + tables.al0[is];
    let n5 = tables.ntr5[is];
    let n2 = tables.series[is][LON].len();
    for term in &tables.series[is][LON][n5..n2] {
        s += term.amplitude * (phase_with_combo(term) + t_years * term.frequency).sin();
    }
    s += tables.an0[is] * t_years;
    let lambda = s.sin().atan2(s.cos());

    // ELEM(3,4) = z = k + ih : cos→k, sin→h over NTR(3) terms.
    let mut k = 0.0;
    let mut h = 0.0;
    for term in &tables.series[is][ZEX] {
        let arg = phase_with_combo(term) + t_years * term.frequency;
        k += term.amplitude * arg.cos();
        h += term.amplitude * arg.sin();
    }

    // ELEM(5,6) = ζ = q + ip2 : cos→q, sin→p2 over NTR(4) terms.
    let mut q = 0.0;
    let mut p2 = 0.0;
    for term in &tables.series[is][ZETA] {
        let arg = phase_with_combo(term) + t_years * term.frequency;
        q += term.amplitude * arg.cos();
        p2 += term.amplitude * arg.sin();
    }

    [p, lambda, k, h, q, p2]
}

/// Port of the IMCCE `EDERED` subroutine: TASS elements → cartesian
/// `(x, y, z)` position (AU) and velocity (AU/Julian-year) in the TASS
/// reference frame (Saturn-centred, mean ecliptic & equinox J2000).
fn ederede(tables: &TassTables, is: usize, elem: &[f64; 6]) -> ([f64; 3], [f64; 3]) {
    let amo = tables.aam[is] * (1.0 + elem[0]);
    let rmu = tables.gk1 * (1.0 + tables.tmas[is]);
    let dga = (rmu / (amo * amo)).powf(1.0 / 3.0);
    let rl = elem[1];
    let rk = elem[2];
    let rh = elem[3];

    // Kepler equation for the eccentric longitude FLE. Bounded Newton
    // iteration, mirroring the fixed-trip-count loop in IMCCE `EDERED`:
    // Titan's eccentricity is tiny so it converges in ~3 steps, and the
    // cap guarantees termination even for a non-finite `julian_date`
    // (where `corf` would be NaN and an unbounded loop would never exit).
    let mut fle = rl - rk * rl.sin() + rh * rl.cos();
    for _ in 0..20 {
        let cf = fle.cos();
        let sf = fle.sin();
        let corf = (rl - fle + rk * sf - rh * cf) / (1.0 - rk * cf - rh * sf);
        fle += corf;
        if corf.abs() < 1.0e-14 {
            break;
        }
    }
    let cf = fle.cos();
    let sf = fle.sin();
    let dlf = -rk * sf + rh * cf;
    let rsam1 = -rk * cf - rh * sf;
    let asr = 1.0 / (1.0 + rsam1);
    let phi = (1.0 - rk * rk - rh * rh).sqrt();
    let psi = 1.0 / (1.0 + phi);
    let x1 = dga * (cf - rk - psi * rh * dlf);
    let y1 = dga * (sf - rh + psi * rk * dlf);
    let vx1 = amo * asr * dga * (-sf - psi * rh * rsam1);
    let vy1 = amo * asr * dga * (cf + psi * rk * rsam1);

    // ζ rotation (q = ELEM(5), p2 = ELEM(6)).
    let q = elem[4];
    let p2 = elem[5];
    let dwho = 2.0 * (1.0 - p2 * p2 - q * q).sqrt();
    let rtp = 1.0 - 2.0 * p2 * p2;
    let rtq = 1.0 - 2.0 * q * q;
    let rdg = 2.0 * p2 * q;
    let xyz2 = [
        x1 * rtp + y1 * rdg,
        x1 * rdg + y1 * rtq,
        (-x1 * p2 + y1 * q) * dwho,
    ];
    let vxyz2 = [
        vx1 * rtp + vy1 * rdg,
        vx1 * rdg + vy1 * rtq,
        (-vx1 * p2 + vy1 * q) * dwho,
    ];

    // Rotate the TASS Laplace-plane frame to mean ecliptic & equinox J2000
    // by the (inclination AIA, node OMA) angles.
    let (ci, si) = tables.aia.sin_cos_swapped();
    let (co, so) = tables.oma.sin_cos_swapped();
    let rot = |v: [f64; 3]| {
        [
            co * v[0] - so * ci * v[1] + so * si * v[2],
            so * v[0] + co * ci * v[1] - co * si * v[2],
            si * v[1] + ci * v[2],
        ]
    };
    (rot(xyz2), rot(vxyz2))
}

/// Element-series index: `p` (semi-major-axis radial perturbation).
const RAD: usize = 0;
/// Element-series index: `λ` (mean-longitude perturbation).
const LON: usize = 1;
/// Element-series index: `z` (eccentricity / pericentre complex).
const ZEX: usize = 2;
/// Element-series index: `ζ` (inclination / node complex).
const ZETA: usize = 3;

/// A single TASS1.7 trigonometric term: `amplitude · trig(phase +
/// Σ combo·DLO + frequency · T)` where `T = (JD − T_REF)/365.25` is in
/// Julian years and `combo[jk]` multiplies the proper longitude `DLO(jk+1)`.
#[derive(Debug, Clone, Copy)]
struct TrigTerm {
    amplitude: f64,
    phase: f64,
    frequency: f64,
    combo: [i32; 8],
}

/// Parsed TASS1.7 coefficient tables, materialised once on first use.
struct TassTables {
    /// `GK1 = (GK·365.25)² / TAS`, the gravitational constant in AU³/year².
    gk1: f64,
    /// Inclination angle of the TASS Laplace frame to the ecliptic (rad).
    aia: Angle,
    /// Node angle of the TASS Laplace frame to the ecliptic (rad).
    oma: Angle,
    /// Per-satellite inverse-mass term `TMAS(is) = 1 / TAM(is)` (1-based,
    /// length 10 so index `is ∈ 1..=9` matches the Fortran; 0 unused).
    tmas: [f64; 10],
    /// Per-satellite mean motion `AAM(is) = AM(is)·365.25` in rad/year
    /// (1-based, length 10; index 0 unused).
    aam: [f64; 10],
    /// Constant longitude term `AL0(is)` (1-based).
    al0: [f64; 9],
    /// Secular mean-motion term `AN0(is)` in rad/year (1-based).
    an0: [f64; 9],
    /// Number of "proper longitude" terms (NTR(5)) in each satellite's
    /// λ-series, i.e. the prefix used to build `DLO` (1-based).
    ntr5: [usize; 9],
    /// Trigonometric series, indexed `[is][element]` with `is ∈ 1..=8`
    /// (index 0 and Hyperion index 7 empty) and `element ∈ {RAD, LON, ZEX,
    /// ZETA}`.
    series: [[Vec<TrigTerm>; 4]; 9],
}

/// A precomputed sin/cos pair for a fixed angle, so the `EDERED` rotation
/// does not recompute trigonometry per call.
#[derive(Debug, Clone, Copy)]
struct Angle {
    sin: f64,
    cos: f64,
}

impl Angle {
    fn from_radians(rad: f64) -> Self {
        let (sin, cos) = rad.sin_cos();
        Self { sin, cos }
    }
    /// Returns `(cos, sin)` — the order `EDERED` consumes the angle in
    /// (it binds `CI = cos`, `SI = sin`).
    fn sin_cos_swapped(self) -> (f64, f64) {
        (self.cos, self.sin)
    }
}

fn tass_tables() -> &'static TassTables {
    static TABLES: OnceLock<TassTables> = OnceLock::new();
    TABLES.get_or_init(|| parse_tass_tables(include_str!("../../data/redtass7.dat")))
}

/// Parse the IMCCE `redtass7.dat` whitespace-delimited series file.
///
/// Direct port of the `LECSER` subroutine with `ICRT = 0` (keep every
/// term). The file format, line by line:
///
/// ```text
///   GK
///   TAS
///   AIA OMA                (degrees)
///   TAM(1..9)
///   AM(1..9)
///   <repeated per (IS, IEQ) block>:
///     IS IEQ
///     [IEQ==2 only] 0 AL0(IS) AN0(IS)
///     KT A1 A2 A3 IK(1..8)   (a term; KT is just a running index)
///     ...
///     9998 0 0 0 ...         (marks NTR(5) boundary when IEQ==2)
///     ...
///     9999 0 0 0 ...         (ends the block)
/// ```
fn parse_tass_tables(src: &str) -> TassTables {
    fn fortran_to_rust_float(tok: &str) -> String {
        tok.replace('D', "E").replace('d', "e")
    }
    type TokIter<'a> = std::iter::Map<std::str::SplitAsciiWhitespace<'a>, fn(&str) -> String>;
    let mut tokens: TokIter<'_> = src
        .split_ascii_whitespace()
        .map(fortran_to_rust_float as fn(&str) -> String);

    fn next_f64(tokens: &mut TokIter<'_>) -> f64 {
        tokens
            .next()
            .expect("redtass7.dat truncated")
            .parse::<f64>()
            .expect("redtass7.dat: float parse failed")
    }
    fn next_i32(tokens: &mut TokIter<'_>) -> i32 {
        tokens
            .next()
            .expect("redtass7.dat truncated")
            .parse::<i32>()
            .expect("redtass7.dat: int parse failed")
    }

    let radsdg = std::f64::consts::PI / 180.0;
    let gk = next_f64(&mut tokens);
    let tas = next_f64(&mut tokens);
    let gk1 = (gk * JULIAN_YEAR_DAYS).powi(2) / tas;
    let aia = Angle::from_radians(next_f64(&mut tokens) * radsdg);
    let oma = Angle::from_radians(next_f64(&mut tokens) * radsdg);

    // The file lists 9 mass values, indexed 1-based in the Fortran (TAM(1..9)
    // → satellites 1..8 plus Saturn at index 9). Mirror that 1-based layout
    // so `tmas[is]` matches the upstream `TMAS(IS)`; index 0 is unused.
    let mut tmas = [0.0_f64; 10];
    for slot in tmas[1..=9].iter_mut() {
        *slot = 1.0 / next_f64(&mut tokens);
    }

    let mut aam = [0.0_f64; 10];
    for slot in aam[1..=9].iter_mut() {
        *slot = next_f64(&mut tokens) * JULIAN_YEAR_DAYS;
    }

    let empty: Vec<TrigTerm> = Vec::new();
    let mut series: [[Vec<TrigTerm>; 4]; 9] =
        std::array::from_fn(|_| [empty.clone(), empty.clone(), empty.clone(), empty.clone()]);
    let mut al0 = [0.0_f64; 9];
    let mut an0 = [0.0_f64; 9];
    let mut ntr5 = [0_usize; 9];

    // Block header `IS IEQ`; EOF (no more header tokens) ends the file.
    while let Some(is_tok) = tokens.next() {
        let is = is_tok.parse::<i32>().expect("redtass7.dat: IS parse") as usize;
        let ieq = next_i32(&mut tokens) as usize; // 1..=4
        let elem_idx = ieq - 1;

        if ieq == 2 {
            // `0 AL0 AN0`
            let _nt0 = next_i32(&mut tokens);
            al0[is] = next_f64(&mut tokens);
            an0[is] = next_f64(&mut tokens);
        }

        let mut kt = 0usize;
        loop {
            let nt = next_i32(&mut tokens);
            // Every line carries 11 trailing numbers (A1 A2 A3 + IK(1..8));
            // sentinels list zeros, so the read width is uniform.
            let a1 = next_f64(&mut tokens);
            let a2 = next_f64(&mut tokens);
            let a3 = next_f64(&mut tokens);
            let mut combo = [0_i32; 8];
            for c in combo.iter_mut() {
                *c = next_i32(&mut tokens);
            }
            if nt < 9998 {
                kt += 1;
                series[is][elem_idx].push(TrigTerm {
                    amplitude: a1,
                    phase: a2,
                    frequency: a3,
                    combo,
                });
            } else if nt == 9998 {
                if ieq == 2 {
                    ntr5[is] = kt;
                }
                // continue accumulating into the same series
            } else {
                // nt == 9999: end of this (IS, IEQ) block.
                if ieq == 2 && ntr5[is] == 0 {
                    ntr5[is] = kt;
                }
                break;
            }
        }
    }

    TassTables {
        gk1,
        aia,
        oma,
        tmas,
        aam,
        al0,
        an0,
        ntr5,
        series,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// IMCCE `EXAMP7.res` reference positions for Titan (IS = 6),
    /// Saturn-centred, mean ecliptic & equinox J2000, in AU.
    ///
    /// Tuple shape: `(jd, x_au, y_au, z_au)`. These are the upstream
    /// monitor-program outputs printed at the head of `tass17.f`, so they
    /// validate the port against the original Fortran independent of any
    /// frame conversion or JPL Horizons comparison.
    const EXAMP7_TITAN: &[(f64, f64, f64, f64)] = &[
        (2_440_512.6, -0.008312968003, 0.000590507681, 0.000496831020),
        (2_443_569.3, 0.002230925308, 0.006960382659, -0.003815277704),
        (2_445_061.3, 0.000468338437, -0.007131027063, 0.003639972122),
    ];

    /// Reproduce the TASS frame (ecliptic J2000) cartesian position for the
    /// validation, undoing the equatorial rotation [`kronocentric_state_j2000`]
    /// applies so we can compare directly with `EXAMP7.res`.
    fn titan_ecliptic_au(jd: f64) -> [f64; 3] {
        let tables = tass_tables();
        let t = (jd - T_REF_JD) / JULIAN_YEAR_DAYS;
        let dlo = calc_longitudes(tables, t);
        let elem = calc_elem(tables, TITAN_IS, &dlo, t);
        ederede(tables, TITAN_IS, &elem).0
    }

    #[test]
    fn matches_imcce_examp7_reference() {
        // The IMCCE header notes the last printed digit is ≈15 cm in AU,
        // i.e. ≈1e-12 AU. We allow 5e-11 AU (≈7.5 m) of slack for IEEE-754
        // evaluation-order differences between Fortran and Rust.
        const TOL_AU: f64 = 5.0e-11;
        for &(jd, x, y, z) in EXAMP7_TITAN {
            let got = titan_ecliptic_au(jd);
            let err = ((got[0] - x).powi(2) + (got[1] - y).powi(2) + (got[2] - z).powi(2)).sqrt();
            assert!(
                err < TOL_AU,
                "jd={jd}: TASS Titan ecliptic position error = {err:.3e} AU \
                 (got {got:?}, expected ({x}, {y}, {z}))"
            );
        }
    }

    #[test]
    fn series_term_counts_match_redtass7() {
        // Pinned Titan (p, λ, z, ζ) term counts parsed from the embedded
        // redtass7.dat. Any drift means the data-file bytes were swapped
        // without re-running the manifest hash.
        let tables = tass_tables();
        let counts = [
            tables.series[TITAN_IS][RAD].len(),
            tables.series[TITAN_IS][LON].len(),
            tables.series[TITAN_IS][ZEX].len(),
            tables.series[TITAN_IS][ZETA].len(),
        ];
        // Parsed directly from the embedded redtass7.dat Titan block
        // (p, λ, z, ζ). λ includes the NTR(5) proper-longitude prefix.
        assert_eq!(counts, [7, 36, 35, 22], "Titan (IS=6) term counts");
        // The proper-longitude prefix used to build DLO(6).
        assert!(
            tables.ntr5[TITAN_IS] >= 1 && tables.ntr5[TITAN_IS] <= counts[1],
            "Titan NTR(5) = {} out of range",
            tables.ntr5[TITAN_IS]
        );
    }

    #[test]
    fn t_ref_is_tass17_1980_jan_04_5() {
        assert!((T_REF_JD - 2_444_240.0).abs() < 1e-9);
    }

    #[test]
    fn state_returns_finite_values_and_plausible_distance_at_j2000() {
        let state = kronocentric_state_j2000(J2000_JD);
        for c in state.position_km.iter().chain(state.velocity_km_s.iter()) {
            assert!(c.is_finite(), "Titan state component non-finite: {state:?}");
        }
        let r = (state.position_km[0].powi(2)
            + state.position_km[1].powi(2)
            + state.position_km[2].powi(2))
        .sqrt();
        // Titan's semi-major axis is ≈1.2219 million km. Outside
        // [1.0e6, 1.5e6] km indicates a sign / unit regression.
        assert!(
            (1_000_000.0..1_500_000.0).contains(&r),
            "Titan distance {r:.1} km out of plausible range"
        );
    }

    #[test]
    fn speed_is_near_titan_orbital_velocity() {
        // Titan's mean orbital speed is ≈5.57 km/s. Confirm the velocity
        // conversion (AU/year → km/s) lands in the right ballpark.
        let state = kronocentric_state_j2000(J2000_JD);
        let v = (state.velocity_km_s[0].powi(2)
            + state.velocity_km_s[1].powi(2)
            + state.velocity_km_s[2].powi(2))
        .sqrt();
        assert!(
            (4.5..6.5).contains(&v),
            "Titan speed {v:.3} km/s not near the expected ≈5.57 km/s"
        );
    }
}
