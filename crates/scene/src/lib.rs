//! Platform-neutral catalog → renderer integration.
//!
//! Catalog storage, identifiers, and policy remain in `catalog`; GPU-facing
//! DTOs remain in `renderer`. Native and WASM hosts share these adapters.

use catalog::{
    render_magnitude_at, CatalogLabelKind, CatalogObjectId, DeepSkyCatalog, DeepSkyId,
    MessierCatalog, NgcBrightCatalog, Star, CONSTELLATION_LABELS, DEEP_SKY_LABELS, STAR_LABELS,
};
use renderer::{
    build_star_instance, DeepSkyMarker, DeepSkyMarkerShape, SkyLabel, SkyLabelKind, StarInstance,
};

pub fn deep_sky_markers() -> Vec<DeepSkyMarker> {
    let ngc = NgcBrightCatalog;
    let messier = MessierCatalog;
    ngc.objects(f32::INFINITY)
        .into_iter()
        .filter(|object| !ngc.resolve_as_member_field(object.id))
        .chain(
            messier
                .objects(f32::INFINITY)
                .into_iter()
                .filter(|object| !messier.resolve_as_member_field(object.id)),
        )
        .map(|object| DeepSkyMarker {
            position: object.position,
            magnitude: object.magnitude,
            size_arcmin: object.size_arcmin,
            shape: match object.id {
                DeepSkyId::Messier(_) => DeepSkyMarkerShape::Diamond,
                DeepSkyId::Ngc(_) | DeepSkyId::Ic(_) => DeepSkyMarkerShape::Ring,
            },
        })
        .collect()
}

pub fn sky_labels() -> Vec<SkyLabel> {
    STAR_LABELS
        .iter()
        .chain(CONSTELLATION_LABELS)
        .chain(DEEP_SKY_LABELS)
        .map(|label| SkyLabel {
            position: label.position,
            text: label.text.to_owned(),
            magnitude: label.magnitude,
            kind: match label.kind {
                CatalogLabelKind::Star => SkyLabelKind::Star,
                CatalogLabelKind::Constellation => SkyLabelKind::Constellation,
                CatalogLabelKind::Messier => SkyLabelKind::Messier,
                CatalogLabelKind::Ngc => SkyLabelKind::Ngc,
            },
        })
        .collect()
}

/// Build renderer instances and the index-aligned host identity sidecar.
pub fn star_instances(
    stars: &[Star],
    limiting_magnitude: f32,
    variable_jd: Option<f64>,
) -> (Vec<StarInstance>, Vec<Option<CatalogObjectId>>) {
    let instances = stars
        .iter()
        .map(|star| {
            let magnitude = variable_jd.map_or(star.magnitude, |jd| {
                render_magnitude_at(
                    star.identifiers.hip,
                    star.identifiers.hd,
                    None,
                    star.magnitude,
                    jd,
                )
            });
            build_star_instance(
                star.position.into(),
                star.proper_motion.into(),
                star.color,
                magnitude,
                limiting_magnitude,
                star.distance_pc,
            )
        })
        .collect();
    let identities = stars
        .iter()
        .map(|star| star.identifiers.resolved_primary())
        .collect();
    (instances, identities)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapters_keep_catalog_policy_out_of_renderer() {
        assert!(!deep_sky_markers().is_empty());
        assert!(sky_labels()
            .iter()
            .any(|label| label.text.contains("Sirius")));
    }
}
