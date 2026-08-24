// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tier 2: xterm.js byte-budget window.
//!
//! This module isolates the rolling byte-budget logic that
//! `Terminal::flush_ansi` uses to apply preemptive backpressure when
//! running inside an xterm.js-based Electron host (VSCode, Hyper,
//! WaveTerminal, Tabby, WarpTerminal). All xterm.js hosts share the
//! same unbounded-buffer-growth failure mode: at high ANSI byte rates,
//! xterm.js's in-memory scrollback grows without bound over multi-hour
//! runs until V8 hits an OOM assertion (SIGTRAP).
//!
//! See `docs/SECURITY_AUDIT.md` §12a (Tier 2 Extension) for the full
//! rationale and threshold sizing. The constants live in `constants.rs`:
//!
//! - `XTERMJS_BYTE_BUDGET_PER_WINDOW` (40 MB): per-window byte budget.
//! - `XTERMJS_BYTE_BUDGET_WINDOW_FRAMES` (600 frames): window length.
//! - `XTERMJS_RIS_RESET_BYTES` (50 MB): cumulative bytes that trigger
//!   an ESC c (RIS) emission to clear xterm.js's scrollback buffer.
//! - `XTERMJS_HARD_CEILING_BYTES` (200 MB): defensive last-resort
//!   ceiling that forces a RIS regardless of window-budget state.
//!
//! `ByteWindow` is a fixed-capacity ring buffer of per-frame byte
//! counts. `push(bytes)` records a frame's contribution; `sum()`
//! returns the current window total. The ring is `Option<u64>` so the
//! initial state (fewer frames than the window size) is distinguishable
//! from a real zero-byte frame -- though for budget purposes both
//! yield sum 0.
//!
//! Cost: O(1) per push, O(N) per sum where N = window size. With N=600
//! and a sum only computed when `xtermjs_host` is true, the amortized
//! cost is ~600 additions every frame, roughly 2.4 us -- negligible vs
//! the flush itself (~50 us typical).

use crate::constants::{
    XTERMJS_BYTE_BUDGET_PER_WINDOW, XTERMJS_HARD_CEILING_BYTES, XTERMJS_RIS_RESET_BYTES,
};

/// Rolling byte-budget window for xterm.js hosts.
///
/// Tracks ANSI bytes flushed over the last `XTERMJS_BYTE_BUDGET_WINDOW_FRAMES`
/// frames. When the window sum exceeds `XTERMJS_BYTE_BUDGET_PER_WINDOW`,
/// `flush_ansi` suppresses the next flush (preemptive backpressure) to
/// let xterm.js drain its in-memory buffer.
///
/// See module-level docs for the full rationale.
pub(crate) struct ByteWindow {
    /// Ring buffer of per-frame byte counts. `None` slots are pre-init.
    slots: Vec<Option<u64>>,
    /// Index of the next slot to write. Wraps modulo `slots.len()`.
    head: usize,
}

impl ByteWindow {
    /// Construct a window of `capacity` slots. All slots start as `None`.
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: vec![None; capacity],
            head: 0,
        }
    }

    /// Record a frame's byte contribution, overwriting the oldest entry.
    /// `bytes` should be the count of ANSI bytes actually flushed (0 if
    /// the flush was suppressed -- that's still a useful signal: it
    /// means backpressure was applied and the budget should naturally
    /// recover).
    pub(crate) fn push(&mut self, bytes: u64) {
        if self.slots.is_empty() {
            return;
        }
        self.slots[self.head] = Some(bytes);
        self.head = (self.head + 1) % self.slots.len();
    }

    /// Sum all recorded slots. `None` slots (pre-init) contribute 0.
    /// Returns 0 if the window has no capacity (degenerate config).
    pub(crate) fn sum(&self) -> u64 {
        self.slots.iter().filter_map(|&slot| slot).sum()
    }

    /// Reset all slots to `None`. Used after a RIS reset, since the
    /// window's purpose (predict OOM pressure) is reset along with
    /// xterm.js's actual buffer.
    pub(crate) fn reset(&mut self) {
        // v50 Rust 1.98.0 bump: clippy::manual_slice_fill now
        // flags the manual `for slot in &mut self.slots { *slot = None; }`
        // pattern (new lint added in 1.98.0). The slice::fill method
        // (stable since Rust 1.50) is the idiomatic replacement and
        // generates identical codegen.
        self.slots.fill(None);
        self.head = 0;
    }

    /// Number of slots currently filled. Used in tests to verify the
    /// ring wrap behavior.
    #[cfg(test)]
    pub(crate) fn filled(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }
}

