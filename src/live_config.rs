// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Live config reload — "The Cosmic Dragon's true Awakening".
//!
//! Watches config.toml for changes, validates strictly, and sends the
//! validated config HashMap to the render thread for a full Cloud rebuild.
//!
//! ## Architecture
//!
//! ```text
//! config.toml → notify watcher thread → mpsc channel → render thread
//!               (parse + validate)      (try_recv/frame)  (rebuild Cloud)
//! ```
//!
//! - Watcher thread: blocks on filesystem events, reparses config on change.
//!   Strict validation — any invalid value rejects the entire config.
//! - Render thread: `try_recv()` each frame; rebuilds Cloud on update.
//!
//! ## Strict validation
//!
//! Uses the same `validate_field_value` rules as `--testconf`. Invalid
//! values reject the entire config with a clear error message.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{event::EventKind, RecommendedWatcher, RecursiveMode, Watcher};

// Polling heartbeat + snapshot dedup live in live_config_poll.rs.
use crate::live_config_poll::{
    env_poll_interval_ms, polling_heartbeat, snapshot_file_state, FileStateSnapshot,
};

use crate::configfile;

/// Global exit code set by live-reload when invalid config is detected.
/// 0 = no error (default), 2 = live-reload validation failure.
/// Main.rs checks this after run_interactive() returns and exits accordingly.
pub static LIVE_RELOAD_EXIT_CODE: AtomicU8 = AtomicU8::new(0);

/// Global error message captured during live-reload failure.
/// Printed to stderr AFTER terminal restoration so the user can see it.
pub static LIVE_RELOAD_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Live config event sent from watcher to render thread.
/// Ok = valid config, rebuild Cloud. Err = invalid, exit cosmostrix.
pub type LiveConfigEvent = Result<HashMap<String, String>, String>;

/// Spawn a config file watcher on a background thread.
///
/// Returns a `Receiver<HashMap<String, String>>` that the render thread polls
/// with `try_recv()` each frame. The watcher validates config strictly before
/// sending — invalid configs are rejected with a stderr error message.
///
/// If the config file doesn't exist or can't be watched, returns `None`.
pub fn spawn_watcher(config_path: PathBuf) -> Option<Receiver<LiveConfigEvent>> {
    if !config_path.exists() {
        lr_trace!(
            "config file does not exist — watcher NOT spawned: {}",
            config_path.display()
        );
        return None;
    }

    lr_trace!("spawning watcher for: {}", config_path.display());

    let (tx, rx) = mpsc::channel::<LiveConfigEvent>();
    let path = config_path.clone();

    let spawn_result = std::thread::Builder::new()
        .name("cosmostrix-config-watcher".to_string())
        .spawn(move || {
            // Catch panics — notify's internal FDs can panic on terminal close.
            lr_trace!("watcher thread started — entering watcher_loop");
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                watcher_loop(path, tx);
            }));
            if let Err(_e) = result {
                // Watcher thread panicked — likely terminal closed.
                lr_trace!("watcher thread PANICKED — setting exit code");
                LIVE_RELOAD_ERROR
                    .lock()
                    .map(|mut guard| {
                        *guard = Some("watcher thread terminated unexpectedly".to_string())
                    })
                    .ok();
                LIVE_RELOAD_EXIT_CODE.store(2, Ordering::Release);
            }
        });

    match spawn_result {
        Ok(_) => lr_trace!("watcher thread spawned successfully — live reload active"),
        Err(e) => {
            lr_trace!("FAILED to spawn watcher thread: {e} — live reload disabled");
            // v25.3: bulletproof write — eprintln! panics on broken stderr.
            use std::io::Write;
            let _ = std::io::stderr().write_fmt(format_args!(
                "[live-reload] FAILED to spawn watcher thread: {e} — live reload disabled\n"
            ));
            return None;
        }
    }

    Some(rx)
}

