// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Live config reload — "The Dragon's true Awakening".
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
//!   Validates EVERY field strictly — any invalid value rejects the entire
//!   config (no partial apply). On error, logs to stderr and keeps old config.
//! - Render thread: `try_recv()` each frame (~1ns on empty channel).
//!   If update pending, rebuilds CloudConfig from base + new config values,
//!   then rebuilds Cloud (full create_cloud + reset). Visual state resets
//!   (rain streams restart) but color/charset/scene changes take effect.
//!
//! ## Strict validation
//!
//! Uses the same `validate_field_value` rules as `--testconf`. If ANY field
//! has an invalid value (e.g. `speed = 100000`), the entire config is
//! rejected with a clear error message. No silent fallback.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{event::EventKind, RecommendedWatcher, RecursiveMode, Watcher};

// v25.1 Termux fix: polling heartbeat + snapshot dedup live in
// live_config_poll.rs (split out so this file stays under the 1200-LOC
// source cap enforced by loc_tests). The triple-signal change detection
// (mtime + size + content hash) handles Android Termux's unreliable
// FUSE mtime without losing dedup correctness.
use crate::live_config_poll::{polling_heartbeat, snapshot_file_state, FileStateSnapshot};

use crate::configfile;

/// Global exit code set by live-reload when invalid config is detected.
/// 0 = no error (default), 2 = live-reload validation failure.
/// Main.rs checks this after run_interactive() returns and exits accordingly.
pub static LIVE_RELOAD_EXIT_CODE: AtomicU8 = AtomicU8::new(0);

// v25: opt-in debug tracing lives in `live_config_trace.rs` (split out so
// this file stays under the 1200-LOC source cap enforced by loc_tests).
// The `lr_trace!` macro is brought into scope by `#[macro_use]` on the
// `mod live_config_trace;` declaration in main.rs.

/// Global error message captured during live-reload failure.
/// Printed to stderr AFTER terminal restoration (in main.rs) so the user
/// can actually see it — printing during alternate-screen mode swallows
/// the output.
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
        return None;
    }

    let (tx, rx) = mpsc::channel::<LiveConfigEvent>();
    let path = config_path.clone();

    std::thread::Builder::new()
        .name("cosmostrix-config-watcher".to_string())
        .spawn(move || {
            // Catch panics in the watcher thread to prevent core dumps.
            // When the terminal is closed, notify's internal inotify/kqueue
            // file descriptors become invalid (EIO), which can trigger panics
            // in notify's internal thread. We catch and convert to graceful shutdown.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                watcher_loop(path, tx);
            }));
            if let Err(_e) = result {
                // Watcher thread panicked — likely terminal closed.
                // Set a flag so the main loop detects the failure and exits.
                LIVE_RELOAD_ERROR
                    .lock()
                    .map(|mut guard| {
                        *guard = Some("watcher thread terminated unexpectedly".to_string())
                    })
                    .ok();
                LIVE_RELOAD_EXIT_CODE.store(2, Ordering::Release);
            }
        })
        .ok()?;

    Some(rx)
}

