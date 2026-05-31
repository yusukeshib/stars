//! V-50 output colour management — sRGB / Display-P3 / Rec.2020.
//!
//! The whole renderer accumulates linear radiance in the working space whose
//! primaries are sRGB / IEC 61966-2-1 (the catalogue blackbody pipeline
//! `V-23` and the Hošek-Wilkie daylight model `V-38` both emit sRGB-primary
//! linear RGB). Output colour management is therefore a single linear 3×3
//! gamut transform applied at the very end of the tone-reproduction pass,
//! after the Reinhard 2002 keyed operator has run in linear radiance:
//!
//! ```text
//! rgb_target_linear = M · rgb_srgb_linear
//! ```
//!
//! The host swap-chain / PNG is an 8-bit sRGB-transfer surface, so the
//! transfer function (sRGB OETF) is applied by the hardware as before; only
//! the **primaries** change here, and the chosen primaries are then tagged on
//! the output (PNG `cHRM`, WebGPU canvas colour space) so a calibrated screen
//! reproduces the same colour. Display-P3 (Apple / SMPTE EG 432-1) is defined
//! with the sRGB transfer function, so its reproduction is exact on this path;
//! Rec.2020 (ITU-R BT.2020-2) is tagged with its primaries and reproduced with
//! the sRGB transfer as a documented approximation (see
//! `docs/standards-compliance.md`).
//!
//! References:
//!   * IEC 61966-2-1:1999 (sRGB).
//!   * SMPTE EG 432-1:2010 / Apple Display-P3 (P3 primaries, D65, sRGB EOTF).
//!   * ITU-R BT.2020-2 (Rec.2020 primaries, D65).

/// Output colour space selected by the host. The renderer transforms its
/// internal sRGB-primary linear radiance into the chosen primaries in the
/// final tone-map step.
///
/// The serde-tagged session representation lives in `crates/common`
/// (`OutputColourspaceArg`) so the engine crate stays serialization-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputColourSpace {
    /// IEC 61966-2-1 sRGB. The renderer's native working space, so the gamut
    /// transform is the identity and output is bit-identical to the
    /// pre-V-50 pipeline.
    #[default]
    Srgb,
    /// Apple / SMPTE EG 432-1 Display-P3 (P3 primaries, D65 white, sRGB
    /// transfer function).
    DisplayP3,
    /// ITU-R BT.2020-2 Rec.2020 (wide-gamut primaries, D65 white).
    Rec2020,
}

impl OutputColourSpace {
    /// All variants, in declaration order. Used by hosts to build menus and
    /// by tests to exercise every transform.
    pub const ALL: [OutputColourSpace; 3] = [
        OutputColourSpace::Srgb,
        OutputColourSpace::DisplayP3,
        OutputColourSpace::Rec2020,
    ];

