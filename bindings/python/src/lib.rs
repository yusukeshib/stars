//! PyO3 bindings for the `stars` astronomy + catalog read-only API (L-21).
//!
//! The goal is reproducibility-by-binding: a notebook reviewer can call the
//! exact same `astronomy::apparent_*` functions the renderer consumes, with
//! identical numerics. The binding is intentionally read-only and side-
//! effect-free — no rendering, no file I/O beyond loading the embedded star
//! catalog. Build for Python via [`maturin`](https://www.maturin.rs/):
//!
//! ```text
//! cd bindings/python && maturin develop --features extension-module
//! python tests/smoke.py
//! ```
//!
//! Plain `cargo check -p stars-py` (run from `make ci` via `make pyo3-check`)
//! validates the binding without a Python toolchain — `extension-module` is
//! opt-in, so the `pyo3` crate links against the host's stub libpython only
//! when explicitly requested for a wheel build.

// `#[pymethods]` expands `PyResult<_>`-returning methods through an internal
// `Into<PyResult<_>>` step that clippy reads as an identity conversion on
// `PyErr`. The expansion is opaque to local `#[allow]`s, so silence the lint
// for the whole binding crate — every conversion that actually appears in
// the source is a deliberate bridge to a Python exception type.
#![allow(clippy::useless_conversion)]

use astronomy::{
    apparent_galilean_moons_topocentric, apparent_planets_topocentric, apparent_titan_topocentric,
    body_altitude_rad as astro_body_altitude_rad, equatorial_to_horizontal,
    evening_plan as astro_evening_plan, jd_utc_to_unix_ms as astro_jd_utc_to_unix_ms,
    julian_date_from_unix_seconds, rise_transit_set as astro_rise_transit_set,
    twilight_band as astro_twilight_band, twilight_indicators as astro_twilight_indicators, AltAz,
    EveningPlan, GalileanMoon, GalileanMoonApparent, MoonApparent, Observer, Planet,
    PlanetApparent, PlanningBody, RiseTransitSet, SunApparent, SunMoonApparent, TimeScales,
    TitanApparent, TwilightIndicator,
};
use catalog::{load_embedded, Star};
use pyo3::exceptions::{PyIOError, PyIndexError, PyValueError};
use pyo3::prelude::*;
use serde_json::Value;

/// Embedded current-schema session template (the committed `dark-sky` preset),
/// used as the base document for [`PySession`] constructors so a
/// Python-built session is always a valid, current `SESSION_SCHEMA_VERSION`
/// document without the binding re-declaring the full schema. The preset
/// files are regenerated whenever the schema bumps, so this tracks the real
/// `crates/common` session layout automatically.
const DEFAULT_SESSION_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/presets/sessions/dark-sky.json"
));

// ---------------------------------------------------------------------------
// Observer
// ---------------------------------------------------------------------------

/// Geographic observer state pinned to a UTC Julian Date.
///
/// Mirrors [`astronomy::Observer`] — the same struct the renderer consumes,
/// so any reading produced here is numerically identical to what the CLI
/// session would render at the same epoch.
#[pyclass(name = "Observer", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PyObserver {
    inner: Observer,
}

#[pymethods]
impl PyObserver {
    /// Build an [`Observer`] from latitude / longitude in degrees and a UTC
    /// Julian Date. Latitude is clamped to ±90°, longitude is wrapped onto
    /// one turn, and non-finite inputs are replaced with 0 — matching the
    /// renderer's input sanitisation.
    #[new]
    fn new(latitude_deg: f64, longitude_deg: f64, jd_utc: f64) -> Self {
        Self {
            inner: Observer::from_degrees(latitude_deg, longitude_deg, jd_utc),
        }
    }

    /// Convenience constructor that accepts a POSIX timestamp (seconds since
    /// 1970-01-01T00:00:00Z) and converts it to a UTC Julian Date via the
    /// same helper the renderer uses.
    #[staticmethod]
    fn from_unix_seconds(latitude_deg: f64, longitude_deg: f64, unix_seconds: f64) -> Self {
        let jd_utc = julian_date_from_unix_seconds(unix_seconds);
        Self::new(latitude_deg, longitude_deg, jd_utc)
    }

    #[getter]
    fn latitude_deg(&self) -> f64 {
        self.inner.latitude_rad.to_degrees()
    }

    #[getter]
    fn longitude_deg(&self) -> f64 {
        self.inner.longitude_rad.to_degrees()
    }

    #[getter]
    fn jd_utc(&self) -> f64 {
        self.inner.time.jd_utc
    }

    #[getter]
    fn jd_ut1(&self) -> f64 {
        self.inner.time.jd_ut1
    }

    #[getter]
    fn jd_tt(&self) -> f64 {
        self.inner.time.jd_tt
    }

    #[getter]
    fn jd_tdb(&self) -> f64 {
        self.inner.time.jd_tdb
    }

    fn __repr__(&self) -> String {
        format!(
            "Observer(lat={:.6}°, lon={:.6}°, jd_utc={:.6})",
            self.latitude_deg(),
            self.longitude_deg(),
            self.jd_utc()
        )
    }
}

impl PyObserver {
    #[cfg(test)]
    pub(crate) fn inner(&self) -> Observer {
        self.inner
    }
}

// ---------------------------------------------------------------------------
// Apparent-body classes (mirrors astronomy::ephemeris / astronomy::moons)
// ---------------------------------------------------------------------------

/// Apparent equatorial position of the Sun for a given observer.
#[pyclass(name = "ApparentSun", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PyApparentSun {
    inner: SunApparent,
}

#[pymethods]
impl PyApparentSun {
    #[getter]
    fn right_ascension_rad(&self) -> f64 {
        self.inner.right_ascension_rad
    }

