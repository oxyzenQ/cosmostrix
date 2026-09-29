<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-diner-11 — the stable-cut pre-flight: cosmostrix audited against zelynic's v11.0.0 generator incident, the exact v100.0.6 shapes run green locally, and the release-notes shape contract becomes gate 19

Owner directive (2026-09-30): "sekalian check commit ini di zelynic
biar kalo gue rilis cosmostrix stable version ngak kena batu lagi
6cb6f837e5d9b3f0b4a3177276d347c3a9076928" — the second zelynic
reference in the rc.1 recovery arc. The first (NIGHT-dinner-25,
commit 834aeb0) was the CI gate bug that left both v100.0.6-rc.1 tag
pipelines polling an already-green CI into their own 1800s timeout;
dinner-10 transplanted that fix. This one (zelynic NIGHT-dinner-26,
commit 6cb6f83) is the NEXT rock in the same lineage: zelynic's
v11.0.0 STABLE publish died eleven seconds into the Create GitHub
Release job because the release-notes generator's flag parser used
bash's `${2:?}` guard, which rejects a present-but-empty value as
hard as a missing one — and a stable release legitimately arrives
with `--since-stable ""` (LAST_STABLE == PREV_TAG by construction,
no distance to render). The whole rc series passed only because
pre-releases always carry a real since-stable distance: the bug was
invisible until the first stable cut switched the code path.

The question this record answers: does cosmostrix's upcoming
v100.0.6 stable cut face the same class of rock anywhere in its
release pipeline — a code path that only a stable cut lights up for
the first time?

## The audit: where the stable cut differs from the rc series

### The generator is structurally immune to the exact zelynic bug

cosmostrix's `scripts/release/generate-release-notes.sh` is a
different generation of the same tool: a POSITIONAL parser
(`<RANGE> <TAG> <IS_PRERELEASE> <PREV_TAG> [LAST_STABLE_TAG]`)
guarded by `$# -lt 4`, not a flag loop with `${2:?}` guards. A
present-but-empty positional argument cannot be rejected by that
construction — the existence check counts arguments, and the
workflow's call site passes every argument quoted
(`"${PREV_TAG}..${TAG}" "${TAG}" "${IS_PRERELEASE}" "${PREV_TAG}"
"${LAST_STABLE_TAG}"`), so empty values arrive as distinct empty
arguments exactly as the zelynic workflow intended its quoted flags
to. The two repos' generators diverged before the flag era; the
zelynic bug class cannot exist here without first re-introducing a
flag parser.

### The production evidence: five consecutive green stable cuts

The stable-only paths are not first-run code. v100.0.0 through
v100.0.5 (published 2026-09-14 through 2026-09-26) each ran the
full stable geometry end-to-end: the generator's stable branch
(PREV == LAST_STABLE, dual range collapsing to a single
"N commits since" line, [!TIP] alert), `make_latest: true`, the
"Trigger AUR sync" repository_dispatch, the whole aur.yml sync
(runs 51 through 56 — five green plus one transient-SSH retry), and
the crates.io stable publish. The most recent stable cut is four
days old at the time of this record.

### What actually changed since v100.0.5 (the real risk surface)

Diffing the release tooling between the v100.0.5 tag (b61b152, the
last green stable) and HEAD found the true delta the v100.0.6 cut
will fly with:

- The ci_gate job itself (added in 041b470, dinner-8 era) — v100.0.5
  was tagged BEFORE the gate existed. Its wait-for-ci.sh carried the
  dead-branch bug dinner-10 fixed at 4407852. Proof: the re-pointed
  rc.1 tag re-ran both tag pipelines through the FIXED gate
  (Guard - Release run 110, crates.io run 13, both completed
  success) — including the queued, in_progress, and
  completed-with-success poll states, the full build matrix, the
  GitHub Release publish, and the crates.io publish.
