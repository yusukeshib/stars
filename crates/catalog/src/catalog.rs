use glam::Vec3;
use serde::Deserialize;

use crate::color::bv_to_rgb;
use crate::coords::radec_hours_deg_to_cartesian;

#[derive(Debug, Deserialize)]
struct RawStar {
    ra: f64,
    dec: f64,
    dist: Option<f64>,
    mag: f64,
    ci: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct Star {
    pub position: Vec3,
    pub magnitude: f32,
    pub color: [f32; 3],
}

const MAX_MAGNITUDE: f64 = 8.0;
/// HYG fills its `dist` (parsecs) column with the sentinel value `100000` for
/// rows where the parallax is missing, negative, or numerically meaningless.
/// We drop those rows: their on-sky position may still be fine, but using
/// them in distance-aware filtering or future 3D rendering would propagate
/// the sentinel as a real distance. Rows with no `dist` at all are kept
/// (HYG leaves the cell empty in that case). 100 kpc is far beyond the
/// stellar Milky Way disc (≈30 kpc), so no real star is filtered out by
/// this threshold.
///
/// HYG also contains the Sun (`proper=Sol`) as a synthetic row at `dist=0`,
/// `ra=0`, `dec=0`, `mag=-26.7`. It is not a background catalogue star: if
/// uploaded to the star renderer it becomes an enormous saturated PSF that
/// looks like a bogus Moon/Sun disk. Solar-system bodies must be rendered by
/// the ephemeris path instead, so non-positive distances are rejected here.
const MIN_DISTANCE_PC: f64 = 0.0;
const MAX_DISTANCE_PC: f64 = 100_000.0;

pub fn load_from_csv(data: &str) -> Vec<Star> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(data.as_bytes());

    let mut stars = Vec::new();

    for result in reader.deserialize::<RawStar>() {
        let raw = match result {
            Ok(r) => r,
            Err(e) => {
                log::warn!("Skipping malformed row: {e}");
                continue;
            }
        };

        if raw.mag > MAX_MAGNITUDE {
            continue;
        }
        if let Some(dist) = raw.dist {
            if dist <= MIN_DISTANCE_PC || dist >= MAX_DISTANCE_PC {
                continue;
            }
        }

        stars.push(Star {
            position: radec_hours_deg_to_cartesian(raw.ra, raw.dec),
            magnitude: raw.mag as f32,
            color: bv_to_rgb(raw.ci.unwrap_or(0.0) as f32),
        });
    }

    log::info!("Loaded {} stars (mag <= {MAX_MAGNITUDE})", stars.len());
    stars
}

#[cfg(feature = "embedded")]
pub fn load_embedded() -> Vec<Star> {
    const CSV_DATA: &str = include_str!("../data/hyg_v42.csv");
    load_from_csv(CSV_DATA)
}

#[cfg(feature = "filesystem")]
pub fn load_from_file(path: impl AsRef<std::path::Path>) -> std::io::Result<Vec<Star>> {
    let data = std::fs::read_to_string(path.as_ref())?;
    Ok(load_from_csv(&data))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "id,hip,hd,hr,gl,bf,proper,ra,dec,dist,pmra,pmdec,rv,mag,absmag,spect,ci,x,y,z,vx,vy,vz,rarad,decrad,pmrarad,pmdecrad,bayer,flam,con,comp,comp_primary,base,lum,var,var_min,var_max";

    #[test]
    fn loads_a_star() {
        let csv = format!(
            "{HEADER}\n1,1,224700,,,,Sirius,6.752477,16.716116,2.637,0.0,0.0,0.0,-1.46,1.45,A1V,-0.01,0,0,0,0,0,0,0,0,0,0,,,Psc,1,1,,1.0,,,\n"
        );
        let stars = load_from_csv(&csv);
        assert_eq!(stars.len(), 1);
        assert!((stars[0].magnitude - (-1.46)).abs() < 0.01);
        assert!(stars[0].position.length() > 0.99);
    }

    #[test]
    fn filters_dim_stars() {
        let csv = format!(
            "{HEADER}\n\
             1,1,,,,,Star1,0.0,0.0,10.0,0.0,0.0,0.0,5.0,0.0,G2V,0.0,0,0,0,0,0,0,0,0,0,0,,,Psc,1,1,,1.0,,,\n\
             2,2,,,,,Star2,0.0,0.0,10.0,0.0,0.0,0.0,9.0,0.0,G2V,0.0,0,0,0,0,0,0,0,0,0,0,,,Psc,1,1,,1.0,,,\n"
        );
        let stars = load_from_csv(&csv);
        assert_eq!(stars.len(), 1, "should filter mag > {MAX_MAGNITUDE}");
    }

    #[test]
    fn filters_synthetic_sun_row() {
        let csv = format!(
            "{HEADER}\n\
             0,,,,,,Sol,0.0,0.0,0.0,0.0,0.0,0.0,-26.7,4.85,G2V,0.656,0,0,0,0,0,0,0,0,0,0,,,Ori,1,1,,1.0,,,\n\
             1,1,,,,,Sirius,6.752477,-16.716116,2.637,0.0,0.0,0.0,-1.46,1.45,A1V,-0.01,0,0,0,0,0,0,0,0,0,0,,,CMa,1,1,,1.0,,,\n"
        );
        let stars = load_from_csv(&csv);
        assert_eq!(stars.len(), 1, "the Sun is not a catalogue star");
        assert!((stars[0].magnitude - (-1.46)).abs() < 0.01);
    }
}
