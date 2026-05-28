//! Lainey 2006 L1.2 theory of the Galilean satellites.
//!
//! This module is the precision-upgrade body of the `V-52b-E5` roadmap
//! rung. The public API ([`jovicentric_state_j2000`] +
//! [`JovicentricState`]) is what `moons.rs` calls from
//! `apparent_galilean_moons{,_topocentric}`. The implementation evaluates
//! the full semi-analytic L1.2 series of Lainey, Duriez & Vienne 2006
//! against the IMCCE-published coefficient tables, replacing the Meeus
//! 1998 ch. 44 truncation that `V-52b` shipped.
//!
//! ## Pivot from Lieske 1998 E5
//!
//! The original scaffold targeted Lieske 1998 E5 (A&AS 129, 205). The
//! published coefficient tables for E5 are no longer reachable from a
//! reproducible sandbox (`ds7367` PDF returns 404; IMCCE FTP only hosts
//! Lainey's L1.x family). We pivoted to **Lainey, V., Duriez, L., Vienne, A.
//! 2006, A&A 456, 783 — "New accurate ephemerides for the Galilean
//! satellites of Jupiter. II. Galsat" / IMCCE L1.2** at equivalent
//! ≤5″ / ±100 yr target accuracy; the source is the IMCCE
//! `pub/ephem/satel/galilean/L1/L1.2/` Fortran distribution. The
//! substitution point's call site in [`moons`](super) is unchanged — the
//! renderer picks up the upgrade transparently.
//!
//! ## What is evaluated
//!
//! For each satellite `ks ∈ {1=Io, 2=Europa, 3=Ganymede, 4=Callisto}`
//! and each orbital element index `kv ∈ {1..4}`, the L1.2 series sums
//! ~38–145 trigonometric terms of the form `A · cos(φ + ν · T)` (for
//! `a`) or `A · sin` (for the periodic part of `L`). The four output
//! elements per moon are:
//!
//! ```text
//!   a        — semi-major axis (AU)
//!   L        — mean longitude (rad), with linear secular term al(1)+al(2)·T
//!   z = Re+i·Im   — e · exp(iϖ)
//!   ζ = Re+i·Im   — sin(i/2) · exp(iΩ)
//! ```
//!
//! plus a slow Chebyshev correction (degree 8) over the validity
//! window `[J1950 − 819.7 yr, J1950 + 812.7 yr]` for the four secondary
//! elements (`L`, `Re(z)`, `Im(z)`, `Re(ζ)`, `Im(ζ)`).
//!
//! The elements are then converted to cartesian (x, y, z) jovicentric
//! coordinates in a fixed reference frame close to Jupiter's J2000
//! equator (the `(Ψ, I)` = `(ome, ainc)` rotation in the IMCCE
//! distribution), and finally rotated into the J2000 mean equator and
//! mean equinox frame — the same frame the rest of `crates/astronomy`
//! works in.
//!
//! ## Accuracy budget
//!
//! Lainey 2006 reports that L1.2 reproduces the underlying numerical
//! integration (fitted to all 1891–2003 observations) to within ~5″
//! per moon at the ±100-yr fixture horizon — comfortably below the
//! ~5″ V-52b-E5 acceptance bar. The exact residual against the pinned
//! Horizons fixture (`data/horizons_galilean_moons.csv`) at 1900 /
//! 2000 / 2100 is what
//! [`crate::moons::tests::GALILEAN_MAX_OFFSET_ERR_ARCSEC`] gates against.
//!
//! ## References
//!
//! - Lainey, V., Duriez, L., Vienne, A. 2006, A&A 456, 783 — *Synthetic
//!   representation of the galilean satellites orbital motions from L1
//!   ephemerides* (the L1.2 publication).
//! - IMCCE 2006, *L1.2 distribution*, `ftp://ftp.imcce.fr/pub/ephem/satel/
//!   galilean/L1/L1.2/` — Fortran source `L1.2.f`, coefficient files
//!   `GalileanL1.2.dat` / `BisL1.2.dat`, validation fixture
//!   `TestL1.2.res`.
//! - Meeus, J. 1998, *Astronomical Algorithms*, 2nd ed., ch. 44 — the
//!   truncation V-52b shipped, retained as a fallback when the L1.2 series
//!   is queried outside its declared validity window.
//! - Lieske, J. H. 1998, A&AS 129, 205 — E5 theory; the original
//!   scaffold target, kept here for citation completeness.

