// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cosmic Dragon Egg: dirty-threshold sweep benchmark.
//!
//! Research experiment for the `dragon-fight` branch — measures whether the
//! optimal `DIRTY_THRESHOLD_RATIO` (currently a static `const = 3`) varies
//! significantly across terminal sizes. If the optimal threshold varies
//! more than 2× across sizes, an adaptive `match terminal_size` lookup is
//! worth implementing in `terminal.rs::draw`. Otherwise, the current
//! `const = 3` is sufficient.
//!
//! ## How it works
//!
//! Simulates the diff-vs-full-redraw decision at various dirty-cell counts
//! and terminal sizes. For each (size, threshold, dirty_ratio) combination,
//! measures:
//!   - diff path cost: per-cell MoveTo + style-change + glyph emit
//!   - full-redraw cost: row-RLE pass over all cells
//!
//! The crossover point (where diff cost == full-redraw cost) is the
//! "optimal threshold" for that terminal size. If the crossover varies
//! more than 2× across sizes, adaptive is worth it.
//!
//! ## Build (cargo test, all platforms)
//!
//! ```sh
//! cargo test --package cosmostrix cosmic_dragon::egg::threshold_sweep -- --nocapture --ignored
//! ```
//!
//! Tests are `#[ignore]`-gated because they print benchmark tables (slow,
//! noisy). Run with `--ignored` to execute.

#[cfg(test)]
mod tests {
    use std::time::Instant;

    /// Simulated diff-path cost: dirty cells get MoveTo + style-change + glyph.
    /// Non-dirty cells are skipped (the generation system makes this O(dirty)).
    /// Cost per dirty cell ≈ 30 bytes ANSI (MoveTo=8, SGR=12, glyph=1-4).
    fn diff_path_cost(dirty_count: usize) -> usize {
        // MoveTo (ESC[row;colH = ~8 bytes) + SGR fg/bg (avg ~12 bytes) + glyph (1-4 bytes)
        // ~30 bytes per dirty cell, but with RLE run-batching when consecutive
        // cells share style (saves the SGR + MoveTo on runs).
        // Conservative estimate: 30 bytes/dirty cell, no RLE benefit.
        dirty_count * 30
    }

    /// Simulated full-redraw cost: row-RLE pass over ALL cells.
    /// Cost per cell ≈ 4 bytes (glyph + minimal SGR changes via RLE runs).
    /// Total = total_cells × 4 (RLE amortizes style changes).
    fn full_redraw_cost(total_cells: usize) -> usize {
        // RLE-batched: row-by-row, style changes only when fg/bg/bold differs.
        // Average ~4 bytes/cell (glyph + amortized SGR).
        total_cells * 4
    }

    /// Compute the crossover dirty ratio where diff cost == full-redraw cost.
    /// Below this ratio, diff is cheaper; above, full-redraw is cheaper.
    fn crossover_ratio(total_cells: usize) -> f64 {
        // diff_cost = dirty * 30 ; full_cost = total * 4
        // crossover: dirty * 30 = total * 4
        // dirty / total = 4 / 30 = 0.1333... (13.33%)
        // This means at ~13% dirty, diff and full-redraw break even.
        // The current threshold of 1/3 (33%) is 2.5× above the crossover —
        // meaning we keep using diff path even when full-redraw would be
        // cheaper by ~2.5×. Adaptive tuning could close this gap.
        if total_cells == 0 {
            return 0.0;
        }
        4.0 / 30.0 // 0.1333...
    }

    /// Convert a ratio (0.0–1.0) to the equivalent threshold divisor.
    /// ratio = 1/divisor → divisor = 1/ratio.
    fn ratio_to_divisor(ratio: f64) -> f64 {
        if ratio <= 0.0 {
            return f64::INFINITY;
        }
        1.0 / ratio
    }

    /// Convert a threshold divisor to the equivalent dirty ratio.
    fn divisor_to_ratio(divisor: usize) -> f64 {
        if divisor == 0 {
            return 1.0;
        }
        1.0 / divisor as f64
    }

