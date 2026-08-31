<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Output Mastery Audit — perf-stats, benchmark, verbose

**Date**: 2026-08-23 · **Owner concern**: suspected hardcoded values,
inconsistency, wrong calculations, and precision problems across the three
output surfaces · **Method**: every reported value traced to its computation
site; every cross-field arithmetic identity checked against the code that
produces it · **Reference pattern**: the FreeBSD peak_fps bugs
(`6b093f1` p1-trim, `6615301` physics floor) — computed values must be
defensible against outliers and platform clock behavior, and their
semantics must be visible in the output itself.

**TL;DR**: zero hardcoded measurement values found (every number traces to
a live computation); three real clarity defects fixed (missing sim column,
invisible period gap, undocumented raw fields); five percentage companions
added for beginner readability. All arithmetic identities now either hold
exactly or their residual is disclosed explicitly.

---

## 1. Findings and fixes

### F1 (MEDIUM, fixed): CELL EFFICIENCY columns did not add up

`render_ns_per_cell` (29.91) + `io_ns_per_cell` (2.28) visibly summed to
32.19 while `total_ns_per_cell` read 96.40 — sim was never shown, and
nothing in the section explained the missing ~64ns. A cross-checking user
concludes the math is wrong.

**Fix**: `sim_ns_per_cell` added (same denominator as the other columns);
shares normalized against the component sum (now sum to 100% of measured
work); `component_coverage_percent` added, disclosing how much of the
wall-clock total the component timers explain (typically ~98%; the rest is
loop bookkeeping outside the measured frame body — documented at the field).
Per-frame, sim+render+io is exact by construction (`io_ms` is defined as
the residual); the only approximation is the wall-clock vs frame-body
boundary, and it is now visible instead of implied.

### F2 (MEDIUM, fixed): BACKPRESSURE looked self-contradictory

Owner's log: `avg: 0.332`, `classification: high` next to
`budget_utilization_avg: 5.67%`. Both values are CORRECT but measure
different things: pressure derives from the FULL frame period
(work + sleep + event polling) vs the target period; utilization derives
from work time only. With target 144 FPS: period 9.28ms vs budget 6.94ms
-> pressure 0.34; work 0.489ms -> utilization 7%. The gap (scheduler
granularity + poll waits) was invisible in the output.

**Fix**: two new leading fields — `frame_period_target_ms` and
`frame_period_avg_ms` (elapsed/frames, labeled "includes sleep + event
polling") — make the gap directly visible; the `basis` and advice texts now
state which quantity each metric derives from and what
"pressure high + utilization low" means.

### F3 (LOW, fixed): undocumented raw fields in PERFORMANCE

`dirty_all_frames` and `dirty_threshold_cells` had no meaning lines.
Meaning fields added (full-redraw frames; the differential->full-redraw
crossover = grid_cells / 8).

### F4 (LOW, fixed): readability percentages

Owner request: percentages so a newcomer grasps the numbers at a glance.

- CELL EFFICIENCY: `dirty_cell_ratio_percent` — "2.97% (56 of 1.9K cells)".
- THROUGHPUT: `render_efficiency_percent` — "2.95% (dirty / theoretical)",
  the differential renderer's share of the theoretical ceiling.
- Both reuse already-computed values; zero new measurement code.

### F5 (INFO): perf-stats BACKPRESSURE basis text was subtly wrong

The old basis said pressure = `clamp(work_s/target_period - 1, ...)`. In
the runtime loop the pressure accumulator
(`PowerManager::observe_frame_end`) is fed the frame-period overshoot, not
the work overshoot — the text described the wrong input. Corrected to
"clamp(frame_period/target_period - 1, ...)".

## 2. Verified clean (no action needed)

- **No hardcoded measurements**: every reported value in the three outputs
  traces to a live computation. The only compile-time constants are build
  metadata (rustc version, git SHA, LTO/panic/strip/PGO flags via
  `env!()` — self-documenting build provenance, correct by design).
- **peak_fps family**: the FreeBSD defenses (p1 trim + 1µs physics floor)
  verified present in `src/bench/mod.rs` after the module move;
  `peak_fps_meaning` documents the semantics.
- **avg_frame_time <-> avg_fps consistency**: LTS fix (bench/mod.rs:655)
  derives both from the same wall-clock interval — verified intact.
- **humanize()**: bounds checked at every tier, including the 999,999 -> 1M
  roll edge; used only where precision is not the point (counts), never
  for timing or ratios.
- **ansi_bytes_per_second**: correctly labeled as an estimate with the
  19-byte/cell derivation surfaced in its basis field.
- **verbose output**: config dump is generated from live config state;
  no stale-count claims found in the current verbose sections (the docs
  audit earlier today cleaned the stale ones).
- **ENDURANCE / DRIFT / VISUAL OBJECTIVE**: all computed values trace to
  their samplers; classification thresholds are named constants with
  sanity tests.

## 3. Design rules going forward (low-maintenance contract)

1. Every computed section exposes a `_meaning`/`_basis` line when the
   number's derivation is not obvious from its name.
2. Cross-field identities either hold exactly or the residual gets its own
   field (`component_coverage_percent` is the template).
3. Percentage companions for count-pairs that require mental division
   (dirty/logical, dirty/theoretical, component shares).
4. No new measurement code for readability fields — percentages are
   derived from already-collected values, so they cannot drift from the
   numbers they annotate.

---

Copyright (C) 2026 rezky_nightky (oxyzenQ). All rights reserved.
cosmostrix and the cosmostrix logo are trademarks of rezky_nightky (oxyzenQ).
<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying on
  any specific number (test count, LOC, FPS, ms timeout), file path,
  function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
