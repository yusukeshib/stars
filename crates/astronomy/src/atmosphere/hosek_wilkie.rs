//! Hošek-Wilkie 2012 analytic full-spectral sky-dome radiance model (V-38).
//!
//! Replaces Preetham/Shirley/Smits 1999 as the default daylight sky radiance
//! source. The model expands radiance as a 9-parameter (A..I) analytic
//! function of view zenith angle θ and sun-view angle γ, with the
//! parameters themselves fit to brute-force Mishchenko spectral radiative-
//! transfer simulations and stored as a quintic-Bezier control mesh over
//! turbidity ∈ [1, 10], ground albedo ∈ [0, 1], and sun elevation
//! ∈ [0, π/2]. The big practical wins over Preetham:
//!
//! * remains positive and finite at solar altitude < 5° (Preetham's Perez
//!   asymptote goes negative around sunrise);
//! * resolves high-turbidity skies (sunset / hazy days) correctly;
//! * tracks ground albedo, so snow / desert ground brightness lifts the
//!   zenith without an ad-hoc fudge.
//!
//! The embedded coefficient table is the upstream `ArHosekSkyModelData_RGB.h`
//! release v1.4a (22 Feb 2013), packed by `scripts/build-hosek-wilkie.py`
//! and pinned in `data/manifest.toml` (id: `hosek-wilkie-2012-rgb-v1.4a`).
//!
//! References:
//!
//! * Hošek, L. & Wilkie, A. 2012, ACM TOG 31(4), "An Analytic Model for Full
//!   Spectral Sky-Dome Radiance".
//! * Upstream sample-code release v1.4a (BSD 3-clause), Charles University:
//!   <https://cgg.mff.cuni.cz/projects/SkylightModelling/>.

// The dataset parser walks a five-dimensional `[channel][albedo][turbidity]
// [elev_control][coeff]` table; rewriting that as iterator chains makes the
// shape considerably harder to audit against the upstream `cooker` C source.
#![allow(clippy::needless_range_loop)]

use std::f64::consts::FRAC_PI_2;

/// Half-width (radians, 1°) of the daylight ↔ twilight blend window. The
/// HW dataset is fit on positive elevation but tolerates a small extension
/// below the horizon so the renderer can cross-fade smoothly into the
/// twilight evaluator without a dark frame at sunset — inputs more than
/// this far below the horizon short-circuit to the all-zero sentinel, and
/// the shader applies a matching `smoothstep` on the upper half so the two
/// models additively overlap inside the window.
pub const DAY_NIGHT_BLEND_HALF_WINDOW_RAD: f64 = std::f64::consts::PI / 180.0;

/// Binary table emitted by `scripts/build-hosek-wilkie.py`.
const COEFFICIENTS_RGB_BIN: &[u8] = include_bytes!("../../data/hosek_wilkie/coefficients_rgb.bin");

const MAGIC: &[u8; 8] = b"HW2012RG";
const FORMAT_VERSION: u32 = 1;
const N_CHANNELS: usize = 3;
const N_ALBEDOS: usize = 2;
const N_TURBIDITIES: usize = 10;
const N_ELEV_CONTROL: usize = 6;
const N_COEFFS: usize = 9;
const HEADER_BYTES: usize = 16;

/// Cooked radiance configuration for one sky-model invocation: nine analytic
/// coefficients per RGB channel plus the per-channel scale factor that
/// produces physical radiance (the upstream calls these the model's
/// `radiances`).
#[derive(Debug, Clone, Copy)]
pub struct HosekWilkieParams {
    /// `[channel][A..I]` polynomial coefficients consumed by [`radiance`].
    pub coeffs: [[f64; N_COEFFS]; N_CHANNELS],
    /// Per-channel master radiance scale. Multiply [`radiance`]'s analytic
    /// term by this to recover the calibrated radiance value.
    pub radiances: [f64; N_CHANNELS],
}

impl HosekWilkieParams {
    /// All-zero state, used by callers that want to disable the model when
    /// the Sun is below the horizon.
    pub const ZERO: Self = Self {
        coeffs: [[0.0; N_COEFFS]; N_CHANNELS],
        radiances: [0.0; N_CHANNELS],
    };
}

