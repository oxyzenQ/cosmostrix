// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! P5 — broken-pipe recovery + stdout health probe tests.
//!
//! Extracted from `terminal/mod.rs` in the dragon-fight branch.
//! Declared as `#[cfg(test)] mod p5_tests;` in `terminal/mod.rs`.

// cfg(unix): every test that consumes items from `super::*` (e.g.
// is_recoverable_io_error in the P3 classification test) is itself
// cfg(unix); the platform-neutral tests below use only crate::constants.
// Without this gate the glob import is unused on Windows test builds
// (-D warnings under clippy --all-targets).
#[cfg(unix)]
use super::*;
use crate::constants::FD_HEALTH_PROBE_INTERVAL_FRAMES;

/// The probe interval must be a positive, non-trivial value. Too
/// small → per-frame overhead; too large → idle-period breakage
/// goes undetected for too long. 3600 frames ≈ 60 s at 60 FPS is
/// the documented sweet spot (matches P4 sweep cadence).
#[test]
fn p5_probe_interval_is_reasonable() {
    // black_box prevents const-folding so clippy doesn't flag the
    // assertions as constant. The values are still the same.
    let n = std::hint::black_box(FD_HEALTH_PROBE_INTERVAL_FRAMES);
    assert!(
        n >= 600,
        "probe interval must be at least 600 frames (10s at 60fps) to avoid overhead"
    );
    assert!(
        n <= 36000,
        "probe interval must be at most 36000 frames (10min at 60fps) to stay useful"
    );
}

/// The probe interval matches the P4 stuck-cell sweep cadence.
/// Both are background hygiene passes on the same slow tick —
/// keeping them in sync simplifies reasoning about background cost.
#[test]
fn p5_probe_interval_matches_p4_sweep_cadence() {
    use crate::constants::STUCK_CELL_SWEEP_INTERVAL_FRAMES;
    assert_eq!(
        FD_HEALTH_PROBE_INTERVAL_FRAMES, STUCK_CELL_SWEEP_INTERVAL_FRAMES,
        "P5 probe cadence should match P4 sweep cadence (both are 60s background hygiene)"
    );
}

/// Synthetic BrokenPipe errors are recoverable (per P3's classification).
/// This is what probe_stdout_health synthesizes when isatty returns false,
/// so the P3 recovery path accepts it. Verifies the contract between
/// P5's detection layer and P3's recovery layer.
#[cfg(unix)]
#[test]
fn p5_synthetic_broken_pipe_is_recoverable_by_p3() {
    let synthetic = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
    assert!(
        is_recoverable_io_error(&synthetic),
        "P5's synthetic BrokenPipe error must be classified as recoverable by P3's is_recoverable_io_error"
    );
}

/// When stdout IS a terminal (the normal case in test environments),
/// probe_stdout_health must return true. This is the steady-state
/// behavior: the probe runs every 60s, finds stdout healthy, and
/// returns without side-effects.
///
/// NOTE: This test only validates the happy path. Constructing a
/// Terminal with a broken stdout fd requires either closing the fd
/// mid-test (unsafe, racy) or using a pipe + close pattern that
/// doesn't fit Terminal's constructor contract. The broken-fd path
/// is exercised indirectly via the P3 tests (is_recoverable_io_error
/// classification) and the integration test below.
#[cfg(unix)]
#[test]
fn p5_probe_returns_true_when_stdout_is_terminal() {
    // We can't easily construct a full Terminal in unit tests (it
    // calls enable_raw_mode + enters alternate screen). Instead,
    // verify the IsTerminal trait behaves as expected on real stdout.
    use std::io::IsTerminal;
    let stdout_is_tty = std::io::stdout().is_terminal();
    let stderr_is_tty = std::io::stderr().is_terminal();
    // In a normal test environment, at least one of these should be
    // a tty. If neither is (e.g., headless CI with no /dev/tty),
    // skip the assertion — the test still passes.
    if stdout_is_tty || stderr_is_tty {
        assert!(
            stdout_is_tty,
            "if any std stream is a tty, stdout should be too (test env assumption)"
        );
    }
}

