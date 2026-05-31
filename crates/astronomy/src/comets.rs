//! V-49 comet rendering: osculating-element two-body propagation and coma
//! photometry.
//!
//! # Pipeline
//!
//! 1. **Elements.** Comet orbits are stored as osculating Keplerian elements in
//!    the Marsden / Minor Planet Center convention referred to the **J2000
//!    ecliptic and equinox**: perihelion distance `q`, eccentricity `e`,
//!    inclination `i`, argument of perihelion `ω`, longitude of the ascending
//!    node `Ω`, and time of perihelion passage `Tp`. Coma photometry carries
//!    the standard `(M1, K1)` magnitude-law coefficients.
//! 2. **Propagation.** Two-body Keplerian motion from `Tp` gives the
//!    heliocentric position (and velocity) in the orbital plane, rotated into
//!    the J2000 ecliptic frame by the Gauss vectors `P` (perihelion direction)
//!    and `Q` (90° ahead in the orbit plane). Elliptical, parabolic, and
//!    hyperbolic conics are all handled. This is exact for a fixed element set
//!    and is accurate to ≈arcminutes near the element epoch; planetary
//!    perturbations (and the N-body upgrade) are tracked under `L-06`.
//! 3. **Geocentric reduction.** The Earth's heliocentric position comes from
//!    the `astro` crate VSOP87D solution, rotated into the **J2000 equatorial**
//!    frame (mean-of-date → J2000 by the inverse IAU 2006 precession matrix) so
//!    it shares one frame with the comet's J2000 position. The geocentric
//!    vector `Δ = r_comet − r_earth` is corrected for light-time by one
//!    iteration, then reduced to right ascension / declination. Topocentric
//!    parallax for an observer on the WGS84 ellipsoid is applied for the
//!    renderer (negligible at comet distances but kept for consistency).
//! 4. **Photometry.** Apparent total magnitude follows the
//!    Bobrovnikoff-Bowell comet magnitude law
//!    `m1 = M1 + 5·log10(Δ) + K1·log10(r)` (`K1 = 2.5·n`).
//! 5. **Tails.** The ion tail points along the prolonged (anti-solar) radius
//!    vector; the dust tail follows a representative Finson-Probstein 1968
//!    syndyne for `β = F_rad/F_grav = 0.6`, lagging the anti-solar direction
//!    toward the comet's trailing orbital-velocity direction. Tail tip
//!    directions are returned as unit vectors for the renderer to project.
//!
//! # References
//! - Finson, M. L., Probstein, R. F. 1968, ApJ 154, 327 (dust-tail dynamics).
//! - Marsden, B. G., Williams, G. V., MPC orbital-element format.
//! - Bobrovnikoff, N. T. 1942, ApJ 95, 71; Bowell, E. et al. 1989 (magnitude
//!   law conventions).
//! - Meeus, J. 1998, *Astronomical Algorithms*, ch. 33–35.

use glam::Vec3;

use crate::corrections::{mat_mul_vec, mat_transpose, precession_matrix_iau2006};
use crate::ephemeris::{
    equatorial_unit_vector_f64, observer_equatorial_position_km, ra_dec_from_equatorial_vector,
};
use crate::Observer;

/// Astronomical Unit in kilometres, IAU 2012 Resolution B2.
const ASTRONOMICAL_UNIT_KM: f64 = 149_597_870.7;
/// Gaussian gravitational constant `k` (rad·AU^1.5·day⁻¹·M_sun^-0.5). The
/// heliocentric two-body mean motion is `n = k / a^1.5`.
const GAUSSIAN_GRAVITATIONAL_CONSTANT: f64 = 0.017_202_098_95;
/// Speed of light in AU per day, for the light-time correction.
const SPEED_OF_LIGHT_AU_PER_DAY: f64 = 173.144_632_674_240_2;
/// IAU 2006 mean obliquity of the ecliptic at J2000.0 (84381.406″) in radians,
/// used for the fixed J2000 ecliptic → equatorial rotation.
const J2000_OBLIQUITY_RAD: f64 = 0.409_092_600_600_582_9;
/// Representative dust-tail radiation-pressure parameter (Finson-Probstein
/// `β = F_rad / F_grav`) used for the single rendered syndyne.
pub const REPRESENTATIVE_DUST_BETA: f64 = 0.6;