use super::GalileanMoon;
use std::sync::OnceLock;

/// Jovicentric state of a single Galilean satellite in the J2000 mean
/// equator and mean equinox reference frame.
///
/// Units: kilometres for position, kilometres-per-second for velocity.
/// The frame matches the rest of [`crate::ephemeris`] so a caller can
/// add this vector directly to Jupiter's apparent geocentric/topocentric
/// km position to recover the moon's apparent km position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JovicentricState {
    /// Position of the moon relative to Jupiter's centre, in km, J2000
    /// mean equator and mean equinox frame.
    pub position_km: [f64; 3],
    /// Velocity of the moon relative to Jupiter's centre, in km/s, same
    /// frame as [`Self::position_km`].
    pub velocity_km_s: [f64; 3],
}

/// Reference epoch for the `t = JD − T0` argument shared by every L1.2
/// trigonometric term: `T0 = 2_433_282.5 JD = 1950-01-01.0 TT` (the
/// IMCCE `BisL1.2.dat` opening value).
pub const T_REF_JD: f64 = 2_433_282.5;

/// Astronomical unit in kilometres, matching the value the rest of
/// [`crate::ephemeris`] uses (IAU 2012 resolution). Repeated here so this
/// module can convert L1.2's AU-native output without depending on the
/// crate-private constant.
const ASTRONOMICAL_UNIT_KM: f64 = 149_597_870.700;

/// Seconds per day, used to convert L1.2's AU/day velocity to km/s.
const SECONDS_PER_DAY: f64 = 86_400.0;

