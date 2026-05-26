//! Messier deep-sky object catalog, embedded at build time.
//!
//! The renderer build script ([`crates/renderer/build.rs`]) compacts
//! `data/messier.csv` into an i16-quantised binary table dropped in
//! `OUT_DIR/messier.bin`. This module owns the decoder; the line-overlay
//! pipeline (`crates/renderer/src/overlay.rs`) converts each entry into a
//! small line-segment marker so deep-sky objects do not leak into the star
//! catalogue API surface.
//!
//! Quantisation budget:
//!
//! - Position: J2000 unit vector quantised by `i16::MAX`, so the round-trip
//!   error is at most ~3 × 10⁻⁵ in each component (≈0.6 arcsec on a unit sphere),
//!   negligible against the markers (which are 5–60 arcmin wide).
//! - Magnitude: × 100, signed 16-bit. Range ±327.67 mag covers anything.
//! - Size: arcminutes × 10, unsigned-bounded 0–3276.7 arcmin. The largest
//!   Messier object (Pleiades, ≈110 arcmin) sits well below the upper bound.

use std::convert::TryInto;

const MESSIER_BINARY_MAGIC: &[u8; 8] = b"MSSR1\0\0\0";
const MESSIER_BINARY_HEADER_LEN: usize = 12;
const MESSIER_BINARY_RECORD_LEN: usize = 6 + 4 * 2 + 2;
const MESSIER_DATA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/messier.bin"));

/// Coarse object classification carried alongside each catalogue entry.
/// Future iterations can colour-code marker rendering by [`MessierKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessierKind {
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

impl MessierKind {
    fn from_tag(tag: u8) -> Self {
        match tag {
            1 => MessierKind::OpenCluster,
            2 => MessierKind::GlobularCluster,
            3 => MessierKind::Galaxy,
            4 => MessierKind::Nebula,
            5 => MessierKind::PlanetaryNebula,
            6 => MessierKind::SupernovaRemnant,
            _ => MessierKind::Other,
        }
    }
}

/// One Messier-catalogue entry as decoded for the renderer. Only marker /
/// label rendering needs these fields; the catalog backend trait is the place
/// for richer downstream consumers.
///
/// `m` and `kind` are not yet read by the marker pass (which colour-codes
/// uniformly and labels through the build-script-generated `MESSIER_LABELS`
/// table) but the public test surface and future hover / classification
/// colour work depend on them, so they stay on the struct.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct MessierObject {
    /// Messier number (1..=110).
    pub(crate) m: u16,
    /// J2000 unit-vector position decoded from the i16-quantised storage.
    pub(crate) position: [f32; 3],
    /// V apparent magnitude.
    pub(crate) magnitude: f32,
    /// Major-axis apparent size in arcminutes.
    pub(crate) size_arcmin: f32,
    /// Object classification (used by the marker pass).
    pub(crate) kind: MessierKind,
}

