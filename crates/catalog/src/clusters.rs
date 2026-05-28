//! Open-cluster membership tags.
//!
//! V-42 ships every bright open cluster as a single deep-sky marker. For the
//! naked-eye-canon clusters that view is wrong: the visible reality is a
//! resolved field of stars (Pleiades, Beehive, Double Cluster). V-53 fixes
//! that by carrying a small membership table that joins HYG / Hipparcos IDs
//! to the parent cluster's DSO identifier so the renderer can:
//!
//! - decide whether to draw the cluster's marker geometry at all (see
//!   [`DeepSkyCatalog::resolve_as_member_field`]); and
//! - in future work, attach a per-star "this is M45" tooltip without
//!   re-joining tables at render time.
//!
//! ### Data shape
//!
//! The committed CSV at `crates/catalog/data/cluster_membership.csv` is the
//! source of truth. It carries `(cluster_id, hyg_id, hip_id, name)` and is
//! parsed exactly once on first call into a static map. The table is small
//! (~30 rows in the bootstrap slice) so the parsed form lives in a
//! [`std::sync::OnceLock`] rather than a build-time binary; the Cantat-Gaudin
//! follow-up that swaps in the full membership catalog will switch to the
//! `build.rs` compaction pattern used by `deepsky.rs`.
//!
//! ### Scope (V-53)
//!
//! The first slice is a *hand-curated showpiece bootstrap* for the four
//! clusters that already carry a V-42 marker:
//!
//! - **Pleiades (M45)** — 9 named members
//! - **Praesepe / Beehive (M44)** — 11 bright core members
//! - **Double Cluster (NGC 869 + NGC 884)** — HYG-resolvable bright members
//!   (HYG truncates near V = 9 so the deep photometric core is not in this
//!   bootstrap; a future Cantat-Gaudin pull will fill the rest).
//!
//! Hyades (Mel 25) is deferred to the follow-up: it has no V-42 marker
//! today, so there is nothing for `resolve_as_member_field` to suppress.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::deepsky::DeepSkyId;

/// One row from the open-cluster membership table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClusterMember {
    /// HYG v4.2 catalog `id` for this member star.
    pub hyg_id: u32,
    /// Hipparcos catalog number when HYG carries one. `None` for HYG rows
    /// (typically faint stars in Double Cluster) where no HIP cross-id is
    /// available.
    pub hip_id: Option<u32>,
}

const CLUSTER_MEMBERSHIP_CSV: &str = include_str!("../data/cluster_membership.csv");

struct ClusterTables {
    by_id: HashMap<DeepSkyId, Vec<ClusterMember>>,
}

fn parsed() -> &'static ClusterTables {
    static TABLES: OnceLock<ClusterTables> = OnceLock::new();
    TABLES.get_or_init(parse_membership_csv)
}

fn parse_membership_csv() -> ClusterTables {
    let mut by_id: HashMap<DeepSkyId, Vec<ClusterMember>> = HashMap::new();
    let mut header_seen = false;
    for (line_number, raw_line) in CLUSTER_MEMBERSHIP_CSV.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if !header_seen {
            // Sanity-check column order so a silent reorder in the CSV
            // does not corrupt the parsed table.
            assert_eq!(
                trimmed,
                "cluster_id,hyg_id,hip_id,name",
                "cluster_membership.csv header mismatch at line {}",
                line_number + 1
            );
            header_seen = true;
            continue;
        }
        let mut fields = trimmed.splitn(4, ',');
        let cluster_id_str = fields.next().expect("cluster_id column present").trim();
        let hyg_str = fields.next().expect("hyg_id column present").trim();
        let hip_str = fields.next().expect("hip_id column present").trim();
        // The fourth field (name) is informational; the renderer ignores it.

        let Some(id) = parse_cluster_id(cluster_id_str) else {
            // Cluster IDs that do not resolve to a current V-42 DSO marker
            // (e.g. `Mel25` for Hyades) are not the renderer's concern yet.
            // They are still committed in the CSV for the Cantat-Gaudin
            // follow-up, but skipped at parse time so the table stays
            // consistent with what `DeepSkyCatalog::resolve_as_member_field`
            // can possibly suppress.
            continue;
        };
        let hyg_id: u32 = hyg_str.parse().unwrap_or_else(|_| {
            panic!(
                "cluster_membership.csv: invalid hyg_id {hyg_str:?} at line {}",
                line_number + 1
            )
        });
        let hip_id = if hip_str.is_empty() {
            None
        } else {
            Some(hip_str.parse::<u32>().unwrap_or_else(|_| {
                panic!(
                    "cluster_membership.csv: invalid hip_id {hip_str:?} at line {}",
                    line_number + 1
                )
            }))
        };
        by_id
            .entry(id)
            .or_default()
            .push(ClusterMember { hyg_id, hip_id });
    }
    ClusterTables { by_id }
}

