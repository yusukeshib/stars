#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConstellationSegment {
    pub start: [f32; 3],
    pub end: [f32; 3],
}

const CONSTELLATION_BOUNDARY_MAGIC: &[u8; 8] = b"CNBND1\0\0";
const CONSTELLATION_LINE_MAGIC: &[u8; 8] = b"CNLIN1\0\0";
const CONSTELLATION_BINARY_HEADER_LEN: usize = 12;
const CONSTELLATION_BINARY_RECORD_LEN: usize = 12;
const CONSTELLATION_BOUNDARY_DATA: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/constellation_boundaries.bin"));
const CONSTELLATION_LINE_DATA: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/constellation_lines.bin"));

/// Modern western constellation stick figures, represented as J2000 unit-vector
/// line segments. The build script compacts the source CSV into an i16 binary
/// table so embedding these overlays follows the same pattern as the star
/// catalog.
pub fn constellation_lines() -> Vec<ConstellationSegment> {
    decode_constellation_segments(CONSTELLATION_LINE_DATA, CONSTELLATION_LINE_MAGIC)
}

/// IAU/Delporte constellation boundaries, represented as J2000 unit-vector line
/// segments. The source CSV is derived from CDS VI/49 boundary data.
pub fn constellation_boundaries() -> Vec<ConstellationSegment> {
    decode_constellation_segments(CONSTELLATION_BOUNDARY_DATA, CONSTELLATION_BOUNDARY_MAGIC)
}

fn decode_constellation_segments(
    data: &[u8],
    expected_magic: &[u8; 8],
) -> Vec<ConstellationSegment> {
    assert!(
        data.len() >= CONSTELLATION_BINARY_HEADER_LEN,
        "constellation catalog is shorter than its header"
    );
    assert_eq!(
        &data[..expected_magic.len()],
        expected_magic,
        "constellation catalog has an unexpected magic header"
    );
    let count = u32::from_le_bytes(data[8..12].try_into().expect("fixed-size count")) as usize;
    let expected_len = CONSTELLATION_BINARY_HEADER_LEN + count * CONSTELLATION_BINARY_RECORD_LEN;
    assert_eq!(
        data.len(),
        expected_len,
        "constellation catalog length does not match its segment count"
    );

    let mut segments = Vec::with_capacity(count);
    for record in
        data[CONSTELLATION_BINARY_HEADER_LEN..].chunks_exact(CONSTELLATION_BINARY_RECORD_LEN)
    {
        segments.push(ConstellationSegment {
            start: [
                decode_unit_i16(record, 0),
                decode_unit_i16(record, 2),
                decode_unit_i16(record, 4),
            ],
            end: [
                decode_unit_i16(record, 6),
                decode_unit_i16(record, 8),
                decode_unit_i16(record, 10),
            ],
        });
    }
    segments
}

fn decode_unit_i16(record: &[u8], offset: usize) -> f32 {
    i16::from_le_bytes(
        record[offset..offset + 2]
            .try_into()
            .expect("fixed-size constellation coordinate"),
    ) as f32
        / i16::MAX as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constellation_line_data_is_well_formed() {
        let segments = constellation_lines();
        assert_eq!(segments.len(), 743);
        assert_segments_are_unit_length(&segments);
    }

    #[test]
    fn constellation_boundary_data_is_well_formed() {
        let segments = constellation_boundaries();
        assert_eq!(segments.len(), 1565);
        assert_segments_are_unit_length(&segments);
    }

    fn assert_segments_are_unit_length(segments: &[ConstellationSegment]) {
        for segment in segments {
            for point in [segment.start, segment.end] {
                let r = (point[0].powi(2) + point[1].powi(2) + point[2].powi(2)).sqrt();
                assert!(
                    (r - 1.0).abs() < 1e-4,
                    "constellation point is not unit length"
                );
            }
        }
    }
}
