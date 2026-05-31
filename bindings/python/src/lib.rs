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
    active_occluders as astro_active_occluders, apparent_galilean_moons_topocentric,
    apparent_planets_topocentric, apparent_titan_topocentric,
    body_altitude_rad as astro_body_altitude_rad, equatorial_to_horizontal,
    evening_plan as astro_evening_plan, find_lunar_occultation as astro_find_lunar_occultation,
    find_mutual_planetary_occultation as astro_find_mutual_planetary_occultation,
    find_planet_transit as astro_find_planet_transit,
    find_solar_eclipse as astro_find_solar_eclipse, jd_utc_to_unix_ms as astro_jd_utc_to_unix_ms,
    julian_date_from_unix_seconds, rise_transit_set as astro_rise_transit_set,
    twilight_band as astro_twilight_band, twilight_indicators as astro_twilight_indicators, AltAz,
    ContactTimes, EveningPlan, GalileanMoon, GalileanMoonApparent, LunarOccultationEvent,
    LunarOccultedBody, MoonApparent, MutualPlanetaryOccultationEvent, Observer, Occluder,
    OccluderTarget, Planet, PlanetApparent, PlanetTransitEvent, PlanningBody, RiseTransitSet,
    SolarEclipseEvent, SunApparent, SunMoonApparent, TimeScales, TitanApparent, TwilightIndicator,
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
// Occultations & eclipses (V-51 planning surface)
// ---------------------------------------------------------------------------

/// Canonical P1..P4 contact times (UTC Julian Dates) for an occultation or
/// eclipse, mirroring [`astronomy::ContactTimes`].
///
/// Each contact is `None` when that phase does not occur for the event (e.g.
/// `p2` / `p3` are `None` for a purely partial solar eclipse; for a lunar
/// occultation of a point-source star `p1 ≈ p2` and `p3 ≈ p4` because external
/// and internal contact coincide).
#[pyclass(name = "ContactTimes", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PyContactTimes {
    inner: ContactTimes,
}

#[pymethods]
impl PyContactTimes {
    /// First exterior contact (ingress begins), UTC Julian Date or `None`.
    #[getter]
    fn p1(&self) -> Option<f64> {
        self.inner.p1
    }
    /// First interior contact (fully entered), UTC Julian Date or `None`.
    #[getter]
    fn p2(&self) -> Option<f64> {
        self.inner.p2
    }
    /// Last interior contact (egress begins), UTC Julian Date or `None`.
    #[getter]
    fn p3(&self) -> Option<f64> {
        self.inner.p3
    }
    /// Last exterior contact (egress ends), UTC Julian Date or `None`.
    #[getter]
    fn p4(&self) -> Option<f64> {
        self.inner.p4
    }

    /// `(p1, p2, p3, p4)` as a tuple of `Optional[float]`.
    fn as_tuple(&self) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
        (self.inner.p1, self.inner.p2, self.inner.p3, self.inner.p4)
    }

    fn __repr__(&self) -> String {
        format!(
            "ContactTimes(p1={:?}, p2={:?}, p3={:?}, p4={:?})",
            self.inner.p1, self.inner.p2, self.inner.p3, self.inner.p4
        )
    }
}

/// A lunar occultation of a planet or star located inside a planning window,
/// mirroring [`astronomy::LunarOccultationEvent`].
#[pyclass(name = "LunarOccultation", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PyLunarOccultation {
    inner: LunarOccultationEvent,
}

#[pymethods]
impl PyLunarOccultation {
    /// Deepest geometry reached in the window: one of `"partial"`,
    /// `"annular-or-transit"`, or `"total"`.
    #[getter]
    fn kind(&self) -> &'static str {
        self.inner.kind.as_kebab_str()
    }
    /// Minimum Moon–body angular separation (radians) inside the window.
    #[getter]
    fn min_separation_rad(&self) -> f64 {
        self.inner.min_separation_rad
    }
    /// UTC Julian Date of minimum separation (peak phase).
    #[getter]
    fn peak_jd_utc(&self) -> f64 {
        self.inner.peak_jd_utc
    }
    /// P1..P4 contact times.
    #[getter]
    fn contacts(&self) -> PyContactTimes {
        PyContactTimes {
            inner: self.inner.contacts,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "LunarOccultation(kind={:?}, peak_jd_utc={:.6}, min_separation_rad={:.6e})",
            self.inner.kind.as_kebab_str(),
            self.inner.peak_jd_utc,
            self.inner.min_separation_rad
        )
    }
}

