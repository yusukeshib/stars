//! V-46 Galactic structural model for the external galactic viewpoint.
//!
//! The external Phase-4 viewpoint (`V-41` / `V-44`) used to draw a single
//! analytic thin Milky Way disc. This module is the *reference* implementation
//! of a Drimmel & Spergel 2001 style multi-component model — a thin disk, a
//! thick disk, a triaxial (boxy) bar/bulge, and a four-arm Reid 2019 log-spiral
//! enhancement — plus a double-exponential dust screen. The same constants are
//! mirrored in `crates/renderer/src/shaders/skyglow.wgsl`
//! (`external_galaxy_volume_radiance`) which ray-marches these functions to
//! shade the external map; the unit tests here pin the model so the shader copy
//! cannot silently drift.
//!
//! # Coordinate frame
//!
//! All positions are **galactocentric** IAU-galactic Cartesian parsecs with the
//! origin at the Galactic Centre, the Sun on the `+x` axis at galactocentric
//! radius [`R_SUN_PC`], `+z` toward the North Galactic Pole, and galactocentric
//! azimuth `phi = atan2(y, x)` increasing from the Sun toward `l = 90°`. This
//! is the same world frame the renderer's external viewpoint draws in (world
//! origin = Galactic Centre, `z` = height above the midplane).
//!
//! # Units
//!
//! [`milky_way_luminosity_density`] returns a stellar **mass** density in
//! `M_sun pc^-3` (a defensible proxy for luminosity density at the level this
//! visual model needs); [`dust_extinction_az`] returns a line-of-sight visual
//! extinction in magnitudes.
//!
//! # References
//! - Drimmel, R., Spergel, D. N. 2001, ApJ 556, 181 — three-component disk +
//!   two-component dust fit to COBE/DIRBE + 2MASS.
//! - Reid, M. J. et al. 2019, ApJ 885, 131 — BeSSeL maser-parallax arm traces
//!   (pitch angles, reference radii/azimuths).
//! - Robitaille, T. P. 2017, A&A 600, A11 — `mwdust` (dust-screen reference).
//! - Bland-Hawthorn, J., Gerhard, O. 2016, ARA&A 54, 529 — structural review
//!   (R_sun = 8.2 kpc, z_sun ≈ 25 pc, local stellar density ≈ 0.1 M_sun/pc^3).

use std::f64::consts::PI;

/// Galactocentric solar radius (pc), IAU 2018 / GRAVITY 2019 value.
pub const R_SUN_PC: f64 = 8122.0;
/// Solar height above the Galactic midplane (pc), Bennett & Bovy 2019.
pub const Z_SUN_PC: f64 = 20.8;

// --- Thin disk -------------------------------------------------------------
/// Thin-disk central mass density (M_sun/pc^3); calibrated with the scale
/// lengths and the Local-arm enhancement (the Sun sits in the Local arm) so
/// the solar-neighbourhood total lands at ≈ 0.1 M_sun/pc^3.
const THIN_RHO0: f64 = 1.243;
/// Thin-disk radial scale length (pc).
const THIN_H_R_PC: f64 = 2600.0;
/// Thin-disk vertical scale height (pc).
const THIN_H_Z_PC: f64 = 300.0;

// --- Thick disk ------------------------------------------------------------
/// Thick-disk central mass density (M_sun/pc^3).
const THICK_RHO0: f64 = 0.039;
/// Thick-disk radial scale length (pc).
const THICK_H_R_PC: f64 = 3600.0;
/// Thick-disk vertical scale height (pc).
const THICK_H_Z_PC: f64 = 900.0;

