//! Observation-planning helpers: twilight states and rise/transit/set tables.

use crate::ephemeris::ASTRONOMICAL_UNIT_KM;
use crate::jupiter_shadows::{
    galilean_shadow_disks_at, JUPITER_OCCLUDER_TARGET, SHADOW_TRANSIT_KIND,
};
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
/// * **V-51e** — Mercury / Venus transit of the Sun
///   ([`OccluderTarget::Sun`] backed by a planet disk). Inner planets
///   only: outer planets cannot transit the solar disk from Earth.
///   Emitted only when the planet is closer to the observer than the
///   Sun (inferior-conjunction side); a superior conjunction places
///   the planet behind the Sun, where the pure-geometry classifier
///   would otherwise spuriously fire.
/// * **V-51f** — mutual planetary occultation
///   ([`OccluderTarget::Planet`] backed by another planet disk). For
///   each unordered pair the closer planet is the front disk; the
///   producer emits one entry per pair currently in contact, with the
///   target index pointing at the farther planet so the analytic-mask
///   shader path subtracts the front disk from the back planet's disk.
/// * **V-52d** — Galilean shadow transit on Jupiter
///   ([`OccluderTarget::Planet`]`(3)` = Jupiter). For each of Io /
///   Europa / Ganymede / Callisto, if the moon's Sun-projected
///   position falls inside Jupiter's apparent disk the producer
///   pushes one front-disk entry whose radius equals the moon's
///   physical radius divided by the Earth-Jupiter distance (the
///   silhouette extent). Off-event frames emit zero V-52d entries
///   and the analytic-mask shader path stays bit-identical to the
///   pre-V-52d render.
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

    // V-51d Moon-on-Planet, V-51e Planet-on-Sun, and V-51f
    // Planet-on-Planet all need the per-planet apparent disk. Compute
    // each one once up-front so the three producers share the work; the
    // indexing follows `Planet::ALL`, which is also the order the
    // renderer packs into `planet_eq_radius[i]`, so the shader's
    // `OCCLUDER_TARGET_PLANET_BASE + i` lookup matches.
    let planet_apparents: [_; 7] =
        core::array::from_fn(|i| apparent_planet_topocentric(observer, Planet::ALL[i]));
    let planet_disks: [ApparentDisk; 7] = core::array::from_fn(|i| {
        ApparentDisk::new(
            planet_apparents[i].direction_equatorial(),
            planet_apparents[i].angular_radius_rad,
        )
    });

    for (i, &planet) in Planet::ALL.iter().enumerate() {
        let p = planet_apparents[i];
        let p_disk = planet_disks[i];

        // V-51d Moon-on-Planet: classify each pair, push only the active
        // ones so the analytic-mask path stays at zero cost off-event.
        let moon_kind = classify_disks(moon_disk, p_disk);
        if !matches!(moon_kind, OccultationKind::None) {
            let obscuration = obscuration_fraction(moon_disk, p_disk) as f64;
            let _ = out.push(Occluder {
                front_dir_eq: moon_front_dir,
                front_radius_rad: moon.angular_radius_rad,
                target: OccluderTarget::Planet(i as u8),
                kind: moon_kind,
                obscuration,
            });
        }

        // V-51e Planet-on-Sun: only inner planets can transit the solar
        // disk from Earth, and the classifier is pure geometry so we
        // also gate on the planet being closer than the Sun. A superior
        // conjunction puts an inner planet behind the Sun with nearly
        // identical apparent direction; without this gate the classifier
        // would spuriously emit an "occlusion" of the Sun by Mercury or
        // Venus when the planet is in fact being hidden by the Sun.
        if !matches!(planet, Planet::Mercury | Planet::Venus) {
            continue;
        }
        if p.distance_au >= sun.distance_au {
            continue;
        }
        let planet_sun_kind = classify_disks(p_disk, sun_disk);
        if matches!(planet_sun_kind, OccultationKind::None) {
            continue;
        }
        let p_dir = p.direction_equatorial();
        let planet_obscuration = obscuration_fraction(p_disk, sun_disk) as f64;
        let _ = out.push(Occluder {
            front_dir_eq: [p_dir.x as f64, p_dir.y as f64, p_dir.z as f64],
            front_radius_rad: p.angular_radius_rad,
            target: OccluderTarget::Sun,
            kind: planet_sun_kind,
            obscuration: planet_obscuration,
        });
    }

    // V-52d Galilean shadow transits on Jupiter. The producer is
    // `galilean_shadow_disks_at`; we feed it the same Jupiter direction
    // / observer distance the renderer's planet-disk path uses so the
    // analytic mask sits exactly on the Jovian sky-plane pixels. Each
    // active shadow becomes one front disk targeting the Planet(3) =
    // Jupiter back disk. The producer returns `None` for moons whose
    // Sun-line projection misses Jupiter's disk, so off-event frames
    // emit zero entries and the V-51b shader short-circuits as
    // before.
    let jupiter_idx = JUPITER_OCCLUDER_TARGET;
    debug_assert!(matches!(jupiter_idx, OccluderTarget::Planet(3)));
    {
        let jupiter = &planet_apparents[3];
        let jupiter_dir_eq = jupiter.direction_equatorial();
        let jupiter_dir_f64 = [
            jupiter_dir_eq.x as f64,
            jupiter_dir_eq.y as f64,
            jupiter_dir_eq.z as f64,
        ];
        let jupiter_distance_km = jupiter.distance_au * ASTRONOMICAL_UNIT_KM;
        let shadows =
            galilean_shadow_disks_at(observer.time.jd_tdb, jupiter_dir_f64, jupiter_distance_km);
        for slot in shadows.iter().flatten() {
            // The shadow's apparent disk is the moon's silhouette on
            // Jupiter; its angular extent matches `moon.radius_km() /
            // Δ_jupiter`. Approximate obscuration is the area ratio
            // (front / back) — same closed form as V-51e Planet-on-Sun.
            let r_front = slot.angular_radius_rad;
            let r_back = jupiter.angular_radius_rad.max(1.0e-12);
            let area_ratio = ((r_front / r_back).powi(2) as f64).clamp(0.0, 1.0);
            let _ = out.push(Occluder {
                front_dir_eq: slot.direction_eq,
                front_radius_rad: r_front,
                target: jupiter_idx,
                kind: SHADOW_TRANSIT_KIND,
                obscuration: area_ratio,
            });
        }
    }

    // V-51f Planet-on-Planet. Iterate unordered pairs once; for each
    // pair pick the closer planet as the front disk (the classifier is
    // pure geometry, so the distance test is what stops a far planet
    // sitting behind a near planet from spuriously masking it). Mutual
    // planetary occultations are rare enough (~once per few decades)
    // that the inner double loop costs ~21 dot products off-event and
    // pushes zero entries, well inside the analytic-mask "zero cost
    // off-event" contract.
    for i in 0..Planet::ALL.len() {
        for j in (i + 1)..Planet::ALL.len() {
            let (front_idx, back_idx) =
                if planet_apparents[i].distance_au <= planet_apparents[j].distance_au {
                    (i, j)
                } else {
                    (j, i)
                };
            let front_disk = planet_disks[front_idx];
            let back_disk = planet_disks[back_idx];
            let kind = classify_disks(front_disk, back_disk);
            if matches!(kind, OccultationKind::None) {
                continue;
            }
            let front_dir = planet_apparents[front_idx].direction_equatorial();
            let obscuration = obscuration_fraction(front_disk, back_disk) as f64;
            let _ = out.push(Occluder {
                front_dir_eq: [front_dir.x as f64, front_dir.y as f64, front_dir.z as f64],
                front_radius_rad: planet_apparents[front_idx].angular_radius_rad,
                target: OccluderTarget::Planet(back_idx as u8),
                kind,
                obscuration,
            });
        }
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

/// One Mercury / Venus transit located inside a planning window (V-51e).
#[derive(Debug, Clone, Copy)]
pub struct PlanetTransitEvent {
    /// Which inner planet transited the solar disk.
    pub planet: Planet,
    /// Deepest geometry reached anywhere in the window. Transits are
    /// always [`OccultationKind::AnnularOrTransit`] at peak — the planet
    /// disk is far smaller than the Sun — but the value is carried for
    /// uniformity with [`SolarEclipseEvent::kind`].
    pub kind: OccultationKind,
    /// Peak obscuration fraction `[0, 1]`. For a transit this is the
    /// area ratio `(r_planet / r_sun)²`, ≈2e-5 for Mercury and ≈1e-3
    /// for Venus.
    pub peak_obscuration: f32,
    /// Julian Date (UTC) of minimum apparent separation.
    pub peak_jd_utc: f64,
    /// Canonical P1..P4 contact times (UTC Julian Dates). P1 / P4 are
    /// the exterior contacts (first / last edge touch); P2 / P3 the
    /// interior contacts when the planet disk fully enters / starts to
    /// leave the solar disk.
    pub contacts: ContactTimes,
}

impl PlanetTransitEvent {
    /// `true` if the planet entered the interior phase (P2..P3) — the
    /// planet's disk fully inside the Sun's disk.
    pub fn is_interior(&self) -> bool {
        self.contacts.is_central()
    }
}

/// Search `[start_jd_utc, end_jd_utc]` for a Mercury or Venus transit
/// across the Sun visible from `observer` (V-51e).
///
/// Returns `None` when the planet-Sun apparent separation never falls
/// below `r_planet + r_sun` inside the window, or when `planet` is not
/// an inner planet (only Mercury and Venus can transit the solar disk
/// from Earth). Like [`find_solar_eclipse`], this is meant to be called
/// once per known transit date with a ~12 h bracket; it does not try to
/// enumerate every transit in a long window.
///
/// The classifier is pure geometry, so the helper additionally gates on
/// the planet being closer than the Sun at the peak instant — a
/// superior-conjunction near-alignment would otherwise appear identical
/// to a transit from the front.
pub fn find_planet_transit(
    observer: Observer,
    planet: Planet,
    start_jd_utc: f64,
    end_jd_utc: f64,
) -> Option<PlanetTransitEvent> {
    if !matches!(planet, Planet::Mercury | Planet::Venus) {
        return None;
    }
    if !(start_jd_utc.is_finite() && end_jd_utc.is_finite()) || end_jd_utc <= start_jd_utc {
        return None;
    }
    // Capture the per-sample (front, back) apparent disks plus the
    // foreground gate — `contact_times` runs on the disks alone, while
    // the peak-finding loop also enforces front < back distance so a
    // superior conjunction is rejected even before geometry.
    let probe = |jd: f64| -> (ApparentDisk, ApparentDisk, bool) {
        let obs = observer_at(observer, jd);
        let sun = apparent_sun_topocentric(obs);
        let p = apparent_planet_topocentric(obs, planet);
        let front = ApparentDisk::new(p.direction_equatorial(), p.angular_radius_rad);
        let back = ApparentDisk::new(sun.direction_equatorial(), sun.angular_radius_rad);
        let in_front = p.distance_au < sun.distance_au;
        (front, back, in_front)
    };
    // 5-minute scan: a Mercury transit ingress is ≲5 min wide, a Venus
    // transit ≳20 min; this catches both without missing the inner
    // contact pair. Matches the cadence `find_solar_eclipse` uses.
    let scan_step = 5.0 / (24.0 * 60.0);
    let mut peak_jd = start_jd_utc;
    let mut peak_obscuration = 0.0_f32;
    let mut peak_kind = OccultationKind::None;
    let mut t = start_jd_utc;
    while t <= end_jd_utc + 1e-12 {
        let (front, back, in_front) = probe(t.min(end_jd_utc));
        if !in_front {
            t += scan_step;
            continue;
        }
        let kind = classify_disks(front, back);
        let obs = obscuration_fraction(front, back);
        if obs > peak_obscuration {
            peak_obscuration = obs;
            peak_jd = t.min(end_jd_utc);
            peak_kind = kind;
        }
        t += scan_step;
    }
    if matches!(peak_kind, OccultationKind::None) || peak_obscuration <= 0.0 {
        return None;
    }
    let disks = |jd: f64| -> (ApparentDisk, ApparentDisk) {
        let (front, back, _) = probe(jd);
        (front, back)
    };
    let contacts = contact_times(start_jd_utc, end_jd_utc, disks);
    Some(PlanetTransitEvent {
        planet,
        kind: peak_kind,
        peak_obscuration,
        peak_jd_utc: peak_jd,
        contacts,
    })
}

/// One mutual planetary occultation event located inside a planning
/// window (V-51f).
#[derive(Debug, Clone, Copy)]
pub struct MutualPlanetaryOccultationEvent {
    /// Planet whose disk is in front at peak (closer to the observer).
    pub front: Planet,
    /// Planet whose disk is being occulted at peak (farther from the
    /// observer).
    pub back: Planet,
    /// Deepest geometry reached inside the window. Mutual planetary
    /// occultations span the full classifier range: a near miss is
    /// [`OccultationKind::Partial`], a small-on-large overlap such as
    /// Mercury behind Jupiter is [`OccultationKind::AnnularOrTransit`],
    /// and a large-on-small overlap such as Venus in front of Mars is
    /// [`OccultationKind::Total`].
    pub kind: OccultationKind,
    /// Minimum apparent separation between the two planet centres in
    /// radians anywhere in the window.
    pub min_separation_rad: f64,
    /// Peak obscuration fraction of the back disk in `[0, 1]`. Saturates
    /// at 1 for total events.
    pub peak_obscuration: f32,
    /// Julian Date (UTC) of minimum separation.
    pub peak_jd_utc: f64,
    /// Canonical P1..P4 contact times (UTC Julian Dates). `P2`/`P3` are
    /// `None` for purely grazing partial events.
    pub contacts: ContactTimes,
}

impl MutualPlanetaryOccultationEvent {
    /// `true` if the event entered a central phase (the smaller disk
    /// fully inside the larger, or the larger fully covering the
    /// smaller).
    pub fn is_central(&self) -> bool {
        self.kind.is_central()
    }
}

/// Search `[start_jd_utc, end_jd_utc]` for a mutual planetary
/// occultation of `planet_a` and `planet_b` visible from `observer`
/// (V-51f).
///
/// Returns `None` when the two planets are the same, when the window
/// is malformed, or when the apparent separation never falls below
/// `r_a + r_b` inside the window. The classifier is pure geometry, so
/// the helper assigns the closer planet at peak as the front disk and
/// the farther one as the back; the same assignment then drives the
/// `contact_times` bisection so `front`/`back` and the P1–P4 instants
/// agree.
///
/// Like [`find_solar_eclipse`] this is meant to be called once per
/// known event date with a ~12 h bracket; it does not try to enumerate
/// every mutual occultation in a long window.
pub fn find_mutual_planetary_occultation(
    observer: Observer,
    planet_a: Planet,
    planet_b: Planet,
    start_jd_utc: f64,
    end_jd_utc: f64,
) -> Option<MutualPlanetaryOccultationEvent> {
    if planet_a == planet_b {
        return None;
    }
    if !(start_jd_utc.is_finite() && end_jd_utc.is_finite()) || end_jd_utc <= start_jd_utc {
        return None;
    }
    // Per-sample apparent disks + the foreground decision. The closer
    // planet is the front disk; this drives both the producer ordering
    // and the obscuration-fraction direction.
    let probe = |jd: f64| -> (ApparentDisk, ApparentDisk, Planet, Planet) {
        let obs = observer_at(observer, jd);
        let a = apparent_planet_topocentric(obs, planet_a);
        let b = apparent_planet_topocentric(obs, planet_b);
        let a_disk = ApparentDisk::new(a.direction_equatorial(), a.angular_radius_rad);
        let b_disk = ApparentDisk::new(b.direction_equatorial(), b.angular_radius_rad);
        if a.distance_au <= b.distance_au {
            (a_disk, b_disk, planet_a, planet_b)
        } else {
            (b_disk, a_disk, planet_b, planet_a)
        }
    };
    // 1-minute scan: mutual planetary occultations have ingress / egress
    // widths of order a few minutes (the planets' apparent diameters
    // are tens of arcseconds and the slower of the two sweeps a few
    // arcsec / minute), so 1 minute is dense enough to bracket all four
    // contacts. The lunar-occultation helper uses the same cadence.
    let scan_step = 1.0 / (24.0 * 60.0);
    let mut peak_jd = start_jd_utc;
    let mut min_sep = f64::INFINITY;
    let mut peak_kind = OccultationKind::None;
    let mut peak_obscuration = 0.0_f32;
    let mut peak_front = planet_a;
    let mut peak_back = planet_b;
    let mut t = start_jd_utc;
    while t <= end_jd_utc + 1e-12 {
        let t_clamped = t.min(end_jd_utc);
        let (front, back, front_p, back_p) = probe(t_clamped);
        let sep = front.separation_rad(back);
        if sep.is_finite() && sep < min_sep {
            min_sep = sep;
            peak_jd = t_clamped;
            peak_kind = classify_disks(front, back);
            peak_obscuration = obscuration_fraction(front, back);
            peak_front = front_p;
            peak_back = back_p;
        }
        t += scan_step;
    }
    if matches!(peak_kind, OccultationKind::None) {
        return None;
    }
    let disks = |jd: f64| -> (ApparentDisk, ApparentDisk) {
        let (front, back, _, _) = probe(jd);
        (front, back)
    };
    let contacts = contact_times(start_jd_utc, end_jd_utc, disks);
    Some(MutualPlanetaryOccultationEvent {
        front: peak_front,
        back: peak_back,
        kind: peak_kind,
        min_separation_rad: min_sep,
        peak_obscuration,
        peak_jd_utc: peak_jd,
        contacts,
    })
}

// ---------------------------------------------------------------------------
// L-09 Observation-planning polish: Moon-impact, visibility scoring,
// recommended-target ranking, and iCalendar export.
//
// The Moon-impact model follows Krisciunas, K. & Schaefer, B. E. 1991,
// PASP 103, 1033, "A model of the brightness of moonlight". Sky-brightness
// luminances are carried in nanolamberts and converted to/from V-band
// surface brightness (mag/arcsec²) with their Eq. 27.
// ---------------------------------------------------------------------------

/// V-band atmospheric extinction coefficient (mag/airmass) adopted by the
/// Krisciunas-Schaefer 1991 moonlight model for a clear, dark site.
pub const KS_V_EXTINCTION_COEFF: f64 = 0.172;

/// Zenith dark-sky V-band surface brightness (mag/arcsec²) for a pristine
/// site. Used as the Moon-free baseline when no site brightness is supplied.
pub const DARK_SKY_ZENITH_V_MAG: f64 = 21.6;

/// Minimum target altitude (degrees) counted as "observable" when scoring the
/// fraction of the dark window a target spends usefully high.
pub const MIN_OBSERVABLE_ALTITUDE_DEG: f64 = 20.0;

/// Sun depression (degrees) below which the sky is treated as dark enough for
/// deep-sky observing (nautical twilight and darker).
pub const DARK_SUN_DEPRESSION_DEG: f64 = 12.0;

/// Krisciunas-Schaefer 1991 relative airmass (their Eq. 3):
/// `X(Z) = (1 − 0.96 sin²Z)^(−1/2)`. Returns a large but finite airmass near
/// and below the horizon so callers never see NaN/∞.
fn ks_airmass(zenith_rad: f64) -> f64 {
    let s = zenith_rad.sin();
    let denom = 1.0 - 0.96 * s * s;
    if denom <= 1.0e-6 {
        40.0
    } else {
        denom.powf(-0.5)
    }
}

/// Moon illuminance outside the atmosphere (K&S 1991 Eq. 20).
/// `phase_angle_deg` is the Sun-Moon-Earth phase angle in degrees
/// (0 = full Moon). The result combines with [`ks_scattering_function`]
/// to give a sky brightness in nanolamberts.
fn moon_illuminance_outside_atmosphere(phase_angle_deg: f64) -> f64 {
    let a = phase_angle_deg.abs().clamp(0.0, 180.0);
    10f64.powf(-0.4 * (3.84 + 0.026 * a + 4.0e-9 * a.powi(4)))
}

/// Krisciunas-Schaefer scattering function `f(ρ)` (their Eqs. 16–18) for an
/// angular separation `separation_deg` (deg) between the Moon and a sky
/// point. Sums a Rayleigh term and an aerosol (Mie) term.
fn ks_scattering_function(separation_deg: f64) -> f64 {
    let rho = separation_deg.clamp(0.0, 180.0);
    let cos_rho = rho.to_radians().cos();
    let rayleigh = 10f64.powf(5.36) * (1.06 + cos_rho * cos_rho);
    let mie = 10f64.powf(6.15 - rho / 40.0);
    rayleigh + mie
}

/// Convert a V-band surface brightness (mag/arcsec²) to luminance in
/// nanolamberts via K&S 1991 Eq. 27: `B = 34.08·exp(20.7233 − 0.92104 V)`.
pub fn nanolamberts_from_v_mag(v_mag_per_arcsec2: f64) -> f64 {
    34.08 * (20.7233 - 0.92104 * v_mag_per_arcsec2).exp()
}

/// Inverse of [`nanolamberts_from_v_mag`]: nanolamberts → V mag/arcsec².
pub fn v_mag_from_nanolamberts(nanolamberts: f64) -> f64 {
    let b = nanolamberts.max(1.0e-6);
    (20.7233 - (b / 34.08).ln()) / 0.92104
}

/// Moon contribution to sky surface brightness at a sky point, in
/// nanolamberts (K&S 1991 Eq. 15). Returns 0 when the Moon is below the
/// horizon.
pub fn moon_sky_brightness_nanolamberts(
    moon_phase_angle_deg: f64,
    moon_zenith_rad: f64,
    target_zenith_rad: f64,
    separation_deg: f64,
) -> f64 {
    if moon_zenith_rad >= std::f64::consts::FRAC_PI_2 {
        return 0.0;
    }
    let i_star = moon_illuminance_outside_atmosphere(moon_phase_angle_deg);
    let f = ks_scattering_function(separation_deg);
    let x_moon = ks_airmass(moon_zenith_rad);
    let x_target = ks_airmass(target_zenith_rad);
    let k = KS_V_EXTINCTION_COEFF;
    f * i_star * 10f64.powf(-0.4 * k * x_moon) * (1.0 - 10f64.powf(-0.4 * k * x_target))
}

/// Angular separation (radians) between two equatorial directions.
fn angular_separation_rad(ra1: f64, dec1: f64, ra2: f64, dec2: f64) -> f64 {
    let cos_sep = dec1.sin() * dec2.sin() + dec1.cos() * dec2.cos() * (ra1 - ra2).cos();
    cos_sep.clamp(-1.0, 1.0).acos()
}

/// Moonlight sky-brightness impact on a target at one instant.
#[derive(Debug, Clone, Copy)]
pub struct MoonImpact {
    pub moon_altitude_rad: f64,
    pub moon_illuminated_fraction: f64,
    pub separation_rad: f64,
    pub dark_sky_v_mag: f64,
    pub moonlit_sky_v_mag: f64,
    /// Sky-brightness degradation in V magnitudes (positive = brighter sky,
    /// i.e. worse contrast). Zero when the Moon is down or new.
    pub delta_v_mag: f64,
}

/// Moon-impact score for a fixed-equatorial target at the observer's instant,
/// against a Moon-free baseline `dark_sky_v_mag` (mag/arcsec²).
pub fn moon_impact(
    observer: Observer,
    target_ra_rad: f64,
    target_dec_rad: f64,
    dark_sky_v_mag: f64,
) -> MoonImpact {
    let moon = apparent_moon_topocentric(observer);
    let lst = lmst_radians(observer.time.jd_ut1, observer.longitude_rad);
    let moon_h = equatorial_to_horizontal(
        moon.right_ascension_rad,
        moon.declination_rad,
        lst,
        observer.latitude_rad,
    );
    let target_h =
        equatorial_to_horizontal(target_ra_rad, target_dec_rad, lst, observer.latitude_rad);
    let separation = angular_separation_rad(
        moon.right_ascension_rad,
        moon.declination_rad,
        target_ra_rad,
        target_dec_rad,
    );
    let moon_zenith = std::f64::consts::FRAC_PI_2 - moon_h.altitude;
    let target_zenith = std::f64::consts::FRAC_PI_2 - target_h.altitude;
    let b_moon = moon_sky_brightness_nanolamberts(
        moon.phase_angle_rad.to_degrees(),
        moon_zenith,
        target_zenith,
        separation.to_degrees(),
    );
    let b_dark = nanolamberts_from_v_mag(dark_sky_v_mag);
    let moonlit_v = v_mag_from_nanolamberts(b_dark + b_moon);
    MoonImpact {
        moon_altitude_rad: moon_h.altitude,
        moon_illuminated_fraction: moon.illuminated_fraction,
        separation_rad: separation,
        dark_sky_v_mag,
        moonlit_sky_v_mag: moonlit_v,
        delta_v_mag: dark_sky_v_mag - moonlit_v,
    }
}

/// Visibility score for a fixed-equatorial target over the coming evening.
#[derive(Debug, Clone, Copy)]
pub struct VisibilityScore {
    pub max_altitude_rad: f64,
    pub max_altitude_jd_utc: f64,
    pub observable_dark_hours: f64,
    pub total_dark_hours: f64,
    /// Observable dark window [start, end] in JD(UTC), if the target clears
    /// [`MIN_OBSERVABLE_ALTITUDE_DEG`] during darkness.
    pub observable_window_jd_utc: Option<(f64, f64)>,
    pub moon: MoonImpact,
    /// Composite score in [0, 1]: altitude × dark-window × Moon-clarity.
    pub score: f64,
}

/// Score how observable a fixed-equatorial target is over the evening window
/// returned by [`evening_window_jd_utc`].
///
/// The composite is the product of three documented terms, each in `[0, 1]`:
/// * altitude — `clamp(max_altitude / 60°, 0, 1)` (saturates at 60°),
/// * dark-window — `clamp(observable_dark_hours / 4 h, 0, 1)`,
/// * Moon-clarity — `10^(−0.4·max(ΔV, 0))`, the relative sky-brightness
///   factor from the [`moon_impact`] degradation (1 = no Moon impact).
pub fn visibility_score(
    observer: Observer,
    target_ra_rad: f64,
    target_dec_rad: f64,
    dark_sky_v_mag: f64,
) -> VisibilityScore {
    let (start, end) = evening_window_jd_utc(observer);
    let step = 5.0 / (24.0 * 60.0); // 5 minutes
    let min_alt = MIN_OBSERVABLE_ALTITUDE_DEG.to_radians();
    let dark_sun_alt = -DARK_SUN_DEPRESSION_DEG.to_radians();

    let mut max_alt = f64::NEG_INFINITY;
    let mut max_jd = start;
    let mut total_dark_days = 0.0;
    let mut obs_dark_days = 0.0;
    let mut win_start: Option<f64> = None;
    let mut win_end: Option<f64> = None;

    let mut t = start;
    while t <= end + 1e-12 {
        let obs_t = observer_at(observer, t);
        let lst = lmst_radians(obs_t.time.jd_ut1, obs_t.longitude_rad);
        let alt =
            equatorial_to_horizontal(target_ra_rad, target_dec_rad, lst, observer.latitude_rad)
                .altitude;
        if alt > max_alt {
            max_alt = alt;
            max_jd = t;
        }
        let sun_alt = body_altitude_rad(obs_t, PlanningBody::Sun);
        if sun_alt < dark_sun_alt {
            total_dark_days += step;
            if alt > min_alt {
                obs_dark_days += step;
                if win_start.is_none() {
                    win_start = Some(t);
                }
                win_end = Some(t);
            }
        }
        t += step;
    }

    let observable_dark_hours = obs_dark_days * 24.0;
    let total_dark_hours = total_dark_days * 24.0;
    let moon = moon_impact(
        observer_at(observer, max_jd),
        target_ra_rad,
        target_dec_rad,
        dark_sky_v_mag,
    );

    let altitude_term = (max_alt.to_degrees() / 60.0).clamp(0.0, 1.0);
    let dark_term = (observable_dark_hours / 4.0).clamp(0.0, 1.0);
    let moon_term = 10f64.powf(-0.4 * moon.delta_v_mag.max(0.0));
    let score = (altitude_term * dark_term * moon_term).clamp(0.0, 1.0);

    let observable_window_jd_utc = match (win_start, win_end) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    };

    VisibilityScore {
        max_altitude_rad: max_alt,
        max_altitude_jd_utc: max_jd,
        observable_dark_hours,
        total_dark_hours,
        observable_window_jd_utc,
        moon,
        score,
    }
}

