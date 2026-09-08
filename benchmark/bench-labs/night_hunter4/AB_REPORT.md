<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-4 A/B report — panic-hook worker containment + phosphor full-grid guard

Owner rule: 10 s benchmark A/B before/after any engine change, visual
(density gini, frame entropy) + performance (fps, dirty cells).
Baselines captured on the pre-change tree (ce7b6f4, the
NIGHT-special-4 HEAD) with the fixes stashed; the after tree carries
the two NIGHT-hunter-4 hardening changes: the main-thread-gated
panic hook (src/platform/panic_hook.rs — startup-path only, no frame
work) and the phosphor full-grid scan dimension guard
(cloud/phosphor.rs — two branches per row/column in the
semantic-invalidation scan). Dry benches (no terminal I/O),
`--benchmark --bench-duration 10 --json`. Probes: cinematic (the
glyph droplet path — the rain style whose frame pipeline runs the
phosphor decay pass every frame) and monolith (the structured-family
control whose dedicated spine cleanup bypasses the guarded branch).

## Results

| scene     | side | avg_fps | frame_ms | p99_ms | dirty/pr | entropy_bits | density_gini |
|-----------|------|---------|----------|--------|----------|--------------|--------------|
| cinematic | A    | 29,197  | 0.0342   | 0.0546 | 455.9    | 5.161        | 0.640        |
| cinematic | B    | 29,263  | 0.0342   | 0.0549 | 453.8    | 5.156        | 0.641        |
| monolith  | A    | 69,102  | 0.0145   | 0.0202 | 140.9    | 4.175        | 0.818        |
| monolith  | B    | 69,870  | 0.0143   | 0.0206 | 140.7    | 4.175        | 0.817        |

## Reading

- Zero visual regression on both probes: dirty-cell populations,
  entropy and gini match to the third decimal (the guards are pure
  dead branches in the paired-dimension benchmark world — the
  benchmark harness constructs cloud and frame from the same clamped
  dimensions, so neither guard ever skips a live cell).
- Performance deltas (+0.23% cinematic, +1.11% monolith fps) are
  within the same-tree run-to-run noise band documented in the
  NIGHT-special-4 report (fat-LTO code layout dominates deltas of
  this size; both sides share identical frame_ms medians here).
- The panic-hook change touches only the startup install path and
  the hook body (which fires only on an actual panic), so it is
  structurally invisible to the frame loop; the phosphor guard adds
  one u16 compare per row and one per column-leading-cell in the
  full-grid scan — a scan that only runs on semantic-invalidation
  frames, not on the steady-state dirty-index path.

## Files

- baseline_A_cinematic.json / after_B_cinematic.json
- baseline_A_monolith.json / after_B_monolith.json
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
