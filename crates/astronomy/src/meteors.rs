//! V-47 meteor-shower model: radiant catalog, date-dependent ZHR, the
//! Koschack-Rendtel 1990 observed-rate formula, and a deterministic meteor
//! stream for the renderer.
//!
//! # Pipeline
//!
//! 1. **Catalog.** [`IMO_WORKING_LIST`] holds the major annual showers from the
//!    IMO Working List of Visual Meteor Showers: radiant α/δ (J2000) at peak,
//!    peak solar longitude λ☉, maximum ZHR, population index `r`, geocentric
//!    velocity `v∞`, and a solar-longitude activity slope `B`.
//! 2. **Activity profile.** [`zhr_at_solar_longitude`] applies the standard
//!    double-exponential-in-λ☉ profile `ZHR(λ) = ZHR_max · 10^(−B·|Δλ|)`
//!    (Jenniskens 1994), so a session away from a shower's peak sees a
//!    correspondingly lower rate and showers outside their active window go to
//!    zero.
//! 3. **Observed rate.** [`observed_rate_per_hour`] inverts the Koschack &
//!    Rendtel 1990 ZHR reduction to recover the rate a real observer sees:
//!    `n = ZHR · sin(h_R) · r^(lm − 6.5) / F`, where `h_R` is the radiant
//!    altitude, `lm` the stellar limiting magnitude, and `F` the
//!    field-obstruction factor (1 for an unobstructed sky).
//! 4. **Stream.** [`meteor_stream`] draws a deterministic Poisson sample of
//!    individual meteors for a render instant, seeded by `(session seed,
//!    time bin)` so the same JSON session reproduces the same meteor stream on
//!    every host. Each meteor is a great-circle streak radiating away from its
//!    shower radiant, with a magnitude drawn from the population index.
//!
//! # References
//! - Koschack, R., Rendtel, J. 1990, WGN 18, 44 ("Determination of spatial
//!   number density and mass index from visual meteor observations").
//! - Jenniskens, P. 1994, A&A 287, 990 ("Meteor stream activity. I. The
//!   annual streams").
//! - Rendtel, J. et al. (annual), *IMO Meteor Shower Calendar*.
//! - McKinley, D. W. R. 1961, *Meteor Science and Engineering*.

use crate::ephemeris::apparent_sun;
use crate::horizontal::equatorial_to_horizontal;
use crate::observer::Observer;
use crate::time::lmst_radians;

/// Standard reference stellar limiting magnitude for the ZHR definition
/// (Koschack & Rendtel 1990): ZHR is the rate a single observer would see with
/// the radiant at the zenith under a 6.5-magnitude sky.
pub const REFERENCE_LIMITING_MAGNITUDE: f64 = 6.5;

/// One annual meteor shower from the IMO Working List of Visual Meteor Showers.
///
/// Radiant coordinates are J2000 equatorial at the shower's maximum; the
/// renderer uses them directly (radiant drift across the few active nights is
/// well below naked-eye streak resolution). Constants are transcribed from the
/// peer-reviewed references above; see `DATA_SOURCES.md`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeteorShower {
    /// Full shower name.
    pub name: &'static str,
    /// IMO three-letter shower code.
    pub code: &'static str,
    /// Radiant right ascension at maximum, degrees (J2000).
    pub radiant_ra_deg: f64,
    /// Radiant declination at maximum, degrees (J2000).
    pub radiant_dec_deg: f64,
    /// Solar longitude of maximum, degrees (J2000).
    pub peak_solar_longitude_deg: f64,
    /// Zenithal hourly rate at maximum (meteors/hour).
    pub zhr_max: f64,
    /// Population index `r` (ratio of successive magnitude-class counts).
    pub population_index: f64,
    /// Geocentric atmospheric-entry velocity `v∞`, km/s.
    pub velocity_km_s: f64,
    /// Activity-profile slope `B` (per degree of solar longitude) in
    /// `ZHR(λ) = ZHR_max · 10^(−B·|Δλ|)`.
    pub activity_slope_b: f64,
}