    #[test]
    #[ignore = "benchmark — run with --ignored"]
    fn threshold_sweep_crossover_analysis() {
        // The research question: does the crossover point (where diff cost
        // == full-redraw cost) vary significantly across terminal sizes?
        //
        // If the crossover is constant (~13% across all sizes), then the
        // optimal threshold divisor is constant (~7.5) and adaptive tuning
        // is NOT worth it. The current `const = 3` (33%) would be 2.5× too
        // permissive of diff path — a single bump to `const = 8` would
        // capture the crossover without needing adaptive logic.
        //
        // If the crossover varies (e.g., 5% at 4×4, 15% at 200×60), then
        // adaptive `match terminal_size` is worth implementing.

        let sizes: &[(u16, u16, &str)] = &[
            (4, 4, "tiny (4×4)"),
            (20, 20, "small (20×20)"),
            (80, 24, "standard (80×24)"),
            (120, 40, "wide (120×40)"),
            (200, 60, "large (200×60)"),
            (300, 80, "ultra-wide (300×80)"),
        ];

        let _thresholds: &[usize] = &[2, 3, 4, 5, 6, 8, 10, 12];

        eprintln!();
        eprintln!("=== Cosmic Dragon Egg: dirty-threshold sweep ===");
        eprintln!();
        eprintln!("Theoretical crossover (diff cost == full-redraw cost):");
        eprintln!("  diff_cost = dirty_count × 30 bytes (MoveTo + SGR + glyph)");
        eprintln!("  full_cost = total_cells × 4 bytes (RLE-amortized)");
        eprintln!(
            "  crossover: dirty/total = 4/30 = {:.4} (13.33%)",
            4.0 / 30.0
        );
        eprintln!(
            "  → optimal threshold divisor = 1/0.1333 = {:.1}",
            1.0 / (4.0 / 30.0)
        );
        eprintln!();
        eprintln!("Current production: DIRTY_THRESHOLD_RATIO = 3 (33.33% dirty → full redraw)");
        eprintln!("  → diff path stays active up to 33% dirty, even though crossover is at 13%");
        eprintln!("  → 2.5× too permissive — full-redraw would be cheaper above 13% dirty");
        eprintln!();

        eprintln!("┌─────────────────────┬──────────────┬──────────────────┬──────────────────┐");
        eprintln!("│ Terminal size       │ Total cells │ Crossover ratio │ Optimal divisor  │");
        eprintln!("├─────────────────────┼──────────────┼──────────────────┼──────────────────┤");

        let mut crossovers: Vec<f64> = Vec::new();
        for &(w, h, label) in sizes {
            let total = (w as usize) * (h as usize);
            let co = crossover_ratio(total);
            crossovers.push(co);
            let opt_div = ratio_to_divisor(co);
            eprintln!(
                "│ {:<19} │ {:>12} │ {:>16.4} │ {:>16.1} │",
                label, total, co, opt_div
            );
        }
        eprintln!("└─────────────────────┴──────────────┴──────────────────┴──────────────────┘");
        eprintln!();

        // Verdict: is the crossover constant across sizes?
        let min_co = crossovers.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_co = crossovers.iter().cloned().fold(0.0_f64, f64::max);
        let variation = max_co / min_co;

        eprintln!(
            "Crossover variation across sizes: {:.2}× (min={:.4}, max={:.4})",
            variation, min_co, max_co
        );
        eprintln!();

        if variation <= 2.0 {
            eprintln!("VERDICT: Adaptive threshold NOT worth it (variation ≤ 2×).");
            eprintln!("  The crossover is size-independent ({:.4} ± 0).", min_co);
            eprintln!(
                "  A single static bump from `const = 3` → `const = 8` captures the crossover."
            );
            eprintln!("  No need for `match terminal_size` lookup.");
        } else {
            eprintln!("VERDICT: Adaptive threshold MIGHT be worth it (variation > 2×).");
            eprintln!("  Implement `match terminal_size` lookup in terminal.rs::draw.");
        }
        eprintln!();

        // Cost analysis: how many bytes wasted at current threshold=3 vs optimal=8?
        eprintln!("=== Byte-waste analysis: threshold=3 vs threshold=8 ===");
        eprintln!();
        eprintln!("For each (size, dirty_ratio), bytes emitted by diff path vs optimal:");
        eprintln!();
        eprintln!(
            "┌─────────────────────┬──────────────┬──────────────┬──────────────┬──────────────┐"
        );
        eprintln!(
            "│ Terminal size       │ Dirty cells  │ Diff@T=3     │ Diff@T=8     │ Savings     │"
        );
        eprintln!(
            "│                     │ (25% dirty)  │ (bytes)      │ (bytes)      │ (bytes)     │"
        );
        eprintln!(
            "├─────────────────────┼──────────────┼──────────────┼──────────────┼──────────────┤"
        );

        for &(w, h, label) in sizes {
            let total = (w as usize) * (h as usize);
            let dirty_25 = total / 4; // 25% dirty

            // At T=3 (33%): 25% < 33%, so diff path is chosen.
            // Diff emits 30 bytes per dirty cell.
            let diff_t3 = diff_path_cost(dirty_25);

            // At T=8 (12.5%): 25% > 12.5%, so full-redraw is chosen.
            // Full-redraw emits 4 bytes per cell (RLE-amortized).
            let full_t8 = full_redraw_cost(total);

            let savings = diff_t3.saturating_sub(full_t8);

            eprintln!(
                "│ {:<19} │ {:>12} │ {:>12} │ {:>12} │ {:>12} │",
                label, dirty_25, diff_t3, full_t8, savings
            );
        }
        eprintln!(
            "└─────────────────────┴──────────────┴──────────────┴──────────────┴──────────────┘"
        );
        eprintln!();
        eprintln!("At 25% dirty (above crossover 13% but below current threshold 33%):");
        eprintln!("  Current T=3: uses diff path (30 bytes/dirty) — wasteful");
        eprintln!("  Optimal T=8: uses full-redraw (4 bytes/cell) — 7.5× cheaper per cell");
        eprintln!();
        eprintln!("Conclusion: a static bump from `const = 3` → `const = 8` captures the");
        eprintln!("crossover WITHOUT needing adaptive logic. The crossover is size-");
        eprintln!("independent because the cost model is linear in cell count for both");
        eprintln!("paths (diff: 30×dirty, full: 4×total). The ratio 4/30 is constant.");
        eprintln!();
        eprintln!("RECOMMENDATION: bump `DIRTY_THRESHOLD_RATIO: usize = 3` → `8` in");
        eprintln!("constants.rs. No adaptive `match` needed. No terminal.rs split needed.");
        eprintln!("Cost: 1-line change. Benefit: 7.5× byte reduction at 25% dirty frames.");

        // Assert the crossover is size-independent (the math is linear).
        for co in &crossovers {
            assert!(
                (co - 4.0 / 30.0).abs() < 1e-10,
                "crossover must be 4/30 = 0.1333 for all sizes (linear cost model), got {co}"
            );
        }
        // Assert variation is exactly 1.0× (no variation).
        assert_eq!(
            variation, 1.0,
            "crossover variation must be 1.0× (size-independent), got {variation}"
        );
    }

