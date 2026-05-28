//! V-56 object search index.
//!
//! Surfaces the existing catalog rows (bright named stars + Messier + bright
//! NGC/IC) under a small free-text query API so the host UI can map user
//! input like `"Sirius"`, `"alp cma"`, `"HD 48915"`, `"M31"`, `"NGC 224"`,
//! `"Jupiter"`, or `"土星"` to a concrete object the renderer already knows
//! about.
//!
//! The index is deliberately scoped: only ~1.2k named / Bayer / Flamsteed
//! stars (`crates/catalog/data/named_stars.tsv`), the 110 Messier objects,
//! the bright NGC/IC subset, and the solar-system bodies known to
//! [`crate::deepsky`] / `astronomy::Planet`. That covers everything the
//! `apps/web` UI can reasonably take the user to without first wiring
//! identifier preservation through the renderer (`L-18`).
//!
//! Ranking is intentionally simple:
//!
//! 1. exact (case-insensitive) match on any identifier → score 0,
//! 2. prefix match on a single identifier token → score 1,
//! 3. case-insensitive substring match on the canonical name → score 2,
//! 4. token-by-token substring match on Bayer / Flamsteed (`"alp cma"`
//!    matches `"Alp"` + `"CMa"`) → score 3,
//!
//! ties broken by magnitude (brighter wins). The cutoff is small (top
//! [`SEARCH_LIMIT_DEFAULT`] = 12 hits) so the dropdown stays usable.

use std::sync::OnceLock;

use crate::deepsky::{DeepSkyCatalog, DeepSkyId, MessierCatalog, NgcBrightCatalog};

/// Default upper bound on the number of hits returned by [`search`].
///
/// Bigger limits don't make the dropdown more useful — the user picks from
/// the top of the list, or types one more character to narrow further.
pub const SEARCH_LIMIT_DEFAULT: usize = 12;

/// Classification of a search hit. The host UI uses this to pick an icon /
/// section header and to know which ephemeris path to call when the user
/// commits to `goto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchKind {
    /// Bright named star from `data/named_stars.tsv` (`proper` /
    /// Bayer / Flamsteed designation).
    Star,
    /// Messier deep-sky object.
    Messier,
    /// NGC deep-sky object from the bright subset.
    Ngc,
    /// IC deep-sky object from the bright subset.
    Ic,
    /// Solar-system body resolved through the ephemeris path (Sun, Moon, or
    /// one of the eight major planets minus Earth).
    SolarSystem,
}

impl SearchKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Star => "star",
            Self::Messier => "messier",
            Self::Ngc => "ngc",
            Self::Ic => "ic",
            Self::SolarSystem => "solar-system",
        }
    }
}

/// Stable identifier for a search hit. The host echoes this back into
/// [`lookup_by_id`] / `goto_object` so the renderer doesn't have to
/// re-parse free-text input on every interaction.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SearchId {
    /// Index into the named-stars table; stable for the lifetime of the
    /// compiled binary because the TSV is committed and hashed by the
    /// manifest.
    NamedStar(u32),
    Messier(u16),
    Ngc(u16),
    Ic(u16),
    /// One of `"sun"`, `"moon"`, `"mercury"`, …, `"neptune"`.
    SolarSystem(&'static str),
}

impl SearchId {
    /// Encode the id as a kebab-style string the host can round-trip through
    /// a URL parameter or a `data-attr`. Inverse: [`SearchId::parse`].
    pub fn encode(&self) -> String {
        match self {
            Self::NamedStar(idx) => format!("star:{idx}"),
            Self::Messier(n) => format!("m:{n}"),
            Self::Ngc(n) => format!("ngc:{n}"),
            Self::Ic(n) => format!("ic:{n}"),
            Self::SolarSystem(name) => format!("ss:{name}"),
        }
    }

