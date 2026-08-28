// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! BenchReportData struct — extracted from `bench_report.rs` to keep
//! that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! All computed metrics needed to build the premium benchmark report.
//! Populated by the measurement loop in `bench/mod.rs` and consumed by
//! `build_premium_report` to produce the final formatted output. This
//! struct keeps the hot measurement code decoupled from the cold
//! report-formatting code.
//!
//! Re-exported from `bench_report.rs` via `pub(crate) use` so all
//! existing `crate::bench_report::BenchReportData` call sites resolve
//! unchanged.

use crate::runtime::ColorMode;

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

    // ── CONFIG enrichment (color/charset/etc. parity with --verbose) ──
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
    /// Active color pipeline label.
    /// (`chroma_dragon` or `legacy_rgb`). Mirrors the `color_pipeline:`
    /// line in `cosmostrix -v` and the `color_pipeline` field in
    /// `cosmostrix --doctor` RENDERER section. The benchmark report must
    /// disclose this so the user can answer "is the chroma dragon running
    /// during my benchmark?" without reading the source.
    pub color_pipeline: &'static str,
    /// (chroma dragon audit): human-readable chroma engine status
    /// during the benchmark run. Explains the relationship between
    /// benchmark mode and the chroma engine (crystal_dragon palette drift
    /// is forced OFF for deterministic p99/max, but climate drift still
    /// runs because it is deterministic and has no rebuild cost). The
    /// owner asked: "when benchmarking mode 'cosmostrix --benchmark' is
    /// the chroma dragon enable/disable?" — this field answers that
    /// question in the report itself.
    pub chroma_in_benchmark: &'static str,
    /// PERF-2: power_dragon state (transparency — not a bench throttle).
    pub power_dragon: bool,
    /// PERF-2: crystal_dragon state (false in bench for determinism).
    pub crystal_dragon: bool,
    /// PERF-2: msg_mode state (skipped in bench per Z-6, shown for parity).
    pub msg_mode: bool,
    /// PERF-2: intro type label (not rendered in bench, config parity).
    pub intro: &'static str,
    /// PERF-2-Supreme: particle effects disabled state (--no-effects,
    /// inverted: true = effects OFF). Auto-enabled in bench mode.
    /// Transparency-only field: particles are input-driven and never
    /// spawn during benchmark runs.
    pub no_effects: bool,

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
    // v50 LTS audit (Issue 2): renamed `glyphs_per_second` →
    // `glyphs_per_second_theoretical` because the value is the
    // theoretical upper bound (full-frame cell count × active-frame
    // rate), NOT the actual rendered throughput. Actual rendered
    // throughput is `dirty_glyphs_per_second`. The old name misled
    // users into thinking it was measured work; the new name makes
    // the semantics self-documenting.
    // The redundant `theoretical_full_frame_glyphs_per_second` field
    // (which held the exact same value) was removed at the same time.
    pub glyphs_per_second_theoretical: u64,
    pub dirty_glyphs_per_second: u64,
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