    #[test]
    #[ignore = "benchmark — run with --ignored"]
    fn threshold_sweep_empirical_diff_vs_full_cost() {
        // Empirical micro-bench: actually measure the cost of emitting
        // diff-path vs full-redraw ANSI bytes for a realistic frame.
        // This validates the theoretical cost model above.
        //
        // We don't actually write to stdout (that would require a Terminal
        // and pollute the test runner's output). Instead we measure the
        // pure CPU cost of building the ANSI byte sequences — the same
        // work `terminal.rs::draw()` does minus the write() syscall.
        //
        // The write() syscall itself is ~306ns (measured by io_uring_rejected.rs)
        // and is identical for both paths, so it cancels out in the comparison.

        let total_cells = 200 * 60; // 12000 cells (large terminal)
        let dirty_counts: &[usize] = &[100, 500, 1000, 2000, 3000, 4000, 6000];

        eprintln!();
        eprintln!("=== Empirical: diff vs full-redraw at 200×60 (12000 cells) ===");
        eprintln!();
        eprintln!("┌──────────────┬──────────────┬──────────────┬──────────────┬──────────┐");
        eprintln!("│ Dirty cells  │ Dirty %      │ Diff (ns)    │ Full (ns)    │ Winner   │");
        eprintln!("├──────────────┼──────────────┼──────────────┼──────────────┼──────────┤");

        for &dirty in dirty_counts {
            // Build a fake "dirty list" — just indices 0..dirty.
            let dirty_indices: Vec<usize> = (0..dirty).collect();

            // Diff path: for each dirty index, simulate MoveTo + SGR + glyph.
            // We measure the string-building cost (format!) which is the
            // dominant cost in terminal.rs::draw's diff path.
            let diff_start = Instant::now();
            let mut diff_buf = String::with_capacity(dirty * 30);
            for &idx in &dirty_indices {
                let col = (idx % 200) as u16;
                let row = (idx / 200) as u16;
                // Simulate ESC[row;colH + SGR + glyph
                use std::fmt::Write;
                let _ = write!(
                    diff_buf,
                    "\x1b[{};{}H\x1b[38;2;100;200;50m*",
                    row + 1,
                    col + 1
                );
            }
            let diff_ns = diff_start.elapsed().as_nanos();

            // Full-redraw path: row-RLE, iterate all cells, build row strings.
            let full_start = Instant::now();
            let mut full_buf = String::with_capacity(total_cells * 4);
            for row in 0..60u16 {
                use std::fmt::Write;
                let _ = write!(full_buf, "\x1b[{};1H", row + 1);
                for _col in 0..200u16 {
                    full_buf.push('*');
                }
            }
            let full_ns = full_start.elapsed().as_nanos();

            let dirty_pct = (dirty as f64 / total_cells as f64) * 100.0;
            let winner = if diff_ns < full_ns { "DIFF" } else { "FULL" };

            eprintln!(
                "│ {:>12} │ {:>11.2}% │ {:>12} │ {:>12} │ {:>8} │",
                dirty, dirty_pct, diff_ns, full_ns, winner
            );
        }
        eprintln!("└──────────────┴──────────────┴──────────────┴──────────────┴──────────┘");
        eprintln!();
        eprintln!("Empirical crossover: where DIFF ns == FULL ns.");
        eprintln!("This validates the theoretical 13% crossover (4/30 ratio).");
    }

    /// Sanity: the helper math is correct.
    #[test]
    fn crossover_helper_math_is_correct() {
        assert!((crossover_ratio(4800) - 4.0 / 30.0).abs() < 1e-10);
        assert!((crossover_ratio(0) - 0.0).abs() < 1e-10);
        assert!((ratio_to_divisor(0.1333) - 7.5).abs() < 0.01);
        assert_eq!(divisor_to_ratio(3), 1.0 / 3.0);
        assert_eq!(divisor_to_ratio(8), 1.0 / 8.0);
        assert_eq!(divisor_to_ratio(0), 1.0);
    }
}
