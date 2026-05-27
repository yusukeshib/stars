//! Pair-wise occultation / eclipse geometry (V-51a).
//!
//! One observer-centric apparent disk hides another when their angular
//! separation falls below the sum of their angular radii. This module
//! exposes that geometry as a pure-function pair-wise classifier together
//! with the obscuration-fraction helper used both by the renderer
//! (analytic-mask subtract path) and by the planning side (eclipse window
//! search + P1..P4 contact-time refinement).
//!
//! The same shape applies to:
//!
//! * Solar eclipses (`front = Moon`, `back = Sun`),
//! * Mercury / Venus transits across the Sun (`front = planet`, `back = Sun`),
//! * Lunar occultations of stars and planets (`front = Moon`,
//!   `back = star or planet`),
//! * Mutual planetary occultations (`front`/`back` = planet pair),
//! * Lunar eclipses, by treating Earth's umbra at the lunar distance as the
//!   apparent disk in front of the Moon (this is the geometry the V-36
//!   visual aid in [`crate::apparent_moon`] already uses; the helper here is
//!   the canonical form).
//!
//! Distinct from the catalog of named bodies, [`ApparentDisk`] is the
//! contract the renderer and the planning helpers agree on: a unit
//! direction in any consistent frame, paired with an apparent angular
//! radius. Callers compute apparent disks from
//! [`crate::apparent_sun_topocentric`] / [`crate::apparent_moon_topocentric`]
//! / [`crate::apparent_planet_topocentric`] for the physical bodies, or
//! synthesise them (Earth umbra at the lunar distance).
//!
//! References:
//!
//! * Meeus, J. 1998, *Astronomical Algorithms*, 2nd ed., ch. 54 ("Solar
//!   Eclipses") — pair-wise apparent-disk geometry and circumstance
//!   conventions.
//! * Espenak, F. & Meeus, J. 2006, NASA TP-2006-214141, *Five Millennium
//!   Canon of Solar Eclipses* — validation circumstances pinned in
//!   `VALIDATION.md`.

use glam::Vec3;

/// Apparent disk of a body as seen by the observer.
///
/// `direction` is a unit vector pointing from the observer toward the
/// body's apparent centre, in *any* frame chosen by the caller (equatorial,
/// ecliptic, ENU, …) as long as the same frame is used for both members of
/// a pair. `angular_radius_rad` is the apparent semidiameter in radians;
/// pass `0` for a point source such as a catalogue star.
#[derive(Debug, Clone, Copy)]
pub struct ApparentDisk {
    pub direction: Vec3,
    pub angular_radius_rad: f64,
}

impl ApparentDisk {
    pub const fn new(direction: Vec3, angular_radius_rad: f64) -> Self {
        Self {
            direction,
            angular_radius_rad,
        }
    }

    /// Angular separation (radians) between two disk centres. Uses an
    /// `f64`-accumulated dot product so partial / annular contact tests are
    /// numerically stable at the 1″ scale even from `f32` directions.
    pub fn separation_rad(self, other: Self) -> f64 {
        separation_rad(self.direction, other.direction)
    }
}

/// Result of classifying one apparent-disk pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OccultationKind {
    /// `Δ ≥ r_front + r_back` — disks do not touch.
    None,
    /// `|r_front − r_back| < Δ < r_front + r_back` — partial occultation
    /// or partial solar eclipse / planetary transit ingress / egress.
    Partial,
    /// `Δ ≤ r_back − r_front` with `r_front < r_back` — the front disk
    /// is fully inside the back disk. Annular solar eclipse,
    /// Mercury / Venus transit interior phase, lunar occultation of a
    /// star (treated as a point source).
    AnnularOrTransit,
    /// `Δ ≤ r_front − r_back` with `r_front ≥ r_back` — the back disk
    /// is fully behind the front disk. Total solar eclipse, total lunar
    /// occultation of a planet.
    Total,
}

impl OccultationKind {
    /// `true` for any contact stronger than [`OccultationKind::None`].
    pub const fn is_occulting(self) -> bool {
        !matches!(self, Self::None)
    }

    /// `true` only at the deepest geometry (annular, transit, or total).
    pub const fn is_central(self) -> bool {
        matches!(self, Self::AnnularOrTransit | Self::Total)
    }

    /// Stable kebab-case label suitable for serialization, CLI output,
    /// HUD strings, and validation diffs.
    pub const fn as_kebab_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Partial => "partial",
            Self::AnnularOrTransit => "annular-or-transit",
            Self::Total => "total",
        }
    }
}

