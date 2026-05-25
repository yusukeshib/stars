//! Physical illuminant helpers for the Sun and Moon.
//!
//! The renderer consumes these values as first-order radiometric controls for
//! daylight / twilight scattering. They are deliberately small and dependency
//! free; richer spectral sampling can replace the constants without changing
//! the public shape.

/// Mean direct normal solar illuminance above the atmosphere, in lux.
///
/// Derived from the modern solar constant scale (~1361 W/m² at the IAU 2012
/// exact astronomical unit) and daylight luminous efficacy near 93 lm/W. It is
/// a broadband photopic rendering scale, not a spectral solar irradiance table;
/// Phase 2 tracks ASTM G-173 / CIE daylight-basis spectra for that.
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
/// regolith. The chromaticity here is a visual-rendering placeholder; Phase 2's
/// spectral/XYZ illuminant row tracks replacing it with a cited lunar spectrum.
/// The absolute scale comes from [`lunar_illuminance_lux`].
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
