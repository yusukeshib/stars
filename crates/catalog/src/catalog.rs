use glam::Vec3;
use serde::Deserialize;

use crate::color::bv_to_rgb;
use crate::coords::radec_to_cartesian;

#[derive(Debug, Deserialize)]
pub struct RawStar {
    pub id: u32,
    pub proper: Option<String>,
    pub ra: f64,
    pub dec: f64,
    pub dist: Option<f64>,
    pub mag: f64,
    pub absmag: Option<f64>,
    pub ci: Option<f64>,
    pub spect: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct Star {
    pub position: Vec3,
    pub magnitude: f32,
    pub color: [f32; 3],
}

const MAX_MAGNITUDE: f64 = 8.0;
const MAX_DISTANCE: f64 = 100_000.0;

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
            if dist >= MAX_DISTANCE {
                continue;
            }
        }

        let position = radec_to_cartesian(raw.ra, raw.dec);
        let color = bv_to_rgb(raw.ci.unwrap_or(0.0) as f32);

        stars.push(Star {
            position,
            magnitude: raw.mag as f32,
            color,
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
pub fn load_from_file(path: &str) -> Vec<Star> {
    let data = std::fs::read_to_string(path).expect("Failed to read star catalog");
    load_from_csv(&data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_catalog() {
        let csv = r#"id,hip,hd,hr,gl,bf,proper,ra,dec,dist,pmra,pmdec,rv,mag,absmag,spect,ci,x,y,z,vx,vy,vz,rarad,decrad,pmrarad,pmdecrad,bayer,flam,con,comp,comp_primary,base,lum,var,var_min,var_max
1,1,224700,,,,Sirius,6.752477,16.716116,2.637,0.0,0.0,0.0,-1.46,1.45,A1V,-0.01,0,0,0,0,0,0,0,0,0,0,,,Psc,1,1,,1.0,,,
"#;
        let stars = load_from_csv(csv);
        assert_eq!(stars.len(), 1);
        assert!((stars[0].magnitude - (-1.46)).abs() < 0.01);
        assert!(stars[0].position.length() > 0.99);
    }

    #[test]
    fn test_magnitude_filter() {
        let csv = r#"id,hip,hd,hr,gl,bf,proper,ra,dec,dist,pmra,pmdec,rv,mag,absmag,spect,ci,x,y,z,vx,vy,vz,rarad,decrad,pmrarad,pmdecrad,bayer,flam,con,comp,comp_primary,base,lum,var,var_min,var_max
1,1,,,,,Star1,0.0,0.0,10.0,0.0,0.0,0.0,5.0,0.0,G2V,0.0,0,0,0,0,0,0,0,0,0,0,,,Psc,1,1,,1.0,,,
2,2,,,,,Star2,0.0,0.0,10.0,0.0,0.0,0.0,9.0,0.0,G2V,0.0,0,0,0,0,0,0,0,0,0,0,,,Psc,1,1,,1.0,,,
"#;
        let stars = load_from_csv(csv);
        assert_eq!(stars.len(), 1, "Should filter out stars with mag > 8.0");
    }
}
