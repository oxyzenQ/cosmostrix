// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-perf-2: the dedicated bench harness for the paths the Z-6
//! bench-mode contract skips (owner-approved 2026-09-24).
//!
//! `--benchmark --bench-cosmetics` routes here instead of the visible
//! premium loop: the measurement itself runs through
//! `run_premium_benchmark_silent` (the same silent loop `--bench-all`
//! uses), which drives the message overlay + the per-frame HUD block
//! when `cfg.bench_cosmetics` is set. What this module owns:
//!
//! - the user-facing entry (`run_cosmetics_benchmark`): one stderr
//!   banner naming what is being measured, then the silent measurement,
//!   then report emission (JSON/text + baseline save/compare — the
//!   exact emission block the premium benchmark uses, extracted here
//!   so both runners share one implementation instead of a copy).
//!
//! Why not the premium loop: the premium runner (premium.rs) is at its
//! LOC cap and its loop is the Z-6 critical-path loop; the harness
//! deliberately reuses the silent loop so `--bench-all` and this mode
//! can never drift apart on measurement mechanics (frame accounting,
//! warmup, alloc/energy/perf snapshot windows).
//!
//! What the harness measures (see CosmeticsReport for the field
//! semantics):
//! - message overlay: draw_message + border-cross detection render
//!   inside `cloud.rain_at`, so their cost lands in avg_render_ms and
//!   end-to-end in fps / dirty cells / alloc counters;
//! - HUD: refresh_colors + write_to_frame (pre-draw) plus
//!   push_frame_time / RSS-CPU sampling / update_metrics /
//!   set_dirty_cell_stats (post-draw metric tick), reported as
//!   hud_avg_ms / hud_max_ms.
//!
//! A/B protocol: run the same scene twice —
//! `cosmostrix --benchmark --json` (plain) vs
//! `cosmostrix --benchmark --bench-cosmetics --json` — the delta is
//! the total cosmetics-path cost for that scene at that size.

use crate::app::CloudConfig;
use crate::output::eprintln_safe;

/// Entry for `--benchmark --bench-cosmetics`.
///
/// The silent measurement loop already validates --bench-scene and
/// --bench-duration (fail-fast before any allocation), so this entry
/// only adds the banner and the emission. No progress bar and no
/// SIGINT registration by design: this is a research harness, not a
/// UX surface, and the silent loop must stay untouched for
/// `--bench-all` parity (its `was_interrupted` contract is
/// hardcoded false and is not this mode's concern).
pub(crate) fn run_cosmetics_benchmark(cfg: &CloudConfig) -> std::io::Result<()> {
    let duration_label = match cfg.bench_duration {
        Some(n) => n.to_string(),
        None => "default".to_string(),
    };
    let size_label = match cfg.screen_size {
        Some((cw, ch)) => format!("{cw}x{ch}"),
        None => "auto size".to_string(),
    };
    eprintln_safe!(
        "cosmetics harness: measuring the Z-6-skipped paths (message \
         overlay + HUD) for {duration_label}s at {size_label} ..."
    );
    let report_data = super::run_premium_benchmark_silent(cfg)?;
    emit_report_output(cfg, &report_data);
    Ok(())
}

/// Report emission shared by the premium benchmark and this harness:
/// JSON when requested, the text report otherwise, plus baseline
/// save/compare (path-whitelisted). Extracted VERBATIM from
/// premium.rs (BL-02 dedup shape preserved: json_opt computed once
/// above the branches); behavior-identical code motion.
pub(crate) fn emit_report_output(cfg: &CloudConfig, data: &crate::bench_report::BenchReportData) {
    // BL-02 (Dragon Hunt v3): dedup — hoist json_opt above the json/text
    // branch so save/compare baseline logic runs once (was duplicated
    // verbatim in both arms).
    let json_opt: Option<String> =
        if cfg.json || cfg.save_baseline.is_some() || cfg.compare_baseline.is_some() {
            Some(crate::bench_json::build_json_string(data))
        } else {
            None
        };

    if cfg.json {
        // Print JSON to stdout (only in --json mode).
        if let Some(ref json) = json_opt {
            crate::output::println_safe!("{json}");
        }
    } else {
        crate::bench_report::build_premium_report(data);
    }

    // Save baseline if requested (v17: path whitelist enforced).
    // For text mode, the JSON was generated above so users don't have to
    // pass --json just to save a baseline.
    if let (Some(path), Some(json)) = (cfg.save_baseline.as_ref(), json_opt.as_ref()) {
        if !crate::is_safe_path(path) {
            eprintln_safe!(
                "error: --save-baseline '{path}' is outside allowed directories\n  \
                 Allowed: ~/.config/cosmostrix/, /etc/cosmostrix/"
            );
        } else {
            match crate::bench_baseline::save_baseline(path, json) {
                Ok(()) => eprintln_safe!("[baseline] saved to {path}"),
                Err(e) => eprintln_safe!("{e}"),
            }
        }
    }

    // Compare baseline if requested (v17: path whitelist enforced).
    if let (Some(path), Some(json)) = (cfg.compare_baseline.as_ref(), json_opt.as_ref()) {
        if !crate::is_safe_path(path) {
            eprintln_safe!(
                "error: --compare-baseline '{path}' is outside allowed directories\n  \
                 Allowed: ~/.config/cosmostrix/, /etc/cosmostrix/"
            );
        } else if let Err(e) = crate::bench_baseline::compare_with_baseline(path, json) {
            eprintln_safe!("{e}");
        }
    }
}
