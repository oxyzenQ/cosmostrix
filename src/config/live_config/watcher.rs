// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Config file watcher thread — extracted from `live_config/mod.rs`
//! to keep that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns the file watcher internals:
//! - `spawn_watcher`: spawns the background thread + channel.
//! - `watcher_loop`: main loop (notify events + polling heartbeat).
//! - `handle_notify_event`: debounces + dedups file change events.
//! - `validate_and_send`: strict config validation + channel send.
//!
//! Re-exported from `live_config/mod.rs` via `pub(crate) use` so all
//! existing `crate::live_config::spawn_watcher` call sites resolve
//! unchanged.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{event::EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::configfile;
use crate::live_config_poll::{
    env_poll_interval_ms, polling_heartbeat, snapshot_file_state, snapshot_file_state_cached,
    FileStateSnapshot,
};

use super::LiveConfigEvent;
use super::{
    push_runtime_warning, push_validation_rejection, LIVE_RELOAD_ERROR, LIVE_RELOAD_EXIT_CODE,
};

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
pub(crate) fn watcher_loop(path: PathBuf, tx: SyncSender<LiveConfigEvent>) {
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
            path.as_path(),
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
pub(crate) fn handle_notify_event(
    event_result: notify::Result<notify::Event>,
    target_file: &Arc<PathBuf>,
    path: &Path,
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
            // size match → SHA-512 hash is skipped → ~20× faster dedup
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
            // S-master-3-v2: size-capped read — oversized files skip the
            // reparse (same as a read error) instead of an unbounded read.
            let content = match crate::config_io::read_config_capped(path) {
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
pub(crate) fn validate_and_send(
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
