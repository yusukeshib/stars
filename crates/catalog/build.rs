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
}

const MAX_MAGNITUDE: f64 = 8.0;
const MAX_DISTANCE_PC: f64 = 100_000.0;
const MAGIC: &[u8; 8] = b"STRBIN1\0";

fn main() {
    println!("cargo:rerun-if-changed=data/hyg_v42.csv");

    // Only the WASM/browser build uses the embedded catalog. Native/test builds
    // keep using the CSV reader and should not require the large data file.
    if env::var_os("CARGO_FEATURE_EMBEDDED").is_none() {
        return;
    }

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
        if raw.dist.is_some_and(|dist| dist >= MAX_DISTANCE_PC) {
            continue;
        }

        let (x, y, z) = radec_hours_deg_to_cartesian(raw.ra, raw.dec);
        records.push(Record {
            x: quantize_unit(x),
            y: quantize_unit(y),
            z: quantize_unit(z),
            mag_cent: quantize_scaled(raw.mag, 100.0),
            ci_milli: quantize_scaled(raw.ci.unwrap_or(0.0), 1000.0),
        });
    }

    let mut writer = BufWriter::new(File::create(out_path).expect("create compact star catalog"));
    writer.write_all(MAGIC).expect("write magic");
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
    }
}

struct Record {
    x: i16,
    y: i16,
    z: i16,
    mag_cent: i16,
    ci_milli: i16,
}

fn radec_hours_deg_to_cartesian(ra_hours: f64, dec_degrees: f64) -> (f64, f64, f64) {
    let ra = ra_hours * (std::f64::consts::PI / 12.0);
    let dec = dec_degrees * (std::f64::consts::PI / 180.0);
    (dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin())
}

fn quantize_unit(value: f64) -> i16 {
    quantize_scaled(value.clamp(-1.0, 1.0), i16::MAX as f64)
}

fn quantize_scaled(value: f64, scale: f64) -> i16 {
    (value * scale)
        .round()
        .clamp(i16::MIN as f64, i16::MAX as f64) as i16
}