/// Classify the apparent-disk geometry of one observer-centric pair.
///
/// `front` is the body closer to the observer (Moon, transiting planet,
/// Earth umbra); `back` is the body being hidden (Sun, occulted star,
/// occulted planet, eclipsed Moon). The caller is responsible for picking
/// the correct foreground/background ordering: this helper is pure
/// geometry and does not consult body distances.
pub fn classify_disks(front: ApparentDisk, back: ApparentDisk) -> OccultationKind {
    let r_front = front.angular_radius_rad.max(0.0);
    let r_back = back.angular_radius_rad.max(0.0);
    let sep = front.separation_rad(back);
    if !sep.is_finite() {
        return OccultationKind::None;
    }
    if sep >= r_front + r_back {
        return OccultationKind::None;
    }
    let diff = (r_front - r_back).abs();
    if sep > diff {
        return OccultationKind::Partial;
    }
    if r_front >= r_back {
        OccultationKind::Total
    } else {
        OccultationKind::AnnularOrTransit
    }
}

/// Fraction of the *back* disk hidden by the *front* disk, in `[0, 1]`.
///
/// During an annular event the back disk is larger than the front disk and
/// the maximum obscuration is `r_front² / r_back² < 1`. During a total
/// occultation the back disk is fully hidden, so this saturates at `1`.
///
/// Uses the lens-area formula for two intersecting circles — same closed
/// form used by Meeus AA §54 for the magnitude of a partial solar
/// eclipse. The expression below is the obscuration ratio (area
/// fraction), not the limb-to-limb magnitude (length fraction).
pub fn obscuration_fraction(front: ApparentDisk, back: ApparentDisk) -> f32 {
    let r_front = front.angular_radius_rad.max(0.0);
    let r_back = back.angular_radius_rad.max(0.0);
    if r_back <= 0.0 {
        // Point source: it is either occulted or not. Classify says so.
        return match classify_disks(front, back) {
            OccultationKind::None => 0.0,
            _ => 1.0,
        };
    }
    let sep = front.separation_rad(back);
    if !sep.is_finite() || sep >= r_front + r_back {
        return 0.0;
    }
    if sep <= (r_back - r_front).abs() {
        // Back fully inside front (total) → 1; front fully inside back
        // (annular) → r_front² / r_back² (the front disk's solid-angle
        // fraction of the back disk).
        if r_front >= r_back {
            return 1.0;
        }
        return ((r_front * r_front) / (r_back * r_back)).clamp(0.0, 1.0) as f32;
    }
    // Two-circle lens area, divided by the back-disk area.
    let rf2 = r_front * r_front;
    let rb2 = r_back * r_back;
    let d2 = sep * sep;
    let cos_alpha = ((d2 + rf2 - rb2) / (2.0 * sep * r_front)).clamp(-1.0, 1.0);
    let cos_beta = ((d2 + rb2 - rf2) / (2.0 * sep * r_back)).clamp(-1.0, 1.0);
    let alpha = cos_alpha.acos();
    let beta = cos_beta.acos();
    let lens = rf2 * (alpha - alpha.sin() * cos_alpha) + rb2 * (beta - beta.sin() * cos_beta);
    (lens / (std::f64::consts::PI * rb2)).clamp(0.0, 1.0) as f32
}

/// Four canonical contact instants for an occultation / eclipse pair.
///
/// `p1` and `p4` mark the first and last external contact (partial begin
/// / end); `p2` and `p3` mark internal contact (totality / annularity /
/// transit ingress / egress) when the geometry reaches a central phase.
/// `Option::None` means the event does not enter that phase inside the
/// requested time window.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContactTimes {
    pub p1: Option<f64>,
    pub p2: Option<f64>,
    pub p3: Option<f64>,
    pub p4: Option<f64>,
}

impl ContactTimes {
    /// `true` if the pair achieved external contact at all in the window.
    pub fn is_event(&self) -> bool {
        self.p1.is_some() || self.p4.is_some()
    }

    /// `true` if the pair entered a central phase (annular / transit / total).
    pub fn is_central(&self) -> bool {
        self.p2.is_some() && self.p3.is_some()
    }
}