/// A solar eclipse circumstance for the observer inside a planning window,
/// mirroring [`astronomy::SolarEclipseEvent`].
#[pyclass(name = "SolarEclipse", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PySolarEclipse {
    inner: SolarEclipseEvent,
}

#[pymethods]
impl PySolarEclipse {
    /// Deepest phase reached anywhere in the window: one of `"partial"`,
    /// `"annular"`, or `"total"`.
    #[getter]
    fn kind(&self) -> &'static str {
        self.inner.kind.as_kebab_str()
    }
    /// Peak obscuration fraction in `[0, 1]`.
    #[getter]
    fn peak_obscuration(&self) -> f32 {
        self.inner.peak_obscuration
    }
    /// UTC Julian Date of peak obscuration.
    #[getter]
    fn peak_jd_utc(&self) -> f64 {
        self.inner.peak_jd_utc
    }
    /// P1..P4 contact times (`p2`/`p3` are `None` for a partial event).
    #[getter]
    fn contacts(&self) -> PyContactTimes {
        PyContactTimes {
            inner: self.inner.contacts,
        }
    }
    /// `True` for a central (annular or total) eclipse.
    fn is_central(&self) -> bool {
        self.inner.is_central()
    }

    fn __repr__(&self) -> String {
        format!(
            "SolarEclipse(kind={:?}, peak_obscuration={:.4}, peak_jd_utc={:.6})",
            self.inner.kind.as_kebab_str(),
            self.inner.peak_obscuration,
            self.inner.peak_jd_utc
        )
    }
}

/// A Mercury / Venus transit of the solar disk, mirroring
/// [`astronomy::PlanetTransitEvent`].
#[pyclass(name = "PlanetTransit", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PyPlanetTransit {
    inner: PlanetTransitEvent,
}

#[pymethods]
impl PyPlanetTransit {
    /// The transiting inner planet (`"mercury"` or `"venus"`).
    #[getter]
    fn planet(&self) -> &'static str {
        planet_name(self.inner.planet)
    }
    /// Geometry label at peak (always `"annular-or-transit"`).
    #[getter]
    fn kind(&self) -> &'static str {
        self.inner.kind.as_kebab_str()
    }
    /// Peak obscuration fraction `(r_planet / r_sun)²` in `[0, 1]`.
    #[getter]
    fn peak_obscuration(&self) -> f32 {
        self.inner.peak_obscuration
    }
    /// UTC Julian Date of minimum apparent separation.
    #[getter]
    fn peak_jd_utc(&self) -> f64 {
        self.inner.peak_jd_utc
    }
    /// P1..P4 contact times (exterior P1/P4, interior P2/P3).
    #[getter]
    fn contacts(&self) -> PyContactTimes {
        PyContactTimes {
            inner: self.inner.contacts,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "PlanetTransit(planet={:?}, peak_obscuration={:.2e}, peak_jd_utc={:.6})",
            planet_name(self.inner.planet),
            self.inner.peak_obscuration,
            self.inner.peak_jd_utc
        )
    }
}

/// A mutual planet-on-planet occultation, mirroring
/// [`astronomy::MutualPlanetaryOccultationEvent`].
#[pyclass(name = "MutualPlanetaryOccultation", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PyMutualOccultation {
    inner: MutualPlanetaryOccultationEvent,
}

#[pymethods]
impl PyMutualOccultation {
    /// Planet whose disk is in front at peak (closer to the observer).
    #[getter]
    fn front(&self) -> &'static str {
        planet_name(self.inner.front)
    }
    /// Planet whose disk is occulted at peak (farther from the observer).
    #[getter]
    fn back(&self) -> &'static str {
        planet_name(self.inner.back)
    }
    /// Deepest geometry reached: `"partial"`, `"annular-or-transit"`, or
    /// `"total"`.
    #[getter]
    fn kind(&self) -> &'static str {
        self.inner.kind.as_kebab_str()
    }
    /// Minimum apparent separation between the two planet centres (radians).
    #[getter]
    fn min_separation_rad(&self) -> f64 {
        self.inner.min_separation_rad
    }
    /// Peak obscuration fraction of the back disk in `[0, 1]`.
    #[getter]
    fn peak_obscuration(&self) -> f32 {
        self.inner.peak_obscuration
    }
    /// UTC Julian Date of minimum separation.
    #[getter]
    fn peak_jd_utc(&self) -> f64 {
        self.inner.peak_jd_utc
    }
    /// P1..P4 contact times (`p2`/`p3` `None` for a grazing partial event).
    #[getter]
    fn contacts(&self) -> PyContactTimes {
        PyContactTimes {
            inner: self.inner.contacts,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "MutualPlanetaryOccultation(front={:?}, back={:?}, kind={:?}, peak_jd_utc={:.6})",
            planet_name(self.inner.front),
            planet_name(self.inner.back),
            self.inner.kind.as_kebab_str(),
            self.inner.peak_jd_utc
        )
    }
}

