//! Compact, catalog-owned labels that hosts can adapt into renderer DTOs.
//!
//! Keeping these records here prevents the renderer build from reading the
//! catalog crate's private source files through undeclared relative paths.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogLabelKind {
    Star,
    Constellation,
    Messier,
    Ngc,
}

#[derive(Debug, Clone, Copy)]
pub struct CatalogLabel {
    pub position: [f32; 3],
    pub text: &'static str,
    pub magnitude: f32,
    pub kind: CatalogLabelKind,
}

#[allow(clippy::approx_constant)]
mod generated {
    use super::{CatalogLabel, CatalogLabelKind};
    include!(concat!(env!("OUT_DIR"), "/label_data.rs"));
}

pub use generated::{CONSTELLATION_LABELS, DEEP_SKY_LABELS, STAR_LABELS};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_label_catalogs_are_populated() {
        assert_eq!(STAR_LABELS.len(), 50);
        assert!(CONSTELLATION_LABELS.len() >= 80);
        assert!(DEEP_SKY_LABELS.len() >= 1_000);
        assert!(STAR_LABELS.iter().all(|label| label.magnitude.is_finite()));
        assert!(DEEP_SKY_LABELS
            .iter()
            .all(|label| label.position.iter().all(|v| v.is_finite())));
    }
}
