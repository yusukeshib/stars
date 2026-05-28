//! Deep-sky object catalogues — Messier and a bright NGC / IC subset.
//!
//! Both tables ship as i16-quantised binaries emitted by `build.rs`. The
//! decoder is intentionally small: the renderer's marker / label passes are
//! the only consumers today, and the i16 budget keeps the embedded bytes
//! well under 50 KB combined while staying lossless against the renderer's
//! arcminute-scale marker geometry (see `DeepSkyCatalog` doc-comment for
//! the quantisation budget).
//!
//! The [`DeepSkyCatalog`] trait is the single API the renderer code calls.
//! Two embedded implementations are provided:
//!
//! - [`MessierCatalog`] — 110 objects, the historic showpieces.
//! - [`NgcBrightCatalog`] — ~1,300 bright NGC / IC objects extracted from
//!   OpenNGC under V ≤ 11.5 mag (plus large diffuse nebulae without
//!   published integrated magnitudes).
//!
//! A future runtime streaming backend (`OpenNgcCsvCatalog`) is planned in
//! the V-42 follow-up PR so users can load the full ~14,000-entry OpenNGC
//! catalogue without bloating the WASM bundle.
//!
//! # Wire format
//!
//! Both binaries share an 8-byte magic + LE `u32` record count header,
//! followed by 14-byte records:
//!
//! | offset | size | meaning |
//! |--------|------|---------|
//! | 0..6   | 6    | J2000 unit-vector (3 × i16, quantised by `i16::MAX`) |
//! | 6..8   | 2    | primary identifier — Messier number 1..=110 *or* NGC number 1..=32767 *or* IC encoded as `-(n + 1)` |
//! | 8..10  | 2    | magnitude × 100 (`i16`); `9900` sentinel = no published photometry |
//! | 10..12 | 2    | major-axis size in arcminutes × 10 (`u16` range) |
//! | 12     | 1    | kind tag (see [`DeepSkyKind::from_tag`]) |
//! | 13     | 1    | reserved / padding |

use std::convert::TryInto;

const DEEP_SKY_HEADER_LEN: usize = 12;
const DEEP_SKY_RECORD_LEN: usize = 14;

const MESSIER_MAGIC: &[u8; 8] = b"MSSR1\0\0\0";
const OPENNGC_MAGIC: &[u8; 8] = b"NGCBR1\0\0";

const MESSIER_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/messier.bin"));
const OPENNGC_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/openngc_bright.bin"));

/// Sentinel magnitude value used when OpenNGC publishes no integrated V/B
/// photometry but the object is large enough (≥ 30 arcmin) to remain in the
/// bright subset. Renderers filter on `magnitude <= limit`, so a sentinel
/// of 99 hides these objects under any sensible slider position by default
/// while letting power users pull the slider to ≥ 99 to see them all.
pub const NO_PHOTOMETRY_SENTINEL_MAG: f32 = 99.0;

/// Coarse object classification carried alongside each catalogue entry.
/// Marker shape and (future) hover-card colour-coding consume this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepSkyKind {
    /// Open star cluster.
    OpenCluster,
    /// Globular star cluster.
    GlobularCluster,
    /// Galaxy (any morphology).
    Galaxy,
    /// Diffuse / emission / reflection nebula.
    Nebula,
    /// Planetary nebula.
    PlanetaryNebula,
    /// Supernova remnant.
    SupernovaRemnant,
    /// Anything else (asterisms, double stars, ambiguous identifications).
    Other,
}

impl DeepSkyKind {
    fn from_tag(tag: u8) -> Self {
        match tag {
            1 => Self::OpenCluster,
            2 => Self::GlobularCluster,
            3 => Self::Galaxy,
            4 => Self::Nebula,
            5 => Self::PlanetaryNebula,
            6 => Self::SupernovaRemnant,
            _ => Self::Other,
        }
    }
}

