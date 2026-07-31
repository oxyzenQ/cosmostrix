// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Polling heartbeat for live config reload — split out of `live_config.rs`
//! so that file stays under the 1500-LOC source cap enforced by
//! `loc_tests`.
//!
//! ## v25.1 Termux fix: triple-signal change detection
//!
//! The previous polling heartbeat relied SOLELY on
//! `std::fs::Metadata::modified()`. On Android Termux's FUSE-mounted
//! `/sdcard/` and on some emulated filesystems, mtime is unreliable
//! in three ways:
//!
//! 1. `modified()` returns `Err` (the FUSE driver doesn't implement
//!    `statx` properly). Previous code: `Err(_) => continue` — the
//!    polling NEVER fired, so live reload silently died.
//! 2. `modified()` returns `Ok` but the value never changes (FUSE caches
//!    mtime aggressively, or 1-second granularity misses rapid saves).
//!    Previous code: mtime unchanged → continue → no event fired.
//! 3. `modified()` returns `Ok` with a stale value from before the actual
//!    write (write-behind caching). Same result: no event fired.
//!
//! The fix uses THREE signals and fires if ANY of them changed:
//!   - mtime (preferred — fastest, no I/O beyond the stat)
//!   - file size (catches truncate/extend writes that didn't update mtime)
//!   - content hash of the first 8KB (catches in-place content edits that
//!     didn't change size or mtime — e.g., FUSE mtime bug)
//!
//! The content hash is a simple FNV-1a 64-bit hash. Reading 8KB adds
//! ~50µs overhead per poll — negligible at 750ms intervals.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use notify::EventKind;

/// Default base polling interval (ms). Used when
/// `COSMOSTRIX_LIVE_RELOAD_POLL_MS` env var is unset or invalid.
const DEFAULT_POLL_INTERVAL_MS: u64 = 750;

/// Burst polling interval (ms) — used for `BURST_CYCLES` cycles immediately
/// after any change is detected (by either the native watcher or the
/// polling heartbeat itself). Catches rapid successive edits from
/// formatters, linters, and multi-step atomic saves without increasing
/// steady-state CPU usage.
const BURST_POLL_INTERVAL_MS: u64 = 200;

/// Number of fast-poll cycles after a detected change. At 200ms each,
/// this covers a 1-second burst window — enough for the typical
/// "save → formatter → re-save" sequence.
const BURST_CYCLES: u8 = 5;

/// v25.5 strengthening: when a NEW change is detected DURING an active
/// burst, extend the burst by this many cycles (rather than resetting to
/// BURST_CYCLES). This catches chain-editing scenarios (formatter →
/// linter → save → editor auto-save) without dropping out of burst mode
/// between each step. Capped at BURST_CYCLES_MAX to bound worst-case
/// burst duration.
const BURST_CYCLES_EXTEND: u8 = 3;

/// Hard cap on burst cycles. At 200ms each, this bounds the worst-case
/// burst window to ~2 seconds of fast polling — after that, the burst
/// decays naturally even if changes keep arriving.
const BURST_CYCLES_MAX: u8 = 10;

