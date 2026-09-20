//! L-17 level-of-detail (LOD) spatial-tile streaming for large catalogues.
//!
//! The full Gaia DR3 source (~1.8 billion rows) cannot be uploaded as one star
//! array, so faint stars are stored as **content-addressable spatial tiles**:
//! the sky is cut into an equirectangular grid, and within each grid cell the
//! rows are split into magnitude *tiers* (bright stars first). A render only
//! decodes the tiles that intersect the current field of view at the depth the
//! observer asked for, so frame-time work scales with the *visible* sky, not
//! the catalogue size.
//!
//! Design (mirrors `docs/catalog-backend-design.md` "LOD, paging, and spatial
//! index plan"):
//!
//! 1. **Magnitude tiering** — tier 0 is the bright all-sky base layer and is
//!    always streamed in full; fainter tiers are streamed only for tiles that
//!    intersect the query cone.
//! 2. **Spatial indexing** — fixed equirectangular cells keyed by
//!    [`TileId`]. Cell selection is conservative (cell-centre angular distance
//!    plus the cell's half-diagonal), so a cell is never wrongly culled.
//! 3. **Content-addressable store** — the [`LodIndex`] maps each populated
//!    [`TileKey`] to a content hash; a [`BlobStore`] reads the tile bytes by
//!    that hash. Identical tiles dedupe to one blob. The committed manifest
//!    pins blobs by SHA-256; the in-engine CAS key is a fast FNV-1a content
//!    hash (dedup/integrity, not cryptographic).
//! 4. **Stable ordering** — streamed stars are returned tier-by-tier then in
//!    the stored row order, so notebook / session outputs stay diffable.
//!
//! Tile payloads are Gaia DR3 CSV (parsed by [`crate::parse_gaia_dr3_csv`]), so
//! the streaming layer reuses the exact ingest + photometric path of the
//! single-file Gaia backend.

use std::collections::HashMap;

use glam::Vec3;

use crate::ingest::parse_gaia_dr3_csv_report_bounded;
use crate::Star;

/// Per-tile safety ceilings for index metadata, raw cache payloads, and CSV
/// decoding. Tiles are generated well below these limits; exceeding either
/// bound indicates corrupt or untrusted cache metadata.
const MAX_TILE_ROWS: u32 = 100_000;
const MAX_TILE_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

/// Number of latitude (declination) bands in the equirectangular tile grid.
/// 18 bands → 10° each. Kept small so the bright base tier stays a handful of
/// tiles while still bounding the per-cell row count for faint tiers.
pub const LAT_BANDS: u16 = 18;

/// Number of longitude (right-ascension) bands. 36 bands → 10° at the equator.
pub const LON_BANDS: u16 = 36;

/// Upper magnitude bound of each LOD tier. A star with magnitude `m` lands in
/// the first tier whose bound it does not exceed; anything fainter than the
/// last bound is dropped from the tiled set (it would never be naked-eye or
/// small-telescope visible). Tier 0 (`m ≤ 6.0`) is the bright all-sky base.
pub const TIER_BOUNDS: [f32; 4] = [6.0, 9.0, 12.0, 16.0];

/// The bright all-sky base tier that is always streamed in full.
pub const BASE_TIER: u8 = 0;

/// Identifier of one spatial-magnitude tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileId {
    /// Magnitude tier index (0 = bright base layer).
    pub tier: u8,
    /// Latitude band index in `0..LAT_BANDS` (band 0 is the south pole cap).
    pub lat: u16,
    /// Longitude band index in `0..LON_BANDS`.
    pub lon: u16,
}

impl TileId {
    /// Whether every component names a cell in the fixed LOD grid.
    pub fn is_valid(self) -> bool {
        usize::from(self.tier) < TIER_BOUNDS.len() && self.lat < LAT_BANDS && self.lon < LON_BANDS
    }

    /// The tier a given apparent magnitude belongs to, or `None` if it is
    /// fainter than the deepest tier and is therefore not tiled.
    pub fn tier_for_magnitude(magnitude: f32) -> Option<u8> {
        TIER_BOUNDS
            .iter()
            .position(|&bound| magnitude <= bound)
            .map(|i| i as u8)
    }