/// A named, fixed-equatorial planning target supplied by a host (catalog
/// star, deep-sky object, or a solar-system body sampled at the evening's
/// midpoint).
#[derive(Debug, Clone)]
pub struct PlanningTarget {
    pub name: String,
    pub right_ascension_rad: f64,
    pub declination_rad: f64,
}

/// A planning target with its computed visibility score.
#[derive(Debug, Clone)]
pub struct ScoredTarget {
    pub target: PlanningTarget,
    pub visibility: VisibilityScore,
}

/// Build planning targets from the default solar-system bodies (all of
/// [`DEFAULT_PLANNING_BODIES`] except the Sun), sampling each body's apparent
/// equatorial position at the midpoint of the coming evening window. Hosts
/// without a star catalog can feed these straight into [`rank_targets`].
pub fn planning_targets_from_bodies(observer: Observer) -> Vec<PlanningTarget> {
    let (start, end) = evening_window_jd_utc(observer);
    let mid = observer_at(observer, 0.5 * (start + end));
    DEFAULT_PLANNING_BODIES
        .into_iter()
        .filter(|body| !matches!(body, PlanningBody::Sun))
        .map(|body| {
            let (ra, dec) = body_equatorial(mid, body);
            PlanningTarget {
                name: body.name().to_string(),
                right_ascension_rad: ra,
                declination_rad: dec,
            }
        })
        .collect()
}

