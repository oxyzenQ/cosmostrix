// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Benchmark report formatting module.
//!
//! Extracted from `bench.rs` to reduce file pressure before Phase 6
//! visual work. Contains all benchmark output formatting helpers,
//! metric meaning constants, COSMIC DRAGON ENGINE diagnostics formatting,
//! and the premium benchmark report builder.
//!
//! Behavior is unchanged — all fields, labels, and values remain identical
//! to their previous in-line locations in `bench.rs`.

use std::env;

use crate::bench_meta::{cpu_model_label, format_rss_kb};
use crate::diagnostics;
use crate::humanize::{humanize, humanize_bytes};
use crate::renderer_info;
use crate::report::Report;

use crate::{color_mode_label, detect_color_mode_auto};

/// Drift percentage above which a benchmark run is classified as "significant drift".
pub(super) const DRIFT_SIGNIFICANT_PERCENT: f64 = 10.0;
/// Maximum dirty-cell ratio (%) for "stable" classification.
pub(super) const STABILITY_DIRTY_RATIO_MAX: f64 = 5.0;
/// Maximum jitter std for "stable" classification.
pub(super) const STABILITY_JITTER_STD_MAX: f64 = 0.5;

// Re-export the meaning constants so external modules (e.g.,
// cloud/tests/tests_visual_depth.rs, bench.rs tests) can keep using
// `crate::bench_report::*_MEANING` import paths after the constants
// were extracted to bench_meta.rs.
//
// ACTIVE_FRAME_RATIO_MEANING is re-exported for external use (see
// bench_helpers.rs:69 which imports it via `bench_report::`).
// AVG_DIRTY_CELL_RATIO_MEANING is consumed internally at line 453
// (BenchReportData::fmt), so it must be in scope here too.
pub(crate) use crate::bench_meta::{ACTIVE_FRAME_RATIO_MEANING, AVG_DIRTY_CELL_RATIO_MEANING};

// v50.0.0-beta.7 LOC refactor: BenchReportData struct extracted to
// bench_report_data.rs to keep this file under the 800-LOC hard cap.
mod bench_report_data;
pub(crate) use bench_report_data::BenchReportData;

// ── Report builder ───────────────────────────────────────────────────────────