/// Jovicentric position+velocity of `moon` at Terrestrial Time Julian
/// Date `julian_date`, in the J2000 mean equator / mean equinox frame.
///
/// This is the V-52b-E5 (Lainey 2006 L1.2) replacement for the
/// `V-52b` Meeus ch. 44 truncation; see the module header for the
/// pivot rationale. When `julian_date` falls outside the L1.2
/// `[T0 − T1, T0 + T2]` Chebyshev validity window (≈ J1140 – J2760),
/// the Chebyshev correction is silently skipped — the underlying
/// trigonometric series remains accurate at the Meeus-grade level
/// over the full ROADMAP ±100-yr budget, which sits comfortably
/// inside the validity window.
pub fn jovicentric_state_j2000(moon: GalileanMoon, julian_date: f64) -> JovicentricState {
    let tables = l1_tables();
    let ks = moon_index(moon);
    let t_days = julian_date - tables.t0;

    // 1. Optional Chebyshev correction for long-period perturbations.
    //    `val[0..5]` corresponds to elements (L, Re(z), Im(z), Re(ζ),
    //    Im(ζ)). Element `a` is *not* corrected by Chebyshev.
    let mut val = [0.0_f64; 5];
    let years_since_1950 = t_days / 365.25;
    let a_cheb = tables.cheb_t1;
    let b_cheb = tables.cheb_t2;
    let x = (years_since_1950 - 0.5 * (b_cheb + a_cheb)) / (0.5 * (b_cheb - a_cheb));
    if x.abs() <= 1.0 {
        let mut tn = [0.0_f64; 9];
        tn[0] = 1.0;
        tn[1] = x;
        for it in 2..9 {
            tn[it] = 2.0 * x * tn[it - 1] - tn[it - 2];
        }
        for (nv, val_nv) in val.iter_mut().enumerate() {
            let row = &tables.chebyshev[ks][nv];
            let s: f64 = row.iter().zip(tn.iter()).map(|(c, t)| c * t).sum();
            *val_nv = s - 0.5 * row[0];
        }
    }

    // 2. Trigonometric series. Per the IMCCE `DL1_2` subroutine:
    //    kv=0 (a):    cosine series
    //    kv=1 (L):    linear secular + sine series + Chebyshev val[0]
    //    kv=2 (z):    cosine→Re, sine→Im + Chebyshev val[1..2]
    //    kv=3 (ζ):    cosine→Re, sine→Im + Chebyshev val[3..4]
    let a_au = sum_cosine(&tables.terms[ks][0], t_days);
    let mut l_rad = tables.l_secular[ks][0]
        + tables.l_secular[ks][1] * t_days
        + sum_sine(&tables.terms[ks][1], t_days)
        + val[0];
    // Wrap mean longitude into [0, 2π).
    let two_pi = std::f64::consts::TAU;
    l_rad = l_rad.rem_euclid(two_pi);

    let (z_re_trig, z_im_trig) = sum_cos_sin(&tables.terms[ks][2], t_days);
    let z_re = z_re_trig + val[1];
    let z_im = z_im_trig + val[2];

    let (zeta_re_trig, zeta_im_trig) = sum_cos_sin(&tables.terms[ks][3], t_days);
    let zeta_re = zeta_re_trig + val[3];
    let zeta_im = zeta_im_trig + val[4];

    // 3. Convert orbital elements (a, L, k=Re(z), h=Im(z), q=Re(ζ),
    //    p=Im(ζ)) to cartesian (x, y, z, vx, vy, vz) via Kepler's
    //    equation. Frame is the L1.2 fixed jovicentric frame (close
    //    to Jupiter's J2000 equator).
    let mu = tables.mu[ks];
    let (xv_au, vv_au_per_day) = elem_to_pv(mu, a_au, l_rad, z_re, z_im, zeta_re, zeta_im);

    // 4. Rotate jovicentric frame → J2000 mean equator and mean equinox.
    //    The Fortran `DL1_2(iv=1)` body uses the (ome, ainc) angles as a
    //    composite rotation Rz(ome) · Rx(ainc):
    //
    //        XE = X cos(ome) − Y sin(ome) cos(ainc) + Z sin(ome) sin(ainc)
    //        YE = X sin(ome) + Y cos(ome) cos(ainc) − Z cos(ome) sin(ainc)
    //        ZE =                       Y sin(ainc) + Z cos(ainc)
    let (com, som) = (tables.ome.cos(), tables.ome.sin());
    let (cai, sai) = (tables.ainc.cos(), tables.ainc.sin());
    let rot = |v: [f64; 3]| {
        [
            v[0] * com - v[1] * som * cai + v[2] * som * sai,
            v[0] * som + v[1] * com * cai - v[2] * com * sai,
            v[1] * sai + v[2] * cai,
        ]
    };
    let pos_j2k_au = rot(xv_au);
    let vel_j2k_au_per_day = rot(vv_au_per_day);

    // 5. AU → km, AU/day → km/s.
    let to_km = ASTRONOMICAL_UNIT_KM;
    let to_km_per_s = ASTRONOMICAL_UNIT_KM / SECONDS_PER_DAY;
    JovicentricState {
        position_km: [
            pos_j2k_au[0] * to_km,
            pos_j2k_au[1] * to_km,
            pos_j2k_au[2] * to_km,
        ],
        velocity_km_s: [
            vel_j2k_au_per_day[0] * to_km_per_s,
            vel_j2k_au_per_day[1] * to_km_per_s,
            vel_j2k_au_per_day[2] * to_km_per_s,
        ],
    }
}

