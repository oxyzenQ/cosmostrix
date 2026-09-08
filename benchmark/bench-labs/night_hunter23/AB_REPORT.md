<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-23 A/B report — intro-color gate fix + watcher signature bundle

Owner rule: 10 s benchmark A/B before/after any code change, visual
(density gini, frame entropy) + performance (fps, dirty cells).
Baseline A built from the clean F2 HEAD (9e7be8f, pro profile); after
B carries the hunter-23 changes (intro-color gate: canonical palette
recognition at startup + live-reload; watcher: WatchSession bundle,
8 params -> 2). Dry benches (no terminal I/O),
`--benchmark --bench-duration 10 --json`. Probes: cinematic (glyph
droplet path) and monolith (structured family control).

Scope note: both changes execute at startup (config_apply gate) or on
the live-reload path (watcher event handling, rebuild gate) — none is
on the frame path. The benches are the owner-rule safety check.

## Results

| scene     | side       | avg_fps | frame_ms | p99_ms | dirty/fr | entropy_bits | density_gini |
|-----------|------------|---------|----------|--------|----------|--------------|--------------|
| cinematic | baseline_A | 29091   | 0.034    | 0.056  | 456.6    | 5.164        | 0.640        |
| cinematic | after_B    | 29165   | 0.034    | 0.055  | 458.3    | 5.167        | 0.639        |
| monolith  | baseline_A | 69911   | 0.014    | 0.019  | 140.8    | 4.175        | 0.818        |
| monolith  | after_B    | 70270   | 0.014    | 0.020  | 140.8    | 4.175        | 0.818        |

## Reading

- monolith: every visual metric identical to the third decimal
  (entropy 4.175/4.175, gini 0.818/0.818, dirty 140.8/140.8); fps
  delta +0.5% — inside noise.
- cinematic: fps delta +0.25%; entropy/gini/dirty stable to ~0.1% —
  inside the documented shared-VM noise band. Zero frame-path code
  changed.
- Behavioral verification (the bench cannot see the gates): 2551-test
  suite with the 5 new gate tests + the live-reload PTY e2e (mid-run
  intro-color switch to a rain-only palette applied and honestly
  diffed at exit).

## Artifacts

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