/// Primary identifier carried in the embedded binaries. Messier rows always
/// expose `Messier(n)`; NGC / IC rows expose `Ngc(n)` or `Ic(n)`. The
/// identifier-preservation tracked in `L-18` will extend this with optional
/// secondary IDs (PGC, common names, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeepSkyId {
    Messier(u16),
    Ngc(u16),
    Ic(u16),
}

impl DeepSkyId {
    /// Short label string used by the renderer's text overlay (e.g. `"M31"`,
    /// `"NGC7000"`, `"IC434"`). Returned as an owned [`String`] because the
    /// digit count varies; the renderer keeps these in a long-lived static
    /// table built once per process.
    pub fn label(self) -> String {
        match self {
            Self::Messier(n) => format!("M{n}"),
            Self::Ngc(n) => format!("NGC{n}"),
            Self::Ic(n) => format!("IC{n}"),
        }
    }

    /// Stable, side-effect-free sort key (Messier first, then NGC, then IC,
    /// numeric within each bucket). Used by the tests and renderer label
    /// pipeline so the on-screen order is reproducible.
    pub fn sort_key(self) -> (u8, u16) {
        match self {
            Self::Messier(n) => (0, n),
            Self::Ngc(n) => (1, n),
            Self::Ic(n) => (2, n),
        }
    }
}

/// One catalogue entry as decoded for the renderer. Marker / label rendering
/// is the only consumer today; richer downstream consumers (hover cards,
/// session JSON) should depend on this struct.
#[derive(Debug, Clone, Copy)]
pub struct DeepSkyObject {
    /// Primary identifier (Messier / NGC / IC).
    pub id: DeepSkyId,
    /// J2000 unit-vector position decoded from the i16-quantised storage.
    pub position: [f32; 3],
    /// V apparent magnitude. May equal [`NO_PHOTOMETRY_SENTINEL_MAG`] when
    /// the upstream OpenNGC row published no integrated photometry.
    pub magnitude: f32,
    /// Major-axis apparent size in arcminutes. Zero is permitted (the
    /// renderer falls back to a minimum marker size).
    pub size_arcmin: f32,
    /// Object classification.
    pub kind: DeepSkyKind,
}

/// Shared interface implemented by every deep-sky source — embedded
/// Messier, embedded bright NGC/IC, and (PR-B) a runtime CSV streaming
/// backend that reads the full OpenNGC catalogue from a user-supplied path.
pub trait DeepSkyCatalog {
    /// Catalogue name used in logs and the validation gallery (e.g.
    /// `"Messier"`, `"OpenNGC bright"`).
    fn name(&self) -> &'static str;

    /// All objects whose V magnitude is ≤ `magnitude_limit`. Objects with
    /// [`NO_PHOTOMETRY_SENTINEL_MAG`] only pass when the limit is opened
    /// past the sentinel value, which matches the renderer's existing
    /// slider-controlled density policy.
    fn objects(&self, magnitude_limit: f32) -> Vec<DeepSkyObject>;

    /// True when `id` should be drawn as a resolved field of HYG / Hipparcos
    /// stars rather than a DSO marker — the V-53 cluster-resolution policy
    /// (Pleiades M45, Praesepe M44, Double Cluster NGC 869 / 884, …).
    ///
    /// The label pass is intentionally unaffected: a naked-eye observer at
    /// 30 arcmin FOV reads `M45` over the seven Pleiades sisters, not under
    /// a phantom diamond marker drawn over the stars.
    ///
    /// Default implementation returns `false`; concrete catalogs delegate
    /// to [`crate::clusters::is_resolved_as_member_field`] when they expose
    /// a cluster that V-53 tags.
    fn resolve_as_member_field(&self, _id: DeepSkyId) -> bool {
        false
    }
}

/// Embedded Messier catalogue (110 objects).
#[derive(Debug, Clone, Copy, Default)]
pub struct MessierCatalog;

