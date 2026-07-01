//! `trot` — runs the TROT tracking daemon: talks to the treadmill, records
//! sessions, and serves the local HTTP/WS API. Terminal subcommands
//! (`trot today`, `trot log --week`, …) land in a later phase; for now the
//! binary starts the daemon and stays up.

use anyhow::Context;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

/// Where TROT keeps its database and runtime handshake file. Override with
/// `TROT_DATA_DIR` (used by tests and by Nowhere when it bundles the daemon).
fn data_dir() -> anyhow::Result<PathBuf> {
    if let Ok(dir) = std::env::var("TROT_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let pd = directories::ProjectDirs::from("", "", "trot")
        .context("could not determine a platform data directory")?;
    Ok(pd.data_dir().to_path_buf())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let dir = data_dir()?;
    let engine = trot_core::start_engine(dir.clone()).await?;

    // Publish {port, token} so the CLI and Nowhere can reach the daemon.
    let runtime = serde_json::json!({ "port": engine.port, "token": engine.state.token });
    let runtime_path = dir.join("runtime.json");
    if let Err(e) = std::fs::write(&runtime_path, serde_json::to_vec_pretty(&runtime)?) {
        tracing::warn!("could not write runtime handshake file: {e}");
    }
    tracing::info!(
        "trot daemon ready — API on 127.0.0.1:{} (handshake: {})",
        engine.port,
        runtime_path.display()
    );

    // Run until Ctrl-C, then ask the engine's workers to stop.
    tokio::signal::ctrl_c().await?;
    engine.state.stop.store(true, Ordering::Relaxed);
    engine.state.wake.notify_waiters();
    let _ = std::fs::remove_file(&runtime_path);
    tracing::info!("trot daemon shutting down");
    Ok(())
}
