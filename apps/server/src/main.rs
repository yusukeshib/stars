//! `stars-server` — headless HTTP host (ROADMAP **L-22**).
//!
//! Wraps the shared render pipeline in `stars_host_common::render` behind a
//! small axum service. The HTTP envelope is the only thing this binary owns;
//! every byte of GPU code, session loading, and preset resolution comes from
//! the same shared host-glue crate (`stars-host-common`) that powers the CLI.
//!
//! ## Endpoints
//!
//! - `GET  /healthz` → JSON status banner with the crate version.
//! - `GET  /presets` → list of built-in scene preset IDs and titles.
//! - `GET  /presets/{id}` → that preset's effective session JSON.
//! - `POST /render` (body: session JSON, query: `?width=&height=&skyglow=`)
//!   → PNG bytes (`Content-Type: image/png`).
//!
//! No external network calls. Observer geocoding stays client-side; the
//! catalog path is server-local and read-only at request time.

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use stars_host_common::{
    encode_png, render_scene_from_catalog_path, scene_preset_infos, session_from_preset,
    ScenePresetArg, ScenePresetInfo, SessionScene, StarSession, DEFAULT_SCREEN_LIMITING_MAGNITUDE,
};
use tokio::net::TcpListener;

/// Headless HTTP host that wraps the shared render pipeline.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Bind address (defaults to loopback only — set `0.0.0.0` to expose).
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// TCP port.
    #[arg(long, default_value_t = 8787)]
    port: u16,

    /// Default catalog path used when an incoming session has no
    /// `catalog.path` of its own.
    #[arg(long, default_value = "crates/catalog/data/hyg_v42.csv")]
    catalog: PathBuf,
}

#[derive(Clone)]
struct AppState {
    catalog: Arc<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var_os("RUST_LOG").is_none() {
        // Keep the default chatty enough to see request lines without
        // requiring users to set RUST_LOG by hand.
        std::env::set_var("RUST_LOG", "stars_server=info,info");
    }
    env_logger::init();
    let args = Args::parse();

    let state = AppState {
        catalog: Arc::new(args.catalog.clone()),
    };
    let app = router(state);
    let addr: SocketAddr = format!("{}:{}", args.host, args.port)
        .parse()
        .with_context(|| format!("invalid bind address {}:{}", args.host, args.port))?;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    log::info!("stars-server listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum serve loop")?;
    Ok(())
}

async fn shutdown_signal() {
    // Best-effort SIGINT handler so `ctrl-C` exits cleanly under
    // `cargo run`. Servers run under PID 1 in containers should
    // additionally handle SIGTERM; tokio's `signal::ctrl_c` covers
    // both on macOS / Linux for our usage.
    let _ = tokio::signal::ctrl_c().await;
    log::info!("stars-server: shutdown signal received");
}

/// Build the axum router. Exposed (crate-private) so the integration tests
/// can mount the same routes against an ephemeral listener.
fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/presets", get(list_presets))
        .route("/presets/{id}", get(get_preset))
        .route("/render", post(render_route))
        .with_state(state)
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "stars-server",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[derive(Debug, Serialize)]
struct PresetSummary {
    id: String,
    title: &'static str,
    description: &'static str,
    validation_focus: &'static str,
}

impl From<&ScenePresetInfo> for PresetSummary {
    fn from(info: &ScenePresetInfo) -> Self {
        Self {
            id: info.id.as_kebab_str().to_string(),
            title: info.title,
            description: info.description,
            validation_focus: info.validation_focus,
        }
    }
}

#[derive(Debug, Serialize)]
struct PresetsListResponse {
    presets: Vec<PresetSummary>,
}

async fn list_presets() -> Json<PresetsListResponse> {
    Json(PresetsListResponse {
        presets: scene_preset_infos()
            .iter()
            .map(PresetSummary::from)
            .collect(),
    })
}