/// Main watcher loop — blocks on filesystem events, reparses on change.
///
/// HYBRID mode: native `notify` watcher (inotify/kqueue/FSEvents) +
/// polling heartbeat (750ms mtime/size/content-hash check). Both feed
/// the same mpsc channel. Dedup via triple-signal snapshot comparison.
fn watcher_loop(path: PathBuf, tx: Sender<LiveConfigEvent>) {
    const DEBOUNCE_MS: u64 = 200;
    // v25.4: env-configurable poll interval + adaptive burst.
    let poll_interval_ms = env_poll_interval_ms();
    let change_counter = Arc::new(AtomicU64::new(0));

    let (notify_tx, notify_rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();

    // Snapshot initial state to avoid startup reload.
    let initial_state = snapshot_file_state(&path);
    let last_processed_state = Arc::new(Mutex::new(initial_state));

    // Spawn polling heartbeat (recovery loop restarts on panic).
    let poll_path = path.clone();
    let poll_tx = notify_tx.clone();
    let poll_counter = change_counter.clone();
    let poll_spawn_result = std::thread::Builder::new()
        .name("cosmostrix-config-poller".to_string())
        .spawn(move || {
            lr_trace!(
                "polling heartbeat thread started — base_interval={}ms",
                poll_interval_ms
            );
            loop {
                let path_inner = poll_path.clone();
                let tx_inner = poll_tx.clone();
                let counter_inner = poll_counter.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    polling_heartbeat(path_inner, tx_inner, poll_interval_ms, counter_inner);
                }));
                match result {
                    Ok(()) => {
                        lr_trace!("polling heartbeat returned normally — channel closed, exiting");
                        break;
                    }
                    Err(_) => {
                        // Polling heartbeat panicked — back off + restart.
                        lr_trace!("polling heartbeat PANICKED — restarting after 1s backoff");
                        use std::io::Write;
                        let _ = std::io::stderr().write_fmt(format_args!(
                            "[live-reload] polling heartbeat panicked — restarting after 1s backoff\n"
                        ));
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
            }
        });

    match poll_spawn_result {
        Ok(_) => {
            lr_trace!("polling heartbeat thread spawned successfully");
        }
        Err(e) => {
            lr_trace!("FAILED to spawn polling heartbeat: {e} — native watcher only");
            // v25: bulletproof write — eprintln! panics on broken stderr.
            use std::io::Write;
            let _ = std::io::stderr().write_fmt(format_args!(
                "[live-reload] failed to spawn polling heartbeat: {e} — native watcher only\n"
            ));
        }
    }

    // Try to initialize the native watcher. If it fails, log a warning
    // and continue — the polling heartbeat will still detect changes.
    lr_trace!("initializing native RecommendedWatcher");
    let mut watcher: Option<RecommendedWatcher> = match RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            let _ = notify_tx.send(res);
        },
        notify::Config::default(),
    ) {
        Ok(w) => {
            lr_trace!("native RecommendedWatcher created successfully");
            Some(w)
        }
        Err(e) => {
            lr_trace!("native watcher unavailable: {e} — relying on polling heartbeat only");
            use std::io::Write;
            let _ = std::io::stderr().write_fmt(format_args!(
                "[live-reload] native watcher unavailable: {e} — relying on polling heartbeat ({poll_interval_ms}ms base interval)\n"
            ));
            None
        }
    };

    // Watch parent directory to catch atomic-save renames.
    let watch_dir = path
        .parent()
        .map(|p| {
            if p.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                p.to_path_buf()
            }
        })
        .unwrap_or_else(|| PathBuf::from("."));

    if let Some(ref mut w) = watcher {
        lr_trace!(
            "registering native watch on directory: {}",
            watch_dir.display()
        );
        if let Err(e) = w.watch(&watch_dir, RecursiveMode::NonRecursive) {
            lr_trace!("native watch registration FAILED: {e} — polling heartbeat only");
            use std::io::Write;
            let _ = std::io::stderr().write_fmt(format_args!(
                "[live-reload] native watcher failed to register {}: {e} — relying on polling heartbeat\n",
                watch_dir.display()
            ));
            watcher = None;
        } else {
            lr_trace!("native watch registered on: {}", watch_dir.display());
        }
    } else {
        lr_trace!("no native watcher — polling heartbeat is the sole change detector");
    }

    let target_file = Arc::new(path.clone());
    let mut last_event = std::time::Instant::now();

    // v25.5: native watcher liveness diagnostic.
    const NATIVE_SILENCE_WARN_SECS: u64 = 30;
    let loop_start = std::time::Instant::now();
    let mut last_native_event: Option<std::time::Instant> = None;
    let mut native_silence_warned = false;

    let _watcher = watcher;
    lr_trace!("event loop started — waiting for events on notify_rx");
    for event_result in notify_rx.iter() {
        // mpsc iter() returns None only when ALL senders drop. Err here =
        // transient watcher error; `continue` (not `break`) keeps poll alive.
        if event_result.is_err() {
            lr_trace!("watcher Err received: {:?}", event_result.as_ref().err());
            use std::io::Write;
            let _ = std::io::stderr().write_fmt(format_args!(
                "[live-reload] transient watcher error (continuing — polling heartbeat still active): {:?}\n",
                event_result.as_ref().err()
            ));
            continue;
        }
        let event = event_result.as_ref().expect("checked is_err above");
        lr_trace!(
            "event loop received event: kind={:?} paths={:?}",
            event.kind,
            event.paths
        );

        // v25.5: classify event source for liveness diagnostic.
        let is_native_event = matches!(
            event.kind,
            EventKind::Modify(notify::event::ModifyKind::Data(_))
                | EventKind::Modify(notify::event::ModifyKind::Name(_))
                | EventKind::Create(_)
                | EventKind::Remove(_)
        );
        if is_native_event {
            last_native_event = Some(std::time::Instant::now());
            native_silence_warned = false;
        } else if last_native_event.is_none()
            && loop_start.elapsed().as_secs() > NATIVE_SILENCE_WARN_SECS
            && !native_silence_warned
        {
            native_silence_warned = true;
            let elapsed = loop_start.elapsed().as_secs();
            lr_trace!("LIVENESS: native silent {elapsed}s — poll-only (OK)");
            use std::io::Write;
            let _ = std::io::stderr().write_fmt(format_args!(
                "[live-reload] native watcher silent {elapsed}s — polling heartbeat sole detector (informational)\n"
            ));
        }

        if !handle_notify_event(
            event_result,
            &target_file,
            &path,
            &tx,
            &mut last_event,
            DEBOUNCE_MS,
            &last_processed_state,
            &change_counter,
        ) {
            break;
        }
    }
    lr_trace!("watcher_loop exited");
}

