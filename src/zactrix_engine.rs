// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Adaptive execution planner for Cosmostrix v4.0.0.
//!
//! Zactrix Engine is an internal deterministic planner that observes terminal
//! dimensions, frame-time pressure, and workload characteristics, then produces
//! a bounded execution plan. It is not a public API. It is not a
//! parallelization framework. The terminal writer remains single-owner at
//! all times.

// Phase 1: Some probe fields are set but not yet consumed by the planner.
// They exist for future adaptive logic and benchmark diagnostics.
#![allow(dead_code)]

use std::thread::available_parallelism;

/// Maximum worker budget hard cap. Never exceeded regardless of hardware.
const WORKER_BUDGET_HARD_CAP: usize = 4;

/// Frame-time pressure threshold (ms) above which SafeFallback is used.
const FRAME_TIME_PRESSURE_EXTREME_MS: f64 = 50.0;

/// Column threshold for "large" classification.
const LARGE_COLS_THRESHOLD: u16 = 200;

/// Row threshold for "large" classification.
const LARGE_ROWS_THRESHOLD: u16 = 60;

/// Column threshold for "very large" classification.
const VERY_LARGE_COLS_THRESHOLD: u16 = 300;

/// Row threshold for "very large" classification.
const VERY_LARGE_ROWS_THRESHOLD: u16 = 80;

// ── Engine Mode ────────────────────────────────────────────────────────────

/// Execution mode selected by the planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub(crate) enum EngineMode {
    /// Single-threaded execution. Default for normal terminal sizes.
    SingleCore,
    /// Large screen or moderate workload. Small bounded worker budget.
    Assist,
    /// Very large screen or benchmark. Moderate bounded worker budget.
    ParallelCompute,
    /// Invalid, zero, or extreme conditions. Always safe.
    SafeFallback,
}

impl EngineMode {
    /// Human-readable label for benchmark diagnostics.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::SingleCore => "single-core",
            Self::Assist => "assist",
            Self::ParallelCompute => "parallel-compute",
            Self::SafeFallback => "safe-fallback",
        }
    }
}

// ── Engine Probe ─────────────────────────────────────────────────────────

/// Observable facts fed to the planner.
#[derive(Debug, Clone, Copy)]
#[must_use]
pub(crate) struct EngineProbe {
    /// Terminal columns.
    pub cols: u16,
    /// Terminal rows.
    pub rows: u16,
    /// Total cell count (cols * rows).
    pub cell_count: usize,
    /// Target frames per second.
    pub target_fps: f64,
    /// Whether this is a benchmark run.
    pub benchmark_mode: bool,
    /// Number of active droplet streams (0 if unknown).
    pub active_streams: usize,
    /// Fraction of dirty cells (0.0 .. 1.0, 0.0 if unknown).
    pub dirty_cell_ratio: f64,
    /// p99 frame time in milliseconds (0.0 if unknown/unmeasured).
    pub frame_time_pressure: f64,
}

impl EngineProbe {
    /// Create a probe from terminal dimensions with sensible defaults.
    pub(crate) const fn from_dimensions(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            cell_count: (cols as usize) * (rows as usize),
            target_fps: 0.0,
            benchmark_mode: false,
            active_streams: 0,
            dirty_cell_ratio: 0.0,
            frame_time_pressure: 0.0,
        }
    }
}

// ── Engine Plan ───────────────────────────────────────────────────────────

/// Execution plan produced by the planner.
#[derive(Debug, Clone, Copy)]
#[must_use]
pub(crate) struct EnginePlan {
    /// Selected execution mode.
    pub mode: EngineMode,
    /// Human-readable reason for the mode selection.
    pub reason: &'static str,
    /// Bounded worker budget (0 for SingleCore/SafeFallback).
    pub worker_budget: usize,
    /// Terminal writer remains single-owner. Always true.
    pub terminal_writer_single_owner: bool,
}

impl EnginePlan {
    /// Create a plan for the given probe using deterministic thresholds.
    pub(crate) fn from_probe(probe: &EngineProbe) -> Self {
        // 1. Zero or invalid dimensions => SafeFallback
        if probe.cols == 0 || probe.rows == 0 || probe.cell_count == 0 {
            return Self {
                mode: EngineMode::SafeFallback,
                reason: "zero or invalid dimensions",
                worker_budget: 0,
                terminal_writer_single_owner: true,
            };
        }

        // 2. Extreme frame-time pressure => SafeFallback
        if probe.frame_time_pressure > FRAME_TIME_PRESSURE_EXTREME_MS {
            return Self {
                mode: EngineMode::SafeFallback,
                reason: "extreme frame-time pressure",
                worker_budget: 0,
                terminal_writer_single_owner: true,
            };
        }

        // 3. Very large screens or benchmark => ParallelCompute
        if probe.cols >= VERY_LARGE_COLS_THRESHOLD && probe.rows >= VERY_LARGE_ROWS_THRESHOLD {
            return Self {
                mode: EngineMode::ParallelCompute,
                reason: "very large screen dimensions",
                worker_budget: bounded_worker_budget(2),
                terminal_writer_single_owner: true,
            };
        }

        if probe.benchmark_mode {
            return Self {
                mode: EngineMode::ParallelCompute,
                reason: "benchmark mode",
                worker_budget: bounded_worker_budget(2),
                terminal_writer_single_owner: true,
            };
        }

        // 4. Large screens => Assist
        if probe.cols >= LARGE_COLS_THRESHOLD || probe.rows >= LARGE_ROWS_THRESHOLD {
            return Self {
                mode: EngineMode::Assist,
                reason: "large screen dimensions",
                worker_budget: bounded_worker_budget(1),
                terminal_writer_single_owner: true,
            };
        }

        // 5. Normal screens => SingleCore
        Self {
            mode: EngineMode::SingleCore,
            reason: "normal terminal dimensions",
            worker_budget: 0,
            terminal_writer_single_owner: true,
        }
    }

