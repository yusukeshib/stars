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
const MESSIER_CATALOG: &str = "data/messier.csv";
const MESSIER_BINARY: &str = "messier.bin";
const MESSIER_BINARY_MAGIC: &[u8; 8] = b"MSSR1\0\0\0";

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
    println!("cargo:rerun-if-changed={MESSIER_CATALOG}");
    let messier_rows = read_messier_rows(MESSIER_CATALOG);
    write_messier_binary(&messier_rows);
    write_label_catalog(&messier_rows);
}

// ---------------------------------------------------------------------------
// Messier catalog: source CSV → OUT_DIR/messier.bin
//
// The binary format mirrors the constellation tables so the renderer's
// decoder stays small: 8-byte magic + 4-byte LE count + N records.
//
// Per-object record (24 bytes):
//   i16 × 3   J2000 unit-vector position (quantised by i16::MAX)
//   i16       Messier number (1..=110)
//   i16       NGC number (-1 if no NGC entry)
//   i16       magnitude × 100 (signed; -1 → unknown if ever needed)
//   i16       major-axis size in arcminutes × 10 (max ~3276 arcmin = 54.6°,
//             well above the largest Messier object Pleiades ≈ 110')
//   u8        kind tag (see MessierKind in src/deepsky.rs)
//   u8        padding
//
// Quantisation precision: positions are accurate to ~10⁻⁴ of a unit sphere
// (≈0.6 arcsec) which is far below the renderer's marker scale.
// ---------------------------------------------------------------------------

// The build script only needs the columns it emits into either `messier.bin`
// (consumed by the marker pass) or `label_data.rs` (consumed by the text
// pass). Adding the `name` / `ngc` / `kind_tag` fields back is the natural
// extension point when richer Messier metadata is exposed (P3-02 identifier
// preservation, future hover/copy UI).
#[derive(Debug, Clone)]
struct MessierRow {
    m: u16,
    ngc: Option<u16>,
    ra_hours: f64,
    dec_deg: f64,
    mag: f64,
    kind_tag: u8,
    size_arcmin: f64,
}

fn read_messier_rows(input: &str) -> Vec<MessierRow> {
    let text = fs::read_to_string(input).expect("read Messier catalog");
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
        let parse_f64 = |name: &str| -> f64 {
            get(name)
                .parse::<f64>()
                .unwrap_or_else(|_| panic!("{input}: invalid {name} at line {}", line_number + 1))
        };
        let m_value: u16 = get("m")
            .parse()
            .unwrap_or_else(|_| panic!("{input}: invalid m at line {}", line_number + 1));
        if !(1..=110).contains(&m_value) {
            panic!(
                "{input}: Messier number {m_value} out of range at line {}",
                line_number + 1
            );
        }
        let ngc_raw = get("ngc");
        let ngc = ngc_raw
            .strip_prefix("NGC")
            .and_then(|s| s.parse::<u16>().ok());
        let kind_tag = match get("type") {
            "OC" => 1u8,
            "GC" => 2,
            "G" => 3,
            "N" => 4,
            "PN" => 5,
            "SNR" => 6,
            _ => 0,
        };
        rows.push(MessierRow {
            m: m_value,
            ngc,
            ra_hours: parse_f64("ra_hours"),
            dec_deg: parse_f64("dec_deg"),
            mag: parse_f64("mag"),
            kind_tag,
            size_arcmin: parse_f64("size_arcmin"),
        });
    }
    if rows.is_empty() {
        panic!("{input}: no Messier rows parsed");
    }
    // Guarantee a stable on-disk order so the binary hash is reproducible.
    rows.sort_by_key(|r| r.m);
    // Sanity: every Messier number 1..=110 present exactly once.
    let mut seen = [false; 111];
    for row in &rows {
        let idx = row.m as usize;
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

fn write_messier_binary(rows: &[MessierRow]) {
    const RECORD_LEN: usize = 6 + 4 * 2 + 2;
    let out_path = Path::new(&env::var("OUT_DIR").expect("OUT_DIR is set")).join(MESSIER_BINARY);
    let mut bytes = Vec::with_capacity(12 + rows.len() * RECORD_LEN);
    bytes.extend_from_slice(MESSIER_BINARY_MAGIC);
    bytes.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    for row in rows {
        let unit = radec_to_unit(row.ra_hours, row.dec_deg);
        for v in unit {
            bytes.extend_from_slice(&quantize_unit_f32(v).to_le_bytes());
        }
        bytes.extend_from_slice(&(row.m as i16).to_le_bytes());
        bytes.extend_from_slice(&(row.ngc.map(|n| n as i16).unwrap_or(-1)).to_le_bytes());
        let mag_q = (row.mag * 100.0).round().clamp(-32768.0, 32767.0) as i16;
        bytes.extend_from_slice(&mag_q.to_le_bytes());
        let size_q = (row.size_arcmin * 10.0).round().clamp(0.0, 32767.0) as i16;
        bytes.extend_from_slice(&size_q.to_le_bytes());
        bytes.push(row.kind_tag);
        bytes.push(0);
    }
    fs::write(out_path, bytes).expect("write compact Messier catalog");
}

fn write_label_catalog(messier_rows: &[MessierRow]) {
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

    // Messier label catalogue: "M1", "M31", ... rendered with the same text
    // pipeline as star/constellation labels. The magnitude is forwarded as
    // the priority key so brighter objects survive screen-space culling first,
    // matching the existing star-label policy.
    code.push_str("pub(crate) struct RawMessierLabel { pub(crate) position: [f32; 3], pub(crate) text: &'static str, pub(crate) magnitude: f32 }\n");
    code.push_str("pub(crate) const MESSIER_LABELS: &[RawMessierLabel] = &[\n");
    for row in messier_rows {
        let pos = radec_to_unit(row.ra_hours, row.dec_deg);
        let label = format!("M{}", row.m);
        code.push_str(&format!(
            "    RawMessierLabel {{ position: [{:.6}, {:.6}, {:.6}], text: {:?}, magnitude: {:.3} }},\n",
            pos[0], pos[1], pos[2], label, row.mag as f32
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
