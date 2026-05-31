mod backend;
mod catalog;
mod clusters;
mod color;
mod coords;
mod deepsky;
mod doubles;
pub mod search;

pub use backend::{
    CatalogBackend, CatalogBackendKind, CatalogError, CatalogIdentifiers, CatalogObjectId,
    CatalogPage, CatalogQuery, CatalogSource,
};
pub use catalog::{load_from_csv, Star};
pub use clusters::{
    cluster_members, is_resolved_as_member_field, resolved_cluster_ids, ClusterMember,
};
pub use color::bv_to_rgb;
pub use coords::radec_hours_deg_to_cartesian;
pub use deepsky::{
    DeepSkyCatalog, DeepSkyId, DeepSkyKind, DeepSkyObject, MessierCatalog, NgcBrightCatalog,
    NO_PHOTOMETRY_SENTINEL_MAG,
};
pub use doubles::{double_stars, resolve_doubles, DoubleStar};
pub use search::{search, SearchId, SearchKind, SearchMatch, SEARCH_LIMIT_DEFAULT};

#[cfg(feature = "filesystem")]
pub use backend::HygCsvBackend;
#[cfg(feature = "filesystem")]
pub use catalog::load_from_file;

#[cfg(feature = "embedded")]
pub use backend::HygEmbeddedBackend;
#[cfg(feature = "embedded")]
pub use catalog::load_embedded;
