use std::collections::HashMap;
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

const HYG_CATALOG: &str = "../catalog/data/hyg_v42.csv";
const STAR_LABEL_COUNT: usize = 50;

const CONSTELLATION_NAMES: &[(&str, &str)] = &[
    ("And", "Andromeda"),
    ("Ant", "Antlia"),
    ("Aps", "Apus"),
    ("Aql", "Aquila"),
    ("Aqr", "Aquarius"),
    ("Ara", "Ara"),
    ("Ari", "Aries"),
    ("Aur", "Auriga"),
    ("Boo", "Bootes"),
    ("CMa", "Canis Major"),
    ("CMi", "Canis Minor"),
    ("CVn", "Canes Venatici"),
    ("Cae", "Caelum"),
    ("Cam", "Camelopardalis"),
    ("Cap", "Capricornus"),
    ("Car", "Carina"),
    ("Cas", "Cassiopeia"),
    ("Cen", "Centaurus"),
    ("Cep", "Cepheus"),
    ("Cet", "Cetus"),
    ("Cha", "Chamaeleon"),
    ("Cir", "Circinus"),
    ("Cnc", "Cancer"),
    ("Col", "Columba"),
    ("Com", "Coma Berenices"),
    ("CrA", "Corona Australis"),
    ("CrB", "Corona Borealis"),
    ("Crt", "Crater"),
    ("Cru", "Crux"),
    ("Crv", "Corvus"),
    ("Cyg", "Cygnus"),
    ("Del", "Delphinus"),
    ("Dor", "Dorado"),
    ("Dra", "Draco"),
    ("Equ", "Equuleus"),
    ("Eri", "Eridanus"),
    ("For", "Fornax"),
    ("Gem", "Gemini"),
    ("Gru", "Grus"),
    ("Her", "Hercules"),
    ("Hor", "Horologium"),
    ("Hya", "Hydra"),
    ("Hyi", "Hydrus"),
    ("Ind", "Indus"),
    ("LMi", "Leo Minor"),
    ("Lac", "Lacerta"),
    ("Leo", "Leo"),
    ("Lep", "Lepus"),
    ("Lib", "Libra"),
    ("Lup", "Lupus"),
    ("Lyn", "Lynx"),
    ("Lyr", "Lyra"),
    ("Men", "Mensa"),
    ("Mic", "Microscopium"),
    ("Mon", "Monoceros"),
    ("Mus", "Musca"),
    ("Nor", "Norma"),
    ("Oct", "Octans"),
    ("Oph", "Ophiuchus"),
    ("Ori", "Orion"),
    ("Pav", "Pavo"),
    ("Peg", "Pegasus"),
    ("Per", "Perseus"),
    ("Phe", "Phoenix"),
    ("Pic", "Pictor"),
    ("PsA", "Piscis Austrinus"),
    ("Psc", "Pisces"),
    ("Pup", "Puppis"),
    ("Pyx", "Pyxis"),
    ("Ret", "Reticulum"),
    ("Scl", "Sculptor"),
    ("Sco", "Scorpius"),
    ("Sct", "Scutum"),
    ("Ser", "Serpens"),
    ("Sex", "Sextans"),
    ("Sge", "Sagitta"),
    ("Sgr", "Sagittarius"),
    ("Tau", "Taurus"),
    ("Tel", "Telescopium"),
    ("TrA", "Triangulum Australe"),
    ("Tri", "Triangulum"),
    ("Tuc", "Tucana"),
    ("UMa", "Ursa Major"),
    ("UMi", "Ursa Minor"),
    ("Vel", "Vela"),
    ("Vir", "Virgo"),
    ("Vol", "Volans"),
    ("Vul", "Vulpecula"),
];

#[derive(Debug, Clone)]
struct CatalogRow {
    ra_hours: f64,
    dec_deg: f64,
    mag: f64,
    proper: String,
    bayer: String,
    flam: String,
    con: String,
}

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

    println!("cargo:rerun-if-changed={HYG_CATALOG}");
    write_label_catalog();
}