    #[getter]
    fn declination_rad(&self) -> f64 {
        self.inner.declination_rad
    }

    /// Horizontal (altitude, azimuth) projection for the given observer's
    /// local sidereal time and latitude. Both are in radians.
    fn altaz(&self, observer: &PyObserver) -> (f64, f64) {
        let lst = astronomy::lmst_radians(observer.inner.time.jd_ut1, observer.inner.longitude_rad);
        let aa = equatorial_to_horizontal(
            self.inner.right_ascension_rad,
            self.inner.declination_rad,
            lst,
            observer.inner.latitude_rad,
        );
        altaz_to_tuple(aa)
    }
}

/// Apparent equatorial position of the Moon for a given observer.
#[pyclass(name = "ApparentMoon", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PyApparentMoon {
    inner: MoonApparent,
}

#[pymethods]
impl PyApparentMoon {
    #[getter]
    fn right_ascension_rad(&self) -> f64 {
        self.inner.right_ascension_rad
    }

    #[getter]
    fn declination_rad(&self) -> f64 {
        self.inner.declination_rad
    }

    fn altaz(&self, observer: &PyObserver) -> (f64, f64) {
        let lst = astronomy::lmst_radians(observer.inner.time.jd_ut1, observer.inner.longitude_rad);
        let aa = equatorial_to_horizontal(
            self.inner.right_ascension_rad,
            self.inner.declination_rad,
            lst,
            observer.inner.latitude_rad,
        );
        altaz_to_tuple(aa)
    }
}

/// Sun + Moon paired apparent state — the same `SunMoonApparent` the
/// renderer reads to drive its illuminant uniform.
#[pyclass(name = "SunMoon", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PySunMoon {
    inner: SunMoonApparent,
}

#[pymethods]
impl PySunMoon {
    #[getter]
    fn sun(&self) -> PyApparentSun {
        PyApparentSun {
            inner: self.inner.sun,
        }
    }

    #[getter]
    fn moon(&self) -> PyApparentMoon {
        PyApparentMoon {
            inner: self.inner.moon,
        }
    }
}

/// Apparent equatorial position of a major planet.
#[pyclass(name = "ApparentPlanet", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PyApparentPlanet {
    inner: PlanetApparent,
}

#[pymethods]
impl PyApparentPlanet {
    /// Lowercase planet name (`"mercury"`, `"venus"`, ..., `"neptune"`).
    #[getter]
    fn name(&self) -> &'static str {
        planet_name(self.inner.planet)
    }

    #[getter]
    fn right_ascension_rad(&self) -> f64 {
        self.inner.right_ascension_rad
    }

    #[getter]
    fn declination_rad(&self) -> f64 {
        self.inner.declination_rad
    }

    #[getter]
    fn magnitude(&self) -> f64 {
        self.inner.magnitude
    }

    fn altaz(&self, observer: &PyObserver) -> (f64, f64) {
        let lst = astronomy::lmst_radians(observer.inner.time.jd_ut1, observer.inner.longitude_rad);
        let aa = equatorial_to_horizontal(
            self.inner.right_ascension_rad,
            self.inner.declination_rad,
            lst,
            observer.inner.latitude_rad,
        );
        altaz_to_tuple(aa)
    }
}

/// Apparent equatorial position of one of Jupiter's four Galilean moons.
#[pyclass(name = "ApparentGalileanMoon", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PyApparentGalileanMoon {
    inner: GalileanMoonApparent,
}

#[pymethods]
impl PyApparentGalileanMoon {
    /// One of `"io"`, `"europa"`, `"ganymede"`, `"callisto"`.
    #[getter]
    fn name(&self) -> &'static str {
        galilean_moon_name(self.inner.moon)
    }

    #[getter]
    fn right_ascension_rad(&self) -> f64 {
        self.inner.right_ascension_rad
    }

    #[getter]
    fn declination_rad(&self) -> f64 {
        self.inner.declination_rad
    }

    #[getter]
    fn magnitude(&self) -> f64 {
        self.inner.magnitude
    }
}

/// Apparent equatorial position of Saturn's Titan (V-52c, Meeus-grade).
#[pyclass(name = "ApparentTitan", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PyApparentTitan {
    inner: TitanApparent,
}

#[pymethods]
impl PyApparentTitan {
    #[getter]
    fn right_ascension_rad(&self) -> f64 {
        self.inner.right_ascension_rad
    }

    #[getter]
    fn declination_rad(&self) -> f64 {
        self.inner.declination_rad
    }

    #[getter]
    fn magnitude(&self) -> f64 {
        self.inner.magnitude
    }
}

// ---------------------------------------------------------------------------
// Catalog
// ---------------------------------------------------------------------------

/// Embedded star catalog (HYG compact subset). Provides random-access lookup
/// by index and a `len()` so notebooks can iterate without loading the full
/// CSV from disk.
#[pyclass(name = "StarCatalog", module = "stars_py")]
pub struct PyStarCatalog {
    stars: Vec<Star>,
}

#[pymethods]
impl PyStarCatalog {
    /// Load the embedded catalog (V-50 compact binary, baked at build time).
    #[staticmethod]
    fn load_embedded() -> Self {
        Self {
            stars: load_embedded(),
        }
    }

    fn __len__(&self) -> usize {
        self.stars.len()
    }

    /// Return the star at `index` as a dict-like Python object. Raises
    /// `IndexError` on out-of-range access, matching Python list semantics.
    fn star(&self, index: usize) -> PyResult<PyStar> {
        match self.stars.get(index).copied() {
            Some(inner) => Ok(PyStar { inner }),
            None => Err(PyIndexError::new_err(format!(
                "star index {index} out of range"
            ))),
        }
    }
}