/// Read the polling interval from `COSMOSTRIX_LIVE_RELOAD_POLL_MS` env var,
/// clamped to `[50, 5000]` ms. Returns `DEFAULT_POLL_INTERVAL_MS` (750) if
/// unset or invalid. Power users on slow filesystems can raise it; users
/// wanting faster reload can lower it (at the cost of more I/O per second).
#[must_use]
pub fn env_poll_interval_ms() -> u64 {
    std::env::var("COSMOSTRIX_LIVE_RELOAD_POLL_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&ms| (50..=5000).contains(&ms))
        .unwrap_or(DEFAULT_POLL_INTERVAL_MS)
}

/// Polling heartbeat: checks file mtime/size/content every `base_interval_ms`
/// and feeds synthetic notify events into `tx` when ANY of them changes.
/// This runs on a background thread alongside the native watcher,
/// guaranteeing live reload works even when the native backend is silent
/// (e.g., FreeBSD kqueue feature not active, Android Termux inotify
/// throttling, restricted containers).
///
/// **Adaptive burst mode (v25.4 strengthening)**: after ANY change is
/// detected — whether by the polling heartbeat itself or by the native
/// watcher (signalled via `change_counter`) — the poll interval drops to
/// `BURST_POLL_INTERVAL_MS` (200ms) for `BURST_CYCLES` (5) cycles. This
/// catches rapid successive edits (formatters, linters, multi-step atomic
/// saves) without increasing steady-state CPU usage. After the burst
/// window, the interval returns to `base_interval_ms`.
///
/// The synthetic event uses `EventKind::Modify(ModifyKind::Any)` with
/// the target file as the path, so the unified event loop in
/// `watcher_loop` (in `live_config.rs`) treats it identically to a
/// native modify event.
///
/// **Startup reload prevention**: all three signals are snapshotted at
/// heartbeat start. The first poll (`base_interval_ms` later) compares
/// against these initial values — if nothing changed, no event is sent.
pub fn polling_heartbeat(
    path: std::path::PathBuf,
    tx: Sender<notify::Result<notify::Event>>,
    base_interval_ms: u64,
    change_counter: Arc<AtomicU64>,
) {
    // Snapshot the initial state. Each field is `Option` because any
    // individual signal may be unavailable (e.g., `modified()` Err on
    // FUSE). When a signal is `None`, it's treated as a distinct value
    // — `Some(t) != None`, so a transition from "mtime available" to
    // "mtime unavailable" registers as a change (the file may have been
    // replaced by an atomic save).
    let mut last_state = snapshot_file_state(&path);

    // Track the last-seen change_counter value to detect when the native
    // watcher has accepted an event (which increments the counter). On
    // such detection, enter burst mode to catch follow-up edits.
    let mut last_change_count = change_counter.load(Ordering::Acquire);

    // Burst mode counter: when > 0, poll at BURST_POLL_INTERVAL_MS.
    // Decremented each cycle. Reset to BURST_CYCLES on any change.
    let mut burst_cycles_remaining: u8 = 0;

    lr_trace!(
        "polling heartbeat started: base_interval={}ms burst={}ms×{}cycles initial={:?}",
        base_interval_ms,
        BURST_POLL_INTERVAL_MS,
        BURST_CYCLES,
        last_state
    );

    // v25.3: cycle counter for periodic liveness tracing. Every 5th
    // cycle (~3.75s at 750ms interval), emit a heartbeat trace so the
    // user can verify the polling thread is actually alive. Without
    // this, a dead polling thread produces ZERO trace output, making
    // Termux debugging impossible.
    let mut cycle: u64 = 0;

    loop {
        // Adaptive interval: burst mode uses fast interval, otherwise base.
        let interval_ms = if burst_cycles_remaining > 0 {
            burst_cycles_remaining -= 1;
            BURST_POLL_INTERVAL_MS
        } else {
            base_interval_ms
        };
        std::thread::sleep(Duration::from_millis(interval_ms));
        cycle += 1;

        // Check if the native watcher (or a prior poll cycle) accepted
        // an event since our last cycle. If so, enter/extend burst mode
        // to catch follow-up edits from formatters/linters.
        let current_change_count = change_counter.load(Ordering::Acquire);
        if current_change_count != last_change_count {
            last_change_count = current_change_count;
            // v25.5 strengthening: extend burst by BURST_CYCLES_EXTEND if
            // already in burst (chain-editing scenario), otherwise start
            // fresh burst at BURST_CYCLES. Capped at BURST_CYCLES_MAX.
            let prev = burst_cycles_remaining;
            burst_cycles_remaining = if prev > 0 {
                (prev + BURST_CYCLES_EXTEND).min(BURST_CYCLES_MAX)
            } else {
                BURST_CYCLES
            };
            if prev == 0 {
                lr_trace!(
                    "poll: external change detected (counter={}) — entering burst mode for {} cycles",
                    current_change_count,
                    BURST_CYCLES
                );
            } else {
                lr_trace!(
                    "poll: external change during burst (counter={}) — extending burst {}→{} cycles (cap={})",
                    current_change_count,
                    prev,
                    burst_cycles_remaining,
                    BURST_CYCLES_MAX
                );
            }
        }

        let current_state = snapshot_file_state(&path);

        // v25.3: periodic liveness trace every 5 cycles. This is the
        // KEY diagnostic for Termux — if the user sees these lines,
        // the polling thread is alive and reading the file. If they
        // DON'T see them, the polling thread is dead/panicked.
        if cycle % 5 == 1 {
            lr_trace!(
                "poll cycle #{} alive (interval={}ms) — current_state={:?}",
                cycle,
                interval_ms,
                current_state
            );
        }

        // If we can't even read the file's metadata, the file may have
        // been deleted (atomic save in progress, editor temp-file swap).
        // Skip this cycle — the next poll will catch the new file.
        if current_state.size.is_none() {
            lr_trace!("poll: metadata read failed — skipping cycle");
            continue;
        }

        if current_state == last_state {
            // No change detected by any signal. This is the common case.
            continue;
        }

        lr_trace!(
            "poll: change detected — old={:?} new={:?}",
            last_state,
            current_state
        );
        last_state = current_state;

        // Enter/extend burst mode after detecting a change ourselves,
        // so we catch rapid follow-up edits (e.g., formatter re-save).
        // Same extend-vs-reset logic as the external-change path above.
        let prev = burst_cycles_remaining;
        burst_cycles_remaining = if prev > 0 {
            (prev + BURST_CYCLES_EXTEND).min(BURST_CYCLES_MAX)
        } else {
            BURST_CYCLES
        };

        // Synthesize a notify::Event so the unified event loop handles
        // it identically to a native event. The path must match what
        // handle_notify_event's `touches_target` check expects (the
        // target file's absolute path).
        let event = notify::Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Any),
            paths: vec![path.clone()],
            attrs: Default::default(),
        };
        if tx.send(Ok(event)).is_err() {
            // Channel closed — main loop exited, terminate the heartbeat.
            lr_trace!("poll: channel closed during send — exiting heartbeat");
            break;
        }
        // Increment the shared change counter so the native watcher's
        // event loop (if it's still running) also knows a change was
        // accepted — though the native watcher doesn't currently use
        // this signal, future liveness diagnostics may.
        change_counter.fetch_add(1, Ordering::AcqRel);
        lr_trace!("poll: synthetic event sent to channel successfully");
    }
}