// --- Bar / bulge (triaxial boxy exponential) -------------------------------
/// Bar central mass density (M_sun/pc^3).
const BAR_RHO0: f64 = 95.0;
/// Bar scale lengths along its principal axes (pc): major, minor, vertical.
const BAR_A_PC: f64 = 1700.0;
const BAR_B_PC: f64 = 700.0;
const BAR_C_PC: f64 = 500.0;
/// Bar position angle relative to the Sun–Galactic-Centre line (radians).
/// Reid 2019 / Wegg 2015: the long axis leads the Sun line by ≈ 27°.
const BAR_ANGLE_RAD: f64 = 0.4712; // 27°
/// Truncation radius of the bar (pc); beyond this the bar term is zero.
const BAR_R_TRUNC_PC: f64 = 3500.0;

// --- Spiral arms (Reid 2019 log-spiral traces) -----------------------------
/// Cross-arm Gaussian half-width (pc) over which the arm enhancement falls.
const ARM_WIDTH_PC: f64 = 350.0;
/// Peak fractional density enhancement on an arm ridge line.
const ARM_AMPLITUDE: f64 = 0.9;

/// One Reid et al. 2019 log-spiral arm trace. The ridge line follows
/// `ln(R/R_ref) = -(phi - phi_ref) * tan(pitch)`, i.e. radius grows with
/// azimuth at the given pitch angle. `phi_ref` is the galactocentric azimuth
/// (radians, our `+x`=Sun convention) at which the arm passes `r_ref_pc`.
#[derive(Debug, Clone, Copy)]
pub struct SpiralArm {
    pub name: &'static str,
    pub r_ref_pc: f64,
    pub phi_ref_rad: f64,
    pub pitch_rad: f64,
}

/// The four dominant arms traced by Reid et al. 2019 (representative ridge
/// anchors). Azimuths are expressed in this module's galactocentric frame
/// (Sun at `phi = 0`), chosen so the Sagittarius-Carina and Perseus arms fall
/// on the correct (inner / outer) side of the Sun.
pub const SPIRAL_ARMS: [SpiralArm; 4] = [
    SpiralArm {
        name: "Scutum-Centaurus",
        r_ref_pc: 5100.0,
        phi_ref_rad: 0.4712, // ~27°
        pitch_rad: 0.2112,   // 12.1°
    },
    SpiralArm {
        name: "Sagittarius-Carina",
        r_ref_pc: 6600.0,
        phi_ref_rad: -0.4014, // ~-23°
        pitch_rad: 0.2234,    // 12.8°
    },
    SpiralArm {
        name: "Local",
        r_ref_pc: 8200.0,
        phi_ref_rad: 0.0873, // ~5°
        pitch_rad: 0.1763,   // 10.1°
    },
    SpiralArm {
        name: "Perseus",
        r_ref_pc: 9900.0,
        phi_ref_rad: std::f64::consts::FRAC_PI_6, // 30°
        pitch_rad: 0.1728,                        // 9.9°
    },
];

// --- Dust screen (double-exponential) --------------------------------------
/// Local in-plane visual extinction gradient (mag/pc) at the Sun.
const DUST_K0_MAG_PER_PC: f64 = 0.0011; // ≈ 1.1 mag/kpc near the plane
/// Dust radial scale length (pc).
const DUST_H_R_PC: f64 = 3000.0;
/// Dust vertical scale height (pc) — dust hugs the plane more tightly than
/// stars (Drimmel & Spergel 2001).
const DUST_H_Z_PC: f64 = 120.0;

fn sech2(x: f64) -> f64 {
    let c = x.cosh();
    1.0 / (c * c)
}

/// Stellar mass density (M_sun/pc^3) at a galactocentric point, summing the
/// thin disk, thick disk, triaxial bar, and the spiral-arm enhancement applied
/// to the thin disk.
pub fn milky_way_luminosity_density(x_pc: f64, y_pc: f64, z_pc: f64) -> f64 {
    let r = (x_pc * x_pc + y_pc * y_pc).sqrt();

    // Thin disk with a sech^2 vertical profile and a log-spiral arm boost.
    let thin_smooth = THIN_RHO0 * (-r / THIN_H_R_PC).exp() * sech2(z_pc / (2.0 * THIN_H_Z_PC));
    let thin = thin_smooth * (1.0 + spiral_arm_enhancement(x_pc, y_pc));

    // Thick disk, exponential in both R and |z|.
    let thick = THICK_RHO0 * (-r / THICK_H_R_PC).exp() * (-(z_pc.abs()) / THICK_H_Z_PC).exp();

    thin + thick + bar_density(x_pc, y_pc, z_pc)
}