/// A single catalog entry. The fields exposed are the renderer-relevant ones:
/// J2000 unit-vector position, V-magnitude, and the linear-sRGB chromaticity
/// vector derived from the catalog B−V via the V-23 pipeline.
#[pyclass(name = "Star", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PyStar {
    inner: Star,
}

#[pymethods]
impl PyStar {
    #[getter]
    fn position(&self) -> (f32, f32, f32) {
        (
            self.inner.position.x,
            self.inner.position.y,
            self.inner.position.z,
        )
    }

    #[getter]
    fn magnitude(&self) -> f32 {
        self.inner.magnitude
    }

    #[getter]
    fn color(&self) -> (f32, f32, f32) {
        (
            self.inner.color[0],
            self.inner.color[1],
            self.inner.color[2],
        )
    }

    #[getter]
    fn hyg_id(&self) -> Option<u32> {
        self.inner.identifiers.hyg
    }

    #[getter]
    fn hip_id(&self) -> Option<u32> {
        self.inner.identifiers.hip
    }
}

// ---------------------------------------------------------------------------
// Observation planning (mirrors astronomy::planning)
// ---------------------------------------------------------------------------

/// Rise / transit / set circumstances for one body over a planning window,
/// mirroring [`astronomy::RiseTransitSet`]. Each time is an `Optional[float]`
/// UTC Julian Date — `None` when the body never crosses the relevant altitude
/// in the window (circumpolar or never-rising).
#[pyclass(name = "RiseTransitSet", module = "stars_py", frozen)]
#[derive(Clone)]
pub struct PyRiseTransitSet {
    inner: RiseTransitSet,
}

#[pymethods]
impl PyRiseTransitSet {
    /// Body label (`"Sun"`, `"Moon"`, `"Mars"`, ...).
    #[getter]
    fn name(&self) -> &'static str {
        self.inner.name
    }

    #[getter]
    fn rise_jd_utc(&self) -> Option<f64> {
        self.inner.rise_jd_utc
    }

    #[getter]
    fn transit_jd_utc(&self) -> Option<f64> {
        self.inner.transit_jd_utc
    }

    #[getter]
    fn set_jd_utc(&self) -> Option<f64> {
        self.inner.set_jd_utc
    }

    /// Altitude (radians) of the body at transit, if it transits in the window.
    #[getter]
    fn transit_altitude_rad(&self) -> Option<f64> {
        self.inner.transit_altitude_rad
    }

    fn __repr__(&self) -> String {
        format!(
            "RiseTransitSet(name={:?}, rise={:?}, transit={:?}, set={:?})",
            self.inner.name,
            self.inner.rise_jd_utc,
            self.inner.transit_jd_utc,
            self.inner.set_jd_utc
        )
    }
}

/// One contiguous twilight band over the planning window, mirroring
/// [`astronomy::TwilightIndicator`].
#[pyclass(name = "TwilightIndicator", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PyTwilightIndicator {
    inner: TwilightIndicator,
}

#[pymethods]
impl PyTwilightIndicator {
    /// One of `"Daylight"`, `"Civil twilight"`, `"Nautical twilight"`,
    /// `"Astronomical twilight"`, `"Night"`.
    #[getter]
    fn band(&self) -> &'static str {
        self.inner.band.label()
    }

    #[getter]
    fn start_jd_utc(&self) -> f64 {
        self.inner.start_jd_utc
    }

    #[getter]
    fn end_jd_utc(&self) -> f64 {
        self.inner.end_jd_utc
    }

    fn __repr__(&self) -> String {
        format!(
            "TwilightIndicator(band={:?}, start={:.6}, end={:.6})",
            self.inner.band.label(),
            self.inner.start_jd_utc,
            self.inner.end_jd_utc
        )
    }
}

/// The full "tonight" plan for an observer — the local-noon-to-noon window
/// with per-body rise/transit/set rows and the twilight-band timeline.
/// Mirrors [`astronomy::EveningPlan`].
#[pyclass(name = "EveningPlan", module = "stars_py", frozen)]
#[derive(Clone)]
pub struct PyEveningPlan {
    inner: EveningPlan,
}

#[pymethods]
impl PyEveningPlan {
    #[getter]
    fn start_jd_utc(&self) -> f64 {
        self.inner.start_jd_utc
    }

    #[getter]
    fn end_jd_utc(&self) -> f64 {
        self.inner.end_jd_utc
    }

    /// Rise/transit/set rows for the default planning bodies (Sun, Moon,
    /// Mercury → Neptune), in that order.
    #[getter]
    fn rows(&self) -> Vec<PyRiseTransitSet> {
        self.inner
            .rows
            .iter()
            .cloned()
            .map(|inner| PyRiseTransitSet { inner })
            .collect()
    }