fn write_label_catalog() {
    let rows = read_hyg_rows(HYG_CATALOG);
    let mut stars = rows.clone();
    stars.sort_by(|a, b| a.mag.total_cmp(&b.mag));
    let star_labels: Vec<_> = stars
        .iter()
        .filter(|row| row.proper != "Sol")
        .take(STAR_LABEL_COUNT)
        .map(|row| {
            let designation = designation(row);
            let text = if row.proper.trim().is_empty() {
                designation
            } else if designation.is_empty() {
                row.proper.clone()
            } else {
                format!("{} / {}", row.proper, designation)
            };
            (
                text,
                radec_to_unit(row.ra_hours, row.dec_deg),
                row.mag as f32,
            )
        })
        .collect();

    let mut sums: HashMap<String, ([f64; 3], f64)> = HashMap::new();
    for row in &rows {
        if row.proper == "Sol" || row.con.trim().is_empty() || row.mag > 5.5 {
            continue;
        }
        let pos = radec_to_unit(row.ra_hours, row.dec_deg);
        // Bright-star weighted centroid: enough to place the label inside the
        // visible asterism without carrying a second boundary dataset.
        let w = 10.0_f64.powf(-0.4 * (row.mag + 1.5));
        let entry = sums.entry(row.con.clone()).or_insert(([0.0; 3], 0.0));
        entry.0[0] += pos[0] as f64 * w;
        entry.0[1] += pos[1] as f64 * w;
        entry.0[2] += pos[2] as f64 * w;
        entry.1 += w;
    }

    let constellation_labels: Vec<_> = CONSTELLATION_NAMES
        .iter()
        .filter_map(|(abbr, name)| {
            let (sum, weight) = sums.get(*abbr)?;
            if *weight <= 0.0 {
                return None;
            }
            let len = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
            if len == 0.0 {
                return None;
            }
            Some((
                *name,
                [
                    (sum[0] / len) as f32,
                    (sum[1] / len) as f32,
                    (sum[2] / len) as f32,
                ],
            ))
        })
        .collect();

    let mut code = String::new();
    code.push_str("// @generated by crates/renderer/build.rs; do not edit.\n");
    code.push_str("pub(crate) struct RawStarLabel { pub(crate) position: [f32; 3], pub(crate) text: &'static str, pub(crate) magnitude: f32 }\n");
    code.push_str("pub(crate) struct RawConstellationLabel { pub(crate) position: [f32; 3], pub(crate) text: &'static str }\n");
    code.push_str("pub(crate) const STAR_LABELS: &[RawStarLabel] = &[\n");
    for (text, pos, mag) in &star_labels {
        code.push_str(&format!(
            "    RawStarLabel {{ position: [{:.6}, {:.6}, {:.6}], text: {:?}, magnitude: {:.3} }},\n",
            pos[0], pos[1], pos[2], text, mag
        ));
    }
    code.push_str("];\n");
    code.push_str("pub(crate) const CONSTELLATION_LABELS: &[RawConstellationLabel] = &[\n");
    for (text, pos) in &constellation_labels {
        code.push_str(&format!(
            "    RawConstellationLabel {{ position: [{:.6}, {:.6}, {:.6}], text: {:?} }},\n",
            pos[0], pos[1], pos[2], text
        ));
    }
    code.push_str("];\n");

    let out_path = Path::new(&env::var("OUT_DIR").expect("OUT_DIR is set")).join("label_data.rs");
    fs::write(out_path, code).expect("write generated label catalog");
}

fn read_hyg_rows(input: &str) -> Vec<CatalogRow> {
    let text = fs::read_to_string(input).expect("read HYG catalog for label generation");
    let mut lines = text.lines();
    let header = lines.next().expect("HYG catalog has header");
    let headers: Vec<String> = header.split(',').map(clean_csv_field).collect();
    let idx = |name: &str| -> usize {
        headers
            .iter()
            .position(|h| h == name)
            .unwrap_or_else(|| panic!("HYG header missing {name}"))
    };
    let i_ra = idx("ra");
    let i_dec = idx("dec");
    let i_mag = idx("mag");
    let i_proper = idx("proper");
    let i_bayer = idx("bayer");
    let i_flam = idx("flam");
    let i_con = idx("con");

    let mut rows = Vec::new();
    for (line_number, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<String> = line.split(',').map(clean_csv_field).collect();
        let parse = |i: usize, name: &str| -> f64 {
            fields
                .get(i)
                .map(String::as_str)
                .unwrap_or("")
                .trim()
                .parse::<f64>()
                .unwrap_or_else(|_| panic!("invalid {name} at {input}:{}", line_number + 2))
        };
        rows.push(CatalogRow {
            ra_hours: parse(i_ra, "ra"),
            dec_deg: parse(i_dec, "dec"),
            mag: parse(i_mag, "mag"),
            proper: fields.get(i_proper).cloned().unwrap_or_default(),
            bayer: fields.get(i_bayer).cloned().unwrap_or_default(),
            flam: fields.get(i_flam).cloned().unwrap_or_default(),
            con: fields.get(i_con).cloned().unwrap_or_default(),
        });
    }
    rows
}

fn clean_csv_field(field: &str) -> String {
    field.trim().trim_matches('"').to_string()
}

fn designation(row: &CatalogRow) -> String {
    let con = row.con.trim();
    if !row.flam.trim().is_empty() && !row.bayer.trim().is_empty() && !con.is_empty() {
        format!("{} {} {}", row.flam.trim(), row.bayer.trim(), con)
    } else if !row.bayer.trim().is_empty() && !con.is_empty() {
        format!("{} {}", row.bayer.trim(), con)
    } else if !row.flam.trim().is_empty() && !con.is_empty() {
        format!("{} {}", row.flam.trim(), con)
    } else {
        String::new()
    }
}

fn radec_to_unit(ra_hours: f64, dec_deg: f64) -> [f32; 3] {
    let ra = ra_hours / 24.0 * std::f64::consts::TAU;
    let dec = dec_deg.to_radians();
    let (sd, cd) = dec.sin_cos();
    let (sr, cr) = ra.sin_cos();
    [(cd * cr) as f32, (cd * sr) as f32, sd as f32]
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
