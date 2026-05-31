//! V-39-Atlas: Falchi et al. 2016 World Atlas zenith-brightness loader.
//!
//! The Bortle / SQM core of `V-39` (see [`crate::skyglow::LightPollution`])
//! lets an observer pick a site class or hand-enter an SQM reading. This
//! module completes the third configuration — sampling the **Falchi et al.
//! 2016 World Atlas of Artificial Night Sky Brightness** by observer
//! latitude / longitude — so "show me the sky from *this* point on Earth"
//! resolves to a real zenith surface brightness.
//!
//! ## Why a compact grid rather than the raw GeoTIFF
//!
//! The published atlas is a global VIIRS-derived GeoTIFF at ~750 m resolution
//! (~1 GB), which is far too large to commit. The supported path is therefore:
//!
//! 1. `scripts/fetch-falchi-atlas.sh` downloads the upstream GeoTIFF, and
//! 2. `scripts/build-falchi-atlas.py` resamples it onto a coarse regular
//!    lat/lng grid of **total zenith V-band sky brightness** (mag/arcsec²),
//!    serialised in the compact [`FalchiAtlas`] binary format below.
//!
//! Hosts load that compact grid at runtime (see the host loader in
//! `stars_host_common`) and pass the sampled brightness through the existing
//! [`crate::skyglow::LightPollution::Sqm`] path, so no renderer or shader
//! change is needed.
//!
//! This module is intentionally **IO-free and dependency-free**: it parses an
//! in-memory byte slice and bilinearly samples it. File reading lives in the
//! host crate so the engine's dependency surface stays minimal.
//!
//! ## Binary format (`FALATL01`, little-endian)
//!
//! | offset | type        | field                                         |
//! |-------:|-------------|-----------------------------------------------|
//! | 0      | `[u8; 8]`   | magic `b"FALATL01"`                            |
//! | 8      | `u32`       | `rows` (latitude samples, north → south)      |
//! | 12     | `u32`       | `cols` (longitude samples, west → east)       |
//! | 16     | `f64`       | `lat_north_deg` (top edge, +north)            |
//! | 24     | `f64`       | `lat_south_deg` (bottom edge)                 |
//! | 32     | `f64`       | `lng_west_deg`  (left edge, +east)            |
//! | 40     | `f64`       | `lng_east_deg`  (right edge)                  |
//! | 48     | `f32[rows*cols]` | row-major zenith V mag/arcsec²; `NaN` = no data |
//!
//! `NaN` cells mark ocean / out-of-coverage pixels (Falchi reports artificial
//! brightness only over land within the VIIRS swath); the sampler treats them
//! as "no artificial contribution" and ignores them in the bilinear blend.
//!
//! ## References
//! - Falchi, F. et al. 2016, *The new world atlas of artificial night sky
//!   brightness*, Science Advances 2, e1600377 (DOI 10.1126/sciadv.1600377).
//! - Cinzano, P., Falchi, F. & Elvidge, C. D. 2001, MNRAS 328, 689.

/// Errors returned when parsing a [`FalchiAtlas`] binary blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtlasError {
    /// The blob is shorter than the fixed header or its payload.
    Truncated,
    /// The leading magic bytes are not `b"FALATL01"`.
    BadMagic,
    /// `rows` or `cols` was zero, or the declared grid bounds are degenerate.
    BadDimensions,
}

impl core::fmt::Display for AtlasError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AtlasError::Truncated => f.write_str("Falchi atlas blob is truncated"),
            AtlasError::BadMagic => f.write_str("Falchi atlas magic mismatch (expected FALATL01)"),
            AtlasError::BadDimensions => f.write_str("Falchi atlas has degenerate dimensions"),
        }
    }
}

impl std::error::Error for AtlasError {}

const MAGIC: &[u8; 8] = b"FALATL01";
const HEADER_LEN: usize = 8 + 4 + 4 + 8 * 4;

/// A coarse regular lat/lng grid of total zenith V-band sky brightness
/// (mag/arcsec²), resampled from the Falchi et al. 2016 World Atlas. See the
/// module docs for the binary layout.
#[derive(Debug, Clone, PartialEq)]
pub struct FalchiAtlas {
    rows: u32,
    cols: u32,
    lat_north_deg: f64,
    lat_south_deg: f64,
    lng_west_deg: f64,
    lng_east_deg: f64,
    /// Row-major `rows * cols` zenith V mag/arcsec²; `NaN` marks no-data.
    values: Vec<f32>,
}

