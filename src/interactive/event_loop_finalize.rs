// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Post-loop session finalization — extracted from `event_loop.rs`.
//!
//! Owns the entire post-loop sequence: shutdown signal, terminal stats
//! capture, the `--perf-stats` performance report (162 lines of
//! `Report` formatting), terminal drop (AB-10 alt-screen restore), and
//! the final-state handoff to `super::set_final_state` for the post-exit
//! verbose summary. Extracting this block keeps `event_loop.rs` under
//! the 1500-LOC file cap without touching the loop body's tight coupling.
//!
//! All counters are read-only borrows; `term` is moved in (consumed by
//! `drop`). Nothing flows back to the caller except `Ok(())`.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::bench_helpers::format_backpressure_section;
use crate::cloud::Cloud;
use crate::constants::*;
use crate::humanize::{humanize, humanize_bytes, humanize_bytes_f64, humanize_throughput};
use crate::report::Report;
use crate::terminal::Terminal;
// CloudConfig is re-exported at the crate root (main.rs: pub use app::CloudConfig).
use crate::CloudConfig;

use super::activity::FrameTimeTracker;
use super::watchdog::SHUTDOWN;

/// Bundled perf counters + references needed by the post-loop report.
///
/// Every field is a snapshot taken at loop exit — the finalize path does
/// not mutate any of them. The struct exists only to keep
/// [`finalize_session`] signature readable (would otherwise be 21 params).
pub(crate) struct SessionStats<'a> {
    pub start_time: Instant,
    pub perf_frames: u64,
    pub perf_drawn_frames: u64,
    pub perf_idle_frames: u64,
    pub perf_overshoot_frames: u64,
    pub perf_dirty_sum: u64,
    pub perf_dirty_samples: u64,
    /// Live grid dimensions at loop exit — denominator for the
    /// avg_dirty_cell_ratio_percent field (owner request 2026-08-23).
    /// Snapshot at exit; a mid-session resize makes the ratio approximate
    /// for pre-resize frames (documented at the field's use site).
    pub grid_cols: u16,
    pub grid_lines: u16,
    pub perf_work_sum_s: f64,
    pub perf_work_max_s: f64,
    pub perf_pressure_sum: f64,
    pub perf_pressure_max: f32,
    pub perf_utilization_sum: f64,
    pub perf_utilization_max: f32,
    pub frame_time_tracker: &'a FrameTimeTracker,
    pub power_manager_phase_transitions: u64,
    pub power_manager_base_target_fps: f64,
    pub endurance_health_score: f64,
    pub endurance_health_classification: &'static str,
}

