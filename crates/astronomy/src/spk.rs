//! JPL DE440-class ephemeris reader: a DAF/SPK Chebyshev kernel parser.
//!
//! This is the `L-06` Chebyshev-kernel reader. It parses NASA NAIF
//! **DAF/SPK** files (the binary format JPL distributes DE440 / DE441 in) and
//! evaluates **Type 2** (Chebyshev position) and **Type 3** (Chebyshev
//! position + velocity) segments, with body-id chaining through the SPK's
//! center tree (e.g. Mars barycenter → SSB, Earth → Earth-Moon barycenter →
//! SSB). It is dependency-free and endianness-aware so it can read the
//! little-endian `.bsp` kernels JPL ships as well as big-endian transfers.
//!
//! # What this buys (`L-06`)
//!
//! The default Sun / Moon / planet path in [`crate::ephemeris`] uses the
//! `astro` crate's truncated VSOP87 / ELP2000 series (visual / planning tier,
//! a few arcseconds). Loading a DE440 kernel through [`SpkKernel`] upgrades
//! that to publication-grade Chebyshev states (Park et al. 2021), while the
//! analytic series remain the offline / WASM fallback — DE440 kernels are
//! large binaries that are not bundled with the crate.
//!
//! # Status
//!
//! Shipped and unit-tested here: the DAF/SPK binary reader, segment selection,
//! Chebyshev position **and** velocity evaluation, center chaining, and the
//! geocentric-equatorial reduction. **Deferred** (see `ROADMAP.md` `L-06` /
//! `DATA_SOURCES.md`): committing or fetching an actual DE440 kernel and the
//! JPL Horizons sub-arcsecond cross-check — both require the multi-megabyte
//! external kernel, so they are out of scope for an offline build. The reader
//! is validated against a synthetic, spec-accurate in-memory SPK whose
//! Chebyshev coefficients are known in closed form, which pins the parser and
//! the evaluator to the published DAF/SPK layout.
//!
//! # References
//! - Park, R. S. et al. 2021, AJ 161, 105 (DE440 / DE441).
//! - Acton, C. H. 1996, Planet. Space Sci. 44, 65 (SPICE toolkit).
//! - NAIF, *DAF Required Reading* and *SPK Required Reading* (segment types 2
//!   and 3, the DAF file / summary record layout).

use std::collections::HashMap;

/// Seconds per day (SPK ephemeris time is seconds past the J2000 epoch).
const SECONDS_PER_DAY: f64 = 86_400.0;
/// Julian Date of the J2000.0 epoch (TDB), the SPK time origin.
const J2000_JD: f64 = 2_451_545.0;
/// Bytes per DAF physical record / per addressable block.
const RECORD_BYTES: usize = 1024;
/// Bytes per double-precision word (DAF word addressing is 8-byte words).
const WORD_BYTES: usize = 8;

/// NAIF integer body identifiers used when querying a kernel. The values match
/// the SPICE convention (barycenters 0–9, Sun 10, planet/​satellite bodies
/// `100·n + m`). Only the bodies the renderer needs are named; any `i32` id is
/// accepted by [`SpkKernel::state_km`].
pub mod naif {
    pub const SOLAR_SYSTEM_BARYCENTER: i32 = 0;
    pub const MERCURY_BARYCENTER: i32 = 1;
    pub const VENUS_BARYCENTER: i32 = 2;
    pub const EARTH_MOON_BARYCENTER: i32 = 3;
    pub const MARS_BARYCENTER: i32 = 4;
    pub const JUPITER_BARYCENTER: i32 = 5;
    pub const SATURN_BARYCENTER: i32 = 6;
    pub const URANUS_BARYCENTER: i32 = 7;
    pub const NEPTUNE_BARYCENTER: i32 = 8;
    pub const SUN: i32 = 10;
    pub const MERCURY: i32 = 199;
    pub const VENUS: i32 = 299;
    pub const EARTH: i32 = 399;
    pub const MOON: i32 = 301;
    pub const MARS: i32 = 499;
}

