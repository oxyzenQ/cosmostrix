<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Night Dragon Maintenance 7 — Every Seat Installs Its Own Tools

Owner: oxyzenQ | Session: night-dragon-maintenance-7 | Date: 2026-10-28
Reference: github.com/oxyzenQ/zelynic commit 871b028 (the sibling fix this
session ports), github.com/oxyzenQ/zejtron (the gate's original verdict law)

## The field report

zelynic's crates.io publish job died in fourteen seconds on tag
v80.0.1-night.1. The "Prebuilt eBPF parity gate" failed in under a second
while the SAME check sat green inside the wholesale gate-keepers run on
the same SHA — the ci_gate had just waited for that wholesale run
successfully. Same tree, same script, opposite verdicts; the difference
was the seat, not the tree. The parity script fail-closes on a missing
b3sum (night-dragon-maintenance-0's law in this repo: a digest that
silently skips is a digest that does not exist), and the publish lane ran
the gate OUTSIDE the wholesale seat — gate-keepers.yml installs the tool,
release.yml installs it too, crates-io.yml was the one lane that forgot.
The runner image's apt set had moved on since the step landed; the first
tag to carry the gate stepped on the gap.

The owner's ask after the fix landed there: make the class stop happening
in cosmostrix too. This document is the census and the cuts.

## The class, named

The zejtron gate's header law: no gate can rot silently behind a missing
CI mirror step. The class has two failure surfaces with one root. Surface
(a): a seat outside the wholesale lane runs a gate whose tools it never
installs, and the first runner-image drift steps on the gap. Surface (b):
a verdict that counts only passes and failures absorbs a skipped gate
into silence, so a partial run reads green — the zelynic
night-dragon-maintenance-9 false green (markdownlint absent locally,
seventeen violations counted only in CI) was exactly that shape. A fix
that only plugs surface (a) leaves the verdict able to hide the next
occurrence; a fix that only plugs surface (b) leaves every seat still
trusting the image. Both surfaces get a cut.

## The cosmostrix census

Sweep of all twelve workflows under .github/workflows/ for direct callers
of scripts/gates/*, scripts/gate-keepers.sh, and scripts/ci/*:

| Seat | What it runs outside the wholesale lane | Verdict |
|------|------------------------------------------|---------|
| cosmic-dragon-guard.yml | the wholesale seat itself: gate-keepers.sh, every pinned tool installed in-step | no gap; the header law already lives here (the shfmt-skipped-v50-line incident is on record in its comments) |
| ci.yml version-sync job | build.sh version-sync, check-version-anti-patterns.sh, check-rust-version-sync.sh | pure bash + repo files; bash is the runner's own shell and cannot drift off the image; the job comment states the contract; no gap |
| ci.yml build jobs | ci-strict-build.sh (cargo wrapper) | the toolchain steps in the same jobs install what they run; no gap |
| crates-io.yml ci_gate | scripts/ci/wait-for-ci.sh (needs curl + jq) | GAP: the seat trusted runner staples, exactly the zelynic shape |
| release.yml ci_gate | scripts/ci/wait-for-ci.sh (needs curl + jq) | GAP: same shape, same fix |
| aur / codeql / fuzz / maintenance / miri / supermassive / supermassive-kernel / gitbot-audit | no repo-script invocations | nothing to provision; no local composite actions exist (sweep-verified) |

Adjacent finding, recorded and left to owner law: release.yml's checksum
step installs b3sum with a guarded cargo install and downgrades a failed
install to a ::warning::, with the sha512 sidecar (the mandatory digest
of night-dragon-maintenance-0) still generated. That is the documented
checksum policy of this repo, not a gate lane; it is named here so the
next sweep does not re-litigate it as an unknown.

## The cuts

### Cut 1 — the verdict stopped lying (scripts/gate-keepers.sh)

The zejtron/zelynic summary shape, ported: a SKIP counter beside PASS and
FAIL, a skip() helper that tallies and records, a FAILED_SECTIONS array
fed by fail(). The verdict line now prints passed / failed / skipped;
skipped gates are listed by name with their install hints; failed
sections are listed by name under COMMIT BLOCKED. Exit codes are
unchanged: the local seat stays usable on minimal machines, the CI
wholesale seat (which installs every pinned tool) stays the enforcer.
A partial run can no longer masquerade as a full one — the verdict that
let the zelynic maintenance-9 false green sleep cannot exist here.

Twenty-five silent-skip sites converted. Each now carries its install
hint (apt, pip, npm, or go install for the external tools) or names the
tree fault (a gate script missing from the repo is a different disease
than a tool missing from the seat, and the summary now says which).

### Cut 2 — the ci-gate seats install their own tools (crates-io.yml, release.yml)

Both ci_gate jobs carry the identical install step, verbatim: a guarded
apt install of jq (with the same Acquire::Retries contract every other
apt step in this repo rides) before the wait step. The wholesale lane was
never the problem; the seats outside it were, and this is the publish
and release lanes paying their own bill instead of trusting the image.

### Cut 3 — wait-for-ci.sh fail-closes on a missing tool

A tool census at the top of the script: a missing curl or jq stops the
seat immediately with ::error:: lines naming the tool and its install
hint. Before this cut, a missing jq died inside newest_run under
set -euo pipefail as a bare "command not found", and — the worse
branch — a jq failure inside classify_failed_run was absorbed by its
`|| jobs=""` guard and read as "failed with no failed jobs", an infra
verdict that would have burned the automatic re-run budget on a wrong
diagnosis of GitHub's side. A tool this gate needs that is absent now
stops the seat HERE, named.

## Proof runs

- Stripped seat (shellcheck, shfmt, yamllint, actionlint, codespell,
  ruff all absent): "18 passed, 0 failed, 6 skipped", all six skipped
  gates named with install hints; exit code 0 unchanged.
- The same seat carrying one real violation (the permissions guard on a
  fresh clone's umask drift): "18 passed, 1 failed, 6 skipped" with the
  failed section named in the summary.
- bash -n clean on both edited scripts.
- The wholesale claim is now checkable: cosmic-dragon-guard installs
  every pinned tool, so its run must print "0 skipped" — a visible,
  checkable zero on every push instead of an assumption.

## Release law notes

No version bump, no tag: owner decides. Maintenance-law commit only; the
next release cut carries it.

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