impl FalchiAtlas {
    /// Parse a `FALATL01` binary blob produced by `build-falchi-atlas.py`.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AtlasError> {
        if bytes.len() < HEADER_LEN {
            return Err(AtlasError::Truncated);
        }
        if &bytes[0..8] != MAGIC {
            return Err(AtlasError::BadMagic);
        }
        let rows = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let cols = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        if rows == 0 || cols == 0 {
            return Err(AtlasError::BadDimensions);
        }
        let read_f64 = |off: usize| f64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
        let lat_north_deg = read_f64(16);
        let lat_south_deg = read_f64(24);
        let lng_west_deg = read_f64(32);
        let lng_east_deg = read_f64(40);
        if !lat_north_deg.is_finite()
            || !lat_south_deg.is_finite()
            || !lng_west_deg.is_finite()
            || !lng_east_deg.is_finite()
            || lat_north_deg <= lat_south_deg
            || lng_east_deg <= lng_west_deg
        {
            return Err(AtlasError::BadDimensions);
        }
        let count = rows as usize * cols as usize;
        let payload = &bytes[HEADER_LEN..];
        if payload.len() < count * 4 {
            return Err(AtlasError::Truncated);
        }
        let mut values = Vec::with_capacity(count);
        for i in 0..count {
            let o = i * 4;
            values.push(f32::from_le_bytes(payload[o..o + 4].try_into().unwrap()));
        }
        Ok(Self {
            rows,
            cols,
            lat_north_deg,
            lat_south_deg,
            lng_west_deg,
            lng_east_deg,
            values,
        })
    }

    /// Grid dimensions `(rows, cols)` = `(latitude, longitude)` samples.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.rows, self.cols)
    }

    /// Geographic bounds `(lat_north, lat_south, lng_west, lng_east)` in
    /// decimal degrees (grid edges, not cell centres).
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        (
            self.lat_north_deg,
            self.lat_south_deg,
            self.lng_west_deg,
            self.lng_east_deg,
        )
    }

    fn value(&self, row: u32, col: u32) -> f32 {
        self.values[row as usize * self.cols as usize + col as usize]
    }

    /// Bilinearly sample the total zenith V-band surface brightness
    /// (mag/arcsec²) at the given observer location.
    ///
    /// Returns `None` when the point lies outside the atlas bounds or when the
    /// four surrounding cells are all no-data (`NaN`) — both cases mean the
    /// caller should keep the natural dark-sky floor rather than invent a
    /// brightness. Longitude is wrapped into `[-180, 180)` before the bounds
    /// test. No-data neighbours are dropped from the blend and the remaining
    /// (in-data) cells are renormalised so a coastline pixel still resolves.
    pub fn sample_zenith_mag_per_arcsec2(&self, lat_deg: f64, lng_deg: f64) -> Option<f64> {
        let lng = wrap_longitude_deg(lng_deg);
        if lat_deg > self.lat_north_deg
            || lat_deg < self.lat_south_deg
            || lng < self.lng_west_deg
            || lng > self.lng_east_deg
        {
            return None;
        }

        // Fractional grid coordinates. Row 0 is the north edge, so latitude
        // decreases with increasing row.
        let lat_span = self.lat_north_deg - self.lat_south_deg;
        let lng_span = self.lng_east_deg - self.lng_west_deg;
        let fr = (self.lat_north_deg - lat_deg) / lat_span * (self.rows - 1).max(1) as f64;
        let fc = (lng - self.lng_west_deg) / lng_span * (self.cols - 1).max(1) as f64;

        let r0 = (fr.floor() as i64).clamp(0, self.rows as i64 - 1) as u32;
        let c0 = (fc.floor() as i64).clamp(0, self.cols as i64 - 1) as u32;
        let r1 = (r0 + 1).min(self.rows - 1);
        let c1 = (c0 + 1).min(self.cols - 1);
        let dr = (fr - r0 as f64).clamp(0.0, 1.0);
        let dc = (fc - c0 as f64).clamp(0.0, 1.0);

        // Weighted bilinear blend that skips NaN (no-data) corners.
        let samples = [
            (self.value(r0, c0), (1.0 - dr) * (1.0 - dc)),
            (self.value(r0, c1), (1.0 - dr) * dc),
            (self.value(r1, c0), dr * (1.0 - dc)),
            (self.value(r1, c1), dr * dc),
        ];
        let mut acc = 0.0f64;
        let mut wsum = 0.0f64;
        for (v, w) in samples {
            if v.is_finite() && w > 0.0 {
                acc += v as f64 * w;
                wsum += w;
            }
        }
        if wsum <= 0.0 {
            return None;
        }
        Some(acc / wsum)
    }
}