/// Run the entire post-loop finalization sequence.
///
/// Order matters (AB-10 rain-screen cleanliness):
/// 1. Set `SHUTDOWN` so the watchdog doesn't false-alarm after normal exit.
/// 2. Compute the final FPS summary line (deferred print until after drop).
/// 3. Capture terminal encoding stats BEFORE drop.
/// 4. `drop(term)` — restores alt screen so subsequent eprintln lands on
///    the main screen, not polluting the rain matrix.
/// 5. Print the final FPS summary line if `--perf-stats` (v50.0.0-beta.6:
///    moved to BEFORE the perf report so it acts as a consistent header).
/// 6. Print the perf report if `--perf-stats` (large `Report` block).
/// 7. Hand off final color/scene/charset/density/speed to
///    `super::set_final_state` for the post-exit verbose summary.
pub(crate) fn finalize_session(
    stats: &SessionStats,
    term: Terminal,
    cloud: &Cloud,
    scene_name: &str,
    charset_preset: &str,
    cfg: &CloudConfig,
) -> std::io::Result<()> {
    SHUTDOWN.store(true, Ordering::Release);

    let final_elapsed = stats.start_time.elapsed();
    let final_elapsed_s = final_elapsed.as_secs_f64().max(BENCH_ELAPSED_MIN_S);
    let final_avg_fps = (stats.perf_frames as f64) / final_elapsed_s;
    let last_work_ms = stats.frame_time_tracker.rolling_avg_ms();
    let final_instant_fps = if last_work_ms > 0.0 {
        (1000.0 / last_work_ms).min(cfg.target_fps)
    } else {
        cfg.target_fps
    };
    let final_fps_line = format!(
        "[cosmostrix] final FPS: {:.1} (instant: {:.1}, target: {:.1}), frames: {}, elapsed: {:.2}s",
        final_avg_fps, final_instant_fps, cfg.target_fps, stats.perf_frames, final_elapsed_s
    );

    let (enc_bytes, enc_flushes, sgr_hits, sgr_misses) = term.encoding_stats();
    let (tier2_skips, tier2_resets, tier2_bytes_since) = term.tier2_stats();

    // AB-10: drop the terminal BEFORE any stderr write so the alt screen
    // is restored and the final FPS line lands on the main screen, not
    // polluting the rain matrix on exit.
    drop(term);

    // v50.0.0-beta.6: print the final FPS summary line BEFORE the perf
    // report so it acts as a consistent header (owner request — the line
    // previously appeared AFTER the report, which was an inconsistent
    // position for a summary). Now the user sees the one-liner first,
    // then the detailed report below it.
    if cfg.perf_stats {
        crate::output::eprintln_safe!("{}", final_fps_line);
    }

    // Print the perf report AFTER the final FPS header — stdout is
    // captured by the alt-screen buffer and lost when Terminal::drop()
    // restores the main screen. Using eprint() (stderr) so the report
    // survives the restore.
    if cfg.perf_stats {
        print_perf_report(
            stats,
            final_elapsed_s,
            final_instant_fps,
            enc_bytes,
            enc_flushes,
            sgr_hits,
            sgr_misses,
            tier2_skips,
            tier2_resets,
            tier2_bytes_since,
        );
    }

    let final_color_name = if cloud.custom_palette_active {
        cfg.custom_palette_name
            .as_deref()
            .map(|n| format!("{n} (custom)"))
            .unwrap_or_else(|| format!("{:?}", cloud.color_scheme()))
    } else {
        format!("{:?}", cloud.color_scheme())
    };
    super::set_final_state(
        &final_color_name,
        scene_name,
        charset_preset,
        cloud.chars_per_sec,
        cloud.droplet_density,
        // v50.0.0-alpha.7: extended fields for live-reload honesty.
        cfg.msg_mode,
        cfg.message.as_deref(),
        cfg.message_border,
        cfg.power_dragon,
        cfg.crystal_dragon,
        cfg.async_mode,
        cfg.intro_color.as_deref(),
        // v50.0.0-beta.7 LTS: ambient effective state (post-live-reload).
        // Owner audit: previously missing — final_runtime_verbose had no
        // way to show what snapback delay / schedule count was in effect.
        cfg.ambient_snapback_secs,
        cfg.ambient_schedule.entries.len(),
    );

    // v50.0.0-beta.6: final FPS line now printed BEFORE the perf report
    // (moved above — see the comment near the drop(term) call). This
    // section previously printed it here (after the report), which was
    // an inconsistent position for a summary header.

    Ok(())
}

