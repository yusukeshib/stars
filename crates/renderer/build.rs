use std::env;
use std::fs;
use std::path::Path;

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

fn quantize_unit_f32(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
}
