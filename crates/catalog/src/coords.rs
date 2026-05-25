use glam::Vec3;
use std::f64::consts::PI;

/// Convert right ascension (hours) + declination (degrees) to a unit-sphere
/// Cartesian position in the J2000 equatorial frame.
///
/// The deliberately verbose name encodes the unit choice in the type so callers
/// can't accidentally pass radians. The mixed units match the HYG catalog's
/// CSV columns (`ra` in hours, `dec` in degrees); for any other input format,
/// convert to (radians, radians) and use the inline trig directly.
///
/// Output: `x = cos δ cos α`, `y = cos δ sin α`, `z = sin δ`.
pub fn radec_hours_deg_to_cartesian(ra_hours: f64, dec_degrees: f64) -> Vec3 {
    let ra = ra_hours * (PI / 12.0);
    let dec = dec_degrees * (PI / 180.0);

    let x = dec.cos() * ra.cos();
    let y = dec.cos() * ra.sin();
    let z = dec.sin();

    Vec3::new(x as f32, y as f32, z as f32)
}

/// Convert HYG proper-motion columns into a Cartesian tangent vector in
/// radians per Julian year.
///
/// HYG carries `pmrarad` / `pmdecrad` in radians/year. `pmrarad` follows the
/// catalog convention μα⋅cosδ, so it multiplies the unit vector in the local
/// increasing-RA direction directly; `pmdecrad` multiplies the increasing-Dec
/// basis. Adding `proper_motion * Δyears` to the unit position and normalising
/// is first-order accurate for the Phase-2 naked-eye stars.
pub fn proper_motion_vector_radians_per_year(
    ra_hours: f64,
    dec_degrees: f64,
    pmra_rad_year: f64,
    pmdec_rad_year: f64,
) -> Vec3 {
    let ra = ra_hours * (PI / 12.0);
    let dec = dec_degrees * (PI / 180.0);
    let (sin_ra, cos_ra) = ra.sin_cos();
    let (sin_dec, cos_dec) = dec.sin_cos();
    let e_ra = [-sin_ra, cos_ra, 0.0];
    let e_dec = [-sin_dec * cos_ra, -sin_dec * sin_ra, cos_dec];
    Vec3::new(
        (pmra_rad_year * e_ra[0] + pmdec_rad_year * e_dec[0]) as f32,
        (pmra_rad_year * e_ra[1] + pmdec_rad_year * e_dec[1]) as f32,
        (pmra_rad_year * e_ra[2] + pmdec_rad_year * e_dec[2]) as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polaris() {
        // Polaris: RA ~2.53h, Dec ~89.26°
        let pos = radec_hours_deg_to_cartesian(2.53, 89.26);
        // Should be nearly at the north pole (z ≈ 1)
        assert!(pos.z > 0.99, "Polaris z={}, expected near 1.0", pos.z);
        assert!(pos.length() > 0.99 && pos.length() < 1.01);
    }

    #[test]
    fn test_origin() {
        // RA=0, Dec=0 → (1, 0, 0)
        let pos = radec_hours_deg_to_cartesian(0.0, 0.0);
        assert!((pos.x - 1.0).abs() < 1e-6);
        assert!(pos.y.abs() < 1e-6);
        assert!(pos.z.abs() < 1e-6);
    }

    #[test]
    fn test_south_pole() {
        // RA=0, Dec=-90 → (0, 0, -1)
        let pos = radec_hours_deg_to_cartesian(0.0, -90.0);
        assert!(pos.x.abs() < 1e-6);
        assert!(pos.y.abs() < 1e-6);
        assert!((pos.z + 1.0).abs() < 1e-6);
    }

    #[test]
    fn proper_motion_vector_is_tangent_to_catalog_direction() {
        let pos = radec_hours_deg_to_cartesian(6.752481, -16.716116);
        let pm = proper_motion_vector_radians_per_year(
            6.752481,
            -16.716116,
            -0.000_002_647_131_177_2,
            -0.000_005_929_659_164,
        );
        assert!(pos.dot(pm).abs() < 1e-10, "proper motion must be tangent");
        assert!((pm.length() - 6.49e-6).abs() < 1e-8);
    }
}
