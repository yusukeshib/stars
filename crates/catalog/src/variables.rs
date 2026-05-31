//! Variable-star light curves (L-20).
//!
//! A small, hand-curated table of well-known variable stars carrying the
//! standardised light-curve *elements* — type, period `P`, epoch `T0`, and the
//! bright/faint V magnitudes — from the AAVSO International Variable Star Index
//! (VSX, Watson 2006) and the GCVS (Samus et al. 2017). From those elements the
//! predicted magnitude at any session time is recovered by **phase folding**,
//! so a host can show "what does this variable look like tonight" and plot a
//! one-period light curve.
//!
//! The model is deliberately a first-order *visual* one (see
//! [`VariableStar::predicted_magnitude`]): a smoothed pulsation curve for
//! Mira / semiregular / Cepheid / RR Lyrae stars and a raised-cosine eclipse
//! for Algol-type (EA) eclipsing binaries. It is not a Fourier light-curve fit;
//! the limitations are recorded in `data/manifest.toml` and `VALIDATION.md`.
//!
//! ## Matching
//!
//! [`variable_for`] joins a catalogue star to its elements by Hipparcos (HIP)
//! or Henry Draper (HD) number, falling back to a case-insensitive proper-name
//! match. The table is independent of the per-star [`crate::Star`] struct (no
//! hot-path widening); hosts look a star up only when its info panel is opened.
//!
//! ## References
//! - Watson, C. L. 2006, SASS 25, 47 ("The International Variable Star Index
//!   (VSX)").
//! - Samus', N. N. et al. 2017, ARep 61, 80 (GCVS 5.1).

use std::sync::OnceLock;

/// Light-curve class for a variable star. Pulsators share one smoothed model;
/// `Algol` uses the eclipsing-binary model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableType {
    /// Long-period (Mira) pulsating giant.
    Mira,
    /// Semiregular pulsating variable (ill-defined period).
    SemiRegular,
    /// Classical Cepheid pulsator.
    Cepheid,
    /// RR Lyrae short-period pulsator.
    RrLyrae,
    /// Algol-type (EA) eclipsing binary.
    Algol,
}

impl VariableType {
    /// Stable lower-case identifier used in the data file and host output.
    pub fn as_str(self) -> &'static str {
        match self {
            VariableType::Mira => "mira",
            VariableType::SemiRegular => "semiregular",
            VariableType::Cepheid => "cepheid",
            VariableType::RrLyrae => "rrlyrae",
            VariableType::Algol => "algol",
        }
    }

    /// True when this class uses the eclipsing-binary light-curve model.
    pub fn is_eclipsing(self) -> bool {
        matches!(self, VariableType::Algol)
    }

    fn parse(s: &str) -> Option<VariableType> {
        Some(match s {
            "mira" => VariableType::Mira,
            "semiregular" => VariableType::SemiRegular,
            "cepheid" => VariableType::Cepheid,
            "rrlyrae" => VariableType::RrLyrae,
            "algol" => VariableType::Algol,
            _ => return None,
        })
    }
}

/// Light-curve elements for one variable star.
#[derive(Debug, Clone)]
pub struct VariableStar {
    /// Informational label (e.g. `"Algol (beta Persei)"`).
    pub name: String,
    /// Hipparcos number for matching, `None` when unknown.
    pub hip: Option<u32>,
    /// Henry Draper number for matching, `None` when unknown.
    pub hd: Option<u32>,
    /// Light-curve class.
    pub var_type: VariableType,
    /// Period `P` in days.
    pub period_days: f64,
    /// Epoch `T0` (JD): maximum light for pulsators, primary minimum for
    /// eclipsing binaries.
    pub epoch_jd: f64,
    /// Brightest (maximum-light) V magnitude.
    pub mag_bright: f64,
    /// Faintest (minimum-light) V magnitude.
    pub mag_faint: f64,
    /// Eclipsing only: secondary-eclipse depth below `mag_bright` (mag).
    pub secondary_depth_mag: f64,
    /// Eclipsing only: half-width of an eclipse in phase units `[0, 0.5]`.
    pub eclipse_half_width: f64,
    /// Literature reference key (e.g. `"VSX"`, `"GCVS"`).
    pub reference: String,
}

