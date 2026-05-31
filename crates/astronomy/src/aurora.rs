//! V-48 aurora model: auroral-oval geometry, geomagnetic coordinates, and the
//! statistically-expected apparent arc an observer sees for a supplied Kp.
//!
//! # Scope
//!
//! This module provides the *statistically expected* auroral-oval position and
//! brightness for a geomagnetic-activity (Kp) input. It deliberately does **not**
//! model real-time auroral morphology (curtains, rays, dynamic substorm motion)
//! — see the ROADMAP `V-48` "deliberate non-goal scope". The renderer paints a
//! green O I 557.7 nm arc, a higher-altitude red O I 630.0 nm band, and a
//! magenta N₂ lower border, positioned from the geometry computed here.
//!
//! # Model
//!
//! 1. **Oval boundary.** [`auroral_oval_boundary`] returns the equatorward and
//!    poleward edges of the auroral oval in corrected geomagnetic latitude as a
//!    function of Kp, calibrated to the Feldstein & Starkov 1967 midnight oval
//!    (equatorward boundary ≈ 63° at Kp = 4).
//! 2. **Geomagnetic coordinates.** [`geomagnetic_latitude_deg`] uses a centered
//!    dipole (IGRF-13 2020 north geomagnetic pole) — a first-order corrected
//!    geomagnetic latitude. Full AACGM is intentionally not implemented; the
//!    dipole is sufficient for naked-eye oval placement and keeps the WASM build
//!    small (documented in `VALIDATION.md`).
//! 3. **Apparent arc.** [`aurora_view`] projects the discrete equatorward arc
//!    (the bright edge) and its O I red curtain onto the observer's local sky
//!    using the standard elevation-of-an-elevated-source relation
//!    ([`emission_apparent_altitude_rad`]). A sub-auroral observer sees a low arc
//!    toward the geomagnetic pole; an observer under the oval sees it overhead /
//!    equatorward.
//!
//! # References
//! - Feldstein, Y. I., Starkov, G. V. 1967, Planet. Space Sci. 15, 209.
//! - Akasofu, S.-I. 1964, Planet. Space Sci. 12, 273 (substorm phases).
//! - Chamberlain, J. W. 1961, *Physics of the Aurora and Airglow* (emission
//!   heights / colours).
//! - Newell, P. T. et al. 2010, JGR 115, A03216 (OVATION Prime).

use std::f64::consts::PI;

/// Geographic latitude of the IGRF-13 (epoch 2020.0) north geomagnetic
/// (centered-dipole) pole, degrees.
pub const GEOMAGNETIC_NORTH_POLE_LAT_DEG: f64 = 80.65;
/// Geographic longitude of the IGRF-13 (epoch 2020.0) north geomagnetic
/// (centered-dipole) pole, degrees east (287.5°E ≡ −72.5°E).
pub const GEOMAGNETIC_NORTH_POLE_LON_DEG: f64 = -72.50;

/// Mean Earth radius (km) for the apparent-altitude geometry.
pub const EARTH_MEAN_RADIUS_KM: f64 = 6371.0;

/// Peak emission height of the O I 557.7 nm green line (km). Chamberlain 1961.
pub const AURORA_GREEN_HEIGHT_KM: f64 = 110.0;
/// Peak emission height of the O I 630.0 nm red line (km) — the high-altitude
/// upper border of the curtain.
pub const AURORA_RED_HEIGHT_KM: f64 = 230.0;
/// Peak emission height of the N₂⁺ / N₂ magenta lower border (km).
pub const AURORA_N2_HEIGHT_KM: f64 = 95.0;

const DEG_TO_RAD: f64 = PI / 180.0;

/// Season of observation, used for the small seasonal shift of the oval and a
/// modest brightness/visibility weighting (dark winter nights show fainter
/// aurora more readily). The boundary location is dominated by Kp; the seasonal
/// term is ≤ 0.5° so it never disturbs the Kp-anchored pinned test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuroraSeason {
    /// Local winter (long dark nights).
    Winter,
    /// Equinox — the reference season the boundary fit is anchored to.
    #[default]
    Equinox,
    /// Local summer (short nights).
    Summer,
}

impl AuroraSeason {
    /// Equatorward-boundary seasonal offset (degrees). Kept small and centred
    /// on 0 at equinox so the pinned Kp=4 → 63° test holds exactly.
    fn equatorward_offset_deg(self) -> f64 {
        match self {
            AuroraSeason::Winter => -0.5,
            AuroraSeason::Equinox => 0.0,
            AuroraSeason::Summer => 0.5,
        }
    }

