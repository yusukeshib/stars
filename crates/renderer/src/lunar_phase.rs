//! CPU twin of the lunar-disk Lambertian phase function used by
//! `shaders/skyglow.wgsl::lunar_phase_lambert`.
//!
//! The GPU shader is what users actually see; this module exists so the
//! geometry — in particular the sign convention that places the *near*
//! (visible) hemisphere on the observer side — has a unit-tested anchor.
//! A previous bug reconstructed the surface normal with `+moon_dir` instead
//! of `-moon_dir`, which silently rendered the far hemisphere and produced
//! the *complementary* phase (a 76 % waxing gibbous appeared as a thin
//! waning crescent). The regression tests below pin that sign.
//!
//! Whenever `lunar_phase_lambert` in the WGSL changes, mirror the change
//! here and update the tests.
//!
//! All vectors are unit vectors in the same equatorial frame the shader
//! uses. `moon_dir` and `sun_dir` point **from the observer toward** the
//! Moon and Sun respectively. `radius_rad` is the Moon's apparent angular
//! radius.

/// Lambertian shading factor for a ray that may hit the lunar disk.
///
/// Returns `0.0` when the ray misses the disk or hits an unlit point, and
/// `clamp(dot(normal, sun_dir), 0, 1)` otherwise, with `normal` the
/// reconstructed surface normal on the **near** hemisphere of the Moon.
pub fn lunar_phase_lambert(
    ray_dir: [f64; 3],
    moon_dir: [f64; 3],
    sun_dir: [f64; 3],
    radius_rad: f64,
) -> f64 {
    let cos_delta = dot(ray_dir, moon_dir).clamp(-1.0, 1.0);
    let delta = cos_delta.acos();
    if delta >= radius_rad {
        return 0.0;
    }

    let r = (delta / radius_rad.max(1e-12)).clamp(0.0, 1.0);

    // Tangent component of the surface normal: the part of `ray_dir`
    // perpendicular to `moon_dir`, normalised.
    let mut tangent = sub(ray_dir, scale(moon_dir, cos_delta));
    let t2 = dot(tangent, tangent);
    if t2 < 1e-20 {
        // Ray hits the Moon dead-centre; tangent direction is degenerate
        // but the radial weight `r` is zero, so any unit tangent works.
        tangent = [1.0, 0.0, 0.0];
    } else {
        let inv = t2.sqrt().recip();
        tangent = scale(tangent, inv);
    }

    // Near-hemisphere normal: along `-moon_dir` toward the observer at the
    // disk centre, fanning out into `tangent` at the limb.
    let along = -(1.0 - r * r).max(0.0).sqrt();
    let normal = normalize(add(scale(moon_dir, along), scale(tangent, r)));

    dot(normal, sun_dir).clamp(0.0, 1.0)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn normalize(a: [f64; 3]) -> [f64; 3] {
    let n = dot(a, a).sqrt();
    if n < 1e-20 {
        [0.0, 0.0, 0.0]
    } else {
        scale(a, n.recip())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RADIUS: f64 = 0.5_f64 * 0.5_f64 * std::f64::consts::PI / 180.0; // 0.25° ~ apparent lunar radius
    const TOL: f64 = 1e-9;

    /// Disk-centre brightness must equal `cos(phase_angle)` on the lit side
    /// and zero on the unlit side. This is the key regression: getting the
    /// near-hemisphere sign wrong flips this to `-cos(phase_angle)`, which
    /// silently turns a gibbous phase into a crescent.
    #[test]
    fn disk_centre_matches_cos_phase_angle() {
        let moon_dir = [1.0, 0.0, 0.0];
        // Sweep phase angle (Sun-Moon-Earth) from full (0°) to new (180°).
        for &phase_deg in &[0.0_f64, 30.0, 58.0, 90.0, 122.0, 150.0, 180.0] {
            let phase = phase_deg.to_radians();
            // Sun direction such that the angle between -moon_dir (Moon->Earth)
            // and sun_dir (Earth->Sun) equals `phase`. Place Sun in the xy
            // plane: at phase=0 the Sun is behind the observer (-x); at
            // phase=180 the Sun is behind the Moon (+x).
            let sun_dir = [-phase.cos(), phase.sin(), 0.0];

            let centre_ray = moon_dir; // looks straight at the Moon's centre
            let lit = lunar_phase_lambert(centre_ray, moon_dir, sun_dir, RADIUS);

            // At phase >= 90° the centre is on the unlit hemisphere.
            let expected = phase.cos().max(0.0);
            assert!(
                (lit - expected).abs() < TOL,
                "phase {phase_deg}°: got {lit}, expected {expected}"
            );
        }
    }

    /// Waxing-gibbous case from the bug report:
    /// 2026-05-26 ~12 UT, phase angle ≈ 58°, ~76 % illuminated. The disk
    /// centre must be brightly lit (cos 58° ≈ 0.53), NOT the dim
    /// `1 - cos 58° ≈ 0.47` that the flipped-normal bug produced.
    #[test]
    fn waxing_gibbous_disk_centre_is_bright() {
        let moon_dir = [1.0, 0.0, 0.0];
        let phase = 58.0_f64.to_radians();
        let sun_dir = [-phase.cos(), phase.sin(), 0.0];

        let lit_centre = lunar_phase_lambert(moon_dir, moon_dir, sun_dir, RADIUS);
        assert!(
            lit_centre > 0.45,
            "waxing gibbous centre too dim: {lit_centre} (regression: \
             near/far hemisphere sign flipped in lunar_phase_lambert)"
        );
        assert!((lit_centre - phase.cos()).abs() < TOL);
    }

    /// Integrated lit area over the disk must equal the standard
    /// illuminated-fraction formula `(1 + cos α)/2` to within numerical
    /// quadrature error, for representative phase angles.
    #[test]
    fn integrated_fraction_matches_textbook_formula() {
        let moon_dir = [0.0, 0.0, 1.0];
        // Build an orthonormal basis (u, v) perpendicular to moon_dir.
        let u = [1.0, 0.0, 0.0];
        let v = [0.0, 1.0, 0.0];
        let radius = RADIUS;
        let n: usize = 401; // odd → samples the centre

        for &phase_deg in &[0.0_f64, 30.0, 58.0, 90.0, 122.0, 150.0] {
            let phase = phase_deg.to_radians();
            // Sun in the xz plane.
            let sun_dir = [phase.sin(), 0.0, -phase.cos()];

            let mut weighted = 0.0;
            let mut covered = 0.0;
            let step = 2.0 * radius / n as f64;
            for i in 0..n {
                let dx = -radius + (i as f64 + 0.5) * step;
                for j in 0..n {
                    let dy = -radius + (j as f64 + 0.5) * step;
                    let off = (dx * dx + dy * dy).sqrt();
                    if off > radius {
                        continue;
                    }
                    // Ray direction = moon_dir tilted by (dx, dy) in (u, v).
                    let ray = normalize([
                        moon_dir[0] + u[0] * dx + v[0] * dy,
                        moon_dir[1] + u[1] * dx + v[1] * dy,
                        moon_dir[2] + u[2] * dx + v[2] * dy,
                    ]);
                    let lit = lunar_phase_lambert(ray, moon_dir, sun_dir, radius);
                    // Foreshorten: a Lambertian sphere fully illuminated
                    // contributes mean brightness 2/3 across the disk (the
                    // hemisphere-averaged value of cos θ_i for the Sun at
                    // the observer's eye), but the *fraction of the disk
                    // that is lit at all* is the textbook (1 + cos α)/2.
                    // We test the latter.
                    if lit > 0.0 {
                        weighted += 1.0;
                    }
                    covered += 1.0;
                }
            }
            let lit_fraction = weighted / covered;
            let expected = 0.5 * (1.0 + phase.cos());
            // 401×401 sampling resolves the terminator to ~1/200 of the
            // disk radius; allow ~1 % absolute tolerance.
            assert!(
                (lit_fraction - expected).abs() < 0.01,
                "phase {phase_deg}°: lit-fraction {lit_fraction} vs expected {expected}"
            );
        }
    }

    /// Ray that misses the disk must return zero.
    #[test]
    fn miss_returns_zero() {
        let moon_dir = [1.0, 0.0, 0.0];
        let sun_dir = [-1.0, 0.0, 0.0];
        let ray = [0.0, 1.0, 0.0]; // 90° away from Moon
        assert_eq!(lunar_phase_lambert(ray, moon_dir, sun_dir, RADIUS), 0.0);
    }

    /// Full Moon (phase = 0): the entire disk must be lit; in particular
    /// the limb on the side opposite the Sun must NOT be lit (which is
    /// what the flipped-normal bug also got wrong).
    #[test]
    fn full_moon_lights_every_disk_point() {
        let moon_dir = [1.0, 0.0, 0.0];
        let sun_dir = [-1.0, 0.0, 0.0]; // Sun behind observer
                                        // Several test rays inside the disk, including off-axis ones.
        let rays = [
            [1.0, 0.0, 0.0],
            normalize([1.0, RADIUS * 0.9, 0.0]),
            normalize([1.0, 0.0, RADIUS * 0.9]),
            normalize([1.0, -RADIUS * 0.5, RADIUS * 0.5]),
        ];
        for ray in rays {
            let lit = lunar_phase_lambert(ray, moon_dir, sun_dir, RADIUS);
            assert!(lit > 0.0, "full-Moon ray {ray:?} came back unlit");
        }
    }
}
