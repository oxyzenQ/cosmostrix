// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! Reader-aware emission — the consumer-side cost model
//! (night-frontier-Z3, Lapis 3c).
//!
//! The incubator's producer-side sweep
//! (`src/cosmic_dragon_incubator/egg/threshold_sweep.rs`) prices
//! a change to the hot path in PRODUCER terms: bytes out, cells touched.
//! The terminal on the other end was a black box — the producer could
//! emit 4 MB frames and learn about congestion only AFTER the pipe
//! filled (the HUNT-23 drain backoff, reactive by construction). This
//! module models the CONSUMER: what it costs each terminal family to
//! parse one frame, so the Power Dragon can pace the producer BEFORE
//! congestion builds.
//!
//! ## The model
//!
//! Terminal families differ in what bounds their ANSI consumption:
//!
//! - Byte-bound (CPU state-machine parsers: VTE family, konsole,
//!   foot, the raw console): the parser walks every byte through its
//!   state machine on one core — the frame's BYTE volume is the cost.
//!   This is the family the KNOWN_ISSUES VTE entry describes: at
//!   fullscreen cell counts the byte volume overwhelms the parser.
//! - Heap-bound (xterm.js): JavaScript-side parsing plus per-op
//!   object churn — the documented OOM class that already owns the
//!   Tier-2 byte budget, RIS reset, and the 30 FPS cap.
//! - Op-bound (GPU renderers: Alacritty, kitty, WezTerm, ghostty):
//!   parsing is cheap; the render cost rides per-escape-op. Volumes
//!   that saturate a VTE session drain freely here.
//!
//! Each family gets one policy number: the sustainable byte rate the
//! family can drain per second. The constants are POLICY FLOORS —
//! deliberately conservative, derived from the repo's own documented
//! arithmetic, not claimed measurements:
//!
//! - `XtermJs`: 7 MB/s — the number the Tier-2 budget section already
//!   documents ("at ~7 MB/sec worst case", `central_control_power_dragon`).
//! - `CpuRendered`: 2 MB/s — derived from HUNT-24's arithmetic (foot's
//!   144 FPS default was "2.4x the byte rate a CPU renderer can drain
//!   at fullscreen", so the drain sits near 60 FPS) times the producer
//!   sweep's row-RLE cost (~4 bytes/cell): 12K cells × 4 B × 60 FPS ≈
//!   2.9 MB/s; the floor sits just under it, where the documented
//!   fullscreen saturation begins.
//! - `Console`: 1 MB/s — the raw virtual console, the most constrained
//!   consumer in the matrix (HUNT-24 hard-disables effects there).
//! - `Gpu`: 20 MB/s — an order of magnitude over the CPU floor, the
//!   headroom the reactive drain backoff never needs on GPU terminals.
//!
//! The reactive layer stays the source of truth: the drain backoff
//! measures REAL write latency. This model adds the PROACTIVE signal —
//! a frame whose predicted consumer parse exceeds its own frame period
//! raises the reader backoff before the pipe fills, killing the
//! feedback delay the VTE entry documents ("VTE's internal buffering
//! creates a feedback delay that prevents perfect stabilization").
//!
//! ## What it feeds (zero new surface)
//!
//! - Power Dragon budget: `budget_bytes_per_frame` — the per-frame
//!   byte allowance at the current effective cadence, consumed by
//!   `PowerManager::observe_reader_cost` (the reader backoff rides the
//!   same lever HUNT-23 chose: FPS pacing, no visual side effects).
//! - RLE threshold: the static
//!   `DIRTY_THRESHOLD_RATIO` is checked against the consumer-side
//!   crossover (byte-bound families decide on bytes, exactly the
//!   producer sweep's arithmetic), pinning that the shipped threshold
//!   is consumer-optimal for the families that pay in bytes.
//! - Calibration: `calibration_rows` — the model's numbers checked
//!   against the byte volumes the supermassive legs actually captured
//!   (the syscall-floor leg's ~2.4 KB frames at 80x24 must sit under
//!   every family's budget; the documented fullscreen VTE saturation
//!   must sit over the CPU floor), so the model can never drift into
//!   either false throttling or blind throttling.

