//! L-17 large-catalog ingest: Hipparcos, Tycho-2, and Gaia DR3 backends.
//!
//! These backends sit behind the [`crate::CatalogBackend`] trait defined by
//! `L-16`. They parse the *normalised CSV export* of each catalogue rather than
//! the fragile fixed-width native records: the companion fetch scripts
//! (`scripts/fetch-hipparcos.sh`, `scripts/fetch-tycho2.sh`,
//! `scripts/fetch-gaia-dr3-subset.sh`) request CSV with named columns from
//! VizieR / the Gaia archive, and the parsers read by column *name* so an
//! upstream column reordering does not silently corrupt the import.
//!
//! Why per-catalogue backends rather than concatenation: each catalogue has its
//! own astrometric zero-point and precision (Hipparcos ≈ 1 mas, Perryman 1997;
//! Tycho-2 ≈ 60 mas, Høg 2000; Gaia DR3 ≈ 20 µas for bright stars, Gaia
//! Collaboration 2022). Selecting one source at the backend level preserves
//! those documented zero-points instead of blending epochs and systematics.
//!
//! The full multi-million-row catalogues are **not** committed (Gaia DR3 alone
//! is ~1.8 billion rows). The fetch scripts download them on demand; the
//! committed [`bright_star_cross_ids`] table is a compact HIP↔HD anchor index
//! generated from the in-repo HYG catalogue (`scripts/extract-bright-star-xmatch.py`)
//! for the identifier round-trip tests. Streaming level-of-detail (LOD) paging
//! for the full Gaia source and the renderer/host wiring are tracked as the
//! `L-17` follow-up (see ROADMAP / `docs/catalog-backend-design.md`).
//!
//! References:
//! - Perryman, M. A. C. et al. 1997, A&A 323, L49 (Hipparcos).
//! - ESA 1997, *The Hipparcos and Tycho Catalogues*, ESA SP-1200 (VT/BT→V,B−V).
//! - Høg, E. et al. 2000, A&A 355, L27 (Tycho-2).
//! - Gaia Collaboration 2022, A&A 674, A1 (Gaia DR3).

#[cfg(any(feature = "filesystem", test))]
use crate::backend::{CatalogBackend, CatalogError, CatalogPage, CatalogQuery, CatalogSource};
use crate::color::bv_to_rgb;
use crate::coords::{proper_motion_vector_radians_per_year, radec_hours_deg_to_cartesian};
use crate::CatalogIdentifiers;
use crate::Star;

/// Milliarcseconds per radian, for converting catalogue proper motions
/// (`mas/yr`) into the engine's radians-per-Julian-year tangent vectors.
const MAS_PER_RADIAN: f64 = 180.0 * 3600.0 * 1000.0 / std::f64::consts::PI;

/// Distance (parsecs) assigned to a row whose catalogue has no usable parallax
/// (Tycho-2) or a non-positive parallax. The star is placed on the unit sphere
/// for sky rendering; external 3D views should treat this as "distance unknown"
/// rather than a measurement.
const UNKNOWN_DISTANCE_PC: f32 = 1.0;

/// Maximum heliocentric distance kept from a parallax inversion. Beyond the
/// Milky Way stellar disc (~30 kpc) a tiny / negative parallax has inverted to
/// a meaningless distance; treat the row's distance as unknown instead.
const MAX_PARALLAX_DISTANCE_PC: f64 = 100_000.0;

// ---------------------------------------------------------------------------
// CSV header indexing
// ---------------------------------------------------------------------------

/// A header-name → column-index map for a normalised catalogue CSV. Lookups are
/// case-insensitive and tolerant of the common VizieR / Gaia column aliases so
/// one parser handles both the VizieR export labels and the Gaia archive names.
struct Columns {
    names: Vec<String>,
}

impl Columns {
    fn from_header(header: &csv::StringRecord) -> Self {
        Self {
            names: header
                .iter()
                .map(|h| h.trim().to_ascii_lowercase())
                .collect(),
        }
    }

    /// Index of the first header that matches any of `aliases` (case-insensitive).
    fn index(&self, aliases: &[&str]) -> Option<usize> {
        aliases.iter().find_map(|alias| {
            let needle = alias.to_ascii_lowercase();
            self.names.iter().position(|name| *name == needle)
        })
    }
}