/// Build + print the `--perf-stats` performance report.
///
/// Pure formatting — no mutation of `stats`. Gated by `cfg.perf_stats` in
/// the caller; this function assumes the gate is already open.
#[allow(clippy::too_many_arguments)]
fn print_perf_report(
    stats: &SessionStats,
    elapsed_s: f64,
    final_instant_fps: f64,
    enc_bytes: u64,
    enc_flushes: u64,
    sgr_hits: u64,
    sgr_misses: u64,
    tier2_skips: u64,
    tier2_resets: u64,
    tier2_bytes_since: u64,
) {
    let frames = stats.perf_frames.max(1);
    let avg_work_ms = (stats.perf_work_sum_s / frames as f64) * 1000.0;
    let avg_pressure = stats.perf_pressure_sum / frames as f64;
    let avg_fps = (stats.perf_frames as f64) / elapsed_s;
    let drawn_ratio = (stats.perf_drawn_frames as f64) / (stats.perf_frames as f64).max(1.0);
    let overshoot_ratio =
        (stats.perf_overshoot_frames as f64) / (stats.perf_frames as f64).max(1.0) * 100.0;
    let pressure_class = if avg_pressure < PERF_PRESSURE_CLASS_LOW {
        "low"
    } else if avg_pressure < PERF_PRESSURE_CLASS_MEDIUM {
        "medium"
    } else {
        "high"
    };

    let mut r = Report::new("COSMOSTRIX PERFORMANCE REPORT");

    {
        let s = r.section("TIMING");
        s.field("elapsed", &format!("{:.3}s", elapsed_s));
        s.field(
            "target_fps",
            &format!("{:.3}", stats.power_manager_base_target_fps),
        );
        s.field("avg_fps", &format!("{:.3}", avg_fps));
        // v30: real instantaneous FPS from last ~1s of frame work times.
        // Capped at target_fps (loop sleeps to maintain target). Read
        // this for "what FPS am I seeing now" — distinct from avg_fps
        // (whole-run average) and BACKPRESSURE.avg (load-shed signal).
        s.field("instant_fps", &format!("{:.3}", final_instant_fps));
        s.field(
            "rolling_avg_frame_time",
            &format!("{:.3}ms", stats.frame_time_tracker.rolling_avg_ms()),
        );
    }

    {
        let s = r.section("FRAMES");
        s.field("total", &stats.perf_frames.to_string());
        s.field(
            "drawn",
            &format!("{} ({:.1}%)", stats.perf_drawn_frames, drawn_ratio * 100.0),
        );
        s.field(
            "idle_visual",
            &format!(
                "{} ({:.1}%)",
                stats.perf_idle_frames,
                (stats.perf_idle_frames as f64) / (stats.perf_frames as f64).max(1.0) * 100.0
            ),
        );
        s.field(
            "overshoot",
            &format!("{} ({:.1}%)", stats.perf_overshoot_frames, overshoot_ratio),
        );
    }

    {
        let s = r.section("MOTION");
        let avg_dirty = if stats.perf_dirty_samples > 0 {
            stats.perf_dirty_sum as f64 / stats.perf_dirty_samples as f64
        } else {
            0.0
        };
        // v50.0.0-beta.6: show total cell count so the user can see the
        // full grid size alongside the average dirty cells (owner request
        // — previously only avg_dirty_cells was shown with no total context,
        // causing confusion about what the number means relative to the grid).
        let total_cells = (stats.grid_cols as u64) * (stats.grid_lines as u64);
        s.field(
            "total_cells",
            &format!(
                "{} ({}x{} grid)",
                humanize(total_cells),
                stats.grid_cols,
                stats.grid_lines
            ),
        );
        s.field(
            "avg_dirty_cells",
            &format!("{:.1} (of {} total)", avg_dirty, humanize(total_cells)),
        );
        // Dirty-cell coverage as a percentage of the live grid, for
        // easy reading (owner request 2026-08-23). Same semantics as the
        // benchmark's avg_dirty_cell_ratio_percent: avg dirty cells / total
        // logical cells. Denominator is the exit-time grid — a resize
        // mid-session makes the ratio approximate for the pre-resize frames
        // (the benchmark has no resize, so its ratio is exact).
        let total_cells_f64 = (stats.grid_cols as f64) * (stats.grid_lines as f64);
        let dirty_ratio_pct = if total_cells_f64 > 0.0 {
            avg_dirty / total_cells_f64 * 100.0
        } else {
            0.0
        };
        s.field(
            "avg_dirty_cell_ratio_percent",
            &format!(
                "{:.2}% (of {}x{} grid)",
                dirty_ratio_pct, stats.grid_cols, stats.grid_lines
            ),
        );
        s.field(
            "visual_fps_hint",
            &format!(
                "{:.1} ({} of {} frames had visual changes)",
                drawn_ratio * stats.power_manager_base_target_fps,
                stats.perf_drawn_frames,
                stats.perf_frames
            ),
        );
    }

    {
        let s = r.section("LATENCY");
        s.field("avg_frame_time", &format!("{:.3}ms", avg_work_ms));
        s.field(
            "max_frame_time",
            &format!("{:.3}ms", stats.perf_work_max_s * 1000.0),
        );
        s.field("jitter", stats.frame_time_tracker.jitter_classification());
    }

    {
        // Backpressure = clamp(work/budget - 1, 0, 2): non-zero ONLY when
        // renderer can't keep up. budget_utilization = work/budget (always
        // non-zero) — companion so the section is informative on healthy hw.
        // avg_frame_period_ms = elapsed/frames (the FULL period: work +
        // sleep + event polling) — bridges the pressure-vs-utilization gap
        // (audit 2026-08-23).
        format_backpressure_section(
            &mut r,
            avg_pressure,
            stats.perf_pressure_max,
            stats.perf_utilization_sum,
            stats.perf_utilization_max,
            stats.perf_frames,
            Duration::from_secs_f64(1.0 / stats.power_manager_base_target_fps),
            avg_work_ms,
            pressure_class,
            stats.perf_overshoot_frames,
            overshoot_ratio,
            (elapsed_s / frames.max(1) as f64) * 1000.0,
        );
    }

    // P5: Endurance health score
    {
        let s = r.section("ENDURANCE");
        s.field(
            "health_score",
            &format!("{:.1}/100", stats.endurance_health_score),
        );
        s.field("classification", stats.endurance_health_classification);
        s.field(
            "phase_transitions",
            &stats.power_manager_phase_transitions.to_string(),
        );
    }

    // ENCODING: actual measured ANSI bytes/frame + SGR cache hit rate.
    // These prove the diff-based + RLE + color cache optimizations work.
    // All byte/count values are rendered via the centralized `humanize_*`
    // helpers in `diagnostics/humanize.rs` so the unit (KiB / MiB / GiB / TiB
    // for bytes, K / M / B for counts) is chosen dynamically from the value.
    // No hardcoded `1024.0` divisor or `KiB/s` literal remains here. The raw
    // integer is preserved in parentheses for power users who need exact
    // precision when diagnosing cache behavior.
    {
        let s = r.section("ENCODING");
        let total_sgr = sgr_hits + sgr_misses;
        let hit_rate = if total_sgr > 0 {
            (sgr_hits as f64 / total_sgr as f64) * 100.0
        } else {
            0.0
        };
        let avg_bytes_per_frame = if enc_flushes > 0 {
            enc_bytes as f64 / enc_flushes as f64
        } else {
            0.0
        };

        s.field(
            "total_ansi_bytes",
            &format!("{} ({})", humanize_bytes(enc_bytes), enc_bytes),
        );
        s.field(
            "frames_flushed",
            &format!("{} ({})", humanize(enc_flushes), enc_flushes),
        );
        s.field(
            "avg_bytes_per_frame",
            &format!(
                "{} ({:.1} B)",
                humanize_bytes_f64(avg_bytes_per_frame),
                avg_bytes_per_frame
            ),
        );
        s.field("bandwidth", &humanize_throughput(enc_bytes, elapsed_s));
        s.field(
            "sgr_cache_hits",
            &format!("{} ({})", humanize(sgr_hits), sgr_hits),
        );
        s.field(
            "sgr_cache_misses",
            &format!("{} ({})", humanize(sgr_misses), sgr_misses),
        );
        s.field("sgr_cache_hit_rate", &format!("{:.1}%", hit_rate));
    }

    // Tier 2: xterm.js host defenses (byte-budget backpressure + RIS reset).
    // All three fields are 0 on native terminals; nonzero only inside
    // VSCode/Hyper/WaveTerminal/Tabby/WarpTerminal. Useful for diagnosing
    // whether the multi-hour OOM crash mode is actually being mitigated.
    // The byte counter uses the centralized binary formatter so a 2 MiB
    // backpressure budget reads as `2.00 MiB` rather than a 7-digit raw.
    {
        let s = r.section("TIER2_XTERMJS");
        s.field("backpressure_skips", &tier2_skips.to_string());
        s.field("ris_resets", &tier2_resets.to_string());
        s.field(
            "bytes_since_last_ris",
            &format!(
                "{} ({})",
                humanize_bytes(tier2_bytes_since),
                tier2_bytes_since
            ),
        );
    }

    r.eprint(); // stderr — survives alt-screen restore (AB-10)
}