/// Score every supplied target and return them sorted by descending
/// visibility score ("tonight's recommended objects").
pub fn rank_targets(
    observer: Observer,
    targets: &[PlanningTarget],
    dark_sky_v_mag: f64,
) -> Vec<ScoredTarget> {
    let mut scored: Vec<ScoredTarget> = targets
        .iter()
        .map(|target| ScoredTarget {
            target: target.clone(),
            visibility: visibility_score(
                observer,
                target.right_ascension_rad,
                target.declination_rad,
                dark_sky_v_mag,
            ),
        })
        .collect();
    scored.sort_by(|a, b| {
        b.visibility
            .score
            .partial_cmp(&a.visibility.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored
}

/// Convert a JD(UTC) to an iCalendar UTC timestamp (`YYYYMMDDTHHMMSSZ`).
fn ical_utc_timestamp(jd_utc: f64) -> String {
    // Fliegel & Van Flandern (1968) JD → Gregorian calendar date.
    let jd = jd_utc + 0.5;
    let z = jd.floor();
    let frac = jd - z;
    let a = if z < 2_299_161.0 {
        z
    } else {
        let alpha = ((z - 1_867_216.25) / 36_524.25).floor();
        z + 1.0 + alpha - (alpha / 4.0).floor()
    };
    let b = a + 1524.0;
    let c = ((b - 122.1) / 365.25).floor();
    let d = (365.25 * c).floor();
    let e = ((b - d) / 30.6001).floor();
    let day = b - d - (30.6001 * e).floor();
    let month = if e < 14.0 { e - 1.0 } else { e - 13.0 };
    let year = if month > 2.0 { c - 4716.0 } else { c - 4715.0 };

    let total_seconds = (frac * 86_400.0).round() as i64;
    let (mut day_i, mut total_seconds) = (day as i64, total_seconds);
    if total_seconds >= 86_400 {
        total_seconds -= 86_400;
        day_i += 1;
    }
    let hour = total_seconds / 3600;
    let minute = (total_seconds % 3600) / 60;
    let second = total_seconds % 60;
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        year as i64, month as i64, day_i, hour, minute, second
    )
}

/// Escape a string for an iCalendar TEXT value (RFC 5545 §3.3.11).
fn ical_escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

/// Export the observable dark windows of scored targets as an RFC 5545
/// iCalendar document. Targets without an observable dark window are
/// skipped. Each event spans the target's `observable_window_jd_utc` and
/// records the transit altitude, visibility score, and Moon ΔV in the
/// description so the calendar entry is self-documenting.
pub fn icalendar_for_targets(scored: &[ScoredTarget]) -> String {
    let mut out = String::new();
    out.push_str("BEGIN:VCALENDAR\r\n");
    out.push_str("VERSION:2.0\r\n");
    out.push_str("PRODID:-//stars//L-09 observation planning//EN\r\n");
    out.push_str("CALSCALE:GREGORIAN\r\n");
    for entry in scored {
        let Some((start, end)) = entry.visibility.observable_window_jd_utc else {
            continue;
        };
        let name = &entry.target.name;
        let dtstart = ical_utc_timestamp(start);
        let dtend = ical_utc_timestamp(end);
        let alt_deg = entry.visibility.max_altitude_rad.to_degrees();
        let summary = format!(
            "Observe {} (alt {:.0}°, score {:.2})",
            name, alt_deg, entry.visibility.score
        );
        let description = format!(
            "Max altitude {:.1}°; observable dark window {:.1} h; \
             Moon ΔV {:+.2} mag/arcsec² (illum {:.0}%).",
            alt_deg,
            entry.visibility.observable_dark_hours,
            entry.visibility.moon.delta_v_mag,
            entry.visibility.moon.moon_illuminated_fraction * 100.0,
        );
        out.push_str("BEGIN:VEVENT\r\n");
        out.push_str(&format!(
            "UID:{}-{}@stars\r\n",
            ical_escape(&name.replace(' ', "-")),
            dtstart
        ));
        out.push_str(&format!("DTSTAMP:{}\r\n", dtstart));
        out.push_str(&format!("DTSTART:{}\r\n", dtstart));
        out.push_str(&format!("DTEND:{}\r\n", dtend));
        out.push_str(&format!("SUMMARY:{}\r\n", ical_escape(&summary)));
        out.push_str(&format!("DESCRIPTION:{}\r\n", ical_escape(&description)));
        out.push_str("END:VEVENT\r\n");
    }
    out.push_str("END:VCALENDAR\r\n");
    out
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
    fn find_planet_transit_rejects_outer_planets() {
        // V-51e gate: only Mercury and Venus can transit the Sun from
        // Earth. The helper must reject Jupiter without even running the
        // scan.
        let start = jd_utc_from_iso_hours(2012, 6, 5, 21.0);
        let end = jd_utc_from_iso_hours(2012, 6, 6, 5.0);
        let observer = Observer::from_degrees(35.68, 139.69, 0.5 * (start + end));
        assert!(find_planet_transit(observer, Planet::Jupiter, start, end).is_none());
        assert!(find_planet_transit(observer, Planet::Mars, start, end).is_none());
    }

    #[test]
    fn find_planet_transit_returns_none_off_transit_day() {
        // 2025-07-01 Tokyo: neither Mercury nor Venus is at inferior
        // conjunction, so the apparent-disk pair stays separated all day.
        let start = jd_utc_from_iso_hours(2025, 7, 1, 0.0);
        let end = jd_utc_from_iso_hours(2025, 7, 1, 24.0);
        let observer = Observer::from_degrees(35.68, 139.69, 0.5 * (start + end));
        assert!(find_planet_transit(observer, Planet::Mercury, start, end).is_none());
        assert!(find_planet_transit(observer, Planet::Venus, start, end).is_none());
    }

    #[test]
    fn find_planet_transit_finds_2012_venus_transit() {
        // 2012-06-06 Venus transit. NASA canon: P1 ≈ 22:09 UT (2012-06-05),
        // P2 ≈ 22:27 UT, greatest transit 01:29 UT (06-06), P3 ≈ 04:31 UT,
        // P4 ≈ 04:49 UT. The whole event lasts ~6h 40m. Bracket a wide
        // window so the contact-time refinement can lock onto all four
        // contacts even if VSOP87 drifts a few minutes.
        let start = jd_utc_from_iso_hours(2012, 6, 5, 21.0);
        let end = jd_utc_from_iso_hours(2012, 6, 6, 6.0);
        let mid = 0.5 * (start + end);
        let observer = Observer::from_degrees(35.68, 139.69, mid);
        let event = find_planet_transit(observer, Planet::Venus, start, end)
            .expect("2012-06-06 Venus transit must be detected");
        assert_eq!(event.planet, Planet::Venus);
        assert!(
            event.is_interior(),
            "expected interior phase (P2/P3) for the 2012 Venus transit, got contacts {:?}",
            event.contacts
        );
        // Venus apparent diameter ≈58″, Sun ≈1890″ at the 2012-06-06
        // distances: peak obscuration ≈ (58/1890)² ≈ 9.4e-4. Well above
        // any partial-crossing noise floor, and far below the ~1 %
        // bar a partial solar eclipse would clear.
        assert!(
            (5.0e-4..2.0e-3).contains(&event.peak_obscuration),
            "peak obscuration {} outside the Venus-transit area-ratio band",
            event.peak_obscuration,
        );
        let p1 = event.contacts.p1.expect("P1 must exist");
        let p4 = event.contacts.p4.expect("P4 must exist");
        let duration_min = (p4 - p1) * 24.0 * 60.0;
        assert!(
            (5.0 * 60.0..8.0 * 60.0).contains(&duration_min),
            "transit duration {duration_min:.1} min outside the 5-8 hr plausibility band",
        );
        assert!(p1 <= event.peak_jd_utc && event.peak_jd_utc <= p4);
    }

    #[test]
    fn active_occluders_emit_planet_on_sun_at_venus_transit_peak() {
        // V-51e producer contract: at the 2012-06-06 Venus transit peak,
        // `active_occluders` must include exactly one Planet(Venus) ↔ Sun
        // pair. The Moon-on-Stars cull entry is always present; the
        // Moon-on-Sun pair is absent (no solar eclipse this date).
        let start = jd_utc_from_iso_hours(2012, 6, 5, 21.0);
        let end = jd_utc_from_iso_hours(2012, 6, 6, 6.0);
        let observer = Observer::from_degrees(35.68, 139.69, 0.5 * (start + end));
        let event = find_planet_transit(observer, Planet::Venus, start, end)
            .expect("Venus transit peak must be detected");
        let peak_observer = Observer::from_degrees(35.68, 139.69, event.peak_jd_utc);
        let list = active_occluders(peak_observer);
        let sun_occluders: Vec<_> = list
            .as_slice()
            .iter()
            .filter(|o| o.target == OccluderTarget::Sun)
            .collect();
        assert_eq!(
            sun_occluders.len(),
            1,
            "expected one Planet→Sun occluder at the Venus transit peak, got {sun_occluders:?}",
        );
        let occ = sun_occluders[0];
        // Front disk must be Venus, not the Moon.
        let venus = apparent_planet_topocentric(peak_observer, Planet::Venus);
        assert!(
            (occ.front_radius_rad - venus.angular_radius_rad).abs() < 1.0e-9,
            "Planet→Sun front radius {} != Venus apparent radius {}",
            occ.front_radius_rad,
            venus.angular_radius_rad,
        );
        // Pure-geometry classifier returns AnnularOrTransit when the
        // front disk is fully inside the back disk; the renderer reads
        // this code to skip the Koomen falloff / corona that only the
        // Moon-on-Sun pair triggers.
        assert_eq!(occ.kind, OccultationKind::AnnularOrTransit);
    }

    #[test]
    fn find_mutual_planetary_occultation_rejects_same_planet() {
        // V-51f: identical front and back collapses to a degenerate
        // self-pair; the helper must refuse without scanning.
        let start = jd_utc_from_iso_hours(2025, 7, 1, 0.0);
        let end = jd_utc_from_iso_hours(2025, 7, 1, 24.0);
        let observer = Observer::from_degrees(35.68, 139.69, 0.5 * (start + end));
        assert!(find_mutual_planetary_occultation(
            observer,
            Planet::Venus,
            Planet::Venus,
            start,
            end,
        )
        .is_none());
    }

    #[test]
    fn find_mutual_planetary_occultation_returns_none_off_event() {
        // 2025-07-01 Tokyo: Venus and Jupiter are several tens of
        // degrees apart all day, so no pair occults any other pair.
        let start = jd_utc_from_iso_hours(2025, 7, 1, 0.0);
        let end = jd_utc_from_iso_hours(2025, 7, 1, 24.0);
        let observer = Observer::from_degrees(35.68, 139.69, 0.5 * (start + end));
        for (a, b) in [
            (Planet::Venus, Planet::Jupiter),
            (Planet::Mercury, Planet::Mars),
            (Planet::Mars, Planet::Saturn),
        ] {
            assert!(
                find_mutual_planetary_occultation(observer, a, b, start, end).is_none(),
                "unexpected mutual occultation between {a:?} and {b:?} on 2025-07-01"
            );
        }
    }

    #[test]
    fn active_occluders_emit_no_planet_on_planet_off_event() {
        // V-51f producer contract off-event: no occluder with both a
        // Planet target *and* a front disk matching one of the seven
        // apparent planet disks. The Moon-on-Stars cull entry plus any
        // Moon-on-Planet entries are unaffected by this assertion.
        let start = jd_utc_from_iso_hours(2025, 7, 1, 0.0);
        let end = jd_utc_from_iso_hours(2025, 7, 1, 24.0);
        let observer = Observer::from_degrees(35.68, 139.69, 0.5 * (start + end));
        let list = active_occluders(observer);
        let moon = apparent_moon_topocentric(observer);
        for occ in list.as_slice() {
            if !matches!(occ.target, OccluderTarget::Planet(_)) {
                continue;
            }
            // V-51d Moon-on-Planet entries carry the lunar apparent
            // radius; V-51f Planet-on-Planet entries carry a planet's
            // apparent radius (~arcseconds, two orders of magnitude
            // smaller). The two are easy to discriminate without
            // re-running the producer logic.
            assert!(
                (occ.front_radius_rad - moon.angular_radius_rad).abs() < 1.0e-9,
                "unexpected Planet-on-Planet occluder off-event: {occ:?}",
            );
        }
    }

    #[test]
    fn active_occluders_skip_planet_on_sun_at_superior_conjunction() {
        // V-51e foreground gate: at superior conjunction the planet sits
        // behind the Sun and the pure-geometry classifier would
        // otherwise spuriously emit a "transit". The producer must skip
        // when `planet.distance_au >= sun.distance_au`.
        //
        // 2024-06-14 ≈ Mercury superior conjunction. Mercury's apparent
        // direction is within ~1° of the Sun's, but it is on the far side
        // of its orbit (distance ≈1.34 AU vs Sun ≈1.015 AU).
        let start = jd_utc_from_iso_hours(2024, 6, 14, 0.0);
        let end = jd_utc_from_iso_hours(2024, 6, 14, 24.0);
        let observer = Observer::from_degrees(35.68, 139.69, 0.5 * (start + end));
        let list = active_occluders(observer);
        assert!(
            list.as_slice()
                .iter()
                .all(|o| o.target != OccluderTarget::Sun),
            "no Sun-targeted occluder must be emitted at Mercury superior conjunction, got {:?}",
            list.as_slice(),
        );
    }

    #[test]
    fn active_occluders_emit_no_galilean_shadow_off_event() {
        // V-52d producer contract off-event: on a quiet date with
        // Jupiter visible, the producer must emit zero Planet(Jupiter)
        // entries whose front-disk radius matches a Galilean moon's
        // shadow extent. The Moon-on-Stars cull entry plus any
        // V-51d/f planet-targeted entries are unaffected by this
        // assertion; only the Galilean-sized shadows are gated here.
        let start = jd_utc_from_iso_hours(2025, 7, 1, 0.0);
        let end = jd_utc_from_iso_hours(2025, 7, 1, 24.0);
        let observer = Observer::from_degrees(35.68, 139.69, 0.5 * (start + end));
        let list = active_occluders(observer);
        let jupiter = apparent_planet_topocentric(observer, Planet::Jupiter);
        for occ in list.as_slice() {
            if occ.target != OccluderTarget::Planet(3) {
                continue;
            }
            // A Galilean shadow's apparent radius is the moon's
            // physical radius divided by the Earth-Jupiter distance
            // (a few hundredths of a Jovian apparent radius). V-51d
            // Moon-on-Jupiter entries are roughly the Moon's apparent
            // radius (≈ 1000″) — three orders of magnitude bigger.
            // The two are easy to discriminate without re-running the
            // producer logic.
            let jupiter_radius = jupiter.angular_radius_rad;
            assert!(
                occ.front_radius_rad > jupiter_radius / 8.0,
                "unexpected Galilean shadow occluder off-event: {occ:?}",
            );
        }
    }

    #[test]
    fn active_occluders_emit_io_shadow_at_2008_12_20_transit() {
        // V-52d positive contract: at 2008-Dec-20 14:00 UT the Io
        // shadow sits well inside the Jovian disk (its 13:14 UT
        // ingress is pinned by
        // `jupiter_shadows::tests::io_shadow_ingress_within_five_minutes_of_horizons_2008_12_20`).
        // The producer must emit one Planet(Jupiter) entry whose
        // front-disk angular radius matches Io's `radius / Δ` extent.
        let observer =
            Observer::from_degrees(35.68, 139.69, jd_utc_from_iso_hours(2008, 12, 20, 14.0));
        let list = active_occluders(observer);
        let jupiter_planet_target = OccluderTarget::Planet(3);
        let jupiter = apparent_planet_topocentric(observer, Planet::Jupiter);
        let io_radius_km = crate::moons::GalileanMoon::Io.radius_km();
        let earth_jupiter_km = jupiter.distance_au * crate::ephemeris::ASTRONOMICAL_UNIT_KM;
        let io_shadow_radius_expected = (io_radius_km / earth_jupiter_km).atan();
        let mut found = false;
        for occ in list.as_slice() {
            if occ.target != jupiter_planet_target {
                continue;
            }
            // Discriminate from any V-51d Moon-on-Jupiter / V-51f
            // Planet-on-Jupiter entries by matching the radius to
            // Io's silhouette extent on Jupiter (a few percent of the
            // Jovian apparent radius).
            if (occ.front_radius_rad - io_shadow_radius_expected).abs()
                < 0.1 * io_shadow_radius_expected
            {
                found = true;
                assert_eq!(occ.kind, OccultationKind::AnnularOrTransit);
                // Obscuration at the area-ratio is small (≪ 1) for a
                // moon shadow against the Jovian disk; pin the
                // ordering rather than the magnitude.
                assert!(occ.obscuration < 1.0);
                assert!(occ.obscuration > 0.0);
            }
        }
        assert!(
            found,
            "Io shadow disk must appear in active_occluders during the 2008-12-20 transit, got {:?}",
            list.as_slice(),
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

    // ----- L-09 Observation-planning polish -----

    #[test]
    fn v_mag_nanolambert_roundtrip() {
        for v in [18.0, 20.0, 21.6, 22.0] {
            let b = nanolamberts_from_v_mag(v);
            let back = v_mag_from_nanolamberts(b);
            assert!((v - back).abs() < 1e-9, "V={v} round-trip got {back}");
        }
        // Pinned anchor: V = 21.6 mag/arcsec² ≈ 78.1 nL (K&S 1991 Eq. 27).
        assert!((nanolamberts_from_v_mag(21.6) - 78.07).abs() < 0.2);
    }

    #[test]
    fn moon_impact_matches_krisciunas_schaefer_reference() {
        // ROADMAP L-09 pinned case: target at 20° altitude, Moon at 60°
        // altitude, 90% illuminated. Illuminated fraction 0.90 ⇒ phase angle
        // α = acos(2·0.90 − 1) = 36.870°. Separation taken as 60°.
        //
        // Worked through K&S 1991 Eqs. 15/20/16-18/3/27 by hand:
        //   I*   = 10^(-0.4(3.84 + 0.026·36.870 + 4e-9·36.870^4)) ≈ 0.011956
        //   f    = 10^5.36(1.06 + cos²60°) + 10^(6.15 - 60/40)    ≈ 344772
        //   X_m  = (1 - 0.96 sin²30°)^-0.5                        ≈ 1.14699
        //   X_t  = (1 - 0.96 sin²70°)^-0.5                        ≈ 2.56244
        //   B_moon ≈ 1147 nL ; B_dark(21.6) ≈ 78.1 nL
        //   V_moonlit ≈ 18.61 ⇒ ΔV ≈ 2.99 mag/arcsec²
        let target_zenith = 70f64.to_radians();
        let moon_zenith = 30f64.to_radians();
        let phase_angle_deg = (2.0 * 0.90 - 1.0_f64).acos().to_degrees();
        let b_moon =
            moon_sky_brightness_nanolamberts(phase_angle_deg, moon_zenith, target_zenith, 60.0);
        assert!(
            (b_moon - 1147.0).abs() < 25.0,
            "B_moon {b_moon} nL deviates from K&S reference 1147 nL"
        );
        let v_moonlit = v_mag_from_nanolamberts(nanolamberts_from_v_mag(21.6) + b_moon);
        let delta_v = 21.6 - v_moonlit;
        assert!(
            (delta_v - 2.99).abs() < 0.06,
            "ΔV {delta_v} deviates from K&S-derived 2.99 mag"
        );
    }

    #[test]
    fn moon_impact_grows_with_illumination_and_proximity() {
        let zt = 60f64.to_radians();
        let zm = 30f64.to_radians();
        // Brighter (fuller) Moon → larger sky brightness.
        let full = moon_sky_brightness_nanolamberts(0.0, zm, zt, 45.0);
        let crescent = moon_sky_brightness_nanolamberts(120.0, zm, zt, 45.0);
        assert!(
            full > crescent,
            "full {full} should exceed crescent {crescent}"
        );
        // Closer to the Moon → larger sky brightness.
        let near = moon_sky_brightness_nanolamberts(0.0, zm, zt, 15.0);
        let far = moon_sky_brightness_nanolamberts(0.0, zm, zt, 120.0);
        assert!(near > far, "near {near} should exceed far {far}");
        // Moon below the horizon contributes nothing.
        let below = moon_sky_brightness_nanolamberts(0.0, 100f64.to_radians(), zt, 45.0);
        assert_eq!(below, 0.0);
    }

    #[test]
    fn visibility_score_in_unit_range_and_ranks_targets() {
        let observer = Observer::from_degrees(35.68, 139.69, 2_460_482.5);
        let targets = planning_targets_from_bodies(observer);
        assert_eq!(targets.len(), DEFAULT_PLANNING_BODIES.len() - 1);
        let ranked = rank_targets(observer, &targets, DARK_SKY_ZENITH_V_MAG);
        assert_eq!(ranked.len(), targets.len());
        for entry in &ranked {
            assert!(
                (0.0..=1.0).contains(&entry.visibility.score),
                "{} score {} out of range",
                entry.target.name,
                entry.visibility.score
            );
        }
        // rank_targets must return descending scores.
        assert!(ranked
            .windows(2)
            .all(|pair| pair[0].visibility.score >= pair[1].visibility.score));
    }

    #[test]
    fn icalendar_export_is_well_formed() {
        let observer = Observer::from_degrees(35.68, 139.69, 2_460_482.5);
        let targets = planning_targets_from_bodies(observer);
        let ranked = rank_targets(observer, &targets, DARK_SKY_ZENITH_V_MAG);
        let ics = icalendar_for_targets(&ranked);
        assert!(ics.starts_with("BEGIN:VCALENDAR\r\n"));
        assert!(ics.trim_end().ends_with("END:VCALENDAR"));
        // Every VEVENT must be paired and carry UTC timestamps.
        let begins = ics.matches("BEGIN:VEVENT").count();
        let ends = ics.matches("END:VEVENT").count();
        assert_eq!(begins, ends);
        if begins > 0 {
            assert!(ics.contains("DTSTART:"));
            assert!(ics.contains('Z'));
        }
    }

    #[test]
    fn ical_timestamp_matches_known_epoch() {
        // JD 2451545.0 (UTC) = 2000-01-01 12:00:00 UTC.
        assert_eq!(ical_utc_timestamp(2_451_545.0), "20000101T120000Z");
    }
}