/// Process a single notify event. Returns `false` if the channel is closed.
/// Dedup (v25.1): mtime + size + content hash; drops if all three equal
/// `last_processed_state`. Critical on Android Termux where mtime is unreliable.
#[allow(clippy::too_many_arguments)]
fn handle_notify_event(
    event_result: notify::Result<notify::Event>,
    target_file: &Arc<PathBuf>,
    path: &PathBuf,
    tx: &Sender<LiveConfigEvent>,
    last_event: &mut std::time::Instant,
    debounce_ms: u64,
    last_processed_state: &Arc<Mutex<FileStateSnapshot>>,
    change_counter: &Arc<AtomicU64>,
) -> bool {
    match event_result {
        Ok(event) => {
            let touches_target = event.paths.iter().any(|p| p == &**target_file);
            if !touches_target {
                lr_trace!(
                    "event ignored (does not touch target): kind={:?} paths={:?}",
                    event.kind,
                    event.paths
                );
                return true;
            }

            let relevant = matches!(
                event.kind,
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
            );
            if !relevant {
                lr_trace!("event ignored (kind not relevant): {:?}", event.kind);
                return true;
            }

            // Debounce: catch native event bursts (atomic-save = 3-5 events/50ms).
            // Snapshot dedup below catches cross-source duplicates.
            let now = std::time::Instant::now();
            if now.duration_since(*last_event) < Duration::from_millis(debounce_ms) {
                lr_trace!(
                    "event debounced (within {}ms of last): kind={:?}",
                    debounce_ms,
                    event.kind
                );
                return true;
            }
            *last_event = now;

            // Small delay for atomic-save rename completion.
            std::thread::sleep(Duration::from_millis(50));

            // SNAPSHOT DEDUP (v25.1): mtime + size + content hash. Drop if
            // equal to last_processed_state on all three signals.
            let current_state = snapshot_file_state(path);
            if current_state.size.is_none() {
                // File doesn't exist (atomic save in progress). Skip —
                // the next event catches the new file.
                lr_trace!("snapshot: file unreadable — skipping event");
                return true;
            }
            {
                let mut guard = match last_processed_state.lock() {
                    Ok(g) => g,
                    Err(_) => return true,
                };
                if *guard == current_state {
                    // Duplicate event for an already-processed state.
                    // Both the native watcher and polling heartbeat
                    // detected the same change; only the first
                    // should trigger a reload.
                    lr_trace!("snapshot dedup: dropping duplicate for {:?}", current_state);
                    return true;
                }
                *guard = current_state;
            }
            lr_trace!(
                "accepted event for {:?} (kind={:?})",
                current_state,
                event.kind
            );

            // v25.4 strengthening: signal polling heartbeat to enter burst
            // mode (200ms × 5 cycles) to catch rapid follow-up edits.
            change_counter.fetch_add(1, Ordering::AcqRel);

            // Reparse via parse_config_text to catch malformed_lines AND unknown_keys.
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => return true,
            };
            let parsed = configfile::parse_config_text(&content);
            lr_trace!(
                "parsed: {} values, {} malformed, {} unknown",
                parsed.values.len(),
                parsed.malformed_lines.len(),
                parsed.unknown_keys.len()
            );
            if parsed.values.is_empty() && parsed.malformed_lines.is_empty() {
                return true; // empty parse — likely empty/whitespace-only file
            }

            if let Err(msg) = validate_and_send(&parsed, tx) {
                use std::io::Write;
                let _ = std::io::stderr().write_fmt(format_args!("[live-reload] {msg}\n"));
            }
            true
        }
        Err(e) => {
            use std::io::Write;
            let _ = std::io::stderr().write_fmt(format_args!("[live-reload] watch error: {e}\n"));
            true
        }
    }
}

/// Validate parsed config strictly, then send Ok(cfg) or Err(msg) to the
/// render thread. Returns Err(msg) if validation failed (caller logs it).
fn validate_and_send(
    parsed: &configfile::ParsedConfig,
    tx: &Sender<LiveConfigEvent>,
) -> Result<(), String> {
    // Check malformed lines first — these are syntax errors.
    if !parsed.malformed_lines.is_empty() {
        let lines: Vec<&str> = parsed
            .malformed_lines
            .iter()
            .take(3)
            .map(String::as_str)
            .collect();
        let msg = format!(
            "malformed line(s): '{}' (expected 'key = value' syntax)",
            lines.join(", ")
        );
        let _ = tx.send(Err(msg.clone()));
        return Err(msg);
    }

    // Check unknown keys.
    if !parsed.unknown_keys.is_empty() {
        let keys: Vec<&str> = parsed
            .unknown_keys
            .iter()
            .take(3)
            .map(String::as_str)
            .collect();
        // v25.6 depth-test fix: append "did you mean" hints for structural
        // mistakes (e.g. color.tune.bold). Returns "" when no hints apply.
        let hints = crate::config_hints::format_hints_block(&parsed.unknown_keys);
        let msg = format!(
            "unknown key(s): '{}' (run 'cosmostrix --testconf' for known keys){hints}",
            keys.join(", ")
        );
        let _ = tx.send(Err(msg.clone()));
        return Err(msg);
    }

    let cfg = &parsed.values;

    // Strict validation: reject entire config if ANY field is invalid.
    match crate::testconf::validate_config_strictly(cfg) {
        Ok(()) => {
            lr_trace!("strict validation OK — sending config to render thread");
            if tx.send(Ok(cfg.clone())).is_err() {
                lr_trace!("channel closed during send (Ok)");
                return Ok(());
            }
            Ok(())
        }
        Err(msg) => {
            lr_trace!("strict validation FAILED: {msg}");
            let _ = tx.send(Err(msg.clone()));
            Err(msg)
        }
    }
}