/// Errors returned while parsing or evaluating a DAF/SPK kernel.
#[derive(Debug, Clone, PartialEq)]
pub enum SpkError {
    /// The blob is shorter than a required record or word range.
    Truncated,
    /// The leading ID word is not a `DAF/SPK` (or legacy `NAIF/DAF`) marker.
    NotDafSpk,
    /// The `LOCFMT` endianness tag is neither `LTL-IEEE` nor `BIG-IEEE`.
    UnknownEndianness,
    /// The DAF header declared a summary layout this reader does not support
    /// (SPK requires `ND = 2`, `NI = 6`).
    BadSummaryLayout { nd: i32, ni: i32 },
    /// A segment used an SPK data type other than 2 (Chebyshev position) or 3
    /// (Chebyshev position + velocity).
    UnsupportedSegmentType(i32),
    /// No segment covers `(body, epoch)` while resolving a state.
    NoCoverage { body: i32, et_seconds: f64 },
    /// The body's center chain to the solar-system barycenter is broken or
    /// cyclic.
    BrokenCenterChain(i32),
}

impl core::fmt::Display for SpkError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SpkError::Truncated => write!(f, "SPK blob is truncated"),
            SpkError::NotDafSpk => write!(f, "not a DAF/SPK kernel (bad ID word)"),
            SpkError::UnknownEndianness => write!(f, "unknown DAF LOCFMT endianness tag"),
            SpkError::BadSummaryLayout { nd, ni } => {
                write!(
                    f,
                    "unsupported DAF summary layout ND={nd}, NI={ni} (SPK needs 2/6)"
                )
            }
            SpkError::UnsupportedSegmentType(t) => {
                write!(f, "unsupported SPK segment data type {t} (only 2 and 3)")
            }
            SpkError::NoCoverage { body, et_seconds } => {
                write!(f, "no SPK segment covers body {body} at ET {et_seconds} s")
            }
            SpkError::BrokenCenterChain(b) => write!(f, "broken SPK center chain for body {b}"),
        }
    }
}

impl std::error::Error for SpkError {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Endian {
    Little,
    Big,
}

impl Endian {
    fn f64(self, b: &[u8]) -> f64 {
        let a: [u8; 8] = b.try_into().expect("8 bytes");
        match self {
            Endian::Little => f64::from_le_bytes(a),
            Endian::Big => f64::from_be_bytes(a),
        }
    }

    fn i32(self, b: &[u8]) -> i32 {
        let a: [u8; 4] = b.try_into().expect("4 bytes");
        match self {
            Endian::Little => i32::from_le_bytes(a),
            Endian::Big => i32::from_be_bytes(a),
        }
    }
}

/// One Chebyshev SPK segment (data type 2 or 3) parsed into evaluable form.
#[derive(Debug, Clone)]
struct Segment {
    target: i32,
    center: i32,
    start_et: f64,
    end_et: f64,
    /// `true` for Type 3 (explicit velocity coefficients), `false` for Type 2.
    has_velocity: bool,
    /// Initial epoch of the first record (seconds past J2000 TDB).
    init: f64,
    /// Seconds covered by each record.
    interval: f64,
    /// Doubles per record (`2 + components·(degree+1)`).
    record_size: usize,
    /// Number of records.
    record_count: usize,
    /// All record doubles, `record_count · record_size` long.
    data: Vec<f64>,
}

impl Segment {
    fn components(&self) -> usize {
        if self.has_velocity {
            6
        } else {
            3
        }
    }

    fn coeffs_per_component(&self) -> usize {
        (self.record_size - 2) / self.components()
    }

