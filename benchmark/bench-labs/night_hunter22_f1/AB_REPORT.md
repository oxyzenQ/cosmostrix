<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-22 (F1) A/B report — startup config-parse memo

Owner rule: 10 s benchmark A/B before/after any engine change, visual
(density gini, frame entropy) + performance (fps, dirty cells).
Baseline A captured on the pre-change tree (bdf34ed, the hunter-22
printers HEAD, pro build from the stashed-clean sources); after B
carries the F1 change (configfile_load.rs extraction + startup-parse
memo + intro palette path fix). Dry benches (no terminal I/O),
`--benchmark --bench-duration 10 --json`. Probes: cinematic (glyph
droplet path) and monolith (structured family control).

Scope note: the memo executes during startup (config_apply +
build_cloud_cfg run before the bench dispatch) and touches no frame
path — the bench loop never loads the config. The behavioral
verification is the 2543-test suite (memo contract + intro palette
path regression) plus the PTY startup smoke (config + verbose +
post-exit chain, real binary).

## Results

| scene     | side       | avg_fps | frame_ms | p99_ms | dirty/fr | entropy_bits | density_gini |
|-----------|------------|---------|----------|--------|----------|--------------|--------------|
| cinematic | baseline_A | 29078   | 0.034    | 0.056  | 455.6    | 5.162        | 0.640        |
| cinematic | after_B    | 28972   | 0.035    | 0.054  | 457.2    | 5.163        | 0.640        |
| monolith  | baseline_A | 70313   | 0.014    | 0.019  | 140.9    | 4.175        | 0.817        |
| monolith  | after_B    | 69500   | 0.014    | 0.021  | 140.9    | 4.175        | 0.817        |

## Reading

- monolith: every visual metric identical to the third decimal
  (entropy 4.175/4.175, gini 0.817/0.817, dirty 140.9/140.9); fps
  delta -1.2% — inside noise.
- cinematic: fps delta -0.4%; entropy/gini/dirty stable to ~0.4% —
  inside the documented shared-VM noise band (27.7K-29.3K fps across
  consecutive same-binary runs, see the night_hunter20 / 21 / 22
  reports). Zero frame-path code changed.
- Startup itself got cheaper (9 parses -> 1) — invisible in a 10 s
  bench by construction, verified functionally instead.

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
  any number, path, or symbol name here.
-->
