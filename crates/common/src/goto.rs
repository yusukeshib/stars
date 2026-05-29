//! V-56 object search → GoTo resolution shared by the native hosts.
//!
//! The web host (`apps/web`) already ships search + GoTo + an info panel by
//! mapping [`catalog::search`] hits to apparent topocentric positions. This
//! module lifts the *engine-side* half of that work — resolving a
//! [`SearchId`] (or a free-text query) to an apparent (alt, az) plus a
//! human-readable info summary — into a host-neutral helper so the CLI
//! (`--goto`) and the desktop viewer (interactive prompt) reach feature
//! parity without each reimplementing the resolver.
//!
//! The web binding stays separate because it also has to emit JSON for the
//! browser UI; the numeric resolution here is the part worth sharing, and the
//! solar-system magnitude conventions are kept identical to `apps/web` so all
//! three hosts report the same numbers.

use anyhow::{anyhow, Result};
use astronomy::{
    apparent_moon_topocentric, apparent_planet_topocentric, apparent_sun_topocentric,
    equatorial_to_horizontal, lmst_radians, Observer, Planet,
};
use catalog::search::{named_star, SOLAR_SYSTEM_BODIES};
use catalog::{
    search as catalog_search, DeepSkyCatalog, DeepSkyId, DeepSkyObject, MessierCatalog,
    NgcBrightCatalog, SearchId, SearchKind,
};
use renderer::LocalView;

/// A resolved GoTo target: where the object is right now for a given observer,
/// plus the descriptive fields a host shows in its info panel / console.
#[derive(Debug, Clone)]
pub struct GotoTarget {
    /// Encoded [`SearchId`] (kebab form), stable for round-tripping.
    pub id: String,
    pub kind: SearchKind,
    /// Primary human-readable label (e.g. `"Vega"`, `"M31"`, `"Saturn"`).
    pub display: String,
    /// Secondary identifiers / aliases (e.g. `"Alp Lyr"`, `"土星"`).
    pub aka: String,
    pub right_ascension_rad: f64,
    pub declination_rad: f64,
    /// Apparent topocentric altitude above the horizon, radians.
    pub altitude_rad: f64,
    /// Apparent topocentric azimuth, radians from North toward East.
    pub azimuth_rad: f64,
    /// Apparent magnitude when known. Solar-system bodies that vary with phase
    /// expose their ephemeris-computed value; the Moon is left `None` because
    /// its visual magnitude is strongly phase-dependent (matches `apps/web`).
    pub magnitude: Option<f64>,
    /// Distance with its unit (`"pc"`, `"AU"`, or `"km"`), when known.
    pub distance: Option<(f64, &'static str)>,
}

impl GotoTarget {
    /// Build a [`LocalView`] centred on this target at the requested vertical
    /// field of view. The view is clamped to the renderer's valid range.
    pub fn local_view(&self, fov_y_rad: f32) -> LocalView {
        LocalView {
            azimuth_rad: self.azimuth_rad as f32,
            altitude_rad: self.altitude_rad as f32,
            fov_y_rad,
        }
        .clamped()
    }

    /// One-line, deterministic info summary for a console / title-bar panel.
    pub fn info_summary(&self) -> String {
        let mut parts = vec![self.display.clone()];
        if !self.aka.is_empty() && self.aka != self.display {
            parts.push(format!("({})", self.aka));
        }
        parts.push(self.kind.label().to_string());
        if let Some(mag) = self.magnitude {
            parts.push(format!("mag {mag:.2}"));
        }
        parts.push(format!(
            "RA {} Dec {}",
            format_ra(self.right_ascension_rad),
            format_dec(self.declination_rad)
        ));
        parts.push(format!(
            "alt {:.1}° az {:.1}°",
            self.altitude_rad.to_degrees(),
            self.azimuth_rad.to_degrees()
        ));
        if let Some((value, unit)) = self.distance {
            parts.push(format!("{value:.3} {unit}"));
        }
        parts.join(" · ")
    }
}

/// Resolve a free-text query (the same grammar [`catalog::search`] accepts) to
/// the best-ranked target for `observer`. Errors when the query matches no
/// known object, so hosts can surface a clear "not found" message.
pub fn resolve_goto_query(query: &str, observer: Observer) -> Result<GotoTarget> {
    let hits = catalog_search(query, 1);
    let top = hits
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no catalog object matches \"{}\"", query.trim()))?;
    resolve_goto_id(top.id.clone(), observer).ok_or_else(|| {
        anyhow!(
            "failed to resolve apparent position for \"{}\"",
            query.trim()
        )
    })
}

