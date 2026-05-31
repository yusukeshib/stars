//! Visual double / binary star resolution (V-54).
//!
//! HYG v4.2 merges some tight visual doubles (Algieba, the epsilon Lyrae
//! "Double Double") into a single catalog row, so at telescope-eyepiece zoom
//! (`V-43`) they appear as one sprite where the eye would clearly see two.
//! This module carries a small Washington Double Star (WDS) derived table that
//! joins the merged HYG primary to its companion's separation `rho`, position
//! angle `theta`, and per-component magnitudes / colours. [`resolve_doubles`]
//! is applied at catalog-load time (both the filesystem HYG-CSV path and the
//! embedded compact-binary path) so every host — CLI, viewer, web — resolves
//! the pairs without any host-specific wiring.
//!
//! ### Acceptance threshold
//!
//! The roadmap asks for "split only when projected separation >= 1 px in the
//! current FOV, otherwise fall back to the single merged sprite (no aliasing)".
//! The native hosts build their GPU instance buffer once and only vary the
//! camera FOV per frame (zoom does not rebuild instances), so a literal
//! FOV-gated branch could not respond to eyepiece zoom. Instead both
//! components are always emitted, and the wide-FOV "merged" appearance is an
//! emergent property of the renderer's *linear* HDR PSF accumulation (`V-16`,
//! `V-17`): two component point-spread functions of flux `f1` and `f2`
//! separated by far less than a pixel sum to a single PSF of flux `f1 + f2`,
//! which is exactly the combined-light sprite. Because the table's component
//! magnitudes combine back to (within ~0.1 mag of) the original merged
//! magnitude, no brightening or aliasing is introduced at any zoom level — the
//! split only becomes *visible* once the separation exceeds a pixel, which is
//! the requested behaviour. See `combined_magnitude_matches_merged_entry`.
//!
//! ### Scope (V-54)
//!
//! A hand-curated WDS showpiece bootstrap. Pairs that HYG already ships as two
//! distinct rows are intentionally omitted to avoid double-counting: Albireo
//! (beta-1/beta-2 Cyg), Castor (alpha Gem A/B), and Mizar -- whose B component
//! is already HYG id 118887 (~19" away) and whose naked-eye companion Alcor is
//! id 65272 (~12' away). Their gold/blue or split appearance already flows
//! through the existing catalog + `V-23` photometry pipeline. Per the roadmap
//! this is deliberately a static-epoch catalog: no spectroscopic-binary
//! modelling and no orbital animation of short-period visual binaries.

use std::f64::consts::PI;
use std::sync::OnceLock;

use glam::Vec3;

use crate::catalog::Star;
use crate::color::bv_to_rgb;
use crate::CatalogIdentifiers;

/// One resolved visual-double pair keyed to the merged HYG primary row.
#[derive(Debug, Clone)]
pub struct DoubleStar {
    /// HYG v4.2 `id` of the merged primary this pair resolves.
    pub hyg_id: u32,
    /// Unit-sphere J2000 position of the primary, used to match the catalog row
    /// even on the embedded path where numeric identifiers are dropped.
    pub primary_position: Vec3,
    /// WDS angular separation of the secondary from the primary, arcseconds.
    pub rho_arcsec: f64,
    /// WDS position angle, degrees measured from North through East.
    pub theta_deg: f64,
    /// Primary component V magnitude.
    pub mag_primary: f32,
    /// Secondary component V magnitude.
    pub mag_secondary: f32,
    /// Primary component B-V colour index.
    pub bv_primary: f32,
    /// Secondary component B-V colour index.
    pub bv_secondary: f32,
    /// Informational label.
    pub name: String,
}

const DOUBLE_STARS_CSV: &str = include_str!("../data/double_stars.csv");

/// Angular tolerance for matching a catalog row to a table primary by position
/// on the embedded path (identifiers are stripped there). 15 arcsec is well
/// above the i16 position quantisation (~6 arcsec) yet far below the gap to
/// any neighbouring catalog star around the bootstrap primaries (nothing else
/// brighter than V=8 lies within 60 arcsec of Algieba or either epsilon Lyrae
/// row), so a row can only match its intended primary and never an already-
/// resolved companion.
const MATCH_TOLERANCE_RAD: f32 = 15.0 / 3600.0 * (PI as f32) / 180.0;

