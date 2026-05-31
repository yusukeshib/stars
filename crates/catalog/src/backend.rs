//! Catalog backend abstractions for Phase 3 scaling work.
//!
//! The current renderer still consumes a flat `Vec<Star>`, but large catalog
//! ingest needs stable seams for source identity, ID preservation, filtering,
//! paging, and future level-of-detail selection. This module defines those
//! seams without adding Gaia/Tycho/Hipparcos data yet.

use std::error::Error;
use std::fmt;

#[cfg(feature = "embedded")]
use crate::catalog::load_embedded;
#[cfg(any(feature = "filesystem", test))]
use crate::catalog::load_from_csv;
use crate::catalog::DEFAULT_MAX_MAGNITUDE;
use crate::Star;

/// Numeric object identifiers that can be preserved without tying the renderer
/// to a specific large catalog schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatalogObjectId {
    Hyg(u32),
    Hipparcos(u32),
    HenryDraper(u32),
    Tycho2(u64),
    GaiaDr3(u64),
}

impl CatalogObjectId {
    pub const fn namespace(self) -> &'static str {
        match self {
            Self::Hyg(_) => "HYG",
            Self::Hipparcos(_) => "HIP",
            Self::HenryDraper(_) => "HD",
            Self::Tycho2(_) => "TYC2",
            Self::GaiaDr3(_) => "Gaia DR3",
        }
    }

    /// Compact family discriminant used by the renderer-instance pick handle
    /// (see [`CatalogIdentifiers::pick_handle`]). `0` is reserved for "no id".
    pub const fn kind_tag(self) -> u32 {
        match self {
            Self::Hyg(_) => 1,
            Self::Hipparcos(_) => 2,
            Self::HenryDraper(_) => 3,
            Self::Tycho2(_) => 4,
            Self::GaiaDr3(_) => 5,
        }
    }

    /// Raw numeric value of the identifier, widened to `u64`.
    pub const fn numeric(self) -> u64 {
        match self {
            Self::Hyg(v) | Self::Hipparcos(v) | Self::HenryDraper(v) => v as u64,
            Self::Tycho2(v) | Self::GaiaDr3(v) => v,
        }
    }

    /// Reconstruct an id from a [`kind_tag`](Self::kind_tag) + numeric value,
    /// the inverse of the pick-handle packing. Returns `None` for tag `0`
    /// ("no id") or an unknown tag.
    pub const fn from_parts(kind_tag: u32, value: u64) -> Option<Self> {
        match kind_tag {
            1 => Some(Self::Hyg(value as u32)),
            2 => Some(Self::Hipparcos(value as u32)),
            3 => Some(Self::HenryDraper(value as u32)),
            4 => Some(Self::Tycho2(value)),
            5 => Some(Self::GaiaDr3(value)),
            _ => None,
        }
    }

    /// Canonical human / SIMBAD-resolvable label, e.g. `"HIP 32349"`,
    /// `"HD 48915"`, `"TYC 5949-2777-1"`, `"Gaia DR3 2947050466531873024"`,
    /// `"HYG 32263"`. This is the single source of truth for the "primary ID
    /// family" the hosts display on hover / click-to-copy (`L-18`) and that the
    /// `L-19` deep links resolve against.
    pub fn label(self) -> String {
        match self {
            Self::Hyg(v) => format!("HYG {v}"),
            Self::Hipparcos(v) => format!("HIP {v}"),
            Self::HenryDraper(v) => format!("HD {v}"),
            Self::Tycho2(v) => {
                let (t1, t2, t3) = crate::ingest::unpack_tyc(v);
                format!("TYC {t1}-{t2}-{t3}")
            }
            Self::GaiaDr3(v) => format!("Gaia DR3 {v}"),
        }
    }
}

/// Known cross-identifiers for one catalog row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CatalogIdentifiers {
    pub primary: Option<CatalogObjectId>,
    pub hyg: Option<u32>,
    pub hip: Option<u32>,
    pub hd: Option<u32>,
    pub tycho2: Option<u64>,
    pub gaia_dr3: Option<u64>,
}

impl CatalogIdentifiers {
    pub(crate) const fn from_hyg_row(hyg: Option<u32>, hip: Option<u32>, hd: Option<u32>) -> Self {
        Self {
            primary: match hyg {
                Some(id) => Some(CatalogObjectId::Hyg(id)),
                None => None,
            },
            hyg,
            hip,
            hd,
            tycho2: None,
            gaia_dr3: None,
        }
    }