    /// The tile a unit-sphere equatorial direction maps into at a given tier.
    pub fn for_direction(tier: u8, dir: Vec3) -> Self {
        let dir = dir.normalize_or_zero();
        let dec = dir.z.clamp(-1.0, 1.0).asin(); // [-π/2, π/2]
        let ra = dir.y.atan2(dir.x).rem_euclid(std::f32::consts::TAU); // [0, 2π)
        let lat_f = (dec + std::f32::consts::FRAC_PI_2) / std::f32::consts::PI; // [0,1]
        let lon_f = ra / std::f32::consts::TAU; // [0,1)
        let lat = ((lat_f * LAT_BANDS as f32) as u16).min(LAT_BANDS - 1);
        let lon = ((lon_f * LON_BANDS as f32) as u16).min(LON_BANDS - 1);
        Self { tier, lat, lon }
    }

    /// Unit-sphere direction of this cell's centre.
    pub fn center_direction(self) -> Vec3 {
        let lat_step = std::f32::consts::PI / LAT_BANDS as f32;
        let lon_step = std::f32::consts::TAU / LON_BANDS as f32;
        let dec = -std::f32::consts::FRAC_PI_2 + (self.lat as f32 + 0.5) * lat_step;
        let ra = (self.lon as f32 + 0.5) * lon_step;
        Vec3::new(dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin())
    }

    /// Conservative angular half-diagonal (radians) of any cell: half the
    /// great-circle span of the widest (equatorial) cell's diagonal. Used to
    /// pad the cull test so a cell is never wrongly excluded.
    pub fn max_half_diagonal_rad() -> f32 {
        let lat_step = std::f32::consts::PI / LAT_BANDS as f32;
        let lon_step = std::f32::consts::TAU / LON_BANDS as f32;
        0.5 * (lat_step * lat_step + lon_step * lon_step).sqrt()
    }
}

/// A `TileKey` is just a `TileId`; the alias documents intent at the index /
/// store boundary (a key into the content-addressable store).
pub type TileKey = TileId;

/// One row of the LOD index: a populated tile and the content hash of its
/// payload blob, plus the row count for cull-budget bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LodTileEntry {
    pub tile: TileId,
    pub content_hash: String,
    pub rows: u32,
}

/// The set of populated tiles for a tiled catalogue, mapping each tile to the
/// content hash of its payload blob.
#[derive(Debug, Clone, Default)]
pub struct LodIndex {
    entries: Vec<LodTileEntry>,
    by_tile: HashMap<TileId, usize>,
}

impl LodIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace the entry for `tile`.
    pub fn insert(&mut self, tile: TileId, content_hash: impl Into<String>, rows: u32) {
        let entry = LodTileEntry {
            tile,
            content_hash: content_hash.into(),
            rows,
        };
        match self.by_tile.get(&tile) {
            Some(&i) => self.entries[i] = entry,
            None => {
                self.by_tile.insert(tile, self.entries.len());
                self.entries.push(entry);
            }
        }
    }

    pub fn entries(&self) -> &[LodTileEntry] {
        &self.entries
    }

    pub fn get(&self, tile: TileId) -> Option<&LodTileEntry> {
        self.by_tile.get(&tile).map(|&i| &self.entries[i])
    }

    /// Total tile count across all tiers (index size, not catalogue row count).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A content-addressable blob reader. Implementations map a content hash to the
/// raw tile payload bytes.
pub trait BlobStore {
    fn get(&self, content_hash: &str) -> Option<Vec<u8>>;
}

/// In-memory content-addressable store, used for tests and small generated
/// fixtures.
#[derive(Debug, Clone, Default)]
pub struct MemoryBlobStore {
    blobs: HashMap<String, Vec<u8>>,
}

impl MemoryBlobStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store `bytes` under their content hash and return that hash, so callers
    /// can register the same hash in a [`LodIndex`].
    pub fn put(&mut self, bytes: impl Into<Vec<u8>>) -> String {
        let bytes = bytes.into();
        let hash = content_hash(&bytes);
        self.blobs.insert(hash.clone(), bytes);
        hash
    }
}

impl BlobStore for MemoryBlobStore {
    fn get(&self, requested_hash: &str) -> Option<Vec<u8>> {
        if !is_valid_content_hash(requested_hash) {
            return None;
        }
        self.blobs
            .get(requested_hash)
            .filter(|bytes| content_hash(bytes) == requested_hash)
            .cloned()
    }
}