    /// The ordered twilight-band timeline across the window.
    #[getter]
    fn twilight(&self) -> Vec<PyTwilightIndicator> {
        self.inner
            .twilight
            .iter()
            .copied()
            .map(|inner| PyTwilightIndicator { inner })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Session (read/write round-trip of the crates/common JSON schema)
// ---------------------------------------------------------------------------

/// A mutable scene/session document matching the `crates/common`
/// `StarSession` JSON schema (camelCase, `SESSION_SCHEMA_VERSION`).
///
/// The binding wraps the parsed JSON value rather than re-declaring the full
/// schema, so loading a session written by `apps/cli`, tweaking the observer /
/// time / view, and re-serialising preserves every other field (overlays,
/// atmosphere, projection, eyepiece, corrections, ...) byte-for-byte. The
/// observer / time / view triad is exposed with typed accessors because that
/// is the part a reproducibility notebook actually edits; mutating `jd_utc`
/// recomputes the dependent time scales with the same
/// [`astronomy::TimeScales`] helper the renderer uses, so the document stays
/// internally consistent.
#[pyclass(name = "Session", module = "stars_py")]
#[derive(Clone)]
pub struct PySession {
    value: Value,
}

#[pymethods]
impl PySession {
    /// Build a session at the given location / UTC Julian Date / view from the
    /// embedded current-schema template. The non-observer fields (overlays,
    /// atmosphere, ...) take the template's defaults; mutate them later via
    /// `to_json` round-trips or set them through the host apps.
    #[new]
    #[pyo3(signature = (latitude_deg, longitude_deg, jd_utc, azimuth_deg=0.0, altitude_deg=45.0, fov_deg=85.0))]
    fn new(
        latitude_deg: f64,
        longitude_deg: f64,
        jd_utc: f64,
        azimuth_deg: f64,
        altitude_deg: f64,
        fov_deg: f64,
    ) -> PyResult<Self> {
        let mut value: Value = serde_json::from_str(DEFAULT_SESSION_TEMPLATE).map_err(|e| {
            PyValueError::new_err(format!("embedded session template is invalid: {e}"))
        })?;
        set_f64(&mut value, "observer", "latitudeDeg", latitude_deg)?;
        set_f64(&mut value, "observer", "longitudeDeg", longitude_deg)?;
        write_time_block(&mut value, &TimeScales::from_utc_julian_date(jd_utc))?;
        set_f64(&mut value, "view", "azimuthDeg", azimuth_deg)?;
        set_f64(&mut value, "view", "altitudeDeg", altitude_deg)?;
        set_f64(&mut value, "view", "fovDeg", fov_deg)?;
        Ok(Self { value })
    }

    /// Build a session from an existing [`Observer`] (carrying its full set of
    /// time scales) plus a view direction. Preserves the observer's exact
    /// `dut1` / leap-second state rather than recomputing from `jd_utc`.
    #[staticmethod]
    #[pyo3(signature = (observer, azimuth_deg=0.0, altitude_deg=45.0, fov_deg=85.0))]
    fn from_observer(
        observer: &PyObserver,
        azimuth_deg: f64,
        altitude_deg: f64,
        fov_deg: f64,
    ) -> PyResult<Self> {
        let mut value: Value = serde_json::from_str(DEFAULT_SESSION_TEMPLATE).map_err(|e| {
            PyValueError::new_err(format!("embedded session template is invalid: {e}"))
        })?;
        set_f64(
            &mut value,
            "observer",
            "latitudeDeg",
            observer.inner.latitude_rad.to_degrees(),
        )?;
        set_f64(
            &mut value,
            "observer",
            "longitudeDeg",
            observer.inner.longitude_rad.to_degrees(),
        )?;
        write_time_block(&mut value, &observer.inner.time)?;
        set_f64(&mut value, "view", "azimuthDeg", azimuth_deg)?;
        set_f64(&mut value, "view", "altitudeDeg", altitude_deg)?;
        set_f64(&mut value, "view", "fovDeg", fov_deg)?;
        Ok(Self { value })
    }

    /// Parse a session from a JSON string. Unknown / future fields are
    /// preserved verbatim through a later `to_json`, so the binding never
    /// silently drops data it does not understand.
    #[staticmethod]
    fn from_json(text: &str) -> PyResult<Self> {
        let value: Value = serde_json::from_str(text)
            .map_err(|e| PyValueError::new_err(format!("invalid session JSON: {e}")))?;
        if !value.is_object() {
            return Err(PyValueError::new_err("session JSON must be an object"));
        }
        Ok(Self { value })
    }

    /// Read and parse a session from a file path.
    #[staticmethod]
    fn load(path: &str) -> PyResult<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| PyIOError::new_err(format!("cannot read session '{path}': {e}")))?;
        Self::from_json(&text)
    }

    /// Serialise back to JSON. `pretty=True` (default) emits the same
    /// 2-space-indented layout the host apps write.
    #[pyo3(signature = (pretty=true))]
    fn to_json(&self, pretty: bool) -> PyResult<String> {
        let result = if pretty {
            serde_json::to_string_pretty(&self.value)
        } else {
            serde_json::to_string(&self.value)
        };
        result.map_err(|e| PyValueError::new_err(format!("failed to serialise session: {e}")))
    }

    /// Write the (pretty) JSON to a file path.
    fn save(&self, path: &str) -> PyResult<()> {
        let text = self.to_json(true)?;
        std::fs::write(path, text)
            .map_err(|e| PyIOError::new_err(format!("cannot write session '{path}': {e}")))
    }

    /// The `schemaVersion` field of the underlying document.
    #[getter]
    fn schema_version(&self) -> PyResult<u64> {
        self.value
            .get("schemaVersion")
            .and_then(Value::as_u64)
            .ok_or_else(|| PyValueError::new_err("session missing integer 'schemaVersion'"))
    }

    #[getter]
    fn latitude_deg(&self) -> PyResult<f64> {
        get_f64(&self.value, "observer", "latitudeDeg")
    }

    #[setter]
    fn set_latitude_deg(&mut self, value: f64) -> PyResult<()> {
        set_f64(&mut self.value, "observer", "latitudeDeg", value)
    }

    #[getter]
    fn longitude_deg(&self) -> PyResult<f64> {
        get_f64(&self.value, "observer", "longitudeDeg")
    }

    #[setter]
    fn set_longitude_deg(&mut self, value: f64) -> PyResult<()> {
        set_f64(&mut self.value, "observer", "longitudeDeg", value)
    }

    #[getter]
    fn jd_utc(&self) -> PyResult<f64> {
        get_f64(&self.value, "time", "jdUtc")
    }

