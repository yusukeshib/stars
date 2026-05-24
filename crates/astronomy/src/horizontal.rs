use glam::{Mat3, Mat4};

/// Altitude/azimuth in radians. Azimuth is measured from North, increasing toward East.
#[derive(Debug, Clone, Copy)]
pub struct AltAz {
    pub altitude: f64,
    pub azimuth: f64,
}

/// Convert equatorial (right ascension, declination) to local horizontal at the given
/// observer latitude and Local Sidereal Time. All inputs in radians.
pub fn equatorial_to_horizontal(ra: f64, dec: f64, lst: f64, lat: f64) -> AltAz {
    let h = lst - ra; // hour angle
    let sin_alt = lat.sin() * dec.sin() + lat.cos() * dec.cos() * h.cos();
    let altitude = sin_alt.clamp(-1.0, 1.0).asin();
    // Azimuth measured from North toward East.
    let azimuth =
        (-h.sin() * dec.cos()).atan2(dec.sin() * lat.cos() - h.cos() * dec.cos() * lat.sin());
    AltAz { altitude, azimuth }
}

/// Rotation that maps a J2000 equatorial unit vector into the observer's local
/// East-North-Up frame.
///
/// `lat_rad` is the observer's latitude. For the geometry below this is
/// strictly the **astronomical latitude** — the angle between the celestial
/// equator and the local gravity vector — because we treat "Up" as the local
/// vertical. For real-world inputs the user supplies geodetic (= geographic)
/// latitude, which differs from astronomical latitude only by the deflection
/// of the vertical (≲1″ essentially everywhere), so the two are interchangeable
/// at Phase 1 precision. The difference from *geocentric* latitude (up to
/// ≈11.5′ at ±45°) does NOT matter for stars — their distances make diurnal
/// parallax negligible — but WILL matter for the Moon and planets in Phase 2.
///
/// `lst_rad` is Local Mean Sidereal Time. The matrix is orthonormal; its
/// transpose maps ENU back to equatorial.
///
/// Trig is evaluated in `f64` so the per-frame uniform retains ≲arcsec
/// precision after the final cast to `f32`. (`f32::sin_cos` at LST near 2π
/// quantises to ≈0.15″ — visible if star labels ever land.)
pub fn equatorial_to_horizontal_matrix(lat_rad: f64, lst_rad: f64) -> Mat4 {
    let (s_phi, c_phi) = lat_rad.sin_cos();
    let (s_theta, c_theta) = lst_rad.sin_cos();

    // Rows: East, North, Up basis vectors expressed in equatorial coords
    // (verified by N × U = E using the right-handed ENU convention).
    // glam Mat3::from_cols stores columns, so we list columns below.
    let r = Mat3::from_cols_array(&[
        // col 0
        -s_theta as f32,
        (-s_phi * c_theta) as f32,
        (c_phi * c_theta) as f32,
        // col 1
        c_theta as f32,
        (-s_phi * s_theta) as f32,
        (c_phi * s_theta) as f32,
        // col 2
        0.0,
        c_phi as f32,
        s_phi as f32,
    ]);
    Mat4::from_mat3(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{gmst_radians, lmst_radians};
    use glam::Vec3;

    fn deg(x: f64) -> f64 {
        x.to_radians()
    }

    #[test]
    fn polaris_near_zenith_at_north_pole() {
        // At the North Pole, Polaris (~RA 2.53h, Dec 89.26°) sits near the zenith.
        let lat = deg(90.0);
        let lst = 0.0; // irrelevant at the pole
        let ra = 2.53 * std::f64::consts::PI / 12.0;
        let dec = deg(89.26);
        let alt_az = equatorial_to_horizontal(ra, dec, lst, lat);
        assert!(
            (alt_az.altitude - deg(89.26)).abs() < 1e-3,
            "altitude={}",
            alt_az.altitude
        );
    }

    #[test]
    fn vernal_equinox_on_meridian() {
        // When LST == RA, the star is on the local meridian: azimuth ≈ 0 or π.
        // For a star with dec > lat, it culminates due south is false in north hemisphere
        // unless dec < lat. Use lat = 60°, dec = 0 → culminates due south, az = π.
        let lat = deg(60.0);
        let ra = 0.0;
        let dec = 0.0;
        let lst = ra; // on meridian
        let alt_az = equatorial_to_horizontal(ra, dec, lst, lat);
        // Altitude should be 90° - lat = 30°
        assert!(
            (alt_az.altitude - deg(30.0)).abs() < 1e-6,
            "alt={}",
            alt_az.altitude
        );
        // Azimuth should be π (due south).
        let az_norm =
            (alt_az.azimuth.rem_euclid(2.0 * std::f64::consts::PI) - std::f64::consts::PI).abs();
        assert!(az_norm < 1e-6, "az={}", alt_az.azimuth);
    }

    #[test]
    fn matrix_zenith_points_to_observer_zenith() {
        // The local zenith (0,0,1 in ENU) corresponds to the equatorial vector
        // (cos φ cos θ, cos φ sin θ, sin φ) in equatorial coords.
        let lat = deg(35.0);
        let lst = deg(120.0);
        let m = equatorial_to_horizontal_matrix(lat, lst);
        let zenith_eq = Vec3::new(
            (lat.cos() * lst.cos()) as f32,
            (lat.cos() * lst.sin()) as f32,
            lat.sin() as f32,
        );
        let local = m.transform_vector3(zenith_eq);
        assert!((local.x).abs() < 1e-5);
        assert!((local.y).abs() < 1e-5);
        assert!((local.z - 1.0).abs() < 1e-5);
    }

    #[test]
    fn matrix_consistent_with_alt_az() {
        // For a few stars, compare matrix-rotated z component (= sin altitude)
        // against direct altitude computation.
        let jd = 2_460_000.5; // arbitrary recent date
        let lat = deg(35.0);
        let lng = deg(139.0); // Tokyo-ish
        let lst = lmst_radians(jd, lng);
        let m = equatorial_to_horizontal_matrix(lat, lst);

        let cases = [
            (deg(101.0), deg(-16.7)), // Sirius-ish
            (deg(279.0), deg(38.78)), // Vega-ish
            (deg(37.95), deg(89.26)), // Polaris-ish
        ];
        for (ra, dec) in cases {
            let v = Vec3::new(
                (dec.cos() * ra.cos()) as f32,
                (dec.cos() * ra.sin()) as f32,
                dec.sin() as f32,
            );
            let local = m.transform_vector3(v);
            let alt_from_matrix = local.z.clamp(-1.0, 1.0).asin() as f64;
            let alt_az = equatorial_to_horizontal(ra, dec, lst, lat);
            assert!(
                (alt_from_matrix - alt_az.altitude).abs() < 1e-4,
                "ra={ra}, dec={dec}: matrix={alt_from_matrix}, direct={}",
                alt_az.altitude
            );
        }
        // Also assert gmst is sane at this jd
        let g = gmst_radians(jd);
        assert!(g.is_finite());
    }
}
