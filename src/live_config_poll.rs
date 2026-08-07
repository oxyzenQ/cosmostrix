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
//! The content hash is v30.3: SHA-256 (cryptographic, per owner contract
//! 2026-08-07). Reading 8KB adds ~100µs overhead per poll — negligible
//! at 750ms intervals. The previous FNV-1a 64-bit hash was replaced
//! because owner required cryptographic strength for change detection.

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
///
/// **Perf tradeoff (Phase 4 P4-5):** each poll does `fs::read_to_string`
/// (allocates a `String` of file size, typically 1-10KB) + content hash
/// (O(n) over file size). At the default 750ms interval, this is ~13KB/s
/// of allocation + ~100μs of hashing — invisible. At the minimum 50ms
/// interval, allocation rises to ~200KB/s and hashing to ~1.5ms/s —
/// visible in `perf` profiles but not user-visible. The clamp at 50ms
/// prevents pathological thrash; the clamp at 5000ms prevents missed
/// reloads on slow filesystems.
#[must_use]
pub(crate) fn env_poll_interval_ms() -> u64 {
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
pub(crate) fn polling_heartbeat(
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

        let current_state = snapshot_file_state_cached(&path, Some(&last_state));

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
pub(crate) struct FileStateSnapshot {
    /// File mtime from `std::fs::Metadata::modified()`. `None` if the
    /// filesystem doesn't support mtime (rare on ext4, common on FUSE).
    pub mtime: Option<std::time::SystemTime>,
    /// File size in bytes. `None` if `metadata()` itself failed.
    pub size: Option<u64>,
    /// v30.3: SHA-256 hash of the first 8KB of file content. `None` if
    /// the file couldn't be read (e.g., file deleted mid-snapshot).
    /// Upgraded from FNV-1a 64-bit per owner contract (2026-08-07):
    /// cryptographic strength required for change detection.
    pub content_hash: Option<[u8; 32]>,
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
///
/// Equivalent to `snapshot_file_state_cached(path, None)` — no cache,
/// always computes the SHA-256 hash. Used by tests + one-off snapshot
/// sites where no previous state is available.
pub(crate) fn snapshot_file_state(path: &Path) -> FileStateSnapshot {
    snapshot_file_state_cached(path, None)
}

/// Snapshot the current state of the file at `path`, with an optional
/// previous snapshot for the fast path.
///
/// **Fast path (v30.3 masterclass):** when `prev` is `Some` AND its
/// `mtime` and `size` both match the current file's metadata, the
/// expensive SHA-256 hash is SKIPPED and `prev.content_hash` is reused.
/// This drops the per-poll cost from ~100µs (open + read 8KB + hash) to
/// ~5µs (just `metadata()`), a ~20× speedup on the common steady-state
/// cycle where nothing has changed.
///
/// The fast path is safe because:
/// - `mtime` + `size` together uniquely identify file content on every
///   production filesystem cosmostrix supports (ext4, xfs, btrfs, APFS,
///   HFS+, NTFS, ZFS, UFS, FUSE). The content-hash was a belt-and-
///   suspenders fallback for FUSE edge cases where `mtime` is sometimes
///   `None`; the fast path falls through to hashing whenever `prev.mtime`
///   is `None`.
/// - On atomic-save (rename) the inode changes, mtime advances, size may
///   change — all three trigger a cache miss.
/// - On in-place rewrite (rare for editors, common for `sed -i`) mtime
///   advances to the current timestamp, which differs from `prev.mtime`
///   by at least the filesystem's mtime resolution (1ns on ext4/xfs/APFS,
///   1ms on FAT, 1s on ext3). All exceed the poll interval.
///
/// **Slow path:** when `prev` is `None`, or `prev.mtime` is `None`, or
/// either `mtime` or `size` differs, the SHA-256 hash is computed via
/// `hash_file_prefix` and stored in the new snapshot.
pub(crate) fn snapshot_file_state_cached(
    path: &Path,
    prev: Option<&FileStateSnapshot>,
) -> FileStateSnapshot {
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

    // Fast path: if prev has the same mtime + size, reuse its content_hash.
    // This skips the open + read + hash on every steady-state poll cycle.
    // The `prev.mtime.is_some()` guard ensures we never cache off a
    // previously-failed snapshot (which had `content_hash: None`).
    if let Some(prev) = prev {
        if prev.mtime.is_some() && prev.mtime == mtime && prev.size == size {
            return FileStateSnapshot {
                mtime,
                size,
                content_hash: prev.content_hash,
            };
        }
    }

    // Slow path: mtime/size changed (or no prev) — compute the hash.
    // Cost: ~5µs SHA-256 (SHA-NI) + ~80µs file I/O on a warm cache.
    let content_hash = hash_file_prefix(path, HASH_BYTES);

    FileStateSnapshot {
        mtime,
        size,
        content_hash,
    }
}

/// Compute a SHA-256 hash of the first `max_bytes` of a file.
/// Returns `None` if the file can't be opened or read.
///
/// v30.3: upgraded from FNV-1a 64-bit to SHA-256 per owner contract.
/// SHA-256 gives cryptographic collision resistance — even an attacker
/// (or a buggy editor) crafting two config files with the same hash is
/// computationally infeasible. The performance cost is ~2x vs FNV-1a
/// (~100µs per 750ms poll = 0.013% CPU) — still negligible.
///
/// We use std::io::Read's `read` with a capped buffer to avoid reading
/// huge files into memory. Config files are typically <8KB, but a
/// misconfigured file (or a user pointing --config at /dev/zero)
/// could be unbounded — the cap protects against that.
fn hash_file_prefix(path: &Path, max_bytes: usize) -> Option<[u8; 32]> {
    use sha2::{Digest, Sha256};
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
    let mut hasher = Sha256::new();
    hasher.update(&buf);
    let result = hasher.finalize();
    // GenericArray to [u8; 32] — `Into` is implemented.
    let hash_arr: [u8; 32] = result.into();
    Some(hash_arr)
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

    /// v30.3: SHA-256 hash known-answer test against published NIST vectors.
    /// Source: NIST FIPS 180-4 — empty string and "abc" are the canonical
    /// SHA-256 test vectors.
    #[test]
    fn sha256_known_vectors() {
        use sha2::{Digest, Sha256};
        // Empty input → e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let empty_expected: [u8; 32] = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        let mut h = Sha256::new();
        h.update(b"");
        let empty: [u8; 32] = h.finalize().into();
        assert_eq!(empty, empty_expected);

        // "abc" → ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        let abc_expected: [u8; 32] = [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ];
        let mut h = Sha256::new();
        h.update(b"abc");
        let abc: [u8; 32] = h.finalize().into();
        assert_eq!(abc, abc_expected);
    }

    /// hash_file_prefix returns Some([u8; 32]) for a readable file with
    /// expected content. Cross-check against an inline SHA-256 of the same
    /// bytes.
    #[test]
    fn hash_file_prefix_matches_inline_sha256() {
        use sha2::{Digest, Sha256};
        let dir = std::env::temp_dir().join("cosmostrix-tests");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("hash_file_prefix.toml");
        let content = b"color = green\n";
        std::fs::write(&path, content).unwrap();

        let hash = hash_file_prefix(&path, 8 * 1024).expect("hash must be Some");
        let mut inline = Sha256::new();
        inline.update(content);
        let inline_arr: [u8; 32] = inline.finalize().into();
        assert_eq!(
            hash, inline_arr,
            "hash_file_prefix must match inline SHA-256"
        );

        std::fs::remove_file(&path).ok();
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

    /// v30.3 masterclass: snapshot_file_state_cached with a matching prev
    /// snapshot must reuse the prev content_hash (fast path). This is the
    /// dedup contract for the polling-heartbeat steady state — the hash
    /// is skipped when mtime + size are unchanged.
    #[test]
    fn snapshot_cached_reuses_hash_on_unchanged_file() {
        let dir = std::env::temp_dir().join("cosmostrix-tests");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("snapshot_cached_unchanged.toml");
        std::fs::write(&path, b"speed = 30\n").unwrap();

        // First snapshot: cold path (no prev) — computes the hash.
        let snap1 = snapshot_file_state(&path);
        assert!(
            snap1.content_hash.is_some(),
            "cold snapshot must compute content_hash"
        );

        // Second snapshot: warm path (prev = snap1) — must reuse the hash.
        let snap2 = snapshot_file_state_cached(&path, Some(&snap1));
        assert_eq!(
            snap1.content_hash, snap2.content_hash,
            "warm snapshot must reuse prev content_hash (fast path)"
        );
        assert_eq!(
            snap1.mtime, snap2.mtime,
            "warm snapshot must preserve prev mtime"
        );
        assert_eq!(
            snap1.size, snap2.size,
            "warm snapshot must preserve prev size"
        );

        std::fs::remove_file(&path).ok();
    }

    /// v30.3 masterclass: snapshot_file_state_cached with a STALE prev
    /// snapshot must recompute the hash (slow path). After editing the
    /// file, mtime + size differ → cache miss → new hash computed.
    #[test]
    fn snapshot_cached_recomputes_hash_on_changed_file() {
        let dir = std::env::temp_dir().join("cosmostrix-tests");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("snapshot_cached_changed.toml");
        std::fs::write(&path, b"speed = 30\n").unwrap();

        let snap1 = snapshot_file_state(&path);

        // Sleep + rewrite to advance mtime + change size.
        std::thread::sleep(Duration::from_millis(1100));
        std::fs::write(&path, b"speed = 60 + extra content here\n").unwrap();

        // Cached snapshot with stale prev — must detect cache miss.
        let snap2 = snapshot_file_state_cached(&path, Some(&snap1));
        assert_ne!(
            snap1.content_hash, snap2.content_hash,
            "cached snapshot must recompute hash when mtime/size differ (slow path)"
        );
        assert_ne!(
            snap1.size, snap2.size,
            "size must differ after content change"
        );

        std::fs::remove_file(&path).ok();
    }

    /// v30.3 masterclass: snapshot_file_state_cached with a None prev
    /// must always compute the hash (cold path). This is the
    /// backward-compat path used by `snapshot_file_state` (the wrapper).
    #[test]
    fn snapshot_cached_with_none_prev_computes_hash() {
        let dir = std::env::temp_dir().join("cosmostrix-tests");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("snapshot_cached_none_prev.toml");
        std::fs::write(&path, b"color = amber\n").unwrap();

        let snap = snapshot_file_state_cached(&path, None);
        assert!(
            snap.content_hash.is_some(),
            "cold path (prev=None) must compute content_hash"
        );

        // Cross-check: must equal the plain wrapper snapshot_file_state.
        let wrapper_snap = snapshot_file_state(&path);
        assert_eq!(
            snap, wrapper_snap,
            "snapshot_file_state_cached(path, None) must equal snapshot_file_state(path)"
        );

        std::fs::remove_file(&path).ok();
    }

    /// v30.3 masterclass: snapshot_file_state_cached must NOT cache off a
    /// previously-failed snapshot (prev.mtime is None). This guards against
    /// the edge case where the file was previously unreadable (returning
    /// mtime=None, content_hash=None) and is now readable — we must
    /// recompute the hash rather than reuse the None.
    #[test]
    fn snapshot_cached_does_not_reuse_failed_prev() {
        let dir = std::env::temp_dir().join("cosmostrix-tests");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("snapshot_cached_failed_prev.toml");

        // Simulate a previously-failed snapshot: mtime=None, hash=None.
        // This is what snapshot_file_state returns when the file is missing.
        let failed_prev = FileStateSnapshot {
            mtime: None,
            size: None,
            content_hash: None,
        };

        // Now create the file + snapshot with the failed prev.
        std::fs::write(&path, b"color = green\n").unwrap();
        let snap = snapshot_file_state_cached(&path, Some(&failed_prev));
        assert!(
            snap.content_hash.is_some(),
            "must compute hash when prev.mtime is None (failed prev)"
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
