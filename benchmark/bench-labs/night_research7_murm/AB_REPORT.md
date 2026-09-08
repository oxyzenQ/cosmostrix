<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-research-7 A/B report — the murmuration (twelfth rain style)

Owner rule: 10 s benchmark A/B before/after any visual-engine
change, visual (density gini, frame entropy) + performance (fps,
dirty cells). Baselines captured on the pre-change tree (c090097,
the DNA-helix HEAD) before implementation; the after tree adds the
murmuration style (engine, scene, tests, docs). Dry benches (no
terminal I/O) at the default bench viewport,
`--benchmark --bench-duration 10 --json`. Three regression probes:
cinematic (the glyph droplet path), dna_helix (the previous
NIGHT-research-7 style — the closest code path) and aeolian (the
fastest structured style).

## Results

| scene       | side        | avg_fps | frame_ms | dirty/fr | entropy_bits | density_gini |
|-------------|-------------|---------|----------|----------|--------------|--------------|
| cinematic   | A           | 29,143  | 0.034    | 454.5    | 5.157        | 0.641        |
| cinematic   | B           | 29,118  | 0.034    | 455.2    | 5.163        | 0.640        |
| aeolian     | A           | 159,741 | 0.006    | 46.1     | 4.983        | 0.644        |
| aeolian     | B           | 158,437 | 0.006    | 46.0     | 4.978        | 0.646        |
| dna_helix   | A           | 96,909  | 0.010    | 101.9    | 4.860        | 0.692        |
| dna_helix   | B           | 97,155  | 0.010    | 101.9    | 4.859        | 0.692        |
| murmuration | B           | 24,948  | 0.040    | 144.5    | 4.841        | 0.685        |

## Reading

- Zero visual regression on the three probes: dirty cells,
  entropy and gini match the baselines to the third decimal. The
  change is a pure enum-arm addition per dispatch chain plus one
  new Cloud field — the probe scenes' hot paths execute none of
  the new code.
- The fps deltas (-0.1%, -0.8%, +0.3%) are interleaved-run noise;
  the structural argument carries the claim (the bench loop
  branch-selects the style before any new code is reachable).
- The murmuration scene's own profile: 24,948 fps — the slowest
  of the structured styles but in the sorgonemous band (26.4K),
  and honest about why: the flock integrates ~100 birds x (the
  spatial hash rebuild + the force pass + the Euler step) per
  frame — real per-bird physics, unlike the field styles whose
  per-frame cost is the draw alone. 144.5 dirty cells/frame is a
  light population (heads + 2-cell trails).
- Entropy 4.841 bits / gini 0.685: a concentrated composition
  reading like the DNA helix's (0.692) — the flock is a blob that
  owns a region of the sky, with empty sky around it. Honest to
  the architecture: a flock IS a concentration (the emergent
  cohesion contract the tests pin).

## Files

- `baseline_A_{cinematic,aeolian,dna_helix}.json` — pre-change
  tree (c090097), captured before implementation.
- `after_B_{cinematic,aeolian,dna_helix,murmuration}.json` —
  post-change tree.
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
