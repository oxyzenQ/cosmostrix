// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic execution planner for Cosmostrix.
//!
//! Cosmostrix is a single-thread, single-core renderer by design.
//! Terminal ANSI output MUST remain single-owner at all times.
//! The scheduler always returns SingleCore mode — no worker threads
//! are spawned, no parallelism overhead exists.

#![allow(dead_code)]

/// Execution mode — always SingleCore for cosmostrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub(crate) enum EngineMode {
    SingleCore,
    SafeFallback,
}

impl EngineMode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::SingleCore => "single-core",
            Self::SafeFallback => "safe-fallback",
        }
    }
}

/// Probe input for planner decision (kept for diagnostic compatibility).
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

/// Execution plan — always SingleCore, zero worker budget.
#[derive(Debug, Clone, Copy)]
#[must_use]
pub(crate) struct EnginePlan {
    pub mode: EngineMode,
    pub reason: &'static str,
    pub worker_budget: usize,
    pub terminal_writer_single_owner: bool,
}

impl EnginePlan {
    /// Always returns SingleCore mode. Cosmostrix is single-thread by design.
    /// Terminal writer is single-owner — non-negotiable invariant.
    pub(crate) fn from_probe(probe: &EngineProbe) -> Self {
        if probe.cols == 0 || probe.rows == 0 || probe.cell_count == 0 {
            return Self {
                mode: EngineMode::SafeFallback,
                reason: "zero or invalid dimensions",
                worker_budget: 0,
                terminal_writer_single_owner: true,
            };
        }
        if probe.frame_time_pressure > 50.0 {
            return Self {
                mode: EngineMode::SafeFallback,
                reason: "extreme frame-time pressure",
                worker_budget: 0,
                terminal_writer_single_owner: true,
            };
        }
        Self {
            mode: EngineMode::SingleCore,
            reason: "single-thread renderer — cosmostrix optimized for single-core execution",
            worker_budget: 0,
            terminal_writer_single_owner: true,
        }
    }

    pub(crate) fn from_dimensions(cols: u16, rows: u16) -> Self {
        Self::from_probe(&EngineProbe::from_dimensions(cols, rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_terminal_selects_single_core() {
        for (cols, rows) in [
            (80, 24),
            (120, 40),
            (160, 50),
            (199, 59),
            (200, 60),
            (300, 80),
        ] {
            let plan = EnginePlan::from_dimensions(cols, rows);
            assert_eq!(
                plan.mode,
                EngineMode::SingleCore,
                "({cols}, {rows}) should be SingleCore — cosmostrix is single-thread"
            );
            assert_eq!(plan.worker_budget, 0);
            assert!(plan.terminal_writer_single_owner);
        }
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
    fn worker_budget_is_always_zero() {
        for (cols, rows) in [(80, 24), (200, 60), (300, 80)] {
            let plan = EnginePlan::from_dimensions(cols, rows);
            assert_eq!(plan.worker_budget, 0);
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
            assert!(plan.reason.len() < 160);
        }
    }
}
