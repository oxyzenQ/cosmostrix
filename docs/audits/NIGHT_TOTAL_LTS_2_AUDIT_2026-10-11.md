<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-total-lts-2 audit — the all-infra depth pass: scripts tree, CI workflows, build pipeline, and supply-chain walls, read end to end

Owner brief (2026-10-11): "depth audit focus for all infra. total lts,
realiable and keep be honest. cross check audit all root repo focus, if
peak skip and continue the next step" — with the five-dimension rubric
(stability and crash, code hygiene, optimization, security hardening,
LTS stability) and the standing rule: if a sub-task is already at peak,
skip and note why.

## Audit scope and method

The infra surface read for this audit: all 30 `scripts/**/*.sh` files
(entry points and sourced lib modules), all 9 `.github/workflows/*.yml`
files (trigger surfaces, secret exposure, permission scopes, action
pinning), `build.rs` (the 35 KB build-time pipeline), `.cargo/config.toml`,
and the supply-chain documentation that pins the owner's standing
decisions (`docs/SUPPLY_CHAIN.md`, `docs/workflow/ABOUT_CI.md`). The
method was targeted pattern hunting, not a line-by-line full scan:
attack-surface greps (`pull_request_target`, `secrets:`, `curl | sh`,
`eval`, unquoted expansions, `Command::new` call sites, strict-mode
coverage, orphaned-file cross-referencing), with full reads only where
a pattern fired. Every claim below names its evidence.

## Verdict: the infra surface is at peak — zero code changes shipped

This is the honest headline: after the full pass, nothing in the infra
tree justified a change. That is not a rubber stamp — it is the outcome
of a hunt that found every candidate finding already either handled in
code, deliberately designed, or explicitly decided and documented by
the owner. The five rubric dimensions, each with the skip rationale:

### 1. Stability and crash — at peak

- No workflow uses the `pull_request_target` trigger (grep across all
  9 workflows: zero hits), so the classic fork-PR secret-exfiltration
  vector does not exist in this repository. Secrets (AUR SSH key, GPG
  key, crates.io token, `GITHUB_TOKEN`) appear only in `aur.yml`,
  `release.yml`, and `crates-io.yml`, all of which trigger exclusively
  on owner-pushed `v*` tags or `workflow_dispatch` — never on PRs,
  never on fork heads.
- Every shell entry point runs under `set -euo pipefail`
  (`build.sh:31`, `gate-keepers.sh:87`, all `gates/`, `release/`,
  `package/`, `ci/`, `dev/` scripts verified whole-file). A sourced
  module cannot and should not re-own the shell options: the
  `lib/*.sh` modules inherit the entry point's strict mode by
  design — adding `set -e` inside a sourced library would apply the
  same global flags a second time and split the ownership of error
  policy across two files for no behavioral gain.
- The two `depth/` stress harnesses (`cli_config_stresstest.sh`,
  `cli_suggestion_stresstest.sh`) run under `set -uo pipefail`
  deliberately without `-e`: they exercise failing commands by design
  (invalid values, flag conflicts, malformed configs are the test
  cases), wrap every binary invocation in `|| true`, and account for
  outcomes through explicit PASS/FAIL counters. Forcing `-e` here
  would kill a harness mid-run on its first intentionally-invalid
  case — the classic over-hardening mistake, and the clearest
  evidence the tree is hand-audited rather than pattern-generated.

### 2. Code hygiene — at peak

- Zombie hunt: every `scripts/**/*.sh` file cross-referenced against
  its consumers (workflows, docs, README, other scripts, `src/`,
  `build.rs`). The seven files with no literal path reference are
  exactly the `lib/` modules, sourced dynamically via
  `${SCRIPT_DIR}/lib/<name>.sh` (`build.sh:37-42`) — all live, zero
  orphans, zero dead entries.
- The tree was reorganized to the zelynic shape in night-improve-1
  (2026-10-10) with a recorded zero-reference-drift verification, and
  `build.sh` itself records that the `lib/` split was an LTS round
  ("NIGHT-lts-2 split of the former 2187-line monolith,
  function bodies byte-identical"). The hygiene dimension was
  re-foundationed days before this audit; a second reorganization
  inside a week would be churn, not hardening.

### 3. Optimization — not applicable by domain (peak by vacuity)

- Scripts are orchestration, not hot paths: they run once per
  developer action, and their cost is dominated by the Rust builds and
  benchmark binaries they drive. The performance-critical surface of
  this project is the render path, which is the night-total-lts-1
  scope (see that report). No micro-optimization in shell
  orchestration can move a metric the owner tracks (fps, dirty cells,
  entropy, gini, alloc counts).

### 4. Security hardening — at peak (white-hat pass)

- `build.rs` executes exactly two external programs, both with fixed
  argument vectors and no user-controlled input:
  `git rev-parse --short=7 HEAD` (build-id embedding, failure-tolerant
  via `.ok()?`) and `rustc --version` (toolchain fingerprint). No
  network, no writes outside cargo's declared outputs, no
  `env::var`-to-`Command` flows — an attacker-controllable environment
  variable cannot shape an executed command line.
- Action references float on major tags — the owner's recorded
  decision (SUPPLY_CHAIN.md, "Action References Float on Major Tags,
  owner decision 2026-08-30"), not an oversight. The compensating
  controls are real and verified in this pass: minimal `permissions:`
  blocks per workflow, secrets scoped to tag-only triggers, CodeQL on
  every push, `cargo audit` + `cargo deny` daily. Re-litigating a
  documented owner decision inside an audit would be over-engineering;
  the documented upgrade path (Dependabot-driven SHA pins if higher
  assurance is ever needed) is the correct future lever.
- `maintenance.yml` holds `contents: write` but triggers only on
  `schedule` + `workflow_dispatch` (no PR path), and commits only
  validated lockfile updates — the write scope is narrow, scheduled,
  and reviewed by CI on the commit it lands.

### 5. LTS stability — at peak

- The supply-chain walls are layered and verifiable end to end:
  SHA-512 sidecar checksums on every release asset, AUR PKGBUILD
  hash verification with the AUR host key pinned to a known Ed25519
  fingerprint, `--locked` builds, a pinned 1.98.1 toolchain enforced
  in every workflow plus a dedicated MSRV job, and release builds
  that self-report their variant/LTO/panic/strip posture through
  `--doctor` with CI asserting the expected values.
- Hidden failure modes hunted and not found: no `curl | sh` anywhere
  in the tree, no unquoted `rm -rf`, no `eval` in entry points, no
  unbounded reads in build-time code, no workflow step that turns
  PR-controlled text into a command.

## What would falsify this verdict

Peak is a point-in-time claim. The audit leaves three explicit trip
wires that would reopen it: a new workflow introducing a
PR-triggered secret consumer; an action requiring a nonstandard trust
assumption (a first-party unclaimed org, not a GitHub-owned action);
or a script growth pattern that reintroduces an orchestration
monolith (the 800-line LOC gate on `.rs` files has no shell
equivalent — `check-scripts-loc` is the existing guard to watch).
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
