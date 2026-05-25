use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

#[derive(Debug, Deserialize)]
struct RawStar {
    ra: f64,
    dec: f64,
    dist: Option<f64>,
    mag: f64,
    ci: Option<f64>,
    pmrarad: Option<f64>,
    pmdecrad: Option<f64>,
}

const MAX_MAGNITUDE: f64 = 8.0;
/// Keep the embedded catalog's distance filter in lock-step with the runtime
/// CSV loader: HYG uses `dist = 100000` as the sentinel for missing / invalid
/// parallax, and the synthetic Sun row has `dist = 0`. Neither belongs in the
/// background star catalog baked into the WASM bundle.
const MIN_DISTANCE_PC: f64 = 0.0;
const MAX_DISTANCE_PC: f64 = 100_000.0;
const STAR_MAGIC: &[u8; 8] = b"STRBIN3\0";

fn main() {
    println!("cargo:rerun-if-changed=data/hyg_v42.csv");

    // Only the WASM/browser build uses the embedded star catalog. Native/test
    // builds keep using the CSV reader and should not require the large data file.
    if env::var_os("CARGO_FEATURE_EMBEDDED").is_none() {
        return;
    }

    generate_star_catalog();
}

fn generate_star_catalog() {
    let csv_path = Path::new("data/hyg_v42.csv");
    let out_path = Path::new(&env::var("OUT_DIR").expect("OUT_DIR is set")).join("stars.bin");

    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .from_path(csv_path)
        .expect("open embedded star catalog CSV");

    let mut records = Vec::new();
    for result in reader.deserialize::<RawStar>() {
        let raw = result.expect("parse embedded star catalog row");
        if raw.mag > MAX_MAGNITUDE {
            continue;
        }
        let distance_pc = match raw.dist {
            Some(dist) if dist > MIN_DISTANCE_PC && dist < MAX_DISTANCE_PC => dist as f32,
            Some(_) => continue,
            None => 1.0,
        };

        let (x, y, z) = radec_hours_deg_to_cartesian(raw.ra, raw.dec);
        let (pmx, pmy, pmz) = proper_motion_vector_radians_per_year(
            raw.ra,
            raw.dec,
            raw.pmrarad.unwrap_or(0.0),
            raw.pmdecrad.unwrap_or(0.0),
        );
        records.push(StarRecord {
            x: quantize_unit_f64(x),
            y: quantize_unit_f64(y),
            z: quantize_unit_f64(z),
            mag_cent: quantize_scaled(raw.mag, 100.0),
            ci_milli: quantize_scaled(raw.ci.unwrap_or(0.0), 1000.0),
            pmx,
            pmy,
            pmz,
            distance_pc,
        });
    }

    let mut writer = BufWriter::new(File::create(out_path).expect("create compact star catalog"));
    writer.write_all(STAR_MAGIC).expect("write magic");
    writer
        .write_all(&(records.len() as u32).to_le_bytes())
        .expect("write star count");
    for record in records {
        writer.write_all(&record.x.to_le_bytes()).expect("write x");
        writer.write_all(&record.y.to_le_bytes()).expect("write y");
        writer.write_all(&record.z.to_le_bytes()).expect("write z");
        writer
            .write_all(&record.mag_cent.to_le_bytes())
            .expect("write magnitude");
        writer
            .write_all(&record.ci_milli.to_le_bytes())
            .expect("write color index");
        writer
            .write_all(&record.pmx.to_le_bytes())
            .expect("write pmx");
        writer
            .write_all(&record.pmy.to_le_bytes())
            .expect("write pmy");
        writer
            .write_all(&record.pmz.to_le_bytes())
            .expect("write pmz");
        writer
            .write_all(&record.distance_pc.to_le_bytes())
            .expect("write distance");
    }
}

struct StarRecord {
    x: i16,
    y: i16,
    z: i16,
    mag_cent: i16,
    ci_milli: i16,
    pmx: f32,
    pmy: f32,
    pmz: f32,
    distance_pc: f32,
}

fn radec_hours_deg_to_cartesian(ra_hours: f64, dec_degrees: f64) -> (f64, f64, f64) {
    let ra = ra_hours * (std::f64::consts::PI / 12.0);
    let dec = dec_degrees * (std::f64::consts::PI / 180.0);
    (dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin())
}

fn proper_motion_vector_radians_per_year(
    ra_hours: f64,
    dec_degrees: f64,
    pmra_rad_year: f64,
    pmdec_rad_year: f64,
) -> (f32, f32, f32) {
    let ra = ra_hours * (std::f64::consts::PI / 12.0);
    let dec = dec_degrees * (std::f64::consts::PI / 180.0);
    let (sin_ra, cos_ra) = ra.sin_cos();
    let (sin_dec, cos_dec) = dec.sin_cos();
    let e_ra = [-sin_ra, cos_ra, 0.0];
    let e_dec = [-sin_dec * cos_ra, -sin_dec * sin_ra, cos_dec];
    (
        (pmra_rad_year * e_ra[0] + pmdec_rad_year * e_dec[0]) as f32,
        (pmra_rad_year * e_ra[1] + pmdec_rad_year * e_dec[1]) as f32,
        (pmra_rad_year * e_ra[2] + pmdec_rad_year * e_dec[2]) as f32,
    )
}

fn quantize_unit_f64(value: f64) -> i16 {
    quantize_scaled(value.clamp(-1.0, 1.0), i16::MAX as f64)
}

fn quantize_scaled(value: f64, scale: f64) -> i16 {
    (value * scale)
        .round()
        .clamp(i16::MIN as f64, i16::MAX as f64) as i16
}