    /// Parse a string produced by [`encode`](Self::encode).
    pub fn parse(raw: &str) -> Option<Self> {
        let (kind, rest) = raw.split_once(':')?;
        match kind {
            "star" => rest.parse::<u32>().ok().map(Self::NamedStar),
            "m" => rest.parse::<u16>().ok().map(Self::Messier),
            "ngc" => rest.parse::<u16>().ok().map(Self::Ngc),
            "ic" => rest.parse::<u16>().ok().map(Self::Ic),
            "ss" => SOLAR_SYSTEM_BODIES
                .iter()
                .find(|b| b.canonical == rest)
                .map(|b| Self::SolarSystem(b.canonical)),
            _ => None,
        }
    }
}

/// One row of the named-stars table loaded from
/// `crates/catalog/data/named_stars.tsv`.
#[derive(Debug, Clone)]
pub struct NamedStar {
    pub index: u32,
    pub proper: Option<String>,
    pub bayer: Option<String>,
    pub flam: Option<String>,
    pub hr: Option<u32>,
    pub hd: Option<u32>,
    pub hip: Option<u32>,
    pub constellation: Option<String>,
    pub right_ascension_rad: f64,
    pub declination_rad: f64,
    pub magnitude: f32,
    pub distance_pc: f32,
}

impl NamedStar {
    /// Best human-readable label: prefer the proper name, then Bayer +
    /// constellation, then Flamsteed + constellation, then `HR <n>`.
    pub fn display(&self) -> String {
        if let Some(p) = &self.proper {
            return p.clone();
        }
        if let (Some(b), Some(c)) = (&self.bayer, &self.constellation) {
            return format!("{b} {c}");
        }
        if let (Some(f), Some(c)) = (&self.flam, &self.constellation) {
            return format!("{f} {c}");
        }
        if let Some(hr) = self.hr {
            return format!("HR {hr}");
        }
        if let Some(hd) = self.hd {
            return format!("HD {hd}");
        }
        if let Some(hip) = self.hip {
            return format!("HIP {hip}");
        }
        format!("Star #{}", self.index)
    }
}

/// One returned search hit.
#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub id: SearchId,
    pub kind: SearchKind,
    /// Lower is better. `0` is an exact identifier match; ties break on
    /// magnitude.
    pub score: u8,
    /// Human-readable label for the dropdown.
    pub display: String,
    /// Optional secondary identifiers shown beneath the display name (e.g.
    /// `"Alp CMa · HR 2491 · HD 48915 · HIP 32349"`).
    pub aka: String,
    pub right_ascension_rad: f64,
    pub declination_rad: f64,
    /// Apparent magnitude when the catalog provides one; planets and the
    /// Sun/Moon expose `None` here because their magnitude is time-dependent
    /// — the host computes that through the ephemeris path on `goto`.
    pub magnitude: Option<f32>,
    pub kind_hint: Option<&'static str>,
}

/// Bright stars table parsed once from `data/named_stars.tsv`.
fn named_stars() -> &'static [NamedStar] {
    static CACHE: OnceLock<Vec<NamedStar>> = OnceLock::new();
    CACHE.get_or_init(|| {
        const TSV: &str = include_str!("../data/named_stars.tsv");
        let mut out = Vec::with_capacity(1300);
        for (i, line) in TSV.lines().enumerate() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 11 {
                continue;
            }
            let opt_string = |s: &str| {
                let t = s.trim();
                if t.is_empty() {
                    None
                } else {
                    Some(t.to_string())
                }
            };
            let opt_u32 = |s: &str| s.trim().parse::<u32>().ok();
            let parse_f64 = |s: &str| s.trim().parse::<f64>().unwrap_or(0.0);
            out.push(NamedStar {
                index: i as u32,
                proper: opt_string(cols[0]),
                bayer: opt_string(cols[1]),
                flam: opt_string(cols[2]),
                hr: opt_u32(cols[3]),
                hd: opt_u32(cols[4]),
                hip: opt_u32(cols[5]),
                constellation: opt_string(cols[6]),
                right_ascension_rad: parse_f64(cols[7]),
                declination_rad: parse_f64(cols[8]),
                magnitude: parse_f64(cols[9]) as f32,
                distance_pc: parse_f64(cols[10]) as f32,
            });
        }
        out
    })
}

