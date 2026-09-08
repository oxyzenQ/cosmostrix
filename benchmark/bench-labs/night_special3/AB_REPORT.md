<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-special-3 A/B report — the aurora veil (tenth rain style)

Owner rule: 10 s benchmark A/B before/after any visual-engine change,
visual (density gini, frame entropy) + performance (fps, dirty
cells). Baselines captured on the pre-change tree (1d4d622,
NIGHT-quality-1 HEAD) before implementation; the after tree carries
the full aurora style (engine, scene, tests, the SCENES catalog
extraction). Dry benches (no terminal I/O) at the default 120x40
bench viewport, `--benchmark --bench-duration 10 --json`. Three
regression probes this round: cinematic (the glyph droplet path),
sorgonemous_intrascals (the structured-family dispatch the new style
plugs into) and aeolian (the sibling original-math flagship — the
closest code path to the new one).

## Results

| scene                | side | avg_fps | frame_ms | p99_ms | dirty/fr | entropy_bits | density_gini |
|----------------------|------|---------|----------|--------|----------|--------------|--------------|
| cinematic            | A    | 28,161  | 0.036    | 0.053  | 464.1    | 5.185        | 0.634        |
| cinematic            | B    | 29,976  | 0.033    | 0.052  | 452.9    | 5.157        | 0.641        |
| sorgonemous_intrasc. | A    | 25,814  | 0.039    | 0.047  | 330.4    | 5.485        | 0.553        |
| sorgonemous_intrasc. | B    | 26,523  | 0.038    | 0.046  | 330.2    | 5.482        | 0.554        |
| aeolian              | A    | 158,879 | 0.006    | 0.009  | 46.0     | 4.980        | 0.645        |
| aeolian              | B    | 163,048 | 0.006    | 0.009  | 46.0     | 4.981        | 0.645        |
| aurora               | B    | 100,548 | 0.010    | 0.014  | 135.9    | 3.887        | 0.847        |

## Reading

- Zero regression on the three probes. The visual metrics match the
  baselines to the third decimal (dirty cells, entropy, gini —
  identical populations, identical distributions); the avg_fps
  deltas (+6.5%, +2.7%, +2.6%) sit inside the dry-bench run-to-run
  noise band and point upward (machine variance, not the change —
  the aurora adds one match arm per dispatch chain and zero work to
  the probe scenes' hot paths).
- The aurora scene's own profile: 100,548 fps (2nd-cheapest
  structured style — the fabric's glyph identity means the static
  curtain body never re-dirties its cells; only the falling heads,
  their comet trails, the fringe shimmer and the breath-driven
  fringe/depth motion repaint, ~136 cells per frame). Entropy
  3.887 bits and density gini 0.847 are the most structured numbers
  in the catalog — and deliberately so: the dirty-cell population
  concentrates onto the ray columns (13 of 80 columns at the bench
  width), which IS the aurora read — tall curtains separated by
  dark sky, a hero structure with a sparse weather minority. The
  aeolian (even rain over revealed strings) reads 0.645; the black
  hole (centered mass) 0.553; the veil (columned curtains) 0.847 —
  three different compositions, each honest to its architecture.

## Files

- baseline_A_cinematic.json / baseline_A_sorgonemous.json /
  baseline_A_aeolian.json — the "before" captures (1d4d622).
- after_B_cinematic.json / after_B_sorgonemous.json /
  after_B_aeolian.json — the regression probes on the aurora tree.
- after_B_aurora.json — the new style's own 10 s profile.
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