/// One active occluder for the observer at the instant of their `Observer`
/// time, mirroring an entry of [`astronomy::ActiveOccluders`]. This is the
/// per-frame geometry the renderer feeds into its occlusion uniform.
#[pyclass(name = "Occluder", module = "stars_py", frozen)]
#[derive(Clone, Copy)]
pub struct PyOccluder {
    inner: Occluder,
}

#[pymethods]
impl PyOccluder {
    /// What is being occulted: `"sun"`, `"moon"`, a planet name, or
    /// `"stars"` (the catalog-star cull entry).
    #[getter]
    fn target(&self) -> String {
        occluder_target_name(self.inner.target)
    }
    /// Geometry label: `"partial"`, `"annular-or-transit"`, or `"total"`.
    #[getter]
    fn kind(&self) -> &'static str {
        self.inner.kind.as_kebab_str()
    }
    /// Fraction of the target disk obscured by the front body, `[0, 1]`.
    #[getter]
    fn obscuration(&self) -> f64 {
        self.inner.obscuration
    }
    /// Angular radius of the occulting (front) disk, radians.
    #[getter]
    fn front_radius_rad(&self) -> f64 {
        self.inner.front_radius_rad
    }
    /// Equatorial unit direction to the occulting (front) body, `(x, y, z)`.
    #[getter]
    fn front_dir_eq(&self) -> (f64, f64, f64) {
        let d = self.inner.front_dir_eq;
        (d[0], d[1], d[2])
    }

    fn __repr__(&self) -> String {
        format!(
            "Occluder(target={:?}, kind={:?}, obscuration={:.4})",
            occluder_target_name(self.inner.target),
            self.inner.kind.as_kebab_str(),
            self.inner.obscuration
        )
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

/// All occluders active for the observer at the instant of their `Observer`
/// time — the per-frame Moon-on-Sun / Moon-on-planet / mutual-planet / star
/// occlusion geometry the renderer consumes (V-51). Returns an empty list
/// when nothing is being occulted (aside from the always-emitted star-cull
/// entry the renderer relies on).
///
/// >>> occ = stars_py.active_occluders(observer)
/// >>> [(o.target, o.kind, round(o.obscuration, 3)) for o in occ]
#[pyfunction]
fn active_occluders(observer: &PyObserver) -> Vec<PyOccluder> {
    astro_active_occluders(observer.inner)
        .as_slice()
        .iter()
        .map(|&inner| PyOccluder { inner })
        .collect()
}

/// Find a lunar occultation of a **planet** inside the `[start_jd_utc,
/// end_jd_utc)` UTC window, or `None` if none occurs. `body` is a planet name
/// (`"mercury"`..`"neptune"`); use [`find_lunar_star_occultation`] for stars.
///
/// >>> ev = stars_py.find_lunar_occultation(obs, "venus", jd0, jd0 + 1.0)
/// >>> if ev: print(ev.kind, ev.peak_jd_utc, ev.contacts.as_tuple())
#[pyfunction]
fn find_lunar_occultation(
    observer: &PyObserver,
    body: &str,
    start_jd_utc: f64,
    end_jd_utc: f64,
) -> PyResult<Option<PyLunarOccultation>> {
    let planet = planet_from_name(body)?;
    Ok(astro_find_lunar_occultation(
        observer.inner,
        LunarOccultedBody::Planet(planet),
        start_jd_utc,
        end_jd_utc,
    )
    .map(|inner| PyLunarOccultation { inner }))
}

/// Find a lunar occultation of a **star** given its unit direction in the
/// date (mean-equator-of-date) equatorial frame, inside the `[start, end)`
/// UTC window. `dir_eq` is an `(x, y, z)` unit vector; this is the advanced
/// entry point used by the renderer's star-cull path. For catalogue stars
/// expressed as J2000 RA/Dec, precess to date first.
#[pyfunction]
fn find_lunar_star_occultation(
    observer: &PyObserver,
    dir_eq: (f64, f64, f64),
    start_jd_utc: f64,
    end_jd_utc: f64,
) -> Option<PyLunarOccultation> {
    let dir = glam::Vec3::new(dir_eq.0 as f32, dir_eq.1 as f32, dir_eq.2 as f32).normalize();
    astro_find_lunar_occultation(
        observer.inner,
        LunarOccultedBody::Star { dir_date_eq: dir },
        start_jd_utc,
        end_jd_utc,
    )
    .map(|inner| PyLunarOccultation { inner })
}

/// Find a solar eclipse visible from the observer inside the `[start_jd_utc,
/// end_jd_utc)` UTC window, or `None`. Reports the deepest phase, peak
/// obscuration, and the P1..P4 contact times.
///
/// >>> ev = stars_py.find_solar_eclipse(obs, jd0, jd0 + 1.0)
/// >>> if ev: print(ev.kind, ev.is_central(), ev.peak_obscuration)
#[pyfunction]
fn find_solar_eclipse(
    observer: &PyObserver,
    start_jd_utc: f64,
    end_jd_utc: f64,
) -> Option<PySolarEclipse> {
    astro_find_solar_eclipse(observer.inner, start_jd_utc, end_jd_utc)
        .map(|inner| PySolarEclipse { inner })
}

/// Find a Mercury or Venus transit of the Sun inside the `[start_jd_utc,
/// end_jd_utc)` UTC window, or `None`. `planet` must be `"mercury"` or
/// `"venus"` (the only inner planets that can transit); any other name raises
/// `ValueError`.
#[pyfunction]
fn find_planet_transit(
    observer: &PyObserver,
    planet: &str,
    start_jd_utc: f64,
    end_jd_utc: f64,
) -> PyResult<Option<PyPlanetTransit>> {
    let planet = planet_from_name(planet)?;
    if !matches!(planet, Planet::Mercury | Planet::Venus) {
        return Err(PyValueError::new_err(
            "only mercury and venus can transit the Sun",
        ));
    }
    Ok(
        astro_find_planet_transit(observer.inner, planet, start_jd_utc, end_jd_utc)
            .map(|inner| PyPlanetTransit { inner }),
    )
}

/// Find a mutual occultation between two planets inside the `[start_jd_utc,
/// end_jd_utc)` UTC window, or `None`. `planet_a` and `planet_b` are planet
/// names and must differ.
#[pyfunction]
fn find_mutual_planetary_occultation(
    observer: &PyObserver,
    planet_a: &str,
    planet_b: &str,
    start_jd_utc: f64,
    end_jd_utc: f64,
) -> PyResult<Option<PyMutualOccultation>> {
    let a = planet_from_name(planet_a)?;
    let b = planet_from_name(planet_b)?;
    if a == b {
        return Err(PyValueError::new_err(
            "planet_a and planet_b must be different planets",
        ));
    }
    Ok(
        astro_find_mutual_planetary_occultation(observer.inner, a, b, start_jd_utc, end_jd_utc)
            .map(|inner| PyMutualOccultation { inner }),
    )
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

/// Resolve a planet name to [`Planet`]. Rejects `"sun"` / `"moon"` and unknown
/// names with a Python `ValueError`.
fn planet_from_name(name: &str) -> PyResult<Planet> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "mercury" => Planet::Mercury,
        "venus" => Planet::Venus,
        "mars" => Planet::Mars,
        "jupiter" => Planet::Jupiter,
        "saturn" => Planet::Saturn,
        "uranus" => Planet::Uranus,
        "neptune" => Planet::Neptune,
        other => {
            return Err(PyValueError::new_err(format!(
                "unknown planet {other:?}; expected one of mercury, venus, mars, jupiter, \
                 saturn, uranus, neptune"
            )))
        }
    })
}