    /// Geometric state `[x, y, z, vx, vy, vz]` (km, km/s) of `target` relative
    /// to `center` at ephemeris time `et` (seconds past J2000 TDB).
    fn state(&self, et: f64) -> [f64; 6] {
        // Locate the record covering `et`, clamped to the segment's coverage so
        // a query exactly at `end_et` still resolves to the last record.
        let mut index = ((et - self.init) / self.interval).floor() as i64;
        index = index.clamp(0, self.record_count as i64 - 1);
        let base = index as usize * self.record_size;
        let mid = self.data[base];
        let radius = self.data[base + 1];
        let ncoef = self.coeffs_per_component();
        let tau = ((et - mid) / radius).clamp(-1.0, 1.0);

        // Chebyshev polynomials T_k(tau) and their derivatives dT_k/dtau.
        let mut t = vec![0.0; ncoef];
        let mut dt = vec![0.0; ncoef];
        t[0] = 1.0;
        if ncoef > 1 {
            t[1] = tau;
            dt[1] = 1.0;
            for k in 2..ncoef {
                t[k] = 2.0 * tau * t[k - 1] - t[k - 2];
                dt[k] = 2.0 * t[k - 1] + 2.0 * tau * dt[k - 1] - dt[k - 2];
            }
        }

        let mut out = [0.0f64; 6];
        let coeff_base = base + 2;
        // dtau/dt = 1 / radius (seconds), used for the Type 2 velocity.
        let dtau_dt = 1.0 / radius;
        for comp in 0..self.components() {
            let c0 = coeff_base + comp * ncoef;
            let mut pos = 0.0;
            let mut der = 0.0;
            for k in 0..ncoef {
                let c = self.data[c0 + k];
                pos += c * t[k];
                der += c * dt[k];
            }
            if comp < 3 {
                out[comp] = pos;
                if !self.has_velocity {
                    // Type 2: velocity is the analytic Chebyshev derivative.
                    out[comp + 3] = der * dtau_dt;
                }
            } else {
                // Type 3: components 3..6 are the velocity series directly.
                out[comp] = pos;
            }
        }
        out
    }
}

/// A parsed DAF/SPK ephemeris kernel ready for state queries.
#[derive(Debug, Clone)]
pub struct SpkKernel {
    segments: Vec<Segment>,
}

impl SpkKernel {
    /// Parse a DAF/SPK kernel from its raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SpkError> {
        if bytes.len() < RECORD_BYTES {
            return Err(SpkError::Truncated);
        }
        let id = &bytes[0..8];
        if id != b"DAF/SPK " && id != b"NAIF/DAF" {
            return Err(SpkError::NotDafSpk);
        }
        // Endianness from LOCFMT (chars 89..96, zero-based 88..96).
        let endian = match &bytes[88..96] {
            b"LTL-IEEE" => Endian::Little,
            b"BIG-IEEE" => Endian::Big,
            _ => return Err(SpkError::UnknownEndianness),
        };
        let nd = endian.i32(&bytes[8..12]);
        let ni = endian.i32(&bytes[12..16]);
        if nd != 2 || ni != 6 {
            return Err(SpkError::BadSummaryLayout { nd, ni });
        }
        let fward = endian.i32(&bytes[76..80]);
        if fward <= 0 {
            return Err(SpkError::Truncated);
        }

        // Summary record layout: ND doubles + ceil(NI/2) doubles per summary.
        let summary_doubles = nd as usize + (ni as usize).div_ceil(2);
        let mut segments = Vec::new();
        let mut record_no = fward;
        let mut guard = 0usize;
        while record_no > 0 {
            guard += 1;
            if guard > 100_000 {
                return Err(SpkError::Truncated);
            }
            let rec = record_slice(bytes, record_no as usize)?;
            let next = endian.f64(&rec[0..8]) as i64;
            let nsum = endian.f64(&rec[16..24]) as usize;
            for s in 0..nsum {
                let off = 24 + s * summary_doubles * WORD_BYTES;
                let end = off + summary_doubles * WORD_BYTES;
                if end > rec.len() {
                    return Err(SpkError::Truncated);
                }
                let start_et = endian.f64(&rec[off..off + 8]);
                let end_et = endian.f64(&rec[off + 8..off + 16]);
                // The NI integers are packed 4 bytes each starting after the ND
                // doubles, in file endianness.
                let int_base = off + nd as usize * WORD_BYTES;
                let read_int = |k: usize| endian.i32(&rec[int_base + k * 4..int_base + k * 4 + 4]);
                let target = read_int(0);
                let center = read_int(1);
                let _frame = read_int(2);
                let seg_type = read_int(3);
                let baddr = read_int(4);
                let eaddr = read_int(5);
                if seg_type != 2 && seg_type != 3 {
                    return Err(SpkError::UnsupportedSegmentType(seg_type));
                }
                segments.push(parse_segment(
                    bytes,
                    endian,
                    target,
                    center,
                    start_et,
                    end_et,
                    seg_type == 3,
                    baddr as usize,
                    eaddr as usize,
                )?);
            }
            record_no = next as i32;
        }
        Ok(Self { segments })
    }

