//! Observation-planning helpers: twilight states and rise/transit/set tables.

use crate::occultation::{
    classify_disks, contact_times, obscuration_fraction, ActiveOccluders, ApparentDisk,
    ContactTimes, Occluder, OccluderTarget, OccultationKind,
};
use crate::{
    apparent_moon_topocentric, apparent_planet_topocentric, apparent_sun_topocentric,
    equatorial_to_horizontal, lmst_radians, Observer, Planet, TimeScales, SECONDS_PER_DAY,
};
use glam::Vec3;

const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;
const SEARCH_STEP_DAYS: f64 = 10.0 / (24.0 * 60.0); // 10 minutes
const REFINE_ITERS: usize = 28;
const SIDEREAL_RATE: f64 = 1.002_737_909_35;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanningBody {
    Sun,
    Moon,
    Planet(Planet),
}

impl PlanningBody {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sun => "Sun",
            Self::Moon => "Moon",
            Self::Planet(planet) => planet.name(),
        }
    }

    fn standard_altitude_rad(self) -> f64 {
        match self {
            // Almanac sunrise/sunset: centre at -50′ accounts for refraction + solar radius.
            Self::Sun => -0.833_333_333 * DEG_TO_RAD,
            // Approximate upper-limb Moon rise/set with mean refraction/parallax folded in.
            Self::Moon => 0.125 * DEG_TO_RAD,
            // Stellar/planetary apparent rise/set with standard refraction.
            Self::Planet(_) => -0.566_666_667 * DEG_TO_RAD,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwilightBand {
    Daylight,
    Civil,
    Nautical,
    Astronomical,
    Night,
}

impl TwilightBand {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Daylight => "Daylight",
            Self::Civil => "Civil twilight",
            Self::Nautical => "Nautical twilight",
            Self::Astronomical => "Astronomical twilight",
            Self::Night => "Night",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TwilightIndicator {
    pub band: TwilightBand,
    pub start_jd_utc: f64,
    pub end_jd_utc: f64,
}

#[derive(Debug, Clone)]
pub struct RiseTransitSet {
    pub name: &'static str,
    pub body: PlanningBody,
    pub rise_jd_utc: Option<f64>,
    pub transit_jd_utc: Option<f64>,
    pub set_jd_utc: Option<f64>,
    pub transit_altitude_rad: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct EveningPlan {
    pub start_jd_utc: f64,
    pub end_jd_utc: f64,
    pub rows: Vec<RiseTransitSet>,
    pub twilight: Vec<TwilightIndicator>,
}

pub const DEFAULT_PLANNING_BODIES: [PlanningBody; 9] = [
    PlanningBody::Sun,
    PlanningBody::Moon,
    PlanningBody::Planet(Planet::Mercury),
    PlanningBody::Planet(Planet::Venus),
    PlanningBody::Planet(Planet::Mars),
    PlanningBody::Planet(Planet::Jupiter),
    PlanningBody::Planet(Planet::Saturn),
    PlanningBody::Planet(Planet::Uranus),
    PlanningBody::Planet(Planet::Neptune),
];

pub fn twilight_band(sun_altitude_rad: f64) -> TwilightBand {
    let deg = sun_altitude_rad.to_degrees();
    if deg >= 0.0 {
        TwilightBand::Daylight
    } else if deg >= -6.0 {
        TwilightBand::Civil
    } else if deg >= -12.0 {
        TwilightBand::Nautical
    } else if deg >= -18.0 {
        TwilightBand::Astronomical
    } else {
        TwilightBand::Night
    }
}

/// UTC Julian Date of local civil midnight for the observer longitude near `jd_utc`.
fn local_midnight_jd_utc(jd_utc: f64, longitude_rad: f64) -> f64 {
    // Observer normalizes longitudes into [0, 2π), but civil local time needs
    // signed east-positive offset from Greenwich. Treat 350° as −10°, not a
    // nearly one-day positive offset, or western-hemisphere planning windows
    // can land on the wrong local date.
    let signed_longitude = if longitude_rad > std::f64::consts::PI {
        longitude_rad - std::f64::consts::TAU
    } else {
        longitude_rad
    };
    let longitude_days = signed_longitude / std::f64::consts::TAU;
    (jd_utc + longitude_days + 0.5).floor() - 0.5 - longitude_days
}

/// Start the planning window at local noon before the coming evening, ending 24h later.
pub fn evening_window_jd_utc(observer: Observer) -> (f64, f64) {
    let midnight = local_midnight_jd_utc(observer.time.jd_utc, observer.longitude_rad);
    let noon = midnight + 0.5;
    let start = if observer.time.jd_utc < noon {
        noon - 1.0
    } else {
        noon
    };
    (start, start + 1.0)
}

fn observer_at(observer: Observer, jd_utc: f64) -> Observer {
    Observer::from_degrees_with_time(
        observer.latitude_rad.to_degrees(),
        observer.longitude_rad.to_degrees(),
        TimeScales::from_utc_julian_date(jd_utc),
    )
}

pub fn body_equatorial(observer: Observer, body: PlanningBody) -> (f64, f64) {
    match body {
        PlanningBody::Sun => {
            let sun = apparent_sun_topocentric(observer);
            (sun.right_ascension_rad, sun.declination_rad)
        }
        PlanningBody::Moon => {
            let moon = apparent_moon_topocentric(observer);
            (moon.right_ascension_rad, moon.declination_rad)
        }
        PlanningBody::Planet(planet) => {
            let p = apparent_planet_topocentric(observer, planet);
            (p.right_ascension_rad, p.declination_rad)
        }
    }
}

pub fn body_altitude_rad(observer: Observer, body: PlanningBody) -> f64 {
    let (ra, dec) = body_equatorial(observer, body);
    let lst = lmst_radians(observer.time.jd_ut1, observer.longitude_rad);
    equatorial_to_horizontal(ra, dec, lst, observer.latitude_rad).altitude
}

fn normalize_event_into_window(mut jd_utc: f64, start_jd_utc: f64, end_jd_utc: f64) -> Option<f64> {
    let sidereal_day = 1.0 / SIDEREAL_RATE;
    while jd_utc < start_jd_utc {
        jd_utc += sidereal_day;
    }
    while jd_utc >= end_jd_utc {
        jd_utc -= sidereal_day;
    }
    (start_jd_utc..end_jd_utc)
        .contains(&jd_utc)
        .then_some(jd_utc)
}

pub fn rise_transit_set(
    observer: Observer,
    body: PlanningBody,
    start_jd_utc: f64,
    end_jd_utc: f64,
) -> RiseTransitSet {
    // Fast almanac-style approximation: evaluate each body once near the
    // middle of the planning window, then solve the hour-angle geometry
    // analytically. The previous implementation rescanned the whole day and
    // refined each event by repeatedly re-running the full VSOP87 planet
    // ephemeris; changing the date in the web UI could therefore block the
    // render loop for hundreds of planet evaluations. For a planning table,
    // minute-scale accuracy is enough and this keeps date changes interactive.
    let sample_jd = 0.5 * (start_jd_utc + end_jd_utc);
    let sample_observer = observer_at(observer, sample_jd);
    let (ra, dec) = body_equatorial(sample_observer, body);
    let start_observer = observer_at(observer, start_jd_utc);
    let lst_start = lmst_radians(start_observer.time.jd_ut1, start_observer.longitude_rad);
    let transit0 = start_jd_utc
        + (ra - lst_start).rem_euclid(std::f64::consts::TAU)
            / (std::f64::consts::TAU * SIDEREAL_RATE);
    let transit = normalize_event_into_window(transit0, start_jd_utc, end_jd_utc);

    let (sin_lat, cos_lat) = observer.latitude_rad.sin_cos();
    let (sin_dec, cos_dec) = dec.sin_cos();
    let h0 = body.standard_altitude_rad();
    let denom = cos_lat * cos_dec;
    let cos_h = if denom.abs() > 1e-12 {
        (h0.sin() - sin_lat * sin_dec) / denom
    } else {
        f64::NAN
    };
    let (rise, set) = if let Some(transit_jd) = transit {
        if (-1.0..=1.0).contains(&cos_h) {
            let dt = cos_h.acos() / (std::f64::consts::TAU * SIDEREAL_RATE);
            (
                normalize_event_into_window(transit_jd - dt, start_jd_utc, end_jd_utc),
                normalize_event_into_window(transit_jd + dt, start_jd_utc, end_jd_utc),
            )
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    let transit_altitude_rad =
        transit.map(|transit_jd| body_altitude_rad(observer_at(observer, transit_jd), body));

    RiseTransitSet {
        name: body.name(),
        body,
        rise_jd_utc: rise,
        transit_jd_utc: transit,
        set_jd_utc: set,
        transit_altitude_rad,
    }
}

pub fn twilight_indicators(
    observer: Observer,
    start_jd_utc: f64,
    end_jd_utc: f64,
) -> Vec<TwilightIndicator> {
    let mut out = Vec::new();
    let mut seg_start = start_jd_utc;
    let mut prev_t = start_jd_utc;
    let mut prev_band = twilight_band(body_altitude_rad(
        observer_at(observer, prev_t),
        PlanningBody::Sun,
    ));
    let mut t = start_jd_utc + SEARCH_STEP_DAYS;
    while t <= end_jd_utc + 1e-12 {
        let cur_t = t.min(end_jd_utc);
        let cur_band = twilight_band(body_altitude_rad(
            observer_at(observer, cur_t),
            PlanningBody::Sun,
        ));
        if cur_band != prev_band {
            let boundary = bisect_twilight_boundary(observer, prev_t, cur_t, prev_band, cur_band);
            out.push(TwilightIndicator {
                band: prev_band,
                start_jd_utc: seg_start,
                end_jd_utc: boundary,
            });
            seg_start = boundary;
            prev_band = cur_band;
        }
        prev_t = cur_t;
        t += SEARCH_STEP_DAYS;
    }
    out.push(TwilightIndicator {
        band: prev_band,
        start_jd_utc: seg_start,
        end_jd_utc,
    });
    out
}

fn bisect_twilight_boundary(
    observer: Observer,
    mut lo: f64,
    mut hi: f64,
    lo_band: TwilightBand,
    hi_band: TwilightBand,
) -> f64 {
    for _ in 0..REFINE_ITERS {
        let mid = 0.5 * (lo + hi);
        let band = twilight_band(body_altitude_rad(
            observer_at(observer, mid),
            PlanningBody::Sun,
        ));
        if band == lo_band {
            lo = mid;
        } else if band == hi_band {
            hi = mid;
        } else {
            // A large step cannot skip more than one twilight boundary in normal use,
            // but keep the interval shrinking if it ever does near polar day/night.
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

pub fn evening_plan(observer: Observer) -> EveningPlan {
    let (start, end) = evening_window_jd_utc(observer);
    let rows = DEFAULT_PLANNING_BODIES
        .into_iter()
        .map(|body| rise_transit_set(observer, body, start, end))
        .collect();
    let twilight = twilight_indicators(observer, start, end);
    EveningPlan {
        start_jd_utc: start,
        end_jd_utc: end,
        rows,
        twilight,
    }
}

pub fn jd_utc_to_unix_ms(jd_utc: f64) -> f64 {
    (jd_utc - crate::UNIX_EPOCH_JD) * SECONDS_PER_DAY * 1000.0
}

/// Instantaneous solar-eclipse state for one observer at one instant
/// (V-51c). Pairs the [`OccultationKind`] of the Moon-on-Sun geometry
/// with the fraction of the solar disk currently hidden by the Moon, so
/// the renderer can scale daylight scattering and draw the analytic
/// occluder mask without re-running the geometry per fragment.
#[derive(Debug, Clone, Copy, Default)]
pub struct SolarEclipseState {
    pub kind: SolarEclipseKind,
    pub obscuration: f32,
}

/// Renderer-facing classification of the current solar-eclipse phase.
/// Mirrors [`OccultationKind`] but is dedicated to the `Moon-occults-Sun`
/// pair so the uniform can be a small fixed-size enum the shader unpacks
/// without a branch table.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SolarEclipseKind {
    /// No contact this instant.
    #[default]
    None,
    /// Partial solar eclipse (limbs touching but not fully inside).
    Partial,
    /// Annular solar eclipse (Moon fully inside the solar disk).
    Annular,
    /// Total solar eclipse (solar disk fully behind the Moon).
    Total,
}

impl SolarEclipseKind {
    pub const fn as_kebab_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Partial => "partial",
            Self::Annular => "annular",
            Self::Total => "total",
        }
    }

    /// Numeric label the shader uses to switch corona / Koomen falloff
    /// branches. Stable across hosts so deterministic re-renders stay
    /// bit-identical.
    pub const fn shader_code(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Partial => 1.0,
            Self::Annular => 2.0,
            Self::Total => 3.0,
        }
    }

    fn from_occultation(kind: OccultationKind, moon_radius: f64, sun_radius: f64) -> Self {
        match kind {
            OccultationKind::None => Self::None,
            OccultationKind::Partial => Self::Partial,
            OccultationKind::AnnularOrTransit => Self::Annular,
            OccultationKind::Total => {
                // The classifier returns `Total` whenever the front disk
                // contains the back disk. For the Moon-on-Sun pair this
                // is a true total solar eclipse only when the Moon is at
                // least as large as the Sun; otherwise it would be an
                // annular event already filtered above. The check here
                // is a defensive guard against floating-point ties.
                if moon_radius >= sun_radius {
                    Self::Total
                } else {
                    Self::Annular
                }
            }
        }
    }
}

/// Compute the instantaneous solar-eclipse state for `observer`.
///
/// This is what the renderer reads every frame: it folds the Moon and
/// Sun apparent topocentric disks into an [`OccultationKind`] and the
/// fraction of the solar disk currently hidden. The helper is cheap
/// (two ephemeris calls + closed-form geometry) so hosts can call it
/// every frame without caching gymnastics.
pub fn solar_eclipse_state(observer: Observer) -> SolarEclipseState {
    let sun = apparent_sun_topocentric(observer);
    let moon = apparent_moon_topocentric(observer);
    let sun_disk = ApparentDisk::new(sun.direction_equatorial(), sun.angular_radius_rad);
    let moon_disk = ApparentDisk::new(moon.direction_equatorial(), moon.angular_radius_rad);
    let kind = SolarEclipseKind::from_occultation(
        classify_disks(moon_disk, sun_disk),
        moon.angular_radius_rad,
        sun.angular_radius_rad,
    );
    let obscuration = if matches!(kind, SolarEclipseKind::None) {
        0.0
    } else {
        obscuration_fraction(moon_disk, sun_disk)
    };
    SolarEclipseState { kind, obscuration }
}

/// Build the list of analytic occluders active for `observer` at the
/// current instant (V-51b).
///
/// This is the producer the renderer reads each frame to populate its
/// `MAX_OCCLUDERS` uniform array. The shader runs one analytic subtract
/// mask per entry, so the function only emits *visible* occluders
/// (apparent disks currently in contact); off-eclipse observers get an
/// empty list and the shader short-circuits on `count == 0`.
///
/// Wired producers (one [`Occluder`] per active pair):
///
/// * **V-51c** — Moon-occults-Sun ([`OccluderTarget::Sun`], same
///   geometry as [`solar_eclipse_state`]), emitted only when the
///   Moon-Sun pair is in contact so off-eclipse frames stay empty.
/// * **V-51d** — Moon-occults-stars
///   ([`OccluderTarget::Stars`], emitted *unconditionally* so the
///   star vertex shader can cull catalog sprites behind the lunar
///   disk every frame; the analytic [`disk_mask`](`crate::occultation`)
///   leaves frames far from any occultation bit-identical).
/// * **V-51d** — Moon-occults-planet
///   ([`OccluderTarget::Planet`] backed by the Moon), emitted only
///   when the Moon-planet pair is in contact.
///
/// Open producers (TODO):
///
/// * `V-51e` — Mercury / Venus transits across the Sun
///   ([`OccluderTarget::Sun`] backed by a planet disk).
/// * `V-51f` — mutual planetary occultation
///   ([`OccluderTarget::Planet`] backed by another planet disk).
///
/// The list is bounded by [`crate::occultation::MAX_OCCLUDERS`]; pushes
/// past capacity are silently dropped rather than allocated.
pub fn active_occluders(observer: Observer) -> ActiveOccluders {
    let mut out = ActiveOccluders::EMPTY;

    // V-51c Moon-on-Sun. Mirrors `solar_eclipse_state` so the analytic-
    // mask path and the Sun-specific photometric falloff cannot drift.
    let sun = apparent_sun_topocentric(observer);
    let moon = apparent_moon_topocentric(observer);
    let sun_disk = ApparentDisk::new(sun.direction_equatorial(), sun.angular_radius_rad);
    let moon_disk = ApparentDisk::new(moon.direction_equatorial(), moon.angular_radius_rad);
    let moon_dir = moon.direction_equatorial();
    let moon_front_dir = [moon_dir.x as f64, moon_dir.y as f64, moon_dir.z as f64];
    let moon_sun_kind = classify_disks(moon_disk, sun_disk);
    if !matches!(moon_sun_kind, OccultationKind::None) {
        let obscuration = obscuration_fraction(moon_disk, sun_disk) as f64;
        let _ = out.push(Occluder {
            front_dir_eq: moon_front_dir,
            front_radius_rad: moon.angular_radius_rad,
            target: OccluderTarget::Sun,
            kind: moon_sun_kind,
            obscuration,
        });
    }

    // V-51d Moon-on-Stars: emit unconditionally. The shader's analytic
    // disk-mask is the actual gate — catalog stars whose direction does
    // not fall inside the Moon's apparent disk are unaffected, so frames
    // far from any lunar occultation stay bit-identical to the pre-V-51d
    // render. Emitting one entry every frame costs the star vertex
    // shader a single dot-product per active occluder; well under the
    // V-51 "no measurable fps regression" contract.
    let _ = out.push(Occluder {
        front_dir_eq: moon_front_dir,
        front_radius_rad: moon.angular_radius_rad,
        target: OccluderTarget::Stars,
        // The `kind` and `obscuration` fields are read by Sun-specific
        // photometric paths only; for the star cull they are inert. We
        // use `AnnularOrTransit` as a stable sentinel meaning "point
        // sources fully inside are hidden".
        kind: OccultationKind::AnnularOrTransit,
        obscuration: 0.0,
    });

    // V-51d Moon-on-Planet: classify each Moon ↔ planet pair, push only
    // the active ones so the analytic-mask path stays at zero cost
    // off-event. Indices follow `Planet::ALL`, which is also the order
    // the renderer packs into `planet_eq_radius[i]`, so the shader's
    // `OCCLUDER_TARGET_PLANET_BASE + i` lookup matches.
    for (i, &planet) in Planet::ALL.iter().enumerate() {
        let p = apparent_planet_topocentric(observer, planet);
        let p_disk = ApparentDisk::new(p.direction_equatorial(), p.angular_radius_rad);
        let kind = classify_disks(moon_disk, p_disk);
        if matches!(kind, OccultationKind::None) {
            continue;
        }
        let obscuration = obscuration_fraction(moon_disk, p_disk) as f64;
        let _ = out.push(Occluder {
            front_dir_eq: moon_front_dir,
            front_radius_rad: moon.angular_radius_rad,
            target: OccluderTarget::Planet(i as u8),
            kind,
            obscuration,
        });
    }

    out
}

/// Body the Moon may occult (V-51d).
///
/// Stars are point sources; the caller passes the apparent direction
/// already mapped into the *equatorial-of-date* frame used by
/// [`apparent_moon_topocentric`] (proper motion, annual aberration, and
/// precession baked in). The direction is treated as fixed across the
/// contact window: a star moves by < 0.01″ over the 1–2 hr event while
/// the Moon sweeps ~5°, so star sidereal motion is negligible at the
/// IOTA contact-time validation contract (5 s).
#[derive(Debug, Clone, Copy)]
pub enum LunarOccultedBody {
    Star { dir_date_eq: Vec3 },
    Planet(Planet),
}

/// One lunar occultation event located inside a planning window.
#[derive(Debug, Clone, Copy)]
pub struct LunarOccultationEvent {
    /// Deepest geometry reached inside the window (peak phase).
    pub kind: OccultationKind,
    /// Minimum Moon-body angular separation in radians inside the
    /// window. Equivalent to the back-disk-centre closest approach.
    pub min_separation_rad: f64,
    /// Julian Date (UTC) of minimum separation.
    pub peak_jd_utc: f64,
    /// Canonical P1..P4 contact times (UTC Julian Dates). For point
    /// sources P1 ≈ P2 and P3 ≈ P4 because external and internal
    /// contact coincide (the back disk has zero radius); the helper
    /// fills both so the same `ContactTimes` shape covers stars,
    /// planets, and solar eclipses uniformly.
    pub contacts: ContactTimes,
}

impl LunarOccultationEvent {
    pub fn is_central(&self) -> bool {
        matches!(
            self.kind,
            OccultationKind::AnnularOrTransit | OccultationKind::Total
        )
    }
}

/// Search `[start_jd_utc, end_jd_utc]` for a lunar occultation of
/// `body` visible from `observer` (V-51d).
///
/// Returns `None` when the Moon-body apparent separation stays above
/// `r_moon + r_back` across the entire window. The Moon's apparent
/// motion is ~0.5°/hour, so the helper drives the search with a
/// 1-minute scan followed by [`contact_times`] bisection refinement
/// — sub-second precision against the 5 s contract pinned in
/// `VALIDATION.md` for IOTA-published lunar occultations.
///
/// Like [`find_solar_eclipse`], this is meant to be called once per
/// known event date with a ±12 h bracket. It does not try to enumerate
/// every occultation in a long window.
pub fn find_lunar_occultation(
    observer: Observer,
    body: LunarOccultedBody,
    start_jd_utc: f64,
    end_jd_utc: f64,
) -> Option<LunarOccultationEvent> {
    if !(start_jd_utc.is_finite() && end_jd_utc.is_finite()) || end_jd_utc <= start_jd_utc {
        return None;
    }
    let disks = |jd: f64| -> (ApparentDisk, ApparentDisk) {
        let obs = observer_at(observer, jd);
        let moon = apparent_moon_topocentric(obs);
        let front = ApparentDisk::new(moon.direction_equatorial(), moon.angular_radius_rad);
        let back = match body {
            LunarOccultedBody::Star { dir_date_eq } => ApparentDisk::new(dir_date_eq, 0.0),
            LunarOccultedBody::Planet(planet) => {
                let p = apparent_planet_topocentric(obs, planet);
                ApparentDisk::new(p.direction_equatorial(), p.angular_radius_rad)
            }
        };
        (front, back)
    };
    // 1-minute scan: lunar occultations have sharp ingress / egress
    // (≲ 1 s for a star, ~30 s for a planet). 1 minute is dense enough
    // to bracket both phases without missing a brief planet event
    // (the Moon's apparent diameter is ~31′, traversed in ~1 hr).
    let scan_step = 1.0 / (24.0 * 60.0);
    let mut peak_jd = start_jd_utc;
    let mut min_sep = f64::INFINITY;
    let mut peak_kind = OccultationKind::None;
    let mut t = start_jd_utc;
    while t <= end_jd_utc + 1e-12 {
        let t_clamped = t.min(end_jd_utc);
        let (f, b) = disks(t_clamped);
        let sep = f.separation_rad(b);
        if sep.is_finite() && sep < min_sep {
            min_sep = sep;
            peak_jd = t_clamped;
            peak_kind = classify_disks(f, b);
        }
        t += scan_step;
    }
    if matches!(peak_kind, OccultationKind::None) {
        return None;
    }
    let contacts = contact_times(start_jd_utc, end_jd_utc, disks);
    Some(LunarOccultationEvent {
        kind: peak_kind,
        min_separation_rad: min_sep,
        peak_jd_utc: peak_jd,
        contacts,
    })
}

/// One solar-eclipse event located inside a planning window.
#[derive(Debug, Clone, Copy)]
pub struct SolarEclipseEvent {
    /// Deepest phase reached anywhere in the window (peak obscuration).
    pub kind: SolarEclipseKind,
    /// Peak obscuration fraction `[0, 1]`.
    pub peak_obscuration: f32,
    /// Julian Date (UTC) of peak obscuration.
    pub peak_jd_utc: f64,
    /// Canonical P1..P4 contact times (UTC Julian Dates). `P2`/`P3` are
    /// `None` for purely partial events.
    pub contacts: ContactTimes,
}

impl SolarEclipseEvent {
    pub fn is_central(&self) -> bool {
        matches!(
            self.kind,
            SolarEclipseKind::Annular | SolarEclipseKind::Total
        )
    }
}

/// Search `[start_jd_utc, end_jd_utc]` for a solar eclipse visible from
/// `observer`. Returns `None` when the Moon-Sun apparent separation
/// never falls below `r_moon + r_sun` inside the window.
///
/// The search uses a coarse 5-minute scan to bracket P1 and a finer
/// pass via [`contact_times`] to refine the contact instants to ≤ 1 s.
/// This is the helper hosts call when adding eclipse markers to the
/// planning UI; it does *not* try to enumerate every eclipse in a long
/// window — call it once per known eclipse date in the canon and pass a
/// ±12 h bracket.
pub fn find_solar_eclipse(
    observer: Observer,
    start_jd_utc: f64,
    end_jd_utc: f64,
) -> Option<SolarEclipseEvent> {
    if !(start_jd_utc.is_finite() && end_jd_utc.is_finite()) || end_jd_utc <= start_jd_utc {
        return None;
    }
    let disks = |jd: f64| -> (ApparentDisk, ApparentDisk) {
        let obs = observer_at(observer, jd);
        let sun = apparent_sun_topocentric(obs);
        let moon = apparent_moon_topocentric(obs);
        (
            ApparentDisk::new(moon.direction_equatorial(), moon.angular_radius_rad),
            ApparentDisk::new(sun.direction_equatorial(), sun.angular_radius_rad),
        )
    };
    // 5 min scan for the peak; the eclipse window over Earth is several
    // hours wide so this is dense enough to catch totality even when it
    // is only a few minutes long.
    let scan_step = 5.0 / (24.0 * 60.0);
    let mut peak_jd = start_jd_utc;
    let mut peak_obscuration = 0.0_f32;
    let mut peak_kind = SolarEclipseKind::None;
    let mut t = start_jd_utc;
    while t <= end_jd_utc + 1e-12 {
        let (front, back) = disks(t.min(end_jd_utc));
        let kind = SolarEclipseKind::from_occultation(
            classify_disks(front, back),
            front.angular_radius_rad,
            back.angular_radius_rad,
        );
        let obs = obscuration_fraction(front, back);
        if obs > peak_obscuration {
            peak_obscuration = obs;
            peak_jd = t.min(end_jd_utc);
            peak_kind = kind;
        }
        t += scan_step;
    }
    if !peak_kind.is_event() || peak_obscuration <= 0.0 {
        return None;
    }
    let contacts = contact_times(start_jd_utc, end_jd_utc, disks);
    Some(SolarEclipseEvent {
        kind: peak_kind,
        peak_obscuration,
        peak_jd_utc: peak_jd,
        contacts,
    })
}

impl SolarEclipseKind {
    fn is_event(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twilight_bands_follow_solar_depression_thresholds() {
        assert_eq!(twilight_band(1.0_f64.to_radians()), TwilightBand::Daylight);
        assert_eq!(twilight_band((-3.0_f64).to_radians()), TwilightBand::Civil);
        assert_eq!(
            twilight_band((-9.0_f64).to_radians()),
            TwilightBand::Nautical
        );
        assert_eq!(
            twilight_band((-15.0_f64).to_radians()),
            TwilightBand::Astronomical
        );
        assert_eq!(twilight_band((-20.0_f64).to_radians()), TwilightBand::Night);
    }

    #[test]
    fn western_longitudes_use_signed_local_day_offset() {
        let east = local_midnight_jd_utc(2_460_000.5, 10_f64.to_radians());
        let west_wrapped = local_midnight_jd_utc(2_460_000.5, 350_f64.to_radians());
        assert!((east - 2_460_000.472_222_222).abs() < 1e-9);
        assert!((west_wrapped - 2_459_999.527_777_778).abs() < 1e-9);
    }

    fn jd_utc_from_iso_hours(year: i32, month: u32, day: u32, hour: f64) -> f64 {
        // Closed-form Gregorian → JD; matches `astro::time::julian_day` for the
        // dates used in the eclipse tests below.
        let (y, m) = if month <= 2 {
            (year - 1, month + 12)
        } else {
            (year, month)
        };
        let a = (y as f64 / 100.0).floor();
        let b = 2.0 - a + (a / 4.0).floor();
        let jd_midnight = (365.25 * (y as f64 + 4716.0)).floor()
            + (30.6001 * (m as f64 + 1.0)).floor()
            + day as f64
            + b
            - 1524.5;
        jd_midnight + hour / 24.0
    }

    #[test]
    fn find_solar_eclipse_finds_2024_mazatlan_totality() {
        // 2024-04-08 total solar eclipse. Peak over Mazatlán ≈ 18:13:08 UT,
        // totality 4m 17s. Bracket a ~6 h local window so the contact-time
        // refinement can lock onto P1..P4 regardless of the exact ephemeris
        // peak.
        let start = jd_utc_from_iso_hours(2024, 4, 8, 15.0);
        let end = jd_utc_from_iso_hours(2024, 4, 8, 21.0);
        let mid = 0.5 * (start + end);
        let observer = Observer::from_degrees(23.219, -106.420, mid);
        let event = find_solar_eclipse(observer, start, end)
            .expect("2024 Mazatl\u{00e1}n total eclipse must be detected");
        assert!(
            matches!(event.kind, SolarEclipseKind::Total),
            "expected Total at Mazatl\u{00e1}n peak, got {:?} (peak obs {})",
            event.kind,
            event.peak_obscuration
        );
        assert!(
            event.peak_obscuration > 0.999,
            "peak obscuration too low: {}",
            event.peak_obscuration
        );
        let p2 = event.contacts.p2.expect("P2 must exist for totality");
        let p3 = event.contacts.p3.expect("P3 must exist for totality");
        let totality_seconds = (p3 - p2) * 86_400.0;
        assert!(
            (60.0..600.0).contains(&totality_seconds),
            "totality duration {totality_seconds} s outside the 1-10 min plausibility band",
        );
    }

    #[test]
    fn find_solar_eclipse_finds_2012_tokyo_annular() {
        // 2012-05-21 annular eclipse over Tokyo, peak ≈ 22:34 UT on 2012-05-20
        // (07:34 JST on the 21st). Bracket the whole local morning.
        let start = jd_utc_from_iso_hours(2012, 5, 20, 19.0);
        let end = jd_utc_from_iso_hours(2012, 5, 21, 1.0);
        let mid = 0.5 * (start + end);
        let observer = Observer::from_degrees(35.68, 139.69, mid);
        let event = find_solar_eclipse(observer, start, end)
            .expect("2012 Tokyo annular eclipse must be detected");
        assert!(
            matches!(
                event.kind,
                SolarEclipseKind::Annular | SolarEclipseKind::Partial
            ),
            "expected Annular (or deep Partial fallback) at Tokyo peak, got {:?}",
            event.kind
        );
        assert!(
            event.peak_obscuration > 0.80,
            "peak obscuration too low: {}",
            event.peak_obscuration
        );
        // Contact times must straddle the peak.
        let p1 = event.contacts.p1.expect("P1 must exist");
        let p4 = event.contacts.p4.expect("P4 must exist");
        assert!(p1 <= event.peak_jd_utc && event.peak_jd_utc <= p4);
    }

    #[test]
    fn active_occluders_off_eclipse_emits_only_moon_on_stars() {
        // V-51d: the Moon-on-Stars cull entry is always present so the
        // star vertex shader can hide catalog sprites behind the lunar
        // disk every frame. Off any solar / planet event the list
        // therefore contains exactly that one entry (with the lunar
        // apparent disk as the front).
        let start = jd_utc_from_iso_hours(2025, 7, 1, 0.0);
        let end = jd_utc_from_iso_hours(2025, 7, 1, 24.0);
        let observer = Observer::from_degrees(35.68, 139.69, 0.5 * (start + end));
        let list = active_occluders(observer);
        assert_eq!(
            list.len(),
            1,
            "expected only the Moon-on-Stars cull entry off-eclipse, got {}",
            list.len()
        );
        let occ = list.as_slice()[0];
        assert_eq!(occ.target, OccluderTarget::Stars);
        // Front disk must be the Moon: radius matches the ELP2000
        // topocentric apparent semidiameter (~0.25°).
        let moon = apparent_moon_topocentric(observer);
        assert!(
            (occ.front_radius_rad - moon.angular_radius_rad).abs() < 1.0e-9,
            "Moon-on-Stars front radius {} != Moon apparent radius {}",
            occ.front_radius_rad,
            moon.angular_radius_rad,
        );
    }

    #[test]
    fn active_occluders_match_solar_eclipse_state_at_mazatlan_peak() {
        // V-51b parity contract: when only the Moon-on-Sun pair is active,
        // `active_occluders` must agree with `solar_eclipse_state` on the
        // kind + obscuration. Anything else would let the analytic-mask
        // path drift away from the Sun-specific photometric falloff.
        let start = jd_utc_from_iso_hours(2024, 4, 8, 15.0);
        let end = jd_utc_from_iso_hours(2024, 4, 8, 21.0);
        let event = find_solar_eclipse(
            Observer::from_degrees(23.219, -106.420, 0.5 * (start + end)),
            start,
            end,
        )
        .expect("Mazatl\u{00e1}n totality must be detected");
        let peak_observer = Observer::from_degrees(23.219, -106.420, event.peak_jd_utc);

        let state = solar_eclipse_state(peak_observer);
        let list = active_occluders(peak_observer);

        // V-51c Moon-on-Sun + V-51d Moon-on-Stars cull entry are both
        // emitted; the Sun pair is the one this test asserts against.
        let occ = *list
            .as_slice()
            .iter()
            .find(|o| o.target == OccluderTarget::Sun)
            .expect("Moon-on-Sun occluder must be present at the Mazatl\u{00e1}n peak");
        // Both producers must classify the deepest geometry the same way.
        // `solar_eclipse_state` collapses `Total`/`AnnularOrTransit` based
        // on relative radii; `Occluder::kind` is the raw pair-wise label.
        match state.kind {
            SolarEclipseKind::Total => assert_eq!(occ.kind, OccultationKind::Total),
            SolarEclipseKind::Annular => {
                assert_eq!(occ.kind, OccultationKind::AnnularOrTransit)
            }
            SolarEclipseKind::Partial => assert_eq!(occ.kind, OccultationKind::Partial),
            SolarEclipseKind::None => panic!("peak frame must be eclipsing"),
        }
        assert!(
            (occ.obscuration as f32 - state.obscuration).abs() < 1.0e-6,
            "obscuration drift: occluder {} vs state {}",
            occ.obscuration,
            state.obscuration,
        );
        // Front-disk direction must be a unit vector aligned with the
        // Moon's apparent topocentric direction.
        let n2 = occ.front_dir_eq[0] * occ.front_dir_eq[0]
            + occ.front_dir_eq[1] * occ.front_dir_eq[1]
            + occ.front_dir_eq[2] * occ.front_dir_eq[2];
        assert!(
            (n2 - 1.0).abs() < 1.0e-5,
            "front_dir_eq not unit: |v|\u{00b2} = {n2}"
        );
    }

    #[test]
    fn find_solar_eclipse_returns_none_on_non_eclipse_day() {
        let start = jd_utc_from_iso_hours(2025, 7, 1, 0.0);
        let end = jd_utc_from_iso_hours(2025, 7, 1, 24.0);
        let observer = Observer::from_degrees(35.68, 139.69, 0.5 * (start + end));
        assert!(find_solar_eclipse(observer, start, end).is_none());
    }

    #[test]
    fn find_lunar_occultation_returns_none_off_event() {
        // Mid-day Tokyo on a day with no scheduled lunar occultation of
        // Jupiter — the Moon-Jupiter apparent separation stays many
        // degrees throughout, so the helper must report no event.
        let start = jd_utc_from_iso_hours(2025, 7, 1, 0.0);
        let end = jd_utc_from_iso_hours(2025, 7, 1, 24.0);
        let observer = Observer::from_degrees(35.68, 139.69, 0.5 * (start + end));
        assert!(find_lunar_occultation(
            observer,
            LunarOccultedBody::Planet(Planet::Jupiter),
            start,
            end
        )
        .is_none());
    }

    #[test]
    fn find_lunar_occultation_detects_synthetic_point_source() {
        // Drive the helper with a fixed direction sitting right on the
        // ecliptic at a Moon-track epoch and assert it both finds the
        // event and pins external contact times symmetrically around
        // the closest approach. This guards the planning surface without
        // depending on the long-term ELP2000 accuracy.
        let start = jd_utc_from_iso_hours(2024, 4, 8, 15.0);
        let end = jd_utc_from_iso_hours(2024, 4, 8, 21.0);
        let mid = 0.5 * (start + end);
        let observer = Observer::from_degrees(23.219, -106.420, mid);
        // At Mazatlán peak the Moon is at the apparent Sun direction
        // (V-51c Total). Pointing the synthetic "star" exactly there
        // guarantees the Moon disk covers it across the totality
        // window, mimicking a daytime lunar occultation of a planet at
        // greatest eclipse.
        let mid_obs = observer_at(observer, mid);
        let sun = apparent_sun_topocentric(mid_obs);
        let body = LunarOccultedBody::Star {
            dir_date_eq: sun.direction_equatorial(),
        };
        let event = find_lunar_occultation(observer, body, start, end)
            .expect("point source aligned with the Sun at greatest eclipse must be occulted");
        assert!(
            event.is_central(),
            "expected central (AnnularOrTransit/Total) phase, got {:?}",
            event.kind
        );
        let p1 = event.contacts.p1.expect("P1 must exist");
        let p4 = event.contacts.p4.expect("P4 must exist");
        assert!(
            p1 <= event.peak_jd_utc && event.peak_jd_utc <= p4,
            "peak must lie inside [P1, P4]"
        );
        // Geometry sanity: closest approach must be below the Moon's
        // apparent radius for any central event.
        let peak_moon = apparent_moon_topocentric(observer_at(observer, event.peak_jd_utc));
        assert!(
            event.min_separation_rad <= peak_moon.angular_radius_rad + 1.0e-6,
            "min separation {} exceeds Moon apparent radius {}",
            event.min_separation_rad,
            peak_moon.angular_radius_rad,
        );
    }

    #[test]
    fn tokyo_evening_plan_has_rows_and_ordered_window() {
        let observer = Observer::from_degrees(35.68, 139.69, 2_460_482.5);
        let plan = evening_plan(observer);
        assert_eq!(plan.rows.len(), DEFAULT_PLANNING_BODIES.len());
        assert!(plan.start_jd_utc < plan.end_jd_utc);
        assert!(plan.rows.iter().all(|row| row.transit_jd_utc.is_some()));
        assert!(plan
            .twilight
            .windows(2)
            .all(|pair| pair[0].end_jd_utc <= pair[1].start_jd_utc + 1e-9));
    }
}