/// Filesystem content-addressable store: blobs live at
/// `<root>/<hash[0..2]>/<hash>`. Mirrors the on-disk layout a fetch script
/// would populate from a remote tile archive.
#[cfg(feature = "filesystem")]
#[derive(Debug, Clone)]
pub struct FsCasBlobStore {
    root: std::path::PathBuf,
}

#[cfg(feature = "filesystem")]
impl FsCasBlobStore {
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Path a given content hash resolves to under this store's root.
    pub fn blob_path(&self, content_hash: &str) -> std::path::PathBuf {
        let prefix = &content_hash[..content_hash.len().min(2)];
        self.root.join(prefix).join(content_hash)
    }

    /// Write `bytes` under their content hash, creating the shard directory.
    /// Returns the content hash so a caller can build an index.
    pub fn put(&self, bytes: &[u8]) -> std::io::Result<String> {
        let hash = content_hash(bytes);
        let path = self.blob_path(&hash);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, bytes)?;
        Ok(hash)
    }
}

#[cfg(feature = "filesystem")]
impl BlobStore for FsCasBlobStore {
    fn get(&self, requested_hash: &str) -> Option<Vec<u8>> {
        if !is_valid_content_hash(requested_hash) {
            return None;
        }
        let bytes = std::fs::read(self.blob_path(requested_hash)).ok()?;
        (content_hash(&bytes) == requested_hash).then_some(bytes)
    }
}

/// A field-of-view query against a tiled catalogue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LodQuery {
    /// View-centre direction (unit sphere, J2000 equatorial). `None` means an
    /// all-sky request (every tile up to `faint_limit_mag`).
    pub center: Option<Vec3>,
    /// Angular radius of the field of view, in radians. Ignored when `center`
    /// is `None`.
    pub radius_rad: f32,
    /// Deepest magnitude to stream. Tiers whose bright bound exceeds this are
    /// skipped entirely.
    pub faint_limit_mag: f32,
    /// Optional hard cap on the number of returned stars (after ordering).
    pub max_rows: Option<usize>,
}

impl LodQuery {
    /// An all-sky query down to `faint_limit_mag`.
    pub fn all_sky(faint_limit_mag: f32) -> Self {
        Self {
            center: None,
            radius_rad: 0.0,
            faint_limit_mag,
            max_rows: None,
        }
    }

    /// A field-of-view cone query.
    pub fn field_of_view(center: Vec3, radius_rad: f32, faint_limit_mag: f32) -> Self {
        Self {
            center: Some(center.normalize_or_zero()),
            radius_rad,
            faint_limit_mag,
            max_rows: None,
        }
    }
}

/// Why a selected LOD tile could not be loaded atomically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LodIssueKind {
    InvalidIndexEntry,
    MissingOrCorruptBlob,
    PayloadTooLarge,
    InvalidUtf8,
    InvalidRows,
}

/// A caller-visible problem associated with one indexed tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LodIssue {
    pub tile: TileId,
    pub kind: LodIssueKind,
    pub message: String,
}

/// Strict streaming failure. The partial stream is retained for diagnostics,
/// but reproducible callers must not render it as a complete catalogue.
#[derive(Debug, Clone)]
pub struct LodStreamError {
    pub partial: LodStream,
}

impl std::fmt::Display for LodStreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LOD stream incomplete: {} tile issue(s)",
            self.partial.issues.len()
        )
    }
}

impl std::error::Error for LodStreamError {}

/// The result of streaming a [`LodQuery`]: the decoded stars plus explicit
/// completeness and cull bookkeeping.
#[derive(Debug, Clone)]
pub struct LodStream {
    pub stars: Vec<Star>,
    /// False when a selected or structurally invalid index entry was skipped.
    pub complete: bool,
    pub issues: Vec<LodIssue>,
    /// Tiles in the index that the query considered (the cull denominator).
    pub tiles_examined: usize,
    /// Tiles actually loaded + decoded (the cull numerator — this is the work
    /// that should scale with the visible sky, not the catalogue size).
    pub tiles_loaded: usize,
}