    /// Number of segments parsed from the kernel.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Geometric state `[x, y, z, vx, vy, vz]` (km, km/s, ICRF/J2000 frame) of
    /// `target` relative to `center` at the given TDB Julian Date.
    ///
    /// Both bodies are resolved to the solar-system barycenter through the
    /// kernel's center tree and differenced, so any pair connected to the
    /// barycenter (e.g. Mars barycenter relative to Earth) resolves even when
    /// no single segment links them directly.
    pub fn state_km(&self, target: i32, center: i32, jd_tdb: f64) -> Result<[f64; 6], SpkError> {
        let et = (jd_tdb - J2000_JD) * SECONDS_PER_DAY;
        let t = self.state_wrt_ssb(target, et)?;
        let c = self.state_wrt_ssb(center, et)?;
        Ok(std::array::from_fn(|i| t[i] - c[i]))
    }

    /// Geometric position `[x, y, z]` (km) of `target` relative to `center`.
    pub fn position_km(&self, target: i32, center: i32, jd_tdb: f64) -> Result<[f64; 3], SpkError> {
        let s = self.state_km(target, center, jd_tdb)?;
        Ok([s[0], s[1], s[2]])
    }

    /// Geocentric astrometric equatorial coordinates of `target`:
    /// `(right_ascension_rad, declination_rad, distance_km)` in the kernel's
    /// ICRF/J2000 frame (which the renderer treats as its J2000 equatorial
    /// frame). This is a geometric (not light-time/aberration corrected) place;
    /// the apparent-place corrections live in [`crate::corrections`].
    pub fn geocentric_equatorial(
        &self,
        target: i32,
        jd_tdb: f64,
    ) -> Result<(f64, f64, f64), SpkError> {
        let p = self.position_km(target, naif::EARTH, jd_tdb)?;
        let distance = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        let ra = p[1].atan2(p[0]).rem_euclid(std::f64::consts::TAU);
        let dec = (p[2] / distance.max(f64::MIN_POSITIVE))
            .clamp(-1.0, 1.0)
            .asin();
        Ok((ra, dec, distance))
    }

    /// State of `body` relative to the solar-system barycenter, walking the
    /// center chain (`body → its center → … → SSB`) and summing.
    fn state_wrt_ssb(&self, body: i32, et: f64) -> Result<[f64; 6], SpkError> {
        if body == naif::SOLAR_SYSTEM_BARYCENTER {
            return Ok([0.0; 6]);
        }
        let mut acc = [0.0f64; 6];
        let mut current = body;
        let mut visited: HashMap<i32, ()> = HashMap::new();
        loop {
            if visited.insert(current, ()).is_some() {
                return Err(SpkError::BrokenCenterChain(body));
            }
            let seg = self.segment_for(current, et).ok_or(SpkError::NoCoverage {
                body: current,
                et_seconds: et,
            })?;
            let s = seg.state(et);
            for i in 0..6 {
                acc[i] += s[i];
            }
            current = seg.center;
            if current == naif::SOLAR_SYSTEM_BARYCENTER {
                return Ok(acc);
            }
        }
    }

    fn segment_for(&self, body: i32, et: f64) -> Option<&Segment> {
        self.segments
            .iter()
            .find(|s| s.target == body && et >= s.start_et && et <= s.end_et)
    }
}

/// Byte slice for DAF physical record `record_no` (1-based).
fn record_slice(bytes: &[u8], record_no: usize) -> Result<&[u8], SpkError> {
    let start = (record_no - 1) * RECORD_BYTES;
    let end = start + RECORD_BYTES;
    bytes.get(start..end).ok_or(SpkError::Truncated)
}

/// Read a 1-based DAF word (double) from the data area.
fn read_word(bytes: &[u8], endian: Endian, word: usize) -> Result<f64, SpkError> {
    let start = (word - 1) * WORD_BYTES;
    let slice = bytes
        .get(start..start + WORD_BYTES)
        .ok_or(SpkError::Truncated)?;
    Ok(endian.f64(slice))
}

