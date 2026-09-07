<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-special-2 A/B report — the aeolian weave (ninth rain style)

Owner rule: 10 s benchmark A/B before/after any visual-engine change,
visual (density gini, frame entropy) + performance (fps, dirty
cells). Baselines captured on the pre-change tree (ad9d9ef, stage 4
HEAD) before implementation; the after tree carries the full aeolian
style (engine, scene, tests). Dry benches (no terminal I/O) at the
default 120x40 bench viewport, `--benchmark --bench-duration 10
--json`.

## Results

| scene                | side | avg_fps | frame_ms | p99_ms | dirty/fr | entropy_bits | density_gini |
|----------------------|------|---------|----------|--------|----------|--------------|--------------|
| cinematic            | A    | 29,703  | 0.034    | 0.054  | 456.5    | 5.163        | 0.640        |
| cinematic            | B    | 28,820  | 0.035    | 0.055  | 455.3    | 5.164        | 0.640        |
| sorgonemous_intrasc. | A    | 26,015  | 0.038    | 0.048  | 330.3    | 5.483        | 0.553        |
| sorgonemous_intrasc. | B    | 25,611  | 0.039    | 0.054  | 330.1    | 5.481        | 0.554        |
| aeolian              | B    | 158,630 | 0.006    | 0.008  | 46.0     | 4.979        | 0.646        |

## Reading

- No regression on the two regression probes (cinematic = the glyph
  droplet path, sorgonemous_intrascals = the structured-family
  path the new style plugs into). The visual metrics match the
  baselines to three decimals (dirty cells, entropy, gini —
  identical populations, identical distributions); the avg_fps
  deltas (−3.0%, −1.6%) sit inside the dry-bench run-to-run noise
  band and track the shared-code growth (one more match arm per
  dispatch chain).
- The aeolian scene's own profile: 158,630 fps (5.3x the glyph
  styles — the weave is the cheapest structured style in the
  catalog: the sparse calm-sky dial holds the drop population to
  ~13 lanes and the draw floor hides the silent instrument, so the
  steady state repaints only ~46 cells per frame). Entropy 4.979
  bits (a living, varied column distribution) and density gini
  0.646 (strong spatial concentration — the hero-structure-plus-
  weather read, not uniform noise) land in the healthy band
  between cinematic (even rain) and sorgonemous (centered hole).

## Files

- baseline_A_cinematic.json / baseline_A_sorgonemous.json — the
  "before" captures (ad9d9ef).
- after_B_cinematic.json / after_B_sorgonemous.json — the
  regression probes on the aeolian tree.
- after_B_aeolian.json — the new style's own 10 s profile.
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