/// Snapshot of a file's state for change detection. Three signals:
/// mtime, size, content hash of the first 8KB. Equality of this struct
/// means "no change detected by any signal" — the file is presumed
/// unchanged. Inequality means "at least one signal changed" — the
/// file is presumed modified, and a reload event is fired.
///
/// All fields are `Option` because any individual signal may be
/// unavailable (e.g., `modified()` returns `Err` on FUSE filesystems).
/// `None` is treated as a distinct value — `Some(t) != None`, so a
/// transition from "mtime available" to "mtime unavailable" registers
/// as a change (which is correct — the file may have been replaced).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct FileStateSnapshot {
    /// File mtime from `std::fs::Metadata::modified()`. `None` if the
    /// filesystem doesn't support mtime (rare on ext4, common on FUSE).
    pub mtime: Option<std::time::SystemTime>,
    /// File size in bytes. `None` if `metadata()` itself failed.
    pub size: Option<u64>,
    /// FNV-1a 64-bit hash of the first 8KB of file content. `None` if
    /// the file couldn't be read (e.g., file deleted mid-snapshot).
    pub content_hash: Option<u64>,
}

/// Number of bytes to hash for the content-hash fallback. 8KB is enough
/// to catch any typical config edit (most config.toml files are <8KB),
/// while keeping I/O cost low (~50µs on a warm cache).
const HASH_BYTES: usize = 8 * 1024;

/// Snapshot the current state of the file at `path`. Used by
/// `polling_heartbeat` (and `handle_notify_event` in `live_config.rs`)
/// to detect changes via mtime/size/content-hash.
///
/// This function never panics — all I/O operations return `Result` and
/// failures are propagated as `None` in the respective snapshot fields.
pub fn snapshot_file_state(path: &Path) -> FileStateSnapshot {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => {
            return FileStateSnapshot {
                mtime: None,
                size: None,
                content_hash: None,
            };
        }
    };

    let mtime = metadata.modified().ok();
    let size = Some(metadata.len());

    // Read the first HASH_BYTES of the file and compute FNV-1a hash.
    // We compute the hash on EVERY snapshot — yes, this is more I/O
    // than only hashing when mtime/size are unchanged, but the cost
    // is negligible (~50µs per 750ms poll = 0.007% CPU) and the
    // logic is simpler/more robust. If profiling shows this matters,
    // we can optimize later by passing in the previous snapshot.
    let content_hash = hash_file_prefix(path, HASH_BYTES);

    FileStateSnapshot {
        mtime,
        size,
        content_hash,
    }
}