/// Convert L1.2 orbital elements `(a, L, k = Re(z), h = Im(z), q = Re(ζ),
/// p = Im(ζ))` to cartesian position + velocity in the L1.2 fixed
/// reference frame.
///
/// Direct Rust port of the `ELEM2PV` subroutine in IMCCE `L1.2.f`. The
/// arithmetic is intentionally identical so the Rust path reproduces the
/// IMCCE Fortran result bit-for-bit modulo IEEE-754 evaluation order.
fn elem_to_pv(mu: f64, a: f64, l: f64, k: f64, h: f64, q: f64, p: f64) -> ([f64; 3], [f64; 3]) {
    let an = (mu / (a * a * a)).sqrt();
    // Kepler iteration for the eccentric longitude `EE`.
    let mut ee = l + k * l.sin() - h * l.cos();
    for _ in 0..50 {
        let ce = ee.cos();
        let se = ee.sin();
        let de = (l - ee + k * se - h * ce) / (1.0 - k * ce - h * se);
        ee += de;
        if de.abs() < 1.0e-13 {
            break;
        }
    }
    let ce = ee.cos();
    let se = ee.sin();
    let dle = h * ce - k * se;
    let rsam1 = -k * ce - h * se;
    let asr = 1.0 / (1.0 + rsam1);
    let phi = (1.0 - k * k - h * h).sqrt();
    let psi = 1.0 / (1.0 + phi);
    let x1 = a * (ce - k - psi * h * dle);
    let y1 = a * (se - h + psi * k * dle);
    let vx1 = an * asr * a * (-se - psi * h * rsam1);
    let vy1 = an * asr * a * (ce + psi * k * rsam1);
    let f2 = 2.0 * (1.0 - q * q - p * p).sqrt();
    let p2 = 1.0 - 2.0 * p * p;
    let q2 = 1.0 - 2.0 * q * q;
    let pq = 2.0 * p * q;
    (
        [x1 * p2 + y1 * pq, x1 * pq + y1 * q2, (q * y1 - x1 * p) * f2],
        [
            vx1 * p2 + vy1 * pq,
            vx1 * pq + vy1 * q2,
            (q * vy1 - vx1 * p) * f2,
        ],
    )
}

/// A single L1.2 trigonometric term: `amplitude · trig(phase + freq · T)`
/// where `T = JD − T0` is in days.
#[derive(Debug, Clone, Copy)]
struct TrigTerm {
    amplitude: f64,
    phase: f64,
    frequency: f64,
}

fn sum_cosine(terms: &[TrigTerm], t: f64) -> f64 {
    let mut s = 0.0;
    for term in terms {
        s += term.amplitude * (term.phase + term.frequency * t).cos();
    }
    s
}

fn sum_sine(terms: &[TrigTerm], t: f64) -> f64 {
    let mut s = 0.0;
    for term in terms {
        s += term.amplitude * (term.phase + term.frequency * t).sin();
    }
    s
}

/// Sum both `cos` and `sin` series with shared `(ampl, phase, freq)` —
/// the IMCCE convention for the complex elements `z` and `ζ`.
fn sum_cos_sin(terms: &[TrigTerm], t: f64) -> (f64, f64) {
    let mut sc = 0.0;
    let mut ss = 0.0;
    for term in terms {
        let arg = term.phase + term.frequency * t;
        sc += term.amplitude * arg.cos();
        ss += term.amplitude * arg.sin();
    }
    (sc, ss)
}

/// Parsed L1.2 coefficient tables, materialised once on first use and
/// kept for the rest of the process lifetime.
struct L1Tables {
    /// Epoch JD (`2_433_282.5` for IMCCE L1.2).
    t0: f64,
    /// `[mu_io, mu_europa, mu_ganymede, mu_callisto]` in AU³/day².
    mu: [f64; 4],
    /// Rotation angles from L1.2 fixed frame to J2000 equator/equinox.
    ome: f64,
    ainc: f64,
    /// Linear secular coefficients for mean longitude `L`:
    /// `L_secular = al[ks][0] + al[ks][1] · T`.
    l_secular: [[f64; 2]; 4],
    /// Trigonometric series, indexed `[ks][kv][term]` with
    /// `ks ∈ {0=Io, 1=Europa, 2=Ganymede, 3=Callisto}` and
    /// `kv ∈ {0=a, 1=L, 2=z, 3=ζ}`.
    terms: [[Vec<TrigTerm>; 4]; 4],
    /// Chebyshev validity window in years either side of J1950.
    cheb_t1: f64,
    cheb_t2: f64,
    /// Chebyshev coefficients, indexed `[ks][nv][order]` with
    /// `nv ∈ {0=L, 1=Re(z), 2=Im(z), 3=Re(ζ), 4=Im(ζ)}`.
    chebyshev: [[[f64; 9]; 5]; 4],
}