async fn get_preset(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<StarSession>, AppError> {
    let preset = preset_from_kebab(&id)
        .ok_or_else(|| AppError::not_found(format!("unknown preset id: {id}")))?;
    // Use the bundled limiting magnitude default so the exported session
    // matches what `make scene-presets` writes to disk.
    let limiting_mag = DEFAULT_SCREEN_LIMITING_MAGNITUDE;
    let session = session_from_preset(
        preset,
        env!("CARGO_PKG_VERSION"),
        "stars-server",
        state.catalog.as_path(),
        limiting_mag,
    )
    .map_err(AppError::internal)?;
    Ok(Json(session))
}

fn preset_from_kebab(id: &str) -> Option<ScenePresetArg> {
    scene_preset_infos()
        .iter()
        .find(|info| info.id.as_kebab_str() == id)
        .map(|info| info.id)
}

#[derive(Debug, Deserialize)]
struct RenderQuery {
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_skyglow")]
    skyglow: bool,
}

fn default_width() -> u32 {
    1280
}
fn default_height() -> u32 {
    720
}
fn default_skyglow() -> bool {
    true
}

async fn render_route(
    State(state): State<AppState>,
    Query(q): Query<RenderQuery>,
    Json(session): Json<StarSession>,
) -> Result<Response, AppError> {
    let scene = session.to_scene().map_err(AppError::bad_request)?;
    let options = stars_host_common::RenderOptions {
        width: q.width.clamp(16, 8192),
        height: q.height.clamp(16, 8192),
        skyglow_enabled: q.skyglow,
    };

    // The GPU pipeline is sync-driven (pollster) — run it on a blocking
    // worker so the axum runtime stays responsive while a single render
    // is in flight. This is fine for the V-L-22 reproducibility-first
    // throughput target.
    let scene_clone: SessionScene = scene;
    let catalog = state.catalog.clone();
    let pixels: Vec<u8> = tokio::task::spawn_blocking(move || {
        pollster::block_on(render_scene_from_catalog_path(
            &scene_clone,
            catalog.as_path(),
            options,
        ))
    })
    .await
    .map_err(|e| AppError::internal(anyhow::anyhow!(e)))?
    .map_err(AppError::internal)?;

    let png = encode_png(options.width, options.height, pixels).map_err(AppError::internal)?;
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    Ok((StatusCode::OK, headers, png).into_response())
}

/// Single error type for all routes. Keeps the surface JSON-shaped so
/// clients (curl, Python, the validation gallery) can rely on a stable
/// `{ "error": ..., "detail": ... }` envelope.
#[derive(Debug)]
struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.into(),
        }
    }
    fn bad_request(err: anyhow::Error) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: format!("{err:#}"),
        }
    }
    fn internal(err: anyhow::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("{err:#}"),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({
            "error": self.status.canonical_reason().unwrap_or("error"),
            "detail": self.message,
        });
        (self.status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Boots the router on an ephemeral port and confirms `/healthz` and
    /// `/presets` round-trip. `/render` is exercised separately (and only
    /// where a GPU adapter is available) because CI lanes without one
    /// would otherwise see a confusing wgpu error.
    #[tokio::test]
    async fn healthz_and_presets_round_trip() {
        let state = AppState {
            catalog: Arc::new(PathBuf::from("crates/catalog/data/hyg_v42.csv")),
        };
        let app = router(state);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // /healthz
        let resp = reqwest::get(format!("http://{addr}/healthz"))
            .await
            .unwrap();
        assert!(resp.status().is_success());
        let json: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["service"], "stars-server");

        // /presets list
        let resp = reqwest::get(format!("http://{addr}/presets"))
            .await
            .unwrap();
        assert!(resp.status().is_success());
        let json: serde_json::Value = resp.json().await.unwrap();
        let presets = json["presets"].as_array().unwrap();
        assert!(!presets.is_empty(), "preset list must be non-empty");
        assert!(presets.iter().any(|p| p["id"] == "tokyo-tonight"));

        // /presets/{unknown} → 404 with the JSON error envelope
        let resp = reqwest::get(format!("http://{addr}/presets/no-such-thing"))
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::NOT_FOUND);
        let json: serde_json::Value = resp.json().await.unwrap();
        assert_eq!(json["error"], "Not Found");

        server.abort();
    }
}
