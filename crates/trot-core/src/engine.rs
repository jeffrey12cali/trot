//! Engine bootstrap: opens storage, starts the API server, the BLE worker, and
//! the rollup/prune loop. No window, no tray — just the tracking engine.

use crate::app::AppState;
use crate::{api, ble, config, db};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const RETENTION_DAYS: f64 = 7.0;
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
    // Lock the data dir down to the owner (0700): it holds the activity DB, the
    // reset snapshot, and (via the daemon) the API-token handshake file. On a
    // shared host another local user must not be able to read them.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&data_dir, std::fs::Permissions::from_mode(0o700));
    }
    config::init_paths(&data_dir);

    let db = Arc::new(db::Db::open(config::db_path())?);

    // One-time Phase-0 retention migration (gated by PRAGMA user_version): backfill
    // rollups over ALL raw history, THEN prune raw beyond retention, THEN VACUUM.
    // Must run before the rollup/prune loop so history is banked into rollups
    // before any raw is dropped. Idempotent — a no-op on every subsequent boot.
    match db.run_startup_migration(RETENTION_DAYS * 86400.0) {
        Ok(res) if res.get("ran").and_then(|v| v.as_bool()).unwrap_or(false) => {
            tracing::info!("phase0 retention migration ran: {res}");
        }
        Ok(_) => {}
        Err(e) => tracing::error!("phase0 retention migration failed: {e}"),
    }

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
