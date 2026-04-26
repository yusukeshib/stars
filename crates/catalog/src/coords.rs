use glam::Vec3;
use std::f64::consts::PI;

/// Convert right ascension (hours) and declination (degrees) to a unit-sphere Cartesian position.
pub fn radec_to_cartesian(ra_hours: f64, dec_degrees: f64) -> Vec3 {
    let ra = ra_hours * (PI / 12.0);
    let dec = dec_degrees * (PI / 180.0);

    let x = dec.cos() * ra.cos();
    let y = dec.cos() * ra.sin();
    let z = dec.sin();

    Vec3::new(x as f32, y as f32, z as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polaris() {
        // Polaris: RA ~2.53h, Dec ~89.26°
        let pos = radec_to_cartesian(2.53, 89.26);
        // Should be nearly at the north pole (z ≈ 1)
        assert!(pos.z > 0.99, "Polaris z={}, expected near 1.0", pos.z);
        assert!(pos.length() > 0.99 && pos.length() < 1.01);
    }

    #[test]
    fn test_origin() {
        // RA=0, Dec=0 → (1, 0, 0)
        let pos = radec_to_cartesian(0.0, 0.0);
        assert!((pos.x - 1.0).abs() < 1e-6);
        assert!(pos.y.abs() < 1e-6);
        assert!(pos.z.abs() < 1e-6);
    }

    #[test]
    fn test_south_pole() {
        // RA=0, Dec=-90 → (0, 0, -1)
        let pos = radec_to_cartesian(0.0, -90.0);
        assert!(pos.x.abs() < 1e-6);
        assert!(pos.y.abs() < 1e-6);
        assert!((pos.z + 1.0).abs() < 1e-6);
    }
}
