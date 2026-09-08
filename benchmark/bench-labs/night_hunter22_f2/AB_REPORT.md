<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-22 (F2) A/B report — duration dual-field deletion

Owner rule: 10 s benchmark A/B before/after any code change, visual
(density gini, frame entropy) + performance (fps, dirty cells).
Baseline A built from the clean F1 HEAD (230e804, pro profile); after
B carries the F2 change (CloudConfig raw `duration` twin deleted,
single `duration_s` source of truth). Dry benches (no terminal I/O),
`--benchmark --bench-duration 10 --json`. Probes: cinematic (glyph
droplet path) and monolith (structured family control).

Scope note: the deletion touches the CloudConfig struct (one field
removed), its construction in build_cloud_cfg, and the event loop's
pre-loop end_time derivation (computed ONCE before the first frame;
the frame path never reads either field). Both bench binaries embed
sha 230e804 (after B was built from the uncommitted working tree on
that HEAD).

## Results

| scene     | side       | avg_fps | frame_ms | p99_ms | dirty/fr | entropy_bits | density_gini |
|-----------|------------|---------|----------|--------|----------|--------------|--------------|
| cinematic | baseline_A | 29038   | 0.034    | 0.053  | 456.3    | 5.163        | 0.640        |
| cinematic | after_B    | 29044   | 0.034    | 0.056  | 456.6    | 5.167        | 0.639        |
| monolith  | baseline_A | 69377   | 0.014    | 0.020  | 140.8    | 4.175        | 0.817        |
| monolith  | after_B    | 69510   | 0.014    | 0.019  | 140.8    | 4.175        | 0.817        |

## Reading

- monolith: every visual metric identical to the third decimal
  (entropy 4.175/4.175, gini 0.817/0.817, dirty 140.8/140.8); fps
  delta +0.2% — inside noise.
- cinematic: fps delta +0.02%; entropy/gini/dirty stable to ~0.1% —
  inside the documented shared-VM noise band. Zero frame-path code
  changed.
- Behavioral verification instead of bench coverage: the timed PTY
  smoke (`--duration 0.6` on the real debug binary) exited cleanly
  at ~630 ms wall, and the no-duration control ran until the 8 s
  external kill — the single `duration_s` field alone drives the
  auto-exit deadline, `None`/`0` keep run-forever behavior.

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