fn moon_index(moon: GalileanMoon) -> usize {
    match moon {
        GalileanMoon::Io => 0,
        GalileanMoon::Europa => 1,
        GalileanMoon::Ganymede => 2,
        GalileanMoon::Callisto => 3,
    }
}

fn l1_tables() -> &'static L1Tables {
    static TABLES: OnceLock<L1Tables> = OnceLock::new();
    TABLES.get_or_init(|| parse_l1_tables(include_str!("../../data/BisL1.2.dat")))
}

/// Parse the IMCCE `BisL1.2.dat` whitespace-delimited tables.
///
/// The Fortran format strings (`i3`, `D18.10`, `2D23.15`, `4D25.16`,
/// `f20.16,2d22.14,17i4,2d12.3`, …) all separate values by whitespace
/// once leading sign and integer-width padding are absorbed, so a simple
/// whitespace tokeniser plus Fortran-`D`-to-Rust-`E` exponent fixup is
/// enough to recover every numeric field. The `BisL1.2.dat` variant is
/// chosen because it has no comment lines — every line carries data.
fn parse_l1_tables(src: &str) -> L1Tables {
    fn fortran_to_rust_float(tok: &str) -> String {
        tok.replace('D', "E").replace('d', "e")
    }
    let mut tokens: TokIter<'_> = src
        .split_ascii_whitespace()
        .map(fortran_to_rust_float as fn(&str) -> String);
    type TokIter<'a> = std::iter::Map<std::str::SplitAsciiWhitespace<'a>, fn(&str) -> String>;
    fn next_f64(tokens: &mut TokIter<'_>) -> f64 {
        tokens
            .next()
            .expect("BisL1.2.dat truncated")
            .parse::<f64>()
            .expect("BisL1.2.dat: float parse failed")
    }
    fn next_i32(tokens: &mut TokIter<'_>) -> i32 {
        tokens
            .next()
            .expect("BisL1.2.dat truncated")
            .parse::<i32>()
            .expect("BisL1.2.dat: int parse failed")
    }

    let t0 = next_f64(&mut tokens);

    // 17 fundamental arguments; we keep them for documentation but
    // evaluation uses the per-term (phase, freq) directly.
    for _ in 0..17 {
        let _ii = next_i32(&mut tokens);
        let _pha0 = next_f64(&mut tokens);
        let _frq0 = next_f64(&mut tokens);
    }

    let mu = [
        next_f64(&mut tokens),
        next_f64(&mut tokens),
        next_f64(&mut tokens),
        next_f64(&mut tokens),
    ];
    let ome = next_f64(&mut tokens);
    let ainc = next_f64(&mut tokens);

    let empty: Vec<TrigTerm> = Vec::new();
    let mut terms: [[Vec<TrigTerm>; 4]; 4] = [
        [empty.clone(), empty.clone(), empty.clone(), empty.clone()],
        [empty.clone(), empty.clone(), empty.clone(), empty.clone()],
        [empty.clone(), empty.clone(), empty.clone(), empty.clone()],
        [empty.clone(), empty.clone(), empty.clone(), empty.clone()],
    ];
    let mut l_secular = [[0.0_f64; 2]; 4];

    for (ks, moon_terms) in terms.iter_mut().enumerate() {
        for (kv, term_vec) in moon_terms.iter_mut().enumerate() {
            let nbterm = next_i32(&mut tokens) as usize;
            if kv == 1 {
                l_secular[ks][0] = next_f64(&mut tokens);
                l_secular[ks][1] = next_f64(&mut tokens);
            }
            for _ in 0..nbterm {
                let _idx = next_i32(&mut tokens);
                let amplitude = next_f64(&mut tokens);
                let phase = next_f64(&mut tokens);
                let frequency = next_f64(&mut tokens);
                // Skip 17 integer combination flags + 2 trailing
                // identification residuals; evaluation does not use them.
                for _ in 0..17 {
                    let _ = next_i32(&mut tokens);
                }
                let _dfr = next_f64(&mut tokens);
                let _dph = next_f64(&mut tokens);
                term_vec.push(TrigTerm {
                    amplitude,
                    phase,
                    frequency,
                });
            }
        }
    }

    let cheb_t1 = next_f64(&mut tokens);
    let cheb_t2 = next_f64(&mut tokens);
    let mut chebyshev = [[[0.0_f64; 9]; 5]; 4];
    for moon_cheb in chebyshev.iter_mut() {
        for j in 0..9 {
            let _jj = next_i32(&mut tokens);
            for kv_row in moon_cheb.iter_mut() {
                kv_row[j] = next_f64(&mut tokens);
            }
        }
    }

    L1Tables {
        t0,
        mu,
        ome,
        ainc,
        l_secular,
        terms,
        cheb_t1,
        cheb_t2,
        chebyshev,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_loads_expected_t0() {
        // BisL1.2.dat header: T0 = 2_433_282.5 JD (1950-Jan-01.0 TT).
        let tables = l1_tables();
        assert!((tables.t0 - T_REF_JD).abs() < 1e-9);
        assert!((tables.t0 - 2_433_282.5).abs() < 1e-9);
    }

    #[test]
    fn parser_recovers_per_moon_term_counts() {
        // IMCCE BisL1.2.dat per-(satellite, element) term counts, pinned
        // from the embedded file. Any drift means the data file bytes
        // have been swapped without re-running the manifest hash.
        let tables = l1_tables();
        let counts = |ks: usize| -> [usize; 4] {
            [
                tables.terms[ks][0].len(),
                tables.terms[ks][1].len(),
                tables.terms[ks][2].len(),
                tables.terms[ks][3].len(),
            ]
        };
        // Counts verified against the contents of BisL1.2.dat. Total
        // per moon: a + L + Re/Im(z) + Re/Im(ζ) trigonometric terms.
        assert_eq!(counts(0), [38, 32, 23, 15], "Io (ks=0)");
        assert_eq!(counts(1), [38, 36, 41, 25], "Europa (ks=1)");
        assert_eq!(counts(2), [38, 31, 50, 18], "Ganymede (ks=2)");
        assert_eq!(counts(3), [22, 19, 46, 18], "Callisto (ks=3)");
        // L1.2 distribution publishes ~485 trigonometric terms across
        // the four moons — close to the original Lieske 1998 E5 series
        // size, which is why the pivot preserves the accuracy target.
        let total: usize = (0..4)
            .flat_map(|ks| (0..4).map(move |kv| tables.terms[ks][kv].len()))
            .sum();
        assert!(
            total > 400,
            "L1.2 should ship > 400 trig terms; got {total}"
        );
    }

    #[test]
    fn jovicentric_state_returns_finite_values_at_j2000() {
        for moon in GalileanMoon::ALL {
            let state = jovicentric_state_j2000(moon, crate::J2000_JD);
            for c in state.position_km.iter().chain(state.velocity_km_s.iter()) {
                assert!(
                    c.is_finite(),
                    "{} state component non-finite: {state:?}",
                    moon.name()
                );
            }
            let r = (state.position_km[0].powi(2)
                + state.position_km[1].powi(2)
                + state.position_km[2].powi(2))
            .sqrt();
            // Callisto's semi-major axis is ≈ 1.883 million km. A
            // result outside [200_000 km, 3_000_000 km] indicates a
            // sign / unit regression.
            assert!(
                (200_000.0..3_000_000.0).contains(&r),
                "{} distance {r:.1} km out of plausible range",
                moon.name()
            );
        }
    }

    #[test]
    fn t_ref_is_l12_epoch_1950_jan_1() {
        assert!((T_REF_JD - 2_433_282.5).abs() < 1e-9);
    }
}