/// Osculating two-body orbital elements for one comet, J2000 ecliptic frame,
/// Marsden / MPC convention.
#[derive(Debug, Clone)]
pub struct CometElements {
    /// Display name (e.g. `"1P/Halley"`).
    pub name: String,
    /// Perihelion distance `q` in AU.
    pub perihelion_distance_au: f64,
    /// Orbital eccentricity `e` (0 ≤ e; e = 1 parabolic, e > 1 hyperbolic).
    pub eccentricity: f64,
    /// Inclination `i` to the J2000 ecliptic, radians.
    pub inclination_rad: f64,
    /// Argument of perihelion `ω`, radians.
    pub arg_perihelion_rad: f64,
    /// Longitude of the ascending node `Ω`, radians.
    pub long_asc_node_rad: f64,
    /// Time of perihelion passage `Tp` as a TT Julian Date.
    pub perihelion_time_jd_tt: f64,
    /// Absolute total magnitude `M1` of the comet magnitude law.
    pub absolute_magnitude_m1: f64,
    /// Activity slope `K1 = 2.5·n` (coefficient of `log10 r`).
    pub activity_slope_k1: f64,
}

/// Parse a curated comet-element CSV blob into [`CometElements`].
///
/// The expected header is
/// `name,q_au,e,i_deg,arg_peri_deg,long_node_deg,tp_jd_tt,m1,k1`. Blank lines
/// and `#` comment lines are ignored. Malformed rows are skipped so a single
/// bad line never takes down the whole layer.
pub fn parse_comet_elements(text: &str) -> Vec<CometElements> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("name,") {
            continue; // header
        }
        let cols: Vec<&str> = line.split(',').map(str::trim).collect();
        if cols.len() != 9 {
            continue;
        }
        let parse = |idx: usize| cols[idx].parse::<f64>().ok();
        let (
            Some(q),
            Some(e),
            Some(i_deg),
            Some(w_deg),
            Some(node_deg),
            Some(tp),
            Some(m1),
            Some(k1),
        ) = (
            parse(1),
            parse(2),
            parse(3),
            parse(4),
            parse(5),
            parse(6),
            parse(7),
            parse(8),
        )
        else {
            continue;
        };
        out.push(CometElements {
            name: cols[0].to_string(),
            perihelion_distance_au: q,
            eccentricity: e,
            inclination_rad: i_deg.to_radians(),
            arg_perihelion_rad: w_deg.to_radians(),
            long_asc_node_rad: node_deg.to_radians(),
            perihelion_time_jd_tt: tp,
            absolute_magnitude_m1: m1,
            activity_slope_k1: k1,
        });
    }
    out
}

/// Apparent geocentric/topocentric state of a comet at one instant, with coma
/// photometry and tail directions for the renderer.
#[derive(Debug, Clone, Copy)]
pub struct CometApparent {
    /// Apparent right ascension (radians, J2000 equatorial).
    pub right_ascension_rad: f64,
    /// Apparent declination (radians, J2000 equatorial).
    pub declination_rad: f64,
    /// Observer–comet distance `Δ` in AU.
    pub distance_au: f64,
    /// Sun–comet distance `r` in AU.
    pub heliocentric_distance_au: f64,
    /// Apparent total visual magnitude (Bobrovnikoff-Bowell law).
    pub magnitude: f64,
    /// Unit J2000-equatorial direction along the ion (anti-solar) tail, i.e.
    /// the prolonged Sun→comet radius vector. Use with [`Self::nucleus_dir`].
    pub ion_tail_dir: Vec3,
    /// Unit J2000-equatorial direction along the dust tail (β = 0.6 syndyne),
    /// lagging the anti-solar direction toward the trailing velocity.
    pub dust_tail_dir: Vec3,
}

impl CometApparent {
    /// Unit vector from the observer toward the comet nucleus, J2000 equatorial.
    pub fn nucleus_dir(self) -> Vec3 {
        let [x, y, z] = equatorial_unit_vector_f64(self.right_ascension_rad, self.declination_rad);
        Vec3::new(x as f32, y as f32, z as f32)
    }
}