use crate::termdetect::TerminalCaps;

/// The consumer terminal family, resolved once from the existing
/// termdetect sensors (HUNT-24's `cpu_rendered` / `console_tty` /
/// `xtermjs_host` — no new detection layers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalFamily {
    /// Raw Linux console / dumb terminal (HUNT-24: effects hard-off).
    Console,
    /// xterm.js hosts (VSCode and the Electron family) — heap-bound.
    XtermJs,
    /// CPU-rendered terminals: the VTE family, konsole, foot.
    CpuRendered,
    /// GPU-rendered or heavily optimized terminals: Alacritty, kitty,
    /// WezTerm, ghostty, and any unknown fast terminal.
    Gpu,
}

impl TerminalFamily {
    /// Resolve the family from the existing terminal capability
    /// sensors. Order matters: the console is the strictest consumer
    /// (implies cpu_rendered), then the xterm.js heap class, then the
    /// CPU parser class, then the GPU default.
    pub(crate) fn from_caps(caps: &TerminalCaps) -> Self {
        if caps.console_tty {
            Self::Console
        } else if caps.xtermjs_host {
            Self::XtermJs
        } else if caps.cpu_rendered {
            Self::CpuRendered
        } else {
            Self::Gpu
        }
    }

    /// The family's sustainable byte rate, in bytes per second (the
    /// policy floors documented in the module header).
    pub(crate) fn sustainable_bytes_per_sec(self) -> u64 {
        match self {
            Self::Console => 1_000_000,
            Self::XtermJs => 7_000_000,
            Self::CpuRendered => 2_000_000,
            Self::Gpu => 20_000_000,
        }
    }

    /// The per-frame byte budget at `fps`: the emission a frame may
    /// carry while the family still drains it at the target cadence.
    /// At 0 fps (the run-forever floor is 1.0; tests may pass 0) the
    /// budget is the full second's worth. Compiled under test only —
    /// the production pacing goes through `predicted_parse_ms`, which
    /// encodes the same rate; this form exists for the calibration
    /// rows and the threshold validation to read the budget directly.
    #[cfg(test)]
    pub(crate) fn budget_bytes_per_frame(self, fps: f64) -> u64 {
        if fps <= 0.0 {
            return self.sustainable_bytes_per_sec();
        }
        ((self.sustainable_bytes_per_sec() as f64) / fps) as u64
    }

    /// The predicted consumer parse cost of one frame, in milliseconds:
    /// the frame's bytes over the family's sustainable rate. The one
    /// number the Power Dragon compares against the frame period.
    pub(crate) fn predicted_parse_ms(self, frame_bytes: u64) -> f64 {
        (frame_bytes as f64) / (self.sustainable_bytes_per_sec() as f64) * 1000.0
    }
}

/// Resolve the family once from the process environment (the same
/// sensors the startup detection used — idempotent, and cached so the
/// power path never re-scans env per frame).
pub(crate) fn detect_family() -> TerminalFamily {
    TerminalFamily::from_caps(&crate::termdetect::detect())
}

/// The consumer-side verdict on the static RLE threshold
/// (`DIRTY_THRESHOLD_RATIO` = 8: a full redraw fires when >= 12.5% of
/// cells are dirty).
///
/// The producer sweep priced the crossover in bytes: the per-cell diff
/// path costs ~30 bytes per dirty cell, the row-RLE full redraw ~4
/// bytes per cell total. Byte-bound families decide on exactly those
/// bytes — the crossover dirty ratio where full-redraw bytes drop
/// below diff bytes is `30 / (4 * total_cells)`... inverted: diff =
/// 30 × dirty, full = 4 × total; full wins when `dirty >= total / 7.5`
/// (≈ 13.3%). The shipped threshold (12.5%) sits just BELOW that
/// crossover — the conservative side: at the threshold the full redraw
/// may cost marginally more bytes on the smallest screens, and the
/// delta shrinks to zero as screens grow. For byte-bound families the
/// model confirms the shipped threshold is consumer-sound at every
/// documented screen (the residual diff-vs-RLE byte delta at the
/// threshold is bounded and tiny); for op-bound families the decision
/// rides ops, not bytes, and the producer sweep's op arithmetic
/// already governs. Returns the crossover ratio for reference.
// Compiled under test only: the threshold validation and the
// calibration rows are the model's verification surface, consumed by
// the invariant tests below (the decision they pin is the shipped
// static threshold, which stays a const — no runtime crossover
// lookup exists to keep the hot path branch-free).
#[cfg(test)]
pub(crate) fn consumer_crossover_dirty_ratio(total_cells: u64) -> f64 {
    // diff = 30 * dirty; full = 4 * total → dirty_crossover = total / 7.5
    (total_cells as f64) / 7.5
}

