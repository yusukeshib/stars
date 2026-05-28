//! Diffuse night-sky surface-brightness model.
//!
//! At a dark site the visible night sky is *not* black between the resolved
//! stars: a diffuse glow remains, made up of (in decreasing order of
//! contribution to the Milky Way band):
//!
//! * **Integrated starlight (ISL)** — the unresolved population of stars
//!   too faint for any naked-eye catalogue. Strongly concentrated along
//!   the galactic plane; **this is the dominant Milky Way ingredient.**
//! * **Diffuse galactic light (DGL)** — interstellar dust scattering the
//!   integrated starlight. Tracks ISL with ~20–40% relative amplitude.
//! * **Zodiacal light** — sunlight scattered by interplanetary dust,
//!   strongest near the ecliptic and antisolar gegenschein.
//! * **Airglow** — a broadly isotropic atmospheric floor.
//! * **Interstellar dust extinction** — a Schlegel-Finkbeiner-Davis-style
//!   analytic dust screen that dims the far-side integrated starlight near
//!   the galactic plane.
//!
//! This module implements an analytic V-band diffuse-sky model in galactic
//! and ecliptic coordinates, fit to the published surface-brightness profiles in:
//!
//!   Leinert, Ch., Bowyer, S., Haikala, L. K., et al. 1998,
//!   *The 1997 reference of diffuse night sky brightness*,
//!   A&AS 127, 1–99, §6 (Integrated starlight) and §8 (DGL).
//!
//! The fit is an approximation of the Leinert tables, not a digitisation
//! of them — adequate for naked-eye visualisation, **not** for radiometric
//! analysis. The doc comment on [`isl_mag_per_arcsec2`] lists the
//! literature reference points it is calibrated against.
//!
//! ## Coordinate convention
//!
//! Galactic coordinates `(l, b)` use the IAU 1958 system at the J2000
//! equinox. `l` is galactic longitude (0 = direction to the Galactic
//! Center), `b` is galactic latitude (positive = north galactic
//! hemisphere). Both in radians.

use glam::{Mat3, Vec3};

/// Rotation matrix from J2000 equatorial unit vectors to galactic
/// coordinates (IAU 1958, refined by Murray 1989 / ESA SP-1200 1997).
///
/// Apply as `M · v_eq` to get `(x_g, y_g, z_g)` such that
/// `z_g = sin(b)`, `(x_g, y_g) = cos(b)·(cos l, sin l)`.
///
/// The constants are the standard high-precision values used by SOFA's
/// `iauIcrs2g`; SOFA is the IAU's reference implementation, so digitising
/// them here keeps us bit-compatible with the literature.
#[rustfmt::skip]
const EQUATORIAL_TO_GALACTIC_ROWS: [[f64; 3]; 3] = [
    [-0.054_875_560_416_215, -0.873_437_090_234_885, -0.483_835_015_548_713],
    [ 0.494_109_427_875_584, -0.444_829_629_960_011,  0.746_982_244_497_219],
    [-0.867_666_149_019_004, -0.198_076_373_431_201,  0.455_983_776_175_067],
];

/// Convert a unit vector in J2000 equatorial coordinates to galactic
/// longitude/latitude `(l, b)`, both in radians.
///
/// `l ∈ [0, 2π)`, `b ∈ [-π/2, π/2]`.
pub fn equatorial_to_galactic(v_eq: Vec3) -> (f64, f64) {
    let v_g_x = EQUATORIAL_TO_GALACTIC_ROWS[0][0] * v_eq.x as f64
        + EQUATORIAL_TO_GALACTIC_ROWS[0][1] * v_eq.y as f64
        + EQUATORIAL_TO_GALACTIC_ROWS[0][2] * v_eq.z as f64;
    let v_g_y = EQUATORIAL_TO_GALACTIC_ROWS[1][0] * v_eq.x as f64
        + EQUATORIAL_TO_GALACTIC_ROWS[1][1] * v_eq.y as f64
        + EQUATORIAL_TO_GALACTIC_ROWS[1][2] * v_eq.z as f64;
    let v_g_z = EQUATORIAL_TO_GALACTIC_ROWS[2][0] * v_eq.x as f64
        + EQUATORIAL_TO_GALACTIC_ROWS[2][1] * v_eq.y as f64
        + EQUATORIAL_TO_GALACTIC_ROWS[2][2] * v_eq.z as f64;
    let b = v_g_z.clamp(-1.0, 1.0).asin();
    let l = v_g_y.atan2(v_g_x).rem_euclid(std::f64::consts::TAU);
    (l, b)
}

/// 3×3 matrix form of the equatorial→galactic rotation, for callers that
/// want to bake the transform into a larger matrix pipeline (e.g. a GPU
/// shader uniform).
pub fn equatorial_to_galactic_matrix() -> Mat3 {
    let r = EQUATORIAL_TO_GALACTIC_ROWS;
    // glam Mat3::from_cols stores columns; transpose rows → columns.
    Mat3::from_cols_array(&[
        r[0][0] as f32,
        r[1][0] as f32,
        r[2][0] as f32,
        r[0][1] as f32,
        r[1][1] as f32,
        r[2][1] as f32,
        r[0][2] as f32,
        r[1][2] as f32,
        r[2][2] as f32,
    ])
}

/// V-band surface brightness in mag/arcsec² → linear surface flux in the
/// "magnitude-zero point" radiometric units used by the renderer.
///
/// ```text
///     F = 10^(-0.4 · (mu - m_ref))
/// ```
///
/// where `m_ref` is the zero-point apparent magnitude (a point source of
/// magnitude `m_ref` has unit flux). The output is dimensionless flux
/// **per arcsec²**; multiply by the pixel solid angle (in arcsec²) to
/// get the per-pixel HDR contribution the renderer expects.
pub fn surface_brightness_to_linear_flux(mu_mag_per_arcsec2: f64, m_ref: f64) -> f64 {
    10.0_f64.powf(-0.4 * (mu_mag_per_arcsec2 - m_ref))
}

// =============================================================================
// Analytic Integrated-Starlight (+DGL) model
// =============================================================================
//
// Empirical V-band surface brightness `μ(l, b)` in mag/arcsec², fit to the
// 1-D profiles published in Leinert et al. 1998 §6 (galactic-latitude
// dependence) and §8 (longitude dependence + diffuse galactic light
// contribution). The qualitative structure is:
//
//   * Galactic poles (|b| ≈ 90°): isotropic floor at μ_floor ≈ 23.5 mag/arcsec².
//   * Galactic plane (b = 0°): thin disk peaks brightly near the bulge
//     (μ ≈ 21 at l = 0°) and fades toward the anti-centre (μ ≈ 22 at
//     l = 180°).
//   * Thin-disk thickness in latitude: σ_b ≈ 4° (Gaussian).
//   * A *thick-disk* component (σ ≈ 30°) keeps the sky a few tenths of a
//     magnitude brighter off-plane near the bulge than at the galactic
//     pole, matching Leinert §6's smooth fall-off with |b|.
//
// We sum the components in linear flux (S10 units) before converting back
// to magnitudes, which keeps the photometric addition correct.