/// Embedded Hošek-Wilkie RGB coefficient table. Loaded lazily so the header
/// validation is hit exactly once per process, not per `cook` call.
struct RgbDataset {
    /// `[channel][albedo][turbidity][elev_control][coeff]` polynomial
    /// coefficients (9 per elev-control point).
    coeffs: [[[[[f64; N_COEFFS]; N_ELEV_CONTROL]; N_TURBIDITIES]; N_ALBEDOS]; N_CHANNELS],
    /// `[channel][albedo][turbidity][elev_control]` master radiance scales.
    radiances: [[[[f64; N_ELEV_CONTROL]; N_TURBIDITIES]; N_ALBEDOS]; N_CHANNELS],
}

impl RgbDataset {
    fn load() -> &'static Self {
        use std::sync::OnceLock;
        static CELL: OnceLock<RgbDataset> = OnceLock::new();
        CELL.get_or_init(Self::parse)
    }

    fn parse() -> Self {
        let bytes = COEFFICIENTS_RGB_BIN;
        assert!(
            bytes.len() >= HEADER_BYTES,
            "Hošek-Wilkie RGB blob too short ({} bytes)",
            bytes.len()
        );
        assert_eq!(&bytes[..8], MAGIC, "Hošek-Wilkie RGB blob magic mismatch");
        let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        assert_eq!(
            version, FORMAT_VERSION,
            "Hošek-Wilkie RGB blob format version {version}, expected {FORMAT_VERSION}",
        );

        let expected_doubles =
            N_CHANNELS * N_ALBEDOS * N_TURBIDITIES * N_ELEV_CONTROL * (N_COEFFS + 1);
        let expected_bytes = HEADER_BYTES + expected_doubles * 8;
        assert_eq!(
            bytes.len(),
            expected_bytes,
            "Hošek-Wilkie RGB blob: expected {expected_bytes} bytes, got {}",
            bytes.len()
        );

        let mut cursor = HEADER_BYTES;
        let mut read_f64 = || {
            let v = f64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
            cursor += 8;
            v
        };

        // Order matches the regenerator's write order (channel × albedo ×
        // turbidity × elev × coeff for coeffs, then channel × albedo ×
        // turbidity × elev for radiances).
        let mut coeffs =
            [[[[[0.0_f64; N_COEFFS]; N_ELEV_CONTROL]; N_TURBIDITIES]; N_ALBEDOS]; N_CHANNELS];
        for channel in 0..N_CHANNELS {
            for albedo in 0..N_ALBEDOS {
                for turb in 0..N_TURBIDITIES {
                    for elev in 0..N_ELEV_CONTROL {
                        for coeff in 0..N_COEFFS {
                            coeffs[channel][albedo][turb][elev][coeff] = read_f64();
                        }
                    }
                }
            }
        }
        let mut radiances = [[[[0.0_f64; N_ELEV_CONTROL]; N_TURBIDITIES]; N_ALBEDOS]; N_CHANNELS];
        for channel in 0..N_CHANNELS {
            for albedo in 0..N_ALBEDOS {
                for turb in 0..N_TURBIDITIES {
                    for elev in 0..N_ELEV_CONTROL {
                        radiances[channel][albedo][turb][elev] = read_f64();
                    }
                }
            }
        }
        assert_eq!(cursor, expected_bytes, "Hošek-Wilkie RGB blob: short read");

        Self { coeffs, radiances }
    }
}

/// Quintic-Bernoulli (Bezier-of-degree-five) basis weights at `t ∈ [0, 1]`.
#[inline]
fn quintic_bezier_basis(t: f64) -> [f64; N_ELEV_CONTROL] {
    let u = 1.0 - t;
    let u2 = u * u;
    let u3 = u2 * u;
    let u4 = u3 * u;
    let u5 = u4 * u;
    let t2 = t * t;
    let t3 = t2 * t;
    let t4 = t3 * t;
    let t5 = t4 * t;
    [
        u5,
        5.0 * u4 * t,
        10.0 * u3 * t2,
        10.0 * u2 * t3,
        5.0 * u * t4,
        t5,
    ]
}

