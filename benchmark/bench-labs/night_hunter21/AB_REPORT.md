<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-21 A/B report — event-loop context-struct refactor

Owner rule: 10 s benchmark A/B before/after any engine change, visual
(density gini, frame entropy) + performance (fps, dirty cells).
Baselines captured on the pre-refactor tree (7a017d7, the
NIGHT-hunter-20 HEAD, using its committed pro binary); the after tree
carries the full context-struct refactor (LoopCtx + sub-structs, 10
sibling signature conversions, event_loop.rs exemption lifted). Dry
benches (no terminal I/O), `--benchmark --bench-duration 10 --json`.
Probes: cinematic (glyph droplet path) and monolith (structured
family control).

Scope note: the bench dispatch is a SEPARATE path from the
interactive loop — the benchmark harness never constructs
`HudState`/`LoopCtx`/the event loop, so the refactored code is
structurally invisible to these benches. This A/B is the owner-rule
safety check; the behavioral verification for the refactored loop is
the PTY e2e set (hud_order_e2e, hud_long_scene_e2e, and the canonical
live-reload smoke — all green; see
docs/research/NIGHT_HUNTER_21_CTX_REFACTOR.md).

## Results

| scene     | side       | avg_fps | frame_ms | p99_ms | dirty/pr | entropy_bits | density_gini |
|-----------|------------|---------|----------|--------|----------|--------------|--------------|
| cinematic | baseline_A | 29294   | 0.034    | 0.054  | 456.6    | 5.162        | 0.640        |
| cinematic | after_B    | 28314   | 0.035    | 0.055  | 461.7    | 5.180        | 0.636        |
| monolith  | baseline_A | 70475   | 0.014    | 0.019  | 140.7    | 4.175        | 0.817        |
| monolith  | after_B    | 69904   | 0.014    | 0.020  | 141.0    | 4.175        | 0.818        |

## Reading

- monolith: every visual metric identical to the third decimal
  (entropy 4.175/4.175, gini 0.817/0.818, dirty 140.7/141.0); fps
  delta −0.8% — inside noise.
- cinematic: fps delta −3.3% with entropy/gini stable to ~0.4% —
  inside the same-tree noise band documented across the
  night_hunter20 / night_hunter4 reports (this shared Xeon VM's
  cinematic runs have ranged 27.7K-29.3K fps across consecutive
  same-binary runs). Zero code on the bench path changed.
- In the interactive loop itself the refactor is reference-count
  neutral: every sibling call previously received N `&mut` params;
  it now receives one `&mut LoopCtx` whose fields resolve to the same
  stack-relative addressing (field access on a local struct vs.
  direct locals). No allocation was added or removed; the only new
  per-frame object (`FrameObs`) is 40 bytes of stack scalars.

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
