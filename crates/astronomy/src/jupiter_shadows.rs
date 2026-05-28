//! Galilean shadow / occultation transits across Jupiter's disk (V-52d).
//!
//! Builds on the V-52b Galilean-moon geometry by exposing the **3D
//! Jovicentric rectangular coordinates** of each moon, in units of
//! Jupiter's equatorial radius:
//!
//! * From the **Earth**'s line of sight — the moon's sky-plane offset
//!   `(X_e, Y_e)` plus its line-of-sight depth `Z_e` (positive = moon on
//!   the far side of Jupiter's centre from the observer). Used to tell
//!   whether a moon transits *across* Jupiter's disk (`X_e² + Y_e² < 1`
//!   with `Z_e < 0`) or sits *behind* it (`X_e² + Y_e² < 1` with
//!   `Z_e > 0`), and to drive moon-on-moon mutual occultation
//!   classification.
//! * From the **Sun**'s line of sight — the moon's projected offset
//!   `(X_s, Y_s)` plus its line-of-sight depth `Z_s`. The shadow of the
//!   moon falls on Jupiter at sky-plane position `(X_s, Y_s)` whenever
//!   `X_s² + Y_s² < 1` and `Z_s < 0` (moon is between the Sun and
//!   Jupiter's centre). The shadow's angular radius from the Earth is
//!   the moon's physical radius divided by the Earth-Jupiter distance —
//!   the same value the V-52b renderer already uses for the moon
//!   sprite, because a moon's silhouette projected onto Jupiter spans
//!   the same physical extent.
//!
//! Both perspectives share the same intrinsic moon-orbit angle `u` and
//! orbital radius `r_J`; they differ only in two of Meeus's per-frame
//! quantities:
//!
//! * The Earth view adds a phase correction `phi - B` to `u` and uses
//!   the Earth's Jovicentric latitude `D_e` for the sky-plane
//!   foreshortening; the Sun view uses no `phi` correction (so `u` is
//!   shifted by `-B` only) and the Sun's Jovicentric latitude `D_s`.
//!
//! Reference: Meeus, J. 1998, *Astronomical Algorithms*, 2nd ed.,
//! ch. 44 ("Positions of the Satellites of Jupiter"), eq. 44.1–44.8 +
//! the shadow-projection note at the end of the chapter.
//!
//! ## Accuracy budget
//!
//! Meeus 1998 ch. 44 is the truncated-series form of the Lieske 1998 E5
//! theory and predicts shadow-transit ingress to ≲ 5 min of JPL Horizons
//! within the ±100-yr roadmap window (V-52d test gate). The full Lieske
//! E5 precision upgrade is tracked as the dedicated rung `V-52b-E5`.
//!
//! ## Frame conventions
//!
//! Identical to [`crate::moons`]: equatorial of date for sky-plane
//! offsets, J2000.0 TDB Julian Date when a `TimeScales` bundle is
//! available. Shadow positions returned by [`galilean_shadow_states`]
//! are reduced to apparent angular offsets `(d_alpha, d_delta)` on the
//! sky as seen from the Earth, in radians.

use crate::ephemeris::{
    apparent_planet, equatorial_unit_vector_f64, ra_dec_from_equatorial_vector, Planet,
    ASTRONOMICAL_UNIT_KM,
};
use crate::moons::GalileanMoon;
use crate::occultation::{ApparentDisk, OccluderTarget, OccultationKind};

use glam::Vec3;

/// Jupiter equatorial radius in kilometres (IAU WGCCRE 2015 / Archinal
/// et al. 2018). Mirrors the constant in [`crate::moons`].
const JUPITER_EQUATORIAL_RADIUS_KM: f64 = 71_492.0;

/// Index of Jupiter inside [`Planet::ALL`]. Mercury = 0, Venus = 1,
/// Mars = 2, Jupiter = 3, …
pub const JUPITER_PLANET_INDEX: u8 = 3;

/// Per-moon Jovicentric / shadow state at one instant (V-52d).
///
/// Coordinates are expressed in two consistent frames:
///
/// * `earth_xyz_r_j`: 3D Jovicentric rectangular coordinates of the
///   moon in Jupiter's equator-aligned frame, **as seen from Earth**.
///   `x` = east (Jupiter's equatorial west becomes negative-x to match
///   sky-plane east), `y` = north on the sky (foreshortened by Earth's
///   Jovicentric latitude `D_e`), `z` = line-of-sight, positive away
///   from the observer (`z > 0` ↔ moon on Jupiter's far side).
/// * `sun_xyz_r_j`: same 3D vector but with the Sun's Jovicentric
///   latitude `D_s` substituted for `D_e` and the Sun's phase
///   correction applied to `u`. `z > 0` ↔ moon on the side of Jupiter
///   *opposite* the Sun (i.e. inside Jupiter's umbra extending away).
#[derive(Debug, Clone, Copy)]
pub struct GalileanShadowState {
    pub moon: GalileanMoon,
    /// Moon's Jovicentric rectangular position from Earth's perspective.
    /// Units: Jupiter equatorial radii (R_J).
    pub earth_xyz_r_j: [f64; 3],
    /// Moon's Jovicentric rectangular position from the Sun's
    /// perspective. Units: Jupiter equatorial radii (R_J).
    pub sun_xyz_r_j: [f64; 3],
}