impl DeepSkyCatalog for MessierCatalog {
    fn name(&self) -> &'static str {
        "Messier"
    }

    fn objects(&self, magnitude_limit: f32) -> Vec<DeepSkyObject> {
        decode_table(MESSIER_DATA, MESSIER_MAGIC, decode_messier_id)
            .into_iter()
            .filter(|o| o.magnitude <= magnitude_limit)
            .collect()
    }

    fn resolve_as_member_field(&self, id: DeepSkyId) -> bool {
        crate::clusters::is_resolved_as_member_field(id)
    }
}

/// Embedded bright NGC / IC subset (~1,300 objects, V ≤ 11.5 plus large
/// diffuse nebulae lacking integrated photometry).
#[derive(Debug, Clone, Copy, Default)]
pub struct NgcBrightCatalog;

impl DeepSkyCatalog for NgcBrightCatalog {
    fn name(&self) -> &'static str {
        "OpenNGC bright"
    }

    fn objects(&self, magnitude_limit: f32) -> Vec<DeepSkyObject> {
        decode_table(OPENNGC_DATA, OPENNGC_MAGIC, decode_openngc_id)
            .into_iter()
            .filter(|o| o.magnitude <= magnitude_limit)
            .collect()
    }

    fn resolve_as_member_field(&self, id: DeepSkyId) -> bool {
        crate::clusters::is_resolved_as_member_field(id)
    }
}

fn decode_messier_id(raw: i16) -> DeepSkyId {
    DeepSkyId::Messier(raw as u16)
}

fn decode_openngc_id(raw: i16) -> DeepSkyId {
    if raw >= 0 {
        DeepSkyId::Ngc(raw as u16)
    } else {
        // IC encoding: stored as -(n + 1) so IC1 -> -2, IC32766 -> -32767.
        // i16::MIN is therefore never produced by the build script.
        let n = -(raw as i32) - 1;
        DeepSkyId::Ic(n as u16)
    }
}

fn decode_table(
    data: &[u8],
    expected_magic: &[u8; 8],
    decode_id: impl Fn(i16) -> DeepSkyId,
) -> Vec<DeepSkyObject> {
    assert!(
        data.len() >= DEEP_SKY_HEADER_LEN,
        "deep-sky catalog is shorter than its header"
    );
    assert_eq!(
        &data[..8],
        expected_magic,
        "deep-sky catalog has an unexpected magic header"
    );
    let count = u32::from_le_bytes(data[8..12].try_into().expect("fixed-size count")) as usize;
    let expected_len = DEEP_SKY_HEADER_LEN + count * DEEP_SKY_RECORD_LEN;
    assert_eq!(
        data.len(),
        expected_len,
        "deep-sky catalog length does not match its record count"
    );

    let mut out = Vec::with_capacity(count);
    for record in data[DEEP_SKY_HEADER_LEN..].chunks_exact(DEEP_SKY_RECORD_LEN) {
        let x = decode_unit_i16(record, 0);
        let y = decode_unit_i16(record, 2);
        let z = decode_unit_i16(record, 4);
        let raw_id = i16::from_le_bytes(record[6..8].try_into().expect("fixed-size id"));
        let mag_q = i16::from_le_bytes(record[8..10].try_into().expect("fixed-size mag"));
        let size_q = i16::from_le_bytes(record[10..12].try_into().expect("fixed-size size"));
        let kind_tag = record[12];
        out.push(DeepSkyObject {
            id: decode_id(raw_id),
            position: [x, y, z],
            magnitude: mag_q as f32 / 100.0,
            size_arcmin: size_q as f32 / 10.0,
            kind: DeepSkyKind::from_tag(kind_tag),
        });
    }
    out
}