fn parsed() -> &'static [DoubleStar] {
    static TABLE: OnceLock<Vec<DoubleStar>> = OnceLock::new();
    TABLE.get_or_init(parse_double_stars_csv)
}

fn parse_double_stars_csv() -> Vec<DoubleStar> {
    let mut out = Vec::new();
    let mut header_seen = false;
    for (line_number, raw_line) in DOUBLE_STARS_CSV.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if !header_seen {
            assert_eq!(
                trimmed,
                "name,hyg_id,ra_hours,dec_deg,rho_arcsec,theta_deg,m1,m2,bv1,bv2,epoch,wds_id",
                "double_stars.csv header mismatch at line {}",
                line_number + 1
            );
            header_seen = true;
            continue;
        }
        let fields: Vec<&str> = trimmed.split(',').map(str::trim).collect();
        assert_eq!(
            fields.len(),
            12,
            "double_stars.csv expects 12 columns at line {}",
            line_number + 1
        );
        let parse_f64 = |idx: usize, what: &str| -> f64 {
            fields[idx].parse::<f64>().unwrap_or_else(|_| {
                panic!(
                    "double_stars.csv: invalid {what} {:?} at line {}",
                    fields[idx],
                    line_number + 1
                )
            })
        };
        let hyg_id = fields[1].parse::<u32>().unwrap_or_else(|_| {
            panic!(
                "double_stars.csv: invalid hyg_id {:?} at line {}",
                fields[1],
                line_number + 1
            )
        });
        let ra_hours = parse_f64(2, "ra_hours");
        let dec_deg = parse_f64(3, "dec_deg");
        out.push(DoubleStar {
            hyg_id,
            primary_position: crate::coords::radec_hours_deg_to_cartesian(ra_hours, dec_deg),
            rho_arcsec: parse_f64(4, "rho_arcsec"),
            theta_deg: parse_f64(5, "theta_deg"),
            mag_primary: parse_f64(6, "m1") as f32,
            mag_secondary: parse_f64(7, "m2") as f32,
            bv_primary: parse_f64(8, "bv1") as f32,
            bv_secondary: parse_f64(9, "bv2") as f32,
            name: fields[0].to_string(),
        });
    }
    out
}

/// All double-star pairs in the bootstrap table.
pub fn double_stars() -> &'static [DoubleStar] {
    parsed()
}

/// Position of the secondary component on the unit sphere, offset from the
/// primary by the WDS separation `rho` at position angle `theta` (North through
/// East). Uses the exact spherical great-circle step rather than a tangent
/// approximation so the few-arcsecond offsets stay accurate.
fn secondary_position(primary: Vec3, rho_arcsec: f64, theta_deg: f64) -> Vec3 {
    let p = primary.as_dvec3().normalize();
    // Recover RA/Dec from the (possibly quantised) unit vector so the tangent
    // basis is consistent on both the CSV and embedded paths.
    let dec = p.z.clamp(-1.0, 1.0).asin();
    let ra = p.y.atan2(p.x);
    let (sin_ra, cos_ra) = ra.sin_cos();
    let (sin_dec, cos_dec) = dec.sin_cos();
    // Local tangent basis matching `coords::radec_hours_deg_to_cartesian`.
    let north = glam::DVec3::new(-sin_dec * cos_ra, -sin_dec * sin_ra, cos_dec);
    let east = glam::DVec3::new(-sin_ra, cos_ra, 0.0);
    let theta = theta_deg.to_radians();
    let dir = north * theta.cos() + east * theta.sin();
    let rho = rho_arcsec * (PI / 180.0 / 3600.0);
    (p * rho.cos() + dir * rho.sin()).normalize().as_vec3()
}

/// Index of the table pair this catalog row resolves, if any. Matches by HYG id
/// when the backend preserved one, else by angular proximity to the table
/// primary (the embedded path drops identifiers).
fn matching_pair(star: &Star, consumed: &[bool]) -> Option<usize> {
    let table = parsed();
    table.iter().enumerate().find_map(|(idx, pair)| {
        if consumed[idx] {
            return None;
        }
        let id_match = star.identifiers.hyg == Some(pair.hyg_id);
        let pos_match = star.position.angle_between(pair.primary_position) < MATCH_TOLERANCE_RAD;
        (id_match || pos_match).then_some(idx)
    })
}