/// A tiled catalogue: a spatial-magnitude [`LodIndex`] backed by a content-
/// addressable [`BlobStore`].
#[derive(Debug, Clone)]
pub struct LodCatalog<S: BlobStore> {
    index: LodIndex,
    store: S,
}

impl<S: BlobStore> LodCatalog<S> {
    pub fn new(index: LodIndex, store: S) -> Self {
        Self { index, store }
    }

    pub fn index(&self) -> &LodIndex {
        &self.index
    }

    /// Whether a tile should be loaded for `query`. The bright base tier is
    /// always loaded; fainter tiers are loaded only when their cell intersects
    /// the query cone (conservative: cell centre within `radius + half-diag`).
    fn tile_selected(&self, tile: TileId, query: &LodQuery) -> bool {
        let Some(center) = query.center else {
            return true; // all-sky
        };
        if tile.tier == BASE_TIER {
            return true; // bright base layer is always in view
        }
        let cos_sep = center
            .normalize_or_zero()
            .dot(tile.center_direction())
            .clamp(-1.0, 1.0);
        let sep = cos_sep.acos();
        sep <= query.radius_rad + TileId::max_half_diagonal_rad()
    }

    /// Stream the tiles needed for `query` in explicit best-effort mode.
    /// Callers must inspect [`LodStream::complete`] before treating the result
    /// as a reproducible catalogue.
    pub fn stream(&self, query: &LodQuery) -> LodStream {
        if query.max_rows == Some(0) {
            return LodStream {
                stars: Vec::new(),
                complete: true,
                issues: Vec::new(),
                tiles_examined: 0,
                tiles_loaded: 0,
            };
        }

        let mut selected: Vec<&LodTileEntry> = Vec::new();
        let mut issues = Vec::new();
        let mut tiles_examined = 0usize;
        for entry in self.index.entries() {
            // Index files are external inputs. Record impossible IDs/metadata
            // rather than silently presenting a partial catalogue as complete.
            if !entry.tile.is_valid() || !(1..=MAX_TILE_ROWS).contains(&entry.rows) {
                issues.push(LodIssue {
                    tile: entry.tile,
                    kind: LodIssueKind::InvalidIndexEntry,
                    message: format!("invalid tile metadata (rows={})", entry.rows),
                });
                continue;
            }
            // A tier can be skipped without geometry work only when even its
            // *brightest* possible star is fainter than the limit. A tier holds
            // stars in `(TIER_BOUNDS[tier-1], TIER_BOUNDS[tier]]`, so its
            // minimum magnitude is the previous tier's bound (−∞ for the base
            // tier). Testing the tier's *bright* bound here would wrongly drop
            // the in-limit stars of a partially-included tier.
            let tier = entry.tile.tier as usize;
            let tier_min_mag = if tier == 0 {
                f32::NEG_INFINITY
            } else {
                TIER_BOUNDS[tier - 1]
            };
            if tier_min_mag >= query.faint_limit_mag {
                continue;
            }
            tiles_examined += 1;
            if self.tile_selected(entry.tile, query) {
                selected.push(entry);
            }
        }
        // Stable ordering: tier ascending, then lat, then lon.
        selected.sort_by_key(|e| (e.tile.tier, e.tile.lat, e.tile.lon));

        let mut stars = Vec::new();
        let mut tiles_loaded = 0usize;
        for entry in selected {
            let remaining = query
                .max_rows
                .map_or(usize::MAX, |limit| limit.saturating_sub(stars.len()));
            if remaining == 0 {
                break;
            }

            let Some(bytes) = self.store.get(&entry.content_hash) else {
                issues.push(LodIssue {
                    tile: entry.tile,
                    kind: LodIssueKind::MissingOrCorruptBlob,
                    message: format!(
                        "missing blob or content-hash mismatch: {}",
                        entry.content_hash
                    ),
                });
                continue;
            };
            if bytes.len() > MAX_TILE_PAYLOAD_BYTES {
                issues.push(LodIssue {
                    tile: entry.tile,
                    kind: LodIssueKind::PayloadTooLarge,
                    message: format!("tile payload is {} bytes", bytes.len()),
                });
                continue;
            }
            let Ok(text) = std::str::from_utf8(&bytes) else {
                issues.push(LodIssue {
                    tile: entry.tile,
                    kind: LodIssueKind::InvalidUtf8,
                    message: "tile payload is not UTF-8".to_owned(),
                });
                continue;
            };
            let expected_rows = entry.rows as usize;
            let report = parse_gaia_dr3_csv_report_bounded(text, Some(expected_rows));
            if !report.diagnostics.is_empty() || report.stars.len() != expected_rows {
                issues.push(LodIssue {
                    tile: entry.tile,
                    kind: LodIssueKind::InvalidRows,
                    message: format!(
                        "decoded {} of {expected_rows} rows with {} diagnostic(s)",
                        report.stars.len(),
                        report.diagnostics.len()
                    ),
                });
                continue;
            }

            tiles_loaded += 1;
            stars.extend(
                report
                    .stars
                    .into_iter()
                    .filter(|star| star.magnitude <= query.faint_limit_mag)
                    .take(remaining),
            );
        }
        LodStream {
            stars,
            complete: issues.is_empty(),
            issues,
            tiles_examined,
            tiles_loaded,
        }
    }

