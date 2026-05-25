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
}