    /// Identifiers for a Hipparcos main-catalogue row. The HIP number is the
    /// backend's primary identifier; an HD cross-ID is preserved when present.
    pub(crate) const fn from_hipparcos_row(hip: u32, hd: Option<u32>) -> Self {
        Self {
            primary: Some(CatalogObjectId::Hipparcos(hip)),
            hyg: None,
            hip: Some(hip),
            hd,
            tycho2: None,
            gaia_dr3: None,
        }
    }

    /// Identifiers for a Tycho-2 row. The packed TYC number is primary; the
    /// optional Hipparcos cross-ID from the Tycho-2 `HIP` column is preserved.
    pub(crate) const fn from_tycho2_row(tycho2: u64, hip: Option<u32>) -> Self {
        Self {
            primary: Some(CatalogObjectId::Tycho2(tycho2)),
            hyg: None,
            hip,
            hd: None,
            tycho2: Some(tycho2),
            gaia_dr3: None,
        }
    }

    /// Identifiers for a Gaia DR3 row. The `source_id` is primary; HIP / HD
    /// cross-IDs are preserved when a cross-match column supplies them.
    pub(crate) const fn from_gaia_row(gaia_dr3: u64, hip: Option<u32>, hd: Option<u32>) -> Self {
        Self {
            primary: Some(CatalogObjectId::GaiaDr3(gaia_dr3)),
            hyg: None,
            hip,
            hd,
            tycho2: None,
            gaia_dr3: Some(gaia_dr3),
        }
    }

    /// The canonical primary identifier for this row, resolving the backend's
    /// explicit `primary` first and otherwise synthesising one from the
    /// preserved cross-IDs in the SIMBAD-friendly priority order
    /// HIP → HD → TYC → Gaia → HYG. Returns `None` only for a row that carries
    /// no identifier at all (e.g. a resolved double-star secondary).
    pub fn resolved_primary(self) -> Option<CatalogObjectId> {
        if let Some(primary) = self.primary {
            return Some(primary);
        }
        if let Some(hip) = self.hip {
            return Some(CatalogObjectId::Hipparcos(hip));
        }
        if let Some(hd) = self.hd {
            return Some(CatalogObjectId::HenryDraper(hd));
        }
        if let Some(tyc) = self.tycho2 {
            return Some(CatalogObjectId::Tycho2(tyc));
        }
        if let Some(gaia) = self.gaia_dr3 {
            return Some(CatalogObjectId::GaiaDr3(gaia));
        }
        self.hyg.map(CatalogObjectId::Hyg)
    }

    /// Canonical primary-ID label (e.g. `"HIP 32349"`), or `None` for an
    /// identifier-less row. This is the string the hosts surface on hover /
    /// click-to-copy (`L-18`).
    pub fn primary_label(self) -> Option<String> {
        self.resolved_primary().map(CatalogObjectId::label)
    }

    /// Pack the resolved primary identifier into the compact
    /// `(kind_tag, value)` handle the renderer carries on every
    /// [`crate::Star`]-derived instance so an on-screen pick can be mapped back
    /// to its catalogue identity without re-reading the catalog. `kind_tag == 0`
    /// means "no identifier". The inverse is [`CatalogObjectId::from_parts`].
    pub fn pick_handle(self) -> (u32, u64) {
        match self.resolved_primary() {
            Some(id) => (id.kind_tag(), id.numeric()),
            None => (0, 0),
        }
    }
}

/// Stable backend/source identity carried by catalog snapshots and future
/// manifests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogSource {
    pub backend: CatalogBackendKind,
    pub name: &'static str,
    pub version: Option<&'static str>,
}

impl CatalogSource {
    pub const HYG_CSV: Self = Self {
        backend: CatalogBackendKind::HygCsv,
        name: "HYG",
        version: Some("4.2"),
    };

    pub const HYG_EMBEDDED: Self = Self {
        backend: CatalogBackendKind::HygEmbedded,
        name: "HYG",
        version: Some("4.2"),
    };

    /// Hipparcos main catalogue (ESA 1997 / VizieR I/239), ≈1 mas astrometry.
    pub const HIPPARCOS: Self = Self {
        backend: CatalogBackendKind::Hipparcos,
        name: "Hipparcos",
        version: Some("I/239"),
    };

    /// Tycho-2 catalogue (Høg 2000 / VizieR I/259), ≈60 mas astrometry.
    pub const TYCHO2: Self = Self {
        backend: CatalogBackendKind::Tycho2,
        name: "Tycho-2",
        version: Some("I/259"),
    };

