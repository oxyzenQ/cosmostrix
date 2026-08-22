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
//!
//! ## S3 (internal independent QA): parse race with non-atomic editor writes
//!
//! If the editor writes the config file non-atomically (truncate + write,
//! e.g. `echo > config.toml` or `tee`), the watcher may read a half-written
//! file mid-save. The strict parser will see malformed lines and reject the
//! entire config, setting LIVE_RELOAD_EXIT_CODE=2 and exiting cosmostrix.
//!
//! Editors that write atomically (temp file + rename: vim, emacs, nano,
//! VSCode, most modern editors) are safe — the watcher sees either the
//! old file or the complete new file, never a partial write.
//!
//! This is a known limitation of file-watcher systems. The  design
//! (exit on validation error) makes this more visible than the old "silently
//! keep last valid config" behavior, but it is the honest choice: a
//! malformed config should not be silently ignored.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{event::EventKind, RecommendedWatcher, RecursiveMode, Watcher};

// Polling heartbeat + snapshot dedup live in live_config_poll.rs.
use crate::live_config_poll::{
    env_poll_interval_ms, polling_heartbeat, snapshot_file_state, snapshot_file_state_cached,
    FileStateSnapshot,
};

use crate::configfile;

// AB-10: session-wide buffered state lives in live_config_state.rs.
// Re-export everything so existing `live_config::LIVE_RELOAD_*` and
// `live_config::push_*` references continue to resolve unchanged.
#[cfg(test)]
pub(crate) use crate::live_config_state::drain_validation_rejections;
#[cfg(test)]
pub(crate) use crate::live_config_state::MAX_REJECTION_LOG;
pub(crate) use crate::live_config_state::{
    drain_runtime_warnings, push_runtime_warning, push_validation_rejection, LIVE_RELOAD_ERROR,
    LIVE_RELOAD_EXIT_CODE,
};

/// Live config event sent from watcher to render thread.
/// Ok = valid config, rebuild Cloud. Err = invalid, exit cosmostrix.
pub(crate) type LiveConfigEvent = Result<HashMap<String, String>, String>;

