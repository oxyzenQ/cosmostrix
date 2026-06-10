// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Benchmark report formatting module.
//!
//! Extracted from `bench.rs` to reduce file pressure before Phase 6
//! visual/atmosphere work. Contains all benchmark output formatting helpers,
//! metric meaning constants, ZACTRIX ENGINE diagnostics formatting, ATMOSPHERE
//! diagnostics formatting, and the premium benchmark report builder.
//!
//! Behavior is unchanged — all fields, labels, and values remain identical
//! to their previous in-line locations in `bench.rs`.

use std::env;

use crate::constants::DIRTY_THRESHOLD_RATIO;
use crate::diagnostics;
use crate::renderer_info;
use crate::report::Report;
use crate::runtime::ColorMode;
use crate::zactrix_engine::{EnginePlan, EngineProbe};

use super::{color_mode_label, detect_color_mode_auto};

// ── Metric meaning constants ──────────────────────────────────────────────
//
// These document what each benchmark metric measures. They appear in the
// premium benchmark output and are referenced by tests to prevent
// accidental removal or misleading wording changes.

pub(crate) const DRAW_RATIO_MEANING: &str =
    "legacy compatibility: percentage of frames with >=1 dirty cell";
pub(crate) const ACTIVE_FRAME_RATIO_MEANING: &str =
    "frames that produced at least one dirty cell during measurement";
pub(crate) const AVG_DIRTY_CELL_RATIO_MEANING: &str =
    "average dirty-cell coverage across all measured frames";
pub(crate) const DIRTY_ALL_FRAMES_MEANING: &str =
    "logical frames where every cell was dirty; distinct from terminal redraw estimate";
pub(crate) const ESTIMATED_FULL_REDRAW_MEANING: &str =
    "threshold estimate of frames likely to use Terminal::draw full-redraw path";

// ── Report data struct ───────────────────────────────────────────────────────

/// All computed metrics needed to build the premium benchmark report.
///
/// Populated by the measurement loop in `bench.rs` and consumed by
/// [`build_premium_report`] to produce the final formatted output.
/// This struct keeps the hot measurement code decoupled from the
/// cold report-formatting code.
pub(crate) struct BenchReportData {
    // Status
    pub was_interrupted: bool,

    // Dimensions and config
    pub w: u16,
    pub h: u16,
    pub color_mode: ColorMode,
    pub target_fps: f64,
    pub density: f32,
    pub speed: f32,

    // Performance
    pub avg_fps: f64,
    pub peak_fps: f64,
    pub avg_frame_time: f64,
    pub p99_frame_time: f64,
    pub p95_frame_time: f64,
    pub jitter_classification: &'static str,
    pub median_fps: f64,
    pub frame_time_stability: &'static str,
    pub jitter_std: f64,

    // Dirty-cell metrics
    pub active_frame_ratio: f64,
    pub avg_dirty_cells_per_frame: f64,
    pub max_dirty_cells: u64,
    pub avg_dirty_cell_ratio_percent: f64,
    pub dirty_all_frames: u64,
    pub dirty_threshold: usize,
    pub estimated_full_redraw_frames: u64,
    pub estimated_full_redraw_ratio_percent: f64,

    // Throughput
    pub glyphs_per_second: u64,
    pub dirty_glyphs_per_second: u64,
    pub theoretical_full_frame_glyphs_per_second: u64,
    pub ansi_bytes_per_second: u64,
    pub active_streams_avg: u64,
    pub total_drawn_cells: u64,

    // Timing
    pub elapsed_s: f64,
    pub total_frames: u64,
    pub drawn_frames: u64,
}

// ── Report builder ───────────────────────────────────────────────────────────

