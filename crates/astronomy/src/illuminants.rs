//! Physical illuminant helpers for the Sun and Moon.
//!
//! The renderer consumes these values as first-order radiometric controls for
//! daylight / twilight scattering. They are deliberately small and dependency
//! free; richer spectral sampling can replace the constants without changing
//! the public shape.

/// Mean direct normal solar illuminance above the atmosphere, in lux.
///
/// The value is derived from the solar constant (~1361 W/m²) and the daylight
/// luminous-efficacy range (~93 lm/W). It represents the top-of-atmosphere
/// photopic illuminance of direct sunlight at 1 AU, before extinction or
/// scattering by the local atmosphere.
pub const SOLAR_ILLUMINANCE_1_AU_LUX: f64 = 127_000.0;

/// Approximate direct solar illuminance at top of atmosphere for an Earth-Sun
/// distance in astronomical units.
pub fn solar_illuminance_lux(distance_au: f64) -> f64 {
    SOLAR_ILLUMINANCE_1_AU_LUX / (distance_au * distance_au)
}

/// CIE D65-like daylight chromaticity converted to linear RGB and normalised
/// to the green channel.
///
/// This is a rendering convenience: the scattering shader still applies the
/// wavelength-dependent Rayleigh/Mie terms, but it needs a white solar input
/// colour in RGB space. D65 is a defensible daylight white until the renderer
/// grows a sampled spectrum.
pub const SOLAR_LINEAR_RGB: [f64; 3] = [0.950, 1.000, 1.089];

/// Full-moon horizontal illuminance at sea level under a clear atmosphere,
/// order-of-magnitude average in lux.
///
/// Moonlight varies by phase, distance, libration, and atmospheric path. The
/// first scattering pass only needs a physically plausible scale so that later
/// ELP2000 Moon geometry can modulate it instead of inventing a new unit.
pub const FULL_MOON_ILLUMINANCE_LUX: f64 = 0.25;

/// Mean geocentric lunar distance, in kilometres, used to scale first-order
/// moonlight brightness before the final ELP2000 / photometric phase law lands.
pub const MEAN_MOON_DISTANCE_KM: f64 = 384_400.0;

/// CIE 1931 XYZ chromaticity/luminance-like weights for the top-of-atmosphere
/// solar illuminant, normalised to `Y=1`. This is the same D65 daylight white
/// as [`SOLAR_LINEAR_RGB`], exposed in XYZ so renderers that work spectrally or
/// colourimetrically do not need to reverse-engineer the RGB convenience value.
pub const SOLAR_XYZ_Y_NORMALIZED: [f64; 3] = [0.95047, 1.0, 1.08883];

/// Approximate full-Moon XYZ colour, normalised to `Y=1`. Moonlight is slightly
/// warmer than D65 after reflecting from the lunar regolith; the absolute scale
/// comes from [`lunar_illuminance_lux`].
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
}
