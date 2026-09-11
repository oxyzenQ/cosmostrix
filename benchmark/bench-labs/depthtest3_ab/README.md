<!-- SPDX-License-Identifier: GPL-3.0-only -->

# depthtest3_ab — NIGHT-depthtest-3 A/B evidence (2026-09-11)

Raw 10 s benchmark JSONs for the oversized-block-name hardening
(docs/research/NIGHT_DEPTHTEST_3_NAME_LEN_CONFIG_E2E.md). Same-pipeline
methodology (the lts_final_r1r8 lesson): both sides are
`cargo build --release`.

- `A_<scene>.json` — commit 9c36a04 (pre-fix), built in a separate
  worktree.
- `B_<scene>.json` — commit 164d37d (HEAD with the fix).
- `B_cinematic_run2.json` / `B_cinematic_run3.json` — same-binary
  repeats: p95 frame time swings up to 9% run-to-run on the
  cinematic tail, which is why single-run p95 deltas are noise (the
  first B run's p95=0.1024 vs run2 0.0950 / run3 0.0939).

Scenes: cinematic, sorgonemous_intrascals.

Summary (contract: max visual delta 1%):

| metric | cinematic delta | sorgonemous delta |
|---|---|---|
| avg fps | -0.18% | +0.09% |
| dirty cells/frame | +0.22% | -0.11% |
| frame entropy (bits) | +0.11% | -0.02% |
| density gini | -0.27% | +0.09% |

FLAT — expected: the change is config-validation cold path (raw-key
pre-scans at load time), zero per-frame code.

<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying on
  anything here.
-->