/// Triaxial boxy bar/bulge density (M_sun/pc^3). The bar long axis is rotated
/// by [`BAR_ANGLE_RAD`] in the plane; a generalised (boxy) exponential gives
/// the peanut-shaped isodensity contours.
fn bar_density(x_pc: f64, y_pc: f64, z_pc: f64) -> f64 {
    let r = (x_pc * x_pc + y_pc * y_pc).sqrt();
    if r > BAR_R_TRUNC_PC {
        return 0.0;
    }
    let (s, c) = BAR_ANGLE_RAD.sin_cos();
    // Rotate into the bar frame.
    let xb = x_pc * c + y_pc * s;
    let yb = -x_pc * s + y_pc * c;
    let m = (xb / BAR_A_PC).powi(2) + (yb / BAR_B_PC).powi(2) + (z_pc / BAR_C_PC).powi(2);
    BAR_RHO0 * (-m.sqrt()).exp()
}

/// Fractional thin-disk density enhancement from the nearest spiral arm. Each
/// arm contributes a Gaussian in cross-arm (radial) distance from its
/// log-spiral ridge line; the maximum over the arms is returned.
pub fn spiral_arm_enhancement(x_pc: f64, y_pc: f64) -> f64 {
    let r = (x_pc * x_pc + y_pc * y_pc).sqrt();
    if r < 1000.0 {
        return 0.0;
    }
    let phi = y_pc.atan2(x_pc);
    let mut best = 0.0_f64;
    for arm in &SPIRAL_ARMS {
        // Radius of the arm ridge at this azimuth, scanning the nearest
        // winding +/- one turn so the spiral matches over the full disk.
        for turn in -1..=1 {
            let dphi = phi - arm.phi_ref_rad + (turn as f64) * 2.0 * PI;
            let r_arm = arm.r_ref_pc * (-dphi * arm.pitch_rad.tan()).exp();
            if !(1000.0..=20000.0).contains(&r_arm) {
                continue;
            }
            let d = (r - r_arm).abs();
            let g = ARM_AMPLITUDE * (-(d * d) / (2.0 * ARM_WIDTH_PC * ARM_WIDTH_PC)).exp();
            if g > best {
                best = g;
            }
        }
    }
    best
}

