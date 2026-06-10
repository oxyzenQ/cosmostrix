// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Adaptive execution planner for Cosmostrix.
//!
//! Zactrix Scheduler is an internal deterministic planner that observes terminal
//! dimensions, frame-time pressure, and workload characteristics, then produces
//! a bounded execution plan. It is not a public API. It is not a
//! parallelization framework. The terminal writer remains single-owner at
//! all times.

#![allow(dead_code)]

use std::thread::available_parallelism;

const WORKER_BUDGET_HARD_CAP: usize = 4;
const FRAME_TIME_PRESSURE_EXTREME_MS: f64 = 50.0;
const LARGE_COLS_THRESHOLD: u16 = 200;
const LARGE_ROWS_THRESHOLD: u16 = 60;
const VERY_LARGE_COLS_THRESHOLD: u16 = 300;
const VERY_LARGE_ROWS_THRESHOLD: u16 = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub(crate) enum EngineMode {
    SingleCore,
    Assist,
    ParallelCompute,
    SafeFallback,
}

impl EngineMode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::SingleCore => "single-core",
            Self::Assist => "assist",
            Self::ParallelCompute => "parallel-compute",
            Self::SafeFallback => "safe-fallback",
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[must_use]
pub(crate) struct EngineProbe {
    pub cols: u16,
    pub rows: u16,
    pub cell_count: usize,
    pub target_fps: f64,
    pub benchmark_mode: bool,
    pub active_streams: usize,
    pub dirty_cell_ratio: f64,
    pub frame_time_pressure: f64,
}

impl EngineProbe {
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

#[derive(Debug, Clone, Copy)]
#[must_use]
pub(crate) struct EnginePlan {
    pub mode: EngineMode,
    pub reason: &'static str,
    pub worker_budget: usize,
    pub terminal_writer_single_owner: bool,
}

impl EnginePlan {
    pub(crate) fn from_probe(probe: &EngineProbe) -> Self {
        if probe.cols == 0 || probe.rows == 0 || probe.cell_count == 0 {
            return Self {
                mode: EngineMode::SafeFallback,
                reason: "zero or invalid dimensions",
                worker_budget: 0,
                terminal_writer_single_owner: true,
            };
        }
        if probe.frame_time_pressure > FRAME_TIME_PRESSURE_EXTREME_MS {
            return Self {
                mode: EngineMode::SafeFallback,
                reason: "extreme frame-time pressure",
                worker_budget: 0,
                terminal_writer_single_owner: true,
            };
        }
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
        if probe.cols >= LARGE_COLS_THRESHOLD || probe.rows >= LARGE_ROWS_THRESHOLD {
            return Self {
                mode: EngineMode::Assist,
                reason: "large screen dimensions",
                worker_budget: bounded_worker_budget(1),
                terminal_writer_single_owner: true,
            };
        }
        Self {
            mode: EngineMode::SingleCore,
            reason: "normal terminal dimensions",
            worker_budget: 0,
            terminal_writer_single_owner: true,
        }
    }

    pub(crate) fn from_dimensions(cols: u16, rows: u16) -> Self {
        Self::from_probe(&EngineProbe::from_dimensions(cols, rows))
    }
}

fn bounded_worker_budget(requested: usize) -> usize {
    let available = available_parallelism().map(|n| n.get()).unwrap_or(1);
    requested.min(available).min(WORKER_BUDGET_HARD_CAP)
}

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
        assert_eq!(
            EnginePlan::from_dimensions(200, 40).mode,
            EngineMode::Assist
        );
        assert_eq!(
            EnginePlan::from_dimensions(100, 60).mode,
            EngineMode::Assist
        );
        assert_eq!(
            EnginePlan::from_dimensions(250, 70).mode,
            EngineMode::Assist
        );
    }

    #[test]
    fn very_large_terminal_selects_parallel_compute() {
        assert_eq!(
            EnginePlan::from_dimensions(300, 80).mode,
            EngineMode::ParallelCompute
        );
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
        assert_eq!(
            EnginePlan::from_dimensions(0, 40).mode,
            EngineMode::SafeFallback
        );
        assert_eq!(
            EnginePlan::from_dimensions(80, 0).mode,
            EngineMode::SafeFallback
        );
    }

    #[test]
    fn extreme_frame_time_pressure_uses_safe_fallback() {
        let probe = EngineProbe {
            frame_time_pressure: 60.0,
            ..EngineProbe::from_dimensions(300, 80)
        };
        assert_eq!(
            EnginePlan::from_probe(&probe).mode,
            EngineMode::SafeFallback
        );
    }

    #[test]
    fn worker_budget_is_bounded() {
        for (cols, rows) in [(80, 24), (200, 60), (300, 80)] {
            let plan = EnginePlan::from_dimensions(cols, rows);
            let available = available_parallelism().map(|n| n.get()).unwrap_or(1);
            assert!(plan.worker_budget <= available.min(WORKER_BUDGET_HARD_CAP));
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
            assert!(EnginePlan::from_probe(probe).terminal_writer_single_owner);
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
            assert!(!plan.reason.is_empty());
            assert!(plan.reason.len() < 80);
        }
    }
}