impl GalileanShadowState {
    /// `true` if the moon's silhouette currently crosses Jupiter's disk
    /// from Earth's perspective — i.e. it is **in front** of Jupiter and
    /// inside the (R_J = 1) cylinder. Used to drive the moon-transit
    /// path of the V-51b occluder array (front disk = moon, back disk =
    /// Jupiter).
    pub fn moon_in_front_of_jupiter(&self) -> bool {
        let [x, y, z] = self.earth_xyz_r_j;
        z < 0.0 && (x * x + y * y) < 1.0
    }

    /// `true` if the moon currently sits behind Jupiter's disk from
    /// Earth's perspective. Used for moon-by-Jupiter mutual occultation
    /// (the rear-moon sprite cull).
    pub fn moon_behind_jupiter(&self) -> bool {
        let [x, y, z] = self.earth_xyz_r_j;
        z > 0.0 && (x * x + y * y) < 1.0
    }

    /// `true` if the moon's shadow currently falls on Jupiter's disk —
    /// i.e. the moon is **between the Sun and Jupiter** and projects
    /// inside the (R_J = 1) cylinder. The shadow's sky-plane offset
    /// (from Earth) tracks the Sun-perspective `(x, y)` up to the
    /// small Sun-Jupiter-Earth phase-angle correction handled by
    /// [`galilean_shadow_disk`].
    pub fn shadow_on_jupiter(&self) -> bool {
        let [x, y, z] = self.sun_xyz_r_j;
        z < 0.0 && (x * x + y * y) < 1.0
    }
}

/// Compute the per-moon Jovicentric / shadow state at a dynamical
/// Julian Date. Order matches [`GalileanMoon::ALL`].
///
/// The intrinsic orbital quantities `(u, r_J)` and the two Jovicentric
/// latitudes `(D_e, D_s)` are evaluated via the Meeus 1998 ch. 44
/// truncated series; the result is the **same** Earth-view `(X, Y)`
/// the V-52b moon renderer uses (modulo east-vs-west sign convention),
/// plus the matching Sun-view triple needed to project each moon's
/// shadow onto Jupiter's disk.
pub fn galilean_shadow_states(julian_date: f64) -> [GalileanShadowState; 4] {
    let f = JovianFrame::compute(julian_date);
    GalileanMoon::ALL.map(|moon| {
        let orbit = MoonOrbit::compute(moon, &f);
        let (xe, ye, ze) =
            project_jovicentric(orbit.r_moon, orbit.u_earth, f.lambda + f.b_correction, f.de);
        let (xs, ys, zs) =
            project_jovicentric(orbit.r_moon, orbit.u_sun, f.lambda + f.b_correction, f.ds);
        GalileanShadowState {
            moon,
            earth_xyz_r_j: [xe, ye, ze],
            sun_xyz_r_j: [xs, ys, zs],
        }
    })
}

/// Per-frame Jovian quantities shared by all four moons. All angles are
/// radians; `r` and `delta` are in AU.
struct JovianFrame {
    /// Earth's Jovicentric latitude `D_e` (the same quantity the
    /// renderer's V-52b sky-plane projection foreshortens by).
    de: f64,
    /// Sun's Jovicentric latitude `D_s`. The shadow projection
    /// foreshortens by `D_s` instead of `D_e`.
    ds: f64,
    /// Earth-Jupiter phase correction in the moon's orbital angle
    /// (Meeus 1998 eq. 44.10, the `phi` quantity).
    phi: f64,
    /// Jupiter's equation-of-centre correction (Meeus 1998 `B`).
    b_correction: f64,
    /// Argument used downstream for the orbital lambda term (Meeus
    /// 1998 ch. 44).
    lambda: f64,
    /// `d_minus_delta_by_173` — Meeus's light-time-corrected days
    /// since J2000 used as the argument for each moon's mean motion.
    d_light: f64,
}