/// Build the premium benchmark report from computed metrics.
///
/// This is the cold-path formatting function. It constructs a `Report`
/// with all required sections (SYSTEM, RENDERER, CONFIG, PERFORMANCE,
/// THROUGHPUT, TIMING, COSMIC DRAGON ENGINE) and prints it to
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
        // Build toolchain + profile metadata (captured at compile time by
        // build.rs, surfaced here so benchmark reports are self-documenting
        // for cross-machine comparison).
        s.field("rustc_version", env!("COSMOSTRIX_RUSTC_VERSION"));
        s.field("git_sha", env!("COSMOSTRIX_GIT_SHA"));
        s.field("cpu_baseline", env!("COSMOSTRIX_CPU_BASELINE"));
        s.field("target_features", env!("COSMOSTRIX_TARGET_FEATURES"));
        s.field("lto", env!("COSMOSTRIX_LTO"));
        s.field("panic", env!("COSMOSTRIX_PANIC"));
        s.field("strip", env!("COSMOSTRIX_STRIP"));
        s.field("pgo", env!("COSMOSTRIX_PGO"));
        // CPU model string (runtime detection) — distinct from the v1/v2/v3/v4
        // variant above. This is the actual chip name, e.g. "Intel(R) Core(TM)
        // i7-12700K CPU @ 3.60GHz". Useful for comparing benchmarks across
        // machines. None on platforms without detection.
        s.field("cpu_model", &cpu_model_label());
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
        // Explicit honest declaration that cosmostrix uses no GPU.
        // cosmostrix is a CPU + stdout renderer — no OpenGL/Vulkan/Metal/
        // DirectX/WebGPU context is ever created. The terminal emulator
        // may use GPU for compositing, but that is outside cosmostrix.
        s.field("gpu_usage", "not_applicable");
        s.field(
            "gpu_basis",
            "cosmostrix is a CPU + stdout renderer; no GPU context is ever created",
        );
    }

    {
        let s = r.section("CONFIG");
        s.field("scene", &data.scene);
        s.field("color_scheme", &data.color_scheme_name);
        // color pipeline parity with --verbose. Without these,
        // two benchmark runs printing `color_scheme: cosmos` could have
        // used completely different render paths (TrueColor vs Color16,
        // custom palette vs built-in, identity tune vs shifted stops).
        s.field("color_mode", data.color_mode_label);
        if let Some(name) = &data.custom_palette_name {
            s.field("color_palette", name);
        }
        if let Some(hex) = &data.custom_palette_bg_hex {
            s.field("color_palette_bg", hex);
        }
        s.field("color_bg", data.color_bg_label);
        s.field("color_tune", &data.color_tune_summary);
        s.field("charset", &data.charset_preset);
        s.field("glyph_count", &data.glyph_count.to_string());
        s.field("rain_style", data.rain_style);
        s.field("monolith_size", data.monolith_size);
        s.field("bold", &data.bold_mode);
        s.field("shading", &data.shading_mode);
        s.field("async_mode", &data.async_mode.to_string());
        s.field("glitch", &data.glitch_enabled.to_string());
        s.field("glitch_level", data.glitch_level);
        s.field("glitch_pct", &format!("{:.1}", data.glitch_pct));
        // (chroma dragon audit): disclose the active color pipeline and
        // the chroma engine status during benchmark. The owner's question
        // "is the chroma dragon enable/disable during --benchmark?" is
        // answered here in plain text. Chroma is ENABLED in benchmark mode
        // — only crystal_dragon `drift` is disabled (palette rebuilds inject
        // timing spikes that break p99/max determinism). Climate drift
        // still runs because it is deterministic.
        s.field("color_pipeline", data.color_pipeline);
        s.field("chroma_in_benchmark", data.chroma_in_benchmark);
        // PERF-1: honest disclosure — cosmetics skipped in bench mode
        s.field(
            "cosmetics_skipped",
            "message border + anomaly zones + CRT vignette + emergent storytelling (bench mode = rain + 3 dragons only)",
        );
        // PERF-2: dragon system + message state for CONFIG transparency.
        s.field("power_dragon", &data.power_dragon.to_string());
        s.field("crystal_dragon", &data.crystal_dragon.to_string());
        s.field("msg_mode", &data.msg_mode.to_string());
        // PERF-2-Supreme: --no-effects state (owner-requested key).
        // true = ALL particle subsystems are no-ops (quantum ripple,
        // border spark, mouse-click flash waves, anomaly zones).
        // Auto-enabled in bench mode (effects_enabled forced false at
        // CloudConfig construction when bench_mode is true).
        // Honest note: particles are mouse/click-driven, so this flag
        // never changes benchmark numbers — it only verifies the
        // CLI/config value took effect.
        s.field("no_effects", &data.no_effects.to_string());
        s.field("intro", data.intro);
        s.field("cols", &data.w.to_string());
        s.field("lines", &data.h.to_string());
        s.field("target_fps", &format!("{:.1}", data.target_fps));
        s.field("density", &format!("{:.2}", data.density));
        s.field("speed", &format!("{:.2}", data.speed));
        s.field("TERM", &term);
        s.field("COLORTERM", &colorterm);
    }

    // ── Benchmark environment (reproducibility metadata) ─────────────
    // Lets users compare reports across machines knowing the OS/governor/
    // terminal context. Rendering extracted to envstat.rs to keep this
    // file under its 800-LOC guard.
    crate::envstat::render_section(&mut r, &data.env);

    // ── COSMIC DRAGON ENGINE METRICS ─────────────────────────────────────────
    // All engine-specific metrics grouped under this header.
    {
        let s = r.section("COSMIC DRAGON ENGINE METRICS");
        // Engine description (audit 2026-08-23): the old one-liner
        // "(diff-based + phosphor)" under-described the engine — the owner
        // remembered it listing the actual techniques. Every term below is
        // verified in source (terminal/draw.rs, frame.rs, bolt.rs,
        // chroma color_cache.rs): the two-path strategy, both RLE forms,
        // the caches, and the zero-alloc guarantee.
        s.field(
            "engine",
            "The Cosmic Dragon Diff-Based Rendering Engine (generation dirty-tracking, dual-path differential/row-RLE, SGR cache, branchless bold, phosphor afterglow)",
        );
        s.field(
            "techniques",
            "1) generation-based dirty tracking: O(1) frame clear via u32 gen bump, wraparound-safe; 2) dual-path strategy: differential below the 12.5% dirty crossover, single-pass row-RLE full redraw above it; 3) contiguous-run batching: sorted dirty indices coalesced into runs with one MoveTo + SGR per run; 4) SGR color cache: pre-formatted escape sequences per palette color; 5) BOLT branchless bold: escape-table lookup instead of branches; 6) SmallVec inline dirty list: 256 slots, zero heap allocation at common terminal sizes; 7) idle-frame fast path: entire render body skipped when nothing changed; 8) double-buffered LastFrame diff cache with Vec reuse across resize storms; 9) phosphor decay afterglow: 3-pass trail system; 10) zero per-frame heap allocation in the differential path",
        );
        s.field("version", env!("CARGO_PKG_VERSION"));
        // disclose the active scene so users can interpret FPS
        // numbers correctly. Different scenes have wildly different
        // throughput characteristics (monolith is ~3-5x faster than
        // cinematic on most terminals). The scene field + scene_note
        // prevents users from comparing apples to oranges across runs.
        s.field("scene", &data.scene);
        s.field(
            "scene_note",
            "FPS varies significantly by scene. The default benchmark scene \
             is 'monolith' (peak throughput). Override with --scene <name>. \
             Other scenes (cinematic, signal, etc.) are heavier and run slower.",
        );
        if data.scene != "monolith" {
            s.field(
                "disclaimer",
                &format!(
                    "Scene '{}' is not the peak-throughput scene. Compare with \
                     'monolith' (the default) for headline FPS claims. The \
                     number above reflects this scene's actual workload, not \
                     the engine's peak capacity.",
                    data.scene
                ),
            );
        }
    }

    {
        let s = r.section("PERFORMANCE");
        s.field("avg_fps", &format!("{:.2}", data.avg_fps));
        s.field("peak_fps", &format!("{:.2}", data.peak_fps));
        s.field(
            "peak_fps_meaning",
            "p1-derived + 1µs floor (fastest plausible 1% of frames); not absolute single-frame peak",
        );
        s.field("avg_frame_time", &format!("{:.4}ms", data.avg_frame_time));
        s.field("p95_frame_time", &format!("{:.4}ms", data.p95_frame_time));
        s.field("p99_frame_time", &format!("{:.4}ms", data.p99_frame_time));
        s.field(
            "p99_9_frame_time",
            &format!("{:.4}ms", data.p99_9_frame_time),
        );
        s.field("max_frame_time", &format!("{:.4}ms", data.max_frame_time));
        s.field(
            "max_frame_time_meaning",
            "worst single-frame spike; what users perceive as jank",
        );
        s.field("frame_jitter", data.jitter_classification);
        s.field("median_fps", &format!("{:.2}", data.median_fps));
        s.field("frame_time_stability", data.frame_time_stability);
        s.field(
            "active_frame_ratio_percent",
            &format!("{:.2}%", data.active_frame_ratio),
        );
        s.field(
            "active_frame_ratio",
            &format!(
                "{:.2}% (frames with >=1 dirty cell)",
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
            &format!(
                "{} ({})",
                humanize(data.max_dirty_cells),
                data.max_dirty_cells
            ),
        );
        s.field(
            "avg_dirty_cell_ratio_percent",
            &format!("{:.2}%", data.avg_dirty_cell_ratio_percent),
        );
        s.field("avg_dirty_cell_ratio_meaning", AVG_DIRTY_CELL_RATIO_MEANING);
        s.field("dirty_all_frames", &data.dirty_all_frames.to_string());
        s.field(
            "dirty_all_frames_meaning",
            "frames that forced a full redraw (>= dirty_threshold dirty cells, or a semantic reset: resize, scene switch, paste)",
        );
        s.field("dirty_threshold_cells", &data.dirty_threshold.to_string());
        s.field(
            "dirty_threshold_meaning",
            "crossover: at or above this many dirty cells the renderer switches from differential to full redraw (grid_cells / 8)",
        );
    }

    {
        let s = r.section("THROUGHPUT");
        // v50 LTS audit (Issue 2): renamed for self-documenting semantics.
        // Old name `glyphs_per_second` implied actual throughput; the
        // value is the theoretical upper bound if every cell were
        // redrawn every frame. Use `dirty_glyphs_per_second` for actual.
        s.field(
            "glyphs_per_second_theoretical",
            &format!(
                "{} ({})",
                humanize(data.glyphs_per_second_theoretical),
                data.glyphs_per_second_theoretical
            ),
        );
        s.field(
            "glyphs_per_second_theoretical_basis",
            "theoretical upper bound: full-frame cell count × active-frame rate (NOT actual). Use dirty_glyphs_per_second for actual rendered throughput.",
        );
        s.field(
            "dirty_glyphs_per_second",
            &format!(
                "{} ({})",
                humanize(data.dirty_glyphs_per_second),
                data.dirty_glyphs_per_second
            ),
        );
        // Percentage companion (audit 2026-08-23): how much of the
        // theoretical ceiling the differential renderer actually draws.
        // 4-5% is typical for monolith (only active streams are dirty).
        if data.glyphs_per_second_theoretical > 0 {
            s.field(
                "render_efficiency_percent",
                &format!(
                    "{:.2}% (dirty / theoretical)",
                    data.dirty_glyphs_per_second as f64 / data.glyphs_per_second_theoretical as f64
                        * 100.0
                ),
            );
        }
        s.field(
            "dirty_glyphs_per_second_basis",
            "actual rendered glyphs/sec — total_drawn_cells / elapsed_s",
        );
        s.field(
            "ansi_bytes_per_second",
            &format!(
                "{} ({})",
                humanize_bytes(data.ansi_bytes_per_second),
                data.ansi_bytes_per_second
            ),
        );
        // v50 LTS audit (Issue 3): added basis note. The 19 bytes/cell
        // figure is documented in constants.rs but was not surfaced in
        // the report, leaving users no way to know the value is an
        // estimate without grep-ping the source.
        s.field(
            "ansi_bytes_per_second_basis",
            "estimated: total_drawn_cells × ANSI_BYTES_PER_CELL_ESTIMATE (19 bytes/cell) / elapsed_s. Actual varies by color mode (TrueColor ≈ 3× Color16) and run-compression; see constants.rs for the 19-byte derivation.",
        );
        s.field("active_streams_avg", &data.active_streams_avg.to_string());
        s.field(
            "cells_drawn_total",
            &format!(
                "{} ({})",
                humanize(data.total_drawn_cells),
                data.total_drawn_cells
            ),
        );
    }

    {
        let s = r.section("TIMING");
        s.field("elapsed", &format!("{:.3}s", data.elapsed_s));
        s.field(
            "total_frames",
            &format!("{} ({})", humanize(data.total_frames), data.total_frames),
        );
        s.field(
            "drawn_frames",
            &format!("{} ({})", humanize(data.drawn_frames), data.drawn_frames),
        );
        // v30 strengthen (audit): removed `frames_with_changes` — it was an
        // exact duplicate of `drawn_frames` (same value, different label).
        // The `drawn_frames` field already means "frames with >=1 dirty cell",
        // which is exactly what `frames_with_changes` claimed to mean.
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

    // ── Resource usage (page faults + context switches) ───────────────
    // Cross-platform via getrusage(RUSAGE_SELF). No permissions required.
    // Deltas computed over the measurement window (cumulative counters
    // sampled at start + end, then subtracted).
    {
        let s = r.section("RESOURCE");
        if let Some(delta) = &data.rusage_delta {
            s.field("minor_faults", &delta.minor_faults.to_string());
            s.field("major_faults", &delta.major_faults.to_string());
            s.field("voluntary_ctxt", &delta.voluntary_ctxt.to_string());
            s.field("involuntary_ctxt", &delta.involuntary_ctxt.to_string());
            s.field(
                "minor_faults_meaning",
                "page reclaims from cache (no disk I/O); high values indicate memory pressure",
            );
            s.field(
                "major_faults_meaning",
                "page faults requiring disk I/O; non-zero means memory not resident",
            );
            s.field(
                "voluntary_ctxt_meaning",
                "process yielded CPU voluntarily (blocking syscall); high = IO-bound",
            );
            s.field(
                "involuntary_ctxt_meaning",
                "process preempted by scheduler (time slice expired); high = CPU contention",
            );
            s.field(
                "resource_basis",
                "getrusage(RUSAGE_SELF) deltas over the measurement window",
            );
        } else {
            s.field("minor_faults", "unsupported");
            s.field("major_faults", "unsupported");
            s.field("voluntary_ctxt", "unsupported");
            s.field("involuntary_ctxt", "unsupported");
            s.field(
                "resource_reason",
                "getrusage not available on this platform (Unix only)",
            );
        }
    }

    // ── Sub-component timing breakdown ─────────────────────────────────
    // Distinguishes "benchmark tool" from "profiling tool": shows where
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
            "cinematic events + spawn rate + droplet physics (cloud.rain_at pre-render)",
        );
        s.field(
            "render_meaning",
            "phosphor decay + anomaly zones + climate fx + message box (frame mutations)",
        );
        s.field(
            "io_meaning",
            "residual: BenchIoWriter write_frame (--bench-io: ANSI to /dev/null) + VisualSampler sampling + clear_dirty + loop bookkeeping",
        );
        let total_avg = data.avg_sim_ms + data.avg_render_ms + data.avg_io_ms;
        if total_avg > 0.0 {
            s.field(
                "sim_share_percent",
                &format!("{:.2}", data.avg_sim_ms / total_avg * 100.0),
            );
            s.field(
                "render_share_percent",
                &format!("{:.2}", data.avg_render_ms / total_avg * 100.0),
            );
            s.field(
                "io_share_percent",
                &format!("{:.2}", data.avg_io_ms / total_avg * 100.0),
            );
        }
    }

    // ── P3: Cell Efficiency (DeepSeek metrics) ──────────────────────
    // Size-independent metrics: ns/cell lets you compare algorithm
    // efficiency across different terminal sizes. If ns/cell stays
    // constant as size grows, the algorithm is O(n). If it grows,
    // there's a super-linear component (O(n²) or worse).
    {
        let s = r.section("CELL EFFICIENCY");
        s.field(
            "logical_cells_per_frame",
            &crate::humanize::humanize(data.logical_cells_per_frame),
        );
        s.field(
            "dirty_cells_per_frame",
            &format!("{:.1}", data.avg_dirty_cells_per_frame),
        );
        // Percentage companion (owner request 2026-08-23): dirty share of
        // the grid, so the differential-rendering efficiency is readable
        // at a glance without dividing two numbers mentally.
        s.field(
            "dirty_cell_ratio_percent",
            &format!(
                "{:.2}% ({} of {} cells)",
                data.avg_dirty_cell_ratio_percent,
                crate::humanize::humanize(data.avg_dirty_cells_per_frame as u64),
                crate::humanize::humanize(data.logical_cells_per_frame)
            ),
        );
        // Component ns/cell trio (audit 2026-08-23): sim was previously
        // missing, so render+io visibly did not add up to total. All three
        // components now shown and share the same denominator (avg dirty
        // cells per frame), so sim + render + io == total.
        let per_cell = |ms: f64| {
            if data.avg_dirty_cells_per_frame > 0.0 {
                ms * 1_000_000.0 / data.avg_dirty_cells_per_frame
            } else {
                0.0
            }
        };
        let sim_ns = per_cell(data.avg_sim_ms);
        let render_ns = data.render_ns_per_cell;
        let io_ns = data.io_ns_per_cell;
        let total_ns = data.total_ns_per_cell;
        s.field("sim_ns_per_cell", &format!("{:.2}", sim_ns));
        s.field("render_ns_per_cell", &format!("{:.2}", render_ns));
        s.field("io_ns_per_cell", &format!("{:.2}", io_ns));
        s.field("total_ns_per_cell", &format!("{:.2}", total_ns));
        // Component shares of the per-cell total — the beginner-friendly
        // view: which stage dominates the per-cell cost.
        //
        // Precision note (audit 2026-08-23): per frame, sim+render+io sums
        // to the measured frame body EXACTLY (io_ms is defined as the
        // residual `frame_time - sim - render`). But total_ns_per_cell
        // derives from avg_frame_time = elapsed/total_frames, the
        // wall-clock average, which also includes ~1-3% of loop time
        // outside the measured body (progress UI, sampler ticks). The
        // shares are therefore normalized against the component sum (they
        // add to 100% of measured work), and component_coverage_percent
        // discloses how much of the wall-clock total the component timers
        // account for — no hidden rounding, no invisible gap.
        let component_sum_ns = sim_ns + render_ns + io_ns;
        if component_sum_ns > 0.0 {
            s.field(
                "sim_share_percent",
                &format!("{:.1}", sim_ns / component_sum_ns * 100.0),
            );
            s.field(
                "render_share_percent",
                &format!("{:.1}", render_ns / component_sum_ns * 100.0),
            );
            s.field(
                "io_share_percent",
                &format!("{:.1}", io_ns / component_sum_ns * 100.0),
            );
            if total_ns > 0.0 {
                s.field(
                    "component_coverage_percent",
                    &format!(
                        "{:.1} (component timers / wall-clock total)",
                        component_sum_ns / total_ns * 100.0
                    ),
                );
            }
            s.field(
                "share_basis",
                "sim/render/io shares of measured frame work (sum to 100%); component_coverage shows how much of total_ns_per_cell the timers explain — the rest is loop bookkeeping outside the measured frame body",
            );
        }
        s.field(
            "ns_per_cell_meaning",
            "nanoseconds per dirty cell; lower = more efficient; size-independent",
        );
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
                s.field("first_half_fps", &format!("{:.2}", f));
                s.field("second_half_fps", &format!("{:.2}", s2));
                s.field("fps_drift_percent", &format!("{:+.2}%", d));
                // Interpret the drift value for the user.
                let interpretation = if d > DRIFT_SIGNIFICANT_PERCENT {
                    "degraded — possible thermal throttle / allocator pressure / cache pollution"
                } else if d < -DRIFT_SIGNIFICANT_PERCENT {
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

    // v50.0.0-beta.7 LOC refactor: engine + terminal + energy +
    // microarchitecture + allocator + visual objective sections
    // extracted to bench_report_engine.rs.
    crate::bench_report_engine::build_engine_sections(&mut r, data);
}

// ── Tests ──────────────────────────────────────────────────────────────────