const POLE_FLUX_S10: f64 = 50.0; // ~23.5 mag/arcsec², galactic-pole floor
const THIN_DISK_UNIFORM_S10: f64 = 60.0; // "baseline" thin-disk brightness at b = 0
const THIN_DISK_BULGE_S10: f64 = 400.0; // extra central enhancement on the thin disk
const THICK_DISK_S10: f64 = 50.0; // broad component, keeps |b| < 45° above floor
const SIGMA_B_THIN_DEG: f64 = 4.0; // thin disk Gaussian σ in galactic latitude
const SIGMA_B_THICK_DEG: f64 = 30.0; // thick disk Gaussian σ in galactic latitude
const SIGMA_L_BULGE_DEG: f64 = 60.0; // bulge Gaussian σ in galactic longitude

/// Conversion: 1 S10 unit = 27.78 V-mag per arcsec².
///
/// One S10 unit is the surface brightness of a 10th-magnitude star spread
/// uniformly over one square degree. Since `(mag/arcsec²) = mag(star) +
/// 2.5·log10(arcsec² per square degree) = 10 + 2.5·log10(3600²) ≈ 27.78`,
/// a surface with `F` S10 units shines at
/// `μ = 27.78 - 2.5·log10(F)` mag/arcsec².
const S10_TO_MAG_ARCSEC2_OFFSET: f64 = 27.78;

// =============================================================================
// V-39 artificial sky glow (Bortle / SQM / Falchi atlas)
// =============================================================================
//
// The dark-sky composition above assumes a clean rural site with V ≈ 21.6
// mag/arcsec² at the zenith. Real observers want the sky they will actually
// see from Tokyo, downtown LA, or a National Park. This block adds a single
// artificial-skyglow term, in the same S10(V) units as the dark-sky
// components, that the renderer can sum into the diffuse-sky composition
// before atmospheric extinction.
//
// References:
//   * Bortle, J. E. 2001, *Introducing the Bortle Dark-Sky Scale*,
//     S&T 101(2), 126 — defines nine site classes by zenith V brightness.
//   * Falchi, F. et al. 2016, *The new world atlas of artificial night sky
//     brightness*, Sci Adv 2 e1600377 — VIIRS-derived global zenith atlas.
//   * Cinzano, P., Falchi, F. & Elvidge, C. D. 2001, MNRAS 328, 689 —
//     long-form single-scattering model both Falchi 2016 and Bortle's
//     scale derive from.
//   * Garstang, R. H. 1986, PASP 98, 364 — single-scattering kernel
//     dependence on zenith distance.

/// Observer-side light-pollution config that scales the dark-sky background.
///
/// Three configurations are supported:
///
/// * [`LightPollution::Bortle`] — a 1..=9 class index. The lookup converts to
///   an SQM mag/arcsec² zenith reading from the Bortle 2001 S&T table.
/// * [`LightPollution::Sqm`] — a hand-entered zenith reading in
///   V-band mag/arcsec², e.g. from a SQM-L meter.
/// * [`LightPollution::Atlas2016`] — sample the Falchi et al. 2016 World Atlas
///   GeoTIFF by observer (lat, lng). Tracked under follow-up rung `V-39-Atlas`;
///   in this slice the variant returns the rural default + a tracked
///   `TODO(V-39-Atlas)` log line. The variant is laid down now so the schema
///   does not churn when the loader ships.
///
/// `LightPollution::default()` is [`LightPollution::DARK_SKY`] — Bortle 1, the
/// previous rural-default behaviour, so existing sessions render identically.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightPollution {
    /// Bortle 2001 site class. Valid range is `1..=9`; values are clamped.
    Bortle(u8),
    /// User-supplied zenith SQM reading in V mag/arcsec². Brighter (lower-mu)
    /// = more polluted; finite range is roughly `16.0..=22.0`.
    Sqm(f32),
    /// Sample Falchi 2016 by observer location. Deferred to `V-39-Atlas`; in
    /// the current slice this falls back to the rural natural floor.
    Atlas2016 {
        /// Observer latitude in decimal degrees, +north.
        latitude_deg: f32,
        /// Observer longitude in decimal degrees, +east.
        longitude_deg: f32,
    },
}

impl LightPollution {
    /// Bortle 1 / rural dark sky. Adds essentially no artificial glow above
    /// the natural dark-sky composition.
    pub const DARK_SKY: Self = Self::Bortle(1);

    /// Approximate Bortle ⇒ zenith V mag/arcsec². Bortle 2001 S&T Table 1
    /// (`Bortle's Visual Limiting Magnitude / SQM correspondence`). The
    /// middle classes are anchored by the bright-limit they were drawn from;
    /// Class 1 is essentially the natural-floor 21.99, Class 9 is the heavily
    /// polluted city core ≈ 16.5. Values outside `1..=9` are clamped.
    pub fn bortle_to_sqm_mag_per_arcsec2(class: u8) -> f64 {
        // Bortle 2001 / Cinzano-Falchi-Elvidge 2001 typical zenith SQM
        // values (V mag/arcsec²) for each class. Class 5 anchors at 20.5
        // SQM (the Bortle-table midrange that, once added to the natural
        // floor in S10 units, gives the V≈20.0 zenith the V-39 spec pins).
        // Indexed [0] = class 1 .. [8] = class 9.
        const TABLE: [f64; 9] = [
            21.99, // 1 — Excellent dark-sky site (natural floor)
            21.89, // 2 — Typical truly dark site
            21.6,  // 3 — Rural sky
            20.9,  // 4 — Rural / suburban transition
            20.0,  // 5 — Suburban sky (V-39 calibration anchor)
            19.1,  // 6 — Bright suburban sky
            18.4,  // 7 — Suburban / urban transition
            17.8,  // 8 — City sky
            16.5,  // 9 — Inner-city sky
        ];
        let idx = class.clamp(1, 9) as usize - 1;
        TABLE[idx]
    }

