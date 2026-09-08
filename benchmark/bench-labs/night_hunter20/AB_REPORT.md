<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-20 A/B report — HUD 64-col minimum usable width

Owner rule: 10 s benchmark A/B before/after any engine change, visual
(density gini, frame entropy) + performance (fps, dirty cells).
Baselines captured on the pre-change tree (b8efb15, changes stashed);
the after tree carries the NIGHT-hunter-20 HUD width mandate:
HUD_MAX_WIDTH 24 → 64, identity-line truncation 14 → 58 chars
(scn/chr), and a NEW 58-char truncation for the previously unbounded
custom palette name (clr). Dry benches (no terminal I/O),
`--benchmark --bench-duration 10 --json`. Probes: cinematic (the
default glyph scene whose HUD identity lines exercise the changed
setters in interactive use) and monolith (the structured-family
control). Note: the bench harness never constructs `HudState` (it
exists only in the interactive event loop), so the changed code is
structurally invisible to the bench path — this A/B is the owner-rule
safety verification.

## Results

| scene     | side       | avg_fps | frame_ms | p99_ms | dirty/pr | entropy_bits | density_gini |
|-----------|------------|---------|----------|--------|----------|--------------|--------------|
| cinematic | baseline_A | 27764   | 0.036    | 0.051  | 463.5    | 5.184        | 0.635        |
| cinematic | after_B    | 29011   | 0.034    | 0.054  | 457.6    | 5.166        | 0.639        |
| monolith  | baseline_A | 69913   | 0.014    | 0.020  | 140.7    | 4.175        | 0.818        |
| monolith  | after_B    | 70195   | 0.014    | 0.020  | 140.8    | 4.175        | 0.817        |

## Reading

- monolith: every visual metric identical to the third decimal
  (entropy 4.175/4.175, gini 0.818/0.817, dirty 140.7/140.8); fps
  delta +0.4% — inside noise.
- cinematic: fps delta +4.5% with a 1.3% dirty-cell shift — a third
  same-tree run landed at 28,814 fps / 458.5 dirty / 5.168 entropy,
  BETWEEN the two sides, confirming the shared-VM run-to-run noise
  band (the host is a Xeon cloud VM with smt off; the previous hunts
  documented the same band). Visual metrics are stable across all
  three runs (entropy 5.166-5.184, gini 0.635-0.639).
- The changed code (HUD setters + width clamp) executes only when the
  HUD is visible in interactive mode; the bench path constructs no
  HudState, so any real regression would have to come from code
  layout (fat-LTO), which dominates deltas of this size.

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