/// Resolve a named-star id back to the underlying row.
pub fn named_star(id: SearchId) -> Option<&'static NamedStar> {
    if let SearchId::NamedStar(idx) = id {
        named_stars().iter().find(|s| s.index == idx)
    } else {
        None
    }
}

/// Famous-name aliases for the most commonly searched Messier objects.
/// The full Messier list is matched by identifier (`"M31"`, `"31"`); this
/// table just lets `"andromeda"` / `"pleiades"` / `"orion"` find the right
/// row.
const MESSIER_NICKNAMES: &[(u16, &str)] = &[
    (1, "Crab Nebula"),
    (8, "Lagoon Nebula"),
    (13, "Hercules Cluster"),
    (16, "Eagle Nebula"),
    (17, "Omega Nebula"),
    (20, "Trifid Nebula"),
    (27, "Dumbbell Nebula"),
    (31, "Andromeda Galaxy"),
    (33, "Triangulum Galaxy"),
    (42, "Orion Nebula"),
    (44, "Beehive Cluster"),
    (45, "Pleiades"),
    (51, "Whirlpool Galaxy"),
    (57, "Ring Nebula"),
    (64, "Black Eye Galaxy"),
    (81, "Bode's Galaxy"),
    (82, "Cigar Galaxy"),
    (97, "Owl Nebula"),
    (101, "Pinwheel Galaxy"),
    (104, "Sombrero Galaxy"),
];

const NGC_NICKNAMES: &[(u16, &str)] = &[
    (224, "Andromeda Galaxy"),
    (598, "Triangulum Galaxy"),
    (869, "Double Cluster (h Persei)"),
    (884, "Double Cluster (χ Persei)"),
    (1976, "Orion Nebula"),
    (1952, "Crab Nebula"),
    (2244, "Rosette Nebula"),
    (2683, "UFO Galaxy"),
    (2632, "Beehive Cluster"),
    (3372, "Carina Nebula"),
    (5128, "Centaurus A"),
    (5139, "Omega Centauri"),
    (5194, "Whirlpool Galaxy"),
    (6611, "Eagle Nebula"),
    (6720, "Ring Nebula"),
    (6853, "Dumbbell Nebula"),
    (6960, "Veil Nebula (Western)"),
    (6992, "Veil Nebula (Eastern)"),
    (7000, "North America Nebula"),
];

/// Solar-system body row. Position evaluation happens in `apps/web` /
/// `apps/cli` (it depends on the observer + epoch), so this table only
/// carries the searchable identifiers + canonical key.
pub struct SolarSystemBody {
    pub canonical: &'static str,
    pub aliases: &'static [&'static str],
    pub display_en: &'static str,
    pub display_ja: &'static str,
}

pub const SOLAR_SYSTEM_BODIES: &[SolarSystemBody] = &[
    SolarSystemBody {
        canonical: "sun",
        aliases: &["sun", "sol", "soleil", "太陽", "ひ", "sunday"],
        display_en: "Sun",
        display_ja: "太陽",
    },
    SolarSystemBody {
        canonical: "moon",
        aliases: &["moon", "luna", "月", "つき"],
        display_en: "Moon",
        display_ja: "月",
    },
    SolarSystemBody {
        canonical: "mercury",
        aliases: &["mercury", "水星", "すいせい"],
        display_en: "Mercury",
        display_ja: "水星",
    },
    SolarSystemBody {
        canonical: "venus",
        aliases: &["venus", "金星", "きんせい"],
        display_en: "Venus",
        display_ja: "金星",
    },
    SolarSystemBody {
        canonical: "mars",
        aliases: &["mars", "火星", "かせい"],
        display_en: "Mars",
        display_ja: "火星",
    },
    SolarSystemBody {
        canonical: "jupiter",
        aliases: &["jupiter", "木星", "もくせい"],
        display_en: "Jupiter",
        display_ja: "木星",
    },
    SolarSystemBody {
        canonical: "saturn",
        aliases: &["saturn", "土星", "どせい"],
        display_en: "Saturn",
        display_ja: "土星",
    },
    SolarSystemBody {
        canonical: "uranus",
        aliases: &["uranus", "天王星", "てんのうせい"],
        display_en: "Uranus",
        display_ja: "天王星",
    },
    SolarSystemBody {
        canonical: "neptune",
        aliases: &["neptune", "海王星", "かいおうせい"],
        display_en: "Neptune",
        display_ja: "海王星",
    },
];