/// Spawn a config file watcher on a background thread.
///
/// Returns a `Receiver<HashMap<String, String>>` that the render thread polls
/// with `try_recv()` each frame. The watcher validates config strictly
/// before sending — invalid configs are rejected with a stderr error.
/// Returns `None` if the file doesn't exist or can't be watched.
pub(crate) fn spawn_watcher(config_path: PathBuf) -> Option<Receiver<LiveConfigEvent>> {
    if !config_path.exists() {
        lr_trace!(
            "config file does not exist — watcher NOT spawned: {}",
            config_path.display()
        );
        return None;
    }

    lr_trace!("spawning watcher for: {}", config_path.display());

    // Pillar 3: bounded channel (cap 64) — prevents unbounded queue growth
    // if the editor saves 1000×/s. When full, events are dropped (try_send)
    // rather than blocking the watcher thread.
    let (tx, rx) = mpsc::sync_channel::<LiveConfigEvent>(64);
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
                // Phase 5 (P3-3/P4-1): on mutex poison, the LIVE_RELOAD_ERROR
                // mutex is also poisoned (same lock). Buffer the diagnostic
                // to the runtime warning log so main.rs can drain it post-exit
                // instead of eprintln-ing here (which leaks into the alt
                // screen rain matrix — AB-10).
                match LIVE_RELOAD_ERROR.lock() {
                    Ok(mut g) => *g = Some("watcher thread terminated unexpectedly".to_string()),
                    Err(_) => push_runtime_warning(
                        "[live-reload] mutex poisoned — watcher thread terminated unexpectedly",
                    ),
                }
                LIVE_RELOAD_EXIT_CODE.store(2, Ordering::Release);
            }
        });

    match spawn_result {
        Ok(_) => lr_trace!("watcher thread spawned successfully — live reload active"),
        Err(e) => {
            lr_trace!("FAILED to spawn watcher thread: {e} — live reload disabled");
            // AB-10: buffer instead of eprintln — leaks into alt screen otherwise.
            push_runtime_warning(&format!(
                "[live-reload] FAILED to spawn watcher thread: {e} — live reload disabled"
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
fn watcher_loop(path: PathBuf, tx: SyncSender<LiveConfigEvent>) {
    const DEBOUNCE_MS: u64 = 200;
    // env-configurable poll interval + adaptive burst.
    let poll_interval_ms = env_poll_interval_ms();
    let change_counter = Arc::new(AtomicU64::new(0));

    // Pillar 3: bounded notify channel (cap 64) — prevents unbounded growth.
    let (notify_tx, notify_rx) = std::sync::mpsc::sync_channel::<notify::Result<notify::Event>>(64);

    // Snapshot initial state to avoid startup reload.
    let last_processed_state = Arc::new(Mutex::new(snapshot_file_state(&path)));

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
                        // AB-10: buffer — eprintln leaks into the alt screen.
                        push_runtime_warning(
                            "[live-reload] polling heartbeat panicked — restarting after 1s backoff",
                        );
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
            // AB-10: buffer — eprintln leaks into the alt screen.
            push_runtime_warning(&format!(
                "[live-reload] failed to spawn polling heartbeat: {e} — native watcher only"
            ));
        }
    }

    // Try native watcher; on failure, log + continue (polling heartbeat covers).
    lr_trace!("initializing native RecommendedWatcher");
    let mut watcher: Option<RecommendedWatcher> = match RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            let _ = notify_tx.try_send(res);
        },
        notify::Config::default(),
    ) {
        Ok(w) => {
            lr_trace!("native RecommendedWatcher created successfully");
            Some(w)
        }
        Err(e) => {
            lr_trace!("native watcher unavailable: {e} — relying on polling heartbeat only");
            // AB-10: buffer — eprintln leaks into the alt screen.
            push_runtime_warning(&format!(
                "[live-reload] native watcher unavailable: {e} — relying on polling heartbeat ({poll_interval_ms}ms base interval)"
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
            // AB-10: buffer — eprintln leaks into the alt screen.
            push_runtime_warning(&format!(
                "[live-reload] native watcher failed to register {}: {e} — relying on polling heartbeat",
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

    // native watcher liveness diagnostic.
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
            // AB-10: buffer — eprintln leaks into the alt screen.
            push_runtime_warning(&format!(
                "[live-reload] transient watcher error (continuing — polling heartbeat still active): {:?}",
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

        // classify event source for liveness diagnostic.
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
            // AB-10: buffer — eprintln leaks into the alt screen.
            push_runtime_warning(&format!(
                "[live-reload] native watcher silent {elapsed}s — polling heartbeat sole detector (informational)"
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

/// Process a single notify event. Returns `false` if channel closed.
/// Dedup: mtime + size + content hash; drops if all three equal
/// `last_processed_state` (critical on Termux where mtime is unreliable).
#[allow(clippy::too_many_arguments)]
fn handle_notify_event(
    event_result: notify::Result<notify::Event>,
    target_file: &Arc<PathBuf>,
    path: &PathBuf,
    tx: &SyncSender<LiveConfigEvent>,
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

            // SNAPSHOT DEDUP: mtime + size + content hash. Drop if
            // equal to last_processed_state on all three signals.
            //
            // masterclass: use `snapshot_file_state_cached` with the
            // previous snapshot as cache. On the common duplicate-event
            // path (native + poll both fire for the same edit), mtime +
            // size match → SHA-256 hash is skipped → ~20× faster dedup
            // check (~5µs instead of ~100µs).
            //
            // The snapshot is computed INSIDE the lock to avoid a TOCTOU:
            // if we computed it outside, another thread could update
            // `last_processed_state` between our snapshot and our
            // compare+update. Holding the lock for the snapshot is safe
            // because the fast path is just `metadata()` (~5µs).
            let current_state = {
                // P1-#11: poison-safe lock. Poisoned mutex → skip, don't panic.
                let guard = match last_processed_state.lock() {
                    Ok(g) => g,
                    Err(_) => return true,
                };
                snapshot_file_state_cached(path, Some(&*guard))
            };
            if current_state.size.is_none() {
                // File doesn't exist (atomic save in progress) — skip.
                lr_trace!("snapshot: file unreadable — skipping event");
                return true;
            }
            {
                // P1-#11: poison-safe lock. Poisoned mutex → skip, don't panic.
                let mut guard = match last_processed_state.lock() {
                    Ok(g) => g,
                    Err(_) => return true,
                };
                if *guard == current_state {
                    // Duplicate event — both native + poll detected same change.
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

            // strengthening: signal polling heartbeat to enter burst
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
                // (bug #15): DO NOT write to stderr here. The watcher
                // thread runs concurrently with the rain render loop, and any
                // stderr write during rain leaks into the alternate-screen
                // buffer — the user sees "weird text" polluting the rain
                // matrix, then rain overwrites it. The rejection is already
                // pushed to LIVE_RELOAD_VALIDATION_REJECTIONS by
                // `validate_and_send` for the post-exit verbose summary.
                // The render thread's Err handler sets LIVE_RELOAD_EXIT_CODE
                // and breaks the rain loop so cosmostrix exits immediately.
                // The error message is printed AFTER terminal restoration
                // in main.rs — never during rain.
                let _ = msg; // acknowledged; intentionally not printed here
            }
            true
        }
        Err(e) => {
            // (bug #15): same reasoning — do NOT write to stderr
            // during rain. Transient watcher errors are recoverable (polling
            // heartbeat covers). Log to lr_trace! for debug builds only.
            lr_trace!("[live-reload] transient watch error (polling heartbeat covers): {e}");
            true
        }
    }
}

/// Validate parsed config strictly, then send Ok(cfg) or Err(msg) to the
/// render thread. Err(msg) returned if validation failed (caller logs it).
fn validate_and_send(
    parsed: &configfile::ParsedConfig,
    tx: &SyncSender<LiveConfigEvent>,
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
        // (bug #14): surface to session rejection log.
        push_validation_rejection(&msg);
        let _ = tx.try_send(Err(msg.clone()));
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
        // depth-test fix: append "did you mean" hints for structural
        // mistakes (e.g. color.tune.bold). Returns "" when no hints apply.
        let hints = crate::config_hints::format_hints_block(&parsed.unknown_keys);
        let msg = format!(
            "unknown key(s): '{}' (run 'cosmostrix --testconf' for known keys){hints}",
            keys.join(", ")
        );
        // (bug #14): surface to session rejection log.
        push_validation_rejection(&msg);
        let _ = tx.try_send(Err(msg.clone()));
        return Err(msg);
    }

    // auto-promoted keys are NOT errors — parser re-homed them to
    // root scope. Surface as info so user knows TOML was structurally off.
    if !parsed.promoted_keys.is_empty() {
        let preview = parsed
            .promoted_keys
            .iter()
            .take(2)
            .map(|(from, to)| format!("{from} -> {to}"))
            .collect::<Vec<_>>()
            .join("; ");
        let extra = parsed.promoted_keys.len().saturating_sub(2);
        lr_trace!(
            "auto-promoted {} key(s): {preview}{}",
            parsed.promoted_keys.len(),
            if extra > 0 {
                format!(" (+{extra} more)")
            } else {
                String::new()
            }
        );
    }

    let cfg = &parsed.values;

    // Strict validation: reject entire config if ANY field is invalid.
    match crate::testconf::validate_config_strictly(cfg) {
        Ok(()) => {
            lr_trace!("strict validation OK — sending config to render thread");
            if tx.try_send(Ok(cfg.clone())).is_err() {
                lr_trace!("channel closed during send (Ok)");
                return Ok(());
            }
            Ok(())
        }
        Err(msg) => {
            // (bug #14): surface rejection to session log so post-exit
            // verbose summary can show it.
            lr_trace!("strict validation FAILED: {msg}");
            push_validation_rejection(&msg);
            let _ = tx.try_send(Err(msg.clone()));
            Err(msg)
        }
    }
}

/// Rebuild a CloudConfig from base + new config values. CLI-only fields
/// preserved from base. Per-field CLI flags (`cli_explicit`) immutable.
#[must_use]
pub(crate) fn rebuild_cloud_config(
    base: &crate::app::CloudConfig,
    cfg: &HashMap<String, String>,
) -> crate::app::CloudConfig {
    let mut new = base.clone();
    // Snapshot CLI-explicit tracker — preserved across reloads.
    // CliExplicit derives Copy, so this is a cheap field copy, not a heap clone.
    let cli = new.cli_explicit;

    lr_trace!(
        "rebuild_cloud_config: cli_explicit = {{color:{}, charset:{}, speed:{}, density:{}, fps:{}, scene:{}, glitch:{}}}",
        cli.color, cli.charset, cli.speed, cli.density, cli.fps, cli.scene, cli.glitch_level
    );

    // depth-test fix: user-set color/charset must win over scene defaults.
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
        if let Ok(palette) = crate::colors_custom::load_custom_palette(cfg, name) {
            new.custom_palette = Some(palette);
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

    // Scene — skip if CLI --scene explicit. scene color/charset are
    // defaults; user config values win.
    if !cli.scene {
        if let Some(v) = cfg.get("scene") {
            // v50 fix: update new.scene_name to match the config's scene
            // value. Without this, the live-reload path left scene_name at
            // base.scene_name (the previous value), so the HUD 'scn:' line
            // showed the old scene even though the rain style had already
            // switched. The event_loop.rs schedule-empty preserve/else
            // branch compares new_cfg.scene_name against preserved_scene_name
            // to decide whether to re-apply scene runtime — both values MUST
            // reflect the config's scene for that branch to fire correctly.
            // This is the source-of-truth fix; the event_loop.rs else branch
            // (commit 51ccafe) is the consumer-side mirror.
            //
            // Normalization: scene names are case-insensitive at lookup
            // (scene::get_scene lowercases internally), but the HUD displays
            // the exact string the user typed. We preserve the original
            // casing from config for display, matching the startup path in
            // main.rs (args.scene.as_deref().unwrap_or(DEFAULT_SCENE)).
            lr_trace!("apply scene='{}' (updating scene_name)", v);
            new.scene_name = v.clone();
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

    // Glitch level — skip if CLI --glitch-level was explicit.
    // (CLI-P-3): re-derive ALL preset values on live reload.
    // (Glitch-BUG3): None arm now resets all 5 preset fields too.
    // BL-01 (Dragon Hunt v3): dedup — delegate to the shared helper in
    // scene_custom.rs (bit-identical preset values, was inlined here).
    // max_dpc is NOT touched — never set by glitch_level presets at startup.
    if !cli.glitch_level {
        if let Some(v) = cfg.get("glitch-level") {
            lr_trace!("apply glitch-level='{}'", v);
            use clap::ValueEnum;
            match crate::config::GlitchLevel::from_str(v, true) {
                Ok(level) => {
                    crate::scene_custom::apply_glitch_level_preset_to_cloud_config(&mut new, level);
                }
                // Unrecognized: flip enable bool only (old fallback).
                // Startup clap rejects bad values, so this shouldn't fire.
                Err(_) => {
                    new.glitch_enabled = !v.trim().eq_ignore_ascii_case("none");
                }
            }
        }
    } else {
        lr_trace!(
            "skip glitch-level (CLI explicit) — glitch_enabled={}",
            new.glitch_enabled
        );
    }

    // color-bg live reload (true = terminal default; false = solid black).
    if let Some(v) = cfg.get("color-bg") {
        new.default_bg = match v.trim().to_ascii_lowercase().as_str() {
            "black" => false,
            "default-background" | "default_background" => true,
            _ => new.default_bg,
        };
        lr_trace!("apply color-bg='{}' → default_bg={}", v, new.default_bg);
    }

    // Monolith size
    if let Some(v) = cfg.get("monolith-size") {
        use clap::ValueEnum;
        if let Ok(size) = crate::runtime::MonolithSize::from_str(v, true) {
            new.monolith_size = size;
        }
    }

    // Crystal Dragon Engine — intent preservation: CLI --crystal-dragon
    // wins over config.toml on live reload.
    if !cli.crystal_dragon {
        if let Some(v) = cfg.get("crystal-dragon") {
            if let Some(b) = crate::config_apply::parse_bool_config("crystal-dragon", v) {
                new.crystal_dragon = b;
            }
        }
    }

    // v50: Power Dragon live reload. No CLI flag exists for power-dragon
    // (config-only setting), so no intent-preservation guard is needed —
    // always read from config. When toggled mid-session, the adaptive
    // throttle state updates on the next frame (no restart needed).
    if let Some(v) = cfg.get("power-dragon") {
        if let Some(b) = crate::config_apply::parse_bool_config("power-dragon", v) {
            new.power_dragon = b;
        }
    }

    // (CLI-P-1): live-reload bold/shadingmode/async-mode (previously
    // silently ignored). Mirrors startup parsers with range validation.
    if let Some(v) = cfg.get("bold").and_then(|s| s.trim().parse::<u8>().ok()) {
        // Range-gate to match startup parse_u8_config("bold", ..., 0, 2).
        // Upstream validate_config_strictly catches out-of-range before this
        // runs, but defense-in-depth prevents silent mis-parsing if that
        // validation ever has a regression.
        new.bold_mode = match v {
            0 => crate::runtime::BoldMode::Off,
            2 => crate::runtime::BoldMode::All,
            _ => crate::runtime::BoldMode::Random,
        };
        if v > 2 {
            // Out-of-range: log and let validate_config_strictly handle
            // rejection on next cycle. Do not apply the parsed value.
            new.bold_mode = base.bold_mode;
        }
    }
    if let Some(v) = cfg
        .get("shadingmode")
        .and_then(|s| s.trim().parse::<u8>().ok())
    {
        new.shading_mode = match v {
            1 => crate::runtime::ShadingMode::DistanceFromHead,
            _ => crate::runtime::ShadingMode::Random,
        };
        if v > 1 {
            new.shading_mode = base.shading_mode;
        }
    }
    if let Some(v) = cfg.get("async-mode") {
        if let Some(b) = crate::config_apply::parse_bool_config("async-mode", v) {
            new.async_mode = b;
        }
    }

    // v20: scene-custom live reload — re-apply fields if active.
    if let Some(ref custom_name) = base.scene_custom_name {
        crate::scene_custom::apply_scene_custom_to_cloud_config(&mut new, cfg, custom_name);
    }

    // (bug #9): color.tune.* live reload — re-parse from cfg HashMap
    // (same path as startup) when at least one color.tune.* key is present.
    // Preserves CLI --color-tune when no [color.tune] block exists.
    let has_tune_keys = cfg.contains_key("color.tune.brightness")
        || cfg.contains_key("color.tune.saturation")
        || cfg.contains_key("color.tune.head")
        || cfg.contains_key("color.tune.body")
        || cfg.contains_key("color.tune.tail");
    if has_tune_keys {
        let new_tune = crate::color_tune::color_tune_from_config(cfg);
        if new_tune.brightness != new.color_tune.brightness
            || new_tune.saturation != new.color_tune.saturation
            || new_tune.head != new.color_tune.head
            || new_tune.body != new.color_tune.body
            || new_tune.tail != new.color_tune.tail
        {
            lr_trace!(
                "apply color.tune live reload: sat={} bright={} head={} body={} tail={} (was sat={} bright={} head={} body={} tail={})",
                new_tune.saturation, new_tune.brightness, new_tune.head, new_tune.body, new_tune.tail,
                new.color_tune.saturation, new.color_tune.brightness, new.color_tune.head, new.color_tune.body, new.color_tune.tail
            );
            new.color_tune = new_tune;
        } else {
            lr_trace!("color.tune: present but unchanged");
        }
    } else {
        lr_trace!("color.tune: no keys in config — preserving base tune (CLI --color-tune wins)");
    }

    // Ambient: re-collect schedule. Event loop pushes to scheduler thread.
    new.ambient_schedule = crate::crystal_dragon_engine::ambient::collect_ambient_schedule(cfg);
    if !new.ambient_schedule.is_empty() {
        lr_trace!(
            "ambient: reloaded {} entries",
            new.ambient_schedule.entries.len()
        );
    }

    new
}

#[cfg(test)]
mod tests;