/// Resolve an already-chosen [`SearchId`] to its apparent position for
/// `observer`. Returns `None` for ids that no longer map to a catalog row.
pub fn resolve_goto_id(id: SearchId, observer: Observer) -> Option<GotoTarget> {
    let fields: ResolvedFields = match &id {
        SearchId::NamedStar(idx) => {
            let star = named_star(SearchId::NamedStar(*idx))?;
            let aka = star
                .bayer
                .as_deref()
                .zip(star.constellation.as_deref())
                .map(|(b, c)| format!("{b} {c}"))
                .unwrap_or_default();
            ResolvedFields {
                kind: SearchKind::Star,
                display: star.display(),
                aka,
                right_ascension_rad: star.right_ascension_rad,
                declination_rad: star.declination_rad,
                magnitude: Some(star.magnitude as f64),
                distance: if star.distance_pc > 0.0 {
                    Some((star.distance_pc as f64, "pc"))
                } else {
                    None
                },
            }
        }
        SearchId::Messier(n) => {
            let object = MessierCatalog
                .objects(99.0)
                .into_iter()
                .find(|o| o.id == DeepSkyId::Messier(*n))?;
            deepsky_fields(&id, object)
        }
        SearchId::Ngc(n) => {
            let object = NgcBrightCatalog
                .objects(99.0)
                .into_iter()
                .find(|o| o.id == DeepSkyId::Ngc(*n))?;
            deepsky_fields(&id, object)
        }
        SearchId::Ic(n) => {
            let object = NgcBrightCatalog
                .objects(99.0)
                .into_iter()
                .find(|o| o.id == DeepSkyId::Ic(*n))?;
            deepsky_fields(&id, object)
        }
        SearchId::SolarSystem(name) => {
            let body = SOLAR_SYSTEM_BODIES.iter().find(|b| b.canonical == *name)?;
            let (ra, dec, mag, dist) = match *name {
                "sun" => {
                    let sun = apparent_sun_topocentric(observer);
                    (
                        sun.right_ascension_rad,
                        sun.declination_rad,
                        Some(-26.74_f64),
                        (sun.distance_au, "AU"),
                    )
                }
                "moon" => {
                    let moon = apparent_moon_topocentric(observer);
                    (
                        moon.right_ascension_rad,
                        moon.declination_rad,
                        None,
                        (moon.distance_km, "km"),
                    )
                }
                other => {
                    let planet = match other {
                        "mercury" => Planet::Mercury,
                        "venus" => Planet::Venus,
                        "mars" => Planet::Mars,
                        "jupiter" => Planet::Jupiter,
                        "saturn" => Planet::Saturn,
                        "uranus" => Planet::Uranus,
                        "neptune" => Planet::Neptune,
                        _ => return None,
                    };
                    let p = apparent_planet_topocentric(observer, planet);
                    (
                        p.right_ascension_rad,
                        p.declination_rad,
                        Some(p.magnitude),
                        (p.distance_au, "AU"),
                    )
                }
            };
            ResolvedFields {
                kind: SearchKind::SolarSystem,
                display: body.display_en.to_string(),
                aka: body.display_ja.to_string(),
                right_ascension_rad: ra,
                declination_rad: dec,
                magnitude: mag,
                distance: Some(dist),
            }
        }
    };

    let lst = lmst_radians(observer.time.jd_ut1, observer.longitude_rad);
    let altaz = equatorial_to_horizontal(
        fields.right_ascension_rad,
        fields.declination_rad,
        lst,
        observer.latitude_rad,
    );

    Some(GotoTarget {
        id: id.encode(),
        kind: fields.kind,
        display: fields.display,
        aka: fields.aka,
        right_ascension_rad: fields.right_ascension_rad,
        declination_rad: fields.declination_rad,
        altitude_rad: altaz.altitude,
        azimuth_rad: altaz.azimuth,
        magnitude: fields.magnitude,
        distance: fields.distance,
    })
}

/// Intermediate catalog fields resolved before the apparent (alt, az) is
/// computed. Keeps [`resolve_goto_id`] readable (and clippy's
/// `type_complexity` lint happy) by avoiding a wide tuple return.
struct ResolvedFields {
    kind: SearchKind,
    display: String,
    aka: String,
    right_ascension_rad: f64,
    declination_rad: f64,
    magnitude: Option<f64>,
    distance: Option<(f64, &'static str)>,
}

fn deepsky_fields(id: &SearchId, object: DeepSkyObject) -> ResolvedFields {
    let position = object.position;
    let x = position[0] as f64;
    let y = position[1] as f64;
    let z = position[2] as f64;
    let len = (x * x + y * y + z * z).sqrt().max(1e-12);
    let ra = y.atan2(x).rem_euclid(std::f64::consts::TAU);
    let dec = (z / len).clamp(-1.0, 1.0).asin();
    let kind = match id {
        SearchId::Messier(_) => SearchKind::Messier,
        SearchId::Ic(_) => SearchKind::Ic,
        _ => SearchKind::Ngc,
    };
    let label = match object.id {
        DeepSkyId::Messier(n) => format!("M{n}"),
        DeepSkyId::Ngc(n) => format!("NGC {n}"),
        DeepSkyId::Ic(n) => format!("IC {n}"),
    };
    ResolvedFields {
        kind,
        display: label.clone(),
        aka: label,
        right_ascension_rad: ra,
        declination_rad: dec,
        magnitude: if object.magnitude < 90.0 {
            Some(object.magnitude as f64)
        } else {
            None
        },
        distance: None,
    }
}

/// Format right ascension (radians) as `HHhMMm`.
fn format_ra(ra_rad: f64) -> String {
    let hours_total = ra_rad.rem_euclid(std::f64::consts::TAU) * 12.0 / std::f64::consts::PI;
    let mut h = hours_total.floor() as i64;
    let mut m = ((hours_total - h as f64) * 60.0).round() as i64;
    if m >= 60 {
        m -= 60;
        h += 1;
    }
    h = h.rem_euclid(24);
    format!("{h:02}h{m:02}m")
}

/// Format declination (radians) as `±DD°MM′`.
fn format_dec(dec_rad: f64) -> String {
    let deg_total = dec_rad.to_degrees();
    let sign = if deg_total < 0.0 { '-' } else { '+' };
    let abs = deg_total.abs();
    let mut d = abs.floor() as i64;
    let mut m = ((abs - d as f64) * 60.0).round() as i64;
    if m >= 60 {
        m -= 60;
        d += 1;
    }
    format!("{sign}{d:02}°{m:02}′")
}

#[cfg(test)]
mod tests {
    use super::*;
    use astronomy::TimeScales;

