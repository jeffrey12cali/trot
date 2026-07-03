//! `trot` — the honest little treadmill tracker.
//!
//! `trot daemon` runs the tracking engine (talks to the treadmill, serves the
//! local API). The other subcommands are thin terminal clients that read the
//! daemon's handshake file and query that API — so the CLI and any UI see the
//! exact same data.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "trot", version, about = "TROT — it's really only treadmilling.")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the tracking daemon (talks to the treadmill, serves the API).
    Daemon,
    /// Today's totals.
    Today,
    /// Whether the daemon is up and a treadmill is connected.
    Status,
    /// Recent sessions.
    Log {
        /// Only sessions from the last 7 days.
        #[arg(long)]
        week: bool,
        /// How many to show.
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
}

/// Where TROT keeps its database + handshake file. Override with `TROT_DATA_DIR`.
fn data_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("TROT_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let pd = directories::ProjectDirs::from("", "", "trot")
        .context("could not determine a platform data directory")?;
    Ok(pd.data_dir().to_path_buf())
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Daemon => run_daemon(),
        Cmd::Today => cmd_today(),
        Cmd::Status => cmd_status(),
        Cmd::Log { week, limit } => cmd_log(week, limit),
    }
}

// ── daemon ──────────────────────────────────────────────────────────────────

fn run_daemon() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let dir = data_dir()?;
        let engine = trot_core::start_engine(dir.clone()).await?;

        // Publish {port, token} so the CLI and Nowhere can reach the daemon.
        let runtime = serde_json::json!({ "port": engine.port, "token": engine.state.token });
        let runtime_path = dir.join("runtime.json");
        if let Err(e) = std::fs::write(&runtime_path, serde_json::to_vec_pretty(&runtime)?) {
            tracing::warn!("could not write handshake file: {e}");
        }
        tracing::info!(
            "trot daemon ready — API on 127.0.0.1:{} (handshake: {})",
            engine.port,
            runtime_path.display()
        );

        wait_for_shutdown_signal().await;
        // Disconnect the treadmill cleanly before we drop the runtime — otherwise
        // the SC110 keeps the link open until it's power-cycled.
        engine.state.shutdown(Duration::from_secs(5)).await;
        let _ = std::fs::remove_file(&runtime_path);
        tracing::info!("trot daemon shutting down");
        Ok::<(), anyhow::Error>(())
    })
}

/// Resolve when the user asks the daemon to stop — Ctrl-C, or SIGTERM (which is
/// what `kill` and most supervisors send). Without the SIGTERM arm a `kill` would
/// bypass the graceful BLE teardown.
async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

// ── CLI client ──────────────────────────────────────────────────────────────

fn daemon_port() -> Result<u16> {
    let path = data_dir()?.join("runtime.json");
    let bytes = std::fs::read(&path)
        .context("no running trot daemon — start it with `trot daemon`")?;
    let v: serde_json::Value = serde_json::from_slice(&bytes)?;
    v["port"]
        .as_u64()
        .map(|p| p as u16)
        .context("handshake file has no port")
}

fn get(path: &str) -> Result<serde_json::Value> {
    let port = daemon_port()?;
    let url = format!("http://127.0.0.1:{port}{path}");
    let resp = ureq::get(&url)
        .call()
        .map_err(|e| anyhow::anyhow!("cannot reach the trot daemon: {e}"))?;
    Ok(resp.into_json()?)
}

fn fmt_dur(s: i64) -> String {
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else {
        format!("{m}m {sec:02}s")
    }
}

fn fmt_dist(distance_raw: i64, unit: &str) -> String {
    let meters = (distance_raw * 10) as f64; // storage keeps decameters
    if unit == "mph" {
        format!("{:.2} mi", meters / 1609.344)
    } else {
        format!("{:.2} km", meters / 1000.0)
    }
}

fn cmd_today() -> Result<()> {
    let v = get("/api/today")?;
    let t = &v["totals"];
    let unit = v["display_unit"].as_str().unwrap_or("km/h");
    println!("Today · {}", v["date"].as_str().unwrap_or(""));
    println!("  steps      {}", t["steps"].as_i64().unwrap_or(0));
    println!("  distance   {}", fmt_dist(t["distance_raw"].as_i64().unwrap_or(0), unit));
    println!("  time       {}", fmt_dur(t["duration_s"].as_i64().unwrap_or(0)));
    println!("  calories   {}", t["calories"].as_i64().unwrap_or(0));
    println!("  sessions   {}", t["sessions"].as_i64().unwrap_or(0));
    println!("\n  It's really only treadmilling.");
    Ok(())
}

fn cmd_status() -> Result<()> {
    let h = get("/api/health")?;
    println!("daemon      up");
    println!(
        "treadmill   {}",
        if h["connected"].as_bool().unwrap_or(false) {
            "connected"
        } else {
            "not connected"
        }
    );
    Ok(())
}

fn cmd_log(week: bool, limit: i64) -> Result<()> {
    let v = get(&format!("/api/sessions?limit={limit}"))?;
    let empty = vec![];
    let rows = v["sessions"].as_array().unwrap_or(&empty);
    let cutoff = if week {
        // seconds since epoch, 7 days ago
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        now - 7.0 * 86400.0
    } else {
        0.0
    };

    let mut shown = 0;
    for s in rows {
        let started = s["started_ts"].as_f64().unwrap_or(0.0);
        if started < cutoff {
            continue;
        }
        let date = s["local_date"].as_str().unwrap_or("");
        let start_steps = s["start_steps"].as_i64().unwrap_or(0);
        let steps = (s["steps_end"].as_i64().unwrap_or(0) - start_steps).max(0);
        let dur = s["duration_s_end"].as_i64().unwrap_or(0);
        println!("{date}   {steps:>6} steps   {:>8}", fmt_dur(dur));
        shown += 1;
    }
    if shown == 0 {
        println!("No sessions yet. Hop on the belt.");
    }
    Ok(())
}