- NIGHT-dinner-1 retry hardening in the workflows (apt
  Acquire::Retries, rustup curl retries, the crates.io idempotency
  probe's curl retries) — all exercised green by those same
  rc.1 runs, except one step detailed below.
- The PKGBUILD's curl flag set gained `--retry-all-errors`
  (dinner-1). This file ships to real AUR users, and the next push
  is the v100.0.6 sync — the first AUR push carrying the new flag.

The two stable-only pieces that rc.1 could not exercise (its
"Trigger AUR sync" step is guarded `prerelease != 'true'`, and
aur.yml never runs for pre-releases):

- The AUR dispatch POST's new curl retry flags: a pure
  flag-addition on a POST that succeeded five consecutive times
  (runs 51-56 use the pre-hardening POST), and the workflow comment
  documents the retry as safe by design — a duplicate dispatch
  queues a second aur-sync run for the same tag, and aur.yml's
  per-tag concurrency group plus the idempotent AUR push make it a
  no-op.
- The PKGBUILD hardening: verified locally instead (below).

## The pre-flight: the exact v100.0.6 shapes, run locally

Following the zelynic dinner-26 method (verify every legal shape
through the real CLI path before the tag exists), both stable-only
surfaces were exercised with the exact v100.0.6 geometry:

### Generator — 4/4 shapes green

- The exact stable body: RANGE `v100.0.5..HEAD` with TAG v100.0.6,
  IS_PRERELEASE false, PREV == LAST_STABLE == v100.0.5 — exit 0,
  [!TIP] stable alert, single range line ("6 commits since
  v100.0.5"), six commit entries bucketed, no dual-range line.
- Pre-release regression shape: [!WARNING] alert, exit 0.
- Initial-release shape (the zelynic legal-empty class through the
  real CLI): `"" "${TAG}" "false" "" ""` — "Initial release."
  marker, exit 0. A present-but-empty positional argument is
  accepted exactly as the workflow's first-release call passes it.
- Usage error: a missing argument still fails with usage and exit 1
  (an EMPTY value is legal; a MISSING one is not — the same
  two-class contract zelynic had to write into its parser, already
  structural here).

### AUR sync — 9/9 checks green

aur.yml's resolve, Update PKGBUILD, and Regenerate .SRCINFO steps
were reproduced verbatim (no network, no SSH) against the current
repo PKGBUILD with TAG=v100.0.6: pkgver resolves to 100.0.6
(hyphen stripped by `${VERSION%%-*}` — the template's current
`pkgver=100.0.6-rc.1` state is owner-convention, never reaches AUR,
and the sed rewrites it regardless), `_tag=` stays the legal-empty
stable shape, bash -n clean on the mutated PKGBUILD, .SRCINFO
regeneration validates pkgver=100.0.6, prepare() constructs
tag=v100.0.6 (correct asset URLs:
cosmostrix-v100.0.6-linux-amd64-v3.tar.gz and friends), and curl
8.14.1 accepts the full hardened flag set (exit 7 connection class
against a closed port, not an option error).

## The gap this audit actually found — and closed

The audit found no rock in the pipeline. It found one in the
testing: the generator had ZERO automated coverage. Every release
body since the generator existed was produced by a script no gate
ever executed — a regression in its parser or render guards would
surface exactly the way zelynic's did, at the next tag, in the
publish job, with the pipeline already committed to the release.
That is the same coverage gap the zelynic incident fell through
("the old shape probe called render_body directly and never touched
the parser"), and it is now closed the same way:

**Gate 19 — `scripts/gates/check-release-notes-shapes.sh`**
(release-notes shape contract): re-executes the generator through
its real positional CLI against a hermetic throwaway git repository
(four synthetic commits in the repo's own subject conventions, four
tags) and asserts the documented decision table — stable
single-range (and NO dual-range line for PREV == LAST_STABLE),
pre-release WARNING alert, dual range rendering BOTH distances,
initial-release legal-empty through the real CLI, the
missing-argument usage error, and the classifier plumbing producing
bucketed achievements. Hermetic by necessity: the CI gate-keepers
checkout is depth-1 with no tags, so the probe must carry its own
history. The gate battery reports 22 passed / 0 failed with the new
check wired into gate-keepers.sh and the cosmic-dragon-guard.yml
check list.

One probe-authoring lesson recorded for the next gate writer: the
first draft piped the generator straight into `grep -q` under
`set -o pipefail` — grep -q exits at first match, the generator
takes EPIPE mid-write, and pipefail turns a FOUND match into a
false failure. The gate now captures output first and greps the
capture.

## Verdict and the owner's release checklist

The v100.0.6 stable cut flies a proven pipeline: every stable-only
code path has production-green runs (five consecutive stables,
latest four days ago), every change since the last green stable is
proven by the re-triggered rc.1 runs or verified by this pre-flight,
and the generator's shape contract is now enforced on every push to
main instead of at the next tag. No zelynic-class rock exists in
the stable path.

The cut itself remains the owner's five-file pattern, unchanged
from b61b152 and d959449: Cargo.toml, Cargo.lock, README.md's TAG
line, aur/cosmostrix-bin/PKGBUILD's pkgver, aur .SRCINFO's pkgver —
tag v100.0.6 on the release commit, push main and the tag together
(the ci_gate exists precisely for that collision), and the pipeline
does the rest. The hyphenated template pkgver is a non-issue: aur.yml
strips the suffix into `_tag` semantics before anything reaches AUR.

Test-infrastructure and docs only: zero Rust source, zero shipped
bytes, zero engine surface — no benchmark applies.
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
