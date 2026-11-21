<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

# NIGHT-frontier-Z6 — the energy-ledger A/B campaign

Two binaries, one commit apart: `baseline` = 0760af7 (pre-Z6),
`after` = the Z6 working tree (the wave-field energy ledger + the
quantum-clock sub-step schedule). Release profile, `--benchmark --json
--bench-duration 10 --bench-io` on the monolith scene at 120x40
(benchmark mode never arms clicks, so the ledger's active-frame pass
and the quantum clock do not execute — the steady state must be
unchanged, and it is, within same-binary run noise):

| metric                   | baseline   | after      | delta  |
|--------------------------|------------|------------|--------|
| avg_fps                  | 82527.90   | 83103.10   | +0.70% |
| dirty_cells_per_frame    | 56.787     | 56.776     | -0.02% |
| frame_entropy_bits       | 3.2963     | 3.2931     | -0.10% |
| density_gini             | 0.8960     | 0.8962     | +0.03% |
| p99_frame_time_ms        | 0.01610    | 0.01643    | +2.04% |
| alloc_calls_per_frame    | 0.000682   | 0.000677   | -0.69% |

Verdict: performance-neutral and visual-neutral within run noise. The
frame_entropy and density_gini deltas sit inside the same-binary
repeat band measured on the `after` binary alone (a second 10 s run
printed entropy 3.2945, gini 0.8961 — the visual_objective sampler
carries residual wall-clock variance), and the fps delta is far inside
the historical same-binary band (~3.5K on this host). p99 moves at the
0.016 ms scale — noise.

## The functional proof (the part the A/B cannot carry)

- The deterministic replay digest is BYTE-IDENTICAL across the change:
  `b3:7e938efa5efcb659dc214a9bf1fc84afdbbe8d6b8e0aab9f67b0b3606f1578d7`
  printed by both binaries — the render surface is untouched.
- `cargo test wave_field`: 16 wave-field pins green (10 Z4b physics +
  6 Z6 ledger laws: closed-form deposit, monotone steps, storm cap,
  ledger identity, quantum-clock quanta, rescale-zero, reset
  semantics).
- The full suite stays green: 3086 passed, 0 failed, 2 ignored.
- The resonance record: under the old fractional sub-step widths, a
  synthetic 1/60 -> 1/45 -> 1/30 -> 1/90 pacing cycle pumped the field
  exponentially (~15% per 4-frame cycle, energy reaching the cap
  inside 600 frames); under the quantum clock the same cycle decays
  monotonically to exactly zero, `dissipated == injected` to the last
  f64 bit. The ledger caught it; the quantum clock killed it.

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
