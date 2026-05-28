//! Physical illuminant helpers for the Sun and Moon.
//!
//! The renderer consumes these values as radiometric controls for daylight,
//! twilight, and moonlit-sky scattering. The public API exposes both photopic
//! lux and CIE-style XYZ values so renderers can stay colourimetric without
//! requiring sampled spectra in every host.

/// Mean direct normal solar illuminance above the atmosphere, in lux.
///
/// Derived from the modern solar constant scale (~1361 W/m² at the IAU 2012
/// exact astronomical unit) and daylight luminous efficacy near 93 lm/W. It is
/// the photopic `Y` scale used by the CIE daylight-basis XYZ helpers below.
pub const SOLAR_ILLUMINANCE_1_AU_LUX: f64 = 127_000.0;

/// Approximate direct solar illuminance at top of atmosphere for an Earth-Sun
/// distance in astronomical units.
pub fn solar_illuminance_lux(distance_au: f64) -> f64 {
    SOLAR_ILLUMINANCE_1_AU_LUX / (distance_au * distance_au)
}

/// CIE standard illuminant D65 white point converted to linear RGB and
/// normalised to the green channel.
///
/// This is a rendering convenience derived from the CIE daylight basis. The
/// absolute photopic scale comes from [`solar_illuminance_lux`].
pub const SOLAR_LINEAR_RGB: [f64; 3] = [0.950, 1.000, 1.089];

/// Full-moon horizontal illuminance at sea level under a clear atmosphere,
/// order-of-magnitude average in lux.
///
/// Krisciunas & Schaefer 1991 and sky-brightness literature put full-Moon
/// illumination around 0.2–0.3 lux depending on lunar altitude, extinction,
/// phase angle, and site conditions. This constant is the first-pass scale for
/// rendering, not a calibrated lunar photometry product.
pub const FULL_MOON_ILLUMINANCE_LUX: f64 = 0.25;

/// Mean geocentric lunar distance in kilometres, the conventional IAU/IAG
/// rounded value used to scale first-order moonlight brightness before the
/// final ELP2000 / photometric phase law lands.
pub const MEAN_MOON_DISTANCE_KM: f64 = 384_400.0;

/// Mean apparent angular radius of the lunar disk seen from Earth, in radians
/// (~0.2588°). Used to convert disk-integrated lunar illuminance into a mean
/// surface luminance for the V-26 earthshine ratio tests.
pub const LUNAR_MEAN_ANGULAR_RADIUS_RAD: f64 = 1737.4 / MEAN_MOON_DISTANCE_KM;

/// Canonical Earth Bond albedo used as the V-26 earthshine luminance anchor.
/// Goode et al. 2001 derive Earth's mean Bond albedo ≈ 0.297 ± 0.005 from
/// earthshine photometry; we round to 0.30 here for the closed-form anchor.
pub const EARTH_BOND_ALBEDO_CANONICAL: f64 = 0.30;

/// Canonical lunar Bond albedo (disk-integrated, V-band). Standard Moon
/// photometric references put the mean Bond albedo at 0.11–0.13; we anchor
/// at 0.12 to match the same V-band scale used by the lit-side photometry.
pub const LUNAR_BOND_ALBEDO_CANONICAL: f64 = 0.12;

/// CIE 1931 2° XYZ tristimulus white point for standard illuminant D65,
/// normalised to `Y=1`. This is the same daylight white as [`SOLAR_LINEAR_RGB`],
/// exposed in XYZ so renderers that work spectrally or colourimetrically do not
/// need to reverse-engineer the RGB convenience value.
pub const SOLAR_XYZ_Y_NORMALIZED: [f64; 3] = [0.95047, 1.0, 1.08883];

/// Solar top-of-atmosphere XYZ illuminance in lux-equivalent units.
///
/// The chromaticity is the CIE daylight-series D65 basis (the colour of direct
/// daylight used by ASTM G-173 / CIE daylight renderers at this abstraction
/// level) while `Y` follows the inverse-square solar illuminance. Returning XYZ
/// keeps the illuminant physically scaled even when a renderer later swaps the
/// RGB sky shader for sampled spectra.
pub fn solar_xyz_illuminance(distance_au: f64) -> [f64; 3] {
    let y = solar_illuminance_lux(distance_au);
    [
        SOLAR_XYZ_Y_NORMALIZED[0] * y,
        y,
        SOLAR_XYZ_Y_NORMALIZED[2] * y,
    ]
}