impl VariableStar {
    /// Phase `∈ [0, 1)` of the star at Julian Date `jd`: the fractional number
    /// of periods elapsed since the epoch `T0`. Phase 0 is maximum light for a
    /// pulsator and primary eclipse minimum for an eclipsing binary.
    pub fn phase(&self, jd: f64) -> f64 {
        if self.period_days <= 0.0 {
            return 0.0;
        }
        ((jd - self.epoch_jd) / self.period_days).rem_euclid(1.0)
    }

    /// Predicted V magnitude at Julian Date `jd` from the light-curve elements.
    ///
    /// * Pulsators (Mira / semiregular / Cepheid / RR Lyrae): a smoothed
    ///   raised-cosine between maximum (`mag_bright` at phase 0) and minimum
    ///   (`mag_faint` at phase 0.5).
    /// * Eclipsing (Algol / EA): flat at `mag_bright` out of eclipse, dipping
    ///   to `mag_faint` through a raised-cosine primary eclipse centred on
    ///   phase 0 and a shallow `secondary_depth_mag` eclipse centred on phase
    ///   0.5, each of half-width `eclipse_half_width`.
    pub fn predicted_magnitude(&self, jd: f64) -> f64 {
        let phase = self.phase(jd);
        if self.var_type.is_eclipsing() {
            let half = self.eclipse_half_width.clamp(1.0e-4, 0.5);
            // Distance (in phase) to the primary eclipse at 0 (or 1) and the
            // secondary eclipse at 0.5.
            let d_primary = phase.min(1.0 - phase);
            let d_secondary = (phase - 0.5).abs();
            let primary = if d_primary < half {
                (self.mag_faint - self.mag_bright)
                    * 0.5
                    * (1.0 + (std::f64::consts::PI * d_primary / half).cos())
            } else {
                0.0
            };
            let secondary = if d_secondary < half {
                self.secondary_depth_mag
                    * 0.5
                    * (1.0 + (std::f64::consts::PI * d_secondary / half).cos())
            } else {
                0.0
            };
            self.mag_bright + primary + secondary
        } else {
            // Smoothed pulsation: bright at phase 0, faint at phase 0.5.
            let amp = self.mag_faint - self.mag_bright;
            self.mag_bright + amp * 0.5 * (1.0 - (std::f64::consts::TAU * phase).cos())
        }
    }

    /// Predicted magnitude *fainter* than maximum light, in magnitudes
    /// (`predicted − mag_bright`, always `≥ 0`).
    pub fn delta_magnitude(&self, jd: f64) -> f64 {
        (self.predicted_magnitude(jd) - self.mag_bright).max(0.0)
    }

    /// `n` evenly phase-spaced `(phase, magnitude)` samples over one period,
    /// for plotting a light curve. `n` is clamped to `>= 2`.
    pub fn light_curve_samples(&self, n: usize) -> Vec<(f64, f64)> {
        let n = n.max(2);
        (0..n)
            .map(|i| {
                let phase = i as f64 / n as f64;
                let jd = self.epoch_jd + phase * self.period_days;
                (phase, self.predicted_magnitude(jd))
            })
            .collect()
    }

    /// A self-contained snapshot of this variable's state at `jd`, for host
    /// info panels and metadata output.
    pub fn summary_at(&self, jd: f64) -> VariableSummary {
        VariableSummary {
            name: self.name.clone(),
            kind: self.var_type.as_str(),
            period_days: self.period_days,
            epoch_jd: self.epoch_jd,
            mag_bright: self.mag_bright,
            mag_faint: self.mag_faint,
            phase: self.phase(jd),
            current_magnitude: self.predicted_magnitude(jd),
            delta_magnitude: self.delta_magnitude(jd),
            reference: self.reference.clone(),
        }
    }
}