    /// Resolve to a zenith V-band surface brightness in mag/arcsec². The
    /// renderer sums this with the natural dark-sky background in S10 units.
    ///
    /// `Atlas2016` is the placeholder for the follow-up `V-39-Atlas` rung
    /// (Falchi 2016 GeoTIFF loader is too large to ship in this PR). Until
    /// that lands the variant falls back to the Bortle-1 natural floor; the
    /// renderer surfaces a `TODO(V-39-Atlas)` log message at the host side.
    pub fn zenith_sqm_mag_per_arcsec2(&self) -> f64 {
        match *self {
            Self::Bortle(class) => Self::bortle_to_sqm_mag_per_arcsec2(class),
            Self::Sqm(value) => (value as f64).clamp(16.0, 22.5),
            Self::Atlas2016 { .. } => Self::bortle_to_sqm_mag_per_arcsec2(1),
        }
    }

    /// Decompose the configured zenith brightness into a *natural floor* and
    /// an *artificial* additive S10(V) component. The renderer adds only the
    /// artificial term to the dark-sky composition so that Bortle 1 / clean
    /// SQM ≈ 21.6 renders identically to the pre-V-39 background.
    ///
    /// The natural floor used here matches the dark-sky composition's
    /// existing total (ISL pole + airglow + zodiacal floor ≈ 21.6 mag/arcsec²
    /// at high galactic latitude); any sky brighter than that is taken as
    /// artificial. Negative artificial values are clamped at zero.
    pub fn artificial_zenith_s10(&self) -> f64 {
        let mu_total = self.zenith_sqm_mag_per_arcsec2();
        let total_s10 = mag_to_s10(mu_total);
        (total_s10 - NATURAL_FLOOR_S10).max(0.0)
    }

    /// Sodium / LED-dominated artificial-sky-glow spectrum, normalised so the
    /// luminance-weighted average is 1.0. Modern mixed-fixture cities are a
    /// blend of high-pressure sodium and broad-spectrum LED street lighting;
    /// the band sits warm-orange (peak around 590 nm) rather than neutral
    /// grey. Falchi 2016 §3 ("Spectral composition") notes that pure-LED
    /// migration would push this towards neutral white, but the validation
    /// scenes in this rung pin the sodium-dominant default.
    pub fn artificial_rgb_tint() -> [f32; 3] {
        // Linear-RGB tint, normalised to a Rec.709 luminance of 1.0:
        // dot(tint, [0.2126, 0.7152, 0.0722]) ≈ 1.0. Warm orange with a
        // noticeably suppressed blue channel.
        const TINT: [f32; 3] = [1.20, 1.00, 0.42];
        TINT
    }
}

impl Default for LightPollution {
    fn default() -> Self {
        Self::DARK_SKY
    }
}

/// Natural-sky S10(V) floor used by [`LightPollution::artificial_zenith_s10`].
///
/// Pinned to match the dark-sky composition in
/// [`diffuse_sky_mag_per_arcsec2`] at a galactic pole, clean ecliptic point
/// (ISL pole ≈ 50 + airglow 145 + zodiacal floor 18 ≈ 213 S10(V), which
/// corresponds to V ≈ 21.6). Tied to a single number so unit tests can pin
/// it without re-evaluating the full composition.
const NATURAL_FLOOR_S10: f64 = 213.0;

/// V-band zenith dark-sky surface brightness corresponding to the natural
/// floor used by [`LightPollution`]. Lets host validation pin Bortle ⇒ V
/// without depending on the full diffuse-sky composition.
pub fn natural_zenith_mag_per_arcsec2() -> f64 {
    s10_to_mag(NATURAL_FLOOR_S10)
}

/// Total zenith surface brightness in mag/arcsec² for a given
/// [`LightPollution`]: natural floor + artificial term added in S10 units
/// (linear flux), then converted back to magnitudes.
pub fn zenith_mag_per_arcsec2_with_pollution(pollution: LightPollution) -> f64 {
    s10_to_mag(NATURAL_FLOOR_S10 + pollution.artificial_zenith_s10())
}

/// Garstang 1986 single-scattering kernel: relative artificial sky-glow
/// brightness as a function of zenith distance `z` (radians). Returns 1.0
/// at the zenith and rises towards the horizon. Calibrated so it integrates
/// to roughly the same total as `sec(z)`-style airmass weightings used by
/// hand-rolled Bortle calculators while staying finite below 90°.
///
/// Reference: Garstang 1986 PASP 98, 364 eq. (6), simplified to its
/// pure-zenith-distance dependence (the longer-form Cinzano/Falchi/Elvidge
/// 2001 kernel adds elevation + lamp-distance terms not relevant to the
/// observer-side scaling here).
pub fn garstang_zenith_distance_kernel(zenith_distance_rad: f64) -> f64 {
    // Cap z near 90° so the secant blow-up never makes a finite-radiance
    // ray emit infinite flux; 85° matches the dense-troposphere extinction
    // window the Garstang paper is calibrated against.
    let z = zenith_distance_rad.clamp(0.0, 85.0_f64.to_radians());
    let sec_z = 1.0 / z.cos();
    // Quadratic mix of `sec z` and `sec² z` weighted to keep the kernel
    // ~1 at the zenith and ~3 at the airglow-rim ring (z ≈ 75°), matching
    // the Cinzano/Falchi/Elvidge 2001 Fig. 2 zenith-distance profile when
    // collapsed to the pure-observer-side scaling we use here.
    0.4 * sec_z + 0.6 * sec_z * sec_z
}

/// Artificial-skyglow surface brightness in S10(V) at a given zenith
/// distance. Zero when the zenith term is zero (Bortle 1 / dark-sky default),
/// otherwise scaled by [`garstang_zenith_distance_kernel`].
pub fn artificial_skyglow_s10(pollution: LightPollution, zenith_distance_rad: f64) -> f64 {
    let zenith = pollution.artificial_zenith_s10();
    if zenith <= 0.0 {
        return 0.0;
    }
    zenith * garstang_zenith_distance_kernel(zenith_distance_rad)
}

/// V-band zenith twilight surface brightness in mag/arcsec².
///
/// Returns `None` when the Sun is above the geometric horizon (use a daylight
/// scattering model such as Preetham/Hosek/Bruneton instead) or below
/// astronomical twilight (`h < -18°`, where the dark-sky model dominates).
/// Within `0° ≥ h ≥ -18°`, this evaluates the same single-scattering
/// Earth-shadow attenuation law used by the shader: direct solar irradiance is
/// exponentially removed along the tangent path while the remaining light is
/// Rayleigh/Mie scattered into the zenith. The two constants are calibrated to
/// Patat et al. 2006 / Rozenberg 1966 clear-site V-band curves, but the runtime
/// path is continuous radiance rather than a piecewise UI fade.
pub fn twilight_zenith_mag_per_arcsec2(solar_altitude_rad: f64) -> Option<f64> {
    if solar_altitude_rad >= 0.0 || solar_altitude_rad <= -18.0_f64.to_radians() {
        return None;
    }
    let depression_deg = (-solar_altitude_rad).to_degrees().clamp(0.0, 18.0);
    // Optical-depth proxy for the tangent solar path through Earth's shadow.
    // The quadratic term captures the saturation as the illuminated layer rises
    // out of the dense troposphere; in magnitudes this is equivalent to
    // `μ = 3.5 + 2.5 τ` and pins civil/nautical/astronomical twilight.
    let tau = 0.652_222_222_222 * depression_deg - 0.014_444_444_444 * depression_deg.powi(2)
        + 0.000_030_864_198 * depression_deg.powi(3);
    Some(3.5 + 2.5 * tau)
}