    /// Set the UTC Julian Date and recompute the dependent time scales
    /// (UT1 / TAI / TT / TDB) from the document's stored `dut1Seconds`.
    #[setter]
    fn set_jd_utc(&mut self, value: f64) -> PyResult<()> {
        let dut1 = get_f64(&self.value, "time", "dut1Seconds").unwrap_or(0.0);
        write_time_block(
            &mut self.value,
            &TimeScales::from_utc_julian_date_with_dut1(value, dut1),
        )
    }

    #[getter]
    fn jd_ut1(&self) -> PyResult<f64> {
        get_f64(&self.value, "time", "jdUt1")
    }

    #[getter]
    fn jd_tt(&self) -> PyResult<f64> {
        get_f64(&self.value, "time", "jdTt")
    }

    #[getter]
    fn jd_tdb(&self) -> PyResult<f64> {
        get_f64(&self.value, "time", "jdTdb")
    }

    #[getter]
    fn azimuth_deg(&self) -> PyResult<f64> {
        get_f64(&self.value, "view", "azimuthDeg")
    }

    #[setter]
    fn set_azimuth_deg(&mut self, value: f64) -> PyResult<()> {
        set_f64(&mut self.value, "view", "azimuthDeg", value)
    }

    #[getter]
    fn altitude_deg(&self) -> PyResult<f64> {
        get_f64(&self.value, "view", "altitudeDeg")
    }

    #[setter]
    fn set_altitude_deg(&mut self, value: f64) -> PyResult<()> {
        set_f64(&mut self.value, "view", "altitudeDeg", value)
    }

    #[getter]
    fn fov_deg(&self) -> PyResult<f64> {
        get_f64(&self.value, "view", "fovDeg")
    }

    #[setter]
    fn set_fov_deg(&mut self, value: f64) -> PyResult<()> {
        set_f64(&mut self.value, "view", "fovDeg", value)
    }

    /// Build an [`Observer`] from this session's observer + time blocks,
    /// preserving the stored time scales so apparent-body queries reproduce
    /// exactly what the renderer would compute for the same session.
    fn observer(&self) -> PyResult<PyObserver> {
        let lat = get_f64(&self.value, "observer", "latitudeDeg")?;
        let lon = get_f64(&self.value, "observer", "longitudeDeg")?;
        let time = time_scales_from_session(&self.value)?;
        Ok(PyObserver {
            inner: Observer::from_degrees_with_time(lat, lon, time),
        })
    }

    fn __repr__(&self) -> String {
        let schema = self.value.get("schemaVersion").and_then(Value::as_u64);
        let lat = get_f64(&self.value, "observer", "latitudeDeg").ok();
        let lon = get_f64(&self.value, "observer", "longitudeDeg").ok();
        let jd = get_f64(&self.value, "time", "jdUtc").ok();
        format!("Session(schema={schema:?}, lat={lat:?}, lon={lon:?}, jd_utc={jd:?})")
    }
}

// ---------------------------------------------------------------------------
// Module-level free functions
// ---------------------------------------------------------------------------

/// Sun + Moon apparent state for the given observer.
#[pyfunction]
fn apparent_sun_moon(observer: &PyObserver) -> PySunMoon {
    PySunMoon {
        inner: SunMoonApparent::for_observer(observer.inner),
    }
}

/// Apparent positions for all 7 major planets, in `astronomy::Planet` order
/// (Mercury, Venus, Mars, Jupiter, Saturn, Uranus, Neptune).
#[pyfunction]
fn apparent_planets(observer: &PyObserver) -> Vec<PyApparentPlanet> {
    apparent_planets_topocentric(observer.inner)
        .into_iter()
        .map(|inner| PyApparentPlanet { inner })
        .collect()
}

/// Apparent positions for Jupiter's four Galilean moons, in
/// `astronomy::GalileanMoon` order (Io, Europa, Ganymede, Callisto).
#[pyfunction]
fn apparent_galilean_moons(observer: &PyObserver) -> Vec<PyApparentGalileanMoon> {
    apparent_galilean_moons_topocentric(observer.inner)
        .into_iter()
        .map(|inner| PyApparentGalileanMoon { inner })
        .collect()
}

/// Apparent position of Saturn's Titan (V-52c).
#[pyfunction]
fn apparent_titan(observer: &PyObserver) -> PyApparentTitan {
    PyApparentTitan {
        inner: apparent_titan_topocentric(observer.inner),
    }
}

/// Build an [`Observer`] from latitude / longitude / POSIX timestamp.
/// Exposed as a module function for the common notebook pattern of building
/// from `datetime.timestamp()`.
#[pyfunction]
fn observer_from_unix_seconds(
    latitude_deg: f64,
    longitude_deg: f64,
    unix_seconds: f64,
) -> PyObserver {
    PyObserver::from_unix_seconds(latitude_deg, longitude_deg, unix_seconds)
}

/// Convert a POSIX timestamp (seconds since 1970-01-01T00:00:00Z) to a UTC
/// Julian Date, using the same helper the renderer and `Observer` use.
#[pyfunction]
#[pyo3(name = "julian_date_from_unix_seconds")]
fn julian_date_from_unix_seconds_fn(unix_seconds: f64) -> f64 {
    julian_date_from_unix_seconds(unix_seconds)
}

/// Convert a UTC Julian Date to POSIX milliseconds (the units the host JSON
/// timeline and web UI use).
#[pyfunction]
fn jd_utc_to_unix_ms(jd_utc: f64) -> f64 {
    astro_jd_utc_to_unix_ms(jd_utc)
}

/// Classify a solar altitude (radians) into its twilight band label
/// (`"Daylight"`, `"Civil twilight"`, ..., `"Night"`).
#[pyfunction]
fn twilight_band(sun_altitude_rad: f64) -> &'static str {
    astro_twilight_band(sun_altitude_rad).label()
}