/// A point-in-time snapshot of a variable star's predicted state.
#[derive(Debug, Clone, PartialEq)]
pub struct VariableSummary {
    pub name: String,
    /// Light-curve class identifier (see [`VariableType::as_str`]).
    pub kind: &'static str,
    pub period_days: f64,
    pub epoch_jd: f64,
    pub mag_bright: f64,
    pub mag_faint: f64,
    /// Current phase `∈ [0, 1)` at the queried time.
    pub phase: f64,
    /// Predicted V magnitude at the queried time.
    pub current_magnitude: f64,
    /// Predicted magnitude fainter than maximum light (`≥ 0`).
    pub delta_magnitude: f64,
    pub reference: String,
}

const VARIABLE_STARS_CSV: &str = include_str!("../data/variable_stars.csv");

fn parsed() -> &'static [VariableStar] {
    static TABLE: OnceLock<Vec<VariableStar>> = OnceLock::new();
    TABLE.get_or_init(parse_variable_stars_csv)
}

fn parse_variable_stars_csv() -> Vec<VariableStar> {
    let mut out = Vec::new();
    let mut header_seen = false;
    for (line_number, raw_line) in VARIABLE_STARS_CSV.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if !header_seen {
            assert_eq!(
                trimmed,
                "name,hip,hd,var_type,period_days,epoch_jd,mag_bright,mag_faint,\
                 secondary_depth_mag,eclipse_half_width,reference",
                "variable_stars.csv header mismatch at line {}",
                line_number + 1
            );
            header_seen = true;
            continue;
        }
        let fields: Vec<&str> = trimmed.split(',').map(str::trim).collect();
        assert_eq!(
            fields.len(),
            11,
            "variable_stars.csv expects 11 columns at line {}",
            line_number + 1
        );
        let parse_f64 = |idx: usize, what: &str| -> f64 {
            fields[idx].parse::<f64>().unwrap_or_else(|_| {
                panic!(
                    "variable_stars.csv: invalid {what} {:?} at line {}",
                    fields[idx],
                    line_number + 1
                )
            })
        };
        let parse_id = |idx: usize| -> Option<u32> {
            match fields[idx].parse::<u32>() {
                Ok(0) | Err(_) => None,
                Ok(v) => Some(v),
            }
        };
        let var_type = VariableType::parse(fields[3]).unwrap_or_else(|| {
            panic!(
                "variable_stars.csv: unknown var_type {:?} at line {}",
                fields[3],
                line_number + 1
            )
        });
        out.push(VariableStar {
            name: fields[0].to_string(),
            hip: parse_id(1),
            hd: parse_id(2),
            var_type,
            period_days: parse_f64(4, "period_days"),
            epoch_jd: parse_f64(5, "epoch_jd"),
            mag_bright: parse_f64(6, "mag_bright"),
            mag_faint: parse_f64(7, "mag_faint"),
            secondary_depth_mag: parse_f64(8, "secondary_depth_mag"),
            eclipse_half_width: parse_f64(9, "eclipse_half_width"),
            reference: fields[10].to_string(),
        });
    }
    out
}

/// All variable stars in the showpiece table.
pub fn variable_stars() -> &'static [VariableStar] {
    parsed()
}