/// Rebuild a CloudConfig from base + new config values.
/// CLI-only fields are preserved from base. Config values override CLI
/// defaults during live reload. Per-field CLI flags (tracked in
/// `cli_explicit`) remain immutable across reloads.
#[must_use]
pub fn rebuild_cloud_config(
    base: &crate::app::CloudConfig,
    cfg: &HashMap<String, String>,
) -> crate::app::CloudConfig {
    let mut new = base.clone();
    // Snapshot CLI-explicit tracker — preserved across reloads.
    let cli = new.cli_explicit.clone();

    lr_trace!(
        "rebuild_cloud_config: cli_explicit = {{color:{}, charset:{}, speed:{}, density:{}, fps:{}, scene:{}, glitch:{}}}",
        cli.color, cli.charset, cli.speed, cli.density, cli.fps, cli.scene, cli.glitch_level
    );

    // v25.5 depth-test fix: user-set color/charset must win over scene defaults.
    let user_set_color = cfg.contains_key("color");
    let user_set_charset = cfg.contains_key("charset");

    // Color scheme — skip if CLI --color was explicit.
    if !cli.color {
        if let Some(v) = cfg.get("color") {
            if let Ok(scheme) = crate::cli::parse_color_scheme(v) {
                lr_trace!("apply color='{}' -> {:?}", v, scheme);
                new.color_scheme = scheme;
            } else {
                lr_trace!(
                    "color='{}' failed to parse — keeping {:?}",
                    v,
                    new.color_scheme
                );
            }
        }
    } else {
        lr_trace!("skip color (CLI explicit) — keeping {:?}", new.color_scheme);
    }

    // v16: Custom color palette live reload (if active at startup).
    if let Some(ref name) = new.custom_palette_name {
        match crate::colors_custom::load_custom_palette(cfg, name) {
            Ok(palette) => {
                lr_trace!(
                    "reloaded custom palette '{}': {} stops",
                    name,
                    palette.colors.len()
                );
                new.custom_palette = Some(palette);
            }
            Err(e) => {
                lr_trace!(
                    "custom palette '{}' reload failed (keeping old): {}",
                    name,
                    e
                );
            }
        }
    }

    // Charset — skip if CLI --charset
    if !cli.charset {
        if let Some(v) = cfg.get("charset") {
            // v25: charset-custom.<name> takes precedence over built-in.
            if let Some(custom_chars) =
                crate::charset_custom::load_custom_charset_if_matches(cfg, v)
            {
                lr_trace!(
                    "apply charset='{}' (custom, {} chars)",
                    v,
                    custom_chars.len()
                );
                new.charset_preset = v.clone();
                new.chars = custom_chars;
            } else if let Ok(charset) = crate::charset::charset_from_str(v, false) {
                lr_trace!("apply charset='{}' (built-in)", v);
                new.charset_preset = v.clone();
                new.chars = crate::charset::build_chars(charset, &new.user_ranges, new.def_ascii);
            } else {
                lr_trace!(
                    "charset='{}' failed to parse — keeping '{}'",
                    v,
                    new.charset_preset
                );
            }
        }
    } else {
        lr_trace!(
            "skip charset (CLI explicit) — keeping '{}'",
            new.charset_preset
        );
    }

    // Scene — skip if CLI --scene explicit. v25.5: scene color/charset are
    // defaults; user config values win.
    if !cli.scene {
        if let Some(v) = cfg.get("scene") {
            if let Some(scene_info) = crate::scene::get_scene(v) {
                new.rain_style = scene_info.config.rain_style;
                if let Some(color) = scene_info.config.color {
                    if !cli.color && !user_set_color {
                        if let Ok(scheme) = crate::cli::parse_color_scheme(color) {
                            lr_trace!("scene '{}' applies default color={:?}", v, scheme);
                            new.color_scheme = scheme;
                        }
                    } else {
                        lr_trace!("scene '{}' color skipped — user/CLI set", v);
                    }
                }
                if let Some(charset_name) = scene_info.config.charset {
                    if !cli.charset && !user_set_charset {
                        if let Some(custom_chars) =
                            crate::charset_custom::load_custom_charset_if_matches(cfg, charset_name)
                        {
                            lr_trace!(
                                "scene '{}' applies default charset='{}' (custom)",
                                v,
                                charset_name
                            );
                            new.charset_preset = charset_name.to_string();
                            new.chars = custom_chars;
                        } else if let Ok(charset) =
                            crate::charset::charset_from_str(charset_name, false)
                        {
                            lr_trace!(
                                "scene '{}' applies default charset='{}' (built-in)",
                                v,
                                charset_name
                            );
                            new.charset_preset = charset_name.to_string();
                            new.chars = crate::charset::build_chars(
                                charset,
                                &new.user_ranges,
                                new.def_ascii,
                            );
                        }
                    } else {
                        lr_trace!("scene '{}' charset skipped — user/CLI set", v);
                    }
                }
                if let Some(speed) = scene_info.config.speed {
                    if !cli.speed {
                        new.speed = speed;
                    }
                }
                if let Some(density) = scene_info.config.density {
                    if !cli.density {
                        new.density = density;
                        new.base_density = density;
                    }
                }
            }
        }
    }

    // Speed — skip if CLI --speed was explicit
    if !cli.speed {
        if let Some(v) = cfg.get("speed") {
            if let Ok(n) = crate::validation::parse_canonical_speed("speed", v) {
                lr_trace!("apply speed='{}' -> {}", v, n);
                new.speed = n;
            } else {
                lr_trace!("speed='{}' failed to parse — keeping {}", v, new.speed);
            }
        }
    } else {
        lr_trace!("skip speed (CLI explicit) — keeping {}", new.speed);
    }

    // Density — skip if CLI --density was explicit
    if !cli.density {
        if let Some(v) = cfg.get("density") {
            if let Ok(n) = crate::validation::parse_canonical_f32_range("density", v, 0.01, 5.0) {
                lr_trace!("apply density='{}' -> {}", v, n);
                new.density = n;
                new.base_density = n;
            } else {
                lr_trace!("density='{}' failed to parse — keeping {}", v, new.density);
            }
        }
    } else {
        lr_trace!("skip density (CLI explicit) — keeping {}", new.density);
    }

    // FPS — skip if CLI --fps was explicit
    if !cli.fps {
        if let Some(v) = cfg.get("fps") {
            if let Ok(n) = crate::validation::parse_canonical_f64_range("fps", v, 1.0, 240.0) {
                lr_trace!("apply fps='{}' -> {}", v, n);
                new.target_fps = n;
            } else {
                lr_trace!("fps='{}' failed to parse — keeping {}", v, new.target_fps);
            }
        }
    } else {
        lr_trace!("skip fps (CLI explicit) — keeping {}", new.target_fps);
    }

    // Glitch level — skip if CLI --glitch-level was explicit
    if !cli.glitch_level {
        if let Some(v) = cfg.get("glitch-level") {
            lr_trace!("apply glitch-level='{}'", v);
            new.noglitch = v.trim().eq_ignore_ascii_case("none");
        }
    } else {
        lr_trace!(
            "skip glitch-level (CLI explicit) — noglitch={}",
            new.noglitch
        );
    }

    // v25.5: color-bg live reload (true = terminal default; false = solid black).
    if let Some(v) = cfg.get("color-bg") {
        new.default_bg = match v.trim().to_ascii_lowercase().as_str() {
            "black" => false,
            "default-background" | "default_background" => true,
            _ => new.default_bg,
        };
        lr_trace!("apply color-bg='{}' → default_bg={}", v, new.default_bg);
    }

    // Glitch times (ms range)
    if let Some(v) = cfg.get("glitchms") {
        if let Some((lo, hi)) = parse_range(v) {
            new.glitch_low = lo;
            new.glitch_high = hi;
        }
    }
    // Linger times
    if let Some(v) = cfg.get("lingerms") {
        if let Some((lo, hi)) = parse_range(v) {
            new.linger_low = lo;
            new.linger_high = hi;
        }
    }

    // Monolith size
    if let Some(v) = cfg.get("monolith-size") {
        use clap::ValueEnum;
        if let Ok(size) = crate::runtime::MonolithSize::from_str(v, true) {
            new.monolith_size = size;
        }
    }

    // Auto color drift
    if let Some(v) = cfg.get("auto-color-drift") {
        new.auto_color_drift = v.trim() == "true";
    }

    // v20: scene-custom live reload. If a custom scene is active (set via
    // --scene-custom <name>), re-apply its fields from the new config so
    // editing [scene-custom.<name>] takes effect immediately.
    if let Some(ref custom_name) = base.scene_custom_name {
        apply_scene_custom_to_cloud_config(&mut new, cfg, custom_name);
    }

    new
}