/// Main watcher loop — blocks on filesystem events, reparses on change.
///
/// Bug 1 fix (v25): HYBRID mode. The native `notify` watcher
/// (kqueue on BSDs, inotify on Linux, FSEvents on macOS,
/// ReadDirectoryChanges on Windows) is the primary event source. A
/// polling heartbeat runs IN PARALLEL on a separate thread, checking
/// the file's mtime every 2 seconds. Both paths feed into the same
/// mpsc channel, so a change is detected via whichever fires first.
///
/// This fixes the FreeBSD silent-failure case: if `RecommendedWatcher`
/// initializes successfully (no error) but the backend produces no
/// events (e.g., kqueue feature not active, restricted container),
/// the polling heartbeat still detects mtime changes and triggers
/// live reload. The previous "either/or" fallback only kicked in when
/// watcher creation FAILED, missing the silent-no-events case.
///
/// When the native watcher fails to initialize OR fails to register
/// the watch path, we still log a warning — but the polling heartbeat
/// runs unconditionally, so live reload always works.
///
/// **Dedup mechanism (Task 1 fix)**: when both the native watcher
/// and the polling heartbeat detect the same file modification, the
/// unified event loop must process it only ONCE. We track the last
/// processed mtime in `last_processed_mtime`. Before processing any
/// event, we read the file's current mtime; if it equals
/// `last_processed_mtime`, the event is a duplicate (the other source
/// already processed this mtime) and is silently dropped. This also
/// skips the startup reload — `last_processed_mtime` is initialized
/// to the file's current mtime at loop start, so no event for the
/// initial state fires.
fn watcher_loop(path: PathBuf, tx: Sender<LiveConfigEvent>) {
    // Debounce window. The time-based debounce catches bursts of native
    // events (atomic-save produces 3-5 events within 50ms); the mtime +
    // content-hash check catches cross-source duplicates (native + polling
    // for the same change).
    const DEBOUNCE_MS: u64 = 200;

    // v25.1 Termux fix: polling interval reduced from 2000ms to 750ms.
    // The previous 2s interval made live reload feel broken on platforms
    // where the native watcher is unreliable (Android/Termux inotify can
    // silently drop events under SELinux pressure or app-standby
    // throttling). With 750ms polling, a config save is detected in under
    // a second even when inotify is dead. The mtime+hash dedup in
    // handle_notify_event prevents duplicate processing.
    const POLL_INTERVAL_MS: u64 = 750;

    let (notify_tx, notify_rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();

    // Initialize last_processed_state to the file's current state.
    // This ensures the polling heartbeat's first check (750ms after start)
    // does NOT trigger an unnecessary startup reload — neither mtime,
    // size, nor content hash has changed since we initialized the tracker.
    let initial_state = snapshot_file_state(&path);
    let last_processed_state = Arc::new(Mutex::new(initial_state));

    // Spawn the polling heartbeat on a background thread. It feeds
    // synthetic events into notify_tx when mtime/size/content changes,
    // so the unified event loop below handles them identically to native
    // events. This guarantees detection even when the native watcher
    // is silent (FreeBSD kqueue edge case, Android Termux inotify
    // throttling, restricted containers).
    //
    // v25.1 Termux fix: the polling thread is now wrapped in an outer
    // recovery loop that restarts `polling_heartbeat` if it panics.
    // Previously, a single panic (e.g., from a transient I/O error
    // during catch_unwind's drop glue) would silently kill the polling
    // thread, leaving only the native watcher — which is exactly the
    // unreliable component on Termux. The recovery loop runs forever
    // (until the channel closes), restarting with a 1s backoff.
    let poll_path = path.clone();
    let poll_tx = notify_tx.clone();
    if let Err(e) = std::thread::Builder::new()
        .name("cosmostrix-config-poller".to_string())
        .spawn(move || {
            loop {
                let path_inner = poll_path.clone();
                let tx_inner = poll_tx.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    polling_heartbeat(path_inner, tx_inner, POLL_INTERVAL_MS);
                }));
                match result {
                    Ok(()) => {
                        // polling_heartbeat returned normally — this only
                        // happens when the channel closed (tx.send err).
                        // No point restarting; exit the recovery loop.
                        lr_trace!("polling heartbeat returned normally — channel closed, exiting recovery loop");
                        break;
                    }
                    Err(_) => {
                        // Polling heartbeat panicked. Log and back off
                        // before restarting. Bulletproof write — eprintln!
                        // panics on broken stderr.
                        use std::io::Write;
                        let _ = std::io::stderr().write_fmt(format_args!(
                            "[live-reload] polling heartbeat panicked — restarting after 1s backoff\n"
                        ));
                        std::thread::sleep(Duration::from_secs(1));
                    }
                }
            }
        })
    {
        // v25: bulletproof write — eprintln! panics on broken stderr,
        // which would fire the panic hook (catch_unwind catches after,
        // but bulletproof write avoids the panic entirely).
        use std::io::Write;
        let _ = std::io::stderr().write_fmt(format_args!(
            "[live-reload] failed to spawn polling heartbeat: {e} — native watcher only\n"
        ));
    }

    // Try to initialize the native watcher. If it fails, log a warning
    // and continue — the polling heartbeat will still detect changes.
    let mut watcher: Option<RecommendedWatcher> = match RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            let _ = notify_tx.send(res);
        },
        notify::Config::default(),
    ) {
        Ok(w) => Some(w),
        Err(e) => {
            // v25: bulletproof write (see spawn_watcher above).
            use std::io::Write;
            let _ = std::io::stderr().write_fmt(format_args!(
                "[live-reload] native watcher unavailable: {e} — relying on polling heartbeat ({POLL_INTERVAL_MS}ms interval)\n"
            ));
            None
        }
    };

    // Watch the parent directory to catch atomic-save renames.
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
        if let Err(e) = w.watch(&watch_dir, RecursiveMode::NonRecursive) {
            // v25: bulletproof write (see spawn_watcher above).
            use std::io::Write;
            let _ = std::io::stderr().write_fmt(format_args!(
                "[live-reload] native watcher failed to register {}: {e} — relying on polling heartbeat\n",
                watch_dir.display()
            ));
            // Drop the broken watcher so it doesn't hold resources.
            watcher = None;
        }
    }

    let target_file = Arc::new(path.clone());
    let mut last_event = std::time::Instant::now();

    // Unified event loop: processes BOTH native events AND synthetic
    // polling events. The dedup logic in handle_notify_event ensures
    // that when both sources detect the same mtime, only the first
    // event triggers a reload.
    let _watcher = watcher;
    for event_result in notify_rx.iter() {
        // notify_rx.iter() yields notify::Result<notify::Event>:
        //   Ok(Event) = filesystem event (create/modify/remove)
        //   Err(Error) = watcher error (e.g., watch removed, OS error)
        // mpsc's iter() returns None (ending the loop) only when ALL
        // senders drop — that's the channel-close case, not Err.
        //
        // v25 fix (Task 1): on Err, `continue` instead of `break`.
        // The previous `break` was the root cause of "live reload silently
        // stops working after some time" on Linux/FreeBSD/macOS. A single
        // transient watcher Err (kqueue/inotify resync, OS buffer overflow,
        // race on file rename) would terminate the consumer loop. The
        // polling heartbeat thread keeps producing synthetic events into
        // the channel, but no one reads them — the channel buffer grows
        // unbounded and the polling thread eventually observes send-Err
        // (receiver dropped) and exits. Net result: live reload is dead
        // even though the polling thread was nominally still running.
        //
        // The fix: log the Err and keep consuming. The polling heartbeat
        // (which is the more reliable source) continues to drive reloads.
        if event_result.is_err() {
            use std::io::Write;
            let _ = std::io::stderr().write_fmt(format_args!(
                "[live-reload] transient watcher error (continuing — polling heartbeat still active): {:?}\n",
                event_result.as_ref().err()
            ));
            lr_trace!("watcher Err, continuing loop");
            continue;
        }
        if !handle_notify_event(
            event_result,
            &target_file,
            &path,
            &tx,
            &mut last_event,
            DEBOUNCE_MS,
            &last_processed_state,
        ) {
            break;
        }
    }
    lr_trace!("watcher_loop exited");
}

