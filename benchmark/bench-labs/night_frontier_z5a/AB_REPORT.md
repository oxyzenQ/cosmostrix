<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

# NIGHT-frontier-Z5a — the paste fuse A/B campaign

Two binaries, one commit apart: `baseline` = 12f9c9a9 (pre-Z5a),
`after` = the Z5a working tree. Release profile, `--benchmark --json
--bench-duration 10 --no-effects`, two scenes x two runs per side, same
host, same toolchain. The fuse touches the input path only; the bench
carries no paste traffic, so the steady state must be unchanged - and
it is, within same-binary run noise.

## Results (mean of 2 runs per side)

| scene     | metric                      | baseline    | after       | delta  |
|-----------|-----------------------------|-------------|-------------|--------|
| monolith  | avg_fps                     | 87890.44    | 87801.42    | -0.10% |
| monolith  | dirty_cells_per_frame       | 56.77       | 56.77       | -0.00% |
| monolith  | frame_entropy_bits          | 3.2953      | 3.2939      | -0.04% |
| monolith  | density_gini                | 0.8961      | 0.8962      | +0.01% |
| cinematic | avg_fps                     | 29668.27    | 29520.08    | -0.50% |
| cinematic | dirty_cells_per_frame       | 455.67      | 456.78      | +0.24% |
| cinematic | frame_entropy_bits          | 5.1607      | 5.1650      | +0.08% |
| cinematic | density_gini                | 0.6402      | 0.6391      | -0.17% |

Verdict: performance-neutral and visual-identical within run noise
(the historical same-binary peak spread on this host is ~3.5K fps on
cinematic; every delta above sits inside that band).

## The functional proof (the part the A/B cannot carry)

- torture 7/7 legs PASS on the Z5a release binary, including the new
  dedicated `paste-wedge` leg (dangling `ESC[200~` + hostile content +
  the `q` quit path wins, rc=0, restore tail intact). Pre-Z5a the same
  shape hit the 30s watchdog SIGKILL (rc=-9).
- The input-flood leg regains the dangling START marker in its junk
  pool and stays green.
- Unit: the rewritten PasteBurstGuard suite (burst arming, suppression
  window, expiry, human-typing pass-through, auto-Repeat immunity,
  shortcut-letter immunity) all green; full `cargo test` suite green.

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
