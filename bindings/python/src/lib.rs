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
    equatorial_to_horizontal, julian_date_from_unix_seconds, AltAz, GalileanMoon,
    GalileanMoonApparent, MoonApparent, Observer, Planet, PlanetApparent, SunApparent,
    SunMoonApparent, TitanApparent,
};
use catalog::{load_embedded, Star};
use pyo3::exceptions::PyIndexError;
use pyo3::prelude::*;

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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
    m.add_class::<PyStarCatalog>()?;
    m.add_class::<PyStar>()?;
    m.add_function(wrap_pyfunction!(apparent_sun_moon, m)?)?;
    m.add_function(wrap_pyfunction!(apparent_planets, m)?)?;
    m.add_function(wrap_pyfunction!(apparent_galilean_moons, m)?)?;
    m.add_function(wrap_pyfunction!(apparent_titan, m)?)?;
    m.add_function(wrap_pyfunction!(observer_from_unix_seconds, m)?)?;
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
}
