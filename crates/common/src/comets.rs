//! V-49 host-tier comet helpers: the embedded curated osculating-element
//! snapshot and the [`renderer::CometLayer`] builders the CLI / viewer / web
//! hosts share.
//!
//! The default render path ships a deterministic, manifest-pinned element
//! snapshot (`crates/common/data/comets/elements.csv`) of historic and current
//! bright comets (Halley, Hale-Bopp, C/2023 A3). Provenance is recorded in
//! `DATA_SOURCES.md` and `data/manifest.toml`.

use astronomy::{parse_comet_elements, CometElements};
use renderer::CometLayer;

/// Curated, manifest-pinned comet osculating-element snapshot embedded at build
/// time. Provenance is recorded in `DATA_SOURCES.md` and `data/manifest.toml`
/// (id `jpl-sbdb-comet-elements-2025-01`).
pub const CURATED_COMET_TEXT: &str = include_str!("../data/comets/elements.csv");

/// Parse the embedded curated comet snapshot into [`CometElements`].
pub fn curated_comet_elements() -> Vec<CometElements> {
    parse_comet_elements(CURATED_COMET_TEXT)
}

/// Build a [`CometLayer`] from the curated snapshot.
pub fn curated_comet_layer(enabled: bool) -> CometLayer {
    CometLayer {
        enabled,
        comets: if enabled {
            curated_comet_elements()
        } else {
            Vec::new()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curated_snapshot_parses_to_several_comets() {
        let comets = curated_comet_elements();
        assert!(
            comets.len() >= 3,
            "expected the curated snapshot to carry >= 3 comets, got {}",
            comets.len()
        );
        assert!(comets.iter().any(|c| c.name.contains("Halley")));
    }

    #[test]
    fn disabled_layer_carries_no_comets() {
        let layer = curated_comet_layer(false);
        assert!(!layer.enabled);
        assert!(layer.comets.is_empty());
    }

    #[test]
    fn enabled_layer_loads_curated_comets() {
        let layer = curated_comet_layer(true);
        assert!(layer.enabled);
        assert!(!layer.comets.is_empty());
    }
}