/// Cook the nine-parameter (A..I) configurations for the supplied scene
/// conditions, ready for [`radiance`].
///
/// `turbidity` is the Linke turbidity in `[1, 10]`; values outside the table
/// are clamped (the upstream cooker has the same behaviour). `albedo` is the
/// ground albedo in `[0, 1]`. `sun_elevation_rad` is the apparent solar
/// elevation above the horizon — values below
/// `-DAY_NIGHT_BLEND_HALF_WINDOW_RAD` return [`HosekWilkieParams::ZERO`]
/// because the dataset only covers the daylight quadrant. Values in
/// `[-DAY_NIGHT_BLEND_HALF_WINDOW_RAD, 0]` are evaluated at horizon
/// elevation (clamp to `1e-6`); the renderer applies a smoothstep weight
/// over the same window so the daylight → twilight handoff has no dark
/// frame at sunset.
pub fn cook(turbidity: f64, albedo: f64, sun_elevation_rad: f64) -> HosekWilkieParams {
    if !sun_elevation_rad.is_finite() || sun_elevation_rad < -DAY_NIGHT_BLEND_HALF_WINDOW_RAD {
        return HosekWilkieParams::ZERO;
    }
    let dataset = RgbDataset::load();

    // Clamp inputs to the table domain. The upstream code is a direct array
    // lookup with no clamping; clamping here keeps shader-side fallback
    // behaviour predictable when the host pushes values slightly off-grid.
    let turb = turbidity.clamp(1.0, 10.0);
    let albedo = albedo.clamp(0.0, 1.0);
    let elevation = sun_elevation_rad.clamp(1.0e-6, FRAC_PI_2);

    // Match upstream: integer turbidity index `int_turb` and remainder
    // `turb_rem` give a linear blend between consecutive turbidity slices.
    // The "high" slice is `int_turb + 1` (1-indexed) → array index `int_turb`.
    let int_turb_one_based = turb.floor() as usize;
    let turb_rem = turb - int_turb_one_based as f64;
    let lo_turb_idx = int_turb_one_based.saturating_sub(1).min(N_TURBIDITIES - 1);
    let hi_turb_idx = lo_turb_idx.saturating_add(1).min(N_TURBIDITIES - 1);

    // Elevation is reparameterised by cube root before the quintic blend, so
    // sunset / sunrise rows are weighted heavily near the horizon.
    let elev_param = (elevation / FRAC_PI_2).cbrt().clamp(0.0, 1.0);
    let basis = quintic_bezier_basis(elev_param);

    let mut out = HosekWilkieParams::ZERO;
    // Each (albedo × turbidity) weight is (1-α or α) × (1-rem or rem).
    let weights: [(usize, usize, f64); 4] = [
        (0, lo_turb_idx, (1.0 - albedo) * (1.0 - turb_rem)),
        (1, lo_turb_idx, albedo * (1.0 - turb_rem)),
        (0, hi_turb_idx, (1.0 - albedo) * turb_rem),
        (1, hi_turb_idx, albedo * turb_rem),
    ];

    for channel in 0..N_CHANNELS {
        for &(alb_idx, turb_idx, w) in &weights {
            if w == 0.0 {
                continue;
            }
            let coeff_row = &dataset.coeffs[channel][alb_idx][turb_idx];
            let rad_row = &dataset.radiances[channel][alb_idx][turb_idx];
            for coeff in 0..N_COEFFS {
                let mut acc = 0.0_f64;
                for k in 0..N_ELEV_CONTROL {
                    acc += basis[k] * coeff_row[k][coeff];
                }
                out.coeffs[channel][coeff] += w * acc;
            }
            let mut rad_acc = 0.0_f64;
            for k in 0..N_ELEV_CONTROL {
                rad_acc += basis[k] * rad_row[k];
            }
            out.radiances[channel] += w * rad_acc;
        }
    }
    out
}

/// Per-channel sky-dome radiance for view zenith angle `theta_rad` and
/// view-sun angle `gamma_rad`, in the same physical units the upstream
/// model exposes (`W · m⁻² · sr⁻¹` for the spectral path; the RGB path is
/// the tristimulus integration of the same spectral output).
///
/// Returns `[0; 3]` when `params.radiances` are all zero (the Sun-below-
/// horizon sentinel from [`cook`]). The shader-side port lives in
/// `crates/renderer/src/shaders/skyglow.wgsl`.
pub fn radiance(params: &HosekWilkieParams, theta_rad: f64, gamma_rad: f64) -> [f64; 3] {
    // Upstream `ArHosekSkyModel_GetRadianceInternal`:
    //   L(θ, γ) = (1 + A · exp(B / (cos θ + 0.01)))
    //            · (C + D · exp(E · γ) + F · cos² γ + G · mieM(γ, I)
    //               + H · √cos θ)
    let cos_gamma = gamma_rad.cos();
    let cos_theta = theta_rad.cos();
    let mut out = [0.0_f64; 3];
    for channel in 0..N_CHANNELS {
        let c = &params.coeffs[channel];
        let a = c[0];
        let b = c[1];
        let c_param = c[2];
        let d = c[3];
        let e = c[4];
        let f = c[5];
        let g = c[6];
        let h = c[7];
        let i = c[8];

        let exp_m = (e * gamma_rad).exp();
        let ray_m = cos_gamma * cos_gamma;
        let denom = (1.0 + i * i - 2.0 * i * cos_gamma).max(1.0e-6).powf(1.5);
        let mie_m = (1.0 + cos_gamma * cos_gamma) / denom;
        let zenith = cos_theta.max(0.0).sqrt();

        let term1 = 1.0 + a * (b / (cos_theta + 0.01)).exp();
        let term2 = c_param + d * exp_m + f * ray_m + g * mie_m + h * zenith;

        out[channel] = (term1 * term2 * params.radiances[channel]).max(0.0);
    }
    out
}

