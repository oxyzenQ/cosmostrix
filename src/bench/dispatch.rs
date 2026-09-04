// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Post-config bench dispatch.
//!
//! Extracted from `main.rs` to keep that file under the 800-LOC cap.
//! Pure code motion — no behavior change.
//!
//! Routes to the appropriate runner based on bench flags:
//! - `--bench-all` → scaling benchmark (JSON output if --json)
//! - `--benchmark` → premium benchmark (report-engine output)
//! - `--bench-frames N` → legacy CI benchmark (parseable BENCH: format)
//! - (none) → interactive mode (the main rain loop)
//!
//! Returns `Some(Ok(()))` when a bench dispatch fires (caller returns
//! immediately). Returns `None` for the interactive path (caller invokes
//! `run_interactive` separately so it can route errors through the
//! post-exit verbose dump).

use crate::app::CloudConfig;
use crate::bench;
use crate::config::Args;
use crate::output::println_safe;

/// Check post-config bench dispatchers.
///
/// Returns `Some(Ok(()))` when a bench dispatch fires (caller should
/// return the result immediately). Returns `None` when no bench flag
/// matched — caller should proceed to interactive mode.
pub(crate) fn dispatch_bench(
    args: &Args,
    cloud_cfg: &CloudConfig,
    fps_user_set: bool,
) -> Option<std::io::Result<()>> {
    if args.bench_all {
        crate::bench_helpers::warn_bench_noop_flags(args, fps_user_set);
        let duration =
            crate::bench_helpers::resolve_bench_duration_args(&args.bench_duration).unwrap_or(2);
        match crate::bench_scale::run_scaling_benchmark(cloud_cfg, duration) {
            Ok(results) => {
                if args.json {
                    println_safe!(
                        "{}",
                        crate::bench_scale::build_scaling_json(&results, &cloud_cfg.scene_name)
                    );
                }
                return Some(Ok(()));
            }
            Err(e) => return Some(Err(e)),
        }
    }

    if args.benchmark {
        crate::bench_helpers::warn_bench_noop_flags(args, fps_user_set);
        return Some(bench::run_premium_benchmark(cloud_cfg));
    }

    if let Some(_bench_frames) = args.bench_frames {
        crate::bench_helpers::warn_bench_noop_flags(args, fps_user_set);
        return Some(bench::run_benchmark(cloud_cfg));
    }

    None
}
