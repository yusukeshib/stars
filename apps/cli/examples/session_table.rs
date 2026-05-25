use std::path::PathBuf;

use anyhow::{Context, Result};
use astronomy::{
    apparent_moon_topocentric, apparent_planets_topocentric, apparent_sun_topocentric,
    equatorial_to_horizontal, lmst_radians, Observer,
};
use stars_host_common::load_session;

fn main() -> Result<()> {
    let session_path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .context("usage: cargo run -p stars-cli --example session-table -- <session.json>")?;
    let scene = load_session(&session_path)
        .with_context(|| format!("failed to load {}", session_path.display()))?
        .to_scene()?;
    let observer =
        Observer::from_degrees_with_time(scene.latitude_deg, scene.longitude_deg, scene.time);
    let lst = lmst_radians(observer.time.jd_ut1, observer.longitude_rad);

    println!(
        "kind,name,jd_utc,ra_deg,dec_deg,alt_deg,az_deg,distance_au,distance_km,angular_radius_arcmin,illuminated_fraction,phase_angle_deg,magnitude,twilight_band"
    );

    let sun = apparent_sun_topocentric(observer);
    print_row(Row {
        kind: "solar-system",
        name: "Sun",
        jd_utc: observer.time.jd_utc,
        right_ascension_rad: sun.right_ascension_rad,
        declination_rad: sun.declination_rad,
        lst_rad: lst,
        lat_rad: observer.latitude_rad,
        distance_au: Some(sun.distance_au),
        distance_km: None,
        angular_radius_rad: Some(sun.angular_radius_rad),
        illuminated_fraction: Some(1.0),
        phase_angle_rad: Some(0.0),
        magnitude: None,
        twilight_band: Some(
            astronomy::twilight_band(
                equatorial_to_horizontal(
                    sun.right_ascension_rad,
                    sun.declination_rad,
                    lst,
                    observer.latitude_rad,
                )
                .altitude,
            )
            .label(),
        ),
    });

    let moon = apparent_moon_topocentric(observer);
    print_row(Row {
        kind: "solar-system",
        name: "Moon",
        jd_utc: observer.time.jd_utc,
        right_ascension_rad: moon.right_ascension_rad,
        declination_rad: moon.declination_rad,
        lst_rad: lst,
        lat_rad: observer.latitude_rad,
        distance_au: None,
        distance_km: Some(moon.distance_km),
        angular_radius_rad: Some(moon.angular_radius_rad),
        illuminated_fraction: Some(moon.illuminated_fraction),
        phase_angle_rad: Some(moon.phase_angle_rad),
        magnitude: None,
        twilight_band: None,
    });

    if scene.planets_enabled {
        for planet in apparent_planets_topocentric(observer) {
            print_row(Row {
                kind: "solar-system",
                name: planet.planet.name(),
                jd_utc: observer.time.jd_utc,
                right_ascension_rad: planet.right_ascension_rad,
                declination_rad: planet.declination_rad,
                lst_rad: lst,
                lat_rad: observer.latitude_rad,
                distance_au: Some(planet.distance_au),
                distance_km: None,
                angular_radius_rad: Some(planet.angular_radius_rad),
                illuminated_fraction: Some(planet.illuminated_fraction),
                phase_angle_rad: Some(planet.phase_angle_rad),
                magnitude: Some(planet.magnitude),
                twilight_band: None,
            });
        }
    }

    Ok(())
}

struct Row<'a> {
    kind: &'a str,
    name: &'a str,
    jd_utc: f64,
    right_ascension_rad: f64,
    declination_rad: f64,
    lst_rad: f64,
    lat_rad: f64,
    distance_au: Option<f64>,
    distance_km: Option<f64>,
    angular_radius_rad: Option<f64>,
    illuminated_fraction: Option<f64>,
    phase_angle_rad: Option<f64>,
    magnitude: Option<f64>,
    twilight_band: Option<&'a str>,
}

fn print_row(row: Row<'_>) {
    let horizontal = equatorial_to_horizontal(
        row.right_ascension_rad,
        row.declination_rad,
        row.lst_rad,
        row.lat_rad,
    );
    println!(
        "{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{},{},{},{},{},{},{}",
        row.kind,
        row.name,
        row.jd_utc,
        row.right_ascension_rad.to_degrees(),
        row.declination_rad.to_degrees(),
        horizontal.altitude.to_degrees(),
        horizontal.azimuth.to_degrees(),
        fmt_optional(row.distance_au),
        fmt_optional(row.distance_km),
        fmt_optional(row.angular_radius_rad.map(|v| v.to_degrees() * 60.0)),
        fmt_optional(row.illuminated_fraction),
        fmt_optional(row.phase_angle_rad.map(f64::to_degrees)),
        fmt_optional(row.magnitude),
        row.twilight_band.unwrap_or(""),
    );
}

fn fmt_optional(value: Option<f64>) -> String {
    value.map(|v| format!("{v:.6}")).unwrap_or_default()
}