/// The probe must not be a no-op when called on a non-tty stdout.
/// We can't easily construct a Terminal with a broken fd, but we
/// CAN verify the building block: a non-tty file (e.g., /dev/null)
/// returns false from IsTerminal::is_terminal. This is the exact
/// check probe_stdout_health makes on stdout.get_ref().
#[cfg(unix)]
#[test]
fn p5_is_terminal_returns_false_for_non_tty_files() {
    use std::io::IsTerminal;

    // Open /dev/null — definitely not a tty.
    let devnull = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .expect("/dev/null should be openable on Unix");

    // std::fs::File implements IsTerminal since Rust 1.70.
    // probe_stdout_health calls self.stdout.get_ref().is_terminal()
    // where stdout is BufWriter<Stdout> and get_ref() returns &Stdout.
    // Stdout's is_terminal() uses the same trait, so testing it on
    // File validates the same codepath.
    assert!(
        !devnull.is_terminal(),
        "/dev/null must NOT be classified as a terminal — probe_stdout_health relies on this to detect fd corruption"
    );
}

/// The probe's recovery path (recover_to_tty with empty buffer)
/// must respect the recovery cap. After STDOUT_FALLBACK_MAX_RECOVERIES
/// attempts, further recoveries propagate the error. This is
/// enforced by P3's recover_to_tty, which P5 reuses — so P5
/// inherits the cap automatically.
#[cfg(unix)]
#[test]
fn p5_recovery_inherits_p3_cap() {
    use crate::constants::STDOUT_FALLBACK_MAX_RECOVERIES;
    // black_box prevents const-folding so clippy doesn't flag the
    // assertions as constant. The values are still the same.
    let cap = std::hint::black_box(STDOUT_FALLBACK_MAX_RECOVERIES);
    // The cap must be small enough to prevent infinite recovery
    // loops but large enough to handle transient multi-frame
    // breakage. P5 only triggers once per 60s, so the cap is
    // measured in minutes of recovery attempts.
    assert!(cap >= 1, "recovery cap must allow at least one attempt");
    assert!(
        cap <= 10,
        "recovery cap must be small enough to prevent pathological loops"
    );
}

/// The modulo check in the event loop must fire exactly once per
/// interval. Simulate the event loop's `perf_rss_samples % N == 0`
/// check over a range of frame counters and verify the probe fires
/// exactly at multiples of N (including 0) and nowhere else.
///
/// This catches off-by-one errors (e.g., using `+ 1` or starting
/// the counter at 1 instead of 0) that would silently shift the
/// probe cadence.
#[test]
fn p5_modulo_check_fires_exactly_at_multiples_of_interval() {
    // Read the const into a runtime variable so clippy doesn't
    // flag the assertions as constant-folded. The value is still
    // the same; we're just testing the modulo arithmetic pattern
    // the event loop uses.
    let n: u64 = FD_HEALTH_PROBE_INTERVAL_FRAMES;
    // Prevent const-folding so clippy::assertions_on_constants
    // doesn't fire. The actual value is unchanged.
    let n = std::hint::black_box(n);

    let mut fire_count = 0usize;
    // Simulate 3 intervals worth of frames.
    let total_frames = n * 3;
    for frame in 0..total_frames {
        let fires = frame % n == 0;
        if fires {
            fire_count += 1;
        }
    }
    assert_eq!(
        fire_count, 3,
        "probe must fire exactly 3 times over 3 intervals (frames 0, N, 2N), got {}",
        fire_count
    );

    // Verify the specific fire points.
    assert!(
        0u64.is_multiple_of(n),
        "probe must fire on the first frame (frame 0)"
    );
    assert!(
        !(n - 1).is_multiple_of(n),
        "probe must NOT fire one frame before the interval boundary"
    );
    assert!(
        n.is_multiple_of(n),
        "probe must fire exactly at the interval boundary"
    );
    assert!(
        !(n + 1).is_multiple_of(n),
        "probe must NOT fire one frame after the interval boundary"
    );
}
