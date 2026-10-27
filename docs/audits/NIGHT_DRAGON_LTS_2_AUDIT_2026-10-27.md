<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-dragon-lts-2 audit — the all-infra depth pass: every workflow (including the three the last audit never saw), the scripts tree, and the build pipeline re-verified

Owner brief (2026-10-27): "night-dragon-lts-2. depth audit focus for
all infra. ultimate total lts, realiable all round and keep be honest.
cross check audit all root repo focus, if peak skip and continue the
next step" — with the five-dimension rubric and the standing rule: if
a sub-task is already at peak, skip and note why.

## Audit scope and method

The 2026-10-11 night-total-lts-2 report closed the then-existing 9
workflows, the scripts tree, and build.rs at peak. Since then the
estate grew to 12 workflows, and the three NEW ones —
`supermassive-kernel.yml` (the KVM micro-VM kernel lanes),
`supermassive.yml`, and `cosmic-dragon-guard.yml` — were written AFTER
that verdict and audited by no one. They also produced the two CI
failures fixed earlier this session. Method: full reads of all three
new workflows (675 lines for supermassive-kernel alone) and all four
`scripts/sandbox/*` files; attack-surface greps across all 12
workflows and 35 shell scripts; a build.rs delta re-verification
against the 2026-10-11 claims; TLS reachability verified live before
the one change below.

## What changed because of this audit (1 commit, pushed)

- `10776c2` — the floor lane fetched the intrepid GA kernel deb and
  the era busybox-initramfs deb over plain `http://old-releases.ubuntu.com`,
  then unpacked them with `dpkg -x` (which bypasses apt's entire GPG
  Release-signature chain) and booted the result inside the micro-VM.
  Any on-path or DNS-level attacker could substitute the payload and
  forge the `SANDBOX-VERDICT: PASS` the lane exists to produce — the
  verdict was silently forgeable. The best lane already pulled its
  mainline kernel over https; the floor lane now matches (4 URL sites
  switched), with the host's TLS endpoints verified live (HTTP 200 on
  both the pool and the dists index) before the switch. YAML-validated
  locally; the lane re-proves itself on CI.

## The five rubric dimensions

### 1. Stability and crash — one real finding (fixed above); the rest of the new-lane surface is disciplined

The three new workflows carry the full estate discipline: every job
has `timeout-minutes` plus layered inner budgets (`timeout 600/900`
around qemu, `--budget` pty watchdogs narrower than the job timeout),
concurrency groups with `cancel-in-progress`, `contents: read` with no
job-level escalation, and fail-closed verdict steps (`grep -q
'^SANDBOX-VERDICT: PASS'`; any `FAIL` line exits 1). The guest-side
PID-1 init (`scripts/sandbox/microvm-init.sh`) never plain-exits,
validates the lane file against a whitelist, and mounts its own tmpfs.
Follow-up recommendations (owner's call, not fixed): verify the
mainline deb against kernel.ubuntu.com's per-release SHA256SUMS after
download (transport is already TLS, so this is defense-in-depth, LOW);
and pin the busybox-initramfs sha256 — the pinned era deb is immutable
by the archive's own contract. Residuals deliberately left (LOW): the
`chmod 666 /dev/kvm` host step is the community-standard pattern on
ephemeral single-tenant GitHub runners (a `usermod -aG kvm` variant
would be tighter); the hardcoded `/tmp/kfloor`/`/tmp/kbest` working
dirs deviate from the scripts tree's own mktemp discipline but sit on
an ephemeral single-tenant runner where the preplant race has no
attacker — fixing it means restructuring cross-step state in the
just-greened lane YAML, churn that outweighs a nil threat. SKIP
RATIONALE recorded for both.

### 2. Code hygiene — the scripts tree and the new lanes are clean

Greps across all 35 shell scripts: zero `eval`, zero pipe-to-shell
downloads, zero raw `/tmp` paths (every temp file is an `mktemp`
template), zero world-writable host redirects (the only `chmod 666`
in the tree is the documented in-guest tmpfs one, deliberately scoped
and commented in `best-payload.sh`). All four `scripts/sandbox/*`
files read in full: no eval, no network, no host-side effect, fail
LOUDLY (not silently) on degradation. The new workflows follow the
estate's action-pinning convention exactly (major-tag pins, zero
`@main` refs — SHA-pinning is an estate-wide decision the owner owns,
see rubric 5). No orphaned or zombie files were found in the infra
tree. At peak; nothing to remove.

