mod catalog;
mod color;
mod coords;

pub use catalog::{load_from_csv, Star};
pub use color::bv_to_rgb;
pub use coords::radec_hours_deg_to_cartesian;

#[cfg(feature = "filesystem")]
pub use catalog::load_from_file;

#[cfg(feature = "embedded")]
pub use catalog::load_embedded;
