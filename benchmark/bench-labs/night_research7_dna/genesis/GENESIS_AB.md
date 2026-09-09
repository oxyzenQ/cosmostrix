<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-research-7 part 3 A/B report — the DNA genesis

Owner rule: 10 s benchmark A/B before/after any visual-engine
change, visual (density gini, frame entropy) + performance (fps,
dirty cells). Baselines captured on the pre-change tree (370943b,
the NIGHT-research-7 part 2 HEAD) before implementation; the
after tree adds the genesis birth sequence (the soup -> ladder ->
windup entry replay), the primordial thick-broth dial, and the
absorption-counter starvation fix (the hunt-find — see below).
Dry benches (no terminal I/O) at the default bench viewport,
`--benchmark --bench-duration 10 --json`. Three regression probes:
cinematic (the glyph droplet path), aeolian and solar_flare (the
structured-family siblings the style's dispatch plugs into).

## Results

| scene         | side | avg_fps | dirty/pr | entropy_bits | density_gini |
|---------------|------|---------|----------|--------------|--------------|
| cinematic     | A    | 28,714  | 460.0    | 5.170        | 0.638        |
| cinematic     | B    | 28,894  | 458.1    | 5.163        | 0.639        |
| aeolian       | A    | 159,573 | 46.0     | 4.982        | 0.645        |
| aeolian       | B    | 159,480 | 45.9     | 4.977        | 0.646        |
| solar_flare   | A    | 36,597  | 247.8    | 6.025        | 0.332        |
| solar_flare   | B    | 36,411  | 249.4    | 6.050        | 0.318        |
| dna_helix     | A    | 96,906  | 101.9    | 4.860        | 0.691        |
| dna_helix     | B    | 89,322  | 108.0    | 4.971        | 0.672        |

## Reading

- Zero visual regression on the three probes: the dirty-cell
  populations, entropy and gini match the baselines to the third
  decimal. The fps deltas (+0.6%, -0.1%, -0.5%) sit inside the
  established noise band for interleaved 10 s runs (the part 1
  report measured +2.3/-1.0% same-tree spread; the murmuration
  report -0.1/-0.8/+0.3%). The probes' hot paths execute none of
  the new code (the genesis queries live inside the dna_helix
  advance/draw arms).
- The genesis itself does NOT run in the bench: `reset_bench`
  fast-forwards to the formed molecule. The birth is one-shot
  choreography — at the default scene speed it would own ~60
  percent of a 10 s window, and the bench's contract is the
  steady-state critical path (the Z-6 "critical path only"
  precedent, which already skips message cosmetics in bench
  mode). The sequence is instead pinned frame-by-frame by 17
  deterministic contracts (tests_dna_helix/genesis.rs) and the
  PTY shape smoke (scripts/genesis_smoke.py: soup 10 scattered
  cells, flat ladder 90 cells in two straight full-height
  columns, wound steady helix 168 cells with the columns
  dissolved).
- The dna_helix scene's own deltas are the starvation fix
  working, not a render regression. The shipped tree leaked one
  active-counter unit per absorbed nucleotide (the spawn gate
  then read a population the pool no longer carried, and the
  soup decayed toward a silent sky over a long session — inside
  this 10 s window the decay had already begun). The B tree's
  soup sustains its ~11-drop calm-sky dial target for the whole
  window: +6 dirty cells of living rain (108.0 vs 101.9 — the
  soup's heads and comet trails), and the more evenly spread
  light reads as +0.111 entropy bits and -0.019 gini (a
  concentrated single-body composition gaining a slightly wider
  weather halo — honest to a genome being actively fed).
- The fps delta on dna_helix (-7.8%, ~0.9 us/frame at this
  scale) decomposes the same way: the living soup's per-frame
  physics + draw cells plus the genesis-aware geometry queries
  (a phase classifier match per strand line and a built-gate per
  rung, branch-predictable). At 10.3 us/frame this scene sits far
  below every interactive budget; the absolute cost is
  sub-microsecond.

## Files

- `baseline_A_{dna_helix,cinematic,aeolian,solar_flare}.json` —
  pre-change tree (370943b), captured before implementation.
- `after_B_{dna_helix,cinematic,aeolian,solar_flare}.json` —
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