/// Topocentric apparent altitude (radians) of a named body for the observer's
/// instant. `body` is one of `"sun"`, `"moon"`, `"mercury"`, ..., `"neptune"`.
#[pyfunction]
fn body_altitude_rad(observer: &PyObserver, body: &str) -> PyResult<f64> {
    Ok(astro_body_altitude_rad(
        observer.inner,
        planning_body_from_name(body)?,
    ))
}

/// Rise / transit / set circumstances for a named body over the
/// `[start_jd_utc, end_jd_utc)` UTC window.
#[pyfunction]
fn rise_transit_set(
    observer: &PyObserver,
    body: &str,
    start_jd_utc: f64,
    end_jd_utc: f64,
) -> PyResult<PyRiseTransitSet> {
    let body = planning_body_from_name(body)?;
    Ok(PyRiseTransitSet {
        inner: astro_rise_transit_set(observer.inner, body, start_jd_utc, end_jd_utc),
    })
}

/// Ordered twilight-band timeline across the `[start_jd_utc, end_jd_utc)` UTC
/// window for the observer's location.
#[pyfunction]
fn twilight_indicators(
    observer: &PyObserver,
    start_jd_utc: f64,
    end_jd_utc: f64,
) -> Vec<PyTwilightIndicator> {
    astro_twilight_indicators(observer.inner, start_jd_utc, end_jd_utc)
        .into_iter()
        .map(|inner| PyTwilightIndicator { inner })
        .collect()
}

/// The full "tonight" plan (local-noon-to-noon window, per-body rise/transit/
/// set rows, and the twilight timeline) for the observer.
#[pyfunction]
fn evening_plan(observer: &PyObserver) -> PyEveningPlan {
    PyEveningPlan {
        inner: astro_evening_plan(observer.inner),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn planning_body_from_name(name: &str) -> PyResult<PlanningBody> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "sun" => PlanningBody::Sun,
        "moon" => PlanningBody::Moon,
        "mercury" => PlanningBody::Planet(Planet::Mercury),
        "venus" => PlanningBody::Planet(Planet::Venus),
        "mars" => PlanningBody::Planet(Planet::Mars),
        "jupiter" => PlanningBody::Planet(Planet::Jupiter),
        "saturn" => PlanningBody::Planet(Planet::Saturn),
        "uranus" => PlanningBody::Planet(Planet::Uranus),
        "neptune" => PlanningBody::Planet(Planet::Neptune),
        other => {
            return Err(PyValueError::new_err(format!(
                "unknown body {other:?}; expected one of sun, moon, mercury, venus, mars, \
                 jupiter, saturn, uranus, neptune"
            )))
        }
    })
}

/// Read a finite `f64` at `value[section][key]`, erroring (rather than
/// returning a silent default) when the field is missing or non-numeric.
fn get_f64(value: &Value, section: &str, key: &str) -> PyResult<f64> {
    value
        .get(section)
        .and_then(|s| s.get(key))
        .and_then(Value::as_f64)
        .ok_or_else(|| PyValueError::new_err(format!("session missing number '{section}.{key}'")))
}

/// Write `value[section][key] = number`, requiring `section` to already be a
/// JSON object so a malformed template fails loudly instead of growing a new
/// top-level field.
fn set_f64(value: &mut Value, section: &str, key: &str, number: f64) -> PyResult<()> {
    let obj = value
        .get_mut(section)
        .and_then(Value::as_object_mut)
        .ok_or_else(|| PyValueError::new_err(format!("session missing object '{section}'")))?;
    obj.insert(key.to_string(), Value::from(number));
    Ok(())
}

/// Write the full `time` block from a [`TimeScales`].
fn write_time_block(value: &mut Value, time: &TimeScales) -> PyResult<()> {
    set_f64(value, "time", "jdUtc", time.jd_utc)?;
    set_f64(value, "time", "jdUt1", time.jd_ut1)?;
    set_f64(value, "time", "jdTai", time.jd_tai)?;
    set_f64(value, "time", "jdTt", time.jd_tt)?;
    set_f64(value, "time", "jdTdb", time.jd_tdb)?;
    set_f64(
        value,
        "time",
        "taiMinusUtcSeconds",
        time.tai_minus_utc_seconds,
    )?;
    set_f64(value, "time", "dut1Seconds", time.dut1_seconds)?;
    Ok(())
}

/// Reconstruct a [`TimeScales`] from a session `time` block. Stored scales are
/// preferred for exact reproduction; any missing scale is recomputed from
/// `jdUtc` + `dut1Seconds` so older / minimal session files still load.
fn time_scales_from_session(value: &Value) -> PyResult<TimeScales> {
    let jd_utc = get_f64(value, "time", "jdUtc")?;
    let dut1 = get_f64(value, "time", "dut1Seconds").unwrap_or(0.0);
    let recomputed = TimeScales::from_utc_julian_date_with_dut1(jd_utc, dut1);
    Ok(TimeScales {
        jd_utc,
        jd_ut1: get_f64(value, "time", "jdUt1").unwrap_or(recomputed.jd_ut1),
        jd_tai: get_f64(value, "time", "jdTai").unwrap_or(recomputed.jd_tai),
        jd_tt: get_f64(value, "time", "jdTt").unwrap_or(recomputed.jd_tt),
        jd_tdb: get_f64(value, "time", "jdTdb").unwrap_or(recomputed.jd_tdb),
        tai_minus_utc_seconds: get_f64(value, "time", "taiMinusUtcSeconds")
            .unwrap_or(recomputed.tai_minus_utc_seconds),
        dut1_seconds: dut1,
    })
}

fn altaz_to_tuple(altaz: AltAz) -> (f64, f64) {
    (altaz.altitude, altaz.azimuth)
}