    /// A fixed, reproducible observer: Tokyo at 2026-04-26T12:00:00Z.
    fn tokyo() -> Observer {
        // 2026-04-26T12:00:00Z ≈ JD(UTC) 2461157.0.
        let time = TimeScales::from_utc_julian_date(2_461_157.0);
        Observer::from_degrees_with_time(35.68, 139.69, time)
    }

    #[test]
    fn query_resolves_named_star() {
        let target = resolve_goto_query("Vega", tokyo()).expect("Vega resolves");
        assert_eq!(target.kind, SearchKind::Star);
        assert!(
            target.display.contains("Vega"),
            "display={}",
            target.display
        );
        assert!(target.id.starts_with("star:"), "id={}", target.id);
        // Vega is bright (V ≈ 0.03).
        let mag = target.magnitude.expect("Vega has photometry");
        assert!(mag < 0.2, "Vega mag={mag}");
        // Apparent position must be finite and within range.
        assert!(target.altitude_rad.is_finite());
        assert!(target.altitude_rad.abs() <= std::f64::consts::FRAC_PI_2 + 1e-9);
        assert!((0.0..std::f64::consts::TAU).contains(&target.azimuth_rad));
    }

    #[test]
    fn query_resolves_messier() {
        let target = resolve_goto_query("M31", tokyo()).expect("M31 resolves");
        assert_eq!(target.kind, SearchKind::Messier);
        assert_eq!(target.id, "m:31");
    }

    #[test]
    fn query_resolves_planet() {
        let target = resolve_goto_query("Saturn", tokyo()).expect("Saturn resolves");
        assert_eq!(target.kind, SearchKind::SolarSystem);
        assert_eq!(target.id, "ss:saturn");
        // Planet magnitude is computed from the ephemeris, so it is present.
        assert!(target.magnitude.is_some());
        assert!(matches!(target.distance, Some((_, "AU"))));
    }

    #[test]
    fn query_resolves_japanese_alias() {
        let target = resolve_goto_query("土星", tokyo()).expect("土星 resolves");
        assert_eq!(target.id, "ss:saturn");
    }

    #[test]
    fn unknown_query_errors() {
        let err = resolve_goto_query("definitely-not-a-real-object", tokyo());
        assert!(err.is_err());
    }

    #[test]
    fn empty_query_errors() {
        assert!(resolve_goto_query("   ", tokyo()).is_err());
    }

    #[test]
    fn local_view_centres_on_target() {
        let target = resolve_goto_query("Vega", tokyo()).expect("Vega resolves");
        let fov = 45.0_f32.to_radians();
        let view = target.local_view(fov);
        assert!((view.azimuth_rad - target.azimuth_rad as f32).abs() < 1e-4);
        assert!((view.altitude_rad - target.altitude_rad as f32).abs() < 1e-4);
    }

    #[test]
    fn info_summary_is_nonempty_and_structured() {
        let target = resolve_goto_query("Sirius", tokyo()).expect("Sirius resolves");
        let summary = target.info_summary();
        assert!(summary.contains("Sirius"), "summary={summary}");
        assert!(summary.contains("mag"), "summary={summary}");
        assert!(summary.contains("RA"), "summary={summary}");
        assert!(summary.contains("alt"), "summary={summary}");
    }

    #[test]
    fn ra_dec_formatting_bounds() {
        assert_eq!(format_ra(0.0), "00h00m");
        // 12h = π radians.
        assert_eq!(format_ra(std::f64::consts::PI), "12h00m");
        assert_eq!(format_dec(0.0), "+00°00′");
        assert_eq!(format_dec(std::f64::consts::FRAC_PI_2), "+90°00′");
        assert_eq!(format_dec(-std::f64::consts::FRAC_PI_2), "-90°00′");
    }
}