/// Process a single notify event. Returns `false` if the channel is closed
/// and the watcher loop should exit.
///
/// **Dedup mechanism (v25.1 Termux fix)**: before processing, snapshots
/// the file's current state (mtime + size + content hash) and compares
/// against `last_processed_state`. If they match on ALL THREE signals,
/// the event is a duplicate (the other source already processed this
/// state) and is silently dropped. This guarantees a single config
/// change triggers exactly one reload, even when both the native
/// watcher and polling heartbeat detect it.
///
/// The triple-signal comparison is critical on Android Termux where
/// mtime may be unreliable but content hash always reflects the actual
/// file state. Without it, a content change that didn't update mtime
/// would either be (a) processed twice (once by native, once by polling)
/// or (b) silently dropped by mtime-only dedup. The snapshot approach
/// handles both cases correctly.
fn handle_notify_event(
    event_result: notify::Result<notify::Event>,
    target_file: &Arc<PathBuf>,
    path: &PathBuf,
    tx: &Sender<LiveConfigEvent>,
    last_event: &mut std::time::Instant,
    debounce_ms: u64,
    last_processed_state: &Arc<Mutex<FileStateSnapshot>>,
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

            // Debounce: catch bursts of native events (atomic-save
            // produces 3-5 events within 50ms). This is a TIME-based
            // dedup; the SNAPSHOT-based dedup below catches cross-source
            // duplicates (native + polling for the same state).
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

            // SNAPSHOT DEDUP (v25.1 Termux fix): snapshot the file's
            // current state (mtime + size + content hash). If it equals
            // last_processed_state on ALL THREE signals, this event is a
            // duplicate — the other source (native or polling) already
            // processed this state. Silently drop it.
            //
            // The triple-signal comparison is essential: on FUSE
            // filesystems where mtime is unreliable, two distinct saves
            // may share the same mtime. The content hash distinguishes
            // them. Conversely, an atomic save may produce a new inode
            // with a new mtime but identical content (e.g., the editor
            // wrote the same buffer twice) — the content hash dedup
            // prevents a redundant reload.
            let current_state = snapshot_file_state(path);
            if current_state.size.is_none() {
                // File doesn't exist (atomic save in progress, file
                // deleted). Skip — the next event will catch the new file.
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

            // Reparse config using parse_config_text (not load_config_file)
            // so we can check malformed_lines AND unknown_keys.
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
                lr_trace!("empty parse result — skipping (likely empty/whitespace-only file)");
                return true;
            }

            if let Err(msg) = validate_and_send(&parsed, tx) {
                // v25: bulletproof write — runs in watcher worker thread.
                use std::io::Write;
                let _ = std::io::stderr().write_fmt(format_args!("[live-reload] {msg}\n"));
            }
            true
        }
        Err(e) => {
            // v25: bulletproof write — runs in watcher worker thread.
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
        let msg = format!(
            "unknown key(s): '{}' (run 'cosmostrix --testconf' for known keys)",
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
                lr_trace!("channel closed during send (Ok) — caller will exit");
                return Ok(()); // channel closed — caller will exit
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

/// Rebuild a CloudConfig from a base template + new config values.
///
/// Takes the original CloudConfig (built from CLI + initial config) and
/// overrides config-derived fields with values from the new config HashMap.
/// CLI-only fields (screen_size, color_tune, message, etc.) are preserved
/// from the base.
///
/// For live reload, config values override CLI defaults (the user is
/// actively editing config.toml and expects those values to take effect).
///
/// v25 priority contract (corrected): `--scene <name>` selects the base
/// scene but does NOT make its managed fields CLI-explicit. config.toml
/// CAN override scene-managed fields when --scene is set via CLI. Only
/// per-field CLI flags (e.g. `-c snow`, `--speed 100`) block config
/// overrides during live reload — they are tracked in `cli_explicit`
/// and remain immutable across reloads.
#[must_use]
pub fn rebuild_cloud_config(
    base: &crate::app::CloudConfig,
    cfg: &HashMap<String, String>,
) -> crate::app::CloudConfig {
    let mut new = base.clone();
    // Bug 3 fix: snapshot the CLI-explicit tracker from the base config.
    // Each config-derived field below consults this tracker before
    // applying — CLI-explicit per-field flags are preserved across live
    // reload. Scene-managed fields are NOT CLI-explicit by proxy; config
    // overrides apply normally for them.
    let cli = new.cli_explicit.clone();

    lr_trace!(
        "rebuild_cloud_config: cli_explicit = {{color:{}, charset:{}, speed:{}, density:{}, fps:{}, scene:{}, glitch:{}}}",
        cli.color, cli.charset, cli.speed, cli.density, cli.fps, cli.scene, cli.glitch_level
    );

    // Color scheme — skip if CLI explicitly set --color
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

    // v16: Custom color palette live reload.
    // If a custom palette was active at startup (via --colors-custom),
    // reload its definition from the new config so editing color values
    // takes effect immediately.
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

    // Charset (requires rebuilding chars vector) — skip if CLI --charset
    if !cli.charset {
        if let Some(v) = cfg.get("charset") {
            // v25: charset-custom.<name> takes precedence over built-in
            // presets when the name matches a [charset-custom.<name>]
            // block in the config. Falls back to built-in charset_from_str.
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

    // Scene (affects rain_style) — skip if CLI --scene was explicit
    if !cli.scene {
        if let Some(v) = cfg.get("scene") {
            if let Some(scene_info) = crate::scene::get_scene(v) {
                new.rain_style = scene_info.config.rain_style;
                // Apply scene color/charset if set — but still respect
                // CLI-explicit color/charset (don't let scene override CLI).
                if let Some(color) = scene_info.config.color {
                    if !cli.color {
                        if let Ok(scheme) = crate::cli::parse_color_scheme(color) {
                            new.color_scheme = scheme;
                        }
                    }
                }
                if let Some(charset_name) = scene_info.config.charset {
                    if !cli.charset {
                        // v25: scene-defined charset name may resolve to a
                        // [charset-custom.<name>] block — check custom first.
                        if let Some(custom_chars) =
                            crate::charset_custom::load_custom_charset_if_matches(cfg, charset_name)
                        {
                            new.charset_preset = charset_name.to_string();
                            new.chars = custom_chars;
                        } else if let Ok(charset) =
                            crate::charset::charset_from_str(charset_name, false)
                        {
                            new.charset_preset = charset_name.to_string();
                            new.chars = crate::charset::build_chars(
                                charset,
                                &new.user_ranges,
                                new.def_ascii,
                            );
                        }
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

    // v17 mastery: legacy advanced config keys (glitchpct, shortpct, rippct,
    // maxdpc) REMOVED from live reload. These are now fully controlled by
    // --glitch-level. The old keys are silently ignored on live reload.

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

    // v17 mastery: scene-custom selector key REMOVED from config.toml.
    // Density map is now loaded only via --scene-custom CLI flag in main.rs.
    // Live reload no longer reads the 'scene-custom' config key.

    // v20: scene-custom live reload. If a custom scene is active (set via
    // --scene-custom <name>), re-apply its fields from the new config so
    // editing [scene-custom.<name>] takes effect immediately. Color/charset
    // changes are applied to the CloudConfig; density-map changes will be
    // picked up by main.rs's monolith_density_map re-resolution on the next
    // Cloud rebuild (the watcher path in event_loop.rs already calls
    // create_cloud + cloud.reset()).
    if let Some(ref custom_name) = base.scene_custom_name {
        apply_scene_custom_to_cloud_config(&mut new, cfg, custom_name);
    }

    new
}

/// Apply a `[scene-custom.<name>]` block from config to a CloudConfig in
/// place. Used by live reload so edits to a custom scene take effect
/// without restarting cosmostrix.
///
/// Fields override the CloudConfig directly. Missing fields are left
/// unchanged (the CloudConfig retains whatever it had before the reload).
/// This mirrors the startup-time behavior in `apply_profile_overrides`
/// but operates on CloudConfig instead of Args.
///
/// v20.1: `base-scene` and `preset` are no longer recognized fields, so
/// they never reach this function — `is_scene_custom_config_key` rejects
/// them and `collect_custom_scenes` skips them. They will instead show up
/// as unknown keys via `--testconf`.
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
        // v20.1: base-scene and preset are no longer recognized; they are
        // filtered upstream by is_scene_custom_config_key. Unknown fields
        // are ignored here.
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
        assert_eq!(
            new.color_scheme,
            crate::runtime::ColorScheme::Green,
            "live reload must apply scene-custom color change"
        );
        // scene_name must remain the custom scene name (not be overwritten).
        assert_eq!(new.scene_name, "test-scene");
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
        assert_eq!(
            new.speed, 25.0,
            "CLI --speed 25 must NOT be overridden by scene=matrix speed=18"
        );
    }
}
