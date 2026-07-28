// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Polling heartbeat for live config reload — split out of `live_config.rs`
//! so that file stays under the 1200-LOC source cap enforced by
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
use std::sync::mpsc::Sender;
use std::time::Duration;

use notify::EventKind;

// Bring the lr_trace! macro into scope (declared in main.rs via
// #[macro_use] on mod live_config_trace).
#[allow(unused_imports)]
use crate::live_config_trace::*;

/// Polling heartbeat: checks file mtime/size/content every `interval_ms`
/// and feeds synthetic notify events into `tx` when ANY of them changes.
/// This runs on a background thread alongside the native watcher,
/// guaranteeing live reload works even when the native backend is silent
/// (e.g., FreeBSD kqueue feature not active, Android Termux inotify
/// throttling, restricted containers).
///
/// The synthetic event uses `EventKind::Modify(ModifyKind::Any)` with
/// the target file as the path, so the unified event loop in
/// `watcher_loop` (in `live_config.rs`) treats it identically to a
/// native modify event.
///
/// **Startup reload prevention**: all three signals are snapshotted at
/// heartbeat start. The first poll (`interval_ms` later) compares
/// against these initial values — if nothing changed, no event is sent.
pub fn polling_heartbeat(
    path: std::path::PathBuf,
    tx: Sender<notify::Result<notify::Event>>,
    interval_ms: u64,
) {
    // Snapshot the initial state. Each field is `Option` because any
    // individual signal may be unavailable (e.g., `modified()` Err on
    // FUSE). When a signal is `None`, it's treated as a distinct value
    // — `Some(t) != None`, so a transition from "mtime available" to
    // "mtime unavailable" registers as a change (the file may have been
    // replaced by an atomic save).
    let mut last_state = snapshot_file_state(&path);

    lr_trace!(
        "polling heartbeat started: interval={}ms initial={:?}",
        interval_ms,
        last_state
    );

    // v25.3: cycle counter for periodic liveness tracing. Every 5th
    // cycle (~3.75s at 750ms interval), emit a heartbeat trace so the
    // user can verify the polling thread is actually alive. Without
    // this, a dead polling thread produces ZERO trace output, making
    // Termux debugging impossible.
    let mut cycle: u64 = 0;

    loop {
        std::thread::sleep(Duration::from_millis(interval_ms));
        cycle += 1;

        let current_state = snapshot_file_state(&path);

        // v25.3: periodic liveness trace every 5 cycles. This is the
        // KEY diagnostic for Termux — if the user sees these lines,
        // the polling thread is alive and reading the file. If they
        // DON'T see them, the polling thread is dead/panicked.
        if cycle % 5 == 1 {
            lr_trace!(
                "poll cycle #{} alive — current_state={:?}",
                cycle,
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
}