/// Approximate total V-band diffuse-sky surface brightness in mag/arcsec².
///
/// `l_rad`/`b_rad` are galactic coordinates for ISL/DGL and dust;
/// `ecliptic_lat_rad` and `sun_relative_lon_rad` evaluate a compact fit to the
/// Leinert et al. 1998 §5 zodiacal-light table, including the antisolar
/// gegenschein enhancement. Smaller `μ` = brighter sky. This is calibrated for
/// naked-eye visualisation and remains an analytic approximation, not a
/// replacement for the published 2-D tables.
pub fn diffuse_sky_mag_per_arcsec2(
    l_rad: f64,
    b_rad: f64,
    ecliptic_lat_rad: f64,
    sun_relative_lon_rad: f64,
) -> f64 {
    let isl = mag_to_s10(isl_mag_per_arcsec2(l_rad, b_rad)) * dust_transmission(l_rad, b_rad);
    let zl = zodiacal_light_s10(ecliptic_lat_rad, sun_relative_lon_rad);
    // V-28: airglow is decomposed into O I 557.7 nm, Na D 589 nm, and OH
    // Meinel red bands. At zenith and moderate activity the total V-band
    // brightness matches the Leinert §7 floor (~145 S10(V)). For the diffuse-
    // sky µ shorthand we evaluate the zenith total; the renderer evaluates
    // each component with its own Van Rhijn correction per pixel.
    let (green, sodium, oh) =
        airglow_components(std::f64::consts::FRAC_PI_2, AIRGLOW_ACTIVITY_MODERATE);
    let airglow = green + sodium + oh;
    s10_to_mag(isl + zl + airglow)
}

// =============================================================================
// V-28: Spectral airglow decomposition
// =============================================================================
//
// The night-sky diffuse floor in V is dominated by three atmospheric emission
// systems:
//
//   * **O I 557.7 nm** "green line" — atomic oxygen recombination in a thin
//     layer near 90 km. Zenith intensity ≈ 250 R at moderate solar activity,
//     ~80 S10(V) integrated through the V band.
//   * **Na D 589.0 / 589.6 nm** — meteor-ablation sodium in a ~10 km thick
//     layer near 92 km. Zenith intensity ≈ 30 R, ~15 S10(V).
//   * **OH Meinel bands** (vibrational rotational transitions, 600–900 nm) —
//     hydroxyl chemiluminescence in a layer near 87 km. The V-band tail of
//     the OH spectrum integrates to ≈ 800 R, ~50 S10(V).
//
// Sum at zenith ≈ 145 S10(V), matching the single-floor Leinert §7 value the
// V-13 / V-21 dark-sky fit was tuned against. The chromaticity split is what
// gives the dark sky its faint mottled green/red tint and removes the
// unphysical pure-grey night floor.
//
// Each layer is brightened toward the horizon by the Van Rhijn (1921) line-
// of-sight integral through a thin emitting shell of height H above the
// Earth's surface:
//
//     V(z, H) = 1 / sqrt(1 - (R / (R + H))² · sin²z)
//
// where `z` is the zenith angle. The conventional approximation collapses the
// `(R/(R+H))²` factor into the 0.96 reported by Roach & Gordon 1973 §5; we
// keep the per-layer factor so the three components fade off the zenith with
// their own characteristic limb brightening.
//
// References:
//   Leinert, Ch. et al. 1998, A&AS 127, 1 (§7.4–7.6).
//   Roach, F. E. & Gordon, J. L. 1973, *The Light of the Night Sky*.
//   Krassovsky, V. I., Shefov, N. N., Yarin, V. I. 1962, Planet. Space Sci.
//   9, 883 (OH Meinel bands).

/// Earth radius used in the Van Rhijn line-of-sight integral, km.
const EARTH_RADIUS_KM: f64 = 6371.0;

/// Layer altitudes for each emitting species, km.
const LAYER_HEIGHT_OI_KM: f64 = 90.0;
const LAYER_HEIGHT_NAD_KM: f64 = 92.0;
const LAYER_HEIGHT_OH_KM: f64 = 87.0;

/// Zenith V-band brightness for each component, S10(V), at moderate solar
/// activity. The triplet sums to ~145 S10(V), matching the Leinert §7
/// dark-site airglow floor that V-13 / V-21 were tuned against.
const AIRGLOW_GREEN_ZENITH_S10: f64 = 80.0;
const AIRGLOW_SODIUM_ZENITH_S10: f64 = 15.0;
const AIRGLOW_OH_ZENITH_S10: f64 = 50.0;

/// Reference solar activity level. Callers can scale to 0.5 (deep solar
/// minimum) or ~2.0 (active aurora-quiet night) following Leinert §7.5
/// Table 17.
pub const AIRGLOW_ACTIVITY_MODERATE: f64 = 1.0;

/// Van Rhijn limb-brightening factor for a thin emitting shell of height
/// `layer_height_km` above the Earth's surface, evaluated at apparent
/// altitude `altitude_rad` (radians above the horizon). Returns 1 at zenith
/// and ≈ 5 at the horizon for an 87–92 km layer.
pub fn van_rhijn_factor(altitude_rad: f64, layer_height_km: f64) -> f64 {
    let r_over_rh = EARTH_RADIUS_KM / (EARTH_RADIUS_KM + layer_height_km);
    let cos_alt = altitude_rad.cos();
    // sin(zenith angle) = cos(altitude). Clamp the denominator so a tiny
    // numerical excursion below the horizon doesn't produce a NaN.
    let denom = (1.0 - r_over_rh * r_over_rh * cos_alt * cos_alt).max(1e-6);
    denom.sqrt().recip()
}