/// Major annual meteor showers (IMO Working List). Activity slopes and ZHR are
/// from Jenniskens 1994 and the IMO calendar; radiants are J2000 at maximum.
pub const IMO_WORKING_LIST: &[MeteorShower] = &[
    MeteorShower {
        name: "Quadrantids",
        code: "QUA",
        radiant_ra_deg: 230.1,
        radiant_dec_deg: 49.5,
        peak_solar_longitude_deg: 283.15,
        zhr_max: 120.0,
        population_index: 2.1,
        velocity_km_s: 41.0,
        activity_slope_b: 2.20,
    },
    MeteorShower {
        name: "Lyrids",
        code: "LYR",
        radiant_ra_deg: 271.4,
        radiant_dec_deg: 33.6,
        peak_solar_longitude_deg: 32.32,
        zhr_max: 18.0,
        population_index: 2.1,
        velocity_km_s: 49.0,
        activity_slope_b: 0.22,
    },
    MeteorShower {
        name: "eta Aquariids",
        code: "ETA",
        radiant_ra_deg: 338.0,
        radiant_dec_deg: -1.0,
        peak_solar_longitude_deg: 45.5,
        zhr_max: 50.0,
        population_index: 2.4,
        velocity_km_s: 66.0,
        activity_slope_b: 0.08,
    },
    MeteorShower {
        name: "Perseids",
        code: "PER",
        radiant_ra_deg: 48.0,
        radiant_dec_deg: 58.0,
        peak_solar_longitude_deg: 140.0,
        zhr_max: 100.0,
        population_index: 2.2,
        velocity_km_s: 59.0,
        activity_slope_b: 0.20,
    },
    MeteorShower {
        name: "Orionids",
        code: "ORI",
        radiant_ra_deg: 95.0,
        radiant_dec_deg: 16.0,
        peak_solar_longitude_deg: 208.0,
        zhr_max: 20.0,
        population_index: 2.5,
        velocity_km_s: 66.0,
        activity_slope_b: 0.12,
    },
    MeteorShower {
        name: "Leonids",
        code: "LEO",
        radiant_ra_deg: 152.0,
        radiant_dec_deg: 22.0,
        peak_solar_longitude_deg: 235.27,
        zhr_max: 15.0,
        population_index: 2.5,
        velocity_km_s: 71.0,
        activity_slope_b: 0.55,
    },
    MeteorShower {
        name: "Geminids",
        code: "GEM",
        radiant_ra_deg: 112.3,
        radiant_dec_deg: 32.5,
        peak_solar_longitude_deg: 262.2,
        zhr_max: 120.0,
        population_index: 2.6,
        velocity_km_s: 35.0,
        activity_slope_b: 0.39,
    },
    MeteorShower {
        name: "Ursids",
        code: "URS",
        radiant_ra_deg: 217.0,
        radiant_dec_deg: 76.0,
        peak_solar_longitude_deg: 270.7,
        zhr_max: 10.0,
        population_index: 3.0,
        velocity_km_s: 33.0,
        activity_slope_b: 0.90,
    },
];

/// Geocentric apparent solar longitude (degrees, J2000 ecliptic) at `jd_tt`.
/// The solar-longitude clock is what indexes meteor-shower activity.
pub fn solar_longitude_deg(jd_tt: f64) -> f64 {
    apparent_sun(jd_tt)
        .ecliptic_longitude_rad
        .to_degrees()
        .rem_euclid(360.0)
}

