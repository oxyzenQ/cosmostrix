<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-22 A/B report — post-exit printer value-structs

Owner rule: 10 s benchmark A/B before/after any engine change, visual
(density gini, frame entropy) + performance (fps, dirty cells).
Baseline A captured on the pre-change tree (f76468f, hunter-21 HEAD,
pro build from the stashed-clean sources); after B carries the full
NIGHT-hunter-22 change (SessionState value struct + final_state.rs
extraction, TerminalIoStats bundle, post_exit args-drop). Dry benches
(no terminal I/O), `--benchmark --bench-duration 10 --json`. Probes:
cinematic (glyph droplet path) and monolith (structured family
control).

Scope note: the bench dispatch is a separate path that exits before
any terminal/event-loop construction — the post-exit family
(final-state statics, SessionState, print_perf_report) never runs
under the bench harness. This A/B is the owner-rule safety check; the
behavioral verification is the 2539-test suite (full 23-field
round-trip + both constructor mapping tests).

## Results

| scene     | side       | avg_fps | frame_ms | p99_ms | dirty/fr | entropy_bits | density_gini |
|-----------|------------|---------|----------|--------|----------|--------------|--------------|
| cinematic | baseline_A | 29159   | 0.034    | 0.053  | 457.3    | 5.161        | 0.640        |
| cinematic | after_B    | 28953   | 0.035    | 0.056  | 458.3    | 5.167        | 0.639        |
| monolith  | baseline_A | 68170   | 0.015    | 0.023  | 140.8    | 4.175        | 0.817        |
| monolith  | after_B    | 70054   | 0.014    | 0.020  | 140.8    | 4.175        | 0.817        |

## Reading

- monolith: every visual metric identical to the third decimal
  (entropy 4.175/4.175, gini 0.817/0.817, dirty 140.8/140.8); fps
  delta +2.8% — inside noise.
- cinematic: fps delta -0.7%; entropy/gini/dirty stable to ~0.2% —
  inside the documented shared-VM noise band (27.7K-29.3K fps across
  consecutive same-binary runs, see the night_hunter20 /
  night_hunter21 reports). Zero code on the bench path changed.
- The change is structurally invisible to the harness: the touched
  code runs once, after a loop the bench never enters.

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
