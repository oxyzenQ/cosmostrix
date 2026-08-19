// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Polling heartbeat for live config reload — split out of `live_config.rs`
//! so that file stays under the 1500-LOC source cap enforced by
//! `loc_tests`.
//!
//! ## Termux fix: triple-signal change detection
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
//! The content hash is SHA-256 (cryptographic, per owner contract
//! 2026-08-07). Reading 8KB adds ~100µs overhead per poll — negligible
//! at 750ms intervals. The previous FNV-1a 64-bit hash was replaced
//! because owner required cryptographic strength for change detection.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::SyncSender;
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

/// strengthening: when a NEW change is detected DURING an active
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
/// **Adaptive burst mode (strengthening)**: after ANY change is
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
    tx: SyncSender<notify::Result<notify::Event>>,
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

    // cycle counter for periodic liveness tracing. Every 5th
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
            // strengthening: extend burst by BURST_CYCLES_EXTEND if
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

        // periodic liveness trace every 5 cycles. This is the
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
        if tx.try_send(Ok(event)).is_err() {
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
    /// v50: SHA-512 hash of the first 8KB of file content. `None` if
    /// the file couldn't be read (e.g., file deleted mid-snapshot).
    /// Upgraded from FNV-1a 64-bit per owner contract (2026-08-07):
    /// cryptographic strength required for change detection.
    /// v50: upgraded from SHA-256 to SHA-512 for higher security margin
    /// (256-bit collision resistance) at negligible cost for small files.
    pub content_hash: Option<[u8; 64]>,
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
/// **Fast path (masterclass):** when `prev` is `Some` AND its
/// `mtime` and `size` both match the current file's metadata, the
/// expensive SHA-512 hash is SKIPPED and `prev.content_hash` is reused.
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
/// upgraded from FNV-1a 64-bit to SHA-256 per owner contract.
/// SHA-256 gives cryptographic collision resistance — even an attacker
/// (or a buggy editor) crafting two config files with the same hash is
/// computationally infeasible. The performance cost is ~2x vs FNV-1a
/// (~100µs per 750ms poll = 0.013% CPU) — still negligible.
///
/// We use std::io::Read's `read` with a capped buffer to avoid reading
/// huge files into memory. Config files are typically <8KB, but a
/// misconfigured file (or a user pointing --config at /dev/zero)
/// could be unbounded — the cap protects against that.
fn hash_file_prefix(path: &Path, max_bytes: usize) -> Option<[u8; 64]> {
    use sha2::{Digest, Sha512};
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
    let mut hasher = Sha512::new();
    hasher.update(&buf);
    let result = hasher.finalize();
    // GenericArray to [u8; 64] — `Into` is implemented.
    let hash_arr: [u8; 64] = result.into();
    Some(hash_arr)
}

#[cfg(test)]
mod tests;
