/// Kubernetes operational endpoints.
///
/// Mount these at `/k8s/*` — they must be outside the auth middleware and
/// ideally on a separate network interface in production (e.g. via a second
/// Axum listener on port 9090 bound to 127.0.0.1 only).
///
///   GET /k8s/healthz  — liveness probe  (is the process alive?)
///   GET /k8s/readyz   — readiness probe (can the process serve traffic?)
///   GET /k8s/metrics  — Prometheus text exposition format
use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
};
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
};

use crate::AppState;

// ── Liveness ──────────────────────────────────────────────────────────────────

/// Always `200 OK` — the kubelet uses this to decide whether to restart the pod.
/// We only return non-200 if the process is so broken it cannot respond at all.
pub async fn healthz() -> StatusCode {
    StatusCode::OK
}

// ── Readiness ─────────────────────────────────────────────────────────────────

/// Returns `200 OK` only when every downstream dependency is reachable.
///
/// Kubernetes stops routing traffic to the pod while this returns non-200, which
/// prevents users from hitting a pod that is mid-startup or has lost its DB.
#[tracing::instrument(skip(state), name = "k8s.readyz")]
pub async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    let mut failures: Vec<&'static str> = Vec::new();

    // ── PostgreSQL pool ───────────────────────────────────────────────────────
    if state.db.get().await.is_err() {
        tracing::warn!("readyz: PostgreSQL pool exhausted or unavailable");
        failures.push("postgres_pool_unavailable");
    }

    // ── ClamAV daemon socket ──────────────────────────────────────────────────
    if state.config.clamav_enabled && !ping_clamd(&state.config.clamd_socket).await {
        tracing::warn!(socket = %state.config.clamd_socket, "readyz: clamd unreachable");
        failures.push("clamd_socket_unreachable");
    }

    // ── Storage writeability ──────────────────────────────────────────────────
    if let Err(e) = check_storage(&state.config.charts_storage_path).await {
        tracing::warn!(path = %state.config.charts_storage_path, error = %e, "readyz: storage not writable");
        failures.push("storage_readonly");
    }

    if failures.is_empty() {
        (StatusCode::OK, Json(json!({ "status": "ok" }))).into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "degraded", "failures": failures })),
        )
            .into_response()
    }
}

/// Sends a `PING` over the clamd Unix socket and checks for `PONG`.
///
/// The clamd null-terminated command protocol:
///   → `zPING\0`
///   ← `PONG\0`
async fn ping_clamd(socket_path: &str) -> bool {
    let Ok(mut stream) = UnixStream::connect(socket_path).await else {
        return false;
    };
    if stream.write_all(b"zPING\0").await.is_err() {
        return false;
    }
    let mut buf = [0u8; 8];
    match stream.read(&mut buf).await {
        Ok(n) if n > 0 => std::str::from_utf8(&buf[..n])
            .map(|s| s.trim_matches('\0').trim() == "PONG")
            .unwrap_or(false),
        _ => false,
    }
}

/// Verifies that the storage path is still writable.
async fn check_storage(path: &str) -> Result<(), std::io::Error> {
    let temp = std::path::Path::new(path).join(".readyz_test");
    tokio::fs::write(&temp, "ok").await?;
    let _ = tokio::fs::remove_file(temp).await;
    Ok(())
}

// ── Prometheus metrics ────────────────────────────────────────────────────────

/// Renders all registered metrics in the Prometheus text exposition format.
///
/// Scraped by your Prometheus instance or the OTel Collector's
/// `prometheus_simple` receiver.
pub async fn metrics_handler(axum::extract::State(state): axum::extract::State<AppState>) -> impl IntoResponse {
    let body = state.metrics.render();
    (
        [(
            header::CONTENT_TYPE,
            // OpenMetrics / Prometheus text format content-type
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}