    /// Relative dark-sky visibility weight applied to the emission intensity.
    fn brightness_weight(self) -> f64 {
        match self {
            AuroraSeason::Winter => 1.1,
            AuroraSeason::Equinox => 1.0,
            AuroraSeason::Summer => 0.85,
        }
    }

    /// Pick a season from a UTC day-of-year for a northern-hemisphere observer
    /// (the auroral oval that matters here is the northern one). Southern-
    /// hemisphere callers can pass the complementary season explicitly.
    pub fn from_day_of_year_north(day_of_year: u32) -> Self {
        // Centre winter on the December/January solstice, summer on June/July.
        match day_of_year {
            d if !(80..=265).contains(&d) => AuroraSeason::Winter,
            d if (140..=205).contains(&d) => AuroraSeason::Summer,
            _ => AuroraSeason::Equinox,
        }
    }
}

/// Equatorward and poleward boundaries of the auroral oval in corrected
/// geomagnetic latitude (degrees), as a function of the planetary Kp index.
///
/// Calibrated to the Feldstein & Starkov 1967 midnight oval: the equatorward
/// boundary is ≈ 63° at Kp = 4 and expands equatorward with rising activity.
/// Returns `(equatorward_lat_deg, poleward_lat_deg)` with
/// `poleward > equatorward`.
pub fn auroral_oval_boundary(kp: f64, season: AuroraSeason) -> (f64, f64) {
    let kp = kp.clamp(0.0, 9.0);
    let equatorward = 67.0 - 1.0 * kp + season.equatorward_offset_deg();
    let poleward = 71.5 - 0.5 * kp + season.equatorward_offset_deg();
    (equatorward, poleward)
}

/// Centered-dipole corrected geomagnetic latitude (degrees) of a geographic
/// position, using the IGRF-13 2020 north geomagnetic pole.
pub fn geomagnetic_latitude_deg(geographic_lat_deg: f64, geographic_lon_deg: f64) -> f64 {
    let lp = GEOMAGNETIC_NORTH_POLE_LAT_DEG * DEG_TO_RAD;
    let phip = GEOMAGNETIC_NORTH_POLE_LON_DEG * DEG_TO_RAD;
    let l = geographic_lat_deg * DEG_TO_RAD;
    let phi = geographic_lon_deg * DEG_TO_RAD;
    let sin_lam = l.sin() * lp.sin() + l.cos() * lp.cos() * (phi - phip).cos();
    sin_lam.clamp(-1.0, 1.0).asin() / DEG_TO_RAD
}

/// Initial great-circle bearing (radians, from local north toward east, in
/// `[0, 2π)`) from a geographic position to the north geomagnetic pole. This is
/// the azimuth toward which the poleward part of the oval lies.
pub fn bearing_to_geomagnetic_pole_rad(geographic_lat_deg: f64, geographic_lon_deg: f64) -> f64 {
    let lp = GEOMAGNETIC_NORTH_POLE_LAT_DEG * DEG_TO_RAD;
    let phip = GEOMAGNETIC_NORTH_POLE_LON_DEG * DEG_TO_RAD;
    let l = geographic_lat_deg * DEG_TO_RAD;
    let dphi = phip - geographic_lon_deg * DEG_TO_RAD;
    let y = dphi.sin() * lp.cos();
    let x = l.cos() * lp.sin() - l.sin() * lp.cos() * dphi.cos();
    y.atan2(x).rem_euclid(2.0 * PI)
}

/// Apparent elevation (radians, above the astronomical horizon) of a luminous
/// point at height `height_km` above the Earth's surface, seen across an
/// Earth-central ground angle `central_angle_rad`.
///
/// Uses the standard relation `tan E = (cos γ − R/(R+h)) / sin γ`. At `γ = 0`
/// the point is overhead (`E = π/2`); `E` falls to 0 at the geometric horizon
/// distance and is negative beyond it.
pub fn emission_apparent_altitude_rad(central_angle_rad: f64, height_km: f64) -> f64 {
    let gamma = central_angle_rad.abs();
    let rh = EARTH_MEAN_RADIUS_KM / (EARTH_MEAN_RADIUS_KM + height_km.max(0.0));
    let num = gamma.cos() - rh;
    let den = gamma.sin();
    if den.abs() < 1e-9 {
        // Directly overhead.
        return PI / 2.0;
    }
    num.atan2(den)
}

