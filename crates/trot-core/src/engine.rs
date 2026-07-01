//! Engine bootstrap: opens storage, starts the API server, the BLE worker, and
//! the rollup/prune loop. No window, no tray — just the tracking engine.

use crate::app::AppState;
use crate::{api, ble, config, db};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const RETENTION_DAYS: f64 = 5.0;
const ROLLUP_INTERVAL_S: f64 = 300.0;

/// A running engine: the shared state plus the loopback port the API is on.
pub struct Engine {
    pub state: Arc<AppState>,
    pub port: u16,
}

/// Start the tracking engine against `data_dir`. Binds the API on an ephemeral
/// loopback port and spawns the BLE worker + rollup loop as background tasks,
/// then returns (the tasks keep running for the life of the process).
pub async fn start_engine(data_dir: PathBuf) -> anyhow::Result<Engine> {
    std::fs::create_dir_all(&data_dir)?;
    config::init_paths(&data_dir);

    let db = Arc::new(db::Db::open(config::db_path())?);

    // High-entropy per-launch token, required on state-changing /api calls so
    // neither another local process nor a cross-site request can drive the API.
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let state = AppState::new(
        db.clone(),
        config::display_unit(),
        config::active_device_id(),
        token,
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    tracing::info!("trot engine API on 127.0.0.1:{port}");

    let router = api::router(state.clone());
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, router).await {
            tracing::error!("api server error: {e}");
        }
    });

    // BLE worker — reconnecting device ingestion.
    let worker_state = state.clone();
    tokio::spawn(async move {
        ble::run(worker_state).await;
    });

    // Rollup + prune loop (catch up at startup, then every interval).
    let rollup_state = state.clone();
    tokio::spawn(async move {
        loop {
            if let Ok(res) = rollup_state.db.rollup_samples() {
                if let Some(n) = res.get("buckets_written").and_then(|v| v.as_i64()) {
                    if n > 0 {
                        tracing::info!("rollup wrote {n} buckets");
                    }
                }
            }
            let _ = rollup_state.db.prune_raw_samples(RETENTION_DAYS * 86400.0);
            tokio::time::sleep(Duration::from_secs_f64(ROLLUP_INTERVAL_S)).await;
        }
    });

    Ok(Engine { state, port })
}