/// Smallest absolute difference between two angles on the 360° circle.
fn angular_diff_deg(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// Date-dependent ZHR for `shower` at solar longitude `lambda_deg` using the
/// Jenniskens 1994 double-exponential activity profile
/// `ZHR(λ) = ZHR_max · 10^(−B·|Δλ|)`.
pub fn zhr_at_solar_longitude(shower: &MeteorShower, lambda_deg: f64) -> f64 {
    let delta = angular_diff_deg(lambda_deg, shower.peak_solar_longitude_deg);
    shower.zhr_max * 10f64.powf(-shower.activity_slope_b * delta)
}

/// Observed meteor rate (meteors/hour) for a shower of zenithal hourly rate
/// `zhr`, with the radiant at altitude `radiant_altitude_rad`, a stellar
/// limiting magnitude `limiting_magnitude`, population index `pop_index`, and a
/// field-obstruction factor `obstruction_f` (1.0 = unobstructed).
///
/// This inverts the Koschack & Rendtel 1990 ZHR reduction
/// `ZHR = n · F · r^(6.5 − lm) / sin(h_R)` for the observed rate `n`:
///
/// ```text
/// n = ZHR · sin(h_R) · r^(lm − 6.5) / F
/// ```
///
/// Returns 0 when the radiant is at or below the horizon.
pub fn observed_rate_per_hour(
    zhr: f64,
    radiant_altitude_rad: f64,
    pop_index: f64,
    limiting_magnitude: f64,
    obstruction_f: f64,
) -> f64 {
    let sin_h = radiant_altitude_rad.sin();
    if sin_h <= 0.0 || zhr <= 0.0 {
        return 0.0;
    }
    let f = obstruction_f.max(1.0);
    let mag_factor = pop_index.powf(limiting_magnitude - REFERENCE_LIMITING_MAGNITUDE);
    zhr * sin_h * mag_factor / f
}

/// Radiant altitude (radians) of `shower` for `observer`.
pub fn radiant_altitude_rad(shower: &MeteorShower, observer: Observer) -> f64 {
    let lst = lmst_radians(observer.time.jd_ut1, observer.longitude_rad);
    let altaz = equatorial_to_horizontal(
        shower.radiant_ra_deg.to_radians(),
        shower.radiant_dec_deg.to_radians(),
        lst,
        observer.latitude_rad,
    );
    altaz.altitude
}

/// A shower active for an observer at one instant, with its current ZHR,
/// radiant altitude, and the observed rate that follows.
#[derive(Debug, Clone, Copy)]
pub struct ActiveShower {
    pub shower: MeteorShower,
    pub zhr_now: f64,
    pub radiant_altitude_rad: f64,
    pub observed_rate_per_hour: f64,
}

/// All catalog showers whose radiant is above the horizon and whose
/// date-dependent observed rate exceeds `min_rate` (meteors/hour), for
/// `observer` at limiting magnitude `limiting_magnitude`.
pub fn active_showers(
    observer: Observer,
    limiting_magnitude: f64,
    min_rate: f64,
) -> Vec<ActiveShower> {
    let lambda = solar_longitude_deg(observer.time.jd_tt);
    IMO_WORKING_LIST
        .iter()
        .filter_map(|shower| {
            let alt = radiant_altitude_rad(shower, observer);
            if alt <= 0.0 {
                return None;
            }
            let zhr_now = zhr_at_solar_longitude(shower, lambda);
            let rate = observed_rate_per_hour(
                zhr_now,
                alt,
                shower.population_index,
                limiting_magnitude,
                1.0,
            );
            if rate < min_rate {
                return None;
            }
            Some(ActiveShower {
                shower: *shower,
                zhr_now,
                radiant_altitude_rad: alt,
                observed_rate_per_hour: rate,
            })
        })
        .collect()
}

/// A single rendered meteor: a great-circle streak in J2000-equatorial unit
/// vectors with a peak visual magnitude.
#[derive(Debug, Clone, Copy)]
pub struct Meteor {
    /// Streak head (apparent start) as a J2000-equatorial unit vector.
    pub start_eq: [f64; 3],
    /// Streak tail (apparent end) as a J2000-equatorial unit vector.
    pub end_eq: [f64; 3],
    /// Peak apparent visual magnitude.
    pub magnitude: f64,
    /// IMO shower code, or `"SPO"` for the sporadic background.
    pub code: &'static str,
}

/// Deterministic 64-bit hash (SplitMix64) used to seed the meteor stream so a
/// `(seed, time bin)` pair reproduces an identical stream on every host.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Uniform `f64` in `[0, 1)` from the RNG state.
fn next_unit(state: &mut u64) -> f64 {
    // 53-bit mantissa for a uniform double.
    (splitmix64(state) >> 11) as f64 / (1u64 << 53) as f64
}

/// Knuth Poisson sampler (deterministic given the RNG state).
fn poisson(lambda: f64, state: &mut u64) -> u32 {
    if lambda <= 0.0 {
        return 0;
    }
    // For large lambda the multiplicative form underflows; the cap below keeps
    // us in the regime where exp(-lambda) is representable, and the meteor cap
    // bounds the visible count anyway.
    let l = (-lambda.min(50.0)).exp();
    let mut k = 0u32;
    let mut p = 1.0;
    loop {
        p *= next_unit(state);
        if p <= l {
            return k;
        }
        k += 1;
        if k > 10_000 {
            return k;
        }
    }
}

/// Build an orthonormal basis `(e1, e2)` spanning the plane perpendicular to
/// the unit vector `n`.
fn perpendicular_basis(n: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    // Pick the world axis least aligned with n to avoid a degenerate cross.
    let helper = if n[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let e1 = normalize(cross(n, helper));
    let e2 = cross(n, e1);
    (e1, e2)
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn unit_vector_ra_dec(ra_rad: f64, dec_rad: f64) -> [f64; 3] {
    let (sra, cra) = ra_rad.sin_cos();
    let (sd, cd) = dec_rad.sin_cos();
    [cd * cra, cd * sra, sd]
}

/// Sample a meteor magnitude from the population index `r`: the cumulative
/// count of meteors brighter than `m` scales as `r^m`, so `m` is drawn by
/// inverting that distribution between a bright cap and the limiting
/// magnitude.
fn sample_magnitude(pop_index: f64, limiting_magnitude: f64, state: &mut u64) -> f64 {
    let m_bright = -2.0_f64;
    let m_faint = limiting_magnitude.max(m_bright + 0.5);
    let r = pop_index.max(1.05);
    let lo = r.powf(m_bright);
    let hi = r.powf(m_faint);
    let u = next_unit(state);
    let target = lo + u * (hi - lo);
    (target.ln() / r.ln()).clamp(m_bright, m_faint)
}

/// Generate one meteor radiating away from `radiant` (unit vector) for the
/// given RNG state, shower code, velocity, and population index.
fn make_meteor(
    radiant: [f64; 3],
    code: &'static str,
    velocity_km_s: f64,
    pop_index: f64,
    limiting_magnitude: f64,
    state: &mut u64,
) -> Meteor {
    let (e1, e2) = perpendicular_basis(radiant);
    // Angular distance of the streak head from the radiant. Meteors are rare
    // very close to the radiant (foreshortened) and spread across the sky out
    // to ~90°; bias toward mid distances.
    let psi = (10.0 + 70.0 * next_unit(state)).to_radians();
    let phi = next_unit(state) * std::f64::consts::TAU;
    let (sp, cp) = phi.sin_cos();
    let tangent = [
        e1[0] * cp + e2[0] * sp,
        e1[1] * cp + e2[1] * sp,
        e1[2] * cp + e2[2] * sp,
    ];
    let (s_psi, c_psi) = psi.sin_cos();
    let start = [
        radiant[0] * c_psi + tangent[0] * s_psi,
        radiant[1] * c_psi + tangent[1] * s_psi,
        radiant[2] * c_psi + tangent[2] * s_psi,
    ];
    // Streak length grows with apparent velocity and with distance from the
    // radiant (head-on meteors near the radiant appear as short points).
    let v_norm = (velocity_km_s / 60.0).clamp(0.3, 1.2);
    let length = ((2.0 + 10.0 * s_psi) * v_norm).to_radians();
    let psi_end = psi + length;
    let (s_end, c_end) = psi_end.sin_cos();
    let end = [
        radiant[0] * c_end + tangent[0] * s_end,
        radiant[1] * c_end + tangent[1] * s_end,
        radiant[2] * c_end + tangent[2] * s_end,
    ];
    Meteor {
        start_eq: normalize(start),
        end_eq: normalize(end),
        magnitude: sample_magnitude(pop_index, limiting_magnitude, state),
        code,
    }
}

/// Deterministic per-render meteor stream.
///
/// The expected number of meteors over `window_seconds` is the sum of the
/// active showers' observed rates (plus a faint sporadic background), scaled by
/// `rate_scale`. A Poisson draw seeded by `(seed, time bin)` — where the time
/// bin is `floor(jd_utc / window_days)` — fixes the count and each meteor's
/// geometry, so the same JSON session reproduces the same stream on every host.
/// At most `max_meteors` are returned.
pub fn meteor_stream(
    observer: Observer,
    limiting_magnitude: f64,
    window_seconds: f64,
    seed: u64,
    rate_scale: f64,
    max_meteors: usize,
) -> Vec<Meteor> {
    if window_seconds <= 0.0 || max_meteors == 0 || rate_scale <= 0.0 {
        return Vec::new();
    }
    let showers = active_showers(observer, limiting_magnitude, 1.0e-3);
    let window_hours = window_seconds / 3600.0;

    // Sporadic background: a low, isotropic rate so the dark sky is never
    // perfectly empty. Roughly an ISO/IMO sporadic ZHR of ~8 reduced for the
    // limiting magnitude (radiant-altitude term absorbed into the isotropic
    // appearance, so no sin(h_R) factor).
    let sporadic_rate = 8.0 * 3.0_f64.powf(limiting_magnitude - REFERENCE_LIMITING_MAGNITUDE) * 0.5;

    let shower_rate: f64 = showers.iter().map(|s| s.observed_rate_per_hour).sum();
    let expected = (shower_rate + sporadic_rate) * window_hours * rate_scale;
    if expected <= 0.0 {
        return Vec::new();
    }

    // Time-binned deterministic seed.
    let window_days = window_seconds / 86_400.0;
    let bin = (observer.time.jd_utc / window_days).floor() as i64;
    let mut state = seed ^ (bin as u64).wrapping_mul(0xD1B5_4A32_D192_ED03) ^ 0x5DEE_CE66_2D2F_AC0B;

    let count = poisson(expected, &mut state).min(max_meteors as u32);
    if count == 0 {
        return Vec::new();
    }

    // Weights for assigning each meteor to a shower or the sporadic source.
    let total_weight = shower_rate + sporadic_rate;
    let mut meteors = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let pick = next_unit(&mut state) * total_weight;
        let mut acc = 0.0;
        let mut chosen: Option<&ActiveShower> = None;
        for s in &showers {
            acc += s.observed_rate_per_hour;
            if pick <= acc {
                chosen = Some(s);
                break;
            }
        }
        let meteor = match chosen {
            Some(s) => {
                let radiant = unit_vector_ra_dec(
                    s.shower.radiant_ra_deg.to_radians(),
                    s.shower.radiant_dec_deg.to_radians(),
                );
                make_meteor(
                    radiant,
                    s.shower.code,
                    s.shower.velocity_km_s,
                    s.shower.population_index,
                    limiting_magnitude,
                    &mut state,
                )
            }
            None => {
                // Sporadic: random whole-sky radiant, steep population index.
                let z = 2.0 * next_unit(&mut state) - 1.0;
                let az = next_unit(&mut state) * std::f64::consts::TAU;
                let r = (1.0 - z * z).max(0.0).sqrt();
                let radiant = [r * az.cos(), r * az.sin(), z];
                make_meteor(radiant, "SPO", 30.0, 3.0, limiting_magnitude, &mut state)
            }
        };
        meteors.push(meteor);
    }
    meteors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::TimeScales;

    fn perseids() -> &'static MeteorShower {
        IMO_WORKING_LIST.iter().find(|s| s.code == "PER").unwrap()
    }

    #[test]
    fn observed_rate_matches_koschack_rendtel_reduction() {
        // Koschack & Rendtel 1990: observed n = ZHR · sin(h_R) · r^(lm − 6.5).
        // Perseids ZHR = 100, radiant altitude 60°, lim mag 6.0, r = 2.2:
        //   sin(60°) = 0.86603, 2.2^(6.0 − 6.5) = 2.2^(−0.5) = 0.67420
        //   n = 100 · 0.86603 · 0.67420 ≈ 58.39 meteors/hour.
        let n = observed_rate_per_hour(100.0, 60f64.to_radians(), 2.2, 6.0, 1.0);
        assert!(
            (n - 58.39).abs() < 5.8,
            "observed rate {n} m/h off reference 58.4"
        );
    }

    #[test]
    fn observed_rate_recovers_zhr_at_standard_conditions() {
        // Radiant at the zenith and a 6.5-mag sky must return exactly ZHR.
        let n = observed_rate_per_hour(
            120.0,
            std::f64::consts::FRAC_PI_2,
            2.6,
            REFERENCE_LIMITING_MAGNITUDE,
            1.0,
        );
        assert!((n - 120.0).abs() < 1e-9);
    }

    #[test]
    fn observed_rate_zero_below_horizon() {
        assert_eq!(observed_rate_per_hour(100.0, -0.1, 2.2, 6.5, 1.0), 0.0);
        assert_eq!(observed_rate_per_hour(100.0, 0.0, 2.2, 6.5, 1.0), 0.0);
    }

    #[test]
    fn activity_profile_peaks_at_maximum_and_decays() {
        let p = perseids();
        let at_peak = zhr_at_solar_longitude(p, p.peak_solar_longitude_deg);
        assert!((at_peak - p.zhr_max).abs() < 1e-9);
        // Five degrees of solar longitude (~5 days) off peak must be lower.
        let off = zhr_at_solar_longitude(p, p.peak_solar_longitude_deg + 5.0);
        assert!(
            off < at_peak && off > 0.0,
            "off-peak {off} vs peak {at_peak}"
        );
        // The wrap-around difference is handled symmetrically.
        let before = zhr_at_solar_longitude(p, p.peak_solar_longitude_deg - 5.0);
        assert!((off - before).abs() < 1e-9);
    }

    #[test]
    fn solar_longitude_advances_through_the_year() {
        // ~late June (Perseid season is mid-August at λ ≈ 140°).
        let jd_aug = 2_460_536.5; // 2024-08-12 ~ Perseid maximum
        let lambda = solar_longitude_deg(jd_aug);
        assert!(
            (lambda - 140.0).abs() < 2.0,
            "solar longitude {lambda} should be ≈140° at Perseid maximum"
        );
    }

    fn observer_at(jd_utc: f64) -> Observer {
        Observer::from_degrees_with_time(35.68, 139.69, TimeScales::from_utc_julian_date(jd_utc))
    }

    #[test]
    fn stream_is_deterministic_for_seed_and_time() {
        let obs = observer_at(2_460_536.9);
        let a = meteor_stream(obs, 6.0, 60.0, 42, 1.0, 64);
        let b = meteor_stream(obs, 6.0, 60.0, 42, 1.0, 64);
        assert_eq!(a.len(), b.len());
        for (m, n) in a.iter().zip(b.iter()) {
            assert_eq!(m.code, n.code);
            assert!((m.magnitude - n.magnitude).abs() < 1e-12);
            assert_eq!(m.start_eq, n.start_eq);
            assert_eq!(m.end_eq, n.end_eq);
        }
    }

    #[test]
    fn stream_differs_with_seed() {
        let obs = observer_at(2_460_536.9);
        let a = meteor_stream(obs, 6.0, 60.0, 1, 1.0, 64);
        let b = meteor_stream(obs, 6.0, 60.0, 2, 1.0, 64);
        // Overwhelmingly likely to differ in count or first-meteor geometry.
        let differ = a.len() != b.len()
            || a.first()
                .zip(b.first())
                .map(|(x, y)| x.start_eq != y.start_eq || x.magnitude != y.magnitude)
                .unwrap_or(true);
        assert!(differ, "different seeds should produce different streams");
    }

    #[test]
    fn stream_respects_cap_and_unit_vectors() {
        let obs = observer_at(2_460_536.9);
        // Huge rate scale would overflow the visible count; the cap holds.
        let stream = meteor_stream(obs, 6.5, 3600.0, 7, 1000.0, 32);
        assert!(stream.len() <= 32);
        for m in &stream {
            let n = (m.start_eq[0].powi(2) + m.start_eq[1].powi(2) + m.start_eq[2].powi(2)).sqrt();
            assert!((n - 1.0).abs() < 1e-9, "start not unit length: {n}");
            let ne = (m.end_eq[0].powi(2) + m.end_eq[1].powi(2) + m.end_eq[2].powi(2)).sqrt();
            assert!((ne - 1.0).abs() < 1e-9, "end not unit length: {ne}");
        }
    }
}