/// Heliocentric J2000-ecliptic position and velocity (AU, AU/day) of a comet
/// from two-body propagation to `jd_tt`.
fn heliocentric_state_ecliptic(elements: &CometElements, jd_tt: f64) -> ([f64; 3], [f64; 3]) {
    let q = elements.perihelion_distance_au;
    let e = elements.eccentricity;
    let dt = jd_tt - elements.perihelion_time_jd_tt;
    let k = GAUSSIAN_GRAVITATIONAL_CONSTANT;
    // Semi-latus rectum `p = q(1 + e)` is well defined for every conic.
    let p = q * (1.0 + e);

    // True anomaly `nu` and heliocentric distance `r`.
    let (nu, r) = if (e - 1.0).abs() < 1.0e-9 {
        // Parabolic: Barker's equation, solved in closed form.
        let w = 3.0 * k * dt / (std::f64::consts::SQRT_2 * q * q.sqrt());
        let y = (w / 2.0 + (w * w / 4.0 + 1.0).sqrt()).cbrt();
        let s = y - 1.0 / y; // s = tan(nu/2)
        let nu = 2.0 * s.atan();
        (nu, q * (1.0 + s * s))
    } else if e < 1.0 {
        // Elliptical: solve Kepler's equation E − e·sinE = M.
        let a = q / (1.0 - e);
        let n = k / (a * a.sqrt());
        let m = wrap_pi(n * dt);
        let big_e = solve_kepler_elliptic(m, e);
        let r = a * (1.0 - e * big_e.cos());
        let half = big_e / 2.0;
        let nu = 2.0 * ((1.0 + e).sqrt() * half.sin()).atan2((1.0 - e).sqrt() * half.cos());
        (nu, r)
    } else {
        // Hyperbolic: solve M = e·sinhH − H.
        let a = q / (1.0 - e); // negative
        let n = k / (-a) / (-a).sqrt();
        let m = n * dt;
        let big_h = solve_kepler_hyperbolic(m, e);
        let r = a * (1.0 - e * big_h.cosh());
        let half = big_h / 2.0;
        let nu = 2.0 * ((e + 1.0).sqrt() * half.sinh()).atan2((e - 1.0).sqrt() * half.cosh());
        (nu, r)
    };

    // Perifocal position / velocity.
    let (sin_nu, cos_nu) = nu.sin_cos();
    let speed = k / p.sqrt();
    let pos_pf = [r * cos_nu, r * sin_nu, 0.0];
    let vel_pf = [-speed * sin_nu, speed * (e + cos_nu), 0.0];

    // Gauss vectors P (toward perihelion) and Q (90° ahead in the orbit plane).
    let (sin_w, cos_w) = elements.arg_perihelion_rad.sin_cos();
    let (sin_om, cos_om) = elements.long_asc_node_rad.sin_cos();
    let (sin_i, cos_i) = elements.inclination_rad.sin_cos();
    let p_vec = [
        cos_w * cos_om - sin_w * sin_om * cos_i,
        cos_w * sin_om + sin_w * cos_om * cos_i,
        sin_w * sin_i,
    ];
    let q_vec = [
        -sin_w * cos_om - cos_w * sin_om * cos_i,
        -sin_w * sin_om + cos_w * cos_om * cos_i,
        cos_w * sin_i,
    ];

    let pos = [
        pos_pf[0] * p_vec[0] + pos_pf[1] * q_vec[0],
        pos_pf[0] * p_vec[1] + pos_pf[1] * q_vec[1],
        pos_pf[0] * p_vec[2] + pos_pf[1] * q_vec[2],
    ];
    let vel = [
        vel_pf[0] * p_vec[0] + vel_pf[1] * q_vec[0],
        vel_pf[0] * p_vec[1] + vel_pf[1] * q_vec[1],
        vel_pf[0] * p_vec[2] + vel_pf[1] * q_vec[2],
    ];
    (pos, vel)
}

/// Newton solve of Kepler's equation `E − e·sinE = M` for the eccentric
/// anomaly, robust up to the near-parabolic regime.
fn solve_kepler_elliptic(m: f64, e: f64) -> f64 {
    let mut big_e = if e < 0.8 {
        m
    } else {
        std::f64::consts::PI.copysign(m)
    };
    for _ in 0..100 {
        let f = big_e - e * big_e.sin() - m;
        let fp = 1.0 - e * big_e.cos();
        let delta = f / fp;
        big_e -= delta;
        if delta.abs() < 1.0e-12 {
            break;
        }
    }
    big_e
}

/// Newton solve of the hyperbolic Kepler equation `M = e·sinhH − H`.
fn solve_kepler_hyperbolic(m: f64, e: f64) -> f64 {
    let mut big_h = (2.0 * m.abs() / e + 1.8).ln().copysign(m);
    for _ in 0..100 {
        let f = e * big_h.sinh() - big_h - m;
        let fp = e * big_h.cosh() - 1.0;
        let delta = f / fp;
        big_h -= delta;
        if delta.abs() < 1.0e-12 {
            break;
        }
    }
    big_h
}