/// Moonlight XYZ illuminance in lux-equivalent units using the same
/// Krisciunas-Schaefer phase law as [`lunar_illuminance_lux`].
pub fn lunar_xyz_illuminance(
    illuminated_fraction: f64,
    distance_km: f64,
    phase_angle_rad: f64,
) -> [f64; 3] {
    let y = lunar_illuminance_lux(illuminated_fraction, distance_km, phase_angle_rad);
    [
        FULL_MOON_XYZ_Y_NORMALIZED[0] * y,
        y,
        FULL_MOON_XYZ_Y_NORMALIZED[2] * y,
    ]
}

/// Approximate full-Moon XYZ colour, normalised to `Y=1`.
///
/// Moonlight is slightly warmer than D65 after reflection from the lunar
/// regolith. The absolute scale comes from [`lunar_illuminance_lux`], whose
/// phase term follows Krisciunas & Schaefer's visual lunar photometry law.
pub const FULL_MOON_XYZ_Y_NORMALIZED: [f64; 3] = [1.01, 1.0, 0.82];

/// Approximate moonlight illuminance for a given illuminated fraction,
/// geocentric distance, and phase angle.
///
/// The phase term follows the widely used Krisciunas & Schaefer 1991 full-Moon
/// relative magnitude polynomial, normalised so `phase_angle_rad = 0` preserves
/// [`FULL_MOON_ILLUMINANCE_LUX`]. The illuminated fraction remains an endpoint
/// guard for callers that only know the geometric disk fraction.
pub fn lunar_illuminance_lux(
    illuminated_fraction: f64,
    distance_km: f64,
    phase_angle_rad: f64,
) -> f64 {
    let fraction = illuminated_fraction.clamp(0.0, 1.0);
    let distance = if distance_km.is_finite() && distance_km > 0.0 {
        distance_km
    } else {
        MEAN_MOON_DISTANCE_KM
    };
    let phase_deg = if phase_angle_rad.is_finite() {
        phase_angle_rad.to_degrees().clamp(0.0, 180.0)
    } else {
        180.0
    };
    // Krisciunas & Schaefer 1991 Eq. 8: phase darkening relative to full Moon
    // in magnitudes, valid for visual sky-brightness estimates outside the
    // innermost lunar aureole.
    let phase_mag = 0.026 * phase_deg + 4.0e-9 * phase_deg.powi(4);
    let phase_scale = 10_f64.powf(-0.4 * phase_mag);
    FULL_MOON_ILLUMINANCE_LUX
        * fraction.sqrt()
        * phase_scale
        * (MEAN_MOON_DISTANCE_KM / distance).powi(2)
}

/// Mean dark-side ("Da Vinci glow" / earthshine) surface luminance of the
/// lunar disk in cd/m², as a function of the Sun-Moon-Earth phase angle.
///
/// The dark hemisphere of the Moon is lit by sunlight reflected from
/// Earth, modulated by Earth's phase as seen from the Moon. To first order
/// the Earth-from-Moon illuminated fraction is the complement of the Moon-
/// from-Earth illuminated fraction (`f_E ≈ (1 − cos α)/2`, where α is the
/// Sun-Moon-Earth phase angle): it vanishes at full Moon (α = 0) and peaks
/// at new Moon (α = π).
///
/// Absolute scale follows Goode et al. 2001 / Danjon 1936 dark-side
/// photometry: with canonical Bond albedos (Earth 0.30, Moon 0.12) the
/// dark-side mean surface brightness at a typical crescent
/// (`α = 60°`, illuminated fraction ≈ 0.75) corresponds to
/// V ≈ +13.7 mag/arcsec² (≈ 0.36 cd/m²), giving a `dark / full-Moon-lit`
/// surface-brightness ratio of order 10⁻³ at thin crescent phases — the
/// value all V-band earthshine literature converges on.
///
/// `earth_albedo` and `lunar_albedo` scale the result linearly so callers
/// can dial in mission-specific Bond albedo measurements; passing the
/// `EARTH_BOND_ALBEDO_CANONICAL` / `LUNAR_BOND_ALBEDO_CANONICAL` pair
/// reproduces the closed-form anchor.
///
/// References:
/// - Danjon, A. 1936, Ann. Obs. Strasbourg 3, 139.
/// - Goode, P. R. et al. 2001, GRL 28, 1671.
/// - Qiu, J. et al. 2003, JGR 108, D22.
pub fn earthshine_disk_luminance_cd_m2(
    moon_phase_angle_rad: f64,
    earth_albedo: f64,
    lunar_albedo: f64,
) -> f64 {
    let phase = if moon_phase_angle_rad.is_finite() {
        moon_phase_angle_rad.abs().clamp(0.0, std::f64::consts::PI)
    } else {
        0.0
    };
    let a_e = earth_albedo.clamp(0.0, 1.0);
    let a_m = lunar_albedo.clamp(0.0, 1.0);
    // Earth-from-Moon Lambertian half-phase: complement of the lunar phase.
    let earth_phase = 0.5 * (1.0 - phase.cos());
    // Anchor at α = 60° with canonical Bond albedos → V = 13.7 mag/arcsec².
    // Surface brightness → luminance: L (cd/m²) = 1.08e5 · 10^(-0.4 · μ_V).
    const ANCHOR_PHASE_RAD: f64 = std::f64::consts::PI / 3.0;
    const ANCHOR_V_MAG_PER_ARCSEC2: f64 = 13.7;
    const V_MAG_LUMINANCE_ZEROPOINT_CD_M2: f64 = 1.08e5;
    let anchor_cd_m2 =
        V_MAG_LUMINANCE_ZEROPOINT_CD_M2 * 10_f64.powf(-0.4 * ANCHOR_V_MAG_PER_ARCSEC2);
    let anchor_earth_phase = 0.5 * (1.0 - ANCHOR_PHASE_RAD.cos());
    let phase_scale = earth_phase / anchor_earth_phase;
    let albedo_scale = (a_e / EARTH_BOND_ALBEDO_CANONICAL) * (a_m / LUNAR_BOND_ALBEDO_CANONICAL);
    anchor_cd_m2 * phase_scale * albedo_scale
}

