mod backend;
mod catalog;
mod color;
mod coords;

pub use backend::{
    CatalogBackend, CatalogBackendKind, CatalogError, CatalogIdentifiers, CatalogObjectId,
    CatalogPage, CatalogQuery, CatalogSource,
};
pub use catalog::{load_from_csv, Star};
pub use color::bv_to_rgb;
pub use coords::radec_hours_deg_to_cartesian;

#[cfg(feature = "filesystem")]
pub use backend::HygCsvBackend;
#[cfg(feature = "filesystem")]
pub use catalog::load_from_file;

#[cfg(feature = "embedded")]
pub use backend::HygEmbeddedBackend;
#[cfg(feature = "embedded")]
pub use catalog::load_embedded;
