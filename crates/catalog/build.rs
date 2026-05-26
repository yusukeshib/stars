//! Build-time compaction for catalog data.
//!
//! Three artifacts are emitted into `OUT_DIR`:
//!
//! - `messier.bin` — i16-quantised Messier catalogue (always built; small).
//! - `openngc_bright.bin` — i16-quantised bright NGC / IC subset
//!   (always built; ~30 KB).
//! - `stars.bin` — quantised HYG star catalogue (only when the `embedded`
//!   feature is enabled, because the source CSV is 30 MB).
//!
//! Wire formats are documented per-section. See `src/deepsky.rs` for the
//! Messier / NGC decoder and `src/catalog.rs` for the HYG decoder.

use serde::Deserialize;
use std::env;
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

// ---------------------------------------------------------------------------
// Deep-sky catalogues (always emitted)
// ---------------------------------------------------------------------------

const MESSIER_CATALOG: &str = "data/messier.csv";
const MESSIER_BINARY: &str = "messier.bin";
const MESSIER_BINARY_MAGIC: &[u8; 8] = b"MSSR1\0\0\0";

const OPENNGC_CATALOG: &str = "data/openngc_bright.csv";
const OPENNGC_BINARY: &str = "openngc_bright.bin";
const OPENNGC_BINARY_MAGIC: &[u8; 8] = b"NGCBR1\0\0";

/// Per-object record (16 bytes) shared by Messier and OpenNGC bright tables:
///
/// ```text
///   i16 x 3   J2000 unit-vector position (quantised by i16::MAX)
///   i16       primary identifier (Messier number 1..=110, or NGC/IC number
///             — IC encoded as -(n+1), so IC1 = -2)
///   i16       magnitude * 100 (signed; +9900 sentinel = no photometry)
///   i16       major-axis size in arcminutes * 10 (max ~3276 arcmin)
///   u8        kind tag (see DeepSkyKind in src/deepsky.rs)
///   u8        padding
/// ```
///
/// Per-table headers are 8-byte magic + LE u32 record count. Positions
/// quantise to ~6 x 10⁻⁵ of a unit sphere (≈12 arcsec), well below the
/// renderer's marker scale of arcminutes.
/// 6-byte position + 3 i16 (id, mag, size) + 1-byte kind + 1-byte pad = 14.
const DEEP_SKY_RECORD_LEN: usize = 6 + 3 * 2 + 2;

#[derive(Debug, Clone)]
struct DeepSkyRow {
    /// Encoded primary ID: positive Messier numbers (1..=110); positive NGC
    /// numbers (no Messier conflicts because Messier is filtered upstream);
    /// negative `-(IC_number + 1)` for IC entries so IC1 -> -2, IC500 -> -501.
    primary_id: i16,
    ra_hours: f64,
    dec_deg: f64,
    /// Sentinel `99.0` means OpenNGC published no integrated magnitude
    /// (large diffuse nebulae kept in the bright subset on size alone).
    mag: f64,
    kind_tag: u8,
    size_arcmin: f64,
}

fn kind_tag(type_code: &str) -> u8 {
    match type_code {
        "OC" => 1,
        "GC" => 2,
        "G" => 3,
        "N" => 4,
        "PN" => 5,
        "SNR" => 6,
        _ => 0,
    }
}

fn parse_csv_rows(
    input: &str,
    primary_column: &str,
    decode_primary: impl Fn(&str, usize) -> i16,
) -> Vec<DeepSkyRow> {
    let text = fs::read_to_string(input).unwrap_or_else(|e| panic!("read {input}: {e}"));
    let mut rows = Vec::new();
    let mut header: Option<Vec<String>> = None;
    for (line_number, raw_line) in text.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if header.is_none() {
            header = Some(trimmed.split(',').map(|s| s.trim().to_string()).collect());
            continue;
        }
        let headers = header.as_ref().expect("header set above");
        let fields: Vec<String> = trimmed.split(',').map(clean_csv_field).collect();
        let get = |name: &str| -> &str {
            let idx = headers
                .iter()
                .position(|h| h == name)
                .unwrap_or_else(|| panic!("{input}: missing column {name}"));
            fields.get(idx).map(String::as_str).unwrap_or("").trim()
        };
        let parse_f = |name: &str| -> f64 {
            get(name)
                .parse::<f64>()
                .unwrap_or_else(|_| panic!("{input}: invalid {name} at line {}", line_number + 1))
        };
        let primary_id = decode_primary(get(primary_column), line_number + 1);
        rows.push(DeepSkyRow {
            primary_id,
            ra_hours: parse_f("ra_hours"),
            dec_deg: parse_f("dec_deg"),
            mag: parse_f("mag"),
            kind_tag: kind_tag(get("type")),
            size_arcmin: parse_f("size_arcmin"),
        });
    }
    if rows.is_empty() {
        panic!("{input}: no rows parsed");
    }
    rows
}