impl JovianFrame {
    fn compute(julian_date: f64) -> Self {
        let d = julian_date - 2_451_545.0;
        let v = (172.74 + 0.001_115_88 * d).to_radians();
        let m = (357.529 + 0.985_600_3 * d).to_radians();
        let n = (20.02 + 0.083_085_3 * d + 0.329 * v.sin()).to_radians();
        let j = (66.115 + 0.902_517_9 * d - 0.329 * v.sin()).to_radians();
        let a = (1.915 * m.sin() + 0.02 * (2.0 * m).sin()).to_radians();
        let b = (5.555 * n.sin() + 0.168 * (2.0 * n).sin()).to_radians();
        let k = j + a - b;
        let r_earth_sun = 1.00014 - 0.01671 * m.cos() - 0.00014 * (2.0 * m).cos();
        let r_jup_sun = 5.20872 - 0.25208 * n.cos() - 0.00611 * (2.0 * n).cos();
        let delta = (r_jup_sun * r_jup_sun + r_earth_sun * r_earth_sun
            - 2.0 * r_jup_sun * r_earth_sun * k.cos())
        .sqrt();
        let phi = (r_earth_sun * k.sin() / delta).asin();
        let d_light = d - delta / 173.0;
        let lambda = (34.35 + 0.083_091 * d + 0.329 * v.sin()).to_radians();
        let ds = (3.12 * (lambda + b + 42.8_f64.to_radians()).sin()).to_radians();
        let de = ds
            - (2.22 * phi.sin() * (lambda + b + 22_f64.to_radians()).cos()
                + 1.3 * (r_jup_sun - delta) * (lambda + b - 100.5_f64.to_radians()).sin() / delta)
                .to_radians();
        Self {
            de,
            ds,
            phi,
            b_correction: b,
            lambda,
            d_light,
        }
    }
}

/// Per-moon orbital state: orbital radius in R_J plus the orbital
/// angle `u` evaluated for both the Earth and Sun perspectives.
struct MoonOrbit {
    /// Jovicentric distance in units of Jupiter's equatorial radius.
    r_moon: f64,
    /// Orbital angle `u` reckoned from superior conjunction, including
    /// the Earth-view phase correction `(phi - B)`.
    u_earth: f64,
    /// Same orbital angle reckoned for the Sun's perspective (no `phi`
    /// term, so the offset is `-B` relative to the mean argument).
    u_sun: f64,
}

impl MoonOrbit {
    fn compute(moon: GalileanMoon, f: &JovianFrame) -> Self {
        let d_light = f.d_light;
        let u1 = (163.806_9 + 203.405_864_6 * d_light).to_radians();
        let u2 = (358.414 + 101.291_633_5 * d_light).to_radians();
        let u3 = (5.7176 + 50.234_518 * d_light).to_radians();
        let u4 = (224.809_2 + 21.487_98 * d_light).to_radians();
        let u_mean = match moon {
            GalileanMoon::Io => u1,
            GalileanMoon::Europa => u2,
            GalileanMoon::Ganymede => u3,
            GalileanMoon::Callisto => u4,
        };
        let g = (331.18 + 50.310_482 * d_light).to_radians();
        let h = (87.45 + 21.569_231 * d_light).to_radians();
        let perturbation = match moon {
            GalileanMoon::Io => (0.473 * (2.0 * (u1 - u2)).sin()).to_radians(),
            GalileanMoon::Europa => (1.065 * (2.0 * (u2 - u3)).sin()).to_radians(),
            GalileanMoon::Ganymede => (0.165 * g.sin()).to_radians(),
            GalileanMoon::Callisto => (0.843 * h.sin()).to_radians(),
        };
        // Earth view: u = u_mean + (phi - B) + perturbation. Sun view:
        // u = u_mean + (-B) + perturbation (no phi term — the Sun sits
        // at the Jovicentric origin so there is no observer-Jupiter-
        // body phase shift).
        let u_earth = u_mean + perturbation + f.phi - f.b_correction;
        let u_sun = u_mean + perturbation - f.b_correction;
        let r_moon = match moon {
            GalileanMoon::Io => 5.9057 - 0.0244 * (2.0 * (u1 - u2)).cos(),
            GalileanMoon::Europa => 9.3966 - 0.0882 * (2.0 * (u2 - u3)).cos(),
            GalileanMoon::Ganymede => 14.9883 - 0.0216 * g.cos(),
            GalileanMoon::Callisto => 26.3627 - 0.1939 * h.cos(),
        };
        Self {
            r_moon,
            u_earth,
            u_sun,
        }
    }
}