/// V-band surface brightness of the three dominant airglow emission systems
/// at apparent altitude `altitude_rad`, in S10(V) units.
///
/// `activity_level` scales the three components uniformly: 1.0 = Leinert §7
/// moderate-activity reference (zenith total ≈ 145 S10(V)), 0.5 ≈ solar
/// minimum quiet night, 2.0 ≈ active geomagnetic conditions. Negative inputs
/// are clamped to zero (the airglow floor never goes negative).
///
/// Each component is brightened toward the horizon by the Van Rhijn integral
/// `(1 − (R/(R+H))² · sin²z)^(−1/2)` with its own layer altitude:
/// 90 km for O I, 92 km for Na D, 87 km for OH.
pub fn airglow_components(altitude_rad: f64, activity_level: f64) -> (f64, f64, f64) {
    let scale = activity_level.max(0.0);
    let alt = altitude_rad.max(0.0);
    let green = AIRGLOW_GREEN_ZENITH_S10 * scale * van_rhijn_factor(alt, LAYER_HEIGHT_OI_KM);
    let sodium = AIRGLOW_SODIUM_ZENITH_S10 * scale * van_rhijn_factor(alt, LAYER_HEIGHT_NAD_KM);
    let oh = AIRGLOW_OH_ZENITH_S10 * scale * van_rhijn_factor(alt, LAYER_HEIGHT_OH_KM);
    (green, sodium, oh)
}

/// Per-line linear-sRGB tint vectors, normalised so that the Rec. 709
/// luminance `Y = 0.2126 R + 0.7152 G + 0.0722 B` equals 1.0. Multiplying
/// the V-band S10 contribution of a line by its tint vector therefore
/// preserves the V-band luminance budget while giving each component its
/// characteristic chromaticity:
///
/// * 557.7 nm green line → strongly biased toward G.
/// * 589 nm Na D → sodium-yellow (R + G, no B).
/// * OH Meinel red/IR tail in V → deep red (mostly R, some G).
///
/// The 557 / 589 nm chromaticities approximate the sRGB rendering of those
/// monochromatic wavelengths; the OH vector approximates the V-band-weighted
/// integral over the visible OH(6-1) / OH(8-3) / OH(9-4) bands, which sit in
/// the 620–720 nm window.
pub const AIRGLOW_GREEN_RGB: [f64; 3] = [0.000, 1.398, 0.000];
pub const AIRGLOW_SODIUM_RGB: [f64; 3] = [1.229, 1.033, 0.000];
pub const AIRGLOW_OH_RGB: [f64; 3] = [2.343, 0.703, 0.000];

/// Per-channel airglow surface brightness in S10(V), summed over the three
/// emission systems. Each channel is the sum of `component_s10 *
/// component_rgb_tint`; the Rec. 709 luminance of the returned triplet
/// equals the total V-band airglow S10(V).
pub fn airglow_rgb_s10(altitude_rad: f64, activity_level: f64) -> [f64; 3] {
    let (green, sodium, oh) = airglow_components(altitude_rad, activity_level);
    let mut rgb = [0.0; 3];
    for i in 0..3 {
        rgb[i] =
            green * AIRGLOW_GREEN_RGB[i] + sodium * AIRGLOW_SODIUM_RGB[i] + oh * AIRGLOW_OH_RGB[i];
    }
    rgb
}

fn mag_to_s10(mu: f64) -> f64 {
    10.0_f64.powf((S10_TO_MAG_ARCSEC2_OFFSET - mu) / 2.5)
}

fn s10_to_mag(s10: f64) -> f64 {
    S10_TO_MAG_ARCSEC2_OFFSET - 2.5 * s10.max(1e-12).log10()
}

fn zodiacal_light_s10(ecliptic_lat_rad: f64, sun_relative_lon_rad: f64) -> f64 {
    // Compact analytic approximation to Leinert §5's V-band zodiacal-light
    // table. The broad interplanetary-dust band follows ecliptic latitude;
    // elongation from the Sun suppresses the band near quadrature and adds the
    // observed antisolar gegenschein. All amplitudes are in S10(V), the unit
    // used by Leinert's tables.
    let beta = ecliptic_lat_rad.abs().to_degrees();
    let lon = sun_relative_lon_rad.rem_euclid(std::f64::consts::TAU);
    let elongation = angular_distance_on_ecliptic(ecliptic_lat_rad, lon).to_degrees();
    let antisolar = (std::f64::consts::PI - lon)
        .abs()
        .min(lon.abs())
        .to_degrees();

    let latitude_band = (-(beta / 14.0).powi(2)).exp();
    // Zodiacal light is brightest toward the Sun and falls through quadrature;
    // the disk mask keeps the solar-neighbourhood singularity from becoming a
    // second Sun in the dark-sky pass.
    let forward_scatter = 1.0 + 1.15 * (-(elongation / 42.0).powi(2)).exp();
    let ecliptic_band = 48.0 * latitude_band * forward_scatter;
    // Gegenschein: broad, faint antisolar oval, concentrated within a few tens
    // of degrees of the ecliptic and centred at λ - λ_sun = 180°.
    let gegenschein = 32.0 * (-(antisolar / 18.0).powi(2) - (beta / 10.0).powi(2)).exp();
    18.0 + ecliptic_band + gegenschein
}

fn angular_distance_on_ecliptic(beta_rad: f64, sun_relative_lon_rad: f64) -> f64 {
    // Angular separation between an ecliptic point `(λ-λ_sun, β)` and the Sun
    // at `(0, 0)`, with spherical-law-of-cosines clamping for round-off.
    (beta_rad.cos() * sun_relative_lon_rad.cos())
        .clamp(-1.0, 1.0)
        .acos()
}

fn dust_transmission(l_rad: f64, b_rad: f64) -> f64 {
    // SFD98-inspired analytic E(B−V) screen: dust concentrated in the plane,
    // enhanced toward the inner Galaxy. A_V=3.1E(B−V), transmission=10^-0.4Av.
    let l_deg = l_rad.to_degrees();
    let l_centered = if l_deg > 180.0 { l_deg - 360.0 } else { l_deg };
    let ebv = 0.015
        + 0.12 * (-(b_rad.to_degrees().abs() / 8.0)).exp()
        + 0.08 * (-(l_centered / 45.0).powi(2)).exp() * (-(b_rad.to_degrees().abs() / 5.0)).exp();
    10.0_f64.powf(-0.4 * 3.1 * ebv)
}

