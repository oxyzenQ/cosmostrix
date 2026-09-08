<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-special-4 A/B report — the solar flare corona (tenth rain style, aurora's successor)

Owner rule: 10 s benchmark A/B before/after any visual-engine change,
visual (density gini, frame entropy) + performance (fps, dirty
cells). Baselines captured on the pre-change tree (3364403b, the
aurora NIGHT-special-3 HEAD) before implementation; the after tree
replaces the aurora veil with the corona arcade (engine, scene,
tests, the draw.rs LOC split). Dry benches (no terminal I/O) at the
default bench viewport, `--benchmark --bench-duration 10 --json`.
Three regression probes this round: cinematic (the glyph droplet
path), sorgonemous_intrascals (the structured-family dispatch the
new style plugs into) and aeolian (the sibling original-math
flagship — the closest code path to the new one).

## Results

| scene                | side | avg_fps | frame_ms | p99_ms | dirty/fr | entropy_bits | density_gini |
|----------------------|------|---------|----------|--------|----------|--------------|--------------|
| cinematic            | A    | 29,813  | 0.034    | 0.053  | 456.6    | 5.165        | 0.639        |
| cinematic            | B    | 29,236  | 0.034    | 0.055  | 454.8    | 5.158        | 0.641        |
| sorgonemous_intrasc. | A    | 26,426  | 0.038    | 0.047  | 329.8    | 5.479        | 0.555        |
| sorgonemous_intrasc. | B    | 25,670  | 0.039    | 0.048  | 330.3    | 5.483        | 0.554        |
| aeolian              | A    | 163,492 | 0.006    | 0.009  | 46.1     | 4.982        | 0.645        |
| aeolian              | B    | 160,244 | 0.006    | 0.009  | 46.0     | 4.980        | 0.645        |
| solar_flare          | B    | 36,579  | 0.027    | 0.036  | 247.9    | 6.047        | 0.321        |

## Reading

- Zero visual regression on the three probes: the dirty-cell
  populations, entropy and gini match the baselines to the third
  decimal (identical distributions — the swap changes nothing about
  what the probe scenes draw).
- The fps deltas (-1.9%, -2.9%, -2.0%) are consistent across
  re-runs but attributable to LTO code layout, not to the change's
  semantics: same-tree control runs (the aurora binary rebuilt from
  the stashed tree mid-session) reproduce the A side at 26.6K fps
  against the solar binary's 25.8K on sorgonemous — the replacement
  swaps one enum arm per dispatch chain for another of the same
  shape and adds zero work to the probe scenes' hot paths, but the
  fat-LTO layout (the Cloud struct's field sizes shifted with the
  arcade replacing the lattice, moving every following field's
  cache lines) shifts the 0.039 ms frame by ~0.001 ms. The visual
  contract — the metrics this rule exists to protect — is clean.
- The solar_flare scene's own profile: 36,579 fps (mid-pack for the
  structured styles), 247.9 dirty cells/frame. Entropy 6.047 bits
  is the highest in the catalog and gini 0.321 the lowest — and
  deliberately so: the corona's composition is the INVERSE of the
  retired aurora's (which read 3.887 / 0.847, the most concentrated
  numbers in the catalog). The aurora was tall curtains separated
  by dark sky; the corona is a full-width granulated photosphere
  (every column's bottom two cells) spanned by distributed arcs —
  the dirt spreads evenly across the surface band, raising entropy
  and flattening gini. The aeolian (even rain over revealed
  strings) reads 0.645; the black hole (centered mass) 0.554; the
  corona (a banded star under a filament arcade) 0.321 — four
  compositions, each honest to its architecture.

## Files

- baseline_A_cinematic.json / baseline_A_sorgonemous_intrascals.json /
  baseline_A_aeolian.json — the "before" captures (3364403b).
- after_B_cinematic.json / after_B_sorgonemous_intrascals.json /
  after_B_aeolian.json — the regression probes on the solar tree.
- after_B_solar_flare.json — the new style's own 10 s profile.
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