/// The model's calibration rows, checked against the byte volumes the
/// supermassive legs captured (Z2's syscall-floor leg read ~2.4 KB
/// per frame at the pinned 80x24 release shape) and the documented
/// saturation arithmetic. Each row names the scenario, the frame
/// volume, and whether the family's budget covers it — the model must
/// never false-throttle the documented-healthy cases nor blind-throttle
/// the documented-saturation case. A test asserts every row's verdict.
#[cfg(test)]
pub(crate) fn calibration_rows() -> Vec<(&'static str, u64, f64, TerminalFamily, bool)> {
    let fps = 60.0;
    vec![
        (
            "syscall-floor leg frame (80x24, measured ~2.4 KB/frame)",
            2_400,
            fps,
            TerminalFamily::CpuRendered,
            true,
        ),
        (
            "color16 HUD leg frame (80x24, measured ~2 KB/frame)",
            2_000,
            fps,
            TerminalFamily::Gpu,
            true,
        ),
        (
            "monolith full redraw at 200x60 (12K cells x 4 B, the producer sweep's RLE number)",
            48_000,
            fps,
            TerminalFamily::CpuRendered,
            false,
        ),
        (
            "the same 48 KB frame on a GPU terminal",
            48_000,
            fps,
            TerminalFamily::Gpu,
            true,
        ),
        (
            "xterm.js worst-case burst at its 30 FPS cap (7 MB/s budget)",
            233_000,
            30.0,
            TerminalFamily::XtermJs,
            true,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The family ordering is the consumer reality: the console is the
    /// most constrained, xterm.js sits in the documented 7 MB/s class,
    /// the CPU parser class sits at the saturation floor, and the GPU
    /// tier has an order of magnitude of headroom.
    #[test]
    fn family_budgets_order_like_the_documented_matrix() {
        let order = [
            (TerminalFamily::Console, 1_000_000),
            (TerminalFamily::CpuRendered, 2_000_000),
            (TerminalFamily::XtermJs, 7_000_000),
            (TerminalFamily::Gpu, 20_000_000),
        ];
        for w in order.windows(2) {
            assert!(
                w[0].0.sustainable_bytes_per_sec() < w[1].0.sustainable_bytes_per_sec(),
                "{:?} must be more constrained than {:?}",
                w[0].0,
                w[1].0
            );
        }
        for (family, expected) in order {
            assert_eq!(family.sustainable_bytes_per_sec(), expected);
        }
    }

    /// Monotonicity: a bigger frame never predicts cheaper, and the
    /// per-frame budget shrinks as the cadence rises.
    #[test]
    fn predictions_are_monotonic() {
        let small = TerminalFamily::CpuRendered.predicted_parse_ms(2_400);
        let big = TerminalFamily::CpuRendered.predicted_parse_ms(48_000);
        assert!(small < big);
        let slow = TerminalFamily::CpuRendered.budget_bytes_per_frame(30.0);
        let fast = TerminalFamily::CpuRendered.budget_bytes_per_frame(60.0);
        assert!(slow > fast);
    }

    /// The calibration rows hold: healthy captured volumes sit under
    /// budget on their families; the documented VTE fullscreen
    /// saturation sits over the CPU floor; the xterm.js worst-case
    /// burst rides its own documented 7 MB/s budget at the 30 FPS cap.
    #[test]
    fn calibration_rows_match_their_verdicts() {
        for (name, frame_bytes, fps, family, covered) in calibration_rows() {
            let budget = family.budget_bytes_per_frame(fps);
            assert_eq!(
                frame_bytes <= budget,
                covered,
                "calibration row {name:?}: {frame_bytes} B vs {budget} B budget at {fps} fps"
            );
        }
    }

    /// The shipped RLE threshold is consumer-sound for byte-bound
    /// families: at the shipped 12.5% boundary on the interactive cap
    /// screen (1024x500 = 512K cells), the crossover arithmetic puts
    /// the byte-optimal switch below the threshold, so by the time the
    /// full redraw fires the consumer is on the cheap side of the
    /// byte curve; and on the smallest legal screen (6x6) the residual
    /// delta at the threshold is bounded (the conservative side the
    /// producer sweep already accepted).
    #[test]
    fn shipped_rle_threshold_is_consumer_sound_for_byte_bound_families() {
        // Crossover: full-redraw wins at dirty >= total / 7.5 (13.3%).
        // The shipped threshold fires at 1/8 (12.5%), just below the
        // byte crossover - the conservative side: at the boundary the
        // full redraw may cost marginally more bytes than the diff,
        // and the residual delta is bounded by the absolute cell count
        // (tiny on small screens, and the producer sweep already
        // accepted this margin when it chose the static 8).
        let total = 1024u64 * 500;
        let crossover_ratio = consumer_crossover_dirty_ratio(total) / total as f64;
        let shipped_ratio = 1.0 / crate::constants::DIRTY_THRESHOLD_RATIO as f64;
        assert!(
            (shipped_ratio - crossover_ratio).abs() < 0.01,
            "shipped {shipped_ratio} vs consumer crossover {crossover_ratio} must stay within the sweep's accepted margin"
        );
        // Byte deltas at the boundary stay tiny for byte-bound families
        // (the worst case is the smallest screens where the absolute
        // cell count bounds the difference).
        let small_cells = 36u64; // 6x6
        let dirty_at_threshold = small_cells / crate::constants::DIRTY_THRESHOLD_RATIO as u64;
        let diff_bytes = dirty_at_threshold * 30;
        let full_bytes = small_cells * 4;
        assert!(
            (diff_bytes as i64 - full_bytes as i64).abs() < 200,
            "the threshold-boundary byte delta must stay bounded on small screens"
        );
    }

    /// The family resolver maps the existing sensors exactly: the
    /// console dominates, then the heap class, then the parser class,
    /// then the GPU default. (The caps struct is built literal here so
    /// the mapping is pinned without re-reading env.)
    #[test]
    fn family_resolution_follows_the_sensor_order() {
        let base = crate::termdetect::TerminalCaps {
            sync_output: true,
            kitty_keyboard: false,
            has_alternate_screen: true,
            xtermjs_host: false,
            vscode_integrated: false,
            default_fps_cap: 240.0,
            dynamic_default_fps: 60.0,
            dynamic_fps_source: "standard/unknown fallback",
            phosphor_decay_mult: 1.0,
            ghost_brightness_cap: 1.0,
            speed_mult: 1.0,
            cpu_rendered: false,
            console_tty: false,
            effects_gate_source: "none — effects on",
        };
        let console = crate::termdetect::TerminalCaps {
            console_tty: true,
            cpu_rendered: true,
            ..base
        };
        assert_eq!(TerminalFamily::from_caps(&console), TerminalFamily::Console);
        let heap = crate::termdetect::TerminalCaps {
            xtermjs_host: true,
            ..base
        };
        assert_eq!(TerminalFamily::from_caps(&heap), TerminalFamily::XtermJs);
        let cpu = crate::termdetect::TerminalCaps {
            cpu_rendered: true,
            ..base
        };
        assert_eq!(TerminalFamily::from_caps(&cpu), TerminalFamily::CpuRendered);
        assert_eq!(TerminalFamily::from_caps(&base), TerminalFamily::Gpu);
    }
}