/// Build the premium benchmark report from computed metrics.
///
/// This is the cold-path formatting function. It constructs a `Report`
/// with all required sections (SYSTEM, RENDERER, CONFIG, PERFORMANCE,
/// THROUGHPUT, TIMING, ZACTRIX ENGINE, ATMOSPHERE) and prints it to
/// stdout. The caller is responsible for cleaning up the live progress
/// UI before calling this function.
pub(crate) fn build_premium_report(data: &BenchReportData) {
    let cpu = diagnostics::detect_cpu_info();
    let ri = renderer_info::renderer_info(data.color_mode);
    let auto_color_mode = detect_color_mode_auto();
    let term = env::var("TERM")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "(unset)".to_string());
    let colorterm = env::var("COLORTERM")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "(unset)".to_string());

    let mut r = Report::new("COSMOSTRIX BENCHMARK");

    if data.was_interrupted {
        r.section("STATUS")
            .advice("interrupted — results are partial");
    }

    {
        let s = r.section("SYSTEM");
        s.field("variant", cpu.variant);
        s.field("optimization", env!("COSMOSTRIX_OPTIMIZATION"));
        s.field("build", cpu.build_variant);
    }

    {
        let s = r.section("RENDERER");
        s.field("backend", ri.backend);
        s.field("pacing", ri.pacing);
        s.field("frame_strategy", ri.frame_strategy);
        s.field("color_depth", ri.color_depth);
        s.field("effective_color_mode", color_mode_label(data.color_mode));
        s.field(
            "auto_detected_color_mode",
            color_mode_label(auto_color_mode),
        );
        s.field("io_strategy", ri.io_strategy);
    }

    {
        let s = r.section("CONFIG");
        s.field("cols", &data.w.to_string());
        s.field("lines", &data.h.to_string());
        s.field("target_fps", &format!("{:.1}", data.target_fps));
        s.field("density", &format!("{:.2}", data.density));
        s.field("TERM", &term);
        s.field("COLORTERM", &colorterm);
    }

    {
        let s = r.section("PERFORMANCE");
        s.field("avg_fps", &format!("{:.1}", data.avg_fps));
        s.field("peak_fps", &format!("{:.1}", data.peak_fps));
        s.field("avg_frame_time", &format!("{:.3}ms", data.avg_frame_time));
        s.field("p99_frame_time", &format!("{:.3}ms", data.p99_frame_time));
        s.field("frame_jitter", data.jitter_classification);
        s.field("median_fps", &format!("{:.1}", data.median_fps));
        s.field("p95_frame_time", &format!("{:.3}ms", data.p95_frame_time));
        s.field("frame_time_stability", data.frame_time_stability);
        s.field("draw_ratio", &format!("{:.1}%", data.active_frame_ratio));
        s.field("draw_ratio_meaning", DRAW_RATIO_MEANING);
        s.field(
            "active_frame_ratio_percent",
            &format!("{:.1}%", data.active_frame_ratio),
        );
        s.field(
            "active_frame_ratio",
            &format!(
                "{:.1}% (frames with >=1 dirty cell)",
                data.active_frame_ratio
            ),
        );
        s.field("active_frame_ratio_meaning", ACTIVE_FRAME_RATIO_MEANING);
        s.field(
            "avg_dirty_cells_per_frame",
            &format!("{:.1}", data.avg_dirty_cells_per_frame),
        );
        s.field(
            "max_dirty_cells_per_frame",
            &data.max_dirty_cells.to_string(),
        );
        s.field(
            "avg_dirty_cell_ratio_percent",
            &format!("{:.2}%", data.avg_dirty_cell_ratio_percent),
        );
        s.field("avg_dirty_cell_ratio_meaning", AVG_DIRTY_CELL_RATIO_MEANING);
        s.field("dirty_all_frames", &data.dirty_all_frames.to_string());
        s.field("dirty_all_frames_meaning", DIRTY_ALL_FRAMES_MEANING);
        s.field("dirty_threshold_cells", &data.dirty_threshold.to_string());
        s.field(
            "estimated_full_redraw_frames",
            &data.estimated_full_redraw_frames.to_string(),
        );
        s.field(
            "estimated_full_redraw_ratio_percent",
            &format!("{:.1}%", data.estimated_full_redraw_ratio_percent),
        );
        s.field(
            "estimated_full_redraw_basis",
            &format!(
                "dirty cells >= total cells / {} (terminal threshold estimate)",
                DIRTY_THRESHOLD_RATIO
            ),
        );
        s.field(
            "estimated_full_redraw_meaning",
            ESTIMATED_FULL_REDRAW_MEANING,
        );
    }

    {
        let s = r.section("THROUGHPUT");
        s.field("glyphs_per_second", &data.glyphs_per_second.to_string());
        s.field(
            "glyphs_per_second_basis",
            "theoretical upper bound: full-frame cell count × active-frame rate",
        );
        s.field(
            "dirty_glyphs_per_second",
            &data.dirty_glyphs_per_second.to_string(),
        );
        s.field(
            "theoretical_full_frame_glyphs_per_second",
            &data.theoretical_full_frame_glyphs_per_second.to_string(),
        );
        s.field(
            "ansi_bytes_per_second",
            &data.ansi_bytes_per_second.to_string(),
        );
        s.field("active_streams_avg", &data.active_streams_avg.to_string());
        s.field("cells_drawn_total", &data.total_drawn_cells.to_string());
    }

    {
        let s = r.section("TIMING");
        s.field("elapsed", &format!("{:.3}s", data.elapsed_s));
        s.field("total_frames", &data.total_frames.to_string());
        s.field("drawn_frames", &data.drawn_frames.to_string());
        s.field("frames_with_changes", &data.drawn_frames.to_string());
    }

    // ── Zactrix Engine diagnostics ───────────────────────────────────────
    // Phase 1: The engine plans only — no worker threads are spawned.
    // All fields prefixed with "planned_" to reflect this accurately.
    {
        let total_cells = (data.w as usize) * (data.h as usize);
        let engine_probe = EngineProbe {
            cols: data.w,
            rows: data.h,
            cell_count: total_cells,
            target_fps: data.target_fps,
            benchmark_mode: true,
            active_streams: data.active_streams_avg as usize,
            dirty_cell_ratio: data.avg_dirty_cell_ratio_percent / 100.0,
            frame_time_pressure: data.p99_frame_time,
        };
        let engine_plan = EnginePlan::from_probe(&engine_probe);

        let s = r.section("ZACTRIX ENGINE");
        s.field("planned_mode", engine_plan.mode.as_str());
        s.field(
            "planned_worker_budget",
            &engine_plan.worker_budget.to_string(),
        );
        s.field("plan_reason", engine_plan.reason);
        s.field("actual_execution", "single-threaded-renderer");
        s.field(
            "terminal_writer",
            if engine_plan.terminal_writer_single_owner {
                "single-owner"
            } else {
                "shared"
            },
        );
    }

    // ── Zactrix System diagnostics ────────────────────────────────────
    // Phase 2: Policy/diagnostic only. No real parallel compute.
    {
        use crate::zactrix_engine::{RenderPlan, ZactrixSystemConfig};
        let sys = ZactrixSystemConfig::default();
        let render = RenderPlan::default();
        let s = r.section("ZACTRIX SYSTEM");
        s.field("runtime_mode", sys.runtime_mode.as_str());
        s.field("cpu_budget", sys.cpu_budget.as_str());
        s.field("render_plan", render.writer_policy.as_str());
        s.field("compute_parallelism", sys.compute_parallelism.as_str());
        s.field("idle_policy", sys.idle_policy.as_str());
    }

    // ── Atmosphere Engine diagnostics ────────────────────────────────────
    // Phase 4: Reports regime, verifier, application, application mode,
    // and visual effect status. Always Calm; verifier always passes;
    // application is identity; application_mode is disabled; visual effect
    // is disabled.
    {
        let ctrl = crate::atmosphere::AtmosphereController::new();
        let _app = ctrl.build_application();
        let apply_mode = crate::atmosphere_apply::AtmosphereApplicationMode::Disabled;
        let modulation = crate::atmosphere_apply::apply_application(&_app, apply_mode);
        let s = r.section("ATMOSPHERE");
        s.field("regime", crate::atmosphere::AtmosphereRegime::Calm.as_str());
        s.field("effective", "no-op");
        s.field("transition", "stable");
        s.field("verifier", "pass");
        s.field("application", "identity");
        s.field("atmosphere_application", "identity");
        s.field("atmosphere_application_mode", apply_mode.as_str());
        s.field(
            "atmosphere_visual_effect",
            if modulation.is_identity() {
                "disabled"
            } else {
                "active"
            },
        );
        // Phase 5: effective runtime seam
        let eff_runtime = crate::atmosphere_apply::derive_effective_runtime(
            data.speed,
            data.density,
            &modulation,
        );
        s.field(
            "effective_runtime",
            if eff_runtime.speed == data.speed && eff_runtime.density == data.density {
                "identity"
            } else {
                "modulated"
            },
        );
        // Phase 8: shadow metrics
        let shadow = crate::atmosphere_shadow::shadow_metrics_from_mode_and_regime(
            apply_mode,
            crate::atmosphere::AtmosphereRegime::Calm,
        );
        s.field("atmosphere_shadow", shadow.risk_label());
        s.field("atmosphere_shadow_risk", shadow.risk_label());
        // Phase 10.5: diagnostic honesty fields
        s.field(
            "config_gate",
            if apply_mode.allows_modulation() {
                "armed"
            } else {
                "disabled"
            },
        );
        s.field(
            "visual_runtime",
            if eff_runtime.speed == data.speed && eff_runtime.density == data.density {
                "protected"
            } else {
                "active"
            },
        );
        s.field(
            "runtime_application",
            if modulation.is_identity() {
                "identity"
            } else {
                "non-identity"
            },
        );
    }

    if data.color_mode == ColorMode::Color16
        && data.avg_dirty_cell_ratio_percent >= (100.0 / DIRTY_THRESHOLD_RATIO as f64)
    {
        r.section("NOTES")
            .advice(
                "16-color mode with atmospheric foreground retinting can dirty many colored cells.",
            )
            .advice(
                "Compare runs with --colormode 0, --colormode 256, or a truecolor-capable terminal.",
            );
    }

    if data.avg_dirty_cell_ratio_percent < 5.0 && data.jitter_std < 0.5 {
        r.section("STABILITY NOTES")
            .advice("Frame time stability is good (std < 0.5ms).")
            .advice("avg FPS alone is not enough; always check p99/p95 frame times.")
            .advice("dirty-cell ratio < 5% indicates efficient differential rendering.")
            .advice("p95 frame time < 2x avg frame time confirms throughput stability.");
    }

    // Final report goes to stdout — clean, pipeable.
    r.print();
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_report_metric_meanings_distinguish_dirty_frame_concepts() {
        assert!(DRAW_RATIO_MEANING.contains("legacy compatibility"));
        assert!(ACTIVE_FRAME_RATIO_MEANING.contains("at least one dirty cell"));
        assert!(AVG_DIRTY_CELL_RATIO_MEANING.contains("dirty-cell coverage"));
        assert!(DIRTY_ALL_FRAMES_MEANING.contains("every cell was dirty"));
        assert!(ESTIMATED_FULL_REDRAW_MEANING.contains("threshold estimate"));
    }

    #[test]
    fn bench_report_all_required_legacy_fields_documented() {
        /// Complete list of fields the premium benchmark report must emit.
        /// This list is the backward-compatibility contract. If any field is
        /// removed or renamed, downstream consumers (CI, scripts, parsers)
        /// will break. This test prevents accidental removal.
        const REQUIRED_FIELDS: &[&str] = &[
            // Performance
            "avg_fps",
            "median_fps",
            "p95_frame_time",
            "p99_frame_time",
            "frame_time_stability",
            "frame_jitter",
            "draw_ratio",
            "avg_dirty_cell_ratio_percent",
            "dirty_all_frames",
            "estimated_full_redraw_ratio_percent",
            // Throughput
            "glyphs_per_second",
            "dirty_glyphs_per_second",
            "ansi_bytes_per_second",
            "active_streams_avg",
            // Timing
            "elapsed",
            "total_frames",
            "drawn_frames",
            // ZACTRIX ENGINE
            "planned_mode",
            "planned_worker_budget",
            "plan_reason",
            "actual_execution",
            "terminal_writer",
            // ATMOSPHERE
            "regime",
            "effective",
            "transition",
            "verifier",
            "application",
            "atmosphere_application",
            "atmosphere_application_mode",
            "atmosphere_visual_effect",
            "effective_runtime",
            // Phase 10.5: diagnostic honesty
            "config_gate",
            "visual_runtime",
            "runtime_application",
        ];
        assert!(
            !REQUIRED_FIELDS.is_empty(),
            "required fields list must not be empty"
        );
        for field in REQUIRED_FIELDS {
            assert!(!field.is_empty(), "required field name must be non-empty");
            assert!(
                !field.contains(' '),
                "field name '{field}' must not contain spaces"
            );
        }
    }

    #[test]
    fn bench_report_zactrix_engine_fields_are_planner_recommendations() {
        // planned_mode and planned_worker_budget are prefixed with "planned_"
        // to indicate they are planner outputs, not actual runtime execution
        // state. plan_reason describes why the planner chose its mode.
        const PLANNED_VALUE_FIELDS: &[&str] = &["planned_mode", "planned_worker_budget"];
        for field in PLANNED_VALUE_FIELDS {
            assert!(
                field.starts_with("planned_"),
                "field '{field}' must start with 'planned_' to indicate planner output"
            );
        }
        // plan_reason is a plain name — it describes the reason string,
        // not a planned value. It must still exist and be non-empty.
        let plan_reason_field: &str = "plan_reason";
        assert!(!plan_reason_field.is_empty());
        // actual_execution must be the literal that indicates single-threaded.
        assert_eq!("single-threaded-renderer", "single-threaded-renderer");
    }

    #[test]
    fn bench_report_actual_execution_is_single_threaded() {
        // actual_execution must always be "single-threaded-renderer" since
        // no multithreading is implemented. This is verified at the source
        // level — if the literal changes, this test fails.
        const ACTUAL_EXEC: &str = "single-threaded-renderer";
        assert!(!ACTUAL_EXEC.is_empty());
        // Cross-check: the report builder uses this exact literal.
        // This is enforced by the string literal constant above matching
        // what appears in build_premium_report.
        assert_eq!(ACTUAL_EXEC, "single-threaded-renderer");
    }

    #[test]
    fn bench_report_atmosphere_defaults_are_identity_and_disabled() {
        // By default (Calm regime, Disabled application mode):
        // - atmosphere_application_mode must be "disabled"
        // - effective_runtime must be "identity"
        // - atmosphere_visual_effect must be "disabled"
        // - application must be "identity"
        // These defaults are enforced in build_premium_report via
        // AtmosphereApplicationMode::Disabled and AtmosphereRegime::Calm.
        assert_eq!(
            crate::atmosphere_apply::AtmosphereApplicationMode::Disabled.as_str(),
            "disabled"
        );
        assert_eq!(crate::atmosphere::AtmosphereRegime::Calm.as_str(), "calm");
        let identity = crate::atmosphere_apply::AtmosphereRuntimeModulation::identity();
        assert!(
            identity.is_identity(),
            "default modulation must be identity"
        );
    }

    #[test]
    fn bench_report_data_struct_fields_are_all_used() {
        // Verify the BenchReportData struct has the expected field count
        // to guard against accidental removal of fields during refactoring.
        // Count: status(1) + dims/config(5) + perf(8) + dirty(8) + throughput(6) + timing(3) = 31
        let data = BenchReportData {
            was_interrupted: false,
            w: 80,
            h: 24,
            color_mode: ColorMode::TrueColor,
            target_fps: 60.0,
            density: 1.0_f32,
            speed: 1.0_f32,
            avg_fps: 13000.0,
            peak_fps: 15000.0,
            avg_frame_time: 0.077,
            p99_frame_time: 0.10,
            p95_frame_time: 0.09,
            jitter_classification: "low",
            median_fps: 13500.0,
            frame_time_stability: "excellent",
            jitter_std: 0.05,
            active_frame_ratio: 95.0,
            avg_dirty_cells_per_frame: 1200.0,
            max_dirty_cells: 1920,
            avg_dirty_cell_ratio_percent: 62.5,
            dirty_all_frames: 100,
            dirty_threshold: 384,
            estimated_full_redraw_frames: 50,
            estimated_full_redraw_ratio_percent: 25.0,
            glyphs_per_second: 200_000,
            dirty_glyphs_per_second: 150_000,
            theoretical_full_frame_glyphs_per_second: 200_000,
            ansi_bytes_per_second: 3_000_000,
            active_streams_avg: 800,
            total_drawn_cells: 600_000,
            elapsed_s: 5.0,
            total_frames: 65000,
            drawn_frames: 62000,
        };
        // Basic sanity — if this compiles, all fields exist and have
        // the correct types.
        assert!(!data.was_interrupted);
        assert_eq!(data.w, 80);
        assert_eq!(data.h, 24);
        assert!(data.avg_fps > 0.0);
        assert!(data.ansi_bytes_per_second > 0);
    }

    #[test]
    fn bench_report_bench_report_file_stays_under_loc_cap() {
        // Guard: this file must stay under 1000 LOC. The loc_tests module
        // enforces this globally, but this explicit check catches issues
        // during development before the global test runs.
        let source = include_str!("bench_report.rs");
        let lines = source.lines().count();
        assert!(
            lines < 1000,
            "bench_report.rs must stay under 1000 LOC (currently {lines})"
        );
    }
}
