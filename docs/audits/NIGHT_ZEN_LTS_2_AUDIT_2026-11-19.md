<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# Infrastructure depth audit: workflows, scripts, gates, release chain (NIGHT-zen-lts-2)

Date: 2026-11-19
Scope: everything that builds, verifies, releases and guards the
product rather than the product itself — the 14 GitHub Actions
workflows in `.github/workflows/`, the 40 shell scripts under
`scripts/`, the gate suite in `scripts/gates/`, the release
reproducibility chain, and the CI watch tooling. Audit baseline:
15656078 (HEAD of main) plus the workflow edits landed with this
commit. Method: structural census of every workflow (job timeouts,
permission blocks, concurrency groups, runner pins), full-file
strict-mode census of every shell script (not just the shebang block),
execution of the six self-audit gates, and YAML re-validation of every
edited workflow.

## Verdict

**One defect found and fixed in this commit; everything else at peak.**

## 1 — The concurrency law had three holdouts (FIXED)

Eleven of fourteen workflows declare the estate-wide cancel-in-progress
group (`group: ${{ github.workflow }}-${{ github.ref }}`,
`cancel-in-progress: true` — ci.yml:1013, fuzz.yml:65 and all release,
maintenance, guard and sandbox lanes). Three did not:

| Workflow | Trigger | Cost of the gap |
|---|---|---|
| `kani.yml` | every push + Sunday schedule | bounded model checking ran to completion for a superseded SHA; a rapid push pair paid for two full sweeps with the first discarded unread |
| `loom.yml` | every push + Sunday schedule | same shape for the preemption-bounded concurrency proofs |
| `codeql.yml` | every push, PR, Sunday schedule | analysis of a superseded commit kept consuming the security lane |

Fix (this commit): the same three-line block appended to each, no
trigger or permission touched. YAML re-validated for all three;
`check-ci-path-filters.py` and `check-actions-pins.sh` re-run green.

## 2 — Shell strict-mode census: no defect, by design

Forty shell scripts. Full-file census (not first-8-lines) for
`set -e` / `set -u` / `pipefail`:

- 28 standalone scripts carry strict mode (mostly `set -uo pipefail`
  or `set -euo pipefail`).
- 8 files under `scripts/lib/` (common.sh, builds.sh, quality.sh,
  miri.sh, pgo.sh, help.sh, github_token.sh, changelog-ledger.sh,
  version-sync.sh) are SOURCED libraries: they intentionally leave
  mode-setting to the sourcing entrypoint, because a lib flipping
  `errexit` would silently change the contract of every caller.
- `scripts/sandbox/floor-payload.sh` and `microvm-init.sh` are POSIX
  `sh` BY LAW: the floor lane's userland is a busybox skeleton and
  `microvm-init.sh` runs as PID 1 inside the initramfs, where dying on
  an unset variable would brick the VM instead of reporting. The
  scripts' own headers document this.
- `scripts/depth/cli_*_stresstest.sh` carry `set -uo pipefail`
  (unset-var + pipe failures fatal) but not `errexit` — deliberate:
  the batteries grep for expected failures and must keep counting.

Verdict: at peak. Adding blanket `set -euo pipefail` to libs and PID-1
payloads would be over-engineering against documented design.

## 3 — The six self-audit gates all pass at this commit

| Gate | Result |
|---|---|
| check-actions-pins.sh | PASS (every third-party action pinned) |
| check-ci-path-filters.py | PASS |
| check-permissions.sh | PASS (tracked modes: only .sh/.py at 755) |
| stale-hunt.py | PASS (zero stale references) |
| emoji-audit.py | PASS |
| check-version-anti-patterns.sh | PASS |
| docs-audit.py | PASS (no broken refs, no stale counts) |

Workflow hardening census at HEAD (all 14 workflows): every job carries
`timeout-minutes`, every workflow carries an explicit `permissions:`
block (maintenance/release/crates-io use per-job least-privilege
blocks), every workflow except the three fixed in section 1 carries the
concurrency group, and every runner is pinned by image name
(NIGHT-runner-pin law) — ubuntu-24.04 / macos-26, no floating
`-latest` aliases.

## 4 — Release chain audit

The reproducible chain is intact end to end at this commit:
`reproducible_build` (two-host matrix, pinned toolchain, flat
artifacts) -> `reproducible_verify` (byte `cmp` + sha256 echo + the
release-shaped binary must actually run its replay oracle). The
night-depth-hardening-4 commit extends this same verify lane with the
cross-libm digest comparison — see
`NIGHT_DEPTH_HARDENING_4_AUDIT_2026-11-19.md` for that half.

`cargo-deny` is not installable in this sandbox (no network registry
access for the plugin); the Security Audit lane (cargo-audit +
deny-equivalent policy via deny.toml) is green on HEAD 15656078, which
covers the advisory and license surface for this audit cycle.

## 5 — Skipped follow-ups and why

- Centralizing the duplicated `concurrency:` block behind a reusable
  workflow: GitHub Actions has no fragment-include; an action or
  workflow_call indirection would cost more review surface than 3 x
  3 lines of YAML.
- Pinning `dtolnay/rust-toolchain@stable` by SHA: the pins gate
  already asserts the current pin policy repo-wide; changing one
  action's pin style would make the estate inconsistent.
- Scripts LOC caps: `check-scripts-loc.sh` passes; no oversized script
  found, so no split work.
