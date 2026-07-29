//! `trot` — the honest little treadmill tracker.
//!
//! `trot daemon` runs the tracking engine (talks to the treadmill, serves the
//! local API). The other subcommands are thin terminal clients that read the
//! daemon's handshake file and query that API — so the CLI and any UI see the
//! exact same data.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::IsTerminal;
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
    /// Scan for nearby treadmills and pick one to pair (interactive).
    Scan {
        /// How long to scan for, in seconds (1–15).
        #[arg(long, default_value_t = 6.0)]
        seconds: f64,
        /// Show every Bluetooth device, not just treadmills.
        #[arg(long)]
        all: bool,
        /// Just print the list; don't show the interactive picker.
        #[arg(long)]
        list: bool,
    },
    /// List paired treadmills (the active one is marked with *).
    Devices,
    /// Pair a treadmill from `trot scan` and make it the active one.
    Pair {
        /// The device id printed by `trot scan`.
        device_id: String,
        /// A friendly name to remember it by.
        #[arg(long)]
        name: Option<String>,
    },
    /// Forget the active treadmill.
    Unpair,
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
        Cmd::Scan { seconds, all, list } => cmd_scan(seconds, all, list),
        Cmd::Devices => cmd_devices(),
        Cmd::Pair { device_id, name } => cmd_pair(device_id, name),
        Cmd::Unpair => cmd_unpair(),
    }
}

// ── daemon ──────────────────────────────────────────────────────────────────