/// Apply a `[scene-custom.<name>]` block from config to a CloudConfig in
/// place. Used by live reload so edits to a custom scene take effect
/// without restarting. Missing fields are left unchanged.
fn apply_scene_custom_to_cloud_config(
    new: &mut crate::app::CloudConfig,
    cfg: &HashMap<String, String>,
    name: &str,
) {
    use clap::ValueEnum;

    let normalized = name.trim().to_ascii_lowercase();
    let prefix = format!("scene-custom.{normalized}.");
    let mut touched_any = false;

    for (key, value) in cfg {
        let Some(field) = key.strip_prefix(&prefix) else {
            continue;
        };
        // v20.1: base-scene and preset filtered upstream; unknown fields ignored.
        touched_any = true;
        match field {
            "color" => {
                if let Ok(scheme) = crate::cli::parse_color_scheme(value) {
                    new.color_scheme = scheme;
                }
            }
            "charset" => {
                // v25: scene-custom blocks may reference a custom charset
                // name (charset-custom.<name>). Check custom first, then
                // fall back to built-in presets.
                if let Some(custom_chars) =
                    crate::charset_custom::load_custom_charset_if_matches(cfg, value)
                {
                    new.charset_preset = value.clone();
                    new.chars = custom_chars;
                } else if let Ok(charset) = crate::charset::charset_from_str(value, false) {
                    new.charset_preset = value.clone();
                    new.chars =
                        crate::charset::build_chars(charset, &new.user_ranges, new.def_ascii);
                }
            }
            "fps" => {
                if let Ok(n) =
                    crate::validation::parse_canonical_f64_range("fps", value, 1.0, 240.0)
                {
                    new.target_fps = n;
                }
            }
            "speed" => {
                if let Ok(n) = crate::validation::parse_canonical_speed("speed", value) {
                    new.speed = n;
                }
            }
            "density" => {
                if let Ok(n) =
                    crate::validation::parse_canonical_f32_range("density", value, 0.01, 5.0)
                {
                    new.density = n;
                    new.base_density = n;
                }
            }
            "glitch-level" => {
                new.noglitch = value.trim().eq_ignore_ascii_case("none");
            }
            "monolith-size" => {
                if let Ok(size) = crate::runtime::MonolithSize::from_str(value, true) {
                    new.monolith_size = size;
                }
            }
            "density-map" => {
                // Re-parse density map from new config. parse_density_map
                // leaks the Vec so it lives for the process lifetime — this
                // is intentional (bounded by config size, a few KB total).
                if let Some(map) = crate::scene_custom::parse_density_map(value) {
                    new.monolith_density_map = Some(map);
                }
            }
            // color-bg / atmosphere-mode / atmosphere-regime are not yet
            // applied to CloudConfig at runtime; they remain as-is from the
            // startup-time resolution. Future work can wire them in.
            "color-bg" | "atmosphere-mode" | "atmosphere-regime" => {}
            _ => {}
        }
    }

    if touched_any {
        // The scene_name stays as the custom scene name (already set at
        // startup). No need to re-assign — custom scenes are first-class
        // citizens and their identity doesn't change on live reload.
        // v25: bulletproof write — runs in watcher worker thread.
        use std::io::Write;
        let _ = std::io::stderr().write_fmt(format_args!(
            "[live-reload] scene-custom '{normalized}': re-applied fields from config\n"
        ));
    }
}

