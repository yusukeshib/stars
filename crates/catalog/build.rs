use serde::Deserialize;
use std::env;
use std::fs::{self, File};
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
/// Keep the embedded catalog's distance filter in lock-step with the runtime
/// CSV loader: HYG uses `dist = 100000` as the sentinel for missing / invalid
/// parallax, and the synthetic Sun row has `dist = 0`. Neither belongs in the
/// background star catalog baked into the WASM bundle.
const MIN_DISTANCE_PC: f64 = 0.0;
const MAX_DISTANCE_PC: f64 = 100_000.0;
const STAR_MAGIC: &[u8; 8] = b"STRBIN1\0";

const CONSTELLATION_TABLES: &[(&str, &str, &[u8; 8])] = &[
    (
        "data/constellation_boundaries.csv",
        "constellation_boundaries.bin",
        b"CNBND1\0\0",
    ),
    (
        "data/constellation_lines.csv",
        "constellation_lines.bin",
        b"CNLIN1\0\0",
    ),
];

fn main() {
    generate_constellation_tables();

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
        if raw
            .dist
            .is_some_and(|dist| dist <= MIN_DISTANCE_PC || dist >= MAX_DISTANCE_PC)
        {
            continue;
        }

        let (x, y, z) = radec_hours_deg_to_cartesian(raw.ra, raw.dec);
        records.push(StarRecord {
            x: quantize_unit_f64(x),
            y: quantize_unit_f64(y),
            z: quantize_unit_f64(z),
            mag_cent: quantize_scaled(raw.mag, 100.0),
            ci_milli: quantize_scaled(raw.ci.unwrap_or(0.0), 1000.0),
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
    }
}

struct StarRecord {
    x: i16,
    y: i16,
    z: i16,
    mag_cent: i16,
    ci_milli: i16,
}

fn generate_constellation_tables() {
    for (input, output, magic) in CONSTELLATION_TABLES {
        println!("cargo:rerun-if-changed={input}");
        let rows = read_constellation_segments(input);
        let out_path = Path::new(&env::var("OUT_DIR").expect("OUT_DIR is set")).join(output);

        let mut bytes = Vec::with_capacity(12 + rows.len() * 12);
        bytes.extend_from_slice(*magic);
        bytes.extend_from_slice(&(rows.len() as u32).to_le_bytes());
        for row in rows {
            for value in row {
                bytes.extend_from_slice(&quantize_unit_f32(value).to_le_bytes());
            }
        }
        fs::write(out_path, bytes).expect("write compact constellation catalog");
    }
}

fn read_constellation_segments(input: &str) -> Vec<[f32; 6]> {
    let text = fs::read_to_string(input).expect("read constellation data table");
    let mut rows = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("x0,") {
            continue;
        }
        let mut fields = trimmed.split(',');
        let mut row = [0.0_f32; 6];
        for slot in &mut row {
            let field = fields.next().unwrap_or_else(|| {
                panic!(
                    "expected 6 comma-separated fields in {input} at line {}",
                    line_number + 1
                )
            });
            *slot = field
                .trim()
                .parse::<f32>()
                .unwrap_or_else(|_| panic!("invalid float in {input} at line {}", line_number + 1));
        }
        if fields.next().is_some() {
            panic!(
                "expected 6 comma-separated fields in {input} at line {}",
                line_number + 1
            );
        }
        assert_unit_vector(input, line_number + 1, &row[0..3]);
        assert_unit_vector(input, line_number + 1, &row[3..6]);
        rows.push(row);
    }
    if rows.is_empty() {
        panic!("{input} contains no constellation segments");
    }
    rows
}

fn assert_unit_vector(input: &str, line_number: usize, v: &[f32]) {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    assert!(
        length.is_finite() && (length - 1.0).abs() < 1.0e-4,
        "non-unit vector in {input} at line {line_number}: length={length}"
    );
}

fn radec_hours_deg_to_cartesian(ra_hours: f64, dec_degrees: f64) -> (f64, f64, f64) {
    let ra = ra_hours * (std::f64::consts::PI / 12.0);
    let dec = dec_degrees * (std::f64::consts::PI / 180.0);
    (dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin())
}

fn quantize_unit_f64(value: f64) -> i16 {
    quantize_scaled(value.clamp(-1.0, 1.0), i16::MAX as f64)
}

fn quantize_scaled(value: f64, scale: f64) -> i16 {
    (value * scale)
        .round()
        .clamp(i16::MIN as f64, i16::MAX as f64) as i16
}

fn quantize_unit_f32(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}
