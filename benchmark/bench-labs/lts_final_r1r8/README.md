<!-- SPDX-License-Identifier: GPL-3.0-only -->

# lts_final_r1r8 — S-night R1-R8 A/B evidence (2026-09-11)

Raw 10 s benchmark JSONs for the LTS final audit
(docs/audits/LTS_FINAL_AUDIT_2026-09-11.md). Probes: cinematic,
sorgonemous_intrascals (the black hole), aeolian.

File roles:

- `before_<scene>.json` — first baseline attempt, captured on the
  hunt-32 session's build.sh-profile binary. Kept as methodology
  evidence only: comparing a build.sh binary against a
  cargo-build binary is NOT a valid A/B (the different codegen
  profile runs the black hole phase cycle at a different frame
  rate, so the 10 s window samples a different phase mix — a
  19 percent dirty-cell artifact on sorgonemous_intrascals).
- `before_rebuild_<scene>.json` — the true A side: commit d187afd
  rebuilt via cargo build --release in a separate worktree (same
  pipeline as the B side). These are the numbers in the audit
  table.
- `after_<scene>.json` — the B side: commit 3df685f (HEAD with the
  S-night-R4 diagnostic-sink hardening), cargo build --release.
- `before_rebuild_run2.json` / `variance_run1.json` /
  `variance_run2.json` — determinism proof: same-binary repeats
  reproduce dirty cells to the first decimal (113.1) and the
  metrics to the third, which is why the table deltas below
  0.13 percent are at the reproducibility floor.

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