fn norm(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

fn tokens(s: &str) -> Vec<String> {
    s.split_whitespace().map(|t| t.to_lowercase()).collect()
}

/// Public search entry-point. Returns up to `limit` ranked matches.
///
/// `limit = 0` is treated as [`SEARCH_LIMIT_DEFAULT`]. The result is
/// deterministic for a given (query, limit) pair.
pub fn search(query: &str, limit: usize) -> Vec<SearchMatch> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let limit = if limit == 0 {
        SEARCH_LIMIT_DEFAULT
    } else {
        limit
    };

    let q_norm = norm(trimmed);
    let q_tokens = tokens(trimmed);
    let mut hits: Vec<SearchMatch> = Vec::new();

    // -- solar-system bodies ------------------------------------------------
    for body in SOLAR_SYSTEM_BODIES {
        let mut best_score: Option<u8> = None;
        for alias in body.aliases.iter().chain(std::iter::once(&body.display_en)) {
            let n = norm(alias);
            if n.is_empty() {
                continue;
            }
            let score = if n == q_norm {
                Some(0)
            } else if n.starts_with(&q_norm) {
                Some(1)
            } else if n.contains(&q_norm) {
                Some(2)
            } else {
                None
            };
            if let Some(s) = score {
                best_score = Some(best_score.map_or(s, |existing| existing.min(s)));
            }
        }
        if let Some(score) = best_score {
            hits.push(SearchMatch {
                id: SearchId::SolarSystem(body.canonical),
                kind: SearchKind::SolarSystem,
                score,
                display: body.display_en.to_string(),
                aka: format!("solar system · {}", body.display_ja),
                right_ascension_rad: 0.0,
                declination_rad: 0.0,
                magnitude: None,
                kind_hint: Some(body.canonical),
            });
        }
    }

    // -- named stars --------------------------------------------------------
    for star in named_stars() {
        let mut score: Option<u8> = None;

        let mut consider = |candidate: &str, exact_bucket: u8, prefix_bucket: u8| {
            let n = norm(candidate);
            if n.is_empty() {
                return;
            }
            let bucket = if n == q_norm {
                Some(exact_bucket)
            } else if n.starts_with(&q_norm) {
                Some(prefix_bucket)
            } else if n.contains(&q_norm) {
                Some(prefix_bucket.saturating_add(1))
            } else {
                None
            };
            if let Some(b) = bucket {
                score = Some(score.map_or(b, |existing| existing.min(b)));
            }
        };

        if let Some(proper) = &star.proper {
            consider(proper, 0, 1);
        }
        // Bayer + constellation as a joint string ("alp cma")
        if let (Some(b), Some(c)) = (&star.bayer, &star.constellation) {
            consider(&format!("{b} {c}"), 0, 1);
            consider(b, 1, 2);
        }
        if let (Some(f), Some(c)) = (&star.flam, &star.constellation) {
            consider(&format!("{f} {c}"), 0, 1);
        }
        if let Some(hr) = star.hr {
            consider(&format!("HR{hr}"), 0, 1);
            consider(&format!("HR {hr}"), 0, 1);
        }
        if let Some(hd) = star.hd {
            consider(&format!("HD{hd}"), 0, 1);
            consider(&format!("HD {hd}"), 0, 1);
        }
        if let Some(hip) = star.hip {
            consider(&format!("HIP{hip}"), 0, 1);
            consider(&format!("HIP {hip}"), 0, 1);
        }

        // Token-by-token Bayer+constellation match: "alp cen" → Alpha
        // Centauri, "21 and" → 21 Andromedae, etc.
        if q_tokens.len() >= 2 {
            let mut joined = String::new();
            if let Some(b) = &star.bayer {
                joined.push_str(&b.to_lowercase());
                joined.push(' ');
            }
            if let Some(f) = &star.flam {
                joined.push_str(&f.to_lowercase());
                joined.push(' ');
            }
            if let Some(c) = &star.constellation {
                joined.push_str(&c.to_lowercase());
            }
            if !joined.is_empty() && q_tokens.iter().all(|t| joined.contains(t.as_str())) {
                score = Some(score.map_or(3, |existing| existing.min(3)));
            }
        }

        if let Some(s) = score {
            let mut aka_parts = Vec::new();
            if let (Some(b), Some(c)) = (&star.bayer, &star.constellation) {
                aka_parts.push(format!("{b} {c}"));
            }
            if let Some(f) = &star.flam {
                if let Some(c) = &star.constellation {
                    aka_parts.push(format!("{f} {c}"));
                }
            }
            if let Some(hr) = star.hr {
                aka_parts.push(format!("HR {hr}"));
            }
            if let Some(hd) = star.hd {
                aka_parts.push(format!("HD {hd}"));
            }
            if let Some(hip) = star.hip {
                aka_parts.push(format!("HIP {hip}"));
            }
            hits.push(SearchMatch {
                id: SearchId::NamedStar(star.index),
                kind: SearchKind::Star,
                score: s,
                display: star.display(),
                aka: aka_parts.join(" · "),
                right_ascension_rad: star.right_ascension_rad,
                declination_rad: star.declination_rad,
                magnitude: Some(star.magnitude),
                kind_hint: None,
            });
        }
    }

    // -- Messier ------------------------------------------------------------
    for object in MessierCatalog.objects(99.0) {
        let n = match object.id {
            DeepSkyId::Messier(n) => n,
            _ => continue,
        };
        let label = format!("M{n}");
        let nickname = MESSIER_NICKNAMES
            .iter()
            .find(|(k, _)| *k == n)
            .map(|(_, name)| *name);
        let mut score: Option<u8> = None;
        let mut consider = |candidate: &str, exact_bucket: u8| {
            let nm = norm(candidate);
            if nm.is_empty() {
                return;
            }
            let bucket = if nm == q_norm {
                Some(exact_bucket)
            } else if nm.starts_with(&q_norm) {
                Some(exact_bucket.saturating_add(1))
            } else if nm.contains(&q_norm) {
                Some(exact_bucket.saturating_add(2))
            } else {
                None
            };
            if let Some(b) = bucket {
                score = Some(score.map_or(b, |existing| existing.min(b)));
            }
        };
        consider(&label, 0);
        consider(&format!("messier {n}"), 0);
        if let Some(nk) = nickname {
            consider(nk, 0);
        }

        if let Some(s) = score {
            let (ra, dec) = unit_to_radec(object.position);
            let mut aka = format!("Messier {n}");
            if let Some(nk) = nickname {
                aka.push_str(" · ");
                aka.push_str(nk);
            }
            hits.push(SearchMatch {
                id: SearchId::Messier(n),
                kind: SearchKind::Messier,
                score: s,
                display: nickname.unwrap_or(&label).to_string(),
                aka,
                right_ascension_rad: ra,
                declination_rad: dec,
                magnitude: if object.magnitude < 90.0 {
                    Some(object.magnitude)
                } else {
                    None
                },
                kind_hint: None,
            });
        }
    }

    // -- bright NGC / IC ----------------------------------------------------
    for object in NgcBrightCatalog.objects(99.0) {
        let (n, kind, label_prefix) = match object.id {
            DeepSkyId::Ngc(n) => (n, SearchKind::Ngc, "NGC"),
            DeepSkyId::Ic(n) => (n, SearchKind::Ic, "IC"),
            DeepSkyId::Messier(_) => continue,
        };
        let label = format!("{label_prefix}{n}");
        let nickname = if kind == SearchKind::Ngc {
            NGC_NICKNAMES.iter().find(|(k, _)| *k == n).map(|(_, v)| *v)
        } else {
            None
        };

        let mut score: Option<u8> = None;
        let mut consider = |candidate: &str, exact_bucket: u8| {
            let nm = norm(candidate);
            if nm.is_empty() {
                return;
            }
            let bucket = if nm == q_norm {
                Some(exact_bucket)
            } else if nm.starts_with(&q_norm) {
                Some(exact_bucket.saturating_add(1))
            } else {
                None
            };
            if let Some(b) = bucket {
                score = Some(score.map_or(b, |existing| existing.min(b)));
            }
        };
        consider(&label, 0);
        consider(&format!("{label_prefix} {n}"), 0);
        if let Some(nk) = nickname {
            consider(nk, 1);
        }

        if let Some(s) = score {
            let (ra, dec) = unit_to_radec(object.position);
            let id = if kind == SearchKind::Ngc {
                SearchId::Ngc(n)
            } else {
                SearchId::Ic(n)
            };
            let mut aka = format!("{label_prefix} {n}");
            if let Some(nk) = nickname {
                aka.push_str(" · ");
                aka.push_str(nk);
            }
            hits.push(SearchMatch {
                id,
                kind,
                score: s,
                display: nickname.unwrap_or(&label).to_string(),
                aka,
                right_ascension_rad: ra,
                declination_rad: dec,
                magnitude: if object.magnitude < 90.0 {
                    Some(object.magnitude)
                } else {
                    None
                },
                kind_hint: None,
            });
        }
    }

    // Stable sort: score ascending, then magnitude ascending (brightest
    // wins), then identifier ordering so the dropdown is deterministic.
    hits.sort_by(|a, b| {
        a.score
            .cmp(&b.score)
            .then_with(|| {
                let ma = a.magnitude.unwrap_or(f32::INFINITY);
                let mb = b.magnitude.unwrap_or(f32::INFINITY);
                ma.partial_cmp(&mb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.display.cmp(&b.display))
    });
    hits.truncate(limit);
    hits
}

fn unit_to_radec(position: [f32; 3]) -> (f64, f64) {
    let x = position[0] as f64;
    let y = position[1] as f64;
    let z = position[2] as f64;
    let len = (x * x + y * y + z * z).sqrt().max(1e-12);
    let ra = y.atan2(x).rem_euclid(std::f64::consts::TAU);
    let dec = (z / len).clamp(-1.0, 1.0).asin();
    (ra, dec)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top_id(query: &str) -> SearchId {
        let hits = search(query, 5);
        assert!(!hits.is_empty(), "no hits for {query:?}");
        hits[0].id.clone()
    }

    #[test]
    fn proper_name_lookup_finds_sirius() {
        let hits = search("Sirius", 5);
        let top = &hits[0];
        assert_eq!(top.kind, SearchKind::Star);
        assert!(top.display.contains("Sirius"), "display={}", top.display);
        // Sirius is the brightest star (V = -1.46).
        let mag = top.magnitude.expect("Sirius has photometry");
        assert!(mag < -1.0, "Sirius mag={mag}");
    }

    #[test]
    fn case_insensitive_lookup() {
        let lo = top_id("vega");
        let hi = top_id("VEGA");
        let mixed = top_id("VeGa");
        assert_eq!(lo, hi);
        assert_eq!(lo, mixed);
    }

    #[test]
    fn bayer_designation_lookup() {
        // Alpha Canis Majoris is Sirius.
        let hits = search("Alp CMa", 5);
        assert!(
            hits.iter().any(|h| h.display.contains("Sirius")),
            "top hits = {hits:?}"
        );
    }

    #[test]
    fn token_bayer_lookup() {
        // "alp cen" → Alpha Centauri (Rigil Kentaurus).
        let hits = search("alp cen", 5);
        assert!(
            hits.iter()
                .any(|h| h.display.contains("Rigil") || h.display.contains("Alp Cen")),
            "top hits = {hits:?}"
        );
    }

    #[test]
    fn hd_identifier_lookup() {
        // HD 48915 is Sirius.
        let hits = search("HD 48915", 5);
        assert!(
            hits.iter().any(|h| h.display.contains("Sirius")),
            "hits = {hits:?}"
        );
    }

    #[test]
    fn hip_identifier_lookup() {
        // HIP 91262 is Vega.
        let hits = search("HIP 91262", 5);
        assert!(
            hits.iter().any(|h| h.display.contains("Vega")),
            "hits = {hits:?}"
        );
    }

    #[test]
    fn messier_lookup() {
        // "M31" → Andromeda.
        let hits = search("M31", 5);
        assert_eq!(hits[0].kind, SearchKind::Messier);
        assert_eq!(hits[0].id, SearchId::Messier(31));
        assert!(
            hits[0].display.contains("Andromeda"),
            "display={}",
            hits[0].display
        );
    }

    #[test]
    fn messier_nickname_lookup() {
        let hits = search("Pleiades", 5);
        assert!(
            hits.iter().any(|h| h.id == SearchId::Messier(45)),
            "hits = {hits:?}"
        );
    }

    #[test]
    fn ngc_lookup() {
        // NGC 869 / NGC 884 form the Double Cluster in Perseus. They are
        // bright enough that the embedded NGC subset always keeps them,
        // and unlike NGC 224 (= M31) they are not de-duplicated by the
        // Messier catalog at build time.
        let hits = search("NGC 869", 5);
        assert!(
            hits.iter().any(|h| matches!(h.id, SearchId::Ngc(869))),
            "hits = {hits:?}"
        );
    }

    #[test]
    fn ngc_nickname_lookup() {
        // "North America" → NGC 7000.
        let hits = search("North America", 5);
        assert!(
            hits.iter().any(|h| matches!(h.id, SearchId::Ngc(7000))),
            "hits = {hits:?}"
        );
    }

    #[test]
    fn planet_lookup() {
        let hits = search("Jupiter", 3);
        assert_eq!(hits[0].kind, SearchKind::SolarSystem);
        assert_eq!(hits[0].id, SearchId::SolarSystem("jupiter"));
    }

    #[test]
    fn moon_and_sun_lookup() {
        assert_eq!(top_id("moon"), SearchId::SolarSystem("moon"));
        assert_eq!(top_id("sun"), SearchId::SolarSystem("sun"));
    }

    #[test]
    fn japanese_planet_lookup() {
        // The Japanese reading should resolve to the same canonical body so
        // bilingual users don't have to switch input mode mid-search.
        let hits = search("土星", 3);
        assert_eq!(hits[0].id, SearchId::SolarSystem("saturn"));
    }

    #[test]
    fn empty_query_returns_nothing() {
        assert!(search("", 5).is_empty());
        assert!(search("   ", 5).is_empty());
    }

    #[test]
    fn search_id_round_trip() {
        for id in [
            SearchId::NamedStar(42),
            SearchId::Messier(31),
            SearchId::Ngc(224),
            SearchId::Ic(434),
            SearchId::SolarSystem("jupiter"),
        ] {
            let encoded = id.encode();
            let parsed = SearchId::parse(&encoded).unwrap_or_else(|| panic!("parse {encoded:?}"));
            assert_eq!(parsed, id);
        }
    }

    #[test]
    fn ranking_prefers_brighter_star_on_score_tie() {
        // Both "Bet" Lyrae (Sheliak) and "Bet" Orionis (Rigel) score the
        // same on the Bayer-only prefix, so the brighter one (Rigel, V ≈
        // 0.18) must come first.
        let hits = search("Bet Ori", 3);
        assert!(
            hits[0].display.contains("Rigel") || hits[0].display.contains("Bet Ori"),
            "first hit = {}",
            hits[0].display
        );
    }
}