fn planet_name(planet: Planet) -> &'static str {
    match planet {
        Planet::Mercury => "mercury",
        Planet::Venus => "venus",
        Planet::Mars => "mars",
        Planet::Jupiter => "jupiter",
        Planet::Saturn => "saturn",
        Planet::Uranus => "uranus",
        Planet::Neptune => "neptune",
    }
}

fn galilean_moon_name(moon: GalileanMoon) -> &'static str {
    match moon {
        GalileanMoon::Io => "io",
        GalileanMoon::Europa => "europa",
        GalileanMoon::Ganymede => "ganymede",
        GalileanMoon::Callisto => "callisto",
    }
}

/// Pure-Rust entry point used by the unit test below so `make ci` exercises
/// the binding without needing a Python interpreter. Returns the Moon's
/// apparent altitude in radians for the supplied (lat, lon, JD_UTC).
pub fn moon_altitude_rad(latitude_deg: f64, longitude_deg: f64, jd_utc: f64) -> f64 {
    let observer = PyObserver::new(latitude_deg, longitude_deg, jd_utc);
    let sun_moon = apparent_sun_moon(&observer);
    let (alt, _az) = sun_moon.moon().altaz(&observer);
    alt
}

// ---------------------------------------------------------------------------
// Module init
// ---------------------------------------------------------------------------

