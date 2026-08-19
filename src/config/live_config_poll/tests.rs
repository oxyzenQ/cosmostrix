// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! live_config_poll tests, extracted from inline `mod tests { ... }` block.
//!
//! Uses `use super::*;` to access parent's private items unchanged.

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

/// v50: SHA-512 hash known-answer test against published NIST vectors.
/// Source: NIST FIPS 180-4 — empty string and "abc" are the canonical
/// SHA-512 test vectors.
#[test]
fn sha512_known_vectors() {
    use sha2::{Digest, Sha512};
    // Empty input → cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e
    let empty_expected: [u8; 64] = [
        0xcf, 0x83, 0xe1, 0x35, 0x7e, 0xef, 0xb8, 0xbd, 0xf1, 0x54, 0x28, 0x50, 0xd6, 0x6d, 0x80,
        0x07, 0xd6, 0x20, 0xe4, 0x05, 0x0b, 0x57, 0x15, 0xdc, 0x83, 0xf4, 0xa9, 0x21, 0xd3, 0x6c,
        0xe9, 0xce, 0x47, 0xd0, 0xd1, 0x3c, 0x5d, 0x85, 0xf2, 0xb0, 0xff, 0x83, 0x18, 0xd2, 0x87,
        0x7e, 0xec, 0x2f, 0x63, 0xb9, 0x31, 0xbd, 0x47, 0x41, 0x7a, 0x81, 0xa5, 0x38, 0x32, 0x7a,
        0xf9, 0x27, 0xda, 0x3e,
    ];
    let mut h = Sha512::new();
    h.update(b"");
    let empty: [u8; 64] = h.finalize().into();
    assert_eq!(empty, empty_expected);

    // "abc" → ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f
    // (NIST FIPS 180-4 SHA-512("abc") vector)
    let abc_expected: [u8; 64] = [
        0xdd, 0xaf, 0x35, 0xa1, 0x93, 0x61, 0x7a, 0xba, 0xcc, 0x41, 0x73, 0x49, 0xae, 0x20, 0x41,
        0x31, 0x12, 0xe6, 0xfa, 0x4e, 0x89, 0xa9, 0x7e, 0xa2, 0x0a, 0x9e, 0xee, 0xe6, 0x4b, 0x55,
        0xd3, 0x9a, 0x21, 0x92, 0x99, 0x2a, 0x27, 0x4f, 0xc1, 0xa8, 0x36, 0xba, 0x3c, 0x23, 0xa3,
        0xfe, 0xeb, 0xbd, 0x45, 0x4d, 0x44, 0x23, 0x64, 0x3c, 0xe8, 0x0e, 0x2a, 0x9a, 0xc9, 0x4f,
        0xa5, 0x4c, 0xa4, 0x9f,
    ];
    let mut h = Sha512::new();
    h.update(b"abc");
    let abc: [u8; 64] = h.finalize().into();
    assert_eq!(abc, abc_expected);
}

/// hash_file_prefix returns Some([u8; 64]) for a readable file with
/// expected content. Cross-check against an inline SHA-512 of the same
/// bytes.
#[test]
fn hash_file_prefix_matches_inline_sha512() {
    use sha2::{Digest, Sha512};
    let dir = std::env::temp_dir().join("cosmostrix-tests");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join("hash_file_prefix.toml");
    let content = b"color = green\n";
    std::fs::write(&path, content).unwrap();

    let hash = hash_file_prefix(&path, 8 * 1024).expect("hash must be Some");
    let mut inline = Sha512::new();
    inline.update(content);
    let inline_arr: [u8; 64] = inline.finalize().into();
    assert_eq!(
        hash, inline_arr,
        "hash_file_prefix must match inline SHA-512"
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
        eprintln!("[warn] snapshot mtime is None — running on a filesystem without mtime support?");
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

/// masterclass: snapshot_file_state_cached with a matching prev
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

/// masterclass: snapshot_file_state_cached with a STALE prev
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

/// masterclass: snapshot_file_state_cached with a None prev
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

/// masterclass: snapshot_file_state_cached must NOT cache off a
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

/// strengthening: worst-case burst duration must stay under 3s.
#[test]
fn burst_cycle_constants_are_sane() {
    // 10 cycles × 200ms = 2s ≤ 3s.
    assert!(
        BURST_CYCLES_MAX as u64 * BURST_POLL_INTERVAL_MS <= 3000,
        "worst-case burst must stay under 3s"
    );
}

// ── (bug #18): polling-heartbeat end-to-end test ──────────────
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
/// (bug #18): raised from the previous informal 5s target to
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
    let (tx, rx) = mpsc::sync_channel::<notify::Result<notify::Event>>(64);
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
    // (bug #18): under heavy parallel test load (especially
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
    // (bug #18): WITHOUT this final write, `handle.join()`
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