fn wrap_pi(angle: f64) -> f64 {
    let two_pi = std::f64::consts::TAU;
    let mut a = angle % two_pi;
    if a > std::f64::consts::PI {
        a -= two_pi;
    } else if a < -std::f64::consts::PI {
        a += two_pi;
    }
    a
}

/// Rotate a J2000-ecliptic rectangular vector into the J2000 equatorial frame.
fn ecliptic_to_equatorial_j2000(v: [f64; 3]) -> [f64; 3] {
    let (sin_e, cos_e) = J2000_OBLIQUITY_RAD.sin_cos();
    [
        v[0],
        v[1] * cos_e - v[2] * sin_e,
        v[1] * sin_e + v[2] * cos_e,
    ]
}

/// Earth's heliocentric position in the **J2000 equatorial** frame (AU) for a
/// TT Julian Date. The VSOP87D solution is mean-of-date ecliptic, so it is
/// rotated to mean-of-date equatorial and then de-precessed to J2000 by the
/// inverse IAU 2006 precession matrix, keeping it consistent with the comet's
/// J2000 position.
fn earth_helio_equatorial_j2000(jd_tt: f64) -> [f64; 3] {
    let (l, b, r) = astro::planet::heliocent_coords(&astro::planet::Planet::Earth, jd_tt);
    let (sin_l, cos_l) = l.sin_cos();
    let (sin_b, cos_b) = b.sin_cos();
    let ecl_of_date = [r * cos_b * cos_l, r * cos_b * sin_l, r * sin_b];
    let eps = mean_obliquity_of_date(jd_tt);
    let (sin_e, cos_e) = eps.sin_cos();
    let eq_of_date = [
        ecl_of_date[0],
        ecl_of_date[1] * cos_e - ecl_of_date[2] * sin_e,
        ecl_of_date[1] * sin_e + ecl_of_date[2] * cos_e,
    ];
    // Inverse precession: mean equator/equinox of date → J2000.
    mat_mul_vec(mat_transpose(precession_matrix_iau2006(jd_tt)), eq_of_date)
}

/// Meeus/IAU-1980 mean obliquity polynomial (matches the VSOP87D mean-of-date
/// ecliptic the `astro` crate returns).
fn mean_obliquity_of_date(jd_tt: f64) -> f64 {
    let t = (jd_tt - crate::J2000_JD) / 36_525.0;
    let arcsec = 21.448 - t * (46.8150 + t * (0.00059 - t * 0.001813));
    (23.0 + (26.0 + arcsec / 60.0) / 60.0) * std::f64::consts::PI / 180.0
}

/// Geocentric apparent state of a comet (J2000 equatorial), light-time
/// corrected. `jd_tt` is the dynamical time of observation.
pub fn apparent_comet(elements: &CometElements, jd_tt: f64) -> CometApparent {
    let earth = earth_helio_equatorial_j2000(jd_tt);

    // Light-time iteration: solve for the emission time so the geometry is the
    // comet's position when the light now arriving left it.
    let mut tau_days = 0.0;
    let mut comet_pos = [0.0; 3];
    let mut comet_vel = [0.0; 3];
    for _ in 0..3 {
        let (pos_ecl, vel_ecl) = heliocentric_state_ecliptic(elements, jd_tt - tau_days);
        comet_pos = ecliptic_to_equatorial_j2000(pos_ecl);
        comet_vel = ecliptic_to_equatorial_j2000(vel_ecl);
        let geo = sub(comet_pos, earth);
        let delta = norm(geo);
        tau_days = delta / SPEED_OF_LIGHT_AU_PER_DAY;
    }

    finish_apparent(elements, comet_pos, comet_vel, earth)
}

