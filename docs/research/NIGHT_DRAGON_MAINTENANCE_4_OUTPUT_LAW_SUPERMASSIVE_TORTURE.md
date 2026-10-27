<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Night Dragon Maintenance 4 — Audit Trio, Output Color Law, Loading Indicator, Supermassive Torture

Owner: oxyzenQ | Session: night-dragon-maintenance-4 | Date: 2026-10-27
Reference: github.com/oxyzenQ/zejtron (mature infra lineage)

## 1. Secret token name consistency audit — verdict: CONSISTENT, zero drift

Fresh repo-wide scan this session (every `secrets.*` reference, every
token-shaped env name across all 11 workflows, the token-resolution
script, and the docs):

| Credential | Canonical secret name | Sites | Alias env | Alias rationale |
|------------|----------------------|-------|-----------|-----------------|
| GitHub API | `GITHUB_TOKEN` | 3 workflows | — | GitHub's reserved name |
| crates.io publish | `CRATES_IO_TOKEN` | crates-io.yml | `CARGO_REGISTRY_TOKEN` | cargo's required registry-token variable — the mapping lives in the workflow, the secret name stays canonical |
| GPG signing key | `GPG_PRIVATE_KEY` | release.yml | — | — |
| GPG passphrase | `GPG_PASSPHRASE` | release.yml | — | — |
| AUR deploy key | `AUR_SSH_PRIVATE_KEY` | aur.yml | `AUR_SSH_KEY` | ssh(1)'s required key-variable name — same aliasing law as cargo |
| AUR passphrase | `AUR_SSH_PASSPHRASE` | aur.yml | — | — |

Consistency findings, with the traps explicitly cleared:

- `AUR_SSH_KEY` is NOT a second secret name — it is the ssh(1)-required
  env variable fed from the one canonical secret
  (`AUR_SSH_KEY: ${{ secrets.AUR_SSH_PRIVATE_KEY }}`). One credential,
  one secret name, one documented alias.
- `CARGO_REGISTRY_TOKEN` likewise: cargo's own variable name, fed from
  `secrets.CRATES_IO_TOKEN`. Identical pattern and identical comment
  law as the zejtron crates-io workflow.
- `GPG_KEY_ID` (release.yml) and `GPG_KEY_FINGERPRINT`
  (maintenance.yml) are PUBLIC identifiers, not secrets: the fingerprint
  is a plaintext workflow constant (the signing key's public identity),
  the key id is parsed from gpg output at run time. The lone `KEY_ID`
  token in maintenance.yml is a local shell variable in the same parse
  block, not a credential name.
- `scripts/lib/github_token.sh` resolves exactly `GITHUB_TOKEN` (env ->
  gh -> git credential helper) — no second name introduced anywhere.
- Docs name the secret once, in the setup instruction
  (docs/workflow/ABOUT_CI.md: `CRATES_IO_TOKEN`), matching the
  workflow; maintenance-0's verdict table already recorded the same
  mapping and this scan found zero drift since.

Verdict: every credential has exactly one canonical secret name, every
alias exists only because the consuming tool (cargo, ssh) mandates that
variable name, and the naming set matches the zejtron reference 1:1
where the surfaces overlap. Nothing to rename; no commit-level churn
warranted beyond this record.

## 2. Miri 30-minute hang mitigation — verdict: ALREADY IN PLACE, three layers

The owner's incident (miri build hanging past 30 minutes because miri
interpreted docs-burden tests) was already answered on this repo in the
night-dragon-maintenance-1 pass, ported from the zejtron lane. This
audit re-verified every layer against the live tree:

1. Admission law (.github/workflows/miri.yml header +
   scripts/lib/miri.sh MIRI_AUDIT_MODULES): the miri lane audits the
   six pointer/alloc/UB families only — never the docs lane. The
   documented incident shape (two full-text README/CHANGELOG scan tests
   burning ~23 interpreted minutes) cannot re-enter without an explicit
   `#[cfg_attr(miri, ignore = "reason")]` opt-out, and a fresh grep
   confirms the six families carry no docs-scan or unbounded-loop test
   bodies today (so zero ignore attrs are needed).
2. Fail-fast cap: the miri job runs `timeout-minutes: 20` — sized to
   the measured record (the interpreted step's own docs record is 1-3
   min, ~5x headroom) — so any future burden test dies loudly inside
   the budget instead of hanging the runner for half an hour.
3. Ownership of the burden work: the docs assertions live in the
   `docs_tests` module (test/docs_tests/, included via src/main.rs and
   verified present) and run natively under `cargo test` in ci.yml —
   the assertions still run per commit, just never under miri's
   interpreter. Miri also cancels superseded runs (concurrency group,
   cancel-in-progress) so rapid pushes cannot queue duplicate hangs.

Verdict: the incident class is structurally closed — burden tests are
admitted nowhere, the budget cannot be exceeded silently, and the
assertions lost nothing (they run in the fast lane every push). No
further mitigation warranted.

## 3. Minimum-kernel low-spec lane audit — verdict: floor is 2.6.27 (documented and live-proven); a 5.15.0 lane would be redundant

The directive asked for the missing supermassive low-spec leg at the
project's minimum kernel, citing 5.15.0. The depth audit found a
different documented truth:

- The docs' kernel law (docs/SYSTEM_REQUIREMENTS.md): the per-syscall
  table's highest REQUIRED entry is `inotify_init1` at 2.6.27 (live
  config reload via the notify crate); `getrandom` (3.17) carries an
  explicit /dev/urandom fallback and `io_uring` (5.1) is documented as
  NOT used. The stated law is "Practical minimum: Linux 2.6.27
  (December 2008)" with the recommended column at 5.0+.
- A repo-wide scan for a 5.15.0 kernel statement returns zero kernel
  hits — every `5.15` match in the tree is an entropy value from
  benchmark prose (bits, not versions).
- The low-spec KVM lane the directive wants already exists and boots
  the REAL documented floor: supermassive-kernel.yml `kernel_floor`
  runs the intrepid GA 2.6.27 kernel in a micro-VM with the era-
  contemporary busybox userland and the battery as root inside the
  guest (night-dragon-maintenance-2), paired with `kernel_best` on the
  newest mainline kernel.

A 5.15.0 lane would sit strictly between the two existing lanes:
2.6.27 already proves the binary boots and renders on a kernel twelve
major versions OLDER than 5.15, and kernel_best proves the newest.
Anything 5.15.0 can prove about low-spec behavior is already implied by
the floor lane passing. Adding it would be a redundant leg — the
over-engineering the standing LTS rule orders to skip. If 5.15.0 ever
becomes a real support boundary, the syscall table is the place that
must first grow a 5.15-only requirement (it has none today); the lane
would then follow the docs, not precede them.

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