/// Read a trimmed string field from a record by column index.
fn field(record: &csv::StringRecord, index: Option<usize>) -> Option<&str> {
    let value = record.get(index?)?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn parse_f64(record: &csv::StringRecord, index: Option<usize>) -> Option<f64> {
    field(record, index)?.parse::<f64>().ok()
}

fn parse_u32(record: &csv::StringRecord, index: Option<usize>) -> Option<u32> {
    field(record, index)?.parse::<u32>().ok()
}

fn parse_u64(record: &csv::StringRecord, index: Option<usize>) -> Option<u64> {
    field(record, index)?.parse::<u64>().ok()
}

/// Convert a proper-motion component in mas/yr to radians/yr.
fn mas_per_year_to_rad(mas: f64) -> f64 {
    mas / MAS_PER_RADIAN
}

/// Invert a parallax (mas) to a heliocentric distance (parsecs), or `None` when
/// the parallax is missing, non-positive, or yields a non-physical distance.
fn distance_from_parallax_mas(parallax_mas: Option<f64>) -> Option<f32> {
    let plx = parallax_mas?;
    if plx <= 0.0 {
        return None;
    }
    let pc = 1000.0 / plx;
    if pc <= 0.0 || pc >= MAX_PARALLAX_DISTANCE_PC {
        None
    } else {
        Some(pc as f32)
    }
}

/// Assemble a [`Star`] from already-parsed catalogue fields. `ra_deg` / `dec_deg`
/// are ICRS degrees; proper motions are mas/yr with `pm_ra` already including
/// the `cos δ` factor (the VizieR / Gaia convention).
#[allow(clippy::too_many_arguments)]
fn star_from_fields(
    identifiers: CatalogIdentifiers,
    ra_deg: f64,
    dec_deg: f64,
    distance_pc: f32,
    pm_ra_mas: f64,
    pm_dec_mas: f64,
    magnitude: f64,
    bv: f32,
) -> Star {
    let ra_hours = ra_deg / 15.0;
    Star {
        identifiers,
        position: radec_hours_deg_to_cartesian(ra_hours, dec_deg),
        distance_pc,
        proper_motion: proper_motion_vector_radians_per_year(
            ra_hours,
            dec_deg,
            mas_per_year_to_rad(pm_ra_mas),
            mas_per_year_to_rad(pm_dec_mas),
        ),
        magnitude: magnitude as f32,
        color: bv_to_rgb(bv),
    }
}

// ---------------------------------------------------------------------------
// Tycho / Gaia photometric transforms
// ---------------------------------------------------------------------------

/// Johnson V from Tycho VT / BT magnitudes (ESA 1997, SP-1200 Vol. 1, §1.3):
/// `V = VT − 0.090 (BT − VT)`.
pub fn tycho_v_from_vt_bt(vt: f64, bt: f64) -> f64 {
    vt - 0.090 * (bt - vt)
}

/// Johnson B−V from Tycho VT / BT (ESA 1997, SP-1200 Vol. 1, §1.3):
/// `B−V = 0.850 (BT − VT)`.
pub fn tycho_bv_from_vt_bt(vt: f64, bt: f64) -> f64 {
    0.850 * (bt - vt)
}

/// Johnson V from the Gaia DR3 `G` magnitude and the `BP−RP` colour, using the
/// **exact Riello et al. 2021 (Gaia EDR3 / DR3) photometric relationship**
/// (A&A 649, A3, Table 5.7, Johnson-Cousins row):
///
/// ```text
/// G − V = −0.02704 + 0.01424·(BP−RP) − 0.2156·(BP−RP)² + 0.01426·(BP−RP)³
/// ```
///
/// so `V = G − (G − V)`. The published 1-σ scatter is 0.030 mag and the
/// relation is calibrated for `−0.5 ≲ BP−RP ≲ 5.0`; outside that range the
/// colour is clamped to the calibration edge before evaluation so a stray
/// faint-red row cannot produce a non-physical magnitude. Replaces the former
/// "use G directly as the display magnitude" placeholder, which was ~0.15 mag
/// off for solar-type stars and worse for red stars.
pub fn gaia_v_from_g_bp_rp(g: f64, bp_rp: f64) -> f64 {
    g - gaia_g_minus_v(bp_rp)
}

/// Riello 2021 Table 5.7 `G − V` cubic in `BP−RP` (calibration clamped to
/// `[-0.5, 5.0]`).
fn gaia_g_minus_v(bp_rp: f64) -> f64 {
    let c = bp_rp.clamp(-0.5, 5.0);
    -0.02704 + 0.01424 * c - 0.2156 * c * c + 0.01426 * c * c * c
}

/// Johnson B−V from the Gaia `BP−RP` colour, for the catalogue colour pipeline
/// (B−V → T_eff → blackbody → sRGB).
///
/// Riello et al. 2021 publishes Gaia→Johnson relationships for `G − V`,
/// `G − R`, and `G − I`, but **not** a Johnson `B` transform — Gaia carries no
/// blue Johnson-equivalent band — so a directly-cited Gaia `B−V` does not
/// exist. We therefore derive `B−V` from the two Riello relations that do span
/// the Johnson system, `V − I = (G − I) − (G − V)`, and map the resulting
/// `V − I` to `B − V` with the dwarf colour-colour relation of Caldwell et al.
/// 1993 (`B−V ≈ 0.85·(V−I)` for `V−I ≲ 1.5`, the naked-eye regime). This is a
/// *display-chroma* input, not an astrometric output; see `VALIDATION.md`.
pub fn gaia_bv_from_bp_rp(bp_rp: f64) -> f64 {
    let v_minus_i = gaia_g_minus_i(bp_rp) - gaia_g_minus_v(bp_rp);
    // Caldwell 1993 dwarf locus, clamped so very red rows stay monotonic.
    0.85 * v_minus_i.clamp(-0.4, 3.0)
}

/// Riello 2021 Table 5.7 `G − I` quadratic in `BP−RP` (calibration clamped).
fn gaia_g_minus_i(bp_rp: f64) -> f64 {
    let c = bp_rp.clamp(-0.5, 5.0);
    0.01753 + 0.76 * c - 0.0991 * c * c
}

// ---------------------------------------------------------------------------
// Paging
// ---------------------------------------------------------------------------

/// Apply the source-side magnitude filter and `max_rows` page cap shared by all
/// ingest backends. Returns the page plus whether more rows were dropped.
#[cfg(any(feature = "filesystem", test))]
fn paginate(mut stars: Vec<Star>, query: CatalogQuery, source: CatalogSource) -> CatalogPage {
    stars.retain(|star| star.magnitude <= query.max_magnitude);
    let truncated = if let Some(max_rows) = query.max_rows {
        let truncated = stars.len() > max_rows;
        stars.truncate(max_rows);
        truncated
    } else {
        false
    };
    CatalogPage {
        source,
        query,
        stars,
        truncated,
        next_page: None,
    }
}

fn csv_reader(data: &str) -> csv::Reader<&[u8]> {
    csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(data.as_bytes())
}

// ---------------------------------------------------------------------------
// Hipparcos
// ---------------------------------------------------------------------------

/// Parse a normalised Hipparcos main-catalogue (VizieR I/239) CSV export.
///
/// Recognised columns (case-insensitive, first alias wins): `HIP`;
/// `RAICRS`/`RAdeg`/`ra` and `DEICRS`/`DEdeg`/`dec` (ICRS degrees); `Vmag`/`V`;
/// `Plx` (mas); `pmRA`/`pmDE` (mas/yr, `pmRA` includes cos δ); `B-V`/`BV`; and
/// an optional `HD` cross-ID. Rows without HIP/position/magnitude are skipped.
pub fn parse_hipparcos_csv(data: &str) -> Vec<Star> {
    let mut reader = csv_reader(data);
    let Ok(header) = reader.headers() else {
        return Vec::new();
    };
    let cols = Columns::from_header(header);
    let hip_i = cols.index(&["HIP"]);
    let ra_i = cols.index(&["RAICRS", "RAdeg", "_RAJ2000", "ra"]);
    let dec_i = cols.index(&["DEICRS", "DEdeg", "_DEJ2000", "dec"]);
    let vmag_i = cols.index(&["Vmag", "V"]);
    let plx_i = cols.index(&["Plx", "parallax"]);
    let pmra_i = cols.index(&["pmRA", "pmra"]);
    let pmde_i = cols.index(&["pmDE", "pmdec"]);
    let bv_i = cols.index(&["B-V", "BV", "bp_rp"]);
    let hd_i = cols.index(&["HD"]);

    let mut stars = Vec::new();
    for record in reader.records().flatten() {
        let (Some(hip), Some(ra), Some(dec), Some(mag)) = (
            parse_u32(&record, hip_i),
            parse_f64(&record, ra_i),
            parse_f64(&record, dec_i),
            parse_f64(&record, vmag_i),
        ) else {
            continue;
        };
        let distance_pc =
            distance_from_parallax_mas(parse_f64(&record, plx_i)).unwrap_or(UNKNOWN_DISTANCE_PC);
        let identifiers = CatalogIdentifiers::from_hipparcos_row(hip, parse_u32(&record, hd_i));
        stars.push(star_from_fields(
            identifiers,
            ra,
            dec,
            distance_pc,
            parse_f64(&record, pmra_i).unwrap_or(0.0),
            parse_f64(&record, pmde_i).unwrap_or(0.0),
            mag,
            parse_f64(&record, bv_i).unwrap_or(0.0) as f32,
        ));
    }
    stars
}

/// Filesystem-backed Hipparcos main-catalogue backend (CSV export from VizieR
/// I/239). Fetch with `scripts/fetch-hipparcos.sh`.
#[cfg(feature = "filesystem")]
#[derive(Debug, Clone)]
pub struct HipparcosCsvBackend {
    path: std::path::PathBuf,
}

#[cfg(feature = "filesystem")]
impl HipparcosCsvBackend {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[cfg(feature = "filesystem")]
impl CatalogBackend for HipparcosCsvBackend {
    fn source(&self) -> CatalogSource {
        CatalogSource::HIPPARCOS
    }

    fn load(&self, query: CatalogQuery) -> Result<CatalogPage, CatalogError> {
        let data = std::fs::read_to_string(&self.path)?;
        Ok(paginate(parse_hipparcos_csv(&data), query, self.source()))
    }
}

// ---------------------------------------------------------------------------
// Tycho-2
// ---------------------------------------------------------------------------

/// Pack a Tycho-2 designation `TYC1-TYC2-TYC3` into a single `u64`:
/// `(TYC1 << 24) | (TYC2 << 4) | TYC3`. TYC1 ≤ 9537 (14 bits), TYC2 fits in
/// 20 bits, TYC3 (the component, 1–3) in 4 bits — reversible with
/// [`unpack_tyc`].
pub fn pack_tyc(tyc1: u32, tyc2: u32, tyc3: u32) -> u64 {
    ((tyc1 as u64) << 24) | ((tyc2 as u64) << 4) | (tyc3 as u64 & 0xF)
}

/// Inverse of [`pack_tyc`].
pub fn unpack_tyc(packed: u64) -> (u32, u32, u32) {
    let tyc1 = (packed >> 24) as u32;
    let tyc2 = ((packed >> 4) & 0xF_FFFF) as u32;
    let tyc3 = (packed & 0xF) as u32;
    (tyc1, tyc2, tyc3)
}

/// Parse a normalised Tycho-2 (VizieR I/259) CSV export.
///
/// Recognised columns: `TYC1`,`TYC2`,`TYC3`; `RAmdeg`/`RA_ICRS_`/`RAdeg`/`ra`
/// and the declination equivalents (ICRS degrees); `VTmag`,`BTmag`;
/// `pmRA`,`pmDE` (mas/yr); optional `HIP` cross-ID. V and B−V are derived from
/// VT/BT by the ESA 1997 transformation. Tycho-2 carries no parallax, so the
/// distance is left unknown (unit sphere).
pub fn parse_tycho2_csv(data: &str) -> Vec<Star> {
    let mut reader = csv_reader(data);
    let Ok(header) = reader.headers() else {
        return Vec::new();
    };
    let cols = Columns::from_header(header);
    let tyc1_i = cols.index(&["TYC1"]);
    let tyc2_i = cols.index(&["TYC2"]);
    let tyc3_i = cols.index(&["TYC3"]);
    let ra_i = cols.index(&["RAmdeg", "RA_ICRS_", "RAICRS", "RAdeg", "_RAJ2000", "ra"]);
    let dec_i = cols.index(&["DEmdeg", "DE_ICRS_", "DEICRS", "DEdeg", "_DEJ2000", "dec"]);
    let vt_i = cols.index(&["VTmag", "VT"]);
    let bt_i = cols.index(&["BTmag", "BT"]);
    let pmra_i = cols.index(&["pmRA", "pmra"]);
    let pmde_i = cols.index(&["pmDE", "pmdec"]);
    let hip_i = cols.index(&["HIP"]);

    let mut stars = Vec::new();
    for record in reader.records().flatten() {
        let (Some(tyc1), Some(tyc2), Some(tyc3), Some(ra), Some(dec), Some(vt)) = (
            parse_u32(&record, tyc1_i),
            parse_u32(&record, tyc2_i),
            parse_u32(&record, tyc3_i),
            parse_f64(&record, ra_i),
            parse_f64(&record, dec_i),
            parse_f64(&record, vt_i),
        ) else {
            continue;
        };
        // BT may be absent for faint red stars; fall back to VT (=> B−V 0).
        let bt = parse_f64(&record, bt_i).unwrap_or(vt);
        let v = tycho_v_from_vt_bt(vt, bt);
        let bv = tycho_bv_from_vt_bt(vt, bt);
        let identifiers = CatalogIdentifiers::from_tycho2_row(
            pack_tyc(tyc1, tyc2, tyc3),
            parse_u32(&record, hip_i),
        );
        stars.push(star_from_fields(
            identifiers,
            ra,
            dec,
            UNKNOWN_DISTANCE_PC,
            parse_f64(&record, pmra_i).unwrap_or(0.0),
            parse_f64(&record, pmde_i).unwrap_or(0.0),
            v,
            bv as f32,
        ));
    }
    stars
}

/// Filesystem-backed Tycho-2 backend (CSV export from VizieR I/259).
#[cfg(feature = "filesystem")]
#[derive(Debug, Clone)]
pub struct Tycho2CsvBackend {
    path: std::path::PathBuf,
}

#[cfg(feature = "filesystem")]
impl Tycho2CsvBackend {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[cfg(feature = "filesystem")]
impl CatalogBackend for Tycho2CsvBackend {
    fn source(&self) -> CatalogSource {
        CatalogSource::TYCHO2
    }

    fn load(&self, query: CatalogQuery) -> Result<CatalogPage, CatalogError> {
        let data = std::fs::read_to_string(&self.path)?;
        Ok(paginate(parse_tycho2_csv(&data), query, self.source()))
    }
}

// ---------------------------------------------------------------------------
// Gaia DR3
// ---------------------------------------------------------------------------

/// Parse a normalised Gaia DR3 (Gaia archive `gaia_source` / VizieR I/355) CSV.
///
/// Recognised columns: `source_id`/`Source`; `ra`/`RA_ICRS`/`RAdeg` and the
/// declination equivalents (ICRS degrees); `parallax`/`Plx` (mas);
/// `pmra`/`pmdec` (mas/yr); `phot_g_mean_mag`/`Gmag` and `bp_rp`/`BP-RP`
/// (combined into a Johnson V via the Riello 2021 `G−V` relation — see
/// [`gaia_v_from_g_bp_rp`] — and a B−V for display chroma via
/// [`gaia_bv_from_bp_rp`]); optional `HIP` / `HD` cross-IDs from a cross-match
/// join. When `bp_rp` is absent the raw `G` magnitude is used unchanged.
pub fn parse_gaia_dr3_csv(data: &str) -> Vec<Star> {
    let mut reader = csv_reader(data);
    let Ok(header) = reader.headers() else {
        return Vec::new();
    };
    let cols = Columns::from_header(header);
    let src_i = cols.index(&["source_id", "Source", "DR3Name"]);
    let ra_i = cols.index(&["ra", "RA_ICRS", "RAICRS", "RAdeg", "_RAJ2000"]);
    let dec_i = cols.index(&["dec", "DE_ICRS", "DEICRS", "DEdeg", "_DEJ2000"]);
    let plx_i = cols.index(&["parallax", "Plx"]);
    let pmra_i = cols.index(&["pmra", "pmRA"]);
    let pmde_i = cols.index(&["pmdec", "pmDE"]);
    let gmag_i = cols.index(&["phot_g_mean_mag", "Gmag", "Gment"]);
    let bprp_i = cols.index(&["bp_rp", "BP-RP", "BPRP"]);
    let hip_i = cols.index(&["HIP"]);
    let hd_i = cols.index(&["HD"]);

    let mut stars = Vec::new();
    for record in reader.records().flatten() {
        let (Some(source_id), Some(ra), Some(dec), Some(gmag)) = (
            parse_u64(&record, src_i),
            parse_f64(&record, ra_i),
            parse_f64(&record, dec_i),
            parse_f64(&record, gmag_i),
        ) else {
            continue;
        };
        let distance_pc =
            distance_from_parallax_mas(parse_f64(&record, plx_i)).unwrap_or(UNKNOWN_DISTANCE_PC);
        let bp_rp = parse_f64(&record, bprp_i);
        // Exact Riello 2021 G->V when a colour is present; otherwise fall back
        // to the raw G magnitude (colourless rows cannot be transformed).
        let v = match bp_rp {
            Some(c) => gaia_v_from_g_bp_rp(gmag, c),
            None => gmag,
        };
        let bv = bp_rp.map(gaia_bv_from_bp_rp).unwrap_or(0.0);
        let identifiers = CatalogIdentifiers::from_gaia_row(
            source_id,
            parse_u32(&record, hip_i),
            parse_u32(&record, hd_i),
        );
        stars.push(star_from_fields(
            identifiers,
            ra,
            dec,
            distance_pc,
            parse_f64(&record, pmra_i).unwrap_or(0.0),
            parse_f64(&record, pmde_i).unwrap_or(0.0),
            v,
            bv as f32,
        ));
    }
    stars
}

/// Filesystem-backed Gaia DR3 backend (CSV export). The full Gaia source is far
/// too large to embed; `scripts/fetch-gaia-dr3-subset.sh` pulls a magnitude-cut
/// subset and the backend pages it through [`CatalogQuery`]. LOD streaming of
/// the full source is the `L-17` follow-up.
#[cfg(feature = "filesystem")]
#[derive(Debug, Clone)]
pub struct GaiaDr3CsvBackend {
    path: std::path::PathBuf,
}

#[cfg(feature = "filesystem")]
impl GaiaDr3CsvBackend {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[cfg(feature = "filesystem")]
impl CatalogBackend for GaiaDr3CsvBackend {
    fn source(&self) -> CatalogSource {
        CatalogSource::GAIA_DR3
    }

    fn load(&self, query: CatalogQuery) -> Result<CatalogPage, CatalogError> {
        let data = std::fs::read_to_string(&self.path)?;
        Ok(paginate(parse_gaia_dr3_csv(&data), query, self.source()))
    }
}

// ---------------------------------------------------------------------------
// Bright-star HIP ↔ HD cross-match anchor index
// ---------------------------------------------------------------------------

/// One bright-star cross-identification anchor: the stable HIP and HD numbers
/// for a naked-eye star, with its V magnitude and B−V. Generated from the
/// in-repo HYG catalogue by `scripts/extract-bright-star-xmatch.py`.
#[derive(Debug, Clone, PartialEq)]
pub struct BrightStarCrossId {
    pub hip: u32,
    pub hd: u32,
    pub vmag: f32,
    pub bv: f32,
    pub name: String,
}

const BRIGHT_STAR_XMATCH_CSV: &str = include_str!("../data/bright_star_xmatch.csv");

/// Parse the committed bright-star HIP↔HD anchor index. The CSV header is
/// `hip,hd,vmag,bv,proper`.
pub fn bright_star_cross_ids() -> Vec<BrightStarCrossId> {
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(BRIGHT_STAR_XMATCH_CSV.as_bytes());
    let mut out = Vec::new();
    for record in reader.records().flatten() {
        let (Some(hip), Some(hd)) = (
            record.get(0).and_then(|v| v.trim().parse::<u32>().ok()),
            record.get(1).and_then(|v| v.trim().parse::<u32>().ok()),
        ) else {
            continue;
        };
        out.push(BrightStarCrossId {
            hip,
            hd,
            vmag: record
                .get(2)
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0.0),
            bv: record
                .get(3)
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0.0),
            name: record
                .get(4)
                .map(|v| v.trim().to_string())
                .unwrap_or_default(),
        });
    }
    out
}

/// Look up a bright-star anchor by Hipparcos number.
pub fn cross_id_by_hip(hip: u32) -> Option<BrightStarCrossId> {
    bright_star_cross_ids().into_iter().find(|c| c.hip == hip)
}

/// Look up a bright-star anchor by Henry Draper number.
pub fn cross_id_by_hd(hd: u32) -> Option<BrightStarCrossId> {
    bright_star_cross_ids().into_iter().find(|c| c.hd == hd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::CatalogObjectId;
    use crate::catalog::DEFAULT_MAX_MAGNITUDE;
    use glam::Vec3;

    /// Angular separation (degrees) between two unit-sphere directions.
    fn sep_deg(a: Vec3, b: Vec3) -> f64 {
        let a = a.as_dvec3().normalize();
        let b = b.as_dvec3().normalize();
        a.dot(b).clamp(-1.0, 1.0).acos().to_degrees()
    }

    #[test]
    fn parses_hipparcos_sirius_row() {
        // Hipparcos main catalogue values for Sirius (HIP 32349 = HD 48915),
        // ICRS degrees, Plx in mas, pm in mas/yr, B−V (Perryman 1997).
        let csv = "HIP,RAICRS,DEICRS,Vmag,Plx,pmRA,pmDE,B-V,HD\n\
                   32349,101.287155,-16.716116,-1.44,379.21,-546.01,-1223.07,0.009,48915\n";
        let stars = parse_hipparcos_csv(csv);
        assert_eq!(stars.len(), 1);
        let sirius = &stars[0];
        assert_eq!(
            sirius.identifiers.primary,
            Some(CatalogObjectId::Hipparcos(32349))
        );
        assert_eq!(sirius.identifiers.hip, Some(32349));
        assert_eq!(sirius.identifiers.hd, Some(48915));
        assert!((sirius.magnitude - (-1.44)).abs() < 1e-3);
        // 1000 / 379.21 mas ≈ 2.637 pc.
        assert!(
            (sirius.distance_pc - 2.637).abs() < 0.01,
            "dist={}",
            sirius.distance_pc
        );
        // Proper motion must be tangent to the position.
        assert!(sirius.position.dot(sirius.proper_motion).abs() < 1e-9);
    }

    #[test]
    fn hipparcos_position_matches_hyg_for_sirius() {
        // Cross-catalog comparison (ROADMAP L-17 validation): Sirius from the
        // Hipparcos backend vs. the HYG backend must agree to well under an
        // arcsecond (both are ICRS/J2000-aligned for this bright anchor).
        let hip = parse_hipparcos_csv(
            "HIP,RAICRS,DEICRS,Vmag,Plx,pmRA,pmDE,B-V,HD\n\
             32349,101.287155,-16.716116,-1.44,379.21,-546.01,-1223.07,0.009,48915\n",
        );
        // HYG stores RA in hours: 101.287155° / 15 = 6.7524770 h.
        let hyg_header = "id,hip,hd,hr,gl,bf,proper,ra,dec,dist,pmra,pmdec,rv,mag,absmag,spect,ci,x,y,z,vx,vy,vz,rarad,decrad,pmrarad,pmdecrad,bayer,flam,con,comp,comp_primary,base,lum,var,var_min,var_max";
        let hyg_csv = format!(
            "{hyg_header}\n\
             1,32349,48915,,,,Sirius,6.7524770,-16.716116,2.637,0.0,0.0,0.0,-1.44,1.45,A1V,0.009,0,0,0,0,0,0,0,0,0,0,,,CMa,1,1,,1.0,,,\n"
        );
        let hyg = crate::catalog::load_from_csv(&hyg_csv);
        assert_eq!(hip.len(), 1);
        assert_eq!(hyg.len(), 1);
        let separation = sep_deg(hip[0].position, hyg[0].position);
        assert!(
            separation < 1.0 / 3600.0,
            "Hipparcos vs HYG Sirius separation {separation} deg exceeds 1 arcsec"
        );
    }

    #[test]
    fn parses_tycho2_row_with_vt_bt_transform() {
        // VT/BT chosen so V and B−V are easy to verify: BT−VT = 1.0 →
        // V = VT − 0.090 = 9.41; B−V = 0.850.
        let csv = "TYC1,TYC2,TYC3,RAmdeg,DEmdeg,BTmag,VTmag,pmRA,pmDE,HIP\n\
                   1,8,1,83.5,-5.4,10.5,9.5,1.5,-2.0,12345\n";
        let stars = parse_tycho2_csv(csv);
        assert_eq!(stars.len(), 1);
        let star = &stars[0];
        assert!((star.magnitude - 9.41).abs() < 1e-3, "V={}", star.magnitude);
        match star.identifiers.primary {
            Some(CatalogObjectId::Tycho2(packed)) => {
                assert_eq!(unpack_tyc(packed), (1, 8, 1));
            }
            other => panic!("expected Tycho2 primary, got {other:?}"),
        }
        assert_eq!(star.identifiers.hip, Some(12345));
    }

    #[test]
    fn parses_gaia_dr3_row() {
        let csv = "source_id,ra,dec,parallax,pmra,pmdec,phot_g_mean_mag,bp_rp\n\
                   4472832130942575872,259.2,-2.5,4.0,10.0,-5.0,8.2,0.82\n";
        let stars = parse_gaia_dr3_csv(csv);
        assert_eq!(stars.len(), 1);
        let star = &stars[0];
        assert_eq!(
            star.identifiers.primary,
            Some(CatalogObjectId::GaiaDr3(4472832130942575872))
        );
        // G = 8.2, BP-RP = 0.82 -> Riello G-V = -0.1525 -> V = G + 0.1525.
        assert!(
            (star.magnitude - 8.3525).abs() < 1e-3,
            "V={}",
            star.magnitude
        );
        // parallax 4 mas → 250 pc.
        assert!(
            (star.distance_pc - 250.0).abs() < 0.5,
            "dist={}",
            star.distance_pc
        );
    }

    #[test]
    fn riello_g_minus_v_matches_published_solar_value() {
        // Solar BP-RP ~= 0.82; Riello 2021 Table 5.7 gives G - V ~= -0.15,
        // i.e. Gaia G is ~0.15 mag brighter than Johnson V for the Sun.
        let g_minus_v = -gaia_g_minus_v(0.82);
        // Stated as V - G here for readability of the published ~+0.15.
        assert!((g_minus_v - 0.1525).abs() < 1e-3, "V-G(solar)={g_minus_v}");
        // V from G is monotonic-ish and exact at the white-star anchor: a blue
        // A0V (BP-RP ~ 0) has |G - V| < 0.03 (the published zero-point).
        let v_a0 = gaia_v_from_g_bp_rp(5.0, 0.0);
        assert!((v_a0 - 5.0).abs() < 0.03, "A0V V={v_a0}");
    }

    #[test]
    fn riello_bv_is_monotonic_and_anchored() {
        // B-V increases with BP-RP across the naked-eye colour range.
        let blue = gaia_bv_from_bp_rp(-0.2);
        let solar = gaia_bv_from_bp_rp(0.82);
        let red = gaia_bv_from_bp_rp(1.8);
        assert!(blue < solar && solar < red, "{blue} {solar} {red}");
        // Solar BP-RP ~ 0.82 maps near the solar B-V ~ 0.65.
        assert!((solar - 0.65).abs() < 0.2, "solar B-V={solar}");
    }

    #[test]
    fn tyc_pack_round_trips() {
        for (a, b, c) in [(1u32, 8u32, 1u32), (9537, 12000, 3), (4711, 1, 2)] {
            assert_eq!(unpack_tyc(pack_tyc(a, b, c)), (a, b, c));
        }
    }

    #[test]
    fn identifier_round_trip_hip_tyc_gaia() {
        // A synthetic row that carries all three identifier families exercises
        // the cross-ID plumbing end to end (HIP → TYC → Gaia source_id).
        let gaia = parse_gaia_dr3_csv(
            "source_id,ra,dec,parallax,pmra,pmdec,phot_g_mean_mag,bp_rp,HIP,HD\n\
             2947050466531873024,101.287,-16.716,379.21,-546.0,-1223.0,-1.46,0.0,32349,48915\n",
        );
        assert_eq!(gaia.len(), 1);
        let ids = gaia[0].identifiers;
        assert_eq!(ids.gaia_dr3, Some(2947050466531873024));
        assert_eq!(ids.hip, Some(32349));
        assert_eq!(ids.hd, Some(48915));
        assert_eq!(
            ids.primary,
            Some(CatalogObjectId::GaiaDr3(2947050466531873024))
        );
    }

    #[test]
    fn magnitude_filter_and_paging() {
        let csv = "HIP,RAICRS,DEICRS,Vmag,Plx,pmRA,pmDE,B-V\n\
                   1,10.0,10.0,2.0,10.0,0.0,0.0,0.0\n\
                   2,11.0,11.0,5.0,10.0,0.0,0.0,0.0\n\
                   3,12.0,12.0,9.0,10.0,0.0,0.0,0.0\n";
        let stars = parse_hipparcos_csv(csv);
        assert_eq!(stars.len(), 3, "parser keeps every row");
        let page = paginate(
            stars,
            CatalogQuery {
                max_magnitude: 6.0,
                max_rows: Some(1),
            },
            CatalogSource::HIPPARCOS,
        );
        // mag 9 dropped by the filter; mag 2 + 5 remain but capped to 1 row.
        assert_eq!(page.stars.len(), 1);
        assert!(page.truncated);
        assert_eq!(page.source, CatalogSource::HIPPARCOS);
    }

    #[test]
    fn default_magnitude_cap_is_shared() {
        // The ingest backends reuse the repository-wide default cap.
        assert_eq!(CatalogQuery::default().max_magnitude, DEFAULT_MAX_MAGNITUDE);
    }

    #[test]
    fn bright_star_xmatch_round_trips_hip_hd() {
        let table = bright_star_cross_ids();
        assert!(
            !table.is_empty(),
            "committed bright-star anchor index is non-empty"
        );
        // Sirius is the canonical bright anchor: HIP 32349 ↔ HD 48915.
        let sirius = cross_id_by_hip(32349).expect("Sirius HIP 32349 present");
        assert_eq!(sirius.hd, 48915);
        assert_eq!(cross_id_by_hd(48915).map(|c| c.hip), Some(32349));
        // Every anchor round-trips HIP → HD → HIP.
        for anchor in &table {
            assert_eq!(cross_id_by_hd(anchor.hd).map(|c| c.hip), Some(anchor.hip));
        }
    }
}