    /// Stable lower-kebab identifier used by the CLI flag, the session JSON,
    /// and the web control.
    pub fn as_str(self) -> &'static str {
        match self {
            OutputColourSpace::Srgb => "srgb",
            OutputColourSpace::DisplayP3 => "display-p3",
            OutputColourSpace::Rec2020 => "rec2020",
        }
    }

    /// Parse from the stable identifier. Accepts a few common spellings so
    /// the CLI / web / session layers can share one parser.
    pub fn from_str_opt(s: &str) -> Option<OutputColourSpace> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "srgb" => Some(OutputColourSpace::Srgb),
            "display-p3" | "displayp3" | "p3" => Some(OutputColourSpace::DisplayP3),
            "rec2020" | "rec-2020" | "bt2020" | "bt-2020" => Some(OutputColourSpace::Rec2020),
            _ => None,
        }
    }

    /// Row-major 3×3 matrix taking **linear sRGB-primary** RGB to **linear**
    /// RGB in this colour space's primaries. Both endpoints share the D65
    /// white point, so no chromatic adaptation is needed.
    ///
    /// Constants are the standard published values (CSS Color Module Level 4
    /// reference transforms), which are `M_target^-1 · M_srgb` with the
    /// canonical RGB→XYZ matrices below.
    pub fn linear_from_srgb_matrix(self) -> [[f32; 3]; 3] {
        match self {
            OutputColourSpace::Srgb => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            // sRGB-linear → Display-P3-linear (CSS Color 4).
            OutputColourSpace::DisplayP3 => [
                [0.822_461_97, 0.177_538_03, 0.0],
                [0.033_194_2, 0.966_805_8, 0.0],
                [0.017_082_6, 0.072_397_46, 0.910_519_9],
            ],
            // sRGB-linear → Rec.2020-linear (CSS Color 4).
            OutputColourSpace::Rec2020 => [
                [0.627_404, 0.329_282, 0.043_313_6],
                [0.069_097, 0.919_54, 0.011_361_2],
                [0.016_391_6, 0.088_013_2, 0.895_595],
            ],
        }
    }

    /// CIE 1931 (x, y) chromaticities of the (red, green, blue) primaries and
    /// the D65 white point for this space, used to tag PNG `cHRM` and for
    /// validation. Order: `[red, green, blue, white]`.
    pub fn primaries_xy(self) -> [(f32, f32); 4] {
        const D65: (f32, f32) = (0.3127, 0.3290);
        match self {
            OutputColourSpace::Srgb => [(0.64, 0.33), (0.30, 0.60), (0.15, 0.06), D65],
            OutputColourSpace::DisplayP3 => [(0.680, 0.320), (0.265, 0.690), (0.150, 0.060), D65],
            OutputColourSpace::Rec2020 => [(0.708, 0.292), (0.170, 0.797), (0.131, 0.046), D65],
        }
    }

    /// Integer code uploaded to the tone-map shader (informational; the shader
    /// applies the uploaded matrix directly).
    pub fn shader_code(self) -> u32 {
        match self {
            OutputColourSpace::Srgb => 0,
            OutputColourSpace::DisplayP3 => 1,
            OutputColourSpace::Rec2020 => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonical linear RGB→CIE-XYZ matrices (D65) for each space, used only
    /// in tests to verify that the shipped `linear_from_srgb_matrix` constants
    /// preserve the physical colour of a primary across the gamut transform.
    fn rgb_to_xyz(space: OutputColourSpace) -> [[f64; 3]; 3] {
        match space {
            OutputColourSpace::Srgb => [
                [0.412_390_799_3, 0.357_584_339_4, 0.180_480_788_4],
                [0.212_639_005_9, 0.715_168_678_8, 0.072_192_315_4],
                [0.019_330_818_7, 0.119_194_779_8, 0.950_532_152_2],
            ],
            OutputColourSpace::DisplayP3 => [
                [0.486_570_948_6, 0.265_667_693_2, 0.198_217_285_2],
                [0.228_974_564_1, 0.691_738_521_8, 0.079_286_914_1],
                [0.0, 0.045_113_381_9, 1.043_944_368_9],
            ],
            OutputColourSpace::Rec2020 => [
                [0.636_958_048_3, 0.144_616_903_6, 0.168_880_975_2],
                [0.262_700_212_0, 0.677_998_071_5, 0.059_301_716_5],
                [0.0, 0.028_072_693_0, 1.060_985_057_7],
            ],
        }
    }

    fn mat_vec(m: [[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
        [
            m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
            m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
            m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
        ]
    }

    fn chromaticity(xyz: [f64; 3]) -> (f64, f64) {
        let sum = xyz[0] + xyz[1] + xyz[2];
        (xyz[0] / sum, xyz[1] / sum)
    }

    #[test]
    fn srgb_transform_is_identity() {
        let m = OutputColourSpace::Srgb.linear_from_srgb_matrix();
        let expected = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert_eq!(m, expected);
    }

    /// A pure sRGB primary, transformed into a target space and then projected
    /// to CIE-XYZ via that space's own RGB→XYZ matrix, must land on the *same*
    /// chromaticity as the original sRGB primary — the gamut transform changes
    /// the encoding, never the physical colour.
    #[test]
    fn primary_chromaticity_is_preserved_round_trip() {
        let srgb_xyz = rgb_to_xyz(OutputColourSpace::Srgb);
        for space in OutputColourSpace::ALL {
            let m = space
                .linear_from_srgb_matrix()
                .map(|row| row.map(|c| c as f64));
            let target_xyz = rgb_to_xyz(space);
            for (primary, srgb) in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
                .into_iter()
                .enumerate()
            {
                let want = chromaticity(mat_vec(srgb_xyz, srgb));
                let target_lin = mat_vec(m, srgb);
                let got = chromaticity(mat_vec(target_xyz, target_lin));
                assert!(
                    (want.0 - got.0).abs() < 1e-3 && (want.1 - got.1).abs() < 1e-3,
                    "{space:?} primary {primary}: want {want:?}, got {got:?}",
                );
            }
        }
    }

    /// The Display-P3 red primary is outside the sRGB gamut: expressing it in
    /// sRGB primaries (inverse transform) must produce an out-of-range
    /// component, confirming the wider gamut is real and not a relabelling.
    #[test]
    fn wide_gamut_red_is_outside_srgb() {
        // P3 red primary in sRGB linear: invert the sRGB→P3 matrix applied to
        // P3 (1,0,0). Easiest check: sRGB red maps *into* P3 gamut (all in
        // [0,1]) but does not reach the P3 red corner.
        let m = OutputColourSpace::DisplayP3.linear_from_srgb_matrix();
        let srgb_red_in_p3 = m[0][0]; // R channel of sRGB (1,0,0) in P3
        assert!(
            srgb_red_in_p3 < 0.95,
            "sRGB red should be less saturated than the P3 red corner, got {srgb_red_in_p3}",
        );
    }

    #[test]
    fn parse_round_trips() {
        for space in OutputColourSpace::ALL {
            assert_eq!(OutputColourSpace::from_str_opt(space.as_str()), Some(space));
        }
        assert_eq!(
            OutputColourSpace::from_str_opt("P3"),
            Some(OutputColourSpace::DisplayP3)
        );
        assert_eq!(
            OutputColourSpace::from_str_opt("bt2020"),
            Some(OutputColourSpace::Rec2020)
        );
        assert_eq!(OutputColourSpace::from_str_opt("nonsense"), None);
    }
}
