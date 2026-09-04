// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Benchmark subsystem for cosmostrix.
//!
//! Provides two benchmark modes:
//!
//! - `--bench-frames N`: Legacy CI/regression benchmark. Runs N frames in a
//!   headless loop and prints results in a parseable `BENCH:` format. Suitable
//!   for automated performance tracking and CI pipelines.
//! - `--benchmark`: Premium user-facing benchmark. Runs for 5 seconds with
//!   a 2-second warmup phase, live progress feedback, and a comprehensive
//!   Report-engine output including avg/peak FPS, frame time percentiles,
//!   jitter classification, and throughput metrics.
//!
//! ## Methodology
//!
//! The premium benchmark is designed for reproducibility:
//! - Warmup phase (2s, configurable via `COSMOSTRIX_BENCH_WARMUP_SECS`):
//!   Allows the CPU to ramp up frequency and JIT/cache to stabilize.
//! - Outlier trimming: p99 frame time is computed after trimming the top
//!   and bottom 1% of samples, eliminating cold-path and OS scheduling noise.
//! - Rolling display: The live UI shows a smoothed average of the last 16
//!   frame times, avoiding flicker from per-frame variance.
//! - Interrupt support: Ctrl+C gracefully stops the benchmark and reports
//!   partial results with an "interrupted" status note.

// Submodule declarations: all bench_*.rs files are now siblings under
// src/bench/. Re-exported as `pub` so that `pub(crate) use bench::*;` in
// main.rs keeps `crate::bench_X::Foo` paths working for the 49 existing
// call sites without touching them.
pub mod bench_baseline;
pub mod bench_comp;
pub mod bench_config_enrichment;
pub mod bench_cpu;
pub mod bench_energy;
pub mod bench_helpers;
pub mod bench_io;
pub mod bench_json;
pub mod bench_mem;
pub mod bench_meta;
pub mod bench_perf;
pub mod bench_progress;
pub mod bench_report;
pub mod bench_report_engine;
#[cfg(test)]
#[path = "../../test/bench/bench_report_tests.rs"]
pub mod bench_report_tests;
pub mod bench_scale;
#[cfg(test)]
#[path = "../../test/bench/bench_tests.rs"]
pub mod bench_tests;
pub mod bench_visual;
mod peak_fps;
mod premium;
mod run_bench;
mod silent;

use crate::bench_report::BenchReportData;
use crate::CloudConfig;

// Re-export bench validation helpers + consts from bench_helpers.rs.
// Pure functions moved out to keep this file under the LOC cap.
// `median_sorted` + `resolve_bench_duration` are re-exported from
// `premium.rs` (where they are actually used); only `validate_bench_scene`
// is re-exported here so `run_bench.rs` + `silent.rs` can reach it via
// `use super::validate_bench_scene`.
pub(crate) use crate::bench_helpers::validate_bench_scene;
#[cfg(test)]
pub(crate) use crate::bench_helpers::{validate_bench_scene_str, VALID_BENCH_SCENES};

// v50.0.0-beta.7 LOC refactor: run_premium_benchmark extracted to
// premium.rs to keep this file under the 800-LOC hard cap.
pub(crate) use premium::run_premium_benchmark;

// v50.0.0-beta.7 LTS: compute_peak_fps extracted to peak_fps.rs to keep
// this file under the 800-LOC cap. Re-exported here so all existing
// compute_peak_fps(...) call sites (including 'use super::*' glob in
// bench_tests.rs) continue to resolve unchanged.
#[allow(unused_imports)]
pub(crate) use peak_fps::compute_peak_fps;

// v50.0.0-beta.7 LOC refactor: run_premium_benchmark_silent extracted
// to silent.rs to keep this file under the 800-LOC hard cap.
pub(crate) use run_bench::run_benchmark;
pub(crate) use silent::run_premium_benchmark_silent;

/// Run benchmark and return the report data without printing.
/// Used by --bench-all scaling automation.
pub(crate) fn run_benchmark_capture(
    cfg: &CloudConfig,
    duration_secs: u64,
) -> std::io::Result<BenchReportData> {
    // Temporarily set bench_duration and run the measurement
    let mut capture_cfg = cfg.clone_config();
    capture_cfg.bench_duration = Some(duration_secs);
    capture_cfg.json = false;
    capture_cfg.save_baseline = None;
    capture_cfg.compare_baseline = None;

    run_premium_benchmark_silent(&capture_cfg)
}

// Submodule (moved from src/ root for clean src/ layout)
mod dispatch;
pub(crate) use dispatch::dispatch_bench;
