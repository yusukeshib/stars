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
pub mod jupiter_shadows;
pub mod moons;
mod observer;
pub mod occultation;
pub mod photometry;
mod planning;
pub mod satellites;
pub mod scintillation;
pub mod skyglow;
mod time;

pub use corrections::{
    annual_aberration, earth_velocity_over_c_j2000, edlen_refractivity_standard_air,
    equation_of_equinoxes, mean_obliquity_iau2006, nutation_iau2000b_approx,
    precession_matrix_iau2006, precession_nutation_matrix, refracted_altitude_saemundsson,
    refraction_per_wavelength, years_since_j2000, Nutation, EDLEN_REFERENCE_REFRACTIVITY,
    REFERENCE_WAVELENGTH_NM, RGB_REFERENCE_WAVELENGTHS_NM,
};
pub use ephemeris::{
    apparent_moon, apparent_moon_topocentric, apparent_planet, apparent_planet_topocentric,
    apparent_planets_topocentric, apparent_saturn_ring, apparent_saturn_ring_topocentric,
    apparent_sun, apparent_sun_topocentric, MoonApparent, Planet, PlanetApparent,
    SaturnRingApparent, SunApparent, SunMoonApparent,
};
pub use horizontal::{equatorial_to_horizontal, equatorial_to_horizontal_matrix, AltAz};
pub use jupiter_shadows::{
    galilean_shadow_disks, galilean_shadow_disks_at, galilean_shadow_states, GalileanShadowDisk,
    GalileanShadowState, JUPITER_OCCLUDER_TARGET, JUPITER_PLANET_INDEX, SHADOW_TRANSIT_KIND,
};
pub use moons::{
    apparent_galilean_moons, apparent_galilean_moons_topocentric, apparent_titan,
    apparent_titan_topocentric, GalileanMoon, GalileanMoonApparent, TitanApparent,
};
pub use observer::Observer;
pub use occultation::{
    classify_disks, contact_times, obscuration_fraction, ActiveOccluders, ApparentDisk,
    ContactTimes, Occluder, OccluderTarget, OccultationKind, MAX_OCCLUDERS,
};
pub use planning::{
    active_occluders, body_altitude_rad, body_equatorial, evening_plan, evening_window_jd_utc,
    find_lunar_occultation, find_mutual_planetary_occultation, find_planet_transit,
    find_solar_eclipse, jd_utc_to_unix_ms, rise_transit_set, solar_eclipse_state, twilight_band,
    twilight_indicators, EveningPlan, LunarOccultationEvent, LunarOccultedBody,
    MutualPlanetaryOccultationEvent, PlanetTransitEvent, PlanningBody, RiseTransitSet,
    SolarEclipseEvent, SolarEclipseKind, SolarEclipseState, TwilightBand, TwilightIndicator,
    DEFAULT_PLANNING_BODIES,
};
pub use satellites::{
    apparent_satellites, parse_tle_set, Satellite, SatelliteApparent, SatelliteError, Tle,
    DEFAULT_STD_MAGNITUDE,
};
pub use time::{
    approximate_tdb_from_tt, gmst_radians, julian_date_from_unix_seconds, lmst_radians,
    tai_minus_utc_seconds_at_jd_utc, LeapSecond, TimeScales, J2000_JD, LEAP_SECONDS,
    SECONDS_PER_DAY, UNIX_EPOCH_JD,
};
