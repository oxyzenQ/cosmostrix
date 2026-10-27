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