/// Locate the four canonical contact times (P1..P4) inside
/// `[start_jd, end_jd]` for a moving apparent-disk pair sampled by `disks`.
///
/// `disks(jd)` must return the `(front, back)` apparent disks at Julian
/// Date `jd`. The helper drives the search with a 30 s grid scan followed
/// by ≤ 30 bisection refinements per contact, giving sub-second precision
/// against the 30 s contract pinned in `VALIDATION.md`.
///
/// The function is intentionally agnostic of which body is on top: pass
/// `(Moon, Sun)` for a solar eclipse, `(planet, Sun)` for a transit,
/// `(Earth umbra at Moon distance, Moon)` for a lunar eclipse, etc.
pub fn contact_times<F>(start_jd: f64, end_jd: f64, disks: F) -> ContactTimes
where
    F: Fn(f64) -> (ApparentDisk, ApparentDisk),
{
    // 30 s scan: small enough to catch the ~1 hr eclipse window and the
    // ≲ 7 min totality without missing it, large enough that two
    // bisections per refinement nail sub-second precision.
    const SCAN_STEP_SECONDS: f64 = 30.0;
    let step = SCAN_STEP_SECONDS / 86_400.0;

    if !(start_jd.is_finite() && end_jd.is_finite()) || end_jd <= start_jd {
        return ContactTimes::default();
    }

    // External separation = sep - (r_front + r_back); zero at P1/P4.
    // Internal separation = sep - |r_front - r_back|; zero at P2/P3.
    let external = |jd: f64| -> f64 {
        let (f, b) = disks(jd);
        f.separation_rad(b) - (f.angular_radius_rad + b.angular_radius_rad)
    };
    let internal = |jd: f64| -> f64 {
        let (f, b) = disks(jd);
        f.separation_rad(b) - (f.angular_radius_rad - b.angular_radius_rad).abs()
    };

    let p1 = find_first_zero(start_jd, end_jd, step, &external, FindMode::Descending);
    let p4 = find_first_zero(start_jd, end_jd, step, &external, FindMode::Ascending);
    let p2 = find_first_zero(start_jd, end_jd, step, &internal, FindMode::Descending);
    let p3 = find_first_zero(start_jd, end_jd, step, &internal, FindMode::Ascending);

    ContactTimes { p1, p2, p3, p4 }
}

#[derive(Clone, Copy)]
enum FindMode {
    /// Earliest sign change from positive to negative (start of contact).
    Descending,
    /// Latest sign change from negative to positive (end of contact).
    Ascending,
}

fn find_first_zero<F>(start: f64, end: f64, step: f64, f: &F, mode: FindMode) -> Option<f64>
where
    F: Fn(f64) -> f64,
{
    let mut prev_t = start;
    let mut prev_v = f(prev_t);
    if !prev_v.is_finite() {
        return None;
    }
    let mut found: Option<(f64, f64, f64, f64)> = None; // (lo, hi, vlo, vhi)
    let mut t = start + step;
    while t <= end + 1e-12 {
        let t_clamped = t.min(end);
        let v = f(t_clamped);
        if !v.is_finite() {
            prev_t = t_clamped;
            prev_v = v;
            t += step;
            continue;
        }
        let crossed = match mode {
            FindMode::Descending => prev_v > 0.0 && v <= 0.0,
            FindMode::Ascending => prev_v <= 0.0 && v > 0.0,
        };
        if crossed {
            found = Some((prev_t, t_clamped, prev_v, v));
            if matches!(mode, FindMode::Descending) {
                break;
            }
        }
        prev_t = t_clamped;
        prev_v = v;
        t += step;
    }
    let (mut lo, mut hi, mut vlo, mut vhi) = found?;
    for _ in 0..30 {
        let mid = 0.5 * (lo + hi);
        let vmid = f(mid);
        if !vmid.is_finite() {
            return None;
        }
        if (vlo <= 0.0) == (vmid <= 0.0) {
            lo = mid;
            vlo = vmid;
        } else {
            hi = mid;
            vhi = vmid;
        }
        if (hi - lo) * 86_400.0 < 0.05 {
            // Sub-50-ms precision is well below the 30 s validation contract.
            break;
        }
    }
    let _ = (vhi, vlo); // silence unused warnings on tight convergence.
    Some(0.5 * (lo + hi))
}

