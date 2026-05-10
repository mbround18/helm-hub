use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use std::time::Duration;
use tokio::time;
use opentelemetry::{global, metrics::Gauge};

use crate::AppState;
use crate::schema::{artifact_versions, artifacts, users};

pub const METRIC_ARTIFACTS_TOTAL: &str = "helm_hub_artifacts_total";
pub const METRIC_VERSIONS_TOTAL: &str = "helm_hub_artifact_versions_total";
pub const METRIC_STORAGE_BYTES_TOTAL: &str = "helm_hub_storage_bytes_total";
pub const METRIC_USERS_TOTAL: &str = "helm_hub_users_total";

#[derive(QueryableByName)]
struct TotalStorage {
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::BigInt>)]
    sum: Option<i64>,
}

/// Starts a background loop that updates global system metrics every minute.
pub async fn start_background_metrics(state: AppState) {
    let mut interval = time::interval(Duration::from_secs(60));
    
    // Create OTEL instruments
    let meter = global::meter("helm-hub-backend");
    let otel_artifacts = meter.f64_gauge(METRIC_ARTIFACTS_TOTAL).build();
    let otel_versions = meter.f64_gauge(METRIC_VERSIONS_TOTAL).build();
    let otel_users = meter.f64_gauge(METRIC_USERS_TOTAL).build();
    let otel_storage = meter.f64_gauge(METRIC_STORAGE_BYTES_TOTAL).build();

    loop {
        interval.tick().await;
        if let Err(e) = update_metrics(&state, &otel_artifacts, &otel_versions, &otel_users, &otel_storage).await {
            tracing::error!(error = %e, "Failed to update background metrics");
        }
    }
}

async fn update_metrics(
    state: &AppState,
    otel_artifacts: &Gauge<f64>,
    otel_versions: &Gauge<f64>,
    otel_users: &Gauge<f64>,
    otel_storage: &Gauge<f64>,
) -> Result<(), crate::error::AppError> {
    let mut conn = state.db.get().await.map_err(|e| crate::error::AppError::Pool(e.to_string()))?;

    // 1. Artifact Count
    let artifact_count: i64 = artifacts::table.count().get_result(&mut conn).await?;
    metrics::gauge!(METRIC_ARTIFACTS_TOTAL).set(artifact_count as f64);
    otel_artifacts.record(artifact_count as f64, &[]);

    // 2. Version Count
    let version_count: i64 = artifact_versions::table.count().get_result(&mut conn).await?;
    metrics::gauge!(METRIC_VERSIONS_TOTAL).set(version_count as f64);
    otel_versions.record(version_count as f64, &[]);

    // 3. User Count
    let user_count: i64 = users::table.count().get_result(&mut conn).await?;
    metrics::gauge!(METRIC_USERS_TOTAL).set(user_count as f64);
    otel_users.record(user_count as f64, &[]);

    // 4. Total Storage Usage
    let total_storage: Option<i64> = diesel::sql_query("SELECT SUM(storage_usage_bytes)::BIGINT as sum FROM users")
        .get_result::<TotalStorage>(&mut conn)
        .await?
        .sum;
        
    let storage_val = total_storage.unwrap_or(0);
    metrics::gauge!(METRIC_STORAGE_BYTES_TOTAL).set(storage_val as f64);
    otel_storage.record(storage_val as f64, &[]);

    tracing::debug!(
        artifacts = artifact_count,
        versions = version_count,
        users = user_count,
        storage = storage_val,
        "Background metrics updated"
    );

    Ok(())
}
