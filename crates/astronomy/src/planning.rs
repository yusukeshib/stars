//! Observation-planning helpers: twilight states and rise/transit/set tables.

use crate::{
    apparent_moon_topocentric, apparent_planet_topocentric, apparent_sun_topocentric,
    equatorial_to_horizontal, lmst_radians, Observer, Planet, TimeScales, SECONDS_PER_DAY,
};

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
