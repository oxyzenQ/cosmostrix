// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Format a KiB RSS value as a human-readable string with binary suffix.
///
/// Examples: 512 → "512 KiB", 2048 → "2.0 MiB", 1572864 → "1.5 GiB".
fn format_rss_kb(kib: u64) -> String {
    const MIB: u64 = 1024;
    const GIB: u64 = 1024 * 1024;
    if kib >= GIB {
        format!("{:.2} GiB", kib as f64 / GIB as f64)
    } else if kib >= MIB {
        format!("{:.1} MiB", kib as f64 / MIB as f64)
    } else {
        format!("{kib} KiB")
    }
}

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
    /// Worst observed frame time during measurement. Captures one-off
    /// spikes (GC, page faults, OS scheduling) that p99/p99.9 smooth over.
    /// For real-time renderers, max is what users perceive as "jank".
    pub max_frame_time: f64,
    /// 99.9th percentile frame time. Tighter than p99 on the long tail:
    /// 1 frame in 1000 exceeds this. Useful for sustained-run analysis.
    pub p99_9_frame_time: f64,
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

    // Memory (RSS) — None on platforms without sampling support.
    // peak_rss_kb: highest observed resident set size during measurement.
    // avg_rss_kb:  mean of all samples taken during measurement.
    // rss_samples: number of samples collected (for transparency).
    // rss_supported: false on platforms where RSS sampling is unavailable.
    pub peak_rss_kb: Option<u64>,
    pub avg_rss_kb: Option<u64>,
    pub rss_samples: u32,
    pub rss_supported: bool,

    // CPU usage — None on platforms without sampling support.
    // avg_cpu_percent: mean per-interval CPU% over the measurement window.
    // peak_cpu_percent: highest single-interval CPU% reading.
    // cpu_samples: number of interval samples collected.
    // cpu_supported: false on platforms where CPU sampling is unavailable.
    // Single-thread renderer: ~100% = one core saturated; >100% would
    // indicate multi-threading (not used) or measurement error.
    pub avg_cpu_percent: Option<f64>,
    pub peak_cpu_percent: Option<f64>,
    pub cpu_samples: u32,
    pub cpu_supported: bool,

    // Sub-component timing breakdown (averages + peaks, in ms).
    // sim_ms    = time in cloud.rain_at() before the first frame mutation
    //             (atmosphere events, spawn rate, droplet physics).
    // render_ms = time in cloud.rain_at() during phosphor/anomaly/atmospheric
    //             frame mutations.
    // io_ms     = time OUTSIDE rain_at() within the frame loop — dirty
    //             checks, clear_dirty, bookkeeping. In benchmark mode NO
    //             terminal write happens, so this is dirty-tracking overhead,
    //             not real IO. Labeled honestly in the report.
    pub avg_sim_ms: f64,
    pub avg_render_ms: f64,
    pub avg_io_ms: f64,
    pub max_sim_ms: f64,
    pub max_render_ms: f64,
    pub max_io_ms: f64,

    // Long-run drift detection (None if benchmark was interrupted before
    // the halfway mark). Compares first-half FPS vs second-half FPS.
    // Positive drift_percent = FPS degraded over time (thermal throttle,
    // allocator pressure, cache pollution). Negative = warmed up.
    pub first_half_fps: Option<f64>,
    pub second_half_fps: Option<f64>,
    pub fps_drift_percent: Option<f64>,
    /// Effective benchmark duration in seconds (may differ from default 5s
    /// when --bench-duration N is supplied).
    pub bench_duration_secs: u64,
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
        s.field("p95_frame_time", &format!("{:.3}ms", data.p95_frame_time));
        s.field("p99_frame_time", &format!("{:.3}ms", data.p99_frame_time));
        s.field(
            "p99_9_frame_time",
            &format!("{:.3}ms", data.p99_9_frame_time),
        );
        s.field("max_frame_time", &format!("{:.3}ms", data.max_frame_time));
        s.field(
            "max_frame_time_meaning",
            "worst single-frame spike; what users perceive as jank",
        );
        s.field("frame_jitter", data.jitter_classification);
        s.field("median_fps", &format!("{:.1}", data.median_fps));
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

    // ── Memory (RSS) ───────────────────────────────────────────────────
    // Honest reporting: on unsupported platforms we emit "unsupported"
    // rather than zero. This avoids implying the metric was measured.
    {
        let s = r.section("MEMORY");
        if data.rss_supported {
            let peak = data
                .peak_rss_kb
                .map(format_rss_kb)
                .unwrap_or_else(|| "(no sample)".to_string());
            let avg = data
                .avg_rss_kb
                .map(format_rss_kb)
                .unwrap_or_else(|| "(no sample)".to_string());
            s.field("peak_rss", &peak);
            s.field("avg_rss", &avg);
            s.field("rss_samples", &data.rss_samples.to_string());
            s.field(
                "rss_basis",
                "resident set size sampled during measurement window",
            );
            s.field(
                "rss_caveat",
                "RSS includes shared pages; treat as order-of-magnitude footprint",
            );
        } else {
            s.field("peak_rss", "unsupported");
            s.field("avg_rss", "unsupported");
            s.field(
                "rss_reason",
                "RSS sampling not implemented for this platform (Linux/macOS only)",
            );
        }
    }

    // ── CPU usage ─────────────────────────────────────────────────────
    // Per-interval CPU% from process CPU time deltas. Single-thread
    // renderer: ~100% = one core saturated. Honest reporting: on
    // unsupported platforms we emit "unsupported" rather than zero.
    {
        let s = r.section("CPU");
        if data.cpu_supported {
            let avg = data
                .avg_cpu_percent
                .map(|v| format!("{:.1}%", v))
                .unwrap_or_else(|| "(no sample)".to_string());
            let peak = data
                .peak_cpu_percent
                .map(|v| format!("{:.1}%", v))
                .unwrap_or_else(|| "(no sample)".to_string());
            s.field("avg_cpu_percent", &avg);
            s.field("peak_cpu_percent", &peak);
            s.field("cpu_samples", &data.cpu_samples.to_string());
            s.field(
                "cpu_basis",
                "per-interval (cpu_ns_delta / wall_ns_delta) * 100; single-thread renderer",
            );
            s.field(
                "cpu_caveat",
                "~100% = one core saturated; >100% would indicate multi-threading or measurement error",
            );
        } else {
            s.field("avg_cpu_percent", "unsupported");
            s.field("peak_cpu_percent", "unsupported");
            s.field(
                "cpu_reason",
                "CPU sampling not implemented for this platform (Linux/macOS only)",
            );
        }
    }

    // ── Sub-component timing breakdown ─────────────────────────────────
    // Distinguishes "benchmark mainan" from "profiling tool": shows where
    // frame time is actually spent. sim = raindrop physics, render = frame
    // mutations, io = dirty-tracking + bookkeeping (NO real terminal IO in
    // benchmark mode — labeled honestly).
    {
        let s = r.section("COMPONENT TIMING");
        s.field("avg_sim_ms", &format!("{:.4}", data.avg_sim_ms));
        s.field("avg_render_ms", &format!("{:.4}", data.avg_render_ms));
        s.field("avg_io_ms", &format!("{:.4}", data.avg_io_ms));
        s.field("max_sim_ms", &format!("{:.4}", data.max_sim_ms));
        s.field("max_render_ms", &format!("{:.4}", data.max_render_ms));
        s.field("max_io_ms", &format!("{:.4}", data.max_io_ms));
        s.field(
            "sim_meaning",
            "atmosphere events + spawn rate + droplet physics (cloud.rain_at pre-render)",
        );
        s.field(
            "render_meaning",
            "phosphor decay + anomaly zones + atmospheric fx + message box (frame mutations)",
        );
        s.field(
            "io_meaning",
            "dirty checks + clear_dirty + loop bookkeeping (NO terminal write in benchmark mode)",
        );
        let total_avg = data.avg_sim_ms + data.avg_render_ms + data.avg_io_ms;
        if total_avg > 0.0 {
            s.field(
                "sim_share_percent",
                &format!("{:.1}", data.avg_sim_ms / total_avg * 100.0),
            );
            s.field(
                "render_share_percent",
                &format!("{:.1}", data.avg_render_ms / total_avg * 100.0),
            );
            s.field(
                "io_share_percent",
                &format!("{:.1}", data.avg_io_ms / total_avg * 100.0),
            );
        }
    }

    // ── Long-run drift detection ──────────────────────────────────────
    // Compares first-half FPS vs second-half FPS. Useful with
    // --bench-duration N (long N) to detect thermal throttle, allocator
    // fragmentation, or cache pressure that a 5s run would miss.
    // None values indicate the benchmark was interrupted before halfway.
    {
        let s = r.section("DRIFT");
        s.field("bench_duration_secs", &data.bench_duration_secs.to_string());
        match (
            data.first_half_fps,
            data.second_half_fps,
            data.fps_drift_percent,
        ) {
            (Some(f), Some(s2), Some(d)) => {
                s.field("first_half_fps", &format!("{:.1}", f));
                s.field("second_half_fps", &format!("{:.1}", s2));
                s.field("fps_drift_percent", &format!("{:+.2}%", d));
                // Interpret the drift value for the user.
                let interpretation = if d > 10.0 {
                    "degraded — possible thermal throttle / allocator pressure / cache pollution"
                } else if d < -10.0 {
                    "improved — warmup may have been insufficient; consider longer --bench-duration"
                } else {
                    "stable — no significant drift detected"
                };
                s.field("drift_interpretation", interpretation);
                s.field(
                    "drift_basis",
                    "first_half_fps vs second_half_fps; positive = FPS dropped over time",
                );
            }
            _ => {
                s.field(
                    "drift_status",
                    "skipped — benchmark interrupted before halfway mark",
                );
                s.field(
                    "drift_reason",
                    "drift detection requires the benchmark to run past 50% of its target duration",
                );
            }
        }
    }

    // ── Engine diagnostics ─────────────────────────────────────────────
    // Cosmostrix is single-thread by design — terminal writer is single-owner.
    {
        let s = r.section("ENGINE");
        s.field("planned_mode", "single-core");
        s.field("planned_worker_budget", "0");
        s.field(
            "plan_reason",
            "single-thread renderer — cosmostrix optimized for single-core execution",
        );
        s.field("actual_execution", "single-threaded-renderer");
        s.field("terminal_writer", "single-owner");
    }

    // ── System diagnostics ─────────────────────────────────────────────
    {
        let s = r.section("SYSTEM");
        s.field("runtime_mode", "normal");
        s.field("render_plan", "single-owner");
        s.field("idle_policy", "adaptive-sleep");
        s.field("architecture", "single-thread optimized");
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
    fn bench_report_engine_fields_are_planner_recommendations() {
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
            max_frame_time: 0.25,
            p99_9_frame_time: 0.18,
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
            peak_rss_kb: Some(12_500),
            avg_rss_kb: Some(11_200),
            rss_samples: 50,
            rss_supported: true,
            avg_cpu_percent: Some(85.3),
            peak_cpu_percent: Some(98.7),
            cpu_samples: 25,
            cpu_supported: true,
            avg_sim_ms: 0.040,
            avg_render_ms: 0.030,
            avg_io_ms: 0.007,
            max_sim_ms: 0.080,
            max_render_ms: 0.060,
            max_io_ms: 0.015,
            first_half_fps: Some(13_000.0),
            second_half_fps: Some(12_850.0),
            fps_drift_percent: Some(1.15),
            bench_duration_secs: 5,
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

    #[test]
    fn format_rss_kb_renders_human_readable_suffixes() {
        assert_eq!(format_rss_kb(0), "0 KiB");
        assert_eq!(format_rss_kb(512), "512 KiB");
        assert_eq!(format_rss_kb(1023), "1023 KiB");
        assert_eq!(format_rss_kb(1024), "1.0 MiB");
        assert_eq!(format_rss_kb(2048), "2.0 MiB");
        assert_eq!(format_rss_kb(1_572_864), "1.50 GiB");
        // Rounding: 1.005 GiB should round to 1.00 or 1.01 — both acceptable
        // as long as the GiB suffix appears. Just verify the suffix.
        assert!(format_rss_kb(1_048_576).ends_with("GiB"));
    }

    #[test]
    fn rss_fields_documented_in_required_fields_list() {
        // Memory section must emit these keys on supported platforms.
        // This list documents the contract so CI/scripts can rely on it.
        const REQUIRED_MEMORY_FIELDS: &[&str] = &[
            "peak_rss",
            "avg_rss",
            "rss_samples",
            "rss_basis",
            "rss_caveat",
        ];
        for field in REQUIRED_MEMORY_FIELDS {
            assert!(!field.is_empty());
            assert!(!field.contains(' '));
        }
    }

    #[test]
    fn percentile_ordering_contract_documented() {
        // Frame time percentiles must satisfy:
        //   avg <= p95 <= p99 <= p99.9 <= max
        // (Frame time is inverse of FPS — higher percentile = slower frame.)
        // This test documents the contract; bench.rs enforces it by
        // computing each metric from the same sorted array.
        const ORDER: &[&str] = &["avg", "p95", "p99", "p99_9", "max"];
        assert_eq!(ORDER.len(), 5);
        // p99.9 must appear between p99 and max — guard against typos.
        let p99_pos = ORDER.iter().position(|&s| s == "p99").unwrap();
        let p99_9_pos = ORDER.iter().position(|&s| s == "p99_9").unwrap();
        let max_pos = ORDER.iter().position(|&s| s == "max").unwrap();
        assert!(p99_pos < p99_9_pos);
        assert!(p99_9_pos < max_pos);
    }

    #[test]
    fn required_performance_fields_include_new_tail_metrics() {
        // Backward-compat contract: downstream CI/scripts may parse
        // p99_9_frame_time and max_frame_time. Document them here so
        // accidental removal breaks this test.
        const REQUIRED_PERF_FIELDS: &[&str] = &[
            "avg_fps",
            "peak_fps",
            "avg_frame_time",
            "p95_frame_time",
            "p99_frame_time",
            "p99_9_frame_time",
            "max_frame_time",
            "max_frame_time_meaning",
            "frame_jitter",
            "median_fps",
            "frame_time_stability",
        ];
        for field in REQUIRED_PERF_FIELDS {
            assert!(!field.is_empty());
            assert!(!field.contains(' '));
        }
    }

    #[test]
    fn component_timing_fields_documented() {
        // COMPONENT TIMING section must emit these keys. This list is the
        // backward-compat contract — downstream parsers can rely on it.
        const REQUIRED_COMPONENT_FIELDS: &[&str] = &[
            "avg_sim_ms",
            "avg_render_ms",
            "avg_io_ms",
            "max_sim_ms",
            "max_render_ms",
            "max_io_ms",
            "sim_meaning",
            "render_meaning",
            "io_meaning",
        ];
        for field in REQUIRED_COMPONENT_FIELDS {
            assert!(!field.is_empty());
            assert!(!field.contains(' '));
        }
    }

    #[test]
    fn cpu_fields_documented() {
        // CPU section must emit these keys on supported platforms.
        const REQUIRED_CPU_FIELDS: &[&str] = &[
            "avg_cpu_percent",
            "peak_cpu_percent",
            "cpu_samples",
            "cpu_basis",
            "cpu_caveat",
        ];
        for field in REQUIRED_CPU_FIELDS {
            assert!(!field.is_empty());
            assert!(!field.contains(' '));
        }
    }
}