### 3. Optimization — not applicable by domain (peak by vacuity)

CI lanes and build plumbing are not performance surfaces; the
engine-level hot paths belong to night-dragon-lts-1, which closed them
with a benchmark A/B showing parity. The one perf-adjacent infra
design — layered budgets sized narrower than job timeouts so a hung
leg fails in minutes, not after the full job — is already optimal for
its purpose (it is exactly what surfaced the 19-minute floor hang as a
598-second stall with zero output, making root-causing possible).

### 4. Security hardening (white-hat pass) — the estate walls hold; one policy deviation slot noted

Cross-workflow evidence table: `pull_request_target` — 0/12 workflows
(the fork-PR secret-exfiltration vector does not exist here);
`continue-on-error: true` — 0/12 (no silently-tolerated gates); named
secrets appear only in `aur.yml` (dispatch-only trigger), `crates-io.yml`
(tag-push gated), `release.yml` (tag-push gated) — never combined with
a `pull_request` trigger; `curl | sh` — 0 hits estate-wide; `${{ }}`
interpolations into shell in the three new workflows are limited to
`github.event_name` / `run_id` / `run_attempt` (enum/numeric — no PR
title, no branch name, no attacker-controlled payload reaches a shell);
cache keys derive from Cargo.lock and run metadata only, and the new
lanes use no upload/download-artifact at all (artifact roundtrips
exist only in previously-audited, trigger-gated workflows). `build.rs`
re-verified against the 2026-10-11 claims: exactly two `Command::new`
calls (`git rev-parse` with hex-validated output, `rustc --version`),
zero network code, env-var inputs pass through whitelisting
normalizers with safe defaults. Noted, not fixed — SKIP RATIONALE:
`cosmic-dragon-guard.yml` downloads the shfmt release binary without a
checksum pin, but that slot is EXPLICITLY the documented owner policy
("Unpinned on purpose, owner policy 2026-08-30" — no hardcoded dep
versions in `.github/*`), transport is TLS with a retry contract, and
the step's only secret reach is the ephemeral read-only token. A
sha256 pin is a cheap hardening the owner can adopt without changing
the policy; it is not this audit's call to make.

### 5. LTS stability — the delta is now audited to the same depth as the estate

Every structural property the 2026-10-11 verdict relied on was
re-verified against the 2026-10-27 tree, not assumed: permissions
blocks (12/12), timeouts and concurrency (12/12), secrets surface
(unchanged and gated), build.rs (claims hold). The three new lanes
were the one honest gap in the estate's audit coverage — a 30-minute
KVM lane pair running root-inside-a-guest with live-internet resolvers
— and this pass closes it. Long-horizon risks that remain are
deliberate, documented owner postures: unfiltered push triggers on
every lane (the 2026-09-13 simplification — every push burns the KVM
minutes by design), tag-pinned actions estate-wide (drift risk owned
by the gate #20 pin-health checker, advisory by default), and live
resolvers that fail loudly with evidence rather than pinning versions
(which would violate the no-hardcoded-deps policy). None is a hidden
failure mode; each is a decision with a name and a date on it.

## What would falsify this verdict

- Any workflow gaining a `pull_request` trigger while touching a named
  secret re-opens the exfiltration wall — the grep is one line, run it.
- The first workflow that interpolates `github.event.*` payload content
  (PR title, branch name, issue body) into a `run:` block breaks the
  injection-clean claim.
- A move to self-hosted or persistent runners invalidates the LOW
  residuals this audit accepted (666 KVM device mode, hardcoded /tmp
  lane dirs) — on a multi-tenant host they become real attack surface
  and must be fixed before the first non-ephemeral job runs.

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
