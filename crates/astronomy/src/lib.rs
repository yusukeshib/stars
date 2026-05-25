//! Lightweight astronomy helpers used by the renderer.
//!
//! Conventions:
//! - Time is expressed as Julian Date (UT1 ≈ UTC, sub-second drift ignored).
//! - Coordinates are J2000-ish equatorial: x = cos δ cos α, y = cos δ sin α, z = sin δ.
//! - Local frame is ENU (East, North, Up) at the observer.
//! - Angles internally are radians; helpers also accept degrees / hours where noted.

mod ephemeris;
mod horizontal;
pub mod illuminants;
mod observer;
pub mod photometry;
pub mod skyglow;
mod time;

pub use ephemeris::{
    apparent_moon, apparent_moon_topocentric, apparent_sun, apparent_sun_topocentric, MoonApparent,
    SunApparent,
};
pub use horizontal::{equatorial_to_horizontal, equatorial_to_horizontal_matrix, AltAz};
pub use observer::Observer;
pub use time::{gmst_radians, julian_date_from_unix_seconds, lmst_radians};