fn parse_cluster_id(value: &str) -> Option<DeepSkyId> {
    if let Some(rest) = value.strip_prefix('M') {
        let n: u16 = rest.parse().ok()?;
        if (1..=110).contains(&n) {
            return Some(DeepSkyId::Messier(n));
        }
        return None;
    }
    if let Some(rest) = value.strip_prefix("NGC") {
        let n: u16 = rest.parse().ok()?;
        if n >= 1 {
            return Some(DeepSkyId::Ngc(n));
        }
        return None;
    }
    if let Some(rest) = value.strip_prefix("IC") {
        let n: u16 = rest.parse().ok()?;
        if n >= 1 {
            return Some(DeepSkyId::Ic(n));
        }
        return None;
    }
    None
}

/// Membership list for a single open cluster keyed by its DSO identifier.
///
/// Returns an empty slice for clusters not in the table (the default case
/// for nearly every DSO). The slice is fixed for the process lifetime; the
/// renderer can cache it across frames without re-querying.
pub fn cluster_members(id: DeepSkyId) -> &'static [ClusterMember] {
    parsed().by_id.get(&id).map(Vec::as_slice).unwrap_or(&[])
}

/// True when the cluster identified by `id` carries a membership list and so
/// should be drawn as a resolved star field rather than a DSO marker.
///
/// This is the predicate consulted by [`crate::DeepSkyCatalog::resolve_as_member_field`].
pub fn is_resolved_as_member_field(id: DeepSkyId) -> bool {
    !cluster_members(id).is_empty()
}

