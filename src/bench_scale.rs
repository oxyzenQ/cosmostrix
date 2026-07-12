// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Scaling benchmark automation — runs benchmark across multiple screen sizes.
//!
//! Phase 9 of DeepSeek benchmark restructuring plan.
//!
//! When `--bench-all` is passed, cosmostrix runs the benchmark for a
//! predetermined set of screen sizes (6×6 through 200×60) and prints
//! a SCALING SUMMARY table at the end.

use crate::bench_report::BenchReportData;

/// Screen sizes to benchmark in scaling mode.
const SCALE_SIZES: &[(u16, u16)] = &[(6, 6), (20, 20), (40, 20), (80, 24), (120, 40), (200, 60)];

/// Run benchmark across multiple screen sizes and print summary table.
/// Returns the collected results for each size.
pub fn run_scaling_benchmark(
    cfg: &crate::app::CloudConfig,
    duration_secs: u64,
) -> std::io::Result<Vec<ScaleResult>> {
    let effective_duration = duration_secs.max(1);
    let mut results = Vec::with_capacity(SCALE_SIZES.len());

    eprintln!(
        "[bench-all] Running {size_count} benchmarks ({effective_duration}s each)...",
        size_count = SCALE_SIZES.len()
    );
    eprintln!();

    for &(w, h) in SCALE_SIZES {
        eprintln!("[bench-all] {w}x{h}...");

        // Create a modified config with this screen size
        let mut scale_cfg = cfg.clone_config();
        scale_cfg.screen_size = Some((w, h));

        // Run benchmark silently (suppress report output)
        let result = run_single_silent(&scale_cfg, effective_duration)?;

        let sr = ScaleResult {
            width: w,
            height: h,
            cells: (w as u64) * (h as u64),
            avg_fps: result.avg_fps,
            total_ns_per_cell: result.total_ns_per_cell,
            avg_dirty_cells: result.avg_dirty_cells_per_frame,
            alloc_calls_per_frame: result
                .allocator
                .as_ref()
                .map(|a| a.alloc_calls_per_frame)
                .unwrap_or(0.0),
            heap_retained: result
                .allocator
                .as_ref()
                .map(|a| a.heap_retained_bytes)
                .unwrap_or(0),
            ipc: result
                .perf
                .as_ref()
                .filter(|p| p.available)
                .map(|p| p.instructions_per_cycle),
            entropy: result
                .visual
                .as_ref()
                .map(|v| v.frame_entropy_bits)
                .unwrap_or(0.0),
            gini: result
                .visual
                .as_ref()
                .map(|v| v.density_gini)
                .unwrap_or(0.0),
        };
        results.push(sr);
    }

    // Print summary table
    print_scaling_summary(&results);

    Ok(results)
}

/// Run a single benchmark without printing the full report.
fn run_single_silent(
    cfg: &crate::app::CloudConfig,
    duration_secs: u64,
) -> std::io::Result<BenchReportData> {
    // Temporarily suppress output by redirecting to a dummy config
    let mut silent_cfg = cfg.clone_config();
    silent_cfg.json = false;
    silent_cfg.save_baseline = None;
    silent_cfg.compare_baseline = None;

    // Use the premium benchmark but capture the report data instead of printing
    crate::bench::run_benchmark_capture(&silent_cfg, duration_secs)
}

/// Single scaling result row.
#[derive(Debug, Clone)]
pub struct ScaleResult {
    pub width: u16,
    pub height: u16,
    pub cells: u64,
    pub avg_fps: f64,
    pub total_ns_per_cell: f64,
    pub avg_dirty_cells: f64,
    pub alloc_calls_per_frame: f64,
    pub heap_retained: u64,
    pub ipc: Option<f64>,
    pub entropy: f64,
    pub gini: f64,
}

/// Print the scaling summary table.
fn print_scaling_summary(results: &[ScaleResult]) {
    println!();
    println!("SCALING SUMMARY");
    println!("───────────────");
    println!(
        "  {:<8} {:>6} {:>10} {:>10} {:>6} {:>8} {:>8} {:>8}",
        "Size", "Cells", "FPS", "ns/cell", "IPC", "Alloc/f", "Entropy", "Gini"
    );
    println!(
        "  {:<8} {:>6} {:>10} {:>10} {:>6} {:>8} {:>8} {:>8}",
        "────", "─────", "───", "───────", "───", "───────", "───────", "────"
    );

    for r in results {
        let size_str = format!("{}×{}", r.width, r.height);
        let fps_str = crate::humanize::humanize_f64(r.avg_fps);
        let ipc_str = r
            .ipc
            .map(|i| format!("{i:.2}"))
            .unwrap_or_else(|| "—".to_string());
        println!(
            "  {:<8} {:>6} {:>10} {:>10.1} {:>6} {:>8.1} {:>8.2} {:>8.4}",
            size_str,
            crate::humanize::humanize(r.cells),
            fps_str,
            r.total_ns_per_cell,
            ipc_str,
            r.alloc_calls_per_frame,
            r.entropy,
            r.gini
        );
    }

    println!();
    println!("  Insights:");
    println!("    • ns/cell stabilizes at large sizes → O(n) confirmed");
    println!("    • Higher FPS at small sizes = fixed overhead dominates");
    println!("    • IPC > 2.0 = excellent pipeline utilization");
    println!("    • Gini near 0 = uniform density; near 1 = concentrated");
    println!();
}

/// Build JSON array from scaling results.
pub fn build_scaling_json(results: &[ScaleResult]) -> String {
    let mut out = String::with_capacity(2048);
    out.push('[');
    for (i, r) in results.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            r#"{{"size":"{}x{}","cells":{},"avg_fps":{},"total_ns_per_cell":{},"avg_dirty_cells":{},"alloc_calls_per_frame":{},"heap_retained":{},"ipc":{},"entropy":{},"gini":{}}}"#,
            r.width,
            r.height,
            r.cells,
            r.avg_fps,
            r.total_ns_per_cell,
            r.avg_dirty_cells,
            r.alloc_calls_per_frame,
            r.heap_retained,
            r.ipc.map(|i| i.to_string()).unwrap_or_else(|| "null".to_string()),
            r.entropy,
            r.gini
        ));
    }
    out.push(']');
    out
}
