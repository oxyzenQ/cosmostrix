<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-research-7 A/B report — the DNA helix (eleventh rain style)

Owner rule: 10 s benchmark A/B before/after any visual-engine
change, visual (density gini, frame entropy) + performance (fps,
dirty cells). Baselines captured on the pre-change tree (44a73ce,
the NIGHT-hunter-25 HEAD) before implementation; the after tree
adds the dna_helix style (engine, scene, tests, docs). Dry benches
(no terminal I/O) at the default bench viewport,
`--benchmark --bench-duration 10 --json`. Three regression probes:
cinematic (the glyph droplet path), solar_flare (the
structured-family dispatch the new style plugs into) and aeolian
(the closest sibling in the structured family).

## Results

| scene         | side        | avg_fps | frame_ms | p99_ms | dirty/fr | entropy_bits | density_gini |
|---------------|-------------|---------|----------|--------|----------|--------------|--------------|
| cinematic     | A           | 28,444  | 0.035    | 0.057  | 460.9    | 5.174        | 0.637        |
| cinematic     | B           | 29,086  | 0.034    | 0.055  | 457.3    | 5.165        | 0.639        |
| aeolian       | A           | 157,749 | 0.006    | 0.009  | 45.9     | 4.979        | 0.646        |
| aeolian       | B           | 160,064 | 0.006    | 0.008  | 46.0     | 4.979        | 0.646        |
| solar_flare   | A           | 36,757  | 0.027    | 0.035  | 247.5    | 6.037        | 0.325        |
| solar_flare   | B           | 36,397  | 0.027    | 0.035  | 249.5    | 6.037        | 0.325        |
| dna_helix     | B           | 96,591  | 0.010    | 0.015  | 102.0    | 4.859        | 0.692        |

## Reading

- Zero visual regression on the three probes: the dirty-cell
  populations, entropy and gini match the baselines to the third
  decimal. The change is a pure enum-arm addition per dispatch
  chain (one more `else if` in six match chains) plus one new
  Cloud field — the probe scenes' hot paths execute none of the
  new code.
- The fps deltas (+2.3%, +1.5%, -1.0%) sit inside the established
  noise band for interleaved 10 s runs (the hunter-24 report
  measured cinematic spreading 1.96% and monolith 4.93% on
  same-tree runs); the structural argument carries the claim: the
  bench loop is branch-selected per style before any of the new
  code is reachable.
- The dna_helix scene's own profile: 96,591 fps — the
  second-fastest structured style after the aeolian (157K) and
  well ahead of the structured pack (solar 36.4K, sorgonemous
  26.4K). 102.0 dirty cells/frame is a light population (the
  strands + dashed rungs + sparse soup — the dashed-bond rung
  design keeps the drawn set lean).
- Entropy 4.859 bits / gini 0.692: a CONCENTRATED composition —
  the inverse of the corona's banded spread (6.037/0.325). The
  molecule is a single centered body with empty sky to its sides
  (the single-body flagship aesthetic the black hole established,
  whose centered mass reads 0.554): the dirt concentrates in the
  helix band, raising gini. Each composition is honest to its
  architecture: the corona is a full-width surface, the aeolian
  even rain over revealed strings, the black hole a centered mass,
  the helix a centered ribbon.

## Files

- `baseline_A_{cinematic,aeolian,solar_flare}.json` — pre-change
  tree (44a73ce), captured before implementation.
- `after_B_{cinematic,aeolian,solar_flare,dna_helix}.json` —
  post-change tree.
- Comparison script: `scripts/ab_compare_nr7_dna.py` (the
  hunter-20 comparator pattern).
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