/// Returns true if the given window-sum + cumulative-since-RIS pair
/// should trigger preemptive backpressure (suppress the next flush).
///
/// Two conditions trigger backpressure:
/// 1. The rolling window sum exceeds the per-window budget.
/// 2. The cumulative bytes since last RIS exceed the hard ceiling
///    (defensive last-resort -- should never fire in practice since
///    the RIS reset at 50 MB fires first, but exists for pathological
///    cases like a single 250 MB full-redraw frame).
///
/// Extracted as a free function so it can be unit-tested without
/// constructing a full `Terminal` (which requires a real TTY).
pub(crate) fn should_backpressure(window_sum: u64, bytes_since_ris: u64) -> bool {
    window_sum >= XTERMJS_BYTE_BUDGET_PER_WINDOW || bytes_since_ris >= XTERMJS_HARD_CEILING_BYTES
}

/// Returns true if the given cumulative-bytes-since-RIS + upcoming-
/// frame-bytes pair should trigger a RIS reset (ESC c emission).
///
/// Two conditions trigger RIS:
/// 1. The cumulative bytes (including the upcoming frame) cross the
///    RIS reset threshold (50 MB).
/// 2. The cumulative bytes since last RIS (excluding the upcoming
///    frame) already exceed the hard ceiling (200 MB). This catches
///    the case where a single frame is so large it skipped past the
///    RIS threshold entirely.
pub(crate) fn should_ris_reset(cumulative_with_frame: u64, bytes_since_ris: u64) -> bool {
    cumulative_with_frame >= XTERMJS_RIS_RESET_BYTES
        || bytes_since_ris >= XTERMJS_HARD_CEILING_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::XTERMJS_BYTE_BUDGET_WINDOW_FRAMES;

    /// The byte budget threshold must be larger than a single frame's
    /// typical ANSI output (~140 KB for a dense dirty-all frame) so a
    /// single frame never triggers backpressure. The budget is sized
    /// for the multi-frame sustained-rate case, not the single-frame
    /// spike case (which is handled by the FPS cap instead).
    #[test]
    fn tier2_byte_budget_larger_than_single_frame() {
        let budget = std::hint::black_box(XTERMJS_BYTE_BUDGET_PER_WINDOW);
        // 256 KiB dense-frame worst case (matches STDOUT_BUF_CAPACITY).
        const SINGLE_FRAME_WORST_CASE: u64 = 256 * 1024;
        assert!(
            budget > SINGLE_FRAME_WORST_CASE,
            "byte budget ({budget}) must exceed single-frame worst case ({SINGLE_FRAME_WORST_CASE}); otherwise a single dense frame would trigger backpressure, causing permanent stutter"
        );
    }

    /// The RIS reset threshold must be smaller than the hard ceiling so
    /// RIS fires first (the ceiling is the defensive last-resort).
    #[test]
    fn tier2_ris_threshold_below_hard_ceiling() {
        let ris = std::hint::black_box(XTERMJS_RIS_RESET_BYTES);
        let ceil = std::hint::black_box(XTERMJS_HARD_CEILING_BYTES);
        assert!(
            ris < ceil,
            "RIS reset threshold ({ris}) must be below hard ceiling ({ceil}); RIS is the primary defense, ceiling is the last-resort backup"
        );
    }

    /// The byte budget window must be wide enough to smooth over single-
    /// frame spikes but narrow enough to catch sustained high-rate output
    /// before xterm.js's buffer crosses 100 MB.
    #[test]
    fn tier2_window_frames_in_reasonable_range() {
        let window = std::hint::black_box(XTERMJS_BYTE_BUDGET_WINDOW_FRAMES);
        // Lower bound: 60 frames ~ 1 s at 60 FPS -- too narrow would
        // trigger backpressure on transient spikes (palette changes,
        // resize events).
        assert!(
            window >= 60,
            "byte-budget window must be at least 60 frames (1s at 60fps) to smooth over spikes"
        );
        // Upper bound: 3600 frames ~ 60 s -- too wide would let xterm.js
        // accumulate >100 MB before backpressure kicks in.
        assert!(
            window <= 3600,
            "byte-budget window must be at most 3600 frames (60s) to catch sustained high-rate output before xterm.js's buffer crosses 100 MB"
        );
    }

    /// ByteWindow ring buffer: pushes record bytes in order, overwriting
    /// the oldest entry once capacity is reached.
    #[test]
    fn byte_window_pushes_overwrite_oldest() {
        let mut w = ByteWindow::with_capacity(3);
        // Initial state: 0 filled.
        assert_eq!(w.filled(), 0);
        assert_eq!(w.sum(), 0);

        w.push(100);
        w.push(200);
        w.push(300);
        assert_eq!(w.filled(), 3);
        assert_eq!(w.sum(), 600);

        // Fourth push overwrites the first entry (100) with 400.
        w.push(400);
        assert_eq!(w.filled(), 3);
        assert_eq!(w.sum(), 900); // 200 + 300 + 400
    }

    /// ByteWindow sum skips None slots (pre-init state).
    #[test]
    fn byte_window_sum_skips_none_slots() {
        let mut w = ByteWindow::with_capacity(5);
        w.push(100);
        w.push(200);
        // Only 2 of 5 slots filled -- sum should be 300, not 5*N.
        assert_eq!(w.filled(), 2);
        assert_eq!(w.sum(), 300);
    }

    /// ByteWindow reset clears all slots back to None.
    #[test]
    fn byte_window_reset_clears_slots() {
        let mut w = ByteWindow::with_capacity(3);
        w.push(100);
        w.push(200);
        assert_eq!(w.filled(), 2);
        assert_eq!(w.sum(), 300);

        w.reset();
        assert_eq!(w.filled(), 0);
        assert_eq!(w.sum(), 0);

        // Pushing after reset starts fresh.
        w.push(50);
        assert_eq!(w.filled(), 1);
        assert_eq!(w.sum(), 50);
    }

    /// ByteWindow with 0 capacity is a no-op (degenerate config, but
    /// must not panic).
    #[test]
    fn byte_window_zero_capacity_is_noop() {
        let mut w = ByteWindow::with_capacity(0);
        w.push(100);
        assert_eq!(w.sum(), 0);
        w.reset(); // must not panic
    }

    /// ByteWindow push of 0 bytes is a valid signal (means backpressure
    /// was applied this frame). The 0 is recorded, not skipped -- it
    /// ages out the oldest entry just like any other push.
    #[test]
    fn byte_window_zero_byte_push_ages_out_oldest() {
        let mut w = ByteWindow::with_capacity(2);
        w.push(1000);
        w.push(2000);
        assert_eq!(w.sum(), 3000);

        // Push 0 (suppressed frame) -- should overwrite the 1000, not
        // be ignored. This is the backpressure-recovery mechanism:
        // suppressed frames age out old high-byte entries, naturally
        // bringing the window sum back under budget.
        w.push(0);
        assert_eq!(w.sum(), 2000); // 2000 + 0
    }

    /// Simulate the budget-recovery loop: a high-byte burst pushes the
    /// window over budget, then a series of 0-byte pushes (suppressed
    /// flushes) brings it back under. This is the exact behavior the
    /// Tier 2 backpressure path relies on for self-healing.
    #[test]
    fn byte_window_budget_recovery_loop() {
        let mut w = ByteWindow::with_capacity(4);
        // Burst: 4 frames of 12 MB each (48 MB total -- exceeds the 40 MB
        // budget). The backpressure path would suppress the 5th frame.
        w.push(12 * 1024 * 1024);
        w.push(12 * 1024 * 1024);
        w.push(12 * 1024 * 1024);
        w.push(12 * 1024 * 1024);
        let budget = XTERMJS_BYTE_BUDGET_PER_WINDOW;
        assert!(w.sum() > budget, "burst should exceed budget");

        // 4 suppressed frames (0 bytes each) age out the burst.
        for _ in 0..4 {
            w.push(0);
        }
        assert_eq!(w.sum(), 0, "after 4 suppressed frames, window must be 0");
        assert!(w.sum() < budget, "window must be back under budget");
    }

    /// `should_backpressure` returns true when window sum exceeds budget.
    #[test]
    fn should_backpressure_fires_when_window_exceeds_budget() {
        let over = XTERMJS_BYTE_BUDGET_PER_WINDOW + 1;
        assert!(
            should_backpressure(over, 0),
            "window sum ({over}) > budget must trigger backpressure"
        );
    }

    /// `should_backpressure` returns true when bytes_since_ris hits the
    /// hard ceiling (defensive last-resort path).
    #[test]
    fn should_backpressure_fires_at_hard_ceiling() {
        let ceil = XTERMJS_HARD_CEILING_BYTES;
        assert!(
            should_backpressure(0, ceil),
            "bytes_since_ris ({ceil}) >= hard ceiling must trigger backpressure"
        );
    }

    /// `should_backpressure` returns false in the steady state (well
    /// under both thresholds).
    #[test]
    fn should_backpressure_false_in_steady_state() {
        assert!(
            !should_backpressure(0, 0),
            "fresh state (0, 0) must not trigger backpressure"
        );
        // A modest window sum (1 MB) with low cumulative (1 MB) should
        // be well under both thresholds.
        assert!(
            !should_backpressure(1024 * 1024, 1024 * 1024),
            "modest byte counts must not trigger backpressure"
        );
    }

    /// `should_ris_reset` returns true when cumulative + frame crosses
    /// the RIS threshold.
    #[test]
    fn should_ris_reset_fires_at_threshold() {
        let just_over = XTERMJS_RIS_RESET_BYTES;
        assert!(
            should_ris_reset(just_over, 0),
            "cumulative ({just_over}) >= RIS threshold must trigger reset"
        );
    }

    /// `should_ris_reset` returns true when bytes_since_ris alone hits
    /// the hard ceiling (the path that catches single-frame spikes
    /// skipping past the RIS threshold).
    #[test]
    fn should_ris_reset_fires_at_hard_ceiling() {
        let ceil = XTERMJS_HARD_CEILING_BYTES;
        assert!(
            should_ris_reset(0, ceil),
            "bytes_since_ris ({ceil}) >= hard ceiling must trigger RIS reset"
        );
    }

    /// `should_ris_reset` returns false in the steady state.
    #[test]
    fn should_ris_reset_false_in_steady_state() {
        assert!(
            !should_ris_reset(0, 0),
            "fresh state (0, 0) must not trigger RIS reset"
        );
        // 10 MB cumulative is well under the 50 MB RIS threshold.
        assert!(
            !should_ris_reset(10 * 1024 * 1024, 5 * 1024 * 1024),
            "modest cumulative must not trigger RIS reset"
        );
    }

    /// `should_ris_reset` is monotonic in `cumulative_with_frame`:
    /// once it returns true, increasing the value further must not
    /// flip it back to false.
    #[test]
    fn should_ris_reset_is_monotonic_in_cumulative() {
        let threshold = XTERMJS_RIS_RESET_BYTES;
        // Just below threshold: false.
        assert!(!should_ris_reset(threshold - 1, 0));
        // At threshold: true.
        assert!(should_ris_reset(threshold, 0));
        // Well past threshold: still true.
        assert!(should_ris_reset(threshold * 10, 0));
    }
}