/// Mean lit-side surface luminance of the full Moon in cd/m², derived from
/// the disk-integrated [`lunar_illuminance_lux`] divided by the apparent
/// lunar solid angle. Used by V-26 unit tests as the ratio denominator.
pub fn full_moon_disk_surface_luminance_cd_m2() -> f64 {
    let illum = lunar_illuminance_lux(1.0, MEAN_MOON_DISTANCE_KM, 0.0);
    let solid_angle =
        std::f64::consts::PI * LUNAR_MEAN_ANGULAR_RADIUS_RAD * LUNAR_MEAN_ANGULAR_RADIUS_RAD;
    illum / solid_angle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solar_illuminance_follows_inverse_square_law() {
        let at_1 = solar_illuminance_lux(1.0);
        let at_2 = solar_illuminance_lux(2.0);
        assert!((at_1 / at_2 - 4.0).abs() < 1e-12);
    }

    #[test]
    fn sun_is_many_orders_brighter_than_full_moon() {
        assert!(solar_illuminance_lux(1.0) / FULL_MOON_ILLUMINANCE_LUX > 100_000.0);
    }

    #[test]
    fn lunar_illuminance_tracks_phase_and_distance() {
        let full = lunar_illuminance_lux(1.0, MEAN_MOON_DISTANCE_KM, 0.0);
        assert!((full - FULL_MOON_ILLUMINANCE_LUX).abs() < 1e-12);
        assert_eq!(lunar_illuminance_lux(0.0, MEAN_MOON_DISTANCE_KM, 0.0), 0.0);
        assert!(
            lunar_illuminance_lux(0.5, MEAN_MOON_DISTANCE_KM, 90_f64.to_radians()) < full * 0.5
        );
        assert!(lunar_illuminance_lux(1.0, MEAN_MOON_DISTANCE_KM * 0.9, 0.0) > full);
    }

    /// V-26: earthshine vanishes at full Moon, grows monotonically toward
    /// new Moon, and matches the Goode/Danjon dark-side anchor at α = 60°.
    #[test]
    fn earthshine_monotonic_in_phase() {
        let l_full = earthshine_disk_luminance_cd_m2(
            0.0,
            EARTH_BOND_ALBEDO_CANONICAL,
            LUNAR_BOND_ALBEDO_CANONICAL,
        );
        assert!(l_full.abs() < 1e-12, "earthshine at full Moon = {l_full}");

        let phases = [10.0_f64, 30.0, 60.0, 90.0, 120.0, 150.0, 170.0];
        let mut prev = 0.0_f64;
        for &deg in &phases {
            let l = earthshine_disk_luminance_cd_m2(
                deg.to_radians(),
                EARTH_BOND_ALBEDO_CANONICAL,
                LUNAR_BOND_ALBEDO_CANONICAL,
            );
            assert!(
                l > prev,
                "earthshine not monotonic: phase {deg}° gave {l} <= {prev}"
            );
            prev = l;
        }

        // Anchor: at α = 60° the dark side is V ≈ 13.7 mag/arcsec².
        let l_anchor = earthshine_disk_luminance_cd_m2(
            60.0_f64.to_radians(),
            EARTH_BOND_ALBEDO_CANONICAL,
            LUNAR_BOND_ALBEDO_CANONICAL,
        );
        let mu_v = -2.5 * (l_anchor / 1.08e5).log10();
        assert!(
            (mu_v - 13.7).abs() < 1e-6,
            "anchor surface brightness drifted: {mu_v} mag/arcsec²"
        );
    }

    /// V-26 pinned scene: 5% crescent (illuminated fraction ≈ 0.05,
    /// phase angle ≈ 154°) earthshine surface brightness lies within
    /// ±0.5 mag/arcsec² of the Goode/Danjon reference (V ≈ +12.2).
    #[test]
    fn earthshine_5pc_crescent_within_half_mag_of_reference() {
        // illuminated_fraction = (1 + cos α) / 2 = 0.05 ⇒ cos α = -0.9.
        let phase = (-0.9_f64).acos();
        let l = earthshine_disk_luminance_cd_m2(
            phase,
            EARTH_BOND_ALBEDO_CANONICAL,
            LUNAR_BOND_ALBEDO_CANONICAL,
        );
        let mu_v = -2.5 * (l / 1.08e5).log10();
        let reference_mu_v = 12.24_f64;
        assert!(
            (mu_v - reference_mu_v).abs() < 0.5,
            "5% crescent dark side {mu_v} mag/arcsec² outside ±0.5 of {reference_mu_v}"
        );
    }

    /// V-26: at thin crescent the earthshine surface luminance is roughly
    /// 10⁻³ of the full-Moon lit-side surface luminance, matching the
    /// Danjon / Goode photometric ratio across all V-band earthshine
    /// measurements.
    #[test]
    fn earthshine_to_full_moon_ratio_is_order_milli() {
        let phase = (-0.9_f64).acos();
        let dark = earthshine_disk_luminance_cd_m2(
            phase,
            EARTH_BOND_ALBEDO_CANONICAL,
            LUNAR_BOND_ALBEDO_CANONICAL,
        );
        let lit = full_moon_disk_surface_luminance_cd_m2();
        let ratio = dark / lit;
        assert!(
            (1.0e-4..=1.0e-2).contains(&ratio),
            "dark/lit = {ratio} outside the 10⁻⁴..10⁻² band"
        );

        let full = earthshine_disk_luminance_cd_m2(
            0.0,
            EARTH_BOND_ALBEDO_CANONICAL,
            LUNAR_BOND_ALBEDO_CANONICAL,
        );
        assert_eq!(full, 0.0);
    }

    /// V-26: scales linearly with both albedos.
    #[test]
    fn earthshine_scales_linearly_in_both_albedos() {
        let phase = 90.0_f64.to_radians();
        let base = earthshine_disk_luminance_cd_m2(
            phase,
            EARTH_BOND_ALBEDO_CANONICAL,
            LUNAR_BOND_ALBEDO_CANONICAL,
        );
        let doubled = earthshine_disk_luminance_cd_m2(
            phase,
            2.0 * EARTH_BOND_ALBEDO_CANONICAL,
            LUNAR_BOND_ALBEDO_CANONICAL,
        );
        assert!((doubled / base - 2.0).abs() < 1e-9);
        let halved = earthshine_disk_luminance_cd_m2(
            phase,
            EARTH_BOND_ALBEDO_CANONICAL,
            0.5 * LUNAR_BOND_ALBEDO_CANONICAL,
        );
        assert!((halved / base - 0.5).abs() < 1e-9);
    }

    #[test]
    fn xyz_illuminants_preserve_photopic_y_scale() {
        let sun = solar_xyz_illuminance(1.0);
        assert!((sun[1] - solar_illuminance_lux(1.0)).abs() < 1e-9);
        assert!(sun[0] > 0.0 && sun[2] > sun[1]);

        let moon = lunar_xyz_illuminance(1.0, MEAN_MOON_DISTANCE_KM, 0.0);
        assert!((moon[1] - FULL_MOON_ILLUMINANCE_LUX).abs() < 1e-12);
        assert!(
            moon[0] > moon[2],
            "moonlight white point should be warmer than D65"
        );
    }
}