/// PyO3 module entry point. The `extension-module` feature gates the cdylib
/// symbol so a plain `cargo check` does not require linking libpython.
#[pymodule]
fn stars_py(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyObserver>()?;
    m.add_class::<PyApparentSun>()?;
    m.add_class::<PyApparentMoon>()?;
    m.add_class::<PySunMoon>()?;
    m.add_class::<PyApparentPlanet>()?;
    m.add_class::<PyApparentGalileanMoon>()?;
    m.add_class::<PyApparentTitan>()?;
    m.add_class::<PyRiseTransitSet>()?;
    m.add_class::<PyTwilightIndicator>()?;
    m.add_class::<PyEveningPlan>()?;
    m.add_class::<PySession>()?;
    m.add_class::<PyStarCatalog>()?;
    m.add_class::<PyStar>()?;
    m.add_function(wrap_pyfunction!(apparent_sun_moon, m)?)?;
    m.add_function(wrap_pyfunction!(apparent_planets, m)?)?;
    m.add_function(wrap_pyfunction!(apparent_galilean_moons, m)?)?;
    m.add_function(wrap_pyfunction!(apparent_titan, m)?)?;
    m.add_function(wrap_pyfunction!(observer_from_unix_seconds, m)?)?;
    m.add_function(wrap_pyfunction!(julian_date_from_unix_seconds_fn, m)?)?;
    m.add_function(wrap_pyfunction!(jd_utc_to_unix_ms, m)?)?;
    m.add_function(wrap_pyfunction!(twilight_band, m)?)?;
    m.add_function(wrap_pyfunction!(body_altitude_rad, m)?)?;
    m.add_function(wrap_pyfunction!(rise_transit_set, m)?)?;
    m.add_function(wrap_pyfunction!(twilight_indicators, m)?)?;
    m.add_function(wrap_pyfunction!(evening_plan, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tokyo, summer solstice midnight UT — the same epoch used by
    /// `examples/notebooks/session_reproducibility.py`. The Moon's altitude
    /// must come out finite and within ±90°; the exact numeric pin lives in
    /// the astronomy crate's own tests, this just smoke-tests the binding
    /// surface end-to-end without a Python interpreter.
    #[test]
    fn moon_altitude_from_pure_rust_entry_point_is_finite() {
        // 2026-06-21T10:20:00Z (same epoch as the civil-twilight preset).
        let unix = 1_782_555_600.0;
        let jd_utc = julian_date_from_unix_seconds(unix);
        let alt = moon_altitude_rad(35.68, 139.69, jd_utc);
        assert!(alt.is_finite(), "moon altitude must be finite, got {alt}");
        assert!(
            (-std::f64::consts::FRAC_PI_2..=std::f64::consts::FRAC_PI_2).contains(&alt),
            "moon altitude {alt} rad outside ±π/2"
        );
    }

    /// The binding's `Observer` constructor must round-trip latitude /
    /// longitude through degrees → radians → degrees without drift, so a
    /// notebook reading `.latitude_deg` back from the wrapper sees its
    /// inputs and not a sanitised re-projection.
    #[test]
    fn observer_round_trips_lat_lon_degrees() {
        let obs = PyObserver::new(35.68, 139.69, 2_460_000.0);
        assert!((obs.latitude_deg() - 35.68).abs() < 1e-9);
        assert!((obs.longitude_deg() - 139.69).abs() < 1e-9);
        assert_eq!(obs.jd_utc(), 2_460_000.0);
    }

    /// The planet list returned by the binding must be in the same order as
    /// `astronomy::apparent_planets_topocentric`, so a notebook that indexes
    /// by integer position matches the renderer's `planet_directions`
    /// uniform layout.
    #[test]
    fn apparent_planets_match_astronomy_order() {
        let obs = PyObserver::new(0.0, 0.0, 2_460_000.0);
        let bound = apparent_planets(&obs);
        let raw = apparent_planets_topocentric(obs.inner());
        assert_eq!(bound.len(), raw.len());
        for (b, r) in bound.iter().zip(raw.iter()) {
            assert_eq!(b.name(), planet_name(r.planet));
            assert!((b.right_ascension_rad() - r.right_ascension_rad).abs() < 1e-12);
        }
    }

    /// Embedded catalog must yield at least one star and an out-of-range
    /// lookup must raise `IndexError` rather than panic — the latter is the
    /// hard contract the notebook-facing surface relies on for safe iteration.
    #[test]
    fn embedded_catalog_loads_and_index_errors_safely() {
        let cat = PyStarCatalog::load_embedded();
        assert!(cat.__len__() > 0, "embedded catalog should not be empty");
        let first = cat.star(0).expect("first star must exist");
        assert!(first.magnitude().is_finite());
        let oob = cat.star(usize::MAX);
        assert!(oob.is_err(), "out-of-range lookup must return an Err");
    }

    /// The evening plan must cover a ~1-day window with one row per default
    /// planning body and a non-empty, contiguous twilight timeline whose
    /// segments tile the window without gaps. This is the contract a planning
    /// notebook relies on when it renders the twilight bar.
    #[test]
    fn evening_plan_window_is_contiguous_and_complete() {
        let obs = PyObserver::new(35.68, 139.69, 2461239.9375);
        let plan = evening_plan(&obs);
        let span = plan.end_jd_utc() - plan.start_jd_utc();
        assert!((span - 1.0).abs() < 1e-6, "plan window should be ~1 day");
        assert_eq!(
            plan.rows().len(),
            astronomy::DEFAULT_PLANNING_BODIES.len(),
            "one row per default planning body"
        );
        let twilight = plan.twilight();
        assert!(!twilight.is_empty(), "twilight timeline must not be empty");
        assert!((twilight[0].start_jd_utc() - plan.start_jd_utc()).abs() < 1e-9);
        assert!(
            (twilight.last().unwrap().end_jd_utc() - plan.end_jd_utc()).abs() < 1e-9,
            "twilight timeline must reach the window end"
        );
        for pair in twilight.windows(2) {
            assert!(
                (pair[0].end_jd_utc() - pair[1].start_jd_utc()).abs() < 1e-9,
                "twilight segments must tile without gaps"
            );
        }
    }

    /// A named-body rise/transit/set query must accept the documented body
    /// names and reject anything else with a Python `ValueError`, never a
    /// panic.
    #[test]
    fn rise_transit_set_validates_body_name() {
        let obs = PyObserver::new(35.68, 139.69, 2461239.9375);
        let row =
            rise_transit_set(&obs, "Mars", 2461239.5, 2461240.5).expect("known body must resolve");
        assert_eq!(row.name(), "Mars");
        assert!(rise_transit_set(&obs, "pluto", 2461239.5, 2461240.5).is_err());
    }

    /// The embedded session template must be a current-schema document with
    /// the observer / time / view blocks the binding edits.
    #[test]
    fn embedded_session_template_is_current_schema() {
        let session = PySession::from_json(DEFAULT_SESSION_TEMPLATE).expect("template parses");
        // The renderer's current schema; the committed presets are regenerated
        // on every bump, so this guards the binding against a stale template.
        assert_eq!(session.schema_version().unwrap(), 7);
        assert!(session.latitude_deg().is_ok());
        assert!(session.jd_utc().is_ok());
        assert!(session.fov_deg().is_ok());
    }

    /// Building a session, mutating observer / time / view, serialising, and
    /// reparsing must preserve the edits, recompute the time scales from the
    /// new `jd_utc`, and leave the rest of the document intact.
    #[test]
    fn session_round_trips_and_recomputes_time_scales() {
        let mut session = PySession::new(35.68, 139.69, 2461239.9375, 155.0, 55.0, 85.0)
            .expect("construct session");
        // Mutate the editable triad.
        session.set_latitude_deg(-31.27).unwrap();
        session.set_longitude_deg(149.07).unwrap();
        session.set_jd_utc(2461300.25).unwrap();
        session.set_fov_deg(60.0).unwrap();

        // jd_utc setter must keep the dependent scales consistent with the
        // astronomy TimeScales helper.
        let expected = TimeScales::from_utc_julian_date(2461300.25);
        assert!((session.jd_tt().unwrap() - expected.jd_tt).abs() < 1e-9);
        assert!((session.jd_tdb().unwrap() - expected.jd_tdb).abs() < 1e-9);

        let json = session.to_json(true).unwrap();
        let reparsed = PySession::from_json(&json).expect("round-trip parses");
        assert!((reparsed.latitude_deg().unwrap() - (-31.27)).abs() < 1e-9);
        assert!((reparsed.fov_deg().unwrap() - 60.0).abs() < 1e-9);
        // A field the binding never touches must survive verbatim.
        assert!(reparsed.value.get("eyepiece").is_some());
        assert_eq!(reparsed.schema_version().unwrap(), 7);

        // The session-derived observer must agree with a direct Observer at
        // the same lat/lon/JD, so queries from a loaded session reproduce the
        // renderer's numerics.
        let from_session = reparsed.observer().unwrap();
        let direct = PyObserver::new(-31.27, 149.07, 2461300.25);
        assert!((from_session.latitude_deg() - direct.latitude_deg()).abs() < 1e-9);
        assert!((from_session.jd_tt() - direct.jd_tt()).abs() < 1e-9);
    }

    /// `from_observer` must carry the observer's exact stored time scales into
    /// the session rather than recomputing, and round-trip back to the same
    /// observer.
    #[test]
    fn session_from_observer_preserves_time_scales() {
        let obs = PyObserver::from_unix_seconds(35.68, 139.69, 1_782_555_600.0);
        let session = PySession::from_observer(&obs, 10.0, 20.0, 70.0).expect("build");
        let rebuilt = session.observer().unwrap();
        assert!((rebuilt.jd_utc() - obs.jd_utc()).abs() < 1e-9);
        assert!((rebuilt.jd_tt() - obs.jd_tt()).abs() < 1e-9);
        assert!((session.azimuth_deg().unwrap() - 10.0).abs() < 1e-9);
    }
}
