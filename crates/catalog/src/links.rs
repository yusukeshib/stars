//! L-19 SIMBAD / VizieR deep links.
//!
//! Builds canonical CDS lookup URLs from the identifiers an object already
//! carries (HIP / HD / HR / proper name / catalog designation), with a J2000
//! coordinate cone-search fallback when no resolvable identifier is known.
//!
//! These are *just strings*: nothing in this module makes a network call, and
//! the rendering pipeline never touches it, so deterministic renders stay
//! offline and reproducible. Hosts surface the links opt-in (the web info
//! panel) or echo them in metadata (CLI / viewer).
//!
//! URL formats follow the documented CDS query interfaces:
//! - SIMBAD identifier query `sim-id?Ident=…` and coordinate query
//!   `sim-coo?Coord=…` (Wenger et al. 2000, A&AS 143, 9).
//! - VizieR cone search `VizieR-4?-c=…&-c.rs=…` (Ochsenbein et al. 2000,
//!   A&AS 143, 23).

/// CDS service base URLs. Pinned to the `cds.unistra.fr` canonical hosts.
const SIMBAD_ID_BASE: &str = "https://simbad.cds.unistra.fr/simbad/sim-id?Ident=";
const SIMBAD_COO_BASE: &str = "https://simbad.cds.unistra.fr/simbad/sim-coo?Coord=";
const VIZIER_BASE: &str = "https://vizier.cds.unistra.fr/viz-bin/VizieR-4?-c=";

/// Default cone-search radius (arcminutes) for coordinate fallbacks. Two
/// arcminutes comfortably brackets proper-motion drift and catalogue position
/// scatter for naked-eye objects without pulling in crowded-field neighbours.
const CONE_RADIUS_ARCMIN: f64 = 2.0;

/// Identifiers available for an object, used to build external CDS deep links.
///
/// Every field except the J2000 coordinates is optional; the coordinate pair
/// is always present so a link can be produced even for an unnamed row. This
/// type deliberately reuses identifiers the hosts already resolve — it does
/// not add new catalogue-ingest or identifier-preservation machinery (that is
/// `L-17` / `L-18`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StarIdentifiers {
    /// Hipparcos catalogue number (HIP).
    pub hip: Option<u32>,
    /// Henry Draper catalogue number (HD).
    pub hd: Option<u32>,
    /// Harvard Revised / Bright Star Catalogue number (HR).
    pub hr: Option<u32>,
    /// Proper name that SIMBAD resolves directly (e.g. `"Sirius"`).
    pub proper_name: Option<String>,
    /// Pre-formatted catalogue designation SIMBAD resolves directly
    /// (e.g. `"M 31"`, `"NGC 224"`, `"IC 434"`).
    pub catalog_designation: Option<String>,
    /// J2000 right ascension, radians. Coordinate-fallback input.
    pub right_ascension_rad: f64,
    /// J2000 declination, radians. Coordinate-fallback input.
    pub declination_rad: f64,
}