    /// Gaia DR3 source catalogue (Gaia Collaboration 2022 / VizieR I/355),
    /// micro-arcsecond astrometry for bright stars.
    pub const GAIA_DR3: Self = Self {
        backend: CatalogBackendKind::GaiaDr3,
        name: "Gaia DR3",
        version: Some("I/355"),
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogBackendKind {
    HygCsv,
    HygEmbedded,
    Hipparcos,
    Tycho2,
    GaiaDr3,
}

impl CatalogBackendKind {
    pub const fn as_kebab_str(self) -> &'static str {
        match self {
            Self::HygCsv => "hyg-csv",
            Self::HygEmbedded => "hyg-embedded",
            Self::Hipparcos => "hipparcos",
            Self::Tycho2 => "tycho2",
            Self::GaiaDr3 => "gaia-dr3",
        }
    }
}

/// Query shape used by current HYG loading and reserved for future paged / LOD
/// backends. `max_magnitude` is a source-filtering limit, not the renderer's
/// observer limiting magnitude.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatalogQuery {
    pub max_magnitude: f32,
    pub max_rows: Option<usize>,
}

impl Default for CatalogQuery {
    fn default() -> Self {
        Self {
            max_magnitude: DEFAULT_MAX_MAGNITUDE,
            max_rows: None,
        }
    }
}

/// One page of catalog stars. The current HYG adapter returns a single complete
/// page; large backends can later set `next_page` and `truncated`.
#[derive(Debug, Clone)]
pub struct CatalogPage {
    pub source: CatalogSource,
    pub query: CatalogQuery,
    pub stars: Vec<Star>,
    pub truncated: bool,
    pub next_page: Option<u64>,
}

pub trait CatalogBackend {
    fn source(&self) -> CatalogSource;
    fn load(&self, query: CatalogQuery) -> Result<CatalogPage, CatalogError>;
}

#[derive(Debug)]
pub enum CatalogError {
    Io(std::io::Error),
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "catalog I/O failed: {error}"),
        }
    }
}

impl Error for CatalogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
        }
    }
}

impl From<std::io::Error> for CatalogError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(feature = "filesystem")]
#[derive(Debug, Clone)]
pub struct HygCsvBackend {
    path: std::path::PathBuf,
}

#[cfg(feature = "filesystem")]
impl HygCsvBackend {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[cfg(feature = "filesystem")]
impl CatalogBackend for HygCsvBackend {
    fn source(&self) -> CatalogSource {
        CatalogSource::HYG_CSV
    }

    fn load(&self, query: CatalogQuery) -> Result<CatalogPage, CatalogError> {
        let data = std::fs::read_to_string(&self.path)?;
        let mut stars = load_from_csv(&data);
        stars.retain(|star| star.magnitude <= query.max_magnitude);
        let truncated = if let Some(max_rows) = query.max_rows {
            let truncated = stars.len() > max_rows;
            stars.truncate(max_rows);
            truncated
        } else {
            false
        };
        Ok(CatalogPage {
            source: self.source(),
            query,
            stars,
            truncated,
            next_page: None,
        })
    }
}

#[cfg(feature = "embedded")]
#[derive(Debug, Clone, Copy, Default)]
pub struct HygEmbeddedBackend;

#[cfg(feature = "embedded")]
impl CatalogBackend for HygEmbeddedBackend {
    fn source(&self) -> CatalogSource {
        CatalogSource::HYG_EMBEDDED
    }