/// Wrap a longitude in decimal degrees into the half-open range `[-180, 180)`.
fn wrap_longitude_deg(lng_deg: f64) -> f64 {
    let mut x = (lng_deg + 180.0).rem_euclid(360.0) - 180.0;
    if x == 180.0 {
        x = -180.0;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialise a `FALATL01` blob for tests. This mirrors the byte layout
    /// `scripts/build-falchi-atlas.py` writes; the values here are a synthetic
    /// fixture (NOT Falchi data) used only to exercise the parser / sampler.
    fn encode(rows: u32, cols: u32, bounds: (f64, f64, f64, f64), values: &[f32]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&rows.to_le_bytes());
        b.extend_from_slice(&cols.to_le_bytes());
        b.extend_from_slice(&bounds.0.to_le_bytes());
        b.extend_from_slice(&bounds.1.to_le_bytes());
        b.extend_from_slice(&bounds.2.to_le_bytes());
        b.extend_from_slice(&bounds.3.to_le_bytes());
        for v in values {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b
    }

    // A 2×2 grid over lat [0, 10], lng [0, 10]:
    //   row 0 (north, lat=10): [18.0, 20.0]
    //   row 1 (south, lat=0):  [22.0, 16.0]
    fn fixture() -> FalchiAtlas {
        let bytes = encode(2, 2, (10.0, 0.0, 0.0, 10.0), &[18.0, 20.0, 22.0, 16.0]);
        FalchiAtlas::from_bytes(&bytes).unwrap()
    }

    #[test]
    fn parses_header_and_dimensions() {
        let a = fixture();
        assert_eq!(a.dimensions(), (2, 2));
        assert_eq!(a.bounds(), (10.0, 0.0, 0.0, 10.0));
    }

    #[test]
    fn rejects_bad_magic_and_truncation() {
        assert_eq!(
            FalchiAtlas::from_bytes(b"nope").unwrap_err(),
            AtlasError::Truncated
        );
        let mut bytes = encode(2, 2, (10.0, 0.0, 0.0, 10.0), &[1.0, 2.0, 3.0, 4.0]);
        bytes[0] = b'X';
        assert_eq!(
            FalchiAtlas::from_bytes(&bytes).unwrap_err(),
            AtlasError::BadMagic
        );
        // Drop the last value → payload too short.
        let short = encode(2, 2, (10.0, 0.0, 0.0, 10.0), &[1.0, 2.0, 3.0]);
        assert_eq!(
            FalchiAtlas::from_bytes(&short).unwrap_err(),
            AtlasError::Truncated
        );
    }

    #[test]
    fn rejects_degenerate_bounds() {
        let bytes = encode(2, 2, (0.0, 0.0, 0.0, 10.0), &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(
            FalchiAtlas::from_bytes(&bytes).unwrap_err(),
            AtlasError::BadDimensions
        );
    }

    #[test]
    fn samples_grid_corners_exactly() {
        let a = fixture();
        // North-west corner (lat=10, lng=0) → row0/col0 = 18.0.
        assert!((a.sample_zenith_mag_per_arcsec2(10.0, 0.0).unwrap() - 18.0).abs() < 1e-9);
        // North-east (lat=10, lng=10) → 20.0.
        assert!((a.sample_zenith_mag_per_arcsec2(10.0, 10.0).unwrap() - 20.0).abs() < 1e-9);
        // South-west (lat=0, lng=0) → 22.0.
        assert!((a.sample_zenith_mag_per_arcsec2(0.0, 0.0).unwrap() - 22.0).abs() < 1e-9);
        // South-east (lat=0, lng=10) → 16.0.
        assert!((a.sample_zenith_mag_per_arcsec2(0.0, 10.0).unwrap() - 16.0).abs() < 1e-9);
    }

    #[test]
    fn bilinear_blend_at_centre() {
        let a = fixture();
        // Centre (lat=5, lng=5) = mean of the four corners = 19.0.
        let mu = a.sample_zenith_mag_per_arcsec2(5.0, 5.0).unwrap();
        assert!((mu - 19.0).abs() < 1e-9, "centre {mu}");
    }

    #[test]
    fn out_of_bounds_returns_none() {
        let a = fixture();
        assert!(a.sample_zenith_mag_per_arcsec2(20.0, 5.0).is_none());
        assert!(a.sample_zenith_mag_per_arcsec2(5.0, 40.0).is_none());
    }

    #[test]
    fn nodata_neighbours_are_skipped() {
        // North row all no-data; south row real. Sampling near the north edge
        // still resolves from the in-data south cells rather than returning
        // NaN or None.
        let bytes = encode(
            2,
            2,
            (10.0, 0.0, 0.0, 10.0),
            &[f32::NAN, f32::NAN, 20.0, 20.0],
        );
        let a = FalchiAtlas::from_bytes(&bytes).unwrap();
        let mu = a.sample_zenith_mag_per_arcsec2(9.0, 5.0).unwrap();
        assert!((mu - 20.0).abs() < 1e-9, "blend skipping NaN got {mu}");

        // All-NaN neighbourhood → None.
        let all_nan = encode(2, 2, (10.0, 0.0, 0.0, 10.0), &[f32::NAN; 4]);
        let a2 = FalchiAtlas::from_bytes(&all_nan).unwrap();
        assert!(a2.sample_zenith_mag_per_arcsec2(5.0, 5.0).is_none());
    }

    #[test]
    fn longitude_wraps_into_range() {
        assert!((wrap_longitude_deg(190.0) - (-170.0)).abs() < 1e-9);
        assert!((wrap_longitude_deg(-190.0) - 170.0).abs() < 1e-9);
        assert!((wrap_longitude_deg(180.0) - (-180.0)).abs() < 1e-9);
        assert!((wrap_longitude_deg(0.0)).abs() < 1e-9);
    }
}