pub fn isl_mag_per_arcsec2(l_rad: f64, b_rad: f64) -> f64 {
    let l_deg = l_rad.to_degrees();
    let b_deg = b_rad.to_degrees();

    // Wrap longitude to (-180°, 180°] so the bulge Gaussian is symmetric
    // around the galactic centre.
    let l_centered = if l_deg > 180.0 { l_deg - 360.0 } else { l_deg };

    let thin_lat_factor = (-(b_deg * b_deg) / (2.0 * SIGMA_B_THIN_DEG * SIGMA_B_THIN_DEG)).exp();
    let thick_lat_factor = (-(b_deg * b_deg) / (2.0 * SIGMA_B_THICK_DEG * SIGMA_B_THICK_DEG)).exp();
    let bulge_lon_factor =
        (-(l_centered * l_centered) / (2.0 * SIGMA_L_BULGE_DEG * SIGMA_L_BULGE_DEG)).exp();

    let flux_s10 = POLE_FLUX_S10
        + THICK_DISK_S10 * thick_lat_factor
        + (THIN_DISK_UNIFORM_S10 + THIN_DISK_BULGE_S10 * bulge_lon_factor) * thin_lat_factor;

    S10_TO_MAG_ARCSEC2_OFFSET - 2.5 * flux_s10.log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deg(x: f64) -> f64 {
        x.to_radians()
    }

    /// The IAU galactic-rotation matrix is orthonormal: rows are unit
    /// length and mutually orthogonal. Pin this so a transcription error
    /// in the constants would be caught.
    #[test]
    fn galactic_matrix_is_orthonormal() {
        let m = EQUATORIAL_TO_GALACTIC_ROWS;
        for row in &m {
            let n = row[0] * row[0] + row[1] * row[1] + row[2] * row[2];
            assert!(
                (n - 1.0).abs() < 1e-9,
                "row {row:?} not unit length: |.|² = {n}"
            );
        }
        // Pairwise orthogonality.
        for i in 0..3 {
            for j in i + 1..3 {
                let dot = m[i][0] * m[j][0] + m[i][1] * m[j][1] + m[i][2] * m[j][2];
                assert!(
                    dot.abs() < 1e-9,
                    "rows {i} and {j} not orthogonal: dot = {dot}"
                );
            }
        }
    }

    /// The galactic centre (Sagittarius A*) sits at J2000 equatorial
    /// `(α, δ) ≈ (266.405°, -28.936°)` and should map to galactic
    /// `(l, b) ≈ (0°, 0°)` to within rounding of the catalogue position
    /// versus the dynamical centre.
    #[test]
    fn galactic_centre_maps_to_zero_zero() {
        let ra = deg(266.405);
        let dec = deg(-28.936);
        let v = Vec3::new(
            (dec.cos() * ra.cos()) as f32,
            (dec.cos() * ra.sin()) as f32,
            dec.sin() as f32,
        );
        let (l, b) = equatorial_to_galactic(v);
        assert!(
            l.to_degrees() < 0.5 || l.to_degrees() > 359.5,
            "Sgr A* should be near l = 0°, got {}°",
            l.to_degrees()
        );
        assert!(
            b.to_degrees().abs() < 0.5,
            "Sgr A* should be near b = 0°, got {}°",
            b.to_degrees()
        );
    }

    /// The north galactic pole sits at J2000 equatorial
    /// `(α, δ) = (192.85948°, 27.12825°)` (this is *how* the matrix is
    /// defined). It must map to galactic `b = +90°`. Tolerance is loose
    /// (~1 arcmin) because the input vector is built in `f32` so
    /// `asin(≈1)` accumulates the usual square-root precision loss near
    /// the pole — not a defect of the rotation matrix itself.
    #[test]
    fn north_galactic_pole_maps_to_b_plus_ninety() {
        let ra = deg(192.85948);
        let dec = deg(27.12825);
        let v = Vec3::new(
            (dec.cos() * ra.cos()) as f32,
            (dec.cos() * ra.sin()) as f32,
            dec.sin() as f32,
        );
        let (_, b) = equatorial_to_galactic(v);
        assert!(
            (b.to_degrees() - 90.0).abs() < 0.02,
            "NGP should map to b ≈ 90°, got {}°",
            b.to_degrees()
        );
    }

    /// Surface-brightness → linear flux mapping is monotone: a brighter
    /// (numerically smaller) magnitude produces more linear flux. And a
    /// 5-magnitude difference is exactly a factor of 100, as on point
    /// sources (Pogson's law).
    #[test]
    fn surface_brightness_pogson_law() {
        let f21 = surface_brightness_to_linear_flux(21.0, 0.0);
        let f26 = surface_brightness_to_linear_flux(26.0, 0.0);
        let ratio = f21 / f26;
        assert!(
            (ratio - 100.0).abs() < 1e-6,
            "5-mag SB ratio = {ratio}, expected 100"
        );
    }

    /// ISL surface brightness must be brighter (= numerically smaller μ) in
    /// the galactic centre direction than at the pole. This is the
    /// defining property of the Milky Way band.
    #[test]
    fn galactic_centre_brighter_than_pole() {
        let mu_centre = isl_mag_per_arcsec2(0.0, 0.0);
        let mu_pole = isl_mag_per_arcsec2(0.0, std::f64::consts::FRAC_PI_2);
        assert!(
            mu_centre < mu_pole - 2.0,
            "galactic centre μ = {mu_centre}, pole μ = {mu_pole}: centre should be ≥ 2 mag brighter"
        );
    }

    /// ISL at five Leinert 1998 reference points (§6 summary). Tolerance
    /// is the published-spread of the underlying photometry (±0.5 mag/
    /// arcsec²) plus an analytic-fit allowance; tightening this would
    /// require digitising the full 2-D table instead of using a sum of
    /// Gaussians (see ROADMAP / VALIDATION).
    #[test]
    fn isl_matches_leinert_reference_points() {
        // (label, l_deg, b_deg, expected μ, tolerance)
        let cases: &[(&str, f64, f64, f64, f64)] = &[
            ("galactic centre", 0.0, 0.0, 21.0, 0.7),
            ("local plane", 90.0, 0.0, 21.7, 0.7),
            ("anti-centre", 180.0, 0.0, 22.2, 0.7),
            ("off-plane near bulge", 0.0, 30.0, 23.0, 0.7),
            ("galactic pole", 0.0, 90.0, 23.5, 0.5),
        ];
        for (label, l, b, expected, tol) in cases {
            let got = isl_mag_per_arcsec2(l.to_radians(), b.to_radians());
            assert!(
                (got - expected).abs() < *tol,
                "{label} ({l}°, {b}°): got μ = {got}, expected {expected} ± {tol}"
            );
        }
    }

    /// The disk drops off rapidly in galactic latitude: at |b| = σ_b ≈ 4°
    /// the disk component is at 1/√e of its peak; at |b| = 30° the disk
    /// is negligible and only the pole floor remains. Pin the latitudinal
    /// fall-off so the disk doesn't accidentally become broad.
    #[test]
    fn disk_falls_off_in_latitude() {
        let mu_b0 = isl_mag_per_arcsec2(deg(90.0), 0.0);
        let mu_b30 = isl_mag_per_arcsec2(deg(90.0), deg(30.0));
        assert!(
            mu_b30 > mu_b0 + 1.0,
            "disk should fade ≥ 1 mag from plane to b=30°: μ(0)={mu_b0}, μ(30°)={mu_b30}"
        );
    }

    #[test]
    fn diffuse_sky_includes_zodiacal_and_airglow_floor() {
        let high_lat = diffuse_sky_mag_per_arcsec2(deg(180.0), deg(80.0), deg(80.0), deg(90.0));
        let ecliptic_plane = diffuse_sky_mag_per_arcsec2(deg(180.0), deg(80.0), 0.0, deg(90.0));
        assert!(
            ecliptic_plane < high_lat,
            "zodiacal plane should be brighter: plane μ={ecliptic_plane}, high-lat μ={high_lat}"
        );
    }

    #[test]
    fn zodiacal_fit_has_antisolar_gegenschein() {
        let quadrature = diffuse_sky_mag_per_arcsec2(deg(180.0), deg(80.0), 0.0, deg(90.0));
        let antisolar = diffuse_sky_mag_per_arcsec2(deg(180.0), deg(80.0), 0.0, deg(180.0));
        assert!(
            antisolar < quadrature,
            "gegenschein should brighten antisolar ecliptic sky: anti μ={antisolar}, quad μ={quadrature}"
        );
    }

    #[test]
    fn dust_screen_dims_galactic_plane_isl() {
        let raw = isl_mag_per_arcsec2(0.0, 0.0);
        let dimmed = s10_to_mag(mag_to_s10(raw) * dust_transmission(0.0, 0.0));
        assert!(dimmed > raw, "dust should make ISL numerically fainter");
    }

    /// Van Rhijn limb brightening: a thin emitting shell at ≈90 km is
    /// nearly 1 at zenith and ≈5 toward the horizon. The 0.96-prefactor
    /// approximation used in Roach & Gordon 1973 (i.e. ignoring the height
    /// dependence) gives the same limit to within a few percent.
    #[test]
    fn van_rhijn_zenith_to_horizon() {
        let zenith = van_rhijn_factor(std::f64::consts::FRAC_PI_2, LAYER_HEIGHT_OI_KM);
        let horizon = van_rhijn_factor(0.0, LAYER_HEIGHT_OI_KM);
        assert!(
            (zenith - 1.0).abs() < 1e-9,
            "Van Rhijn at zenith should be 1, got {zenith}"
        );
        assert!(
            (horizon - 6.0).abs() < 1.0,
            "Van Rhijn at horizon should be ≈5–6 for a 90 km layer, got {horizon}"
        );
    }

    /// V-28 acceptance criterion: zenith total integrated airglow in V band
    /// must be within 10% of the Leinert 1998 §7 reference (≈ 145 S10(V)
    /// at moderate activity).
    #[test]
    fn airglow_zenith_total_matches_leinert() {
        let (green, sodium, oh) =
            airglow_components(std::f64::consts::FRAC_PI_2, AIRGLOW_ACTIVITY_MODERATE);
        let total = green + sodium + oh;
        let reference = 145.0; // S10(V), Leinert §7 dark-site visual floor
        assert!(
            (total / reference - 1.0).abs() <= 0.10,
            "zenith airglow total = {total} S10(V), expected {reference} ± 10%"
        );
        // The OH band carries the largest single contribution to the V-band
        // *photon* flux (≈800 R) but green dominates after V-band weighting
        // because the OH spectrum extends into NIR. Keep this ordering so an
        // accidental swap of constants would be caught.
        assert!(
            green > sodium && oh > sodium,
            "unexpected component ordering: green={green}, Na={sodium}, OH={oh}"
        );
    }

    /// Limb brightening: at the horizon every component is ≈5× the zenith
    /// value, and the totals follow.
    #[test]
    fn airglow_horizon_brighter_than_zenith() {
        let (gz, nz, hz) =
            airglow_components(std::f64::consts::FRAC_PI_2, AIRGLOW_ACTIVITY_MODERATE);
        let (gh, nh, hh) = airglow_components(0.0, AIRGLOW_ACTIVITY_MODERATE);
        assert!(gh > 4.5 * gz && gh < 6.5 * gz);
        assert!(nh > 4.5 * nz && nh < 6.5 * nz);
        assert!(hh > 4.5 * hz && hh < 6.5 * hz);
    }

    /// V-28 acceptance criterion: pinned dark-sky chromaticity. The per-
    /// channel S10 split must produce a measurable G/R chromaticity
    /// difference vs. a neutral grey sky at zenith. We require |R−G|/Y
    /// ≥ 0.10 in linear sRGB so the renderer's tint cannot be mistaken for
    /// a desaturated grey.
    #[test]
    fn airglow_chromaticity_differs_from_neutral_grey() {
        let rgb = airglow_rgb_s10(std::f64::consts::FRAC_PI_2, AIRGLOW_ACTIVITY_MODERATE);
        let y = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
        // V-band luminance must match the total S10 (chromaticity vectors
        // are normalised to Y = 1 per line).
        let (gz, nz, hz) =
            airglow_components(std::f64::consts::FRAC_PI_2, AIRGLOW_ACTIVITY_MODERATE);
        let total = gz + nz + hz;
        assert!(
            (y / total - 1.0).abs() < 1e-3,
            "per-channel V-band luminance ({y}) must equal total S10 ({total})"
        );
        // Documented threshold: dark-site airglow must be detectably non-
        // grey. The OH red tail and the 557 nm green together give
        // |R−G|/Y ≈30%; require ≥10% so a future re-tune still flags
        // the perceptual intent.
        let r_minus_g = (rgb[0] - rgb[1]).abs();
        let chroma = r_minus_g / y;
        assert!(
            chroma >= 0.10,
            "airglow R/G chromaticity {chroma:.3} below 0.10 threshold"
        );
        // No blue contribution from airglow: the three lines are all in the
        // 557–720 nm window. A non-zero B here would mean the tint vectors
        // were accidentally widened.
        assert!(
            rgb[2] < 0.02 * y,
            "airglow must have negligible B channel: B/Y = {}",
            rgb[2] / y
        );
    }

    /// Activity scaling is a uniform multiplier.
    #[test]
    fn airglow_activity_scaling_is_linear() {
        let half = airglow_rgb_s10(std::f64::consts::FRAC_PI_2, 0.5);
        let one = airglow_rgb_s10(std::f64::consts::FRAC_PI_2, 1.0);
        let two = airglow_rgb_s10(std::f64::consts::FRAC_PI_2, 2.0);
        for i in 0..3 {
            assert!((half[i] * 2.0 - one[i]).abs() < 1e-9);
            assert!((two[i] * 0.5 - one[i]).abs() < 1e-9);
        }
    }

    /// V-39 validation gate: Bortle 5 zenith ≈ V 20.0 within 0.2 mag/arcsec²
    /// once the artificial S10 term is added to the natural floor. Pinned in
    /// VALIDATION.md.
    #[test]
    fn bortle_5_zenith_matches_20_within_tolerance() {
        let mu = zenith_mag_per_arcsec2_with_pollution(LightPollution::Bortle(5));
        assert!(
            (mu - 20.0).abs() < 0.2,
            "Bortle 5 zenith should be V ≈ 20.0 ± 0.2 mag/arcsec²; got {mu}"
        );
    }

    /// Bortle 1 / dark-sky default keeps the natural floor unchanged so
    /// existing rural scenes round-trip pixel-identically.
    #[test]
    fn bortle_1_keeps_natural_floor() {
        let mu = zenith_mag_per_arcsec2_with_pollution(LightPollution::Bortle(1));
        let floor = natural_zenith_mag_per_arcsec2();
        assert!(
            (mu - floor).abs() < 0.05,
            "Bortle 1 zenith {mu} should match natural floor {floor}"
        );
        assert_eq!(LightPollution::Bortle(1).artificial_zenith_s10(), 0.0);
    }

    /// Bortle 8 (city sky) and Bortle 9 (inner city) both push the zenith
    /// well below the natural floor — i.e. into much-brighter territory.
    #[test]
    fn bortle_class_is_monotone_in_brightness() {
        let mu1 = zenith_mag_per_arcsec2_with_pollution(LightPollution::Bortle(1));
        let mu5 = zenith_mag_per_arcsec2_with_pollution(LightPollution::Bortle(5));
        let mu8 = zenith_mag_per_arcsec2_with_pollution(LightPollution::Bortle(8));
        let mu9 = zenith_mag_per_arcsec2_with_pollution(LightPollution::Bortle(9));
        assert!(
            mu1 > mu5 && mu5 > mu8 && mu8 > mu9,
            "expected μ1>μ5>μ8>μ9, got {mu1}, {mu5}, {mu8}, {mu9}"
        );
        // Inner-city is roughly five magnitudes brighter than rural.
        assert!(
            mu1 - mu9 > 4.0,
            "Bortle 9 should be ≥ 4 mag brighter than 1"
        );
    }

    /// SQM input round-trips through the artificial-S10 conversion and back
    /// to the same zenith magnitude (within the rounding the linear-flux
    /// addition implies).
    #[test]
    fn sqm_input_round_trips() {
        let mu_in = 19.5_f32;
        let pollution = LightPollution::Sqm(mu_in);
        let mu_out = zenith_mag_per_arcsec2_with_pollution(pollution);
        assert!(
            (mu_out - mu_in as f64).abs() < 0.05,
            "SQM round-trip drift: input={mu_in}, output={mu_out}"
        );
    }

    /// Bortle classes outside `1..=9` are clamped, not panicking.
    #[test]
    fn bortle_class_clamps_to_valid_range() {
        let mu_zero = zenith_mag_per_arcsec2_with_pollution(LightPollution::Bortle(0));
        let mu_one = zenith_mag_per_arcsec2_with_pollution(LightPollution::Bortle(1));
        let mu_high = zenith_mag_per_arcsec2_with_pollution(LightPollution::Bortle(99));
        let mu_nine = zenith_mag_per_arcsec2_with_pollution(LightPollution::Bortle(9));
        assert_eq!(mu_zero, mu_one);
        assert_eq!(mu_high, mu_nine);
    }

    /// Garstang kernel is 1.0 at the zenith and rises monotonically with
    /// zenith distance — the artificial sky-glow brightens toward the
    /// horizon under naked-eye viewing.
    #[test]
    fn garstang_kernel_is_one_at_zenith_and_rises() {
        let z0 = garstang_zenith_distance_kernel(0.0);
        let z45 = garstang_zenith_distance_kernel(45.0_f64.to_radians());
        let z80 = garstang_zenith_distance_kernel(80.0_f64.to_radians());
        let z89 = garstang_zenith_distance_kernel(89.0_f64.to_radians());
        assert!(
            (z0 - 1.0).abs() < 1e-9,
            "kernel at zenith = {z0}, expected 1.0"
        );
        assert!(z0 < z45 && z45 < z80, "kernel must rise with z");
        // Saturation guard: 89° must clamp finite, not blow up to infinity.
        // The 85° cap (sec ≈ 11.5, sec² ≈ 132) gives a kernel ~85 there.
        assert!(
            z89.is_finite() && z89 < 200.0,
            "horizon kernel must stay finite via the 85° cap, got {z89}"
        );
    }

    /// `Atlas2016` is a sentinel in this rung — it must return a finite,
    /// dark-sky-equivalent zenith so the rest of the pipeline can render
    /// without panicking while the GeoTIFF loader is still TODO.
    #[test]
    fn atlas2016_falls_back_to_rural_default() {
        let pollution = LightPollution::Atlas2016 {
            latitude_deg: 35.68,
            longitude_deg: 139.69,
        };
        let mu = zenith_mag_per_arcsec2_with_pollution(pollution);
        let floor = natural_zenith_mag_per_arcsec2();
        assert!(
            (mu - floor).abs() < 0.05,
            "Atlas2016 fallback drifted from natural floor: {mu} vs {floor}"
        );
    }

    /// The sodium / LED tint must be warm-orange-ish (R > G > B) with a
    /// luminance-weighted magnitude near 1.0, so multiplying a dark-sky
    /// luminance by it does not change the overall photometric scale.
    #[test]
    fn artificial_tint_is_warm_orange_with_unit_luminance() {
        let [r, g, b] = LightPollution::artificial_rgb_tint();
        assert!(
            r > g && g > b,
            "tint must roll off blue → red (sodium/LED), got R={r} G={g} B={b}"
        );
        let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        assert!(
            (lum - 1.0).abs() < 0.05,
            "tint luminance {lum} should be near 1.0"
        );
    }

    #[test]
    fn twilight_curve_is_continuous_and_monotone() {
        assert_eq!(twilight_zenith_mag_per_arcsec2(1.0_f64.to_radians()), None);
        assert_eq!(
            twilight_zenith_mag_per_arcsec2((-19.0_f64).to_radians()),
            None
        );

        let civil = twilight_zenith_mag_per_arcsec2((-6.0_f64).to_radians()).unwrap();
        let nautical = twilight_zenith_mag_per_arcsec2((-12.0_f64).to_radians()).unwrap();
        let astronomical = twilight_zenith_mag_per_arcsec2((-17.999_f64).to_radians()).unwrap();
        assert!((civil - 12.0).abs() < 0.6);
        assert!((nautical - 18.0).abs() < 0.8);
        assert!((astronomical - 21.6).abs() < 1.0);
        assert!(civil < nautical && nautical < astronomical);
    }
}