/// Replace every merged HYG row that matches a WDS pair with its two resolved
/// component sprites, suppressing the merged sprite. Rows with no matching pair
/// pass through unchanged.
///
/// This is the single chokepoint both catalog-load paths call, so CLI, viewer,
/// and web resolve identical pairs.
pub fn resolve_doubles(stars: Vec<Star>) -> Vec<Star> {
    let table = parsed();
    if table.is_empty() {
        return stars;
    }
    let mut consumed = vec![false; table.len()];
    let mut out = Vec::with_capacity(stars.len() + table.len());
    for star in stars {
        match matching_pair(&star, &consumed) {
            Some(idx) => {
                consumed[idx] = true;
                let pair = &table[idx];
                // Primary keeps the catalog position, distance, proper motion,
                // and identifiers; only its photometry is taken from the table.
                let mut primary = star;
                primary.magnitude = pair.mag_primary;
                primary.color = bv_to_rgb(pair.bv_primary);
                // Secondary is offset by (rho, theta); it carries no identifiers
                // so search / ID preservation never sees a duplicate HYG id.
                let secondary = Star {
                    identifiers: CatalogIdentifiers::default(),
                    position: secondary_position(primary.position, pair.rho_arcsec, pair.theta_deg),
                    distance_pc: primary.distance_pc,
                    proper_motion: primary.proper_motion,
                    magnitude: pair.mag_secondary,
                    color: bv_to_rgb(pair.bv_secondary),
                };
                out.push(primary);
                out.push(secondary);
            }
            None => out.push(star),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn star_at(hyg: Option<u32>, ra_hours: f64, dec_deg: f64, mag: f32) -> Star {
        Star {
            identifiers: CatalogIdentifiers::from_hyg_row(hyg, None, None),
            position: crate::coords::radec_hours_deg_to_cartesian(ra_hours, dec_deg),
            distance_pc: 25.0,
            proper_motion: Vec3::ZERO,
            magnitude: mag,
            color: [1.0, 1.0, 1.0],
        }
    }

    fn arcsec_between(a: Vec3, b: Vec3) -> f64 {
        // Measure in f64: a ~14" angle between two unit vectors is below f32
        // dot-product precision and would cancel to zero in single precision.
        a.as_dvec3()
            .normalize()
            .angle_between(b.as_dvec3().normalize())
            * 180.0
            / PI
            * 3600.0
    }

    /// Position angle of `b` relative to `a`, degrees North through East.
    fn position_angle_deg(a: Vec3, b: Vec3) -> f64 {
        let p = a.as_dvec3().normalize();
        let dec = p.z.clamp(-1.0, 1.0).asin();
        let ra = p.y.atan2(p.x);
        let (sin_ra, cos_ra) = ra.sin_cos();
        let (sin_dec, cos_dec) = dec.sin_cos();
        let north = glam::DVec3::new(-sin_dec * cos_ra, -sin_dec * sin_ra, cos_dec);
        let east = glam::DVec3::new(-sin_ra, cos_ra, 0.0);
        let d = (b.as_dvec3().normalize() - p).normalize();
        let mut pa = d.dot(east).atan2(d.dot(north)).to_degrees();
        if pa < 0.0 {
            pa += 360.0;
        }
        pa
    }

    #[test]
    fn bootstrap_table_loads() {
        let table = double_stars();
        assert_eq!(table.len(), 3, "V-54 bootstrap ships three merged pairs");
        assert!(table.iter().any(|d| d.hyg_id == 50440), "Algieba present");
        assert!(
            table.iter().any(|d| d.hyg_id == 91633),
            "epsilon-1 Lyr present"
        );
        assert!(
            table.iter().any(|d| d.hyg_id == 91639),
            "epsilon-2 Lyr present"
        );
        // Mizar is already resolved by HYG (A=65173, B=118887, Alcor=65272),
        // so it must NOT be in the split table or we would add a phantom third
        // component.
        assert!(
            !table.iter().any(|d| d.hyg_id == 65173),
            "Mizar is already two HYG rows; it must not be re-split"
        );
    }

    #[test]
    fn algieba_splits_into_two_components_by_id() {
        let merged = vec![star_at(Some(50440), 10.332873, 19.841489, 2.01)];
        let out = resolve_doubles(merged);
        assert_eq!(out.len(), 2, "Algieba resolves into A and B");
        assert!((out[0].magnitude - 2.37).abs() < 1e-3);
        assert!((out[1].magnitude - 3.64).abs() < 1e-3);
        let sep = arcsec_between(out[0].position, out[1].position);
        assert!(
            (sep - 4.6).abs() < 0.3,
            "Algieba A/B separation {sep:.2}\" should match WDS 4.6\""
        );
        let pa = position_angle_deg(out[0].position, out[1].position);
        assert!(
            (pa - 126.0).abs() < 1.0,
            "Algieba A/B position angle {pa:.1} deg should match WDS 126 deg"
        );
    }

    #[test]
    fn algieba_splits_when_identifiers_are_stripped() {
        // Embedded path: no HYG id, match by position only.
        let merged = vec![star_at(None, 10.332873, 19.841489, 2.01)];
        let out = resolve_doubles(merged);
        assert_eq!(out.len(), 2, "Algieba resolves via position match too");
    }

    #[test]
    fn already_resolved_companion_is_not_re_split() {
        // HYG id 118887 is Mizar B, sitting ~19" from Mizar A. With the 15"
        // tolerance it must not be matched as a primary (it is not in the
        // table and is outside tolerance of any table primary anyway).
        let mizar_b = vec![star_at(Some(118887), 13.404, 54.921, 3.95)];
        let out = resolve_doubles(mizar_b);
        assert_eq!(out.len(), 1, "already-resolved Mizar B passes through");
    }

    #[test]
    fn double_double_resolves_into_four_sprites() {
        let merged = vec![
            star_at(Some(91633), 18.738984, 39.670123, 4.67),
            star_at(Some(91639), 18.739661, 39.612721, 4.59),
        ];
        let out = resolve_doubles(merged);
        assert_eq!(out.len(), 4, "epsilon Lyrae Double Double -> four sprites");
    }

    #[test]
    fn non_double_passes_through_unchanged() {
        // Sirius is not in the bootstrap table.
        let sirius = vec![star_at(Some(32263), 6.752477, -16.716116, -1.46)];
        let out = resolve_doubles(sirius);
        assert_eq!(out.len(), 1, "ordinary stars are untouched");
        assert!((out[0].magnitude - (-1.46)).abs() < 1e-3);
    }

    #[test]
    fn combined_magnitude_matches_merged_entry() {
        // Energy-conservation guard: each pair's two component magnitudes must
        // combine (Pogson flux sum) back to within 0.15 mag of the merged HYG
        // magnitude, so always-splitting introduces no wide-FOV brightening.
        // (hyg_id, merged HYG V magnitude)
        let merged_mags = [(50440_u32, 2.01), (91633, 4.67), (91639, 4.59)];
        for (hyg, merged) in merged_mags {
            let pair = double_stars()
                .iter()
                .find(|d| d.hyg_id == hyg)
                .expect("pair present");
            let f1 = 10f64.powf(-0.4 * pair.mag_primary as f64);
            let f2 = 10f64.powf(-0.4 * pair.mag_secondary as f64);
            let combined = -2.5 * (f1 + f2).log10();
            assert!(
                (combined - merged).abs() < 0.15,
                "pair {hyg}: combined {combined:.3} vs merged {merged:.3}"
            );
        }
    }

    #[test]
    fn albireo_components_are_a_gold_blue_pair() {
        // Albireo ships as two HYG rows already; V-54's gate is that the V-23
        // photometry pipeline renders the gold (cool K3 II) / blue (B8 V) pair.
        // HYG v4.2 B-V: beta-1 Cyg = 1.088, beta-2 Cyg = -0.095.
        let gold = bv_to_rgb(1.088);
        let blue = bv_to_rgb(-0.095);
        assert!(gold[0] > gold[2], "Albireo A should be gold (R > B)");
        assert!(blue[2] > blue[0], "Albireo B should be blue (B > R)");
    }

    #[test]
    fn algieba_components_keep_golden_colour() {
        let out = resolve_doubles(vec![star_at(Some(50440), 10.332873, 19.841489, 2.01)]);
        assert_eq!(out.len(), 2);
        for component in &out {
            assert!(
                component.color[0] >= component.color[2],
                "Algieba components are K-type golden (R >= B)"
            );
        }
    }
}