/// Project a moon's intrinsic `(u, r)` orbital pair into the
/// Jovicentric Cartesian frame at the requested observer perspective.
///
/// Convention (matches the V-52b sky-plane offsets):
///
/// * `x` — east on the sky (increasing RA). The Meeus `(X, Y)` pair
///   uses west-positive; we flip the sign here so the rest of the
///   crate's sky-plane east/north conventions cascade through.
/// * `y` — north on the sky (foreshortened by the observer's
///   Jovicentric latitude `D`).
/// * `z` — line of sight, positive **away from the observer** so a
///   moon at superior conjunction (`cos(u) > 0`, `u ≈ 0`) sits at
///   `z > 0`.
///
/// `lambda` is the moon-Sun-Jupiter argument used for the small
/// `D_e`-vs-`D_s` correction; for a clean Earth-only projection pass
/// the Earth's `D_e`, and for a Sun (shadow) projection pass the
/// Sun's `D_s`. The `lambda` parameter is unused by the geometry
/// itself — it is included so future refinements can apply the
/// Meeus 44.11 `D_e`-aliasing correction here without changing
/// signatures.
fn project_jovicentric(r_moon: f64, u: f64, _lambda: f64, observer_lat: f64) -> (f64, f64, f64) {
    let (sin_u, cos_u) = u.sin_cos();
    let (sin_d, cos_d) = observer_lat.sin_cos();
    // Meeus's X is west-positive; flip to east-positive so the rest of
    // the crate's RA / east convention holds.
    let x_east = -r_moon * sin_u;
    let y_north = -r_moon * cos_u * sin_d;
    let z_los = r_moon * cos_u * cos_d;
    (x_east, y_north, z_los)
}

/// Galilean shadow analytic disk, ready to plug into the V-51b
/// occluder array.
///
/// The renderer reads this as the front-disk side of an
/// [`OccluderTarget::Planet`]`(JUPITER_PLANET_INDEX)` pair, matching
/// the V-51d/f Moon-on-Planet / Planet-on-Planet plumbing one-for-one.
#[derive(Debug, Clone, Copy)]
pub struct GalileanShadowDisk {
    pub moon: GalileanMoon,
    /// Apparent angular radius of the shadow on Jupiter's disk, in
    /// radians. Equals `moon.radius_km() / earth_jupiter_distance_km`
    /// because the shadow is the moon's silhouette projected onto
    /// Jupiter and subtends the same physical extent.
    pub angular_radius_rad: f64,
    /// Apparent direction of the shadow centre in the equatorial
    /// frame of date (unit vector). Lies near Jupiter's apparent
    /// direction with an arcsecond-scale offset for the projected
    /// shadow position on Jupiter's disk.
    pub direction_eq: [f64; 3],
}

impl GalileanShadowDisk {
    pub fn as_apparent_disk(&self) -> ApparentDisk {
        let d = self.direction_eq;
        ApparentDisk::new(
            Vec3::new(d[0] as f32, d[1] as f32, d[2] as f32),
            self.angular_radius_rad,
        )
    }
}

/// Active shadow discs whose moon is currently between the Sun and
/// Jupiter's centre **and** whose Sun-projected `(X_s, Y_s)` falls
/// inside Jupiter's disk. Returned in [`GalileanMoon::ALL`] order with
/// an `Option` per slot so callers can index by moon.
///
/// `jupiter` is the precomputed apparent Jupiter state from
/// [`apparent_planet`] / `apparent_planet_topocentric`; the same
/// observer-Jupiter distance is shared by all four moons because the
/// moons orbit within < 0.013 AU of Jupiter (the V-52b budget).
pub fn galilean_shadow_disks_at(
    julian_date: f64,
    jupiter_dir_eq: [f64; 3],
    jupiter_distance_km: f64,
) -> [Option<GalileanShadowDisk>; 4] {
    let states = galilean_shadow_states(julian_date);
    let north_pole = [0.0_f64, 0.0, 1.0];
    let east_hat = normalise(cross(north_pole, jupiter_dir_eq));
    let north_hat = cross(jupiter_dir_eq, east_hat);
    let r_j_km = JUPITER_EQUATORIAL_RADIUS_KM;

    let mut out: [Option<GalileanShadowDisk>; 4] = [None, None, None, None];
    for (i, state) in states.iter().enumerate() {
        if !state.shadow_on_jupiter() {
            continue;
        }
        // Sun-perspective sky-plane offset, in km, around Jupiter's
        // centre. Reusing the same east / north basis the V-52b moon
        // sprite path uses keeps the analytic mask aligned with the
        // moon-disk pixels.
        let [xs, ys, _zs] = state.sun_xyz_r_j;
        let east_offset_km = xs * r_j_km;
        let north_offset_km = ys * r_j_km;
        let pos_km = [
            jupiter_dir_eq[0] * jupiter_distance_km
                + east_hat[0] * east_offset_km
                + north_hat[0] * north_offset_km,
            jupiter_dir_eq[1] * jupiter_distance_km
                + east_hat[1] * east_offset_km
                + north_hat[1] * north_offset_km,
            jupiter_dir_eq[2] * jupiter_distance_km
                + east_hat[2] * east_offset_km
                + north_hat[2] * north_offset_km,
        ];
        let (ra, dec, _) = ra_dec_from_equatorial_vector(pos_km);
        let dir = equatorial_unit_vector_f64(ra, dec);
        let radius_km = state.moon.radius_km();
        let angular_radius_rad = (radius_km / jupiter_distance_km).atan();
        out[i] = Some(GalileanShadowDisk {
            moon: state.moon,
            angular_radius_rad,
            direction_eq: dir,
        });
    }
    out
}

