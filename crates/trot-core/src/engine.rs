//! Engine bootstrap: opens storage, starts the API server, the BLE worker, and
//! the rollup/prune loop. No window, no tray — just the tracking engine.

use crate::app::AppState;
use crate::db::{RETENTION_DAYS, ROLLUP_INTERVAL_S};
use crate::{api, ble, config, db};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

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

    // BLE worker — reconnecting device ingestion. Supervised: a panic here used
    // to kill ingestion silently for the rest of the process's life, with the API
    // still cheerfully serving stale data.
    let worker_state = state.clone();
    tokio::spawn(async move {
        supervise("ble worker", worker_state, ble::run).await;
    });

    // Rollup + prune loop (catch up at startup, then every interval). Also
    // supervised: if this dies, `last_rolled_ts` stops advancing, raw samples stop
    // being pruned, and day totals silently get slower and slower.
    let rollup_state = state.clone();
    tokio::spawn(async move {
        supervise("rollup loop", rollup_state, |s| async move {
            loop {
                match s.db.rollup_samples() {
                    Ok(res) => {
                        if let Some(n) = res.get("buckets_written").and_then(|v| v.as_i64()) {
                            if n > 0 {
                                tracing::info!("rollup wrote {n} buckets");
                            }
                        }
                    }
                    Err(e) => tracing::warn!("rollup failed: {e}"),
                }
                if let Err(e) = s.db.prune_raw_samples(RETENTION_DAYS * 86400.0) {
                    tracing::warn!("prune failed: {e}");
                }
                tokio::time::sleep(Duration::from_secs_f64(ROLLUP_INTERVAL_S)).await;
            }
        })
        .await;
    });

    Ok(Engine { state, port })
}

/// Run a long-lived background task, restarting it if it panics.
///
/// These tasks are spawned detached, so without this a panic would abort just
/// that task: `tokio::spawn` catches the unwind and drops the JoinHandle's error
/// on the floor. Ingestion or retention would stop dead while the process kept
/// running and the API kept answering — the worst kind of failure, because
/// nothing looks broken. We log loudly and restart after a short delay, and stop
/// for good once shutdown has been requested.
async fn supervise<F, Fut>(name: &'static str, state: Arc<AppState>, mut task: F)
where
    F: FnMut(Arc<AppState>) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    use futures::FutureExt;
    /// Long enough that a persistently-panicking task can't spin the CPU.
    const RESTART_DELAY: Duration = Duration::from_secs(5);

    while !state.stop.load(std::sync::atomic::Ordering::Relaxed) {
        let run = std::panic::AssertUnwindSafe(task(state.clone())).catch_unwind();
        match run.await {
            Ok(()) => break, // returned normally — it is done, don't restart
            Err(panic) => {
                let msg = panic
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| panic.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "<non-string panic>".into());
                tracing::error!("{name} panicked ({msg}); restarting in {RESTART_DELAY:?}");
                tokio::time::sleep(RESTART_DELAY).await;
            }
        }
    }
    tracing::info!("{name} stopped");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A panicking background task must be restarted, not silently abandoned —
    /// and it must stop for good once shutdown is requested.
    #[tokio::test(start_paused = true)]
    async fn supervise_restarts_a_panicking_task_until_stop() {
        let db = Arc::new(db::Db::open(":memory:").unwrap());
        let state = AppState::new(db, "km/h".into(), None, "tok".into());
        let runs = Arc::new(AtomicU32::new(0));

        let counter = runs.clone();
        let supervised = state.clone();
        let handle = tokio::spawn(async move {
            supervise("panicky", supervised, move |s| {
                let counter = counter.clone();
                async move {
                    // Panic on the first few runs, then request shutdown so the
                    // supervisor exits and the test terminates.
                    let n = counter.fetch_add(1, Ordering::SeqCst);
                    if n < 3 {
                        panic!("boom {n}");
                    }
                    s.stop.store(true, Ordering::Relaxed);
                }
            })
            .await;
        });

        // start_paused auto-advances the clock over the restart sleeps.
        tokio::time::timeout(Duration::from_secs(60), handle)
            .await
            .expect("supervisor should finish once stop is set")
            .unwrap();

        assert_eq!(
            runs.load(Ordering::SeqCst),
            4,
            "three panics should be restarted, then the clean run ends it"
        );
    }
}