/// Parse "LOW,HIGH" range string.
fn parse_range(s: &str) -> Option<(u16, u16)> {
    let (lo, hi) = s.split_once(',')?;
    let lo: u16 = lo.trim().parse().ok()?;
    let hi: u16 = hi.trim().parse().ok()?;
    Some((lo.min(hi), lo.max(hi)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_invalid_speed() {
        let mut cfg = HashMap::new();
        cfg.insert("speed".to_string(), "100000".to_string());
        let result = crate::testconf::validate_config_strictly(&cfg);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("speed"));
    }

    #[test]
    fn validate_rejects_invalid_density() {
        let mut cfg = HashMap::new();
        cfg.insert("density".to_string(), "99.0".to_string());
        let result = crate::testconf::validate_config_strictly(&cfg);
        assert!(result.is_err());
    }

    #[test]
    fn validate_accepts_valid_config() {
        let mut cfg = HashMap::new();
        cfg.insert("speed".to_string(), "30".to_string());
        cfg.insert("density".to_string(), "0.85".to_string());
        cfg.insert("fps".to_string(), "60".to_string());
        let result = crate::testconf::validate_config_strictly(&cfg);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_skips_block_keys() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "scene-custom.test.base-scene".to_string(),
            "monolith".to_string(),
        );
        cfg.insert("speed".to_string(), "30".to_string());
        let result = crate::testconf::validate_config_strictly(&cfg);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_rejects_invalid_charset() {
        let mut cfg = HashMap::new();
        cfg.insert("charset".to_string(), "hackeres".to_string());
        let result = crate::testconf::validate_config_strictly(&cfg);
        assert!(result.is_err());
    }

    #[test]
    fn validate_rejects_invalid_atmosphere_regime() {
        let mut cfg = HashMap::new();
        cfg.insert("atmosphere-regime".to_string(), "adaptivee".to_string());
        let result = crate::testconf::validate_config_strictly(&cfg);
        assert!(result.is_err());
    }

    #[test]
    fn parse_range_handles_whitespace() {
        assert_eq!(parse_range(" 200 , 300 "), Some((200, 300)));
        assert_eq!(parse_range("300,200"), Some((200, 300)));
    }

    #[test]
    fn parse_range_rejects_invalid() {
        assert_eq!(parse_range("abc"), None);
        assert_eq!(parse_range("200"), None);
    }

    // ── v25.1 Termux fix: triple-signal change detection tests live
    // in `live_config_poll::tests` (same module as the implementation).
    // The split keeps this file under the 1200-LOC source cap.

    // ── v20: scene-custom live reload tests ──

    /// Build a minimal CloudConfig for testing rebuild_cloud_config.
    fn minimal_cloud_config() -> crate::app::CloudConfig {
        use crate::atmosphere_apply::{AtmosphereApplicationMode, AtmosphereRuntimeModulation};
        use crate::rain_style::RainStyle;
        use crate::runtime::{BoldMode, ColorMode, ColorScheme, MonolithSize, ShadingMode};

        crate::app::CloudConfig {
            color_mode: ColorMode::TrueColor,
            fullwidth: false,
            shading_mode: ShadingMode::Random,
            bold_mode: BoldMode::Random,
            async_mode: true,
            default_bg: true,
            color_scheme: ColorScheme::NeonPurple,
            custom_palette: None,
            custom_palette_name: None,
            rain_style: RainStyle::Glyph,
            noglitch: false,
            glitch_pct: 10.0,
            glitch_low: 300,
            glitch_high: 400,
            linger_low: 400,
            linger_high: 600,
            short_pct: 50.0,
            die_early_pct: 33.0,
            max_dpc: 5,
            density: 0.75,
            speed: 9.0,
            monolith_size: MonolithSize::Normal,
            chars: vec!['0', '1'],
            message: None,
            message_border: false,
            target_fps: 60.0,
            duration: None,
            duration_s: None,
            bench_frames: None,
            benchmark: false,
            bench_duration: None,
            screen_size: None,
            color_tune: crate::color_tune::ColorTune::IDENTITY,
            json: false,
            save_baseline: None,
            compare_baseline: None,
            bench_io: false,
            bench_all: false,
            verbose: false,
            density_auto: true,
            base_density: 0.75,
            perf_stats: false,
            screensaver: false,
            intro: crate::config::IntroType::None,
            mouse: false,
            charset_preset: "binary".to_string(),
            user_ranges: vec![],
            def_ascii: false,
            auto_color_drift: false,
            atmosphere_modulation: AtmosphereRuntimeModulation::identity(),
            atmosphere_mode: AtmosphereApplicationMode::Disabled,
            monolith_density_map: None,
            config_path_for_watcher: None,
            scene_name: "test-scene".to_string(),
            scene_custom_name: Some("test-scene".to_string()),
            cli_explicit: crate::app::CliExplicit::default(),
        }
    }

    #[test]
    fn rebuild_applies_scene_custom_color_change() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "scene-custom.test-scene.color".to_string(),
            "green".to_string(),
        );
        let base = minimal_cloud_config();
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(new.color_scheme, crate::runtime::ColorScheme::Green);
        assert_eq!(new.scene_name, "test-scene");
    }

    /// v25.5: user color wins over scene default (depth-test bug fix).
    #[test]
    fn rebuild_user_color_wins_over_scene_default() {
        let mut cfg = HashMap::new();
        cfg.insert("color".to_string(), "cosmos".to_string());
        cfg.insert("scene".to_string(), "carbonic".to_string());
        let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
        assert_eq!(new.color_scheme, crate::runtime::ColorScheme::Cosmos);
    }

    /// v25.5: user charset wins over scene default (depth-test bug fix).
    #[test]
    fn rebuild_user_charset_wins_over_scene_default() {
        let mut cfg = HashMap::new();
        cfg.insert("charset".to_string(), "retro".to_string());
        cfg.insert("scene".to_string(), "carbonic".to_string());
        let new = rebuild_cloud_config(&minimal_cloud_config(), &cfg);
        assert_eq!(new.charset_preset, "retro");
    }

    /// v25.5: color-bg live reload (was startup-only — depth-test bug fix).
    #[test]
    fn rebuild_applies_color_bg_live_reload() {
        let base = minimal_cloud_config();
        assert!(base.default_bg);
        let mut cfg = HashMap::new();
        cfg.insert("color-bg".to_string(), "black".to_string());
        assert!(
            !rebuild_cloud_config(&base, &cfg).default_bg,
            "black → solid black"
        );
        let mut cfg2 = HashMap::new();
        cfg2.insert("color-bg".to_string(), "default-background".to_string());
        assert!(
            rebuild_cloud_config(&base, &cfg2).default_bg,
            "default-background → terminal default"
        );
    }

    /// v25.5: unrecognized color-bg keeps old setting.
    #[test]
    fn rebuild_color_bg_unrecognized_keeps_old() {
        let base = minimal_cloud_config();
        let mut cfg = HashMap::new();
        cfg.insert("color-bg".to_string(), "purple".to_string());
        assert_eq!(
            rebuild_cloud_config(&base, &cfg).default_bg,
            base.default_bg
        );
    }

    #[test]
    fn rebuild_applies_scene_custom_speed_and_density_changes() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "scene-custom.test-scene.speed".to_string(),
            "24".to_string(),
        );
        cfg.insert(
            "scene-custom.test-scene.density".to_string(),
            "0.50".to_string(),
        );
        let base = minimal_cloud_config();
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(new.speed, 24.0);
        assert!((new.density - 0.50).abs() < f32::EPSILON);
        assert!((new.base_density - 0.50).abs() < f32::EPSILON);
    }

    #[test]
    fn rebuild_applies_scene_custom_density_map_change() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "scene-custom.test-scene.density-map".to_string(),
            "1.0,0.5,0.0,0.8".to_string(),
        );
        let base = minimal_cloud_config();
        let new = rebuild_cloud_config(&base, &cfg);
        let map = new
            .monolith_density_map
            .expect("density-map must be parsed and applied");
        assert_eq!(map.len(), 4);
        assert_eq!(map[0], 1.0);
        assert_eq!(map[2], 0.0);
    }

    #[test]
    fn rebuild_ignores_scene_custom_base_scene_unknown_field() {
        // v20.1: `base-scene` is no longer a recognized field. The
        // apply_scene_custom_to_cloud_config helper iterates raw cfg keys
        // (including unknown ones) but only acts on known fields, so the
        // color_scheme should remain the base's NeonPurple.
        let mut cfg = HashMap::new();
        cfg.insert(
            "scene-custom.test-scene.base-scene".to_string(),
            "storm".to_string(),
        );
        let mut base = minimal_cloud_config();
        base.scene_custom_name = Some("test-scene".to_string());
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(
            new.color_scheme,
            crate::runtime::ColorScheme::NeonPurple,
            "base-scene must have no effect"
        );
    }

    #[test]
    fn rebuild_without_scene_custom_name_does_not_apply_custom_fields() {
        // When scene_custom_name is None (no --scene-custom active), the
        // scene-custom.* keys in config must NOT be applied — they belong
        // to a different scene and could clobber the active one.
        let mut cfg = HashMap::new();
        cfg.insert(
            "scene-custom.other-scene.color".to_string(),
            "green".to_string(),
        );
        let mut base = minimal_cloud_config();
        base.scene_custom_name = None;
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(
            new.color_scheme,
            crate::runtime::ColorScheme::NeonPurple,
            "scene-custom fields must not apply when no custom scene is active"
        );
    }

    /// Bug 3 test: CLI-explicit color must NOT be overridden by config.toml
    /// during live reload. The priority contract is CLI > config.toml > scene.
    /// Without the `cli_explicit` tracker, `rebuild_cloud_config` would
    /// blindly apply `color = "snow"` from config, clobbering the user's
    /// `-c green` CLI flag.
    #[test]
    fn rebuild_preserves_cli_explicit_color_over_config() {
        let mut cfg = HashMap::new();
        cfg.insert("color".to_string(), "snow".to_string());
        let mut base = minimal_cloud_config();
        // Simulate the user running `cosmostrix -c green`: the CLI flag
        // is recorded as explicit, and the color_scheme is set to Green.
        base.cli_explicit.color = true;
        base.color_scheme = crate::runtime::ColorScheme::Green;
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(
            new.color_scheme,
            crate::runtime::ColorScheme::Green,
            "CLI --color green must NOT be overridden by config.toml color=snow"
        );
    }

    /// Bug 3 test: when CLI did NOT explicitly set a field, config.toml
    /// overrides scene defaults. This is the normal live-reload path.
    #[test]
    fn rebuild_applies_config_color_when_cli_not_explicit() {
        let mut cfg = HashMap::new();
        cfg.insert("color".to_string(), "snow".to_string());
        let base = minimal_cloud_config();
        // base.cli_explicit.color is false (default) — no CLI override.
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(
            new.color_scheme,
            crate::runtime::ColorScheme::Snow,
            "config.toml color=snow must apply when CLI did not set --color"
        );
    }

    /// Bug 3 test: CLI-explicit speed must NOT be overridden by scene's
    /// speed default during live reload (e.g., user runs `cosmostrix -s 25`,
    /// config.toml has `scene = "matrix"` which sets speed=18 — CLI wins).
    #[test]
    fn rebuild_preserves_cli_explicit_speed_over_scene() {
        let mut cfg = HashMap::new();
        cfg.insert("scene".to_string(), "matrix".to_string());
        let mut base = minimal_cloud_config();
        base.cli_explicit.speed = true;
        base.cli_explicit.scene = false;
        base.speed = 25.0;
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(new.speed, 25.0, "CLI --speed wins over scene default");
    }

    /// v25.6 FIX D: validate_and_send returns Err on bad config, but the
    /// render thread NO LONGER sets LIVE_RELOAD_EXIT_CODE — only true
    /// watcher-thread panics do. Verifies Err(msg) is returned AND exit
    /// code remains 0 (caller stores in LIVE_RELOAD_ERROR, doesn't
    /// escalate). v25.6 FIX E: the error also includes a targeted hint
    /// explaining `bold` is top-level, not a [color.tune] field.
    #[test]
    fn validate_and_send_returns_err_without_setting_exit_code() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut parsed = configfile::ParsedConfig::default();
        parsed.unknown_keys.push("color.tune.bold".to_string());
        let result = validate_and_send(&parsed, &tx);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("color.tune.bold"));
        assert!(msg.contains("top-level"), "need structural hint: {msg}");
        assert!(msg.contains("[color.tune]"), "need section ref: {msg}");
        assert_eq!(LIVE_RELOAD_EXIT_CODE.load(Ordering::Acquire), 0);
    }
}
