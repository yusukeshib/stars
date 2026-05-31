mod backend;
mod catalog;
mod clusters;
mod color;
mod coords;
mod deepsky;
mod doubles;
mod ingest;
mod links;
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
pub use ingest::{
    bright_star_cross_ids, cross_id_by_hd, cross_id_by_hip, gaia_bv_from_bp_rp, pack_tyc,
    parse_gaia_dr3_csv, parse_hipparcos_csv, parse_tycho2_csv, tycho_bv_from_vt_bt,
    tycho_v_from_vt_bt, unpack_tyc, BrightStarCrossId,
};
pub use links::{simbad_query_url, vizier_query_url, StarIdentifiers};
pub use search::{search, SearchId, SearchKind, SearchMatch, SEARCH_LIMIT_DEFAULT};

#[cfg(feature = "filesystem")]
pub use backend::HygCsvBackend;
#[cfg(feature = "filesystem")]
pub use catalog::load_from_file;
#[cfg(feature = "filesystem")]
pub use ingest::{GaiaDr3CsvBackend, HipparcosCsvBackend, Tycho2CsvBackend};

#[cfg(feature = "embedded")]
pub use backend::HygEmbeddedBackend;
#[cfg(feature = "embedded")]
pub use catalog::load_embedded;
