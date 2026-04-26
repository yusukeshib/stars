mod catalog;
pub mod color;
pub mod coords;

pub use catalog::{load_from_csv, RawStar, Star};

#[cfg(feature = "filesystem")]
pub use catalog::load_from_file;

#[cfg(feature = "embedded")]
pub use catalog::load_embedded;
