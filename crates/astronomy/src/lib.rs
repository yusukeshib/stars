//! Lightweight astronomy helpers used by the renderer.
//!
//! Conventions:
//! - Civil input time is UTC; Earth rotation uses UT1; ephemerides use TT/TDB.
//! - Coordinates are J2000-ish equatorial: x = cos δ cos α, y = cos δ sin α, z = sin δ.
//! - Local frame is ENU (East, North, Up) at the observer.
//! - Angles internally are radians; helpers also accept degrees / hours where noted.

pub mod atmosphere;
pub mod corrections;
mod ephemeris;
mod horizontal;
pub mod illuminants;
mod observer;
pub mod occultation;
pub mod photometry;
mod planning;
pub mod scintillation;
pub mod skyglow;
mod time;

pub use corrections::{
    annual_aberration, earth_velocity_over_c_j2000, equation_of_equinoxes, mean_obliquity_iau2006,
    nutation_iau2000b_approx, precession_matrix_iau2006, precession_nutation_matrix,
    refracted_altitude_saemundsson, years_since_j2000, Nutation,
};
pub use ephemeris::{
    apparent_moon, apparent_moon_topocentric, apparent_planet, apparent_planet_topocentric,
    apparent_planets_topocentric, apparent_sun, apparent_sun_topocentric, MoonApparent, Planet,
    PlanetApparent, SunApparent, SunMoonApparent,
};
pub use horizontal::{equatorial_to_horizontal, equatorial_to_horizontal_matrix, AltAz};
pub use observer::Observer;
pub use occultation::{
    classify_disks, contact_times, obscuration_fraction, ActiveOccluders, ApparentDisk,
    ContactTimes, Occluder, OccluderTarget, OccultationKind, MAX_OCCLUDERS,
};
pub use planning::{
    active_occluders, body_altitude_rad, body_equatorial, evening_plan, evening_window_jd_utc,
    find_lunar_occultation, find_solar_eclipse, jd_utc_to_unix_ms, rise_transit_set,
    solar_eclipse_state, twilight_band, twilight_indicators, EveningPlan, LunarOccultationEvent,
    LunarOccultedBody, PlanningBody, RiseTransitSet, SolarEclipseEvent, SolarEclipseKind,
    SolarEclipseState, TwilightBand, TwilightIndicator, DEFAULT_PLANNING_BODIES,
};
pub use time::{
    approximate_tdb_from_tt, gmst_radians, julian_date_from_unix_seconds, lmst_radians,
    tai_minus_utc_seconds_at_jd_utc, LeapSecond, TimeScales, J2000_JD, LEAP_SECONDS,
    SECONDS_PER_DAY, UNIX_EPOCH_JD,
};
