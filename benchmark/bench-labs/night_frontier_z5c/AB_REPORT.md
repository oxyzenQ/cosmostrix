<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# NIGHT-frontier-Z5c — the unsafe-law A/B campaign

Two binaries, one commit apart: `baseline` = 8288b8ff (pre-Z5c),
`after` = the Z5c working tree (the forbid(unsafe_code) attributes +
the census gate). Release profile, `--benchmark --json
--bench-duration 10 --no-effects`, two scenes x two runs per side, same
host, same toolchain. The forbid attribute is a compile-time lint —
zero codegen effect — so the steady state must be unchanged, and it
is, within same-binary run noise:

| scene     | metric                | baseline  | after     | delta  |
|-----------|-----------------------|-----------|-----------|--------|
| monolith  | avg_fps               | 87625.64  | 87722.04  | +0.11% |
| monolith  | dirty_cells_per_frame | 56.78     | 56.79     | +0.01% |
| monolith  | frame_entropy_bits    | 3.2939    | 3.2958    | +0.06% |
| monolith  | density_gini          | 0.8962    | 0.8961    | -0.01% |
| cinematic | avg_fps               | 29668.82  | 29502.12  | -0.56% |
| cinematic | dirty_cells_per_frame | 455.66    | 456.80    | +0.25% |
| cinematic | frame_entropy_bits    | 5.1624    | 5.1649    | +0.05% |
| cinematic | density_gini          | 0.6401    | 0.6391    | -0.16% |

Verdict: performance-neutral and visual-identical within run noise —
the deltas sit inside the historical same-binary band (the cinematic
fps spread alone is ~3.5K on this host).

## The functional proof (the part the A/B cannot carry)

- `cargo check --all-targets` fails the moment any unsafe block lands
  in the five pinned trees (chroma, crystal, cosmic cloud/frame/
  runtime) — the compiler now IS the gate; a probe unsafe in
  `frame.rs` was compile-rejected before being removed.
- `scripts/gates/unsafe-census.py` passes on the clean tree (61 unsafe
  lines across 17 OS-boundary files, exactly the pin) and fails on a
  synthetic drift probe (a NEW-unsafe finding naming the file).
- The chroma test seat's env mutation routes through the crate-root
  `test_support::set_env_var` helper — the one audited unsafe home for
  the test harness, outside the forbidden trees; the full
  `cargo test` suite (3082 tests) stays green.

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