impl StarIdentifiers {
    /// Best identifier string SIMBAD can resolve by name, in catalogue
    /// priority order: HIP, HD, HR, proper name, catalogue designation.
    /// Returns `None` when only coordinates are available.
    pub fn preferred_identifier(&self) -> Option<String> {
        if let Some(hip) = self.hip {
            return Some(format!("HIP {hip}"));
        }
        if let Some(hd) = self.hd {
            return Some(format!("HD {hd}"));
        }
        if let Some(hr) = self.hr {
            return Some(format!("HR {hr}"));
        }
        if let Some(name) = &self.proper_name {
            let t = name.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
        if let Some(desig) = &self.catalog_designation {
            let t = desig.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
        None
    }
}

/// SIMBAD lookup URL for the given identifiers.
///
/// Prefers an identifier query (`sim-id`) when a resolvable identifier exists,
/// otherwise falls back to a J2000 coordinate cone search (`sim-coo`).
pub fn simbad_query_url(ids: &StarIdentifiers) -> String {
    match ids.preferred_identifier() {
        Some(identifier) => format!("{SIMBAD_ID_BASE}{}", encode_query_value(&identifier)),
        None => {
            let coord = format_coord(ids.right_ascension_rad, ids.declination_rad);
            format!(
                "{SIMBAD_COO_BASE}{}&Radius={CONE_RADIUS_ARCMIN}&Radius.unit=arcmin",
                encode_query_value(&coord)
            )
        }
    }
}

/// VizieR all-catalogue cone search at the object's J2000 position.
///
/// VizieR indexes by position rather than by a single primary identifier, so
/// the cone search is the canonical entry point regardless of which names the
/// object carries.
pub fn vizier_query_url(ids: &StarIdentifiers) -> String {
    let coord = format_coord(ids.right_ascension_rad, ids.declination_rad);
    format!(
        "{VIZIER_BASE}{}&-c.rs={CONE_RADIUS_ARCMIN}",
        encode_query_value(&coord)
    )
}

/// Format a J2000 position as `"<ra_deg> <signed_dec_deg>"` in decimal degrees.
/// The space is later percent-encoded; both CDS services parse this form.
fn format_coord(ra_rad: f64, dec_rad: f64) -> String {
    let ra_deg = ra_rad.rem_euclid(std::f64::consts::TAU).to_degrees();
    let dec_deg = dec_rad.to_degrees();
    format!("{ra_deg:.6} {dec_deg:+.6}")
}

/// Percent-encode a query-string value. Unreserved characters (RFC 3986
/// `A-Za-z0-9-_.~`) pass through, spaces become `+`, and everything else is
/// percent-encoded so the URL is safe to drop straight into an `href`.
fn encode_query_value(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for &b in raw.as_bytes() {
        match b {
            b' ' => out.push('+'),
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push(hex_digit(b >> 4));
                out.push(hex_digit(b & 0x0f));
            }
        }
    }
    out
}

fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'A' + (nibble - 10)) as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hip_identifier_takes_priority() {
        let ids = StarIdentifiers {
            hip: Some(32349),
            hd: Some(48915),
            hr: Some(2491),
            proper_name: Some("Sirius".to_string()),
            ..Default::default()
        };
        assert_eq!(
            simbad_query_url(&ids),
            "https://simbad.cds.unistra.fr/simbad/sim-id?Ident=HIP+32349"
        );
    }

    #[test]
    fn identifier_priority_falls_through() {
        // HD when no HIP.
        let hd = StarIdentifiers {
            hd: Some(48915),
            hr: Some(2491),
            ..Default::default()
        };
        assert_eq!(hd.preferred_identifier().as_deref(), Some("HD 48915"));
        // HR when no HIP/HD.
        let hr = StarIdentifiers {
            hr: Some(2491),
            proper_name: Some("Sirius".to_string()),
            ..Default::default()
        };
        assert_eq!(hr.preferred_identifier().as_deref(), Some("HR 2491"));
        // Proper name when no catalogue numbers.
        let name = StarIdentifiers {
            proper_name: Some("Sirius".to_string()),
            catalog_designation: Some("M 31".to_string()),
            ..Default::default()
        };
        assert_eq!(name.preferred_identifier().as_deref(), Some("Sirius"));
        // Catalogue designation when that is all there is.
        let desig = StarIdentifiers {
            catalog_designation: Some("NGC 224".to_string()),
            ..Default::default()
        };
        assert_eq!(desig.preferred_identifier().as_deref(), Some("NGC 224"));
    }

    #[test]
    fn deepsky_designation_is_encoded() {
        let ids = StarIdentifiers {
            catalog_designation: Some("M 31".to_string()),
            ..Default::default()
        };
        assert_eq!(
            simbad_query_url(&ids),
            "https://simbad.cds.unistra.fr/simbad/sim-id?Ident=M+31"
        );
    }

    #[test]
    fn coordinate_fallback_when_no_identifier() {
        // Sirius J2000 ≈ 101.287°, −16.716°.
        let ra = 101.287_f64.to_radians();
        let dec = (-16.716_f64).to_radians();
        let ids = StarIdentifiers {
            right_ascension_rad: ra,
            declination_rad: dec,
            ..Default::default()
        };
        let simbad = simbad_query_url(&ids);
        assert!(
            simbad.starts_with(
                "https://simbad.cds.unistra.fr/simbad/sim-coo?Coord=101.287000+-16.716000"
            ),
            "simbad={simbad}"
        );
        assert!(
            simbad.contains("&Radius=2&Radius.unit=arcmin"),
            "simbad={simbad}"
        );
    }

    #[test]
    fn vizier_is_a_coordinate_cone_search() {
        let ra = 101.287_f64.to_radians();
        let dec = (-16.716_f64).to_radians();
        let ids = StarIdentifiers {
            // Even with an identifier, VizieR uses the position.
            hip: Some(32349),
            right_ascension_rad: ra,
            declination_rad: dec,
            ..Default::default()
        };
        assert_eq!(
            vizier_query_url(&ids),
            "https://vizier.cds.unistra.fr/viz-bin/VizieR-4?-c=101.287000+-16.716000&-c.rs=2"
        );
    }

    #[test]
    fn positive_declination_keeps_explicit_sign() {
        // Vega J2000 ≈ 279.234°, +38.784°.
        let ra = 279.234_f64.to_radians();
        let dec = 38.784_f64.to_radians();
        let ids = StarIdentifiers {
            right_ascension_rad: ra,
            declination_rad: dec,
            ..Default::default()
        };
        // The "+" before the declination is percent-encoded as %2B; the space
        // separator becomes "+".
        assert!(
            vizier_query_url(&ids).contains("-c=279.234000+%2B38.784000"),
            "{}",
            vizier_query_url(&ids)
        );
    }

    #[test]
    fn ra_wraps_into_range() {
        let ids = StarIdentifiers {
            right_ascension_rad: -0.001,
            declination_rad: 0.0,
            ..Default::default()
        };
        // Negative RA wraps to just under 360°, never emits a leading minus.
        let url = vizier_query_url(&ids);
        assert!(url.contains("-c=359."), "{url}");
    }

    #[test]
    fn no_identifier_no_coordinates_is_origin_cone() {
        let ids = StarIdentifiers::default();
        assert_eq!(ids.preferred_identifier(), None);
        // Still produces a valid (origin) coordinate URL rather than panicking.
        assert!(simbad_query_url(&ids).contains("sim-coo?Coord=0.000000+%2B0.000000"));
    }
}