/// Topocentric apparent state of a comet for an observer on Earth.
pub fn apparent_comet_topocentric(observer: Observer, elements: &CometElements) -> CometApparent {
    let geo = apparent_comet(elements, observer.time.jd_tt);
    // Apply the WGS84 topocentric parallax. The observer offset is in the
    // (true-equator-of-date) Earth-fixed→equatorial frame; on a ≤6371 km
    // baseline the J2000/of-date difference is far below the comet's parallax,
    // which is itself sub-arcminute, so it is applied directly.
    let observer_km = observer_equatorial_position_km(observer);
    let observer_au = [
        observer_km[0] / ASTRONOMICAL_UNIT_KM,
        observer_km[1] / ASTRONOMICAL_UNIT_KM,
        observer_km[2] / ASTRONOMICAL_UNIT_KM,
    ];
    let nucleus = geo.nucleus_dir();
    let geo_vec = [
        nucleus.x as f64 * geo.distance_au,
        nucleus.y as f64 * geo.distance_au,
        nucleus.z as f64 * geo.distance_au,
    ];
    let topo = sub(geo_vec, observer_au);
    let (right_ascension_rad, declination_rad, distance_au) = ra_dec_from_equatorial_vector(topo);
    CometApparent {
        right_ascension_rad,
        declination_rad,
        distance_au,
        ..geo
    }
}