fn separation_rad(a: Vec3, b: Vec3) -> f64 {
    let ax = a.x as f64;
    let ay = a.y as f64;
    let az = a.z as f64;
    let bx = b.x as f64;
    let by = b.y as f64;
    let bz = b.z as f64;
    let an = (ax * ax + ay * ay + az * az).sqrt();
    let bn = (bx * bx + by * by + bz * bz).sqrt();
    if an <= 0.0 || bn <= 0.0 {
        return f64::INFINITY;
    }
    let cos = ((ax * bx + ay * by + az * bz) / (an * bn)).clamp(-1.0, 1.0);
    cos.acos()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(angle_rad: f64) -> Vec3 {
        Vec3::new(angle_rad.cos() as f32, angle_rad.sin() as f32, 0.0)
    }

    #[test]
    fn disjoint_disks_classify_as_none() {
        let a = ApparentDisk::new(dir(0.0), 0.005);
        let b = ApparentDisk::new(dir(0.10), 0.005);
        assert_eq!(classify_disks(a, b), OccultationKind::None);
        assert_eq!(obscuration_fraction(a, b), 0.0);
    }

    #[test]
    fn touching_disks_are_partial() {
        // Moon ≈ Sun in radius (solar eclipse near-equality): half-overlap.
        let moon = ApparentDisk::new(dir(0.0), 0.00465);
        let sun = ApparentDisk::new(dir(0.005), 0.00465);
        let kind = classify_disks(moon, sun);
        assert_eq!(kind, OccultationKind::Partial);
        let f = obscuration_fraction(moon, sun);
        assert!((0.30..0.70).contains(&f), "got {f}");
    }

    #[test]
    fn concentric_total_eclipse_obscures_back_disk() {
        let moon = ApparentDisk::new(dir(0.0), 0.005);
        let sun = ApparentDisk::new(dir(0.0), 0.0046);
        assert_eq!(classify_disks(moon, sun), OccultationKind::Total);
        assert!((obscuration_fraction(moon, sun) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn concentric_annular_obscuration_is_area_ratio() {
        let moon = ApparentDisk::new(dir(0.0), 0.0040);
        let sun = ApparentDisk::new(dir(0.0), 0.0050);
        assert_eq!(classify_disks(moon, sun), OccultationKind::AnnularOrTransit);
        let expected = (0.0040_f64 / 0.0050).powi(2) as f32;
        assert!((obscuration_fraction(moon, sun) - expected).abs() < 1e-6);
    }

    #[test]
    fn point_source_back_is_either_in_or_out() {
        let moon = ApparentDisk::new(dir(0.0), 0.00465);
        let star_in = ApparentDisk::new(dir(0.001), 0.0);
        let star_out = ApparentDisk::new(dir(0.10), 0.0);
        assert!(classify_disks(moon, star_in).is_central());
        assert_eq!(obscuration_fraction(moon, star_in), 1.0);
        assert_eq!(classify_disks(moon, star_out), OccultationKind::None);
        assert_eq!(obscuration_fraction(moon, star_out), 0.0);
    }

    #[test]
    fn contact_times_bracket_a_synthetic_solar_eclipse() {
        // Move the Moon across a stationary Sun at a uniform angular rate.
        // Pin contact times to known values: the Moon's centre passes the
        // Sun's centre at jd=1.0, the disks first touch when their centres
        // are 2r apart, etc.
        let r = 0.00465_f64;
        let rate = 4.0 * r; // rad / day
        let sun = ApparentDisk::new(Vec3::new(1.0, 0.0, 0.0), r);
        let p1_expected = 1.0 - (2.0 * r) / rate;
        let p4_expected = 1.0 + (2.0 * r) / rate;
        let p2_expected = 1.0; // r_front == r_back so internal == external = 0 only at sep=0.

        let disks = |jd: f64| -> (ApparentDisk, ApparentDisk) {
            let offset = (jd - 1.0) * rate;
            // Construct a Moon direction with the requested small offset
            // and unit length so separation == offset for small angles.
            let theta = offset;
            let moon_dir = Vec3::new(theta.cos() as f32, theta.sin() as f32, 0.0);
            (ApparentDisk::new(moon_dir, r), sun)
        };
        let contacts = contact_times(0.5, 1.5, disks);
        let p1 = contacts.p1.expect("p1 must exist");
        let p4 = contacts.p4.expect("p4 must exist");
        assert!(
            (p1 - p1_expected).abs() * 86_400.0 < 1.0,
            "p1 off: {p1} vs {p1_expected}"
        );
        assert!(
            (p4 - p4_expected).abs() * 86_400.0 < 1.0,
            "p4 off: {p4} vs {p4_expected}"
        );
        // Equal-radius case: P2 and P3 collapse to the central instant.
        if let (Some(p2), Some(p3)) = (contacts.p2, contacts.p3) {
            assert!((p2 - p2_expected).abs() * 86_400.0 < 1.0);
            assert!((p3 - p2_expected).abs() * 86_400.0 < 1.0);
        }
    }

    #[test]
    fn contact_times_handle_empty_window() {
        let r = 0.005_f64;
        let near = ApparentDisk::new(Vec3::new(1.0, 0.0, 0.0), r);
        let far = ApparentDisk::new(Vec3::new(0.0, 1.0, 0.0), r);
        let contacts = contact_times(0.0, 1.0, |_| (near, far));
        assert!(!contacts.is_event());
    }

    #[test]
    fn separation_is_symmetric_and_independent_of_frame_scale() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        let sep = separation_rad(a, b);
        assert!((sep - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        // Non-unit inputs are still classified by direction.
        let sep_scaled = separation_rad(a * 3.0, b * 7.0);
        assert!((sep - sep_scaled).abs() < 1e-9);
    }
}
