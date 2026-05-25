//! Reference atmosphere radiance helpers for the Phase 2 visual pipeline.
//!
//! Runtime rendering happens in `crates/renderer/src/shaders/skyglow.wgsl`, but
//! the scalar pieces below give the Rust side pinned, documented values for the
//! same cited model family: Preetham/Shirley/Smits daylight zenith luminance and
//! a continuous solar-depression twilight curve tied to clear-site visual sky
//! brightness. They are intentionally small reference functions, not a spectral
//! multiple-scattering solver.

const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;

/// Perez/Preetham zenith luminance in cd/m² for a directly sunlit sky.
///
/// Implements the zenith luminance term from Preetham, Shirley & Smits 1999,
/// with turbidity clamped to the paper's practical daylight range. Returns
/// `None` when the Sun is below the geometric horizon because Preetham is a
/// daylight model, not a twilight model.
pub fn preetham_zenith_luminance_cd_m2(solar_altitude_rad: f64, turbidity: f64) -> Option<f64> {
    if solar_altitude_rad <= 0.0 || !solar_altitude_rad.is_finite() {
        return None;
    }
    let t = turbidity.clamp(1.7, 10.0);
    let theta_s = (std::f64::consts::FRAC_PI_2 - solar_altitude_rad)
        .clamp(0.0, std::f64::consts::FRAC_PI_2 - 0.01);
    let chi = (4.0 / 9.0 - t / 120.0) * (std::f64::consts::PI - 2.0 * theta_s);
    Some(((4.0453 * t - 4.9710) * chi.tan() - 0.2155 * t + 2.4192).max(0.0) * 1000.0)
}

/// Clear-site zenith twilight brightness in cd/m².
///
/// This is the luminance form of [`crate::skyglow::twilight_zenith_mag_per_arcsec2`]:
/// solar depression controls an Earth-shadow optical-depth proxy, continuous
/// from civil through astronomical twilight. It deliberately returns `None` for
/// daylight and for fully dark sky so callers compose it with daylight and
/// Phase 1' dark-sky radiance instead of using hard-coded colour fades.
pub fn twilight_zenith_luminance_cd_m2(solar_altitude_rad: f64) -> Option<f64> {
    let mu = crate::skyglow::twilight_zenith_mag_per_arcsec2(solar_altitude_rad)?;
    // V-band mag/arcsec² to luminance: L[cd/m²] = 10.8e4 * 10^(-0.4 μ).
    // The 10.8e4 factor is the standard visual conversion for astronomical
    // surface brightness and keeps the twilight reference in absolute units.
    Some(108_000.0 * 10.0_f64.powf(-0.4 * mu))
}

/// Smooth physical-domain selector for daylight/twilight/dark-sky composition.
///
/// Returns `(daylight, twilight, dark)` weights from solar altitude. The weights
/// are labels for model validity domains, not exposure gates: radiance remains
/// additive and each term is still computed in physical units.
pub fn solar_depression_domain_weights(solar_altitude_rad: f64) -> (f64, f64, f64) {
    let h = solar_altitude_rad / DEG_TO_RAD;
    let daylight = smoothstep(-0.5, 0.5, h);
    let dark = 1.0 - smoothstep(-18.5, -17.5, h);
    let twilight = (1.0 - daylight - dark).clamp(0.0, 1.0);
    (daylight, twilight, dark)
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preetham_daylight_is_bright_and_domain_limited() {
        assert_eq!(
            preetham_zenith_luminance_cd_m2((-1.0_f64).to_radians(), 2.5),
            None
        );
        let noon = preetham_zenith_luminance_cd_m2(60_f64.to_radians(), 2.5).unwrap();
        let low_sun = preetham_zenith_luminance_cd_m2(5_f64.to_radians(), 2.5).unwrap();
        assert!(
            noon > 4_000.0,
            "clear daylight zenith should be thousands of cd/m²"
        );
        assert!(
            low_sun < noon,
            "low-sun zenith should be dimmer than high-sun zenith"
        );
    }

    #[test]
    fn twilight_luminance_is_continuous_and_fades_to_dark_sky() {
        let civil = twilight_zenith_luminance_cd_m2((-6.0_f64).to_radians()).unwrap();
        let nautical = twilight_zenith_luminance_cd_m2((-12.0_f64).to_radians()).unwrap();
        let astronomical = twilight_zenith_luminance_cd_m2((-17.999_f64).to_radians()).unwrap();
        assert!(civil > nautical && nautical > astronomical);
        assert!(civil > 1.0e-1);
        assert!(astronomical < 1.0e-3);
        assert_eq!(twilight_zenith_luminance_cd_m2(1_f64.to_radians()), None);
        assert_eq!(
            twilight_zenith_luminance_cd_m2((-19_f64).to_radians()),
            None
        );
    }

    #[test]
    fn solar_domain_weights_cover_day_twilight_and_night() {
        let (day, twi, dark) = solar_depression_domain_weights(10_f64.to_radians());
        assert!(day > 0.99 && twi < 0.01 && dark < 0.01);
        let (day, twi, dark) = solar_depression_domain_weights((-6_f64).to_radians());
        assert!(twi > 0.99 && day < 0.01 && dark < 0.01);
        let (day, twi, dark) = solar_depression_domain_weights((-20_f64).to_radians());
        assert!(dark > 0.99 && day < 0.01 && twi < 0.01);
    }
}