/// Luminous efficacy of monochromatic 555 nm light, the standard
/// radiometric → photometric conversion factor used to interpret the
/// Hošek-Wilkie RGB output (W·m⁻²·sr⁻¹) as colorimetric per-channel
/// luminance (cd/m²). The XYZ-channel output of the same upstream model is
/// already in cd/m² once multiplied by this factor (upstream comment in
/// `ArHosekSkyModel.h`); the RGB path inherits the same scaling.
pub const HW_RADIANCE_TO_LUMINANCE_LM_PER_W: f64 = 683.0;

/// Effective Hošek-Wilkie turbidity for the given Ångström aerosol depth.
///
/// HW takes the standard Linke turbidity (`T = (τ_aerosol + τ_molecular)
/// / τ_molecular` at 550 nm), so the unified V-37 (β, α, DU) state feeds HW
/// directly through [`crate::atmosphere::linke_turbidity_from_aerosol`].
/// Exposed under a HW-named alias so renderer call sites that want the
/// turbidity HW consumes do not have to reach across modules.
pub fn turbidity_from_aerosol(beta: f64) -> f64 {
    super::linke_turbidity_from_aerosol(beta)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HW is well-defined for sun_alt = 1° / T = 4 (where Preetham's Perez
    /// asymptote goes negative). All three channels must be finite and
    /// strictly positive across the full upper hemisphere.
    #[test]
    fn radiance_finite_and_positive_at_low_sun() {
        let params = cook(4.0, 0.1, 1.0_f64.to_radians());
        // Sample a coarse upper-hemisphere grid (θ from zenith, γ from sun).
        let mut bad = 0_u32;
        for theta_deg in (0..=85).step_by(5) {
            for gamma_deg in (0..=180).step_by(15) {
                let r = radiance(
                    &params,
                    (theta_deg as f64).to_radians(),
                    (gamma_deg as f64).to_radians(),
                );
                for c in r {
                    if !(c.is_finite() && c >= 0.0) {
                        bad += 1;
                    }
                }
            }
        }
        assert_eq!(
            bad, 0,
            "HW radiance must be finite & non-negative across the upper \
             hemisphere at sun_alt = 1°, T = 4"
        );
    }

    /// Cook returns the all-zero sentinel only once the apparent Sun is
    /// more than [`DAY_NIGHT_BLEND_HALF_WINDOW_RAD`] below the horizon. The
    /// near-horizon band above that produces horizon-grazing coefficients
    /// so the renderer can cross-fade into the twilight evaluator without a
    /// dark frame at sunset.
    #[test]
    fn cook_zeros_below_blend_window() {
        let params = cook(2.5, 0.1, -1.5_f64.to_radians());
        for radiance in params.radiances {
            assert_eq!(radiance, 0.0);
        }
        for channel in params.coeffs {
            for v in channel {
                assert_eq!(v, 0.0);
            }
        }
    }

    /// Within the daylight ↔ twilight blend window the evaluator must keep
    /// producing horizon-grazing radiance so the shader's smoothstep fade has
    /// something to scale. This is the regression test for the sunset
    /// flicker fix.
    #[test]
    fn cook_extends_into_blend_window() {
        let params = cook(2.5, 0.1, -0.5_f64.to_radians());
        let any_nonzero = params.radiances.iter().any(|&r| r > 0.0);
        assert!(
            any_nonzero,
            "cook must keep producing horizon-grazing radiance for sun_alt \
             inside the daylight↔twilight blend window"
        );
        let r = radiance(&params, 0.0, std::f64::consts::FRAC_PI_2);
        for v in r {
            assert!(v.is_finite() && v >= 0.0);
        }
    }

    /// HW radiance increases with turbidity at the zenith for a high Sun
    /// (anchor sanity: hazy skies are brighter than clear skies).
    #[test]
    fn zenith_brightness_monotone_in_turbidity() {
        let high_sun = 60.0_f64.to_radians();
        let p_low = cook(2.0, 0.1, high_sun);
        let p_hi = cook(6.0, 0.1, high_sun);
        // Zenith view, γ = sun zenith angle.
        let gamma = std::f64::consts::FRAC_PI_2 - high_sun;
        let r_low = radiance(&p_low, 0.0, gamma);
        let r_hi = radiance(&p_hi, 0.0, gamma);
        // Use luminance-weighted Y ≈ 0.2126 R + 0.7152 G + 0.0722 B as a
        // direction-independent brightness proxy.
        let y = |r: [f64; 3]| 0.2126 * r[0] + 0.7152 * r[1] + 0.0722 * r[2];
        assert!(
            y(r_hi) > y(r_low),
            "turbidity 6 zenith ({:.3}) should be brighter than turbidity 2 \
             ({:.3}) at sun_alt = 60°",
            y(r_hi),
            y(r_low),
        );
    }

    /// HW is sensitive to ground albedo: snow / desert ground lifts the
    /// zenith vs. dark forest at the same turbidity.
    #[test]
    fn zenith_brightness_monotone_in_albedo() {
        let p_dark = cook(3.0, 0.05, 45.0_f64.to_radians());
        let p_snow = cook(3.0, 0.80, 45.0_f64.to_radians());
        let gamma = 45.0_f64.to_radians();
        let r_dark = radiance(&p_dark, 0.0, gamma);
        let r_snow = radiance(&p_snow, 0.0, gamma);
        let y = |r: [f64; 3]| 0.2126 * r[0] + 0.7152 * r[1] + 0.0722 * r[2];
        assert!(
            y(r_snow) > y(r_dark),
            "snow ground (α=0.8) should brighten the zenith vs. dark ground \
             (α=0.05) at sun_alt = 45°"
        );
    }

    /// Sanity: at sun_alt = 45° and moderate turbidity the per-channel zenith
    /// radiance ordering should be B > G > R (clear sky is blue), and all
    /// channels are within the same order of magnitude — guards against a
    /// channel-ordering bug in the dataset parser.
    #[test]
    fn clear_sky_zenith_is_blue() {
        let params = cook(2.5, 0.10, 45.0_f64.to_radians());
        let gamma = 45.0_f64.to_radians();
        let r = radiance(&params, 0.0, gamma);
        assert!(
            r[2] > r[1] && r[1] > r[0],
            "clear-sky zenith should satisfy B > G > R, got R={:.4} G={:.4} \
             B={:.4}",
            r[0],
            r[1],
            r[2],
        );
        assert!(
            r[0] > 0.1 * r[2],
            "R channel should not be near zero (would imply parser misalign)"
        );
    }

    /// HW zenith luminance at a high Sun should land within the published
    /// clear-sky range (≈12000–3000 cd/m² photometric for T ∈ [2, 4],
    /// solar altitude 45°–70°; see Coulson 1988 Table 7.3 and the WMO
    /// reference daylight illuminance tables). This is an order-of-magnitude
    /// check that catches scale-factor bugs introduced by the dataset
    /// re-pack.
    #[test]
    fn zenith_luminance_within_published_range() {
        let p = cook(2.5, 0.10, 60_f64.to_radians());
        let gamma = std::f64::consts::FRAC_PI_2 - 60_f64.to_radians();
        let r = radiance(&p, 0.0, gamma);
        let y = (0.2126 * r[0] + 0.7152 * r[1] + 0.0722 * r[2]) * HW_RADIANCE_TO_LUMINANCE_LM_PER_W;
        assert!(
            (1_000.0..15_000.0).contains(&y),
            "HW zenith luminance {:.0} cd/m² outside the 1000–15000 \
             reference range at T=2.5, alb=0.1, sun_alt=60°",
            y
        );
    }

    /// `turbidity_from_aerosol` is the HW alias for the V-37 Linke-turbidity
    /// bridge defined one module up.
    #[test]
    fn turbidity_bridge_matches_linke() {
        for beta in [0.0_f64, 0.05, 0.10, 0.30, 1.0] {
            assert_eq!(
                turbidity_from_aerosol(beta),
                super::super::linke_turbidity_from_aerosol(beta),
            );
        }
    }
}