/// Statistically-expected emission intensity for a Kp index, in `[0, 1]`.
///
/// A smoothstep onset: naked-eye aurora is rare below Kp ≈ 2 even under the
/// oval and saturates toward high activity. Scaled by the seasonal dark-sky
/// visibility weight.
pub fn aurora_intensity(kp: f64, season: AuroraSeason) -> f64 {
    let kp = kp.clamp(0.0, 9.0);
    let s = ((kp - 1.0) / 6.0).clamp(0.0, 1.0);
    let smooth = s * s * (3.0 - 2.0 * s);
    (smooth * season.brightness_weight()).clamp(0.0, 1.0)
}

/// Apparent aurora arc geometry in the observer's local horizontal frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuroraView {
    /// True when an above-horizon, non-zero-intensity arc is expected.
    pub visible: bool,
    /// Azimuth of the arc (radians, from north toward east).
    pub center_azimuth_rad: f64,
    /// Apparent altitude (radians) of the green discrete arc (its O I 557.7 nm
    /// base at ≈ 110 km).
    pub center_altitude_rad: f64,
    /// Apparent vertical extent (radians) of the curtain — the rise from the
    /// green base to the O I 630.0 nm red top at ≈ 230 km over the same arc
    /// footprint.
    pub vertical_extent_rad: f64,
    /// Half-width of the arc in azimuth (radians); the oval spans a broad range
    /// of geomagnetic longitudes so the rendered arc tapers over this span.
    pub azimuth_half_width_rad: f64,
    /// Emission intensity in `[0, 1]` ([`aurora_intensity`]).
    pub intensity: f64,
}

