// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Benchmark report formatting module.
//!
//! Extracted from `bench.rs` to reduce file pressure before Phase 6
//! visual work. Contains all benchmark output formatting helpers,
//! metric meaning constants, ZACTRIX ENGINE diagnostics formatting,
//! and the premium benchmark report builder.
//!
//! Behavior is unchanged — all fields, labels, and values remain identical
//! to their previous in-line locations in `bench.rs`.

use std::env;

use crate::bench_meta::{cpu_model_label, format_rss_kb};
use crate::constants::DIRTY_THRESHOLD_RATIO;
use crate::diagnostics;
use crate::renderer_info;
use crate::report::Report;
use crate::runtime::ColorMode;

use super::{color_mode_label, detect_color_mode_auto};

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
    /// Active scene name for this benchmark run (e.g. "cinematic", "monolith",
    /// "signal"). Resolved from `CloudConfig::scene_name` so the report makes
    /// clear which scene generated the metrics — critical for comparing runs.
    pub scene: String,
    /// Canonical color scheme name (e.g. "cosmos", "neon-purple", "green").
    /// Resolved from `CloudConfig::color_scheme` via `theme::canonical_name_for_scheme`.
    /// Lets users reproduce a run's exact palette without guessing the enum
    /// variant from a Debug dump.
    pub color_scheme_name: String,
    /// Charset preset name as supplied on the CLI (e.g. "matrix", "zen",
    /// "katakana"). For custom charsets defined via `[charset-custom.<name>]`
    /// in config.toml, this is the custom block name the user passed to
    /// `--charset <name>` (or "auto" if they didn't pass --charset).
    pub charset_preset: String,
    /// Number of distinct glyphs in the active char pool. Benchmark throughput
    /// is glyph-pool-size invariant (the renderer doesn't slow down with more
    /// glyphs), but having this number in the report makes it obvious whether
    /// a run used a tiny pool (zen: 1 glyph) or a large one (katakana: ~80).
    pub glyph_count: usize,
    /// Rain style: "glyph" (default per-column rain) or "monolith" (single
    /// central pillar). Surfaces the `--scene monolith` vs `--scene cinematic`
    /// distinction in the CONFIG section so FPS comparisons across scenes
    /// can be interpreted correctly.
    pub rain_style: &'static str,
    /// Monolith size ("small"/"normal"/"large"). Only meaningful when
    /// `rain_style == "monolith"`; included unconditionally for completeness.
    pub monolith_size: &'static str,
    /// Bold mode: "Off"/"Random"/"All". Affects glyph weight, which has a
    /// small but measurable impact on terminal rendering throughput (bold
    /// glyphs use a different SGR sequence).
    pub bold_mode: String,
    /// Shading mode: "Random"/"DistanceFromHead". Affects whether droplet
    /// colors are picked per-cell or computed from head distance — the
    /// latter is slightly more expensive but produces smoother gradients.
    pub shading_mode: String,

    // ── v25.17: CONFIG enrichment (color/charset/etc. parity with --verbose) ──
    // These fields close the gap between the rich `--verbose` dump and the
    // sparse benchmark CONFIG section. Benchmark reproducibility depends on
    // knowing the EXACT color pipeline (mode, palette, tune, bg) and the
    // glitch/async flags — without these, two runs that print the same
    // `color_scheme: cosmos` can have wildly different throughputs.
    /// Human-readable color mode label (e.g. "24-bit truecolor", "16-color").
    /// TrueColor emits ~3x more ANSI bytes per cell than Color16, so this
    /// is critical for interpreting ansi_bytes_per_second.
    pub color_mode_label: &'static str,
    /// Active custom palette name, or None when a built-in scheme is in use.
    /// Lets users reproduce a run that used `--colors-custom mythme`.
    pub custom_palette_name: Option<String>,
    /// Hex string of the custom palette's bg field (e.g. "#0000ce"), or None
    /// when no custom palette is active OR the palette has no bg field.
    /// Surfaces the actual on-screen background so users can verify the
    /// config.toml `bg = "..."` value was applied.
    pub custom_palette_bg_hex: Option<String>,
    /// `--color-bg` setting as a descriptive label ("black", "default-background").
    /// Combined with custom_palette_bg_hex, fully disambiguates the bg pipeline.
    pub color_bg_label: &'static str,
    /// Compact color-tune summary ("sat=1.00 bright=1.00 head=1.00 body=1.00 tail=1.00").
    /// Non-identity tune changes the palette stops, which can shift SGR byte counts.
    pub color_tune_summary: String,
    /// Async mode flag (true = variable column speeds, false = uniform).
    /// Affects droplet spawn distribution and dirty-cell patterns per frame.
    pub async_mode: bool,
    /// Glitch enabled flag (derived from `glitch_level != GlitchLevel::None`).
    pub glitch_enabled: bool,
    /// Glitch intensity preset name ("none"/"subtle"/"default"/...).
    pub glitch_level: &'static str,
    /// Glitch trigger probability as a percentage (0.0–100.0).
    pub glitch_pct: f32,
    /// Auto color drift flag. When true, the ColorEcosystem autonomously
    /// rotates palettes — long benchmark runs may cross palette transitions.
    pub auto_color_drift: bool,
    /// v30.3 (chroma dragon audit): the active color pipeline label
    /// (`chroma_dragon` or `legacy_rgb`). Mirrors the `color_pipeline:`
    /// line in `cosmostrix -v` and the `color_pipeline` field in
    /// `cosmostrix --doctor` RENDERER section. The benchmark report must
    /// disclose this so the user can answer "is the chroma dragon running
    /// during my benchmark?" without reading the source.
    pub color_pipeline: &'static str,
    /// v30.3 (chroma dragon audit): human-readable chroma engine status
    /// during the benchmark run. Explains the relationship between
    /// benchmark mode and the chroma engine (palette drift is disabled
    /// for deterministic p99/max, but climate drift still runs because
    /// it is deterministic and has no rebuild cost). The owner asked:
    /// "when benchmarking mode 'cosmostrix --benchmark' is the chroma
    /// dragon enable/disable?" — this field answers that question in
    /// the report itself.
    pub chroma_in_benchmark: &'static str,

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

    // P3: Cells per frame (DeepSeek metrics)
    /// Total logical cells per frame = width × height.
    pub logical_cells_per_frame: u64,
    /// Nanoseconds per cell for the render phase (render_ms / dirty_cells).
    /// Lower = more efficient. Size-independent metric for algorithm comparison.
    pub render_ns_per_cell: f64,
    /// Nanoseconds per cell for the IO/bookkeeping phase (io_ms / dirty_cells).
    pub io_ns_per_cell: f64,
    /// Total nanoseconds per cell (render + io + sim) / dirty_cells.
    pub total_ns_per_cell: f64,

    // Phase 2: Terminal I/O wet metrics (None when --bench-io not used)
    pub terminal_io: Option<crate::bench_io::TerminalIoMetrics>,

    // Phase 3: RAPL energy metrics (Linux only)
    pub energy: Option<crate::bench_energy::EnergyMetrics>,

    // Phase 4: Perf counter metrics (Linux x86 only)
    pub perf: Option<crate::bench_perf::PerfMetrics>,

    // Phase 5: Allocator tracing metrics
    pub allocator: Option<crate::alloc_trace::AllocMetrics>,

    // Phase 6: Visual objective metrics
    pub visual: Option<crate::bench_visual::VisualMetrics>,

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

    // Resource usage deltas (page faults + context switches) over the
    // measurement window. None on platforms without getrusage. Cumulative
    // counters sampled at start + end, then subtracted.
    pub rusage_delta: Option<crate::usagestat::ResourceSnapshot>,

    // Benchmark environment (reproducibility metadata). Collected once
    // at benchmark start — no per-frame cost. Lets users compare reports
    // across machines knowing the OS/governor/terminal context.
    pub env: crate::envstat::EnvSnapshot,

    // Sub-component timing breakdown (averages + peaks, in ms).
    // sim_ms    = time in cloud.rain_at() before the first frame mutation
    //             (cinematic events, spawn rate, droplet physics).
    // render_ms = time in cloud.rain_at() during phosphor/anomaly/climate
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
/// THROUGHPUT, TIMING, ZACTRIX ENGINE) and prints it to
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
        // Cosmostrix is a CPU + stdout renderer — no OpenGL/Vulkan/Metal/
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
        // v25.17: color pipeline parity with --verbose. Without these,
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
        s.field("auto_color_drift", &data.auto_color_drift.to_string());
        // v30.3 (chroma dragon audit): disclose the active color pipeline and
        // the chroma engine status during benchmark. The owner's question
        // "is the chroma dragon enable/disable during --benchmark?" is
        // answered here in plain text. Chroma is ENABLED in benchmark mode
        // — only palette *drift* is disabled (palette rebuilds inject
        // timing spikes that break p99/max determinism). Climate drift
        // still runs because it is deterministic.
        s.field("color_pipeline", data.color_pipeline);
        s.field("chroma_in_benchmark", data.chroma_in_benchmark);
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
    // file under its 1000-LOC guard.
    crate::envstat::render_section(&mut r, &data.env);

    // ── COSMIC DRAGON ENGINE METRICS ─────────────────────────────────────────
    // All engine-specific metrics grouped under this header.
    {
        let s = r.section("COSMIC DRAGON ENGINE METRICS");
        s.field(
            "engine",
            "The Cosmic Dragon Diff-Based Rendering Engine (diff-based + RLE + phosphor)",
        );
        s.field("version", env!("CARGO_PKG_VERSION"));
        // v25.16: disclose the active scene so users can interpret FPS
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
        s.field("dirty_threshold_cells", &data.dirty_threshold.to_string());
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
        s.field(
            "render_ns_per_cell",
            &format!("{:.1}", data.render_ns_per_cell),
        );
        s.field("io_ns_per_cell", &format!("{:.1}", data.io_ns_per_cell));
        s.field(
            "total_ns_per_cell",
            &format!("{:.1}", data.total_ns_per_cell),
        );
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
    //
    // v30 strengthen (audit): removed the entire SYSTEM section. Its four
    // fields (runtime_mode, render_plan, idle_policy, architecture) were
    // all hardcoded string constants with no runtime basis — they described
    // a fictional "runtime mode" that doesn't exist, and duplicated concepts
    // already covered by the ENGINE section below (render_plan="single-owner"
    // == ENGINE.terminal_writer="single-owner"). Keeping them would violate
    // the honesty contract: the report must reflect actual state, not
    // feel-good constants.
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

    // Atmosphere Engine subsystem eliminated (Dragon Hunt v2 Phase 6 Tier E
    // item 31 — full elimination). The entire ATMOSPHERE diagnostic section
    // has been removed. Design knowledge preserved in
    // docs/archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md.

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

    // ── Phase 2: Terminal I/O (wet) ──────────────────────────────────
    {
        let s = r.section("TERMINAL I/O");
        match &data.terminal_io {
            Some(io) if io.enabled => {
                s.field("status", "enabled (wet)");
                s.field("target", &io.target);
                s.field(
                    "write_bandwidth",
                    &format!("{:.1} MB/s", io.bandwidth_mbps()),
                );
                s.field(
                    "avg_write_latency",
                    &format!("{:.1} µs", io.avg_latency_us()),
                );
                s.field("backpressure_events", &io.backpressure_events.to_string());
                s.field(
                    "effective_write_fps",
                    &crate::humanize::humanize_f64(io.effective_write_fps()),
                );
                s.field(
                    "total_bytes_written",
                    &crate::humanize::humanize(io.bytes_written),
                );
            }
            _ => {
                s.field("status", "dry (use --bench-io for wet mode)");
            }
        }
    }

    // ── Phase 3: ENERGY (RAPL, Linux only) ───────────────────────────
    {
        let s = r.section("ENERGY");
        match &data.energy {
            Some(e) if e.available => {
                s.field("status", "available (RAPL)");
                s.field("packages", &e.package_count.to_string());
                s.field("total_energy", &format!("{:.2} J", e.total_energy_joules));
                s.field("avg_power", &format!("{:.2} W", e.avg_power_watts));
                s.field(
                    "energy_per_frame",
                    &format!("{:.1} µJ", e.energy_per_frame_uj),
                );
                s.field(
                    "energy_per_cell",
                    &format!("{:.1} nJ", e.energy_per_cell_nj),
                );
            }
            _ => {
                s.field("status", "not available (RAPL requires Linux + powercap)");
                s.field(
                    "hint",
                    "See docs/BENCHMARK_ADVANCED.md for setup instructions",
                );
            }
        }
    }

    // ── Phase 4: MICROARCHITECTURE (perf counters, Linux x86 only) ───
    {
        let s = r.section("MICROARCHITECTURE");
        match &data.perf {
            Some(p) if p.available => {
                s.field("status", "available (perf_event_open)");
                s.field("cycles", &crate::humanize::humanize(p.cycles));
                s.field("instructions", &crate::humanize::humanize(p.instructions));
                s.field("ipc", &format!("{:.2}", p.instructions_per_cycle));
                s.field(
                    "branch_instructions",
                    &crate::humanize::humanize(p.branch_instructions),
                );
                s.field("branch_misses", &crate::humanize::humanize(p.branch_misses));
                s.field(
                    "branch_mispredict_rate",
                    &format!("{:.2}%", p.branch_mispredict_rate),
                );
                s.field("note", "Linux x86 perf counters; varies by CPU model");
            }
            _ => {
                s.field("status", "not available (perf counters require Linux x86)");
                s.field(
                    "hint",
                    "See docs/BENCHMARK_ADVANCED.md for setup instructions",
                );
            }
        }
    }

    // ── Phase 5: ALLOCATOR ────────────────────────────────────────────
    {
        let s = r.section("ALLOCATOR");
        match &data.allocator {
            Some(a) => {
                s.field("alloc_calls", &crate::humanize::humanize(a.alloc_calls));
                s.field("dealloc_calls", &crate::humanize::humanize(a.dealloc_calls));
                s.field("realloc_calls", &crate::humanize::humanize(a.realloc_calls));
                s.field(
                    "alloc_calls_per_frame",
                    &format!("{:.1}", a.alloc_calls_per_frame),
                );
                s.field(
                    "dealloc_calls_per_frame",
                    &format!("{:.1}", a.dealloc_calls_per_frame),
                );
                s.field(
                    "bytes_allocated",
                    &crate::humanize::humanize(a.bytes_allocated_total),
                );
                s.field(
                    "bytes_deallocated",
                    &crate::humanize::humanize(a.bytes_deallocated_total),
                );
                s.field(
                    "heap_retained",
                    &crate::humanize::humanize(a.heap_retained_bytes),
                );
                if a.heap_virtual_kib > 0 {
                    s.field("heap_virtual", &format!("{} KiB", a.heap_virtual_kib));
                }
            }
            None => {
                s.field("status", "not measured");
            }
        }
    }

    // ── Phase 6: VISUAL OBJECTIVE METRICS ────────────────────────────
    {
        let s = r.section("VISUAL OBJECTIVE");
        match &data.visual {
            Some(v) => {
                s.field(
                    "frame_entropy_bits",
                    &format!("{:.2}", v.frame_entropy_bits),
                );
                s.field("density_gini", &format!("{:.4}", v.density_gini));
                s.field(
                    "color_transition_delta",
                    &format!("{:.2}", v.color_transition_delta_avg),
                );
                s.field("samples", &v.samples.to_string());
                s.field(
                    "entropy_meaning",
                    "Shannon entropy of dirty-cell column distribution; higher = more uniform",
                );
                s.field(
                    "gini_meaning",
                    "0 = perfectly uniform density, 1 = maximally concentrated",
                );
            }
            None => {
                s.field("status", "not measured");
            }
        }
    }

    // Final report goes to stdout — clean, pipeable.
    r.print();
}

// ── Tests ──────────────────────────────────────────────────────────────────