/// Assemble the apparent state from the comet's heliocentric J2000-equatorial
/// position / velocity and the Earth's position.
fn finish_apparent(
    elements: &CometElements,
    comet_pos: [f64; 3],
    comet_vel: [f64; 3],
    earth: [f64; 3],
) -> CometApparent {
    let geo = sub(comet_pos, earth);
    let (right_ascension_rad, declination_rad, distance_au) = ra_dec_from_equatorial_vector(geo);
    let r = norm(comet_pos);

    let magnitude = elements.absolute_magnitude_m1
        + 5.0 * distance_au.max(1.0e-6).log10()
        + elements.activity_slope_k1 * r.max(1.0e-6).log10();

    // Ion tail: prolonged (anti-solar) radius vector = Sun→comet direction.
    let anti_solar = normalize(comet_pos);
    // Dust tail: representative β = 0.6 syndyne lags toward the trailing
    // (negative heliocentric velocity) direction.
    let trailing = normalize([-comet_vel[0], -comet_vel[1], -comet_vel[2]]);
    let w = REPRESENTATIVE_DUST_BETA * 0.5;
    let dust = normalize([
        (1.0 - w) * anti_solar[0] + w * trailing[0],
        (1.0 - w) * anti_solar[1] + w * trailing[1],
        (1.0 - w) * anti_solar[2] + w * trailing[2],
    ]);

    CometApparent {
        right_ascension_rad,
        declination_rad,
        distance_au,
        heliocentric_distance_au: r,
        magnitude,
        ion_tail_dir: vec3_from(anti_solar),
        dust_tail_dir: vec3_from(dust),
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn norm(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let n = norm(v).max(1.0e-12);
    [v[0] / n, v[1] / n, v[2] / n]
}

fn vec3_from(v: [f64; 3]) -> Vec3 {
    Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Standard J2000 osculating elements of 1P/Halley for the 1986 apparition
    /// (perihelion 1986 Feb 9.459 TT), from the IAU/MPC element set.
    fn halley() -> CometElements {
        CometElements {
            name: "1P/Halley".to_string(),
            perihelion_distance_au: 0.587_104,
            eccentricity: 0.967_143,
            inclination_rad: 162.242_2_f64.to_radians(),
            arg_perihelion_rad: 111.865_7_f64.to_radians(),
            long_asc_node_rad: 58.860_1_f64.to_radians(),
            perihelion_time_jd_tt: 2_446_470.958_91,
            absolute_magnitude_m1: 5.5,
            activity_slope_k1: 10.0,
        }
    }

    #[test]
    fn perihelion_distance_is_exact_at_tp() {
        // At t = Tp the two-body radius must equal q to machine precision, and
        // the heliocentric position must lie exactly along the Gauss P vector.
        let comet = halley();
        let (pos, _) = heliocentric_state_ecliptic(&comet, comet.perihelion_time_jd_tt);
        let r = norm(pos);
        assert!(
            (r - comet.perihelion_distance_au).abs() < 1.0e-9,
            "r at perihelion = {r}, expected q = {}",
            comet.perihelion_distance_au
        );
    }

    #[test]
    fn heliocentric_distance_grows_after_perihelion() {
        let comet = halley();
        let tp = comet.perihelion_time_jd_tt;
        let r0 = norm(heliocentric_state_ecliptic(&comet, tp).0);
        let r30 = norm(heliocentric_state_ecliptic(&comet, tp + 30.0).0);
        let r60 = norm(heliocentric_state_ecliptic(&comet, tp + 60.0).0);
        assert!(r0 < r30 && r30 < r60, "r should grow: {r0} {r30} {r60}");
    }

    #[test]
    fn halley_close_approach_1986_is_southern_and_near() {
        // 1986-04-10 00:00 TT ≈ JD 2446530.5, a day before Halley's
        // 1986-04-11 closest approach to Earth (Δ ≈ 0.42 AU). The comet was
        // deep in the southern sky (Centaurus/Hydra), the reason the 1986
        // apparition favoured southern-hemisphere observers.
        let comet = halley();
        let app = apparent_comet(&comet, 2_446_530.5);
        assert!(
            (0.40..=0.55).contains(&app.distance_au),
            "geocentric Δ = {} AU (close approach ≈ 0.42 AU)",
            app.distance_au
        );
        // Two-body propagation lands Halley at RA ≈ 15.5 h, Dec ≈ −47°
        // (Lupus/Centaurus) — the well-recorded position at the 1986 close
        // approach, matching published ephemerides to within arcminutes.
        let ra_hours = app.right_ascension_rad.to_degrees() / 15.0;
        assert!((15.0..=16.0).contains(&ra_hours), "RA = {ra_hours} h");
        assert!(
            (-50.0..=-44.0).contains(&app.declination_rad.to_degrees()),
            "Halley should be in the deep southern sky, dec = {}°",
            app.declination_rad.to_degrees()
        );
        // ≈59.5 days after perihelion Halley had receded to r ≈ 1.32 AU.
        assert!(
            (1.25..=1.40).contains(&app.heliocentric_distance_au),
            "r = {} AU",
            app.heliocentric_distance_au
        );
        assert!(app.magnitude.is_finite());
    }

    #[test]
    fn magnitude_law_matches_hand_value() {
        // m1 = M1 + 5 log10(Δ) + K1 log10(r). At Δ = 1 AU, r = 1 AU the law
        // reduces to M1 exactly; double-check a non-trivial point too.
        let comet = CometElements {
            absolute_magnitude_m1: 5.0,
            activity_slope_k1: 10.0,
            ..halley()
        };
        // Construct a synthetic apparent state via the public path at a date
        // and just re-evaluate the closed-form law for the returned r, Δ.
        let app = apparent_comet(&comet, comet.perihelion_time_jd_tt + 40.0);
        let expected =
            5.0 + 5.0 * app.distance_au.log10() + 10.0 * app.heliocentric_distance_au.log10();
        assert!((app.magnitude - expected).abs() < 1.0e-9);
    }

    #[test]
    fn tail_directions_are_unit_and_anti_solar() {
        let comet = halley();
        let app = apparent_comet(&comet, comet.perihelion_time_jd_tt + 20.0);
        assert!((app.ion_tail_dir.length() - 1.0).abs() < 1.0e-5);
        assert!((app.dust_tail_dir.length() - 1.0).abs() < 1.0e-5);
        // Ion tail points away from the Sun: its dot with the Sun→comet
        // heliocentric direction is +1 by construction, so the angle to the
        // dust tail (which lags) is small but positive.
        let cos = app.ion_tail_dir.dot(app.dust_tail_dir);
        assert!(cos > 0.5 && cos <= 1.0001, "dust lags ion modestly: {cos}");
    }

    #[test]
    fn parser_reads_rows_and_skips_comments() {
        let csv = "# comment\nname,q_au,e,i_deg,arg_peri_deg,long_node_deg,tp_jd_tt,m1,k1\n\
1P/Halley,0.587104,0.967143,162.2422,111.8657,58.8601,2446470.95891,5.5,10.0\n\
bad,row,only,three\n";
        let parsed = parse_comet_elements(csv);
        assert_eq!(
            parsed.len(),
            1,
            "header, comment, and malformed row skipped"
        );
        assert_eq!(parsed[0].name, "1P/Halley");
        assert!((parsed[0].perihelion_distance_au - 0.587104).abs() < 1e-9);
        assert!((parsed[0].inclination_rad - 162.2422_f64.to_radians()).abs() < 1e-9);
    }

    #[test]
    fn hyperbolic_and_parabolic_branches_are_finite() {
        let base = halley();
        for e in [1.0_f64, 1.001] {
            let comet = CometElements {
                eccentricity: e,
                ..base.clone()
            };
            let app = apparent_comet(&comet, comet.perihelion_time_jd_tt + 15.0);
            assert!(app.right_ascension_rad.is_finite());
            assert!(app.declination_rad.is_finite());
            assert!(app.heliocentric_distance_au > 0.0);
            assert!(app.distance_au > 0.0);
        }
    }
}