    /// Convenience: create a plan from dimensions alone.
    pub(crate) fn from_dimensions(cols: u16, rows: u16) -> Self {
        Self::from_probe(&EngineProbe::from_dimensions(cols, rows))
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────

/// Compute a bounded worker budget that never exceeds available parallelism
/// or the hard cap.
fn bounded_worker_budget(requested: usize) -> usize {
    let available = available_parallelism().map(|n| n.get()).unwrap_or(1);
    requested.min(available).min(WORKER_BUDGET_HARD_CAP)
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_terminal_selects_single_core() {
        for (cols, rows) in [(80, 24), (120, 40), (160, 50), (199, 59)] {
            let plan = EnginePlan::from_dimensions(cols, rows);
            assert_eq!(
                plan.mode,
                EngineMode::SingleCore,
                "({cols}, {rows}) should be SingleCore"
            );
            assert_eq!(plan.worker_budget, 0);
            assert!(plan.terminal_writer_single_owner);
        }
    }

    #[test]
    fn large_terminal_selects_assist() {
        let plan = EnginePlan::from_dimensions(200, 40);
        assert_eq!(plan.mode, EngineMode::Assist);

        let plan = EnginePlan::from_dimensions(100, 60);
        assert_eq!(plan.mode, EngineMode::Assist);

        let plan = EnginePlan::from_dimensions(250, 70);
        assert_eq!(plan.mode, EngineMode::Assist);
    }

    #[test]
    fn very_large_terminal_selects_parallel_compute() {
        let plan = EnginePlan::from_dimensions(300, 80);
        assert_eq!(plan.mode, EngineMode::ParallelCompute);
    }

    #[test]
    fn benchmark_mode_selects_parallel_compute() {
        let probe = EngineProbe {
            benchmark_mode: true,
            ..EngineProbe::from_dimensions(120, 40)
        };
        let plan = EnginePlan::from_probe(&probe);
        assert_eq!(plan.mode, EngineMode::ParallelCompute);
        assert!(plan.reason.contains("benchmark"));
    }

    #[test]
    fn zero_dimensions_use_safe_fallback() {
        let plan = EnginePlan::from_dimensions(0, 40);
        assert_eq!(plan.mode, EngineMode::SafeFallback);

        let plan = EnginePlan::from_dimensions(80, 0);
        assert_eq!(plan.mode, EngineMode::SafeFallback);
    }

    #[test]
    fn extreme_frame_time_pressure_uses_safe_fallback() {
        let probe = EngineProbe {
            frame_time_pressure: 60.0,
            ..EngineProbe::from_dimensions(300, 80)
        };
        let plan = EnginePlan::from_probe(&probe);
        assert_eq!(plan.mode, EngineMode::SafeFallback);
        assert!(plan.reason.contains("extreme"));
    }

    #[test]
    fn worker_budget_is_bounded() {
        for (cols, rows) in [(80, 24), (200, 60), (300, 80)] {
            let plan = EnginePlan::from_dimensions(cols, rows);
            let available = available_parallelism().map(|n| n.get()).unwrap_or(1);
            assert!(
                plan.worker_budget <= available.min(WORKER_BUDGET_HARD_CAP),
                "worker_budget {} must be <= {} for ({cols}, {rows})",
                plan.worker_budget,
                available.min(WORKER_BUDGET_HARD_CAP)
            );
        }
    }

    #[test]
    fn terminal_writer_always_single_owner() {
        let probes: Vec<EngineProbe> = vec![
            EngineProbe::from_dimensions(80, 24),
            EngineProbe::from_dimensions(300, 80),
            EngineProbe {
                frame_time_pressure: 100.0,
                ..EngineProbe::from_dimensions(120, 40)
            },
            EngineProbe {
                benchmark_mode: true,
                ..EngineProbe::from_dimensions(120, 40)
            },
        ];
        for probe in &probes {
            let plan = EnginePlan::from_probe(probe);
            assert!(
                plan.terminal_writer_single_owner,
                "terminal_writer_single_owner must be true for probe {:?}",
                probe
            );
        }
    }

    #[test]
    fn engine_mode_labels_are_deterministic() {
        assert_eq!(EngineMode::SingleCore.as_str(), "single-core");
        assert_eq!(EngineMode::Assist.as_str(), "assist");
        assert_eq!(EngineMode::ParallelCompute.as_str(), "parallel-compute");
        assert_eq!(EngineMode::SafeFallback.as_str(), "safe-fallback");
    }

    #[test]
    fn plan_reasons_are_non_empty() {
        let probes: Vec<EngineProbe> = vec![
            EngineProbe::from_dimensions(80, 24),
            EngineProbe::from_dimensions(200, 60),
            EngineProbe::from_dimensions(300, 80),
            EngineProbe::from_dimensions(0, 40),
            EngineProbe {
                frame_time_pressure: 100.0,
                ..EngineProbe::from_dimensions(120, 40)
            },
            EngineProbe {
                benchmark_mode: true,
                ..EngineProbe::from_dimensions(120, 40)
            },
        ];
        for probe in &probes {
            let plan = EnginePlan::from_probe(probe);
            assert!(!plan.reason.is_empty(), "reason must be non-empty");
            assert!(
                plan.reason.len() < 80,
                "reason should be concise, got: {}",
                plan.reason.len()
            );
        }
    }
}