/// Line-of-sight visual extinction (magnitudes) from the Sun out to
/// `distance_pc` toward Galactic coordinates `(l, b)`, integrating a
/// double-exponential dust disk. `A` grows with distance, is largest in the
/// plane (`b = 0`), and is zero at zero distance.
pub fn dust_extinction_az(distance_pc: f64, l_rad: f64, b_rad: f64) -> f64 {
    if distance_pc <= 0.0 {
        return 0.0;
    }
    let (sb, cb) = b_rad.sin_cos();
    let (sl, cl) = l_rad.sin_cos();
    // Step along the line of sight, accumulating local dust density.
    let n = 64usize;
    let step = distance_pc / n as f64;
    let mut a = 0.0;
    for i in 0..n {
        let s = (i as f64 + 0.5) * step;
        // Heliocentric -> galactocentric Cartesian. Sun at (R_SUN, 0, Z_SUN);
        // +x toward GC means the GC is at -l direction, so x decreases toward
        // l = 0. We only need R and z for the axisymmetric dust disk.
        let x = R_SUN_PC - s * cb * cl;
        let y = -s * cb * sl;
        let z = Z_SUN_PC + s * sb;
        let r = (x * x + y * y).sqrt();
        let dens = (-(r - R_SUN_PC) / DUST_H_R_PC).exp() * (-(z.abs()) / DUST_H_Z_PC).exp();
        a += DUST_K0_MAG_PER_PC * dens * step;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solar_neighbourhood_density_is_about_point_one() {
        // ROADMAP V-46 pin: at the IAU 2018 solar position the local stellar
        // mass density is ≈ 0.1 M_sun/pc^3 (Bland-Hawthorn & Gerhard 2016).
        let rho = milky_way_luminosity_density(R_SUN_PC, 0.0, Z_SUN_PC);
        assert!(
            (rho - 0.1).abs() < 0.03,
            "solar density {rho} should be ≈ 0.1 M_sun/pc^3"
        );
    }

    #[test]
    fn density_falls_with_height_and_radius() {
        let midplane = milky_way_luminosity_density(R_SUN_PC, 0.0, 0.0);
        let high = milky_way_luminosity_density(R_SUN_PC, 0.0, 1500.0);
        assert!(midplane > high, "midplane {midplane} > 1.5 kpc up {high}");
        let inner = milky_way_luminosity_density(4000.0, 0.0, 0.0);
        let outer = milky_way_luminosity_density(14000.0, 0.0, 0.0);
        assert!(inner > outer, "inner disk {inner} > outer disk {outer}");
    }

    #[test]
    fn bar_dominates_the_centre() {
        let centre = milky_way_luminosity_density(0.0, 0.0, 0.0);
        let solar = milky_way_luminosity_density(R_SUN_PC, 0.0, Z_SUN_PC);
        assert!(
            centre > 50.0 * solar,
            "Galactic centre {centre} should vastly exceed solar {solar}"
        );
    }

    #[test]
    fn spiral_arms_raise_density_over_interarm() {
        // Sample the Sagittarius-Carina ridge and an inter-arm point at the
        // same radius; the on-arm density must be higher.
        let arm = SPIRAL_ARMS[1];
        let phi = 0.0; // toward the Sun azimuth
        let dphi = phi - arm.phi_ref_rad;
        let r_arm = arm.r_ref_pc * (-dphi * arm.pitch_rad.tan()).exp();
        let on_arm = spiral_arm_enhancement(r_arm * phi.cos(), r_arm * phi.sin());
        // A point one full arm-width inward at the same azimuth.
        let r_inter = r_arm - 4.0 * ARM_WIDTH_PC;
        let inter = spiral_arm_enhancement(r_inter * phi.cos(), r_inter * phi.sin());
        assert!(
            on_arm > inter + 0.3,
            "on-arm enhancement {on_arm} should exceed interarm {inter}"
        );
    }

    #[test]
    fn dust_extinction_monotonic_and_plane_concentrated() {
        // Zero distance -> zero extinction.
        assert_eq!(dust_extinction_az(0.0, 0.0, 0.0), 0.0);
        // Monotone increasing with distance in the plane toward the GC.
        let near = dust_extinction_az(500.0, 0.0, 0.0);
        let far = dust_extinction_az(3000.0, 0.0, 0.0);
        assert!(far > near && near > 0.0, "dust must grow: {near} -> {far}");
        // In-plane extinction exceeds a steep out-of-plane sightline.
        let plane = dust_extinction_az(2000.0, 0.0, 0.0);
        let pole = dust_extinction_az(2000.0, 0.0, 60_f64.to_radians());
        assert!(plane > pole, "in-plane {plane} > out-of-plane {pole}");
        // Local in-plane gradient is order ~1 mag/kpc.
        let one_kpc = dust_extinction_az(1000.0, 90_f64.to_radians(), 0.0);
        assert!(
            (0.3..=3.0).contains(&one_kpc),
            "local 1 kpc extinction {one_kpc} mag out of expected range"
        );
    }
}
