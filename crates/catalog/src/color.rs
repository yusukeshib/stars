/// Convert B−V color index to a physically calibrated display sRGB triple.
///
/// Pipeline:
/// 1. B−V → effective temperature using Ballesteros (2012), Eq. 14:
///    `T = 4600 K · (1/(0.92(B−V)+1.7) + 1/(0.92(B−V)+0.62))`.
/// 2. Integrate a Planck blackbody spectrum through analytic CIE 1931
///    2° colour-matching functions (Wyman, Sloan & Shirley 2013's compact
///    Gaussian fit to the tabulated CIE curves).
/// 3. Convert XYZ → linear sRGB (D65 matrix) and apply the sRGB transfer curve.
///
/// The result is normalized to its brightest channel because catalogue stars
/// carry brightness separately in the renderer; this function supplies chroma.
pub fn bv_to_rgb(bv: f32) -> [f32; 3] {
    let t = bv_to_effective_temperature_k(bv);
    let [x, y, z] = blackbody_to_xyz(t);
    xyz_to_srgb_chroma(x, y, z)
}

fn bv_to_effective_temperature_k(bv: f32) -> f32 {
    let bv = bv.clamp(-0.4, 2.0);
    4600.0 * (1.0 / (0.92 * bv + 1.7) + 1.0 / (0.92 * bv + 0.62))
}

fn blackbody_to_xyz(temp_k: f32) -> [f32; 3] {
    let mut x = 0.0;
    let mut y = 0.0;
    let mut z = 0.0;

    // 5 nm sampling over the visible range is enough for smooth blackbody
    // spectra and keeps this dependency-free for WASM startup.
    let mut wavelength_nm = 380.0;
    while wavelength_nm <= 780.0 {
        let spd = planck_relative(wavelength_nm, temp_k);
        let [cx, cy, cz] = cie_1931_fit(wavelength_nm);
        x += spd * cx;
        y += spd * cy;
        z += spd * cz;
        wavelength_nm += 5.0;
    }

    [x, y, z]
}

fn planck_relative(wavelength_nm: f32, temp_k: f32) -> f32 {
    // Relative spectral radiance. The common 2hc² scale factor cancels during
    // chroma normalization, so only λ^-5 / (exp(c2/(λT))-1) is needed.
    const C2_NM_K: f32 = 1.438_776_9e7;
    let l = wavelength_nm;
    1.0 / (l.powi(5) * ((C2_NM_K / (l * temp_k)).exp() - 1.0))
}

fn cie_1931_fit(wavelength_nm: f32) -> [f32; 3] {
    // Wyman, Sloan & Shirley 2013, "Simple Analytic Approximations to the
    // CIE XYZ Color Matching Functions". Piecewise asymmetric Gaussians.
    let w = wavelength_nm;
    let tx1 = (w - 442.0) * if w < 442.0 { 0.0624 } else { 0.0374 };
    let tx2 = (w - 599.8) * if w < 599.8 { 0.0264 } else { 0.0323 };
    let tx3 = (w - 501.1) * if w < 501.1 { 0.0490 } else { 0.0382 };
    let x = 0.362 * (-0.5 * tx1 * tx1).exp() + 1.056 * (-0.5 * tx2 * tx2).exp()
        - 0.065 * (-0.5 * tx3 * tx3).exp();

    let ty1 = (w - 568.8) * if w < 568.8 { 0.0213 } else { 0.0247 };
    let ty2 = (w - 530.9) * if w < 530.9 { 0.0613 } else { 0.0322 };
    let y = 0.821 * (-0.5 * ty1 * ty1).exp() + 0.286 * (-0.5 * ty2 * ty2).exp();

    let tz1 = (w - 437.0) * if w < 437.0 { 0.0845 } else { 0.0278 };
    let tz2 = (w - 459.0) * if w < 459.0 { 0.0385 } else { 0.0725 };
    let z = 1.217 * (-0.5 * tz1 * tz1).exp() + 0.681 * (-0.5 * tz2 * tz2).exp();

    [x.max(0.0), y.max(0.0), z.max(0.0)]
}

fn xyz_to_srgb_chroma(x: f32, y: f32, z: f32) -> [f32; 3] {
    let r = 3.2406 * x - 1.5372 * y - 0.4986 * z;
    let g = -0.9689 * x + 1.8758 * y + 0.0415 * z;
    let b = 0.0557 * x - 0.2040 * y + 1.0570 * z;

    // Gamut-map by clipping negative channels, then normalize chroma.
    let mut rgb = [r.max(0.0), g.max(0.0), b.max(0.0)];
    let m = rgb[0].max(rgb[1]).max(rgb[2]);
    if m > 0.0 {
        rgb = [rgb[0] / m, rgb[1] / m, rgb[2] / m];
    }

    [
        srgb_encode(rgb[0]),
        srgb_encode(rgb[1]),
        srgb_encode(rgb[2]),
    ]
}

fn srgb_encode(linear: f32) -> f32 {
    if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
    .clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ballesteros_temperature_decreases_with_bv() {
        assert!(bv_to_effective_temperature_k(-0.3) > bv_to_effective_temperature_k(0.65));
        assert!(bv_to_effective_temperature_k(0.65) > bv_to_effective_temperature_k(1.5));
    }

    #[test]
    fn test_white_star() {
        let [r, g, b] = bv_to_rgb(0.0);
        assert!(
            r > 0.75 && g > 0.75 && b > 0.75,
            "Expected whitish: ({r}, {g}, {b})"
        );
    }

    #[test]
    fn test_blue_star() {
        let [r, _g, b] = bv_to_rgb(-0.3);
        assert!(b > r, "Expected blue > red for B-V=-0.3: ({r}, {b})");
    }

    #[test]
    fn test_red_star() {
        let [r, _g, b] = bv_to_rgb(1.5);
        assert!(
            r > 0.9 && b < 0.65,
            "Expected red star for B-V=1.5: ({r}, {b})"
        );
    }
}