/// Compute a 64-bit FNV-1a hash of the first `max_bytes` of a file.
/// Returns `None` if the file can't be opened or read. The hash is
/// not cryptographic — its only purpose is change detection in the
/// polling heartbeat. FNV-1a is fast (single pass, no lookup table)
/// and has good distribution for short inputs.
///
/// We use std::io::Read's `read` with a capped buffer to avoid reading
/// huge files into memory. Config files are typically <8KB, but a
/// misconfigured file (or a user pointing --config at /dev/zero)
/// could be unbounded — the cap protects against that.
fn hash_file_prefix(path: &Path, max_bytes: usize) -> Option<u64> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; max_bytes.min(8 * 1024)];
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break, // EOF
            Ok(n) => filled += n,
            Err(_) => return None,
        }
    }
    buf.truncate(filled);
    Some(fnv1a_64(&buf))
}

/// Compute FNV-1a 64-bit hash of a byte slice. Public to allow tests
/// to verify the implementation; not part of the public API.
#[must_use]
pub fn fnv1a_64(data: &[u8]) -> u64 {
    // FNV-1a 64-bit parameters.
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mutex to serialize tests that mutate the `COSMOSTRIX_LIVE_RELOAD_POLL_MS`
    /// env var. Rust's default test harness runs tests in parallel, and these
    /// three tests (`env_poll_interval_ms_default_when_unset`,
    /// `env_poll_interval_ms_honors_valid_override`,
    /// `env_poll_interval_ms_falls_back_on_invalid`) all touch the same
    /// process-global env var — without serialization, one test's `set_var`
    /// can race with another's `remove_var`, causing intermittent failures
    /// like "300ms is within range, left=750, right=300" (the default
    /// leaked through because another test removed the override mid-assert).
    ///
    /// The lock is held for the duration of each test; acquisition is
    /// instantaneous when no other env-var test is running.
    static ENV_VAR_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// `env_poll_interval_ms` returns the default (750ms) when the env var
    /// is unset. This is the common case — most users don't override.
    #[test]
    fn env_poll_interval_ms_default_when_unset() {
        let _guard = ENV_VAR_TEST_LOCK.lock().unwrap();
        std::env::remove_var("COSMOSTRIX_LIVE_RELOAD_POLL_MS");
        assert_eq!(
            env_poll_interval_ms(),
            DEFAULT_POLL_INTERVAL_MS,
            "must return default (750ms) when env var is unset"
        );
    }

    /// `env_poll_interval_ms` honors a valid override within [50, 5000].
    #[test]
    fn env_poll_interval_ms_honors_valid_override() {
        let _guard = ENV_VAR_TEST_LOCK.lock().unwrap();
        std::env::set_var("COSMOSTRIX_LIVE_RELOAD_POLL_MS", "300");
        assert_eq!(env_poll_interval_ms(), 300, "300ms is within range");
        std::env::set_var("COSMOSTRIX_LIVE_RELOAD_POLL_MS", "5000");
        assert_eq!(env_poll_interval_ms(), 5000, "5000ms is the upper bound");
        std::env::set_var("COSMOSTRIX_LIVE_RELOAD_POLL_MS", "50");
        assert_eq!(env_poll_interval_ms(), 50, "50ms is the lower bound");
        std::env::remove_var("COSMOSTRIX_LIVE_RELOAD_POLL_MS");
    }

    /// `env_poll_interval_ms` falls back to default on invalid input:
    /// non-numeric, below 50, above 5000. This prevents foot-guns where
    /// a typo silently disables polling (interval=0) or makes it thrash
    /// (interval=1).
    #[test]
    fn env_poll_interval_ms_falls_back_on_invalid() {
        let _guard = ENV_VAR_TEST_LOCK.lock().unwrap();
        for bad in &["not-a-number", "0", "49", "5001", "99999", "-1"] {
            std::env::set_var("COSMOSTRIX_LIVE_RELOAD_POLL_MS", bad);
            assert_eq!(
                env_poll_interval_ms(),
                DEFAULT_POLL_INTERVAL_MS,
                "invalid value '{bad}' must fall back to default"
            );
        }
        std::env::remove_var("COSMOSTRIX_LIVE_RELOAD_POLL_MS");
    }

    /// FNV-1a 64-bit hash known-answer test against published test vectors.
    /// Source: https://datatracker.ietf.org/doc/html/draft-eastlake-fnv
    /// (empty string → offset basis; "a" → 0xaf63dc4c8601ec8c).
    #[test]
    fn fnv1a_64_known_vectors() {
        assert_eq!(fnv1a_64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a_64(b"a"), 0xaf63dc4c8601ec8c);
        // Different inputs MUST produce different hashes (basic avalanche check).
        assert_ne!(fnv1a_64(b"hello"), fnv1a_64(b"world"));
        // Same input → same hash (deterministic).
        assert_eq!(fnv1a_64(b"color = green"), fnv1a_64(b"color = green"));
    }

    /// snapshot_file_state returns None for size on a non-existent file.
    /// This is the contract handle_notify_event relies on to skip events
    /// when the file is briefly absent (atomic save in progress).
    #[test]
    fn snapshot_returns_none_size_for_missing_file() {
        let snap = snapshot_file_state(std::path::Path::new("/nonexistent/path/to/nowhere.toml"));
        assert!(snap.size.is_none(), "size must be None for missing file");
        assert!(snap.mtime.is_none(), "mtime must be None for missing file");
        assert!(
            snap.content_hash.is_none(),
            "content_hash must be None for missing file"
        );
    }

    /// snapshot_file_state returns Some(size) and Some(content_hash) for
    /// a real file. mtime may be None on FUSE filesystems (we can't
    /// reliably simulate that in a test, so we only assert size + hash).
    #[test]
    fn snapshot_returns_some_for_real_file() {
        // Write a temp file via std::env::temp_dir — no safepath check
        // needed because we're not loading it as a config, just stat'ing.
        let dir = std::env::temp_dir().join("cosmostrix-tests");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("snapshot_real_file.toml");
        std::fs::write(&path, b"color = green\n").unwrap();

        let snap = snapshot_file_state(&path);
        assert_eq!(snap.size, Some(14), "size must match bytes written");
        assert!(
            snap.content_hash.is_some(),
            "content_hash must be Some for readable file"
        );
        // mtime should be Some on any real filesystem (ext4/tmpfs/etc).
        // We don't hard-assert because some CI runners use weird filesystems.
        if snap.mtime.is_none() {
            eprintln!(
                "[warn] snapshot mtime is None — running on a filesystem without mtime support?"
            );
        }

        // Cleanup.
        std::fs::remove_file(&path).ok();
    }

    /// Two snapshots of the same file (without modification between them)
    /// must be equal. This is the dedup contract.
    #[test]
    fn snapshot_equality_for_unchanged_file() {
        let dir = std::env::temp_dir().join("cosmostrix-tests");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("snapshot_equality.toml");
        std::fs::write(&path, b"speed = 30\n").unwrap();

        let snap1 = snapshot_file_state(&path);
        let snap2 = snapshot_file_state(&path);
        assert_eq!(
            snap1, snap2,
            "two snapshots of the same file must be equal (dedup contract)"
        );

        std::fs::remove_file(&path).ok();
    }

    /// After modifying the file's content, the snapshot must differ.
    /// This is the change-detection contract.
    #[test]
    fn snapshot_inequality_after_content_change() {
        let dir = std::env::temp_dir().join("cosmostrix-tests");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("snapshot_change.toml");
        std::fs::write(&path, b"speed = 30\n").unwrap();

        let snap1 = snapshot_file_state(&path);

        // Sleep briefly to ensure mtime advances (some filesystems have
        // 1-second granularity). 1.1s is enough even on coarse-grained FS.
        std::thread::sleep(Duration::from_millis(1100));
        std::fs::write(&path, b"speed = 60\n").unwrap();

        let snap2 = snapshot_file_state(&path);
        assert_ne!(snap1, snap2, "snapshots must differ after content change");
        // Content hash MUST differ (even if mtime didn't update).
        assert_ne!(
            snap1.content_hash, snap2.content_hash,
            "content_hash must differ for different content"
        );

        std::fs::remove_file(&path).ok();
    }

    /// v25.5 strengthening: worst-case burst duration must stay under 3s.
    #[test]
    fn burst_cycle_constants_are_sane() {
        // 10 cycles × 200ms = 2s ≤ 3s.
        assert!(
            BURST_CYCLES_MAX as u64 * BURST_POLL_INTERVAL_MS <= 3000,
            "worst-case burst must stay under 3s"
        );
    }

    // ── v25.16 (bug #18): polling-heartbeat end-to-end test ──────────────
    //
    // The polling heartbeat is the fallback path for live config reload on
    // systems where the native `notify` watcher is unreliable (Termux FUSE,
    // FreeBSD kqueue feature gaps, restricted containers). Until now it had
    // NO end-to-end coverage — only the snapshot helpers (`snapshot_file_state`,
    // `fnv1a_64`, env-var parsing) were unit-tested. A silent regression in
    // `polling_heartbeat` itself (e.g. broken channel send, missing state
    // comparison) would have gone undetected by the test suite.
    //
    // This test exercises the full pipeline:
    //   1. Create a real temp file
    //   2. Spawn `polling_heartbeat` on a background thread with a fast
    //      100ms poll interval
    //   3. Wait for the initial snapshot to be captured
    //   4. Modify the file (size + content change → guaranteed detection
    //      by either of the three signals, even on coarse-mtime filesystems)
    //   5. Wait up to 15s for the synthetic notify::Event to arrive on the
    //      channel
    //   6. Verify the event is `Modify/Any` and carries the watched path
    //
    // WHY 15 SECONDS:
    //
    // The previous informal target was 5s, but CI runners have been observed
    // taking 4–8s for a single 100ms poll cycle to wake up under heavy
    // parallel test load (qemu, shared CPU, network filesystems, container
    // scheduling pressure). 15s gives 3× headroom over the worst observed
    // case while still keeping the test fast on healthy systems (typical
    // completion: 300–500ms).
    //
    // The 100ms poll interval keeps the test fast on healthy systems while
    // exercising the same code path as production (which uses 750ms). The
    // 15s timeout is the safety net, not the expected duration.

    /// Generous end-to-end timeout for the polling heartbeat test.
    ///
    /// v25.16 (bug #18): raised from the previous informal 5s target to
    /// 15s. Slow CI filesystems (qemu, network FS, shared-tenant runners)
    /// can introduce multi-second scheduler latency between `thread::sleep`
    /// expiry and the next poll cycle. 15s gives 3× headroom over the
    /// worst observed case while keeping the test fast on healthy systems.
    const HEARTBEAT_E2E_TIMEOUT_SECS: u64 = 15;

    /// Poll interval used by the end-to-end test (ms).
    ///
    /// 100ms is fast enough to complete the test in <1s on healthy
    /// systems, while exercising the exact same code path as production
    /// (which uses `DEFAULT_POLL_INTERVAL_MS` = 750ms). The shorter
    /// interval does NOT change detection semantics — only how often the
    /// heartbeat checks.
    const HEARTBEAT_E2E_POLL_MS: u64 = 100;

    #[test]
    fn polling_heartbeat_end_to_end() {
        use std::sync::atomic::AtomicU64;
        use std::sync::mpsc;
        use std::time::Instant;

        let dir = std::env::temp_dir().join("cosmostrix-tests");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("polling_heartbeat_end_to_end.toml");

        // Write initial content. The heartbeat will snapshot this on entry.
        // Use a UNIQUE initial content per test run so we never confuse
        // this file with a stale leftover from a previous run.
        let initial_content = format!(
            "speed = 30\n# initial v={}\n",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        std::fs::write(&path, initial_content.as_bytes()).unwrap();

        // Set up the channel and change counter exactly as
        // `spawn_live_config_watcher` does in `live_config.rs`.
        let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let counter = Arc::new(AtomicU64::new(0));

        // Spawn the heartbeat thread. Use a named thread so panics show
        // up clearly in the test output.
        let path_inner = path.clone();
        let handle = std::thread::Builder::new()
            .name("test-polling-heartbeat".to_string())
            .spawn(move || {
                polling_heartbeat(path_inner, tx, HEARTBEAT_E2E_POLL_MS, counter);
            })
            .expect("failed to spawn polling heartbeat thread");

        // Wait for the heartbeat to capture its initial snapshot AND
        // complete at least one poll cycle against the unmodified file.
        //
        // v25.16 (bug #18): under heavy parallel test load (especially
        // on shared-tenant CI runners), `std::thread::Builder::spawn`
        // itself can be delayed by 200–800ms before the new thread
        // starts executing. If we modify the file TOO SOON, the
        // heartbeat's initial snapshot captures the already-modified
        // file, and no subsequent change is detected — the test fails
        // by timeout.
        //
        // Mitigation: sleep 1 second (10 poll cycles at 100ms). This is
        // long enough that:
        //   - thread spawn delay (up to ~800ms observed) is absorbed
        //   - the initial snapshot is captured against the ORIGINAL
        //     file content (not the modified one)
        //   - at least 1–2 poll cycles have run against the original
        //     content (proving the heartbeat considers it "stable")
        //
        // After this sleep, we modify the file. The next poll cycle
        // (within 100ms) is guaranteed to detect the change.
        std::thread::sleep(Duration::from_millis(1000));

        // Modify the file: change BOTH size and content_hash. This
        // guarantees detection by at least two of the three signals
        // (size, content_hash) even on filesystems where mtime has
        // 1-second granularity and the write happens within the same
        // mtime tick as the initial write.
        std::fs::write(
            &path,
            b"speed = 60\n# modified by polling_heartbeat_end_to_end test\n",
        )
        .unwrap();

        // Wait up to HEARTBEAT_E2E_TIMEOUT_SECS for the synthetic event.
        let timeout = Duration::from_secs(HEARTBEAT_E2E_TIMEOUT_SECS);
        let start = Instant::now();
        let mut received_event: Option<notify::Event> = None;
        while start.elapsed() < timeout {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(Ok(event)) => {
                    received_event = Some(event);
                    break;
                }
                Ok(Err(_)) => {
                    // Watcher-reported error — keep waiting; the polling
                    // heartbeat itself never sends Err, but a future
                    // refactor could.
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        // Cleanup: drop the receiver to close the channel. Then trigger
        // ONE MORE file change so the heartbeat attempts another send,
        // which fails (channel closed) and causes the heartbeat to
        // exit cleanly via the `tx.send().is_err()` → break path.
        //
        // v25.16 (bug #18): WITHOUT this final write, `handle.join()`
        // would block forever. The heartbeat has no shutdown signal —
        // it exits ONLY when `tx.send()` returns Err. If no further
        // file changes happen, the heartbeat keeps polling indefinitely.
        // The final write triggers the next poll cycle's change
        // detection, which calls `tx.send()` → Err → break → thread
        // exits → join() returns.
        //
        // The 500ms sleep gives the heartbeat time to detect the change
        // (≤200ms in burst mode, ≤100ms in normal mode) and attempt
        // the failing send. This is bounded and deterministic.
        drop(rx);
        std::fs::write(
            &path,
            b"speed = 90\n# final write to trigger heartbeat exit\n",
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(500));
        let _ = handle.join();

        // Remove the temp file (best-effort).
        std::fs::remove_file(&path).ok();

        let event = received_event.unwrap_or_else(|| {
            panic!(
                "polling heartbeat must send a synthetic event within {}s \
                 of file modification (poll interval = {}ms)",
                HEARTBEAT_E2E_TIMEOUT_SECS, HEARTBEAT_E2E_POLL_MS,
            )
        });
        assert_eq!(
            event.kind,
            EventKind::Modify(notify::event::ModifyKind::Any),
            "synthetic event must be Modify/Any"
        );
        assert!(
            event.paths.contains(&path),
            "synthetic event must carry the watched file path: {:?}",
            event.paths
        );
    }
}