/// Decode the embedded Messier table into a `Vec<MessierObject>`.
///
/// Panics with a descriptive message if the embedded bytes have the wrong
/// magic or do not match the recorded record count — both indicate a build
/// script bug rather than user input.
pub(crate) fn messier_objects() -> Vec<MessierObject> {
    assert!(
        MESSIER_DATA.len() >= MESSIER_BINARY_HEADER_LEN,
        "messier catalog is shorter than its header"
    );
    assert_eq!(
        &MESSIER_DATA[..MESSIER_BINARY_MAGIC.len()],
        MESSIER_BINARY_MAGIC,
        "messier catalog has an unexpected magic header"
    );
    let count = u32::from_le_bytes(
        MESSIER_DATA[8..12]
            .try_into()
            .expect("fixed-size record count"),
    ) as usize;
    let expected_len = MESSIER_BINARY_HEADER_LEN + count * MESSIER_BINARY_RECORD_LEN;
    assert_eq!(
        MESSIER_DATA.len(),
        expected_len,
        "messier catalog length does not match its record count"
    );

    let mut out = Vec::with_capacity(count);
    for record in MESSIER_DATA[MESSIER_BINARY_HEADER_LEN..].chunks_exact(MESSIER_BINARY_RECORD_LEN)
    {
        let x = decode_unit_i16(record, 0);
        let y = decode_unit_i16(record, 2);
        let z = decode_unit_i16(record, 4);
        let m = i16::from_le_bytes(record[6..8].try_into().expect("fixed-size")) as u16;
        let _ngc = i16::from_le_bytes(record[8..10].try_into().expect("fixed-size"));
        let mag_q = i16::from_le_bytes(record[10..12].try_into().expect("fixed-size"));
        let size_q = i16::from_le_bytes(record[12..14].try_into().expect("fixed-size"));
        let kind_tag = record[14];
        out.push(MessierObject {
            m,
            position: [x, y, z],
            magnitude: mag_q as f32 / 100.0,
            size_arcmin: size_q as f32 / 10.0,
            kind: MessierKind::from_tag(kind_tag),
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

    #[test]
    fn embedded_table_has_110_entries() {
        let objs = messier_objects();
        assert_eq!(objs.len(), 110);
        // Every Messier number 1..=110 is present.
        let mut seen = [false; 111];
        for obj in &objs {
            let idx = obj.m as usize;
            assert!((1..=110).contains(&idx), "M{} out of range", obj.m);
            assert!(!seen[idx], "duplicate M{}", obj.m);
            seen[idx] = true;
        }
        for (n, present) in seen.iter().enumerate().skip(1) {
            assert!(*present, "missing M{n}");
        }
    }

    #[test]
    fn positions_are_unit_length() {
        for obj in messier_objects() {
            let r = (obj.position[0].powi(2) + obj.position[1].powi(2) + obj.position[2].powi(2))
                .sqrt();
            assert!(
                (r - 1.0).abs() < 1.0e-3,
                "M{} position not unit length: r={}",
                obj.m,
                r
            );
        }
    }

    #[test]
    fn m31_lands_near_andromeda() {
        // M31 J2000: RA ≈ 00h42m44s = 0.71234 h, Dec ≈ +41.269°.
        // Expected unit vector ≈ (0.7390, 0.1395, 0.6591). Allow generous slack
        // for the i16 round-trip + 6-decimal CSV serialisation.
        let m31 = messier_objects()
            .into_iter()
            .find(|o| o.m == 31)
            .expect("M31");
        let expected = [0.7390_f32, 0.1395_f32, 0.6591_f32];
        for (i, (got, want)) in m31.position.iter().zip(expected.iter()).enumerate() {
            assert!(
                (got - want).abs() < 5.0e-3,
                "M31 component {i}: got {got}, want {want}"
            );
        }
        assert!(
            m31.magnitude > 3.0 && m31.magnitude < 4.0,
            "M31 mag {}",
            m31.magnitude
        );
        assert!(m31.size_arcmin > 60.0, "M31 size {}'", m31.size_arcmin);
        assert_eq!(m31.kind, MessierKind::Galaxy);
    }

    #[test]
    fn m42_is_in_orion_below_equator() {
        // M42 (Orion Nebula): Dec ≈ −5.4° → z component is negative.
        let m42 = messier_objects()
            .into_iter()
            .find(|o| o.m == 42)
            .expect("M42");
        assert!(
            m42.position[2] < 0.0,
            "M42 should be below celestial equator"
        );
        assert_eq!(m42.kind, MessierKind::Nebula);
    }

    #[test]
    fn m45_pleiades_is_bright_open_cluster() {
        let m45 = messier_objects()
            .into_iter()
            .find(|o| o.m == 45)
            .expect("M45");
        assert!(m45.magnitude < 2.5, "M45 should be naked-eye bright");
        assert!(m45.size_arcmin > 60.0, "M45 should be a large open cluster");
        assert_eq!(m45.kind, MessierKind::OpenCluster);
    }

    #[test]
    fn magnitude_and_size_quantisation_round_trips() {
        // Spot checks pinning the i16-quantisation contract. M1 mag = 8.4,
        // major axis = 8.0'; M31 mag = 3.44, major axis = 177.83'.
        let objs = messier_objects();
        let m1 = objs.iter().find(|o| o.m == 1).expect("M1");
        assert!((m1.magnitude - 8.4).abs() < 0.02);
        assert!((m1.size_arcmin - 8.0).abs() < 0.2);
        let m31 = objs.iter().find(|o| o.m == 31).expect("M31");
        assert!((m31.magnitude - 3.44).abs() < 0.02);
        assert!((m31.size_arcmin - 177.83).abs() < 0.2);
    }
}
