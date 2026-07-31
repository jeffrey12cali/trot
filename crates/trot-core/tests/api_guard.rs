//! End-to-end tests for the API's security guard and its bounds checks.
//!
//! This is the surface that decides who may read your activity data and who may
//! wipe it, so it gets exercised as real HTTP: a request goes through the actual
//! `Router` — middleware, extractors, handlers — rather than calling a handler
//! function directly. Everything here was previously untested.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt; // brings `oneshot`
use trot_core::app::AppState;
use trot_core::{api, db::Db};

const TOKEN: &str = "test-token-abc123";

fn app() -> axum::Router {
    let db = Arc::new(Db::open(":memory:").unwrap());
    let state = AppState::new(db, "km/h".into(), None, TOKEN.into());
    api::router(state)
}

/// Build a request; `Host` defaults to loopback so only the header under test varies.
fn req(method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("host", "127.0.0.1:1234")
        .body(Body::empty())
        .unwrap()
}

async fn status(r: Request<Body>) -> StatusCode {
    app().oneshot(r).await.unwrap().status()
}

// ---- token ----------------------------------------------------------------

#[tokio::test]
async fn writes_require_the_token() {
    // No token at all.
    assert_eq!(
        status(req("POST", "/api/disconnect")).await,
        StatusCode::FORBIDDEN
    );

    // A wrong token.
    let mut r = req("POST", "/api/disconnect");
    r.headers_mut()
        .insert("x-sc110-token", "nope".parse().unwrap());
    assert_eq!(status(r).await, StatusCode::FORBIDDEN);

    // The real token gets through.
    let mut r = req("POST", "/api/disconnect");
    r.headers_mut()
        .insert("x-sc110-token", TOKEN.parse().unwrap());
    assert_eq!(status(r).await, StatusCode::OK);
}

#[tokio::test]
async fn reads_do_not_require_the_token() {
    // Deliberate: a process running as you could read the SQLite file anyway,
    // so a token on reads would be theatre. Documented in the README.
    assert_eq!(status(req("GET", "/api/health")).await, StatusCode::OK);
}

// ---- Host header (DNS-rebinding) ------------------------------------------

#[tokio::test]
async fn non_loopback_host_is_rejected() {
    for host in [
        "evil.example",
        "127.0.0.1.evil.example", // suffix trick
        "localhost.evil.example",
        "0.0.0.0:1234",
        "192.168.1.10:1234", // a LAN address is not loopback
    ] {
        let r = Request::builder()
            .method("GET")
            .uri("/api/health")
            .header("host", host)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            status(r).await,
            StatusCode::FORBIDDEN,
            "host must be rejected: {host}"
        );
    }
}

#[tokio::test]
async fn loopback_hosts_are_accepted() {
    for host in [
        "127.0.0.1:1234",
        "localhost:1234",
        "[::1]:1234",
        "localhost",
    ] {
        let r = Request::builder()
            .method("GET")
            .uri("/api/health")
            .header("host", host)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            status(r).await,
            StatusCode::OK,
            "host must be allowed: {host}"
        );
    }
}

// ---- Origin (covers the /ws upgrade, which CORS does not) ------------------

#[tokio::test]
async fn a_hostile_origin_is_rejected_even_on_reads() {
    for origin in ["https://evil.example", "http://127.0.0.1.evil.example"] {
        let mut r = req("GET", "/api/state");
        r.headers_mut().insert("origin", origin.parse().unwrap());
        assert_eq!(
            status(r).await,
            StatusCode::FORBIDDEN,
            "origin must be rejected: {origin}"
        );
    }
}

#[tokio::test]
async fn the_app_and_dev_origins_are_accepted() {
    for origin in [
        "tauri://localhost",
        "http://tauri.localhost",
        "https://tauri.localhost",
        "http://localhost:5199",
    ] {
        let mut r = req("GET", "/api/state");
        r.headers_mut().insert("origin", origin.parse().unwrap());
        assert_eq!(
            status(r).await,
            StatusCode::OK,
            "origin must be allowed: {origin}"
        );
    }
}

#[tokio::test]
async fn no_origin_header_passes_through() {
    // The CLI and the app's own Rust calls send no Origin; they must not be
    // caught by a check aimed at browsers.
    assert_eq!(status(req("GET", "/api/state")).await, StatusCode::OK);
}

// ---- response hardening ----------------------------------------------------

#[tokio::test]
async fn responses_carry_nosniff() {
    let resp = app().oneshot(req("GET", "/api/health")).await.unwrap();
    assert_eq!(
        resp.headers().get("x-content-type-options").unwrap(),
        "nosniff"
    );
}

// ---- bounds / guard rails --------------------------------------------------

#[tokio::test]
async fn analytics_rejects_an_absurd_bucket_count() {
    // 5 years at minute resolution is ~2.6M buckets — a cheap request that would
    // otherwise turn into a CPU and memory amplifier.
    let r = req(
        "GET",
        "/api/analytics?metric=steps&resolution=minute&range_days=1825",
    );
    assert_eq!(status(r).await, StatusCode::BAD_REQUEST);

    // The same range at a sane resolution is fine.
    let r = req(
        "GET",
        "/api/analytics?metric=steps&resolution=day&range_days=1825",
    );
    assert_eq!(status(r).await, StatusCode::OK);
}

#[tokio::test]
async fn analytics_validates_metric_and_resolution() {
    for uri in [
        "/api/analytics?metric=DROP+TABLE&resolution=hour&range_days=1",
        "/api/analytics?metric=steps&resolution=fortnight&range_days=1",
        "/api/analytics?metric=steps&resolution=hour&range_days=0",
    ] {
        assert_eq!(
            status(req("GET", uri)).await,
            StatusCode::BAD_REQUEST,
            "{uri}"
        );
    }
}

#[tokio::test]
async fn reset_refuses_when_there_is_nothing_to_reset() {
    // Guards a real data-loss path: a second reset used to export the (now
    // empty) database over the snapshot the first reset had saved.
    let mut r = req("POST", "/api/data/reset");
    r.headers_mut()
        .insert("x-sc110-token", TOKEN.parse().unwrap());
    let resp = app().oneshot(r).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ok"], false, "an empty database must not be 'reset'");
    assert!(
        v["error"].as_str().unwrap().contains("already empty"),
        "the error should say why: {v}"
    );
}

#[tokio::test]
async fn health_reports_the_engine_version() {
    // The desktop app ships the engine as a separate sidecar, so it can be older
    // than the app bundling it; clients surface both.
    let resp = app().oneshot(req("GET", "/api/health")).await.unwrap();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn unknown_routes_are_404_not_500() {
    assert_eq!(status(req("GET", "/api/nope")).await, StatusCode::NOT_FOUND);
}