    fn load(&self, query: CatalogQuery) -> Result<CatalogPage, CatalogError> {
        let mut stars = load_embedded();
        stars.retain(|star| star.magnitude <= query.max_magnitude);
        let truncated = if let Some(max_rows) = query.max_rows {
            let truncated = stars.len() > max_rows;
            stars.truncate(max_rows);
            truncated
        } else {
            false
        };
        Ok(CatalogPage {
            source: self.source(),
            query,
            stars,
            truncated,
            next_page: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "id,hip,hd,hr,gl,bf,proper,ra,dec,dist,pmra,pmdec,rv,mag,absmag,spect,ci,x,y,z,vx,vy,vz,rarad,decrad,pmrarad,pmdecrad,bayer,flam,con,comp,comp_primary,base,lum,var,var_min,var_max";

    #[test]
    fn catalog_backend_kind_uses_session_snapshot_names() {
        assert_eq!(CatalogBackendKind::HygCsv.as_kebab_str(), "hyg-csv");
        assert_eq!(CatalogSource::HYG_CSV.name, "HYG");
        assert_eq!(CatalogSource::HYG_CSV.version, Some("4.2"));
    }

    #[test]
    fn identifiers_preserve_hyg_cross_ids() {
        let ids = CatalogIdentifiers::from_hyg_row(Some(42), Some(123), Some(456));
        assert_eq!(ids.primary, Some(CatalogObjectId::Hyg(42)));
        assert_eq!(ids.hyg, Some(42));
        assert_eq!(ids.hip, Some(123));
        assert_eq!(ids.hd, Some(456));
    }

    #[test]
    fn l18_primary_label_formats_each_family() {
        assert_eq!(CatalogObjectId::Hipparcos(32349).label(), "HIP 32349");
        assert_eq!(CatalogObjectId::HenryDraper(48915).label(), "HD 48915");
        assert_eq!(CatalogObjectId::Hyg(32263).label(), "HYG 32263");
        assert_eq!(CatalogObjectId::GaiaDr3(42).label(), "Gaia DR3 42");
        let tyc = crate::ingest::pack_tyc(5949, 2777, 1);
        assert_eq!(CatalogObjectId::Tycho2(tyc).label(), "TYC 5949-2777-1");
    }

    #[test]
    fn l18_resolved_primary_priority_and_label() {
        // Explicit primary wins.
        let hyg = CatalogIdentifiers::from_hyg_row(Some(7), Some(32349), Some(48915));
        assert_eq!(hyg.primary_label().as_deref(), Some("HYG 7"));
        // No explicit primary -> HIP before HD.
        let embedded = CatalogIdentifiers::from_hyg_row(None, Some(32349), Some(48915));
        assert_eq!(embedded.primary_label().as_deref(), Some("HIP 32349"));
        assert_eq!(
            embedded.resolved_primary(),
            Some(CatalogObjectId::Hipparcos(32349))
        );
        // HD only.
        let hd_only = CatalogIdentifiers::from_hyg_row(None, None, Some(48915));
        assert_eq!(hd_only.primary_label().as_deref(), Some("HD 48915"));
        // Identifier-less row (e.g. resolved double secondary).
        assert_eq!(CatalogIdentifiers::default().primary_label(), None);
    }

    #[test]
    fn l18_pick_handle_round_trips_through_kind_and_value() {
        for id in [
            CatalogObjectId::Hipparcos(32349),
            CatalogObjectId::HenryDraper(48915),
            CatalogObjectId::Hyg(32263),
            CatalogObjectId::Tycho2(crate::ingest::pack_tyc(5949, 2777, 1)),
            CatalogObjectId::GaiaDr3(2_947_050_466_531_873_024),
        ] {
            let ids = CatalogIdentifiers {
                primary: Some(id),
                ..Default::default()
            };
            let (kind, value) = ids.pick_handle();
            assert_ne!(kind, 0);
            assert_eq!(CatalogObjectId::from_parts(kind, value), Some(id));
        }
        // No id -> tag 0 -> None.
        let (kind, value) = CatalogIdentifiers::default().pick_handle();
        assert_eq!(kind, 0);
        assert_eq!(CatalogObjectId::from_parts(kind, value), None);
    }

    #[test]
    fn query_shape_filters_and_truncates_hyg_rows() {
        let csv = format!(
            "{HEADER}\n\
             1,1,11,,,,Bright,0.0,0.0,10.0,0.0,0.0,0.0,1.0,0.0,G2V,0.0,0,0,0,0,0,0,0,0,0,0,,,Ori,1,1,,1.0,,,\n\
             2,2,22,,,,Medium,0.0,0.0,10.0,0.0,0.0,0.0,4.5,0.0,G2V,0.0,0,0,0,0,0,0,0,0,0,0,,,Ori,1,1,,1.0,,,\n\
             3,3,33,,,,Dim,0.0,0.0,10.0,0.0,0.0,0.0,7.0,0.0,G2V,0.0,0,0,0,0,0,0,0,0,0,0,,,Ori,1,1,,1.0,,,\n"
        );
        let path = std::env::temp_dir().join(format!(
            "stars-hyg-backend-{}-{}.csv",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("test clock should be after Unix epoch")
                .as_nanos()
        ));
        std::fs::write(&path, csv).expect("write temporary HYG fixture");
        let backend = HygCsvBackend::new(path.clone());
        let page = backend
            .load(CatalogQuery {
                max_magnitude: 5.0,
                max_rows: Some(1),
            })
            .expect("load temporary HYG fixture");
        std::fs::remove_file(path).expect("remove temporary HYG fixture");

        assert_eq!(page.source, CatalogSource::HYG_CSV);
        assert_eq!(page.query.max_magnitude, 5.0);
        assert!(page.truncated);
        assert_eq!(page.stars.len(), 1);
        assert_eq!(page.stars[0].identifiers.hyg, Some(1));
    }
}