/// Human-readable label for an [`OccluderTarget`] (`"sun"`, `"moon"`, a planet
/// name, or `"stars"`).
fn occluder_target_name(target: OccluderTarget) -> String {
    match target {
        OccluderTarget::Sun => "sun".to_string(),
        OccluderTarget::Moon => "moon".to_string(),
        OccluderTarget::Stars => "stars".to_string(),
        OccluderTarget::Planet(i) => Planet::ALL
            .get(i as usize)
            .map(|p| planet_name(*p).to_string())
            .unwrap_or_else(|| format!("planet[{i}]")),
    }
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
    m.add_class::<PyContactTimes>()?;
    m.add_class::<PyLunarOccultation>()?;
    m.add_class::<PySolarEclipse>()?;
    m.add_class::<PyPlanetTransit>()?;
    m.add_class::<PyMutualOccultation>()?;
    m.add_class::<PyOccluder>()?;
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
    m.add_function(wrap_pyfunction!(active_occluders, m)?)?;
    m.add_function(wrap_pyfunction!(find_lunar_occultation, m)?)?;
    m.add_function(wrap_pyfunction!(find_lunar_star_occultation, m)?)?;
    m.add_function(wrap_pyfunction!(find_solar_eclipse, m)?)?;
    m.add_function(wrap_pyfunction!(find_planet_transit, m)?)?;
    m.add_function(wrap_pyfunction!(find_mutual_planetary_occultation, m)?)?;
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

    /// L-18: catalogue identifiers must survive the compact embedded (STRBIN4)
    /// / WASM path, so an on-screen pick on the web build can resolve a star's
    /// canonical identity. Sirius is HIP 32349 / HD 48915. This pins the same
    /// identity the HYG-CSV path produces (the `catalog` cross-ID tests) end to
    /// end through the embedded decoder.
    #[test]
    fn l18_embedded_preserves_sirius_identifiers() {
        let stars = catalog::load_embedded();
        let sirius = stars
            .iter()
            .find(|s| s.identifiers.hip == Some(32349))
            .expect("Sirius (HIP 32349) present in embedded catalog");
        assert_eq!(
            sirius.identifiers.primary_label().as_deref(),
            Some("HIP 32349")
        );
        assert_eq!(sirius.identifiers.hd, Some(48915));
        let (kind, value) = sirius.identifiers.pick_handle();
        assert_eq!(
            catalog::CatalogObjectId::from_parts(kind, value),
            Some(catalog::CatalogObjectId::Hipparcos(32349))
        );
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

    /// The occultation/eclipse finders must accept the documented body names,
    /// never panic on an empty window, and reject bad names with a Python
    /// `ValueError` rather than a panic. The window is intentionally a single
    /// instant so the search degenerates to a no-event probe — the contract
    /// under test is the binding's argument handling and Option mapping, not
    /// the astronomy crate's event detection (pinned in its own tests).
    #[test]
    fn occultation_finders_validate_and_map_options() {
        let obs = PyObserver::new(35.68, 139.69, 2461239.9375);
        // Degenerate window: end <= start → the astronomy layer returns None.
        assert!(
            find_lunar_occultation(&obs, "venus", 2461239.9375, 2461239.9375)
                .unwrap()
                .is_none()
        );
        assert!(find_solar_eclipse(&obs, 2461239.9375, 2461239.9375).is_none());
        assert!(
            find_planet_transit(&obs, "mercury", 2461239.9375, 2461239.9375)
                .unwrap()
                .is_none()
        );
        assert!(find_mutual_planetary_occultation(
            &obs,
            "venus",
            "mars",
            2461239.9375,
            2461239.9375
        )
        .unwrap()
        .is_none());

        // Name validation: planet finders reject sun/moon/unknown.
        assert!(find_lunar_occultation(&obs, "moon", 2461239.5, 2461240.5).is_err());
        assert!(find_planet_transit(&obs, "mars", 2461239.5, 2461240.5).is_err());
        assert!(
            find_mutual_planetary_occultation(&obs, "venus", "venus", 2461239.5, 2461240.5)
                .is_err(),
            "identical planets must be rejected"
        );

        // The star-occultation entry point accepts a direction tuple and
        // never panics on a degenerate window.
        assert!(
            find_lunar_star_occultation(&obs, (1.0, 0.0, 0.0), 2461239.9375, 2461239.9375)
                .is_none()
        );
    }

    /// `active_occluders` must return a bounded, well-formed list (never
    /// panicking) whose entries carry resolvable target labels — the contract
    /// a notebook iterating the per-frame occlusion geometry relies on.
    #[test]
    fn active_occluders_are_bounded_and_labelled() {
        let obs = PyObserver::new(35.68, 139.69, 2461239.9375);
        let occluders = active_occluders(&obs);
        assert!(
            occluders.len() <= astronomy::MAX_OCCLUDERS,
            "occluder count must not exceed the renderer's uniform bound"
        );
        for occ in &occluders {
            // Target label must be one of the documented strings.
            let target = occ.target();
            assert!(
                matches!(target.as_str(), "sun" | "moon" | "stars")
                    || planet_from_name(&target).is_ok(),
                "unexpected occluder target label {target:?}"
            );
            assert!((0.0..=1.0).contains(&occ.obscuration()));
            assert!(occ.front_radius_rad() >= 0.0);
        }
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