fn read_messier_rows(input: &str) -> Vec<DeepSkyRow> {
    let mut rows = parse_csv_rows(input, "m", |value, line| {
        let n: i16 = value
            .parse()
            .unwrap_or_else(|_| panic!("{input}: invalid m at line {line}"));
        if !(1..=110).contains(&n) {
            panic!("{input}: Messier number M{n} out of range at line {line}");
        }
        n
    });
    rows.sort_by_key(|r| r.primary_id);
    let mut seen = [false; 111];
    for row in &rows {
        let idx = row.primary_id as usize;
        if seen[idx] {
            panic!("{input}: duplicate Messier number M{idx}");
        }
        seen[idx] = true;
    }
    for (n, present) in seen.iter().enumerate().skip(1) {
        if !present {
            panic!("{input}: missing Messier number M{n}");
        }
    }
    rows
}

fn read_openngc_rows(input: &str) -> Vec<DeepSkyRow> {
    parse_csv_rows(input, "name", |value, line| {
        // "NGC<number>" -> +n, "IC<number>" -> -(n+1). Suffix letters
        // ("NGC4567A") are not preserved in the i16 primary id; the suffix
        // distinguishes catalogue components in the source CSV but the
        // marker pass only needs position + numeric ID for the label.
        let stripped = value.trim_end_matches(|c: char| c.is_ascii_alphabetic());
        if let Some(rest) = value.strip_prefix("NGC") {
            let digits = rest.trim_end_matches(|c: char| c.is_ascii_alphabetic());
            let n: i32 = digits
                .parse()
                .unwrap_or_else(|_| panic!("{input}: invalid NGC name {value:?} at line {line}"));
            if !(1..=32767).contains(&n) {
                panic!("{input}: NGC{n} out of i16 range at line {line}");
            }
            n as i16
        } else if let Some(rest) = value.strip_prefix("IC") {
            let digits = rest.trim_end_matches(|c: char| c.is_ascii_alphabetic());
            let n: i32 = digits
                .parse()
                .unwrap_or_else(|_| panic!("{input}: invalid IC name {value:?} at line {line}"));
            if !(1..=32766).contains(&n) {
                panic!("{input}: IC{n} out of i16 range at line {line}");
            }
            -(n + 1) as i16
        } else {
            panic!("{input}: unrecognised name {value:?} (raw={stripped:?}) at line {line}");
        }
    })
}

fn write_deepsky_binary(rows: &[DeepSkyRow], out_name: &str, magic: &[u8; 8]) {
    let out_path = Path::new(&env::var("OUT_DIR").expect("OUT_DIR is set")).join(out_name);
    let mut bytes = Vec::with_capacity(12 + rows.len() * DEEP_SKY_RECORD_LEN);
    bytes.extend_from_slice(magic);
    bytes.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    for row in rows {
        let unit = radec_to_unit(row.ra_hours, row.dec_deg);
        for v in unit {
            bytes.extend_from_slice(&quantize_unit_f32(v).to_le_bytes());
        }
        bytes.extend_from_slice(&row.primary_id.to_le_bytes());
        let mag_q = (row.mag * 100.0).round().clamp(-32768.0, 32767.0) as i16;
        bytes.extend_from_slice(&mag_q.to_le_bytes());
        let size_q = (row.size_arcmin * 10.0).round().clamp(0.0, 32767.0) as i16;
        bytes.extend_from_slice(&size_q.to_le_bytes());
        bytes.push(row.kind_tag);
        bytes.push(0);
    }
    fs::write(out_path, bytes).expect("write compact deep-sky catalog");
}

fn clean_csv_field(field: &str) -> String {
    field.trim().trim_matches('"').to_string()
}

fn radec_to_unit(ra_hours: f64, dec_deg: f64) -> [f32; 3] {
    let ra = ra_hours / 24.0 * std::f64::consts::TAU;
    let dec = dec_deg.to_radians();
    let (sd, cd) = dec.sin_cos();
    let (sr, cr) = ra.sin_cos();
    [(cd * cr) as f32, (cd * sr) as f32, sd as f32]
}

fn quantize_unit_f32(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}

// ---------------------------------------------------------------------------
// HYG star catalogue (embedded feature only)
// ---------------------------------------------------------------------------

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
    println!("cargo:rerun-if-changed={MESSIER_CATALOG}");
    println!("cargo:rerun-if-changed={OPENNGC_CATALOG}");
    let messier_rows = read_messier_rows(MESSIER_CATALOG);
    write_deepsky_binary(&messier_rows, MESSIER_BINARY, MESSIER_BINARY_MAGIC);
    let openngc_rows = read_openngc_rows(OPENNGC_CATALOG);
    write_deepsky_binary(&openngc_rows, OPENNGC_BINARY, OPENNGC_BINARY_MAGIC);

    // Only the WASM/browser build uses the embedded star catalog. Native/test
    // builds keep using the CSV reader and should not require the large data file.
    if env::var_os("CARGO_FEATURE_EMBEDDED").is_none() {
        return;
    }

    println!("cargo:rerun-if-changed=data/hyg_v42.csv");
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