fn run_daemon() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Refuse to start a second daemon on the same data directory. Two daemons
    // would fight over the Bluetooth adapter, both overwrite runtime.json (so the
    // CLI would reach only one of them), and interleave writes into the same
    // SQLite file. We probe for a *live* daemon rather than just a lock file, so a
    // stale handshake left by a crash never blocks a legitimate restart.
    if let Some((port, _)) = live_daemon() {
        anyhow::bail!(
            "a trot daemon is already running on this data directory \
             (API on 127.0.0.1:{port}).\n\
             Stop it first, or point this one somewhere else with TROT_DATA_DIR."
        );
    }

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let dir = data_dir()?;
        let engine = trot_core::start_engine(dir.clone()).await?;

        // Publish {port, token} so the CLI and Nowhere can reach the daemon. This
        // file holds the API token; `atomic_write` creates it 0600 before writing
        // a byte, so it is never briefly world-readable on a shared host.
        let runtime = serde_json::json!({ "port": engine.port, "token": engine.state.token });
        let runtime_path = dir.join("runtime.json");
        if let Err(e) =
            trot_core::config::atomic_write(&runtime_path, &serde_json::to_vec_pretty(&runtime)?)
        {
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

/// Resolve when the daemon should stop — Ctrl-C, SIGTERM (what `kill` and most
/// supervisors send), or our PARENT DYING. The parent arm matters for Nowhere:
/// on macOS a Cmd-Q can tear the app down without it sending us a signal or
/// killing us, orphaning the daemon with the treadmill still connected. Watching
/// for the orphan lets us disconnect cleanly instead of leaking the BLE link
/// until the treadmill is power-cycled. (Also covers a Nowhere crash.)
async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let parent = wait_for_parent_death();
        tokio::pin!(parent);
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                    _ = &mut parent => {}
                }
            }
            Err(_) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = &mut parent => {}
                }
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Resolves when our parent process goes away. Reparenting flips `getppid()` (an
/// orphan is adopted by init/launchd, pid 1), so we record the launching pid and
/// watch for it to change. If we were started directly by init there's no parent
/// to lose, so we never resolve.
#[cfg(unix)]
async fn wait_for_parent_death() {
    let initial = unsafe { libc::getppid() };
    if initial <= 1 {
        std::future::pending::<()>().await;
    }
    loop {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        if unsafe { libc::getppid() } != initial {
            tracing::info!("trot: parent process gone — shutting down to release the treadmill");
            return;
        }
    }
}

// ── CLI client ──────────────────────────────────────────────────────────────

/// The daemon's handshake: (port, token) from runtime.json, or None if no file.
fn runtime() -> Option<(u16, String)> {
    let path = data_dir().ok()?.join("runtime.json");
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
    let port = v["port"].as_u64()? as u16;
    let token = v["token"].as_str().unwrap_or("").to_string();
    Some((port, token))
}

fn daemon_port() -> Result<u16> {
    runtime()
        .map(|(p, _)| p)
        .context("no running trot daemon — start it with `trot daemon`")
}

/// (port, token) if a daemon is actually reachable — not just a stale handshake
/// file left behind by a crash. Used to decide between the API and doing the work
/// locally: while the daemon is up it owns the Bluetooth adapter, so scanning and
/// pairing must go through it.
fn live_daemon() -> Option<(u16, String)> {
    let (port, token) = runtime()?;
    // Connection-refused on loopback returns immediately when nothing's listening.
    ureq::get(&format!("http://127.0.0.1:{port}/api/health"))
        .call()
        .ok()
        .map(|_| (port, token))
}

fn get(path: &str) -> Result<serde_json::Value> {
    let port = daemon_port()?;
    let url = format!("http://127.0.0.1:{port}{path}");
    let resp = ureq::get(&url)
        .call()
        .map_err(|e| anyhow::anyhow!("cannot reach the trot daemon: {e}"))?;
    Ok(resp.into_json()?)
}

/// POST to the daemon. Mutating routes require the per-launch token (loopback
/// Host + `x-sc110-token`), so we read both from the handshake file.
fn post(path: &str, body: serde_json::Value) -> Result<serde_json::Value> {
    let (port, token) = runtime().context("no running trot daemon — start it with `trot daemon`")?;
    let url = format!("http://127.0.0.1:{port}{path}");
    let resp = ureq::post(&url)
        .set("x-sc110-token", &token)
        .send_json(body)
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

// ── pairing ───────────────────────────────────────────────────────────────────

fn cmd_scan(seconds: f64, all: bool, list: bool) -> Result<()> {
    // If the daemon is up it owns the adapter, so let it scan; otherwise scan
    // ourselves so you can pair before ever starting the daemon.
    let v = if live_daemon().is_some() {
        get(&format!("/api/scan?seconds={seconds}&all_devices={all}"))?
    } else {
        trot_core::config::init_paths(&data_dir()?);
        tokio::runtime::Runtime::new()?.block_on(trot_core::ble::scan(seconds, all))?
    };

    let empty = vec![];
    let devices = v["devices"].as_array().unwrap_or(&empty);
    if devices.is_empty() {
        println!("No treadmills found. Make sure it's powered on and nearby, then try again.");
        println!("(Tip: `trot scan --all` lists every Bluetooth device.)");
        return Ok(());
    }

    let name_of = |d: &serde_json::Value| match d["name"].as_str().unwrap_or("") {
        "" => "(unnamed)".to_string(),
        n => n.to_string(),
    };

    // Interactive picker by default on a real terminal; plain list when piped or
    // when --list is passed, so scripts keep working.
    if list || !std::io::stdout().is_terminal() {
        for d in devices {
            let id = d["device_id"].as_str().unwrap_or("");
            let sig = d["rssi"].as_i64().map(|r| format!("   {r} dBm")).unwrap_or_default();
            println!("  {}{sig}", name_of(d));
            println!("      {id}");
        }
        return Ok(());
    }

    let labels: Vec<String> = devices
        .iter()
        .map(|d| {
            let sig = d["rssi"].as_i64().map(|r| format!("  ·  {r} dBm")).unwrap_or_default();
            format!("{}{sig}", name_of(d))
        })
        .collect();

    let choice = dialoguer::Select::new()
        .with_prompt("Select a treadmill  (↑/↓ to move, Enter to pair, Esc to cancel)")
        .items(&labels)
        .default(0)
        .interact_opt()?;

    match choice {
        Some(i) => {
            let id = devices[i]["device_id"].as_str().unwrap_or("");
            let name = match devices[i]["name"].as_str().unwrap_or("") {
                "" => None,
                n => Some(n.to_string()),
            };
            do_pair(id, name)?;
        }
        None => println!("Cancelled — nothing paired."),
    }
    Ok(())
}

fn cmd_devices() -> Result<()> {
    let v = if live_daemon().is_some() {
        get("/api/devices")?
    } else {
        trot_core::config::init_paths(&data_dir()?);
        let cfg = trot_core::config::load_devices();
        serde_json::json!({
            "active": cfg.active,
            "devices": cfg.devices.iter()
                .map(|d| serde_json::json!({"id": d.id, "name": d.name}))
                .collect::<Vec<_>>(),
        })
    };

    let active = v["active"].as_str();
    let empty = vec![];
    let devices = v["devices"].as_array().unwrap_or(&empty);
    if devices.is_empty() {
        println!("No paired treadmills yet. Run `trot scan`, then `trot pair <id>`.");
        return Ok(());
    }
    for d in devices {
        let id = d["id"].as_str().unwrap_or("");
        let name = match d["name"].as_str().unwrap_or("") {
            "" => "(unnamed)",
            n => n,
        };
        let mark = if Some(id) == active { "* " } else { "  " };
        println!("{mark}{name}");
        println!("      {id}");
    }
    println!("\n* = active (the treadmill the daemon connects to)");
    Ok(())
}

fn cmd_pair(device_id: String, name: Option<String>) -> Result<()> {
    let id = device_id.trim();
    if id.is_empty() {
        anyhow::bail!("device id must not be empty — copy it from `trot scan`");
    }
    do_pair(id, name)
}

/// Pair a device and make it active. Goes through a live daemon (which connects
/// immediately) or writes the config directly when the daemon is down.
fn do_pair(id: &str, name: Option<String>) -> Result<()> {
    let label = name.clone().unwrap_or_else(|| id.to_string());
    if live_daemon().is_some() {
        post("/api/pair", serde_json::json!({ "device_id": id, "name": name }))?;
        println!("Paired \"{label}\". The running daemon is connecting to it now.");
    } else {
        trot_core::config::init_paths(&data_dir()?);
        trot_core::config::add_and_activate(id, name.as_deref());
        println!("Paired \"{label}\" and set as active.");
        println!("Start tracking with:  trot daemon");
    }
    Ok(())
}

fn cmd_unpair() -> Result<()> {
    if live_daemon().is_some() {
        post("/api/unpair", serde_json::json!({}))?;
    } else {
        trot_core::config::init_paths(&data_dir()?);
        if let Some(active) = trot_core::config::active_device_id() {
            trot_core::config::forget(&active);
        }
    }
    println!("Unpaired the active treadmill.");
    Ok(())
}