    /// Strict variant for validation and reproducible renders.
    pub fn stream_strict(&self, query: &LodQuery) -> Result<LodStream, LodStreamError> {
        let stream = self.stream(query);
        if stream.complete {
            Ok(stream)
        } else {
            Err(LodStreamError { partial: stream })
        }
    }
}

/// Fast, non-cryptographic content hash (FNV-1a, 64-bit, hex) used as the CAS
/// key inside the engine. It is only an integrity check for a trusted cache,
/// not proof of cryptographic authenticity against malicious content. The
/// provenance-grade hash for committed fixtures remains SHA-256 via
/// `data/manifest.toml`.
fn is_valid_content_hash(value: &str) -> bool {
    value.len() == 16 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn content_hash(bytes: &[u8]) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::CatalogObjectId;

    /// Build a one-row Gaia CSV tile payload for a star at (ra_deg, dec_deg).
    fn gaia_tile(source_id: u64, ra_deg: f64, dec_deg: f64, gmag: f64) -> String {
        format!(
            "source_id,ra,dec,parallax,pmra,pmdec,phot_g_mean_mag,bp_rp\n\
             {source_id},{ra_deg},{dec_deg},5.0,0.0,0.0,{gmag},0.8\n"
        )
    }

    /// A small synthetic tiled catalogue: a bright base star in every other
    /// longitude band at the equator, plus faint stars scattered across all
    /// longitude bands at one declination band.
    fn synthetic_catalog() -> LodCatalog<MemoryBlobStore> {
        let mut store = MemoryBlobStore::new();
        let mut index = LodIndex::new();
        let mut id = 1u64;
        // Bright base tier: one star per even longitude band at the equator.
        for lon in (0..LON_BANDS).step_by(2) {
            let ra = (lon as f64 + 0.5) * (360.0 / LON_BANDS as f64);
            let payload = gaia_tile(id, ra, 0.0, 4.0);
            let hash = store.put(payload.into_bytes());
            let tile = TileId::for_direction(
                BASE_TIER,
                crate::coords::radec_hours_deg_to_cartesian(ra / 15.0, 0.0),
            );
            index.insert(tile, hash, 1);
            id += 1;
        }
        // Faint tier 2 (m ~ 11): one star in *every* longitude band at the
        // equator — the layer whose load cost must stay bounded by the FOV.
        for lon in 0..LON_BANDS {
            let ra = (lon as f64 + 0.5) * (360.0 / LON_BANDS as f64);
            let payload = gaia_tile(id, ra, 0.0, 11.0);
            let hash = store.put(payload.into_bytes());
            let tile = TileId::for_direction(
                2,
                crate::coords::radec_hours_deg_to_cartesian(ra / 15.0, 0.0),
            );
            index.insert(tile, hash, 1);
            id += 1;
        }
        LodCatalog::new(index, store)
    }

    #[test]
    fn magnitude_tiering_assigns_expected_tiers() {
        assert_eq!(TileId::tier_for_magnitude(-1.4), Some(0));
        assert_eq!(TileId::tier_for_magnitude(6.0), Some(0));
        assert_eq!(TileId::tier_for_magnitude(8.5), Some(1));
        assert_eq!(TileId::tier_for_magnitude(11.0), Some(2));
        assert_eq!(TileId::tier_for_magnitude(15.5), Some(3));
        assert_eq!(TileId::tier_for_magnitude(20.0), None);
    }

    #[test]
    fn direction_round_trips_to_its_own_cell() {
        for &(ra_h, dec) in &[(0.0, 0.0), (6.75, -16.7), (18.0, 60.0), (12.0, -89.0)] {
            let dir = crate::coords::radec_hours_deg_to_cartesian(ra_h, dec);
            let tile = TileId::for_direction(1, dir);
            // The cell centre must map back to the same cell.
            let back = TileId::for_direction(1, tile.center_direction());
            assert_eq!(tile, back, "ra={ra_h} dec={dec}");
        }
    }

    #[test]
    fn all_sky_query_streams_every_tile() {
        let cat = synthetic_catalog();
        let stream = cat.stream(&LodQuery::all_sky(12.0));
        // base (18) + faint (36) tiles.
        assert_eq!(stream.tiles_loaded, cat.index().len());
        assert_eq!(stream.stars.len(), (LON_BANDS / 2 + LON_BANDS) as usize);
    }

    #[test]
    fn faint_limit_skips_deep_tiers() {
        let cat = synthetic_catalog();
        // A bright-only query never touches the faint tier-2 tiles.
        let stream = cat.stream(&LodQuery::all_sky(6.0));
        assert_eq!(stream.tiles_loaded, (LON_BANDS / 2) as usize);
        assert!(stream.stars.iter().all(|s| s.magnitude <= 6.0));
    }

    #[test]
    fn faint_limit_inside_a_tier_keeps_its_in_limit_stars() {
        // Regression: a tier holds stars brighter than its bright bound, so a
        // faint limit that falls *inside* a tier must still load that tier and
        // surface its in-limit stars (not skip the whole tier on its bound).
        let mut store = MemoryBlobStore::new();
        let mut index = LodIndex::new();
        // One tier-1 tile (bound 9.0) holding a mag-7 star, at RA 0 / Dec 0.
        let payload = gaia_tile(42, 0.0, 0.0, 7.0);
        let hash = store.put(payload.into_bytes());
        let dir = crate::coords::radec_hours_deg_to_cartesian(0.0, 0.0);
        index.insert(TileId::for_direction(1, dir), hash, 1);
        let cat = LodCatalog::new(index, store);
        // Faint limit 8.0 lands inside tier 1 (6,9]; the mag-7 star must show.
        let stream = cat.stream(&LodQuery::all_sky(8.0));
        assert_eq!(stream.tiles_loaded, 1, "tier-1 tile must not be culled");
        assert_eq!(stream.stars.len(), 1);
        // G=7.0 with BP−RP=0.8 → Riello V ≈ 7.146, still within the 8.0 limit.
        let m = stream.stars[0].magnitude;
        assert!(m > 7.0 && m <= 8.0, "V={m}");
    }

    #[test]
    fn narrow_fov_culls_faint_tiles_but_keeps_bright_base() {
        let cat = synthetic_catalog();
        // 5° FOV around RA 0, Dec 0.
        let center = crate::coords::radec_hours_deg_to_cartesian(0.0, 0.0);
        let stream = cat.stream(&LodQuery::field_of_view(center, 5.0_f32.to_radians(), 12.0));
        // The full bright base tier stays (always-on, 18 tiles) but only a
        // couple of faint tiles near RA 0 load.
        let base_tiles = (LON_BANDS / 2) as usize;
        assert!(
            stream.tiles_loaded >= base_tiles && stream.tiles_loaded <= base_tiles + 3,
            "loaded={} base={base_tiles}",
            stream.tiles_loaded
        );
    }

    #[test]
    fn lod_cull_does_not_blow_up_with_catalog_size() {
        // "Bench coverage" as a deterministic scaling assertion (the CI image
        // has no criterion / wall-clock harness): grow the faint layer by
        // sub-dividing every longitude band into many declination bands and
        // confirm the per-frame *loaded* faint-tile count for a fixed narrow
        // FOV stays bounded while the total index grows ~linearly.
        let build = |dec_bands: u16| -> LodCatalog<MemoryBlobStore> {
            let mut store = MemoryBlobStore::new();
            let mut index = LodIndex::new();
            let mut id = 1u64;
            for lat in 0..dec_bands.min(LAT_BANDS) {
                let dec = -88.0 + (lat as f64) * (176.0 / dec_bands as f64);
                for lon in 0..LON_BANDS {
                    let ra = (lon as f64 + 0.5) * (360.0 / LON_BANDS as f64);
                    let hash = store.put(gaia_tile(id, ra, dec, 11.0).into_bytes());
                    let tile = TileId::for_direction(
                        2,
                        crate::coords::radec_hours_deg_to_cartesian(ra / 15.0, dec),
                    );
                    index.insert(tile, hash, 1);
                    id += 1;
                }
            }
            LodCatalog::new(index, store)
        };

        let center = crate::coords::radec_hours_deg_to_cartesian(0.0, 0.0);
        let q = LodQuery::field_of_view(center, 5.0_f32.to_radians(), 12.0);

        let small = build(2);
        let large = build(18);
        let small_s = small.stream(&q);
        let large_s = large.stream(&q);

        // The index (total tiles) grew ~9x...
        assert!(large.index().len() >= 4 * small.index().len());
        // ...but the loaded faint-tile count for the fixed FOV stayed bounded
        // and small (a handful of cells around the view centre), i.e. O(FOV)
        // not O(catalogue).
        assert!(
            large_s.tiles_loaded <= 6,
            "loaded faint tiles {} should stay bounded",
            large_s.tiles_loaded
        );
        assert!(small_s.tiles_loaded <= 6);
    }

    #[test]
    fn streaming_is_deterministic_and_preserves_gaia_ids() {
        let cat = synthetic_catalog();
        let q = LodQuery::all_sky(12.0);
        let a = cat.stream(&q);
        let b = cat.stream(&q);
        let ids_a: Vec<_> = a.stars.iter().map(|s| s.identifiers.primary).collect();
        let ids_b: Vec<_> = b.stars.iter().map(|s| s.identifiers.primary).collect();
        assert_eq!(ids_a, ids_b, "streaming must be deterministic");
        assert!(a
            .stars
            .iter()
            .all(|s| matches!(s.identifiers.primary, Some(CatalogObjectId::GaiaDr3(_)))));
    }

    #[test]
    fn invalid_tile_ids_are_ignored_without_panicking() {
        let mut store = MemoryBlobStore::new();
        let hash = store.put(gaia_tile(1, 0.0, 0.0, 4.0).into_bytes());
        let mut index = LodIndex::new();
        for tile in [
            TileId {
                tier: TIER_BOUNDS.len() as u8,
                lat: 0,
                lon: 0,
            },
            TileId {
                tier: 0,
                lat: LAT_BANDS,
                lon: 0,
            },
            TileId {
                tier: 0,
                lat: 0,
                lon: LON_BANDS,
            },
        ] {
            index.insert(tile, hash.clone(), 1);
        }
        let stream = LodCatalog::new(index, store).stream(&LodQuery::all_sky(12.0));
        assert_eq!(stream.tiles_examined, 0);
        assert_eq!(stream.tiles_loaded, 0);
        assert!(stream.stars.is_empty());
    }

    #[test]
    fn max_rows_stops_before_loading_later_tiles() {
        let mut store = MemoryBlobStore::new();
        let mut index = LodIndex::new();
        for (lon, source_id) in [(0, 1), (1, 2)] {
            let hash = store.put(gaia_tile(source_id, lon as f64 * 10.0, 0.0, 4.0));
            index.insert(
                TileId {
                    tier: BASE_TIER,
                    lat: 9,
                    lon,
                },
                hash,
                1,
            );
        }
        let mut query = LodQuery::all_sky(6.0);
        query.max_rows = Some(1);
        let stream = LodCatalog::new(index, store).stream(&query);
        assert_eq!(stream.stars.len(), 1);
        assert_eq!(stream.tiles_loaded, 1);
    }

    #[test]
    fn corrupt_or_mismatched_tile_payloads_are_rejected_atomically() {
        let cases = [
            (gaia_tile(1, 0.0, 0.0, 4.0), 2),
            (
                "source_id,ra,dec,phot_g_mean_mag\n\"unterminated,0,0,4\n".to_owned(),
                1,
            ),
            (
                "source_id,ra,dec,phot_g_mean_mag\nNaN,0,0,4\n".to_owned(),
                1,
            ),
        ];

        for (payload, rows) in cases {
            let mut store = MemoryBlobStore::new();
            let hash = store.put(payload);
            let mut index = LodIndex::new();
            index.insert(
                TileId {
                    tier: BASE_TIER,
                    lat: 9,
                    lon: 0,
                },
                hash,
                rows,
            );
            let stream = LodCatalog::new(index, store).stream(&LodQuery::all_sky(6.0));
            assert!(stream.stars.is_empty());
            assert_eq!(stream.tiles_loaded, 0);
            assert!(!stream.complete);
            assert_eq!(stream.issues.len(), 1);
        }
    }

    #[test]
    fn strict_stream_rejects_incomplete_catalog() {
        let mut index = LodIndex::new();
        index.insert(
            TileId {
                tier: BASE_TIER,
                lat: 9,
                lon: 0,
            },
            "0000000000000000",
            1,
        );
        let catalog = LodCatalog::new(index, MemoryBlobStore::new());
        let error = catalog
            .stream_strict(&LodQuery::all_sky(6.0))
            .expect_err("missing tile must fail strict streaming");
        assert_eq!(
            error.partial.issues[0].kind,
            LodIssueKind::MissingOrCorruptBlob
        );
    }

    #[test]
    fn tile_metadata_and_payload_bounds_are_enforced() {
        let mut store = MemoryBlobStore::new();
        let oversized_hash = store.put(vec![b'x'; MAX_TILE_PAYLOAD_BYTES + 1]);
        let valid_hash = store.put(gaia_tile(1, 0.0, 0.0, 4.0));
        let mut index = LodIndex::new();
        index.insert(
            TileId {
                tier: BASE_TIER,
                lat: 9,
                lon: 0,
            },
            oversized_hash,
            1,
        );
        index.insert(
            TileId {
                tier: BASE_TIER,
                lat: 9,
                lon: 1,
            },
            valid_hash,
            MAX_TILE_ROWS + 1,
        );

        let stream = LodCatalog::new(index, store).stream(&LodQuery::all_sky(6.0));
        assert!(stream.stars.is_empty());
        assert_eq!(stream.tiles_loaded, 0);
        assert!(!stream.complete);
        assert_eq!(stream.issues.len(), 2);
    }

    #[test]
    fn content_hash_dedupes_identical_payloads_and_rejects_mismatches() {
        let mut store = MemoryBlobStore::new();
        let h1 = store.put(b"abc".to_vec());
        let h2 = store.put(b"abc".to_vec());
        let h3 = store.put(b"xyz".to_vec());
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
        assert_eq!(store.get(&h1).as_deref(), Some(b"abc".as_slice()));

        store.blobs.insert(h1.clone(), b"corrupted".to_vec());
        assert_eq!(store.get(&h1), None);
        assert_eq!(store.get("../../outside-cas"), None);
    }

    #[cfg(feature = "filesystem")]
    #[test]
    fn fs_cas_store_round_trips_a_tile() {
        let root = std::env::temp_dir().join(format!(
            "stars-lod-cas-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = FsCasBlobStore::new(&root);
        let payload = gaia_tile(2947050466531873024, 101.287, -16.716, 0.4);
        let hash = store.put(payload.as_bytes()).expect("write blob");

        let mut index = LodIndex::new();
        let tile = TileId::for_direction(
            BASE_TIER,
            crate::coords::radec_hours_deg_to_cartesian(101.287 / 15.0, -16.716),
        );
        index.insert(tile, hash.clone(), 1);
        let cat = LodCatalog::new(index, store);

        let stream = cat.stream(&LodQuery::all_sky(6.0));
        assert_eq!(stream.tiles_loaded, 1);
        assert_eq!(stream.stars.len(), 1);
        assert_eq!(
            stream.stars[0].identifiers.primary,
            Some(CatalogObjectId::GaiaDr3(2947050466531873024))
        );

        std::fs::write(cat.store.blob_path(&hash), b"corrupted").unwrap();
        let corrupted = cat.stream(&LodQuery::all_sky(6.0));
        assert_eq!(corrupted.tiles_loaded, 0);
        assert!(corrupted.stars.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }
}