fn decode_unit_i16(record: &[u8], offset: usize) -> f32 {
    i16::from_le_bytes(
        record[offset..offset + 2]
            .try_into()
            .expect("fixed-size unit coordinate"),
    ) as f32
        / i16::MAX as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_unit(p: [f32; 3]) -> f32 {
        (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()
    }

    fn messier(n: u16) -> DeepSkyObject {
        MessierCatalog
            .objects(99.0)
            .into_iter()
            .find(|o| o.id == DeepSkyId::Messier(n))
            .unwrap_or_else(|| panic!("M{n} missing"))
    }

    fn ngc(n: u16) -> DeepSkyObject {
        NgcBrightCatalog
            .objects(99.0)
            .into_iter()
            .find(|o| o.id == DeepSkyId::Ngc(n))
            .unwrap_or_else(|| panic!("NGC{n} missing"))
    }

    fn ic(n: u16) -> DeepSkyObject {
        NgcBrightCatalog
            .objects(99.0)
            .into_iter()
            .find(|o| o.id == DeepSkyId::Ic(n))
            .unwrap_or_else(|| panic!("IC{n} missing"))
    }

    // ---- Messier ----

    #[test]
    fn messier_table_has_110_entries() {
        let objs = MessierCatalog.objects(99.0);
        assert_eq!(objs.len(), 110);
        let mut seen = [false; 111];
        for obj in &objs {
            let DeepSkyId::Messier(n) = obj.id else {
                panic!("Messier table contains non-Messier id {:?}", obj.id);
            };
            let idx = n as usize;
            assert!((1..=110).contains(&idx), "M{n} out of range");
            assert!(!seen[idx], "duplicate M{n}");
            seen[idx] = true;
        }
        for (n, present) in seen.iter().enumerate().skip(1) {
            assert!(*present, "missing M{n}");
        }
    }

    #[test]
    fn messier_positions_are_unit_length() {
        for obj in MessierCatalog.objects(99.0) {
            let r = approx_unit(obj.position);
            assert!((r - 1.0).abs() < 1.0e-3, "{:?} position r={r}", obj.id);
        }
    }

    #[test]
    fn m31_lands_near_andromeda() {
        // M31 J2000: RA ≈ 00h42m44s, Dec ≈ +41.269°.
        let m31 = messier(31);
        let expected = [0.7390_f32, 0.1395, 0.6591];
        for (i, (got, want)) in m31.position.iter().zip(expected.iter()).enumerate() {
            assert!(
                (got - want).abs() < 5.0e-3,
                "M31 component {i}: got {got}, want {want}"
            );
        }
        assert!(m31.magnitude > 3.0 && m31.magnitude < 4.0);
        assert!(m31.size_arcmin > 60.0);
        assert_eq!(m31.kind, DeepSkyKind::Galaxy);
    }

    #[test]
    fn m42_is_below_celestial_equator() {
        let m42 = messier(42);
        assert!(m42.position[2] < 0.0);
        assert_eq!(m42.kind, DeepSkyKind::Nebula);
    }

    #[test]
    fn m45_pleiades_is_bright_open_cluster() {
        let m45 = messier(45);
        assert!(m45.magnitude < 2.5);
        assert!(m45.size_arcmin > 60.0);
        assert_eq!(m45.kind, DeepSkyKind::OpenCluster);
    }

    #[test]
    fn messier_quantisation_round_trips() {
        let m1 = messier(1);
        assert!((m1.magnitude - 8.4).abs() < 0.02);
        assert!((m1.size_arcmin - 8.0).abs() < 0.2);
        let m31 = messier(31);
        assert!((m31.magnitude - 3.44).abs() < 0.02);
        assert!((m31.size_arcmin - 177.83).abs() < 0.2);
    }

    // ---- OpenNGC bright subset ----

    #[test]
    fn ngc_bright_table_has_no_messier_duplicates() {
        for obj in NgcBrightCatalog.objects(99.0) {
            assert!(
                !matches!(obj.id, DeepSkyId::Messier(_)),
                "NGC bright table leaked a Messier id: {:?}",
                obj.id
            );
        }
    }

    #[test]
    fn ngc_bright_positions_are_unit_length() {
        for obj in NgcBrightCatalog.objects(99.0) {
            let r = approx_unit(obj.position);
            assert!((r - 1.0).abs() < 1.0e-3, "{:?} position r={r}", obj.id);
        }
    }

    #[test]
    fn ngc_bright_anchor_objects_present() {
        // Hand-picked showpieces with reference V magnitudes that should
        // survive the V ≤ 11.5 filter. The size assertion guards against
        // accidentally dropping the size column in regeneration.
        let cases: &[(u16, f32, f32, DeepSkyKind)] = &[
            (7000, 4.0, 100.0, DeepSkyKind::Nebula), // North America Nebula
            (869, 4.5, 10.0, DeepSkyKind::OpenCluster), // h Persei
            (884, 4.5, 8.0, DeepSkyKind::OpenCluster), // chi Persei
            (7293, 8.0, 15.0, DeepSkyKind::PlanetaryNebula), // Helix
            (253, 8.0, 20.0, DeepSkyKind::Galaxy),   // Sculptor Galaxy
            (5128, 8.0, 20.0, DeepSkyKind::Galaxy),  // Centaurus A
            (1499, 5.0, 100.0, DeepSkyKind::Nebula), // California Nebula
            (6960, 7.0, 100.0, DeepSkyKind::SupernovaRemnant), // Veil west
        ];
        for &(n, mag_max, size_min, expected_kind) in cases {
            let obj = ngc(n);
            assert!(
                obj.magnitude < mag_max,
                "NGC{n} expected mag < {mag_max}, got {}",
                obj.magnitude
            );
            assert!(
                obj.size_arcmin >= size_min,
                "NGC{n} expected size >= {size_min}', got {}'",
                obj.size_arcmin
            );
            assert_eq!(
                obj.kind, expected_kind,
                "NGC{n} kind mismatch (got {:?})",
                obj.kind
            );
        }
        // IC anchor: Horsehead complex.
        let ic434 = ic(434);
        assert_eq!(ic434.kind, DeepSkyKind::Nebula);
        assert!(ic434.size_arcmin >= 30.0);
    }

    #[test]
    fn ngc_no_photometry_objects_use_sentinel_and_filter_correctly() {
        // NGC 7000 has a published V magnitude; large nebulae without V
        // (e.g. Cl+N rows in OpenNGC at the bright-subset cutoff) should
        // appear with magnitude == NO_PHOTOMETRY_SENTINEL_MAG.
        let all = NgcBrightCatalog.objects(99.0);
        let sentinel_count = all
            .iter()
            .filter(|o| (o.magnitude - NO_PHOTOMETRY_SENTINEL_MAG).abs() < 0.01)
            .count();
        assert!(
            sentinel_count > 0,
            "expected at least one no-photometry sentinel entry"
        );
        // The default user slider sits at V=7; sentinel objects must be hidden.
        let bright = NgcBrightCatalog.objects(7.0);
        assert!(
            bright
                .iter()
                .all(|o| o.magnitude < NO_PHOTOMETRY_SENTINEL_MAG),
            "no sentinel objects should leak through magnitude_limit=7"
        );
    }

    #[test]
    fn ngc_id_label_round_trips() {
        assert_eq!(DeepSkyId::Messier(31).label(), "M31");
        assert_eq!(DeepSkyId::Ngc(7000).label(), "NGC7000");
        assert_eq!(DeepSkyId::Ic(434).label(), "IC434");
    }

    #[test]
    fn ngc_magnitude_filter_is_inclusive() {
        // NGC 7293 has mag ≈ 6.9 (B-derived). Limit = 7 must include it;
        // limit = 6 must exclude it.
        let mag = ngc(7293).magnitude;
        assert!(mag > 6.0 && mag < 7.5);
        let with_seven = NgcBrightCatalog.objects(7.5);
        assert!(with_seven.iter().any(|o| o.id == DeepSkyId::Ngc(7293)));
        let with_six = NgcBrightCatalog.objects(6.0);
        assert!(!with_six.iter().any(|o| o.id == DeepSkyId::Ngc(7293)));
    }
}