#[allow(clippy::too_many_arguments)]
fn parse_segment(
    bytes: &[u8],
    endian: Endian,
    target: i32,
    center: i32,
    start_et: f64,
    end_et: f64,
    has_velocity: bool,
    baddr: usize,
    eaddr: usize,
) -> Result<Segment, SpkError> {
    if eaddr < baddr || baddr == 0 {
        return Err(SpkError::Truncated);
    }
    // The final four words of every Type 2/3 array are the directory:
    // INIT, INTLEN, RSIZE, N.
    let init = read_word(bytes, endian, eaddr - 3)?;
    let interval = read_word(bytes, endian, eaddr - 2)?;
    let record_size = read_word(bytes, endian, eaddr - 1)? as usize;
    let record_count = read_word(bytes, endian, eaddr)? as usize;
    if record_size < 3 || record_count == 0 {
        return Err(SpkError::Truncated);
    }
    let total = record_size * record_count;
    let mut data = Vec::with_capacity(total);
    for w in 0..total {
        data.push(read_word(bytes, endian, baddr + w)?);
    }
    Ok(Segment {
        target,
        center,
        start_et,
        end_et,
        has_velocity,
        init,
        interval,
        record_size,
        record_count,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builder for a spec-accurate little-endian DAF/SPK byte image, used to
    /// pin the reader against the published layout. Records and segment arrays
    /// are laid out exactly as JPL writes them; segment Chebyshev coefficients
    /// are supplied by the test so the expected states are known in closed
    /// form.
    struct SpkBuilder {
        endian: Endian,
        /// (target, center, has_velocity, start_et, end_et, init, interval,
        ///  record_size, records-as-flat-doubles)
        segments: Vec<SegSpec>,
    }

    struct SegSpec {
        target: i32,
        center: i32,
        has_velocity: bool,
        init: f64,
        interval: f64,
        record_size: usize,
        records: Vec<f64>,
    }

    impl SpkBuilder {
        fn new(endian: Endian) -> Self {
            Self {
                endian,
                segments: Vec::new(),
            }
        }

        fn push(&mut self, spec: SegSpec) {
            self.segments.push(spec);
        }

        fn w_f64(&self, out: &mut Vec<u8>, v: f64) {
            match self.endian {
                Endian::Little => out.extend_from_slice(&v.to_le_bytes()),
                Endian::Big => out.extend_from_slice(&v.to_be_bytes()),
            }
        }

        fn w_i32(&self, out: &mut Vec<u8>, v: i32) {
            match self.endian {
                Endian::Little => out.extend_from_slice(&v.to_le_bytes()),
                Endian::Big => out.extend_from_slice(&v.to_be_bytes()),
            }
        }

        fn build(&self) -> Vec<u8> {
            // Layout: record 1 = file record; record 2 = summary record;
            // record 3 = name record; record 4.. = segment data, each segment
            // padded to a whole number of records.
            let n_meta_records = 3;
            // Compute segment data placement (1-based word addresses).
            let mut seg_addr = Vec::new(); // (baddr, eaddr)
            let mut data_records: Vec<Vec<f64>> = Vec::new();
            let mut next_record = n_meta_records + 1;
            for spec in &self.segments {
                let arr_len = spec.records.len() + 4; // + INIT, INTLEN, RSIZE, N
                let baddr_word = (next_record - 1) * (RECORD_BYTES / WORD_BYTES) + 1;
                let eaddr_word = baddr_word + arr_len - 1;
                seg_addr.push((baddr_word as i32, eaddr_word as i32));
                let mut rec_doubles: Vec<f64> = spec.records.clone();
                rec_doubles.push(spec.init);
                rec_doubles.push(spec.interval);
                rec_doubles.push(spec.record_size as f64);
                rec_doubles.push((spec.records.len() / spec.record_size) as f64);
                // Pad to whole records.
                let words_per_record = RECORD_BYTES / WORD_BYTES;
                let records_used = arr_len.div_ceil(words_per_record);
                rec_doubles.resize(records_used * words_per_record, 0.0);
                data_records.push(rec_doubles);
                next_record += records_used;
            }

            // --- File record (record 1) ---
            let mut file_rec = Vec::with_capacity(RECORD_BYTES);
            file_rec.extend_from_slice(b"DAF/SPK ");
            self.w_i32(&mut file_rec, 2); // ND
            self.w_i32(&mut file_rec, 6); // NI
            file_rec.extend_from_slice(&[0u8; 60]); // LOCIFN
            self.w_i32(&mut file_rec, 2); // FWARD -> summary record 2
            self.w_i32(&mut file_rec, 2); // BWARD
            self.w_i32(&mut file_rec, 0); // FREE (unused by reader)
            match self.endian {
                Endian::Little => file_rec.extend_from_slice(b"LTL-IEEE"),
                Endian::Big => file_rec.extend_from_slice(b"BIG-IEEE"),
            }
            file_rec.resize(RECORD_BYTES, 0);

            // --- Summary record (record 2) ---
            let mut sum_rec = Vec::with_capacity(RECORD_BYTES);
            self.w_f64(&mut sum_rec, 0.0); // NEXT
            self.w_f64(&mut sum_rec, 0.0); // PREV
            self.w_f64(&mut sum_rec, self.segments.len() as f64); // NSUM
            for (i, spec) in self.segments.iter().enumerate() {
                self.w_f64(&mut sum_rec, spec_start(spec)); // start ET
                self.w_f64(&mut sum_rec, spec_end(spec)); // end ET
                                                          // 6 integers packed: target, center, frame, type, baddr, eaddr.
                self.w_i32(&mut sum_rec, spec.target);
                self.w_i32(&mut sum_rec, spec.center);
                self.w_i32(&mut sum_rec, 1); // frame = J2000
                self.w_i32(&mut sum_rec, if spec.has_velocity { 3 } else { 2 });
                self.w_i32(&mut sum_rec, seg_addr[i].0);
                self.w_i32(&mut sum_rec, seg_addr[i].1);
            }
            sum_rec.resize(RECORD_BYTES, 0);

            // --- Name record (record 3) ---
            let name_rec = vec![0u8; RECORD_BYTES];

            let mut out = Vec::new();
            out.extend_from_slice(&file_rec);
            out.extend_from_slice(&sum_rec);
            out.extend_from_slice(&name_rec);
            for rec in &data_records {
                for &d in rec {
                    self.w_f64(&mut out, d);
                }
            }
            out
        }
    }

    fn spec_start(spec: &SegSpec) -> f64 {
        spec.init
    }
    fn spec_end(spec: &SegSpec) -> f64 {
        spec.init + spec.interval * (spec.records.len() / spec.record_size) as f64
    }

    /// One Type 2 record (degree-2 Chebyshev, 3 coeffs/component) whose X/Y/Z
    /// position polynomials are known in closed form so the test can predict
    /// the evaluated state.
    fn type2_record(mid: f64, radius: f64, coeffs: [[f64; 3]; 3]) -> Vec<f64> {
        let mut r = vec![mid, radius];
        for c in coeffs {
            r.extend_from_slice(&c);
        }
        r
    }

    /// Reference Chebyshev evaluation (T0..T2) for the test's expected values.
    fn cheb_pos(coeffs: &[f64; 3], tau: f64) -> f64 {
        let t0 = 1.0;
        let t1 = tau;
        let t2 = 2.0 * tau * tau - 1.0;
        coeffs[0] * t0 + coeffs[1] * t1 + coeffs[2] * t2
    }

    fn single_type2_kernel(endian: Endian) -> (Vec<u8>, [[f64; 3]; 3], f64, f64) {
        // Segment: Mars (499) wrt SSB (0), one record covering ET [0, 100].
        let mid = 50.0;
        let radius = 50.0;
        let coeffs = [[100.0, 10.0, 1.0], [-200.0, 5.0, -2.0], [30.0, -3.0, 0.5]];
        let mut b = SpkBuilder::new(endian);
        b.push(SegSpec {
            target: naif::MARS,
            center: naif::SOLAR_SYSTEM_BARYCENTER,
            has_velocity: false,
            init: 0.0,
            interval: 100.0,
            record_size: 2 + 3 * 3,
            records: type2_record(mid, radius, coeffs),
        });
        (b.build(), coeffs, mid, radius)
    }

    #[test]
    fn parses_and_evaluates_type2_position() {
        let (bytes, coeffs, mid, radius) = single_type2_kernel(Endian::Little);
        let k = SpkKernel::from_bytes(&bytes).expect("parse");
        assert_eq!(k.segment_count(), 1);

        // Query at ET = 75 s past J2000 → JD.
        let et = 75.0;
        let jd = J2000_JD + et / SECONDS_PER_DAY;
        let s = k
            .state_km(naif::MARS, naif::SOLAR_SYSTEM_BARYCENTER, jd)
            .unwrap();
        let tau = (et - mid) / radius;
        // Tolerance is ~1e-4, not machine epsilon: recovering ET seconds from an
        // absolute J2000 Julian Date loses ~1e-4 s to f64 cancellation
        // (2451545.000868 − 2451545.0), which is astronomically negligible but
        // larger than 1e-9. A structural parser/eval bug would miss by whole km.
        for axis in 0..3 {
            let expected = cheb_pos(&coeffs[axis], tau);
            assert!(
                (s[axis] - expected).abs() < 1e-4,
                "axis {axis}: got {}, want {expected}",
                s[axis]
            );
        }
    }

    #[test]
    fn type2_velocity_matches_finite_difference() {
        let (bytes, _coeffs, _mid, _radius) = single_type2_kernel(Endian::Little);
        let k = SpkKernel::from_bytes(&bytes).expect("parse");
        let et = 60.0;
        let jd = J2000_JD + et / SECONDS_PER_DAY;
        let s = k
            .state_km(naif::MARS, naif::SOLAR_SYSTEM_BARYCENTER, jd)
            .unwrap();
        // Central finite difference of position (km) over ±1 s.
        let dt = 1.0;
        let jp = J2000_JD + (et + dt) / SECONDS_PER_DAY;
        let jm = J2000_JD + (et - dt) / SECONDS_PER_DAY;
        let pp = k
            .position_km(naif::MARS, naif::SOLAR_SYSTEM_BARYCENTER, jp)
            .unwrap();
        let pm = k
            .position_km(naif::MARS, naif::SOLAR_SYSTEM_BARYCENTER, jm)
            .unwrap();
        for axis in 0..3 {
            let fd = (pp[axis] - pm[axis]) / (2.0 * dt);
            assert!(
                (s[axis + 3] - fd).abs() < 1e-4,
                "axis {axis}: analytic {} vs finite-diff {fd}",
                s[axis + 3]
            );
        }
    }

    #[test]
    fn big_endian_kernel_parses_identically() {
        let (le, coeffs, mid, radius) = single_type2_kernel(Endian::Little);
        let (be, _, _, _) = single_type2_kernel(Endian::Big);
        let kl = SpkKernel::from_bytes(&le).unwrap();
        let kb = SpkKernel::from_bytes(&be).unwrap();
        let et = 42.0;
        let jd = J2000_JD + et / SECONDS_PER_DAY;
        let sl = kl.state_km(naif::MARS, 0, jd).unwrap();
        let sb = kb.state_km(naif::MARS, 0, jd).unwrap();
        let tau = (et - mid) / radius;
        for axis in 0..3 {
            let expected = cheb_pos(&coeffs[axis], tau);
            assert!((sl[axis] - expected).abs() < 1e-4);
            assert!((sb[axis] - expected).abs() < 1e-4);
            // Little- and big-endian images must decode bit-identically.
            assert!((sl[axis] - sb[axis]).abs() < 1e-12);
        }
    }

    #[test]
    fn center_chaining_resolves_geocentric_state() {
        // Two segments: Mars (499) wrt SSB, Earth (399) wrt SSB. Geocentric
        // Mars = pos(499) − pos(399).
        let mars = [[100.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]];
        let earth = [[40.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]];
        let mut b = SpkBuilder::new(Endian::Little);
        b.push(SegSpec {
            target: naif::MARS,
            center: 0,
            has_velocity: false,
            init: 0.0,
            interval: 100.0,
            record_size: 11,
            records: type2_record(50.0, 50.0, mars),
        });
        b.push(SegSpec {
            target: naif::EARTH,
            center: 0,
            has_velocity: false,
            init: 0.0,
            interval: 100.0,
            record_size: 11,
            records: type2_record(50.0, 50.0, earth),
        });
        let k = SpkKernel::from_bytes(&b.build()).unwrap();
        let jd = J2000_JD + 50.0 / SECONDS_PER_DAY; // tau = 0 → only T0 term
        let geo = k.position_km(naif::MARS, naif::EARTH, jd).unwrap();
        assert!((geo[0] - 60.0).abs() < 1e-9, "got {}", geo[0]); // 100 − 40
                                                                 // Geocentric RA on the +x axis is 0; distance 60 km.
        let (ra, dec, dist) = k.geocentric_equatorial(naif::MARS, jd).unwrap();
        assert!((dist - 60.0).abs() < 1e-9);
        assert!(ra.abs() < 1e-9 || (ra - std::f64::consts::TAU).abs() < 1e-9);
        assert!(dec.abs() < 1e-9);
    }

    #[test]
    fn indirect_center_chain_through_barycenter() {
        // Moon (301) wrt EMB (3); EMB (3) wrt SSB (0). state_wrt_ssb(301) must
        // sum both links.
        let moon = [[5.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]];
        let emb = [[1000.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]];
        let mut b = SpkBuilder::new(Endian::Little);
        b.push(SegSpec {
            target: naif::MOON,
            center: naif::EARTH_MOON_BARYCENTER,
            has_velocity: false,
            init: 0.0,
            interval: 100.0,
            record_size: 11,
            records: type2_record(50.0, 50.0, moon),
        });
        b.push(SegSpec {
            target: naif::EARTH_MOON_BARYCENTER,
            center: 0,
            has_velocity: false,
            init: 0.0,
            interval: 100.0,
            record_size: 11,
            records: type2_record(50.0, 50.0, emb),
        });
        let k = SpkKernel::from_bytes(&b.build()).unwrap();
        let jd = J2000_JD + 50.0 / SECONDS_PER_DAY;
        let p = k.position_km(naif::MOON, 0, jd).unwrap();
        assert!((p[0] - 1005.0).abs() < 1e-9, "got {}", p[0]);
    }

    #[test]
    fn type3_uses_explicit_velocity_series() {
        // Type 3 record: 6 components × 2 coeffs each (degree 1).
        // Position X = 10 + 3·tau ; velocity VX = 7 (constant series).
        let record_size = 2 + 6 * 2;
        let mut rec = vec![50.0, 50.0]; // mid, radius
        let comp_coeffs: [[f64; 2]; 6] = [
            [10.0, 3.0], // X
            [0.0, 0.0],  // Y
            [0.0, 0.0],  // Z
            [7.0, 0.0],  // VX
            [0.0, 0.0],  // VY
            [0.0, 0.0],  // VZ
        ];
        for c in comp_coeffs {
            rec.extend_from_slice(&c);
        }
        let mut b = SpkBuilder::new(Endian::Little);
        b.push(SegSpec {
            target: naif::MARS,
            center: 0,
            has_velocity: true,
            init: 0.0,
            interval: 100.0,
            record_size,
            records: rec,
        });
        let k = SpkKernel::from_bytes(&b.build()).unwrap();
        let et = 50.0; // tau = 0
        let jd = J2000_JD + et / SECONDS_PER_DAY;
        let s = k.state_km(naif::MARS, 0, jd).unwrap();
        assert!((s[0] - 10.0).abs() < 1e-4); // X position at tau=0
        assert!((s[3] - 7.0).abs() < 1e-9); // VX from explicit series (tau-independent)
    }

    #[test]
    fn rejects_non_daf_and_truncated_blobs() {
        assert_eq!(
            SpkKernel::from_bytes(b"short").unwrap_err(),
            SpkError::Truncated
        );
        let mut not_spk = vec![0u8; RECORD_BYTES];
        not_spk[0..8].copy_from_slice(b"NOTADAF!");
        assert_eq!(
            SpkKernel::from_bytes(&not_spk).unwrap_err(),
            SpkError::NotDafSpk
        );
    }

    #[test]
    fn reports_missing_coverage() {
        let (bytes, _, _, _) = single_type2_kernel(Endian::Little);
        let k = SpkKernel::from_bytes(&bytes).unwrap();
        // ET 500 s is past the single record's [0,100] coverage.
        let jd = J2000_JD + 500.0 / SECONDS_PER_DAY;
        match k.state_km(naif::MARS, 0, jd) {
            Err(SpkError::NoCoverage { body, .. }) => assert_eq!(body, naif::MARS),
            other => panic!("expected NoCoverage, got {other:?}"),
        }
    }
}
