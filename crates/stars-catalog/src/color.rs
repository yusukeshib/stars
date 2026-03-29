/// Convert B-V color index to an approximate RGB color.
///
/// Uses a piecewise linear approximation of the blackbody color sequence.
/// B-V ranges roughly from -0.4 (hot blue) to 2.0 (cool red).
pub fn bv_to_rgb(bv: f32) -> [f32; 3] {
    let bv = bv.clamp(-0.4, 2.0);

    let r;
    let g;
    let b;

    // Red channel
    if bv < 0.0 {
        r = 0.61 + 0.11 * bv + 0.1 * bv * bv;
    } else if bv < 0.4 {
        r = 0.83 + 0.17 * bv;
    } else {
        r = 1.0;
    }

    // Green channel
    if bv < 0.0 {
        g = 0.70 + 0.07 * bv + 0.1 * bv * bv;
    } else if bv < 0.4 {
        g = 0.87 + 0.11 * bv;
    } else if bv < 1.6 {
        g = 1.0 - 0.47 * (bv - 0.4);
    } else {
        g = 0.44;
    }

    // Blue channel
    if bv < -0.1 {
        b = 1.0;
    } else if bv < 0.5 {
        b = 1.0 - 1.67 * (bv + 0.1);
    } else {
        b = 0.0;
    }

    [r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_white_star() {
        // B-V ≈ 0.0 should be roughly white
        let [r, g, b] = bv_to_rgb(0.0);
        assert!(r > 0.7 && g > 0.7 && b > 0.7, "Expected whitish: ({r}, {g}, {b})");
    }

    #[test]
    fn test_blue_star() {
        // B-V = -0.3 should be bluish
        let [r, _g, b] = bv_to_rgb(-0.3);
        assert!(b > r, "Expected blue > red for B-V=-0.3");
    }

    #[test]
    fn test_red_star() {
        // B-V = 1.5 should be reddish
        let [r, _g, b] = bv_to_rgb(1.5);
        assert!(r > 0.9 && b < 0.1, "Expected red star for B-V=1.5");
    }
}