/// Compute the statistically-expected apparent aurora arc for a northern-
/// hemisphere geographic observer and a supplied Kp index.
///
/// The discrete bright arc is placed at the equatorward oval boundary and the
/// red curtain rises poleward/upward from it. When the observer is poleward of
/// the boundary the arc appears toward the geomagnetic pole; when equatorward,
/// it appears low toward the pole-ward horizon and vanishes once the boundary
/// drops below the local horizon.
pub fn aurora_view(
    geographic_lat_deg: f64,
    geographic_lon_deg: f64,
    kp: f64,
    season: AuroraSeason,
) -> AuroraView {
    let intensity = aurora_intensity(kp, season);
    let (equatorward, _poleward) = auroral_oval_boundary(kp, season);
    let geomag_lat = geomagnetic_latitude_deg(geographic_lat_deg, geographic_lon_deg);
    let pole_bearing = bearing_to_geomagnetic_pole_rad(geographic_lat_deg, geographic_lon_deg);

    // Signed ground angle (poleward positive) from the observer to the discrete
    // equatorward arc, along the geomagnetic meridian.
    let gamma_deg = equatorward - geomag_lat;
    let (azimuth, central_deg) = if gamma_deg >= 0.0 {
        (pole_bearing, gamma_deg)
    } else {
        ((pole_bearing + PI).rem_euclid(2.0 * PI), -gamma_deg)
    };
    let central = central_deg * DEG_TO_RAD;

    let el_green = emission_apparent_altitude_rad(central, AURORA_GREEN_HEIGHT_KM);
    let el_red = emission_apparent_altitude_rad(central, AURORA_RED_HEIGHT_KM);
    let vertical_extent = (el_red - el_green).abs().max(4.0 * DEG_TO_RAD);

    // Visible if some part of the curtain (green base up to the red top) is
    // above the horizon and the activity is high enough to register.
    let visible = intensity > 0.0 && el_red > 0.0;

    AuroraView {
        visible,
        center_azimuth_rad: azimuth,
        center_altitude_rad: el_green,
        vertical_extent_rad: vertical_extent,
        azimuth_half_width_rad: 75.0 * DEG_TO_RAD,
        intensity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equatorward_boundary_at_kp4_is_63_degrees() {
        // ROADMAP V-48 pinned validation: Kp=4 equatorward boundary at
        // corrected geomagnetic latitude ≈ 63° within 1°.
        let (eq, pole) = auroral_oval_boundary(4.0, AuroraSeason::Equinox);
        assert!((eq - 63.0).abs() < 1.0, "equatorward {eq} should be ≈63°");
        assert!(pole > eq, "poleward {pole} must exceed equatorward {eq}");
    }

    #[test]
    fn oval_expands_equatorward_with_activity() {
        let (eq_quiet, _) = auroral_oval_boundary(1.0, AuroraSeason::Equinox);
        let (eq_storm, _) = auroral_oval_boundary(8.0, AuroraSeason::Equinox);
        assert!(
            eq_storm < eq_quiet,
            "higher Kp must push the equatorward boundary to lower latitude: \
             quiet {eq_quiet}, storm {eq_storm}"
        );
    }

    #[test]
    fn season_shift_is_small() {
        let (eq_w, _) = auroral_oval_boundary(4.0, AuroraSeason::Winter);
        let (eq_s, _) = auroral_oval_boundary(4.0, AuroraSeason::Summer);
        assert!((eq_w - eq_s).abs() <= 1.0, "seasonal shift must stay ≤ 1°");
    }

    #[test]
    fn geomagnetic_latitude_for_tromso_is_high() {
        // Tromsø, Norway (69.65°N, 18.96°E) sits near corrected geomagnetic
        // latitude ~67°.
        let lam = geomagnetic_latitude_deg(69.65, 18.96);
        assert!((lam - 67.5).abs() < 1.5, "Tromsø geomag lat {lam} ≈ 67°");
    }

    #[test]
    fn overhead_emission_is_at_zenith() {
        let el = emission_apparent_altitude_rad(0.0, AURORA_GREEN_HEIGHT_KM);
        assert!((el - PI / 2.0).abs() < 1e-6, "γ=0 must be the zenith");
    }

    #[test]
    fn emission_drops_below_horizon_with_distance() {
        // Beyond the geometric horizon distance the elevation goes negative.
        let near = emission_apparent_altitude_rad(2.0 * DEG_TO_RAD, AURORA_GREEN_HEIGHT_KM);
        let far = emission_apparent_altitude_rad(20.0 * DEG_TO_RAD, AURORA_GREEN_HEIGHT_KM);
        assert!(near > 0.0, "a 2° distant arc is above the horizon");
        assert!(far < 0.0, "a 20° distant 110 km arc is below the horizon");
        assert!(near > far);
    }

    #[test]
    fn red_curtain_appears_higher_than_green_base() {
        // Over the same ground footprint, the 230 km red emission appears at a
        // higher apparent altitude than the 110 km green base.
        let central = 4.0 * DEG_TO_RAD;
        let g = emission_apparent_altitude_rad(central, AURORA_GREEN_HEIGHT_KM);
        let r = emission_apparent_altitude_rad(central, AURORA_RED_HEIGHT_KM);
        assert!(r > g, "red top {r} should sit above green base {g}");
    }

    #[test]
    fn intensity_is_monotonic_and_bounded() {
        let lo = aurora_intensity(1.0, AuroraSeason::Equinox);
        let mid = aurora_intensity(5.0, AuroraSeason::Equinox);
        let hi = aurora_intensity(9.0, AuroraSeason::Equinox);
        assert!((0.0..=1.0).contains(&lo));
        assert!((0.0..=1.0).contains(&hi));
        assert!(lo <= mid && mid <= hi, "intensity must rise with Kp");
        assert_eq!(aurora_intensity(0.0, AuroraSeason::Equinox), 0.0);
    }

    #[test]
    fn subauroral_observer_sees_low_arc_toward_the_pole() {
        // A geomagnetic-latitude ~58° observer at Kp=5 (oval eq edge ~62°) sees
        // a low arc toward the geomagnetic pole.
        // Pick a site near 60°N, 10°E (southern Norway), geomag lat ~58–59°.
        let view = aurora_view(60.0, 10.0, 5.0, AuroraSeason::Equinox);
        assert!(
            view.visible,
            "Kp=5 aurora should be visible from ~58° geomag"
        );
        assert!(
            view.center_altitude_rad > 0.0 && view.center_altitude_rad < 35.0 * DEG_TO_RAD,
            "expected a low arc, got {} deg",
            view.center_altitude_rad / DEG_TO_RAD
        );
        assert!(view.intensity > 0.0);
    }

    #[test]
    fn quiet_low_latitude_shows_no_aurora() {
        // Tokyo (35.68°N, 139.69°E) at Kp=2 — far equatorward of the oval.
        let view = aurora_view(35.68, 139.69, 2.0, AuroraSeason::Equinox);
        assert!(!view.visible, "no naked-eye aurora over Tokyo at Kp=2");
    }
}