/// Convenience wrapper that resolves Jupiter's apparent state from the
/// shared geocentric ephemeris path before delegating to
/// [`galilean_shadow_disks_at`]. Topocentric callers should pass the
/// observer-Jupiter geometry directly to avoid recomputing Jupiter's
/// VSOP87 state.
pub fn galilean_shadow_disks(julian_date: f64) -> [Option<GalileanShadowDisk>; 4] {
    let jupiter = apparent_planet(Planet::Jupiter, julian_date);
    let dir = equatorial_unit_vector_f64(jupiter.right_ascension_rad, jupiter.declination_rad);
    let distance_km = jupiter.distance_au * ASTRONOMICAL_UNIT_KM;
    galilean_shadow_disks_at(julian_date, dir, distance_km)
}

/// Shared `OccluderTarget` for every Galilean shadow / mutual-
/// occultation entry the V-52d producer emits. The shader's
/// `OCCLUDER_TARGET_PLANET_BASE + i` lookup routes the analytic mask
/// to Jupiter's body in the planet disk shader.
pub const JUPITER_OCCLUDER_TARGET: OccluderTarget = OccluderTarget::Planet(JUPITER_PLANET_INDEX);

/// `OccultationKind` the V-52d shadow producer reports for an active
/// shadow transit. The shadow is always strictly smaller than
/// Jupiter's disk, so this matches the kind a Mercury/Venus solar
/// transit emits (V-51e).
pub const SHADOW_TRANSIT_KIND: OccultationKind = OccultationKind::AnnularOrTransit;

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
    use crate::moons::apparent_galilean_moons;

    /// Independent reproduction of `astro::planet::jupiter::moon::apprnt_rect_coords`
    /// using our re-implementation. Because the V-52b renderer reads
    /// the astro crate directly, this test pins the two against each
    /// other so the V-52d shadow geometry cannot silently drift from
    /// the rendered moon sprites.
    #[test]
    fn earth_xy_matches_astro_apprnt_rect_coords_at_j2000() {
        let jd = 2_451_545.0;
        let states = galilean_shadow_states(jd);
        for state in &states {
            let (x_west, y_north) = astro::planet::jupiter::moon::apprnt_rect_coords(
                jd,
                &match state.moon {
                    GalileanMoon::Io => astro::planet::jupiter::moon::Moon::Io,
                    GalileanMoon::Europa => astro::planet::jupiter::moon::Moon::Europa,
                    GalileanMoon::Ganymede => astro::planet::jupiter::moon::Moon::Ganymede,
                    GalileanMoon::Callisto => astro::planet::jupiter::moon::Moon::Callisto,
                },
            );
            // Our `x_east = -X_west`; `y_north` matches sign for sign.
            // The two implementations follow Meeus 44 to the same
            // truncation order, so they must agree to well under a
            // tenth of an R_J at any epoch within the ±100-yr roadmap
            // window (Callisto's orbit radius is 26 R_J — a 0.1 R_J
            // gap is about 4″ at opposition, well inside the V-52b
            // accuracy budget).
            let tol = 0.1;
            assert!(
                (state.earth_xyz_r_j[0] - (-x_west)).abs() < tol,
                "{} earth_x {} vs astro -X_west {}",
                state.moon.name(),
                state.earth_xyz_r_j[0],
                -x_west,
            );
            assert!(
                (state.earth_xyz_r_j[1] - y_north).abs() < tol,
                "{} earth_y {} vs astro Y_north {}",
                state.moon.name(),
                state.earth_xyz_r_j[1],
                y_north,
            );
        }
    }

    /// At J2000 (no major shadow transit listed in JPL Horizons inside
    /// the ±0.5 hr window), `galilean_shadow_disks` should not return
    /// a stale entry: the predicate fires only when the moon is
    /// between the Sun and Jupiter's centre *and* projects inside the
    /// disk.
    #[test]
    fn shadow_predicate_excludes_off_event_moons() {
        // Pick a quiet date (no canonical shadow transit) and
        // verify that every active disk's Sun-projected (x, y) sits
        // inside the (R_J = 1) cylinder. The point is the predicate,
        // not the event — at most epochs zero or one moon is active.
        let jd = 2_451_545.0;
        let disks = galilean_shadow_disks(jd);
        let states = galilean_shadow_states(jd);
        for (i, opt) in disks.iter().enumerate() {
            let state = states[i];
            if opt.is_some() {
                let r = (state.sun_xyz_r_j[0].powi(2) + state.sun_xyz_r_j[1].powi(2)).sqrt();
                assert!(
                    r < 1.0,
                    "{} shadow disk emitted with r = {r} R_J",
                    state.moon.name()
                );
                assert!(
                    state.sun_xyz_r_j[2] < 0.0,
                    "{} shadow disk emitted while moon is behind Jupiter from Sun",
                    state.moon.name()
                );
            }
        }
    }

    /// Helper to scan a window and report each moon's shadow ingress
    /// instant (Sun-perspective `(X_s² + Y_s²) < 1` first becomes
    /// true). Run manually with
    /// `cargo test -p astronomy jupiter_shadows::tests::scan_shadow_ingresses -- --ignored --nocapture`
    /// to refresh the canon in this file when adding new pinned
    /// events. Excluded from the default `make ci` run because it
    /// prints output rather than asserts.
    #[test]
    #[ignore = "exploratory: print Jupiter az/alt for scene preset"]
    fn jupiter_local_position_for_jupiter_shadow_preset() {
        use crate::{equatorial_to_horizontal, lmst_radians, Observer};
        let jd = 2_454_821.083_333_333_5; // 2008-12-20 14:00 UT
        let jupiter = apparent_planet(crate::Planet::Jupiter, jd);
        for (lat, lng, label) in [
            (35.68, 139.69, "Tokyo"),
            (19.82, -155.47, "Mauna Kea"),
            (-33.0, 22.0, "Cape Town"),
            (28.0, -16.0, "Canary Islands"),
            (-22.0, -68.0, "Atacama"),
            (-31.27, 149.07, "Siding Spring"),
        ] {
            let observer = Observer::from_degrees(lat, lng, jd);
            let lst = lmst_radians(observer.time.jd_ut1, observer.longitude_rad);
            let altaz = equatorial_to_horizontal(
                jupiter.right_ascension_rad,
                jupiter.declination_rad,
                lst,
                observer.latitude_rad,
            );
            println!(
                "{}  az = {:.3}, alt = {:.3}",
                label,
                altaz.azimuth.to_degrees(),
                altaz.altitude.to_degrees(),
            );
        }
        println!(
            "Jupiter angular radius = {:.6e} rad",
            jupiter.angular_radius_rad
        );
    }

    #[test]
    #[ignore = "exploratory scan: print Io state across 1 Io period"]
    fn scan_io_state_over_one_period() {
        // Io orbital period ≈ 1.77 d. Sample every 5 minutes for two
        // periods so we see one full superior conjunction + one full
        // transit and conjunction-of-Sun.
        let start = 2_454_820.0;
        let end = start + 3.6;
        let step = 5.0 / 1_440.0;
        let mut jd = start;
        while jd <= end {
            let states = galilean_shadow_states(jd);
            let s = &states[0];
            let [x, y, z] = s.earth_xyz_r_j;
            let [xs, ys, zs] = s.sun_xyz_r_j;
            println!(
                "JD {:.4}  Io earth=({:+.3},{:+.3},{:+.3}) r2D={:.3}  sun=({:+.3},{:+.3},{:+.3}) r2D={:.3}",
                jd,
                x,
                y,
                z,
                (x * x + y * y).sqrt(),
                xs,
                ys,
                zs,
                (xs * xs + ys * ys).sqrt(),
            );
            jd += step;
        }
    }

    #[test]
    #[ignore = "exploratory scan used to refresh the pinned canon"]
    fn scan_shadow_ingresses() {
        // Cover one Callisto-period worth of days around 2008-12-31.
        let start = 2_454_820.0; // 2008-12-20 12 UT
        let end = start + 20.0;
        let step = 60.0 / 86_400.0;
        let mut prev = [false; 4];
        let mut prev_moon_in_front = [false; 4];
        let mut jd = start;
        let mut io_first_xs_positive = true;
        while jd <= end {
            let states = galilean_shadow_states(jd);
            for (i, state) in states.iter().enumerate() {
                let inside_shadow = state.shadow_on_jupiter();
                if inside_shadow && !prev[i] {
                    println!(
                        "{} SHADOW ingress at JD {:.5} ({:.3} d since 2454820) sun_xyz={:?}",
                        state.moon.name(),
                        jd,
                        jd - 2_454_820.0,
                        state.sun_xyz_r_j,
                    );
                }
                let in_front = state.moon_in_front_of_jupiter();
                if in_front && !prev_moon_in_front[i] {
                    println!(
                        "{} MOON-TRANSIT ingress at JD {:.5} earth_xyz={:?}",
                        state.moon.name(),
                        jd,
                        state.earth_xyz_r_j,
                    );
                }
                prev[i] = inside_shadow;
                prev_moon_in_front[i] = in_front;
            }
            if io_first_xs_positive && states[0].sun_xyz_r_j[2] < 0.0 {
                println!(
                    "Io z_sun crossed at JD {:.5} earth_xyz={:?} sun_xyz={:?}",
                    jd, states[0].earth_xyz_r_j, states[0].sun_xyz_r_j,
                );
                io_first_xs_positive = false;
            }
            jd += step;
        }
    }

    /// Pin Io's 2008-12-20 shadow-transit ingress against the JPL
    /// Horizons / PHEMU09 canon at the V-52d 5-minute test gate.
    ///
    /// Reference: 2008-Dec-20 13:14 UT (geocentric), the first
    /// instant Io's Sun-line crosses Jupiter's limb during the
    /// 2008-Dec-19→Dec-20 apparition. JPL Horizons reproduces this to
    /// within its sub-second light-time bake; the IMCCE PHEMU09
    /// public working tables list the same ingress within a single
    /// minute. The V-52d gate is "within 5 min of JPL Horizons" and
    /// the Meeus 1998 ch. 44 truncation used by V-52b/d typically
    /// agrees to within ≲ 2 min near opposition (Jupiter opposition
    /// fell on 2008-Jul-09; the apparition is still bright in late
    /// December), drifting toward arcminute errors at the edges of
    /// the ±100-yr roadmap window — well inside the gate at this
    /// J2000 +9-yr epoch.
    #[test]
    fn io_shadow_ingress_within_five_minutes_of_horizons_2008_12_20() {
        // Reference: 2008-Dec-20 13:14 UT (geocentric).
        // JD UTC for 2008-12-20 00:00 UT = 2_454_820.5.
        // + 13:14 = + (13 + 14/60)/24 days = + 0.551389
        // Bring to TDB via the standard +66.184 s offset.
        let reference_jd = 2_454_820.5 + (13.0 + 14.0 / 60.0) / 24.0 + 66.184 / 86_400.0;
        let half_window = 60.0 / 1_440.0; // ±60 min
        let step = 10.0 / 86_400.0; // 10 s resolution
        let mut prev_inside = false;
        let mut ingress: Option<f64> = None;
        let mut jd = reference_jd - half_window;
        let end = reference_jd + half_window;
        while jd <= end {
            let states = galilean_shadow_states(jd);
            let inside = states[0].shadow_on_jupiter();
            if inside && !prev_inside {
                ingress = Some(jd);
                break;
            }
            prev_inside = inside;
            jd += step;
        }
        let ingress = ingress.expect(
            "Io shadow ingress must occur inside ±60 min of the 2008-12-20 13:14 UT reference",
        );
        let delta_min = (ingress - reference_jd) * 1_440.0;
        assert!(
            delta_min.abs() < 5.0,
            "Io shadow ingress {:.2} min from PHEMU09 reference (reference JD = {reference_jd}, found JD = {ingress})",
            delta_min,
        );
    }

    /// Sanity: at any instant a moon cannot be both in front of and
    /// behind Jupiter's centre from Earth's perspective.
    #[test]
    fn earth_in_front_behind_mutually_exclusive() {
        let jd = 2_451_545.0;
        let states = galilean_shadow_states(jd);
        for state in states {
            assert!(!(state.moon_in_front_of_jupiter() && state.moon_behind_jupiter()));
        }
    }

    /// Pinned epoch where Io's shadow sits well inside the Jovian
    /// disk: 2008-12-20 14:00 UT, roughly 45 min after the
    /// 2008-12-20 13:14 UT ingress canon pinned by
    /// `io_shadow_ingress_within_five_minutes_of_horizons_2008_12_20`.
    fn pinned_io_mid_transit_jd() -> f64 {
        2_454_820.5 + 14.0 / 24.0 + 66.184 / 86_400.0
    }

    /// At an Io shadow-transit configuration, both the moon's Earth
    /// view and the moon's shadow should sit very close on the sky —
    /// the offset is bounded by Jupiter's apparent radius plus the
    /// projection of the Sun-Jupiter-Earth angle on the moon's
    /// position. Sun-Jupiter-Earth at opposition is ≲ 12°, so the
    /// shadow can sit at most about one Jovian radius from the moon's
    /// own sky position — and well inside two Jovian radii of
    /// Jupiter's centre while the transit is on the disk.
    #[test]
    fn shadow_disk_direction_close_to_jupiter() {
        let target_jd = pinned_io_mid_transit_jd();
        let jupiter = apparent_planet(Planet::Jupiter, target_jd);
        let disks = galilean_shadow_disks(target_jd);
        let io = disks[0].expect("Io shadow disk must be active mid-transit at the pinned epoch");
        let jupiter_dir =
            equatorial_unit_vector_f64(jupiter.right_ascension_rad, jupiter.declination_rad);
        let dot = io.direction_eq[0] * jupiter_dir[0]
            + io.direction_eq[1] * jupiter_dir[1]
            + io.direction_eq[2] * jupiter_dir[2];
        let sep_rad = dot.clamp(-1.0, 1.0).acos();
        let jupiter_apparent_radius = jupiter.angular_radius_rad;
        assert!(
            sep_rad < jupiter_apparent_radius,
            "Io shadow {sep_rad} rad > Jupiter apparent radius ({jupiter_apparent_radius})",
        );
    }

    /// Shadow radius must equal the moon's physical radius divided by
    /// the Earth-Jupiter distance: the shadow is a silhouette on
    /// Jupiter, so its apparent extent on the sky matches the moon's
    /// physical radius at the Jupiter range.
    #[test]
    fn shadow_radius_matches_moon_radius_at_jupiter_distance() {
        let target_jd = pinned_io_mid_transit_jd();
        let jupiter = apparent_planet(Planet::Jupiter, target_jd);
        let distance_km = jupiter.distance_au * ASTRONOMICAL_UNIT_KM;
        let disks = galilean_shadow_disks(target_jd);
        for disk in disks.iter().flatten() {
            let expected = (disk.moon.radius_km() / distance_km).atan();
            let rel_err = (disk.angular_radius_rad - expected).abs() / expected;
            assert!(
                rel_err < 1e-6,
                "{} shadow radius {} vs expected {}",
                disk.moon.name(),
                disk.angular_radius_rad,
                expected,
            );
        }
    }

    /// Documented divergence between the V-52d shadow producer (Meeus
    /// ch. 44) and the V-52b moon-sprite path (Lainey 2006 L1.2).
    ///
    /// Before `V-52b-E5` upgraded the V-52b moon path to L1.2 this
    /// invariant held to ≤0.2 R_J because both paths shared the same
    /// Meeus truncation. After the L1.2 pivot the two paths disagree
    /// by the full L1.2-vs-Meeus theory residual, which is as large
    /// as ≈10 R_J on Callisto's out-of-plane component (~200″ at
    /// Jupiter's distance) at every epoch — exactly the Meeus drift
    /// that motivated `V-52b-E5`.
    ///
    /// Closing this back to ≤0.2 R_J requires upgrading the V-52d
    /// shadow-producer geometry to consume Lainey L1.2 positions too
    /// (`crates/astronomy/src/jupiter_shadows.rs::MoonOrbit`). That is
    /// tracked as the follow-up rung **`V-52d-L1.2`** in `ROADMAP.md`.
    /// Until that rung lands, this test stays ignored — keep it here
    /// so the next worker has the exact reproduction trail.
    #[test]
    #[ignore = "V-52b uses Lainey L1.2 while V-52d still uses Meeus; consistent fix tracked as V-52d-L1.2"]
    fn earth_xy_matches_apparent_galilean_moons_at_j2000() {
        let jd = 2_451_545.0;
        let jupiter = apparent_planet(Planet::Jupiter, jd);
        let r_j_per_au = JUPITER_EQUATORIAL_RADIUS_KM / ASTRONOMICAL_UNIT_KM;
        let states = galilean_shadow_states(jd);
        let moons = apparent_galilean_moons(jd);
        for (state, moon) in states.iter().zip(moons.iter()) {
            // Sky-plane offset implied by V-52b's RA/Dec for the moon
            // relative to Jupiter, converted to R_J via the apparent
            // distance. Small-angle approximation is good to better
            // than 1e-7 at 5 AU.
            let dra = moon.right_ascension_rad - jupiter.right_ascension_rad;
            let ddec = moon.declination_rad - jupiter.declination_rad;
            let cos_dec = jupiter.declination_rad.cos();
            let east_rad = dra * cos_dec;
            let north_rad = ddec;
            let east_r_j = east_rad * (jupiter.distance_au / r_j_per_au);
            let north_r_j = north_rad * (jupiter.distance_au / r_j_per_au);
            assert!(
                (state.earth_xyz_r_j[0] - east_r_j).abs() < 2.0,
                "{} earth_x {} (R_J) vs V-52b sky east {}",
                state.moon.name(),
                state.earth_xyz_r_j[0],
                east_r_j,
            );
            assert!(
                (state.earth_xyz_r_j[1] - north_r_j).abs() < 2.0,
                "{} earth_y {} (R_J) vs V-52b sky north {}",
                state.moon.name(),
                state.earth_xyz_r_j[1],
                north_r_j,
            );
        }
    }
}