/// All cluster IDs that carry a membership list. Used by tests and by the
/// renderer's gallery generator when it wants to iterate the resolved
/// clusters without joining tables.
pub fn resolved_cluster_ids() -> Vec<DeepSkyId> {
    let mut ids: Vec<DeepSkyId> = parsed().by_id.keys().copied().collect();
    ids.sort_by_key(|id| id.sort_key());
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pleiades 7 named bright stars per Cantat-Gaudin / Hipparcos: Alcyone,
    /// Atlas, Electra, Maia, Merope, Taygeta, Pleione. These seven Hipparcos
    /// IDs come from HYG v4.2 and are what the V-53 roadmap gate asks for.
    const PLEIADES_NAMED_HIP: &[u32] = &[
        17499, // Electra
        17531, // Taygeta
        17573, // Maia
        17608, // Merope
        17702, // Alcyone
        17847, // Atlas
        17851, // Pleione
    ];

    #[test]
    fn pleiades_named_seven_are_members() {
        let members = cluster_members(DeepSkyId::Messier(45));
        assert!(
            !members.is_empty(),
            "Pleiades membership table should be populated"
        );
        for &hip in PLEIADES_NAMED_HIP {
            assert!(
                members.iter().any(|m| m.hip_id == Some(hip)),
                "Pleiades member with HIP {hip} should be present"
            );
        }
    }

    #[test]
    fn pleiades_is_resolved_as_member_field() {
        assert!(is_resolved_as_member_field(DeepSkyId::Messier(45)));
    }

    #[test]
    fn praesepe_is_resolved_as_member_field() {
        assert!(is_resolved_as_member_field(DeepSkyId::Messier(44)));
    }

    #[test]
    fn double_cluster_is_resolved_as_member_field() {
        assert!(is_resolved_as_member_field(DeepSkyId::Ngc(869)));
        assert!(is_resolved_as_member_field(DeepSkyId::Ngc(884)));
    }

    #[test]
    fn unrelated_dso_is_not_resolved_as_member_field() {
        // M31 (Andromeda) is a galaxy — never resolved into HYG stars.
        assert!(!is_resolved_as_member_field(DeepSkyId::Messier(31)));
        // NGC 7000 (North America Nebula) is also untagged.
        assert!(!is_resolved_as_member_field(DeepSkyId::Ngc(7000)));
        // IC 434 (Horsehead background) is also untagged.
        assert!(!is_resolved_as_member_field(DeepSkyId::Ic(434)));
    }

    #[test]
    fn cluster_member_hyg_ids_are_unique_per_cluster() {
        for id in resolved_cluster_ids() {
            let mut hygs: Vec<u32> = cluster_members(id).iter().map(|m| m.hyg_id).collect();
            hygs.sort_unstable();
            let len_before = hygs.len();
            hygs.dedup();
            assert_eq!(
                hygs.len(),
                len_before,
                "cluster {:?} contains a duplicate HYG id",
                id
            );
        }
    }

    #[test]
    fn resolved_cluster_ids_match_v53_scope() {
        let mut ids = resolved_cluster_ids();
        ids.sort_by_key(|id| id.sort_key());
        let expected = vec![
            DeepSkyId::Messier(44),
            DeepSkyId::Messier(45),
            DeepSkyId::Ngc(869),
            DeepSkyId::Ngc(884),
        ];
        assert_eq!(ids, expected, "V-53 ships exactly these four clusters");
    }

    /// Reference J2000 positions (RA hours, Dec degrees) for the 7 named
    /// Pleiades stars, taken from SIMBAD / HIP 17499 .. HIP 17851 at the
    /// catalog epoch. The roadmap gate for V-53 demands sub-1' agreement
    /// against these references at the renderer's 30' FOV.
    const PLEIADES_REFERENCES: &[(u32, &str, f64, f64)] = &[
        (17499, "Electra", 3.747927, 24.113339),
        (17531, "Taygeta", 3.753470, 24.467278),
        (17573, "Maia", 3.763779, 24.367748),
        (17608, "Merope", 3.772104, 23.948358),
        (17702, "Alcyone", 3.791410, 24.105137),
        (17847, "Atlas", 3.819373, 24.053415),
        (17851, "Pleione", 3.819782, 24.136712),
    ];

    /// Look up a HYG row for `hyg_id` in the bundled `hyg_v42.csv`. Returns
    /// `(ra_hours, dec_deg)`. Used by the Pleiades validation gate only.
    fn lookup_hyg_position(hyg_id: u32) -> Option<(f64, f64)> {
        // The CSV is large (~32 MB) but only loaded once per test process.
        // `load_from_csv` parses every row; here we only need a single row,
        // so we scan the raw text instead to keep the test fast.
        use std::sync::OnceLock;
        static HYG_TEXT: OnceLock<String> = OnceLock::new();
        let text = HYG_TEXT
            .get_or_init(|| std::fs::read_to_string("data/hyg_v42.csv").expect("read HYG CSV"));
        let mut lines = text.lines();
        lines.next()?; // header
        for line in lines {
            let mut fields = line.split(',');
            let id_str = fields.next()?;
            // Skip 7 columns to reach `ra`, then 1 more to reach `dec`.
            let _hip = fields.next();
            let _hd = fields.next();
            let _hr = fields.next();
            let _gl = fields.next();
            let _bf = fields.next();
            let _proper = fields.next();
            let ra = fields.next()?;
            let dec = fields.next()?;
            if let Ok(id) = id_str.parse::<u32>() {
                if id == hyg_id {
                    let ra = ra.parse::<f64>().ok()?;
                    let dec = dec.parse::<f64>().ok()?;
                    return Some((ra, dec));
                }
            }
        }
        None
    }

    fn arcmin_separation(ra_a: f64, dec_a: f64, ra_b: f64, dec_b: f64) -> f64 {
        // Angular separation in arcminutes via the great-circle (haversine
        // is overkill at < 1' so use a flat-cosine approximation that is
        // accurate to better than 0.1' over the Pleiades' few-arcminute
        // residual budget).
        let dec_mean = ((dec_a + dec_b) * 0.5).to_radians();
        let dra = (ra_b - ra_a) * 15.0 * dec_mean.cos(); // hours -> degrees
        let ddec = dec_b - dec_a;
        (dra * dra + ddec * ddec).sqrt() * 60.0
    }

    #[test]
    fn pleiades_named_seven_positions_match_within_one_arcminute() {
        // V-53 validation gate: at 30' FOV the 7 named Pleiades stars
        // must render at their correct positions within 1'. The renderer
        // consumes HYG positions directly, so it is sufficient to assert
        // that the HYG row keyed by each member's `hyg_id` sits within
        // 1' of the reference SIMBAD / Hipparcos J2000 catalog value.
        let members = cluster_members(DeepSkyId::Messier(45));
        for &(hip, name, ref_ra, ref_dec) in PLEIADES_REFERENCES {
            let member = members
                .iter()
                .find(|m| m.hip_id == Some(hip))
                .unwrap_or_else(|| panic!("Pleiades member {name} (HIP {hip}) missing from table"));
            let (hyg_ra, hyg_dec) = lookup_hyg_position(member.hyg_id)
                .unwrap_or_else(|| panic!("HYG id {} missing from hyg_v42.csv", member.hyg_id));
            let sep = arcmin_separation(ref_ra, ref_dec, hyg_ra, hyg_dec);
            assert!(
                sep < 1.0,
                "Pleiades {name} (HIP {hip}, HYG {}): HYG position is {sep:.3}' from reference",
                member.hyg_id
            );
        }
    }

    #[test]
    fn parse_cluster_id_accepts_documented_prefixes() {
        assert_eq!(parse_cluster_id("M45"), Some(DeepSkyId::Messier(45)));
        assert_eq!(parse_cluster_id("NGC869"), Some(DeepSkyId::Ngc(869)));
        assert_eq!(parse_cluster_id("IC434"), Some(DeepSkyId::Ic(434)));
        // Out-of-range Messier numbers are rejected at parse time so the
        // CSV can never tag a non-existent Messier object.
        assert_eq!(parse_cluster_id("M0"), None);
        assert_eq!(parse_cluster_id("M111"), None);
        // Unknown prefixes (Melotte, Collinder) are silently dropped so the
        // CSV can document future-scope membership without polluting the
        // renderer-visible table.
        assert_eq!(parse_cluster_id("Mel25"), None);
        assert_eq!(parse_cluster_id("Cr285"), None);
    }
}
