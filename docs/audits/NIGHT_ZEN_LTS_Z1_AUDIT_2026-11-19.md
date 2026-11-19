<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# Infrastructure depth audit Z-1: the soak lane, the supply-chain pins, and the gatekeeper estate at the Z9 head (NIGHT-zen-lts-Z-1)

Date: 2026-11-19
Scope: everything that builds, verifies, releases and guards the
product, audited as the delta since NIGHT-zen-lts-2 (which closed at
14 workflows and 40 scripts): the tree now carries 15 workflows (the
Z6-S5 soak lane joined), 43 census gates and pins grew by three
(unsafe-census, lock-census, scope-census), two new supermassive legs
(wakeup_budget.py, emission_floor.py), a hardened `deny.toml`, and the
Z5c compiler-enforced unsafe law in Cargo.toml. Audit baseline: 258a415
(HEAD of main). Method: full structural census of the new workflow,
execution of the complete 28-check gatekeeper suite, ruff and header
census of every new Python leg, the deny.toml and lints diffs reviewed
in full, and one environment-sourced failure root-caused to evidence.

## Verdict

**One gate failure found, root-caused to the AUDIT SANDBOX (not the
repo), resolved through the gate's own fix path; everything else at
peak. The new soak lane passes the full structural census on its first
audit.**

## 1 - The soak lane's first structural audit: at peak

`soak.yml` (night-frontier-Z6 S5, the determinism-anchored endurance
lane) audited against the estate-wide hardening laws:

| Law | Evidence |
|---|---|
| least privilege | workflow-level `permissions: contents: read` |
| supersede, don't queue | concurrency group + `cancel-in-progress: true` (a second Sunday trigger stops the still-running soak instead of billing a duplicate multi-hour runner) |
| bounded | `timeout-minutes: 355` with the arithmetic in the comment (build + two anchors + window + 120 s grace) inside the 6-hour hosted cap |
| runner pin law | `ubuntu-24.04`, no floating alias |
| credential hygiene | `actions/checkout@v7` + `persist-credentials: false` |
| honest scoping | dispatch input clamped at 330 minutes with an explicit failure naming the self-hosted path for the real 1440-minute anchor |
| record retention | census artifacts upload `if: always()` |

The lane is scheduled-only (Sunday 03:00 UTC) so the two-minute
per-push law is untouched, and the harness owns the pass law
(soak_determinism.sh) while the workflow only schedules and budgets -
the same split every other lane uses.

## 2 - The supply chain hardened with teeth (Z6 delta): at peak

- `deny.toml`: `yanked = "deny"` (a yanked crate must never re-enter a
  build, advisory or no advisory - the arrayref republish precedent)
  and wildcard requirements denied (no `*` version delegation).
- Three new census gates enforce the pins mechanically:
  unsafe-census (61 unsafe lines across 17 files, exactly the pinned
  surface, render core compiler-forbidden), lock-census (121 crate
  names across 124 package entries, exactly the pinned graph, zero
  malicious-family crates), scope-census (12 runtime + 2 dev
  dependencies, zero capability markers in src/).
- Cargo `[lints]` carry the proof cfgs (loom, kani) and
  `undocumented_unsafe_blocks = "deny"`.

## 3 - New Python legs: hygiene at peak

`wakeup_budget.py` (the whole-frame syscall budget) and
`emission_floor.py` (the stream identity) both: ruff-clean, carry the
frozen SPDX header, declare their PLATFORM law in the header (Linux-
only lanes, strace versus stream-reader scope split documented), and
join the suite-count law via the 92aa69c sync (ledger text matches the
tree).

## 4 - The permissions-gate incident: environment artifact, root-caused

The full gatekeeper run in this sandbox reported 27 passed / 1 failed -
the permissions gate, at 1476 working-tree violations. Root cause
chain, each step evidenced:

1. The sandbox umask is `0002`; the clone's checkout therefore lands
   files at 664 and directories at 775.
2. The GIT INDEX is law-compliant: `git ls-files -s` censuses exactly
   1180 x `100644` and 66 x `100755` - the tracked state never
   deviated.
3. A scratch clone performed under `umask 022` PASSES the same gate:
   "OK: 1246 tracked files at 644/755 (66 executables), 230 directories
   at 755, shebang parity holds across 66 scripts".
4. Resolution: the tree was restored to the law via the gate's own
   `--fix` path (664 -> 644, a no-op for git since both map to
   100644), and the full suite re-ran **28 passed, 0 failed**.

Honest note for future agent sessions: run `umask 022` before cloning
or expect this artifact; CI (umask 022) has always passed this gate.

## 5 - Skipped follow-ups and why

- A Dependabot config: the estate pins dependencies by policy and
  enforces the pin health through check-actions-pins.sh and the
  actions-health lane; adding a second mechanism would duplicate the
  review surface without new teeth.
- Centralizing the `concurrency:` block behind a reusable workflow:
  unchanged from zen-lts-2's rationale (GitHub Actions has no fragment
  include; three lines per file is the cheaper contract).
- Pinning actionlint itself: actionlint.yaml documents exactly why the
  floating install is the owner's policy (a freshly-released actionlint
  must not fail on labels it does not know yet; the config file is the
  knowledge carrier).

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
