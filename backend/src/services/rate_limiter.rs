use axum::{
    Json,
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::{Timelike, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncConnection, RunQueryDsl};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;

use crate::{AppState, schema::rate_limit_windows};

const ANON_LIMIT: i32 = 1_000;
const AUTH_LIMIT: i32 = 10_000;

/// Axum middleware that enforces per-IP / per-token hourly rate limits.
///
/// Anonymous requests are keyed by the **actual TCP remote address** (not
/// spoofable headers).  Authenticated Bearer-token requests use a hash of the
/// token value and get the higher 10 000/hr limit.
pub async fn rate_limit(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    let (key, limit) = classify(&state, &req, &addr);

    if check_and_increment(&state, &key, limit).await {
        next.run(req).await
    } else {
        (
            StatusCode::TOO_MANY_REQUESTS,
            [
                ("Retry-After", "3600"),
                (
                    "X-RateLimit-Limit",
                    if limit == AUTH_LIMIT { "10000" } else { "1000" },
                ),
                ("X-RateLimit-Reset", "3600"),
            ],
            Json(serde_json::json!({
                "error": "Rate limit exceeded. Authenticated requests get 10 000/hr; \
                          anonymous requests get 1 000/hr."
            })),
        )
            .into_response()
    }
}

/// Returns `(rate_limit_key, limit)` for this request.
///
/// Authenticated (Bearer token) callers are keyed by a truncated hash of the
/// raw token value and receive the higher limit.
///
/// For anonymous callers:
/// - If `TRUST_PROXY` is enabled, we use the first IP from `X-Forwarded-For`.
/// - Otherwise, we use the actual TCP remote address.
fn classify(state: &AppState, req: &Request, remote_addr: &SocketAddr) -> (String, i32) {
    if let Some(auth) = req.headers().get("authorization")
        && let Ok(val) = auth.to_str()
        && let Some(token) = val.strip_prefix("Bearer ")
    {
        let hash = Sha256::digest(token.as_bytes());
        let key = format!(
            "tok:{}",
            hash.iter()
                .take(8)
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        return (key, AUTH_LIMIT);
    }

    // ── Anonymous Caller ─────────────────────────────────────────────────────

    // If we trust the proxy, try to get the real client IP from the forwarded header.
    if state.config.trust_proxy {
        if let Some(forwarded) = req.headers().get("x-forwarded-for")
            && let Ok(s) = forwarded.to_str()
        {
            // X-Forwarded-For can be a comma-separated list; the first one is the client.
            if let Some(ip) = s.split(',').next().map(|s| s.trim()) {
                return (format!("ip:{ip}"), ANON_LIMIT);
            }
        }
    }

    // Fallback to the real TCP peer address.
    (format!("ip:{}", remote_addr.ip()), ANON_LIMIT)
}

/// Returns `true` if the request is within the limit (and increments the
/// counter), `false` if it is exceeded.  Fails **open** on DB errors.
async fn check_and_increment(state: &AppState, key: &str, limit: i32) -> bool {
    let mut conn = match state.db.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "rate_limit: DB connection failed — failing open");
            return true;
        }
    };

    let now = Utc::now();
    let window = now
        .date_naive()
        .and_hms_opt(now.hour(), 0, 0)
        .unwrap()
        .and_utc();

    let result = conn
        .transaction::<bool, diesel::result::Error, _>(async |conn| {
            use rate_limit_windows::dsl::{
                count, key as key_col, rate_limit_windows as table, window_start,
            };

            let existing = table
                .find(key)
                .select((count, window_start))
                .first::<(i32, chrono::DateTime<Utc>)>(conn)
                .await
                .optional()?;

            let new_count: i32 = match existing {
                None => {
                    diesel::insert_into(table)
                        .values((key_col.eq(key), count.eq(1), window_start.eq(&window)))
                        .execute(conn)
                        .await?;
                    1
                }
                Some((_, ref ws)) if *ws < window => {
                    diesel::update(table.find(key))
                        .set((count.eq(1), window_start.eq(&window)))
                        .execute(conn)
                        .await?;
                    1
                }
                Some((c, _)) => {
                    let next = c + 1;
                    diesel::update(table.find(key))
                        .set(count.eq(next))
                        .execute(conn)
                        .await?;
                    next
                }
            };

            Ok(new_count <= limit)
        })
        .await;

    match result {
        Ok(allowed) => allowed,
        Err(e) => {
            tracing::error!(error = %e, key, "rate_limit: transaction failed — failing open");
            true
        }
    }
}