/// `L-20` renderer brightness override: the apparent V magnitude to *render*
/// for a catalogue star at session time `jd` (Julian Date).
///
/// When the star matches a known variable (by HIP / HD / proper name) this is
/// the variable's phase-folded [`VariableStar::predicted_magnitude`], so a
/// rendered Mira / Algol sprite dims and brightens with the session epoch.
/// Non-variable stars (and the absence of a session time at the call site)
/// keep their static catalogue `base_magnitude`, preserving catalogue purity
/// for the rest of the sky. This is the single source of truth the native and
/// WASM instance builders share so they cannot drift.
pub fn render_magnitude_at(
    hip: Option<u32>,
    hd: Option<u32>,
    proper_name: Option<&str>,
    base_magnitude: f32,
    jd: f64,
) -> f32 {
    match variable_for(hip, hd, proper_name) {
        Some(v) => v.predicted_magnitude(jd) as f32,
        None => base_magnitude,
    }
}

/// Find the variable-star elements for a catalogue star, matching by HIP, then
/// HD, then case-insensitive proper name. Returns `None` for non-variable
/// stars.
pub fn variable_for(
    hip: Option<u32>,
    hd: Option<u32>,
    proper_name: Option<&str>,
) -> Option<&'static VariableStar> {
    let table = parsed();
    if let Some(hip) = hip {
        if let Some(v) = table.iter().find(|v| v.hip == Some(hip)) {
            return Some(v);
        }
    }
    if let Some(hd) = hd {
        if let Some(v) = table.iter().find(|v| v.hd == Some(hd)) {
            return Some(v);
        }
    }
    if let Some(name) = proper_name {
        let needle = name.trim().to_ascii_lowercase();
        if !needle.is_empty() {
            if let Some(v) = table.iter().find(|v| {
                // Match the leading token of the label (before a parenthesis)
                // or the full label, case-insensitively.
                let label = v.name.to_ascii_lowercase();
                label == needle || label.starts_with(&format!("{needle} ("))
            }) {
                return Some(v);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn algol() -> &'static VariableStar {
        variable_for(Some(14576), None, None).expect("Algol present")
    }

    #[test]
    fn table_loads_expected_rows() {
        let table = variable_stars();
        assert_eq!(table.len(), 6, "L-20 showpiece ships six variables");
        assert!(variable_for(Some(14576), None, None).is_some(), "Algol");
        assert!(variable_for(Some(10826), None, None).is_some(), "Mira");
        assert!(
            variable_for(None, Some(39801), None).is_some(),
            "Betelgeuse by HD"
        );
        assert!(
            variable_for(None, None, Some("Mira")).is_some(),
            "Mira by name"
        );
        assert!(
            variable_for(Some(99999), None, None).is_none(),
            "non-variable"
        );
    }

    #[test]
    fn algol_primary_minimum_returns_documented_depth() {
        // ROADMAP L-20 acceptance test: at the documented primary-minimum epoch
        // Algol is at its faintest (V = 3.39), so delta-m is the full eclipse
        // depth 3.39 - 2.12 = 1.27 mag.
        let v = algol();
        let m_min = v.predicted_magnitude(v.epoch_jd);
        assert!((m_min - 3.39).abs() < 1.0e-3, "Algol min V = {m_min}");
        assert!(
            (v.delta_magnitude(v.epoch_jd) - 1.27).abs() < 1.0e-3,
            "Algol primary-minimum delta-m"
        );
        // One full period later it is back at minimum (periodicity).
        let m_next = v.predicted_magnitude(v.epoch_jd + v.period_days);
        assert!((m_next - m_min).abs() < 1.0e-6, "periodic");
    }

    /// L-20 renderer override (`render_magnitude_at`): at Algol's primary
    /// minimum a star matched by HIP renders at the faint magnitude (3.39),
    /// fully one magnitude dimmer than the catalogue maximum; an unmatched
    /// star keeps its catalogue magnitude unchanged.
    #[test]
    fn render_magnitude_override_dims_variable_at_minimum() {
        let v = algol();
        // Matched by HIP at primary minimum -> faint magnitude, not the
        // catalogue base (2.12) we pass as the would-be static value.
        let rendered = render_magnitude_at(Some(14576), None, None, 2.12, v.epoch_jd);
        assert!(
            (rendered - 3.39).abs() < 1.0e-3,
            "Algol should render at minimum V=3.39, got {rendered}"
        );
        // A quarter period later it is back at maximum.
        let max = render_magnitude_at(
            Some(14576),
            None,
            None,
            2.12,
            v.epoch_jd + 0.25 * v.period_days,
        );
        assert!(
            (max - v.mag_bright as f32).abs() < 1.0e-4,
            "Algol max {max}"
        );
        // A non-variable star (Sirius HIP 32349) passes its catalogue
        // magnitude through untouched -> catalogue purity preserved.
        let unchanged = render_magnitude_at(Some(32349), Some(48915), None, -1.46, v.epoch_jd);
        assert!(
            (unchanged - (-1.46)).abs() < 1.0e-9,
            "non-variable unchanged"
        );
    }

    #[test]
    fn algol_out_of_eclipse_is_at_maximum() {
        // Quarter phase is well clear of both eclipses -> full brightness.
        let v = algol();
        let jd = v.epoch_jd + 0.25 * v.period_days;
        assert!(
            (v.predicted_magnitude(jd) - v.mag_bright).abs() < 1.0e-9,
            "Algol out of eclipse"
        );
        assert!(v.delta_magnitude(jd) < 1.0e-9);
    }

    #[test]
    fn algol_secondary_eclipse_is_shallow() {
        // Phase 0.5 is the secondary eclipse: a small dip, far less than the
        // primary depth.
        let v = algol();
        let d = v.delta_magnitude(v.epoch_jd + 0.5 * v.period_days);
        assert!(d > 0.0 && d < 0.1, "secondary depth {d}");
    }

    #[test]
    fn mira_phase_folds_between_extremes() {
        // Mira: maximum light at the epoch, minimum half a period later.
        let v = variable_for(Some(10826), None, None).expect("Mira");
        assert!((v.predicted_magnitude(v.epoch_jd) - v.mag_bright).abs() < 1.0e-9);
        let m_mid = v.predicted_magnitude(v.epoch_jd + 0.5 * v.period_days);
        assert!((m_mid - v.mag_faint).abs() < 1.0e-9, "Mira minimum {m_mid}");
        // A fixed session time a quarter-period after maximum is mid-range.
        let m_q = v.predicted_magnitude(v.epoch_jd + 0.25 * v.period_days);
        let mid = 0.5 * (v.mag_bright + v.mag_faint);
        assert!((m_q - mid).abs() < 1.0e-6, "Mira quarter-phase {m_q}");
    }

    #[test]
    fn predicted_magnitude_stays_within_range() {
        for v in variable_stars() {
            for i in 0..200 {
                let jd = v.epoch_jd + (i as f64 / 200.0) * v.period_days;
                let m = v.predicted_magnitude(jd);
                assert!(
                    m >= v.mag_bright - 1.0e-9 && m <= v.mag_faint + 1.0e-9,
                    "{} out of range at i={i}: {m}",
                    v.name
                );
            }
        }
    }

    #[test]
    fn light_curve_samples_cover_one_period() {
        let v = algol();
        let samples = v.light_curve_samples(64);
        assert_eq!(samples.len(), 64);
        assert!((samples[0].0 - 0.0).abs() < 1.0e-9);
        // The faintest sample must reach close to the eclipse depth.
        let faintest = samples.iter().map(|(_, m)| *m).fold(f64::MIN, f64::max);
        assert!(faintest > v.mag_bright + 1.0, "eclipse sampled: {faintest}");
    }

    #[test]
    fn summary_is_self_consistent() {
        let v = algol();
        let s = v.summary_at(v.epoch_jd);
        assert_eq!(s.kind, "algol");
        assert!((s.current_magnitude - 3.39).abs() < 1.0e-3);
        assert!((s.delta_magnitude - 1.27).abs() < 1.0e-3);
        assert!((s.phase - 0.0).abs() < 1.0e-9);
    }
}
