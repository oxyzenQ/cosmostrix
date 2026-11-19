<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# Depth hardening 5: the pinned digest survives the Z9 delta, the five-area cross-check re-proven at the new head (NIGHT-depth-hardening-5)

Date: 2026-11-19
Scope: the owner's five-area total-LTS cross-check over the root repo,
continuing NIGHT-depth-hardening-4 with one sharpened target:
deterministic output on every Linux distribution at kernel 5.15 and
above, re-MEASURED at the new head rather than assumed. The delta since
the last audit is twenty-eight commits (night-frontier-Z5a/b/c, Z6
S-series, the Z9 program: entries 55-58) touching 83 files, +9105/-378
lines, including the sharpest surface in the tree - the emission path
itself (night-frontier-Z9-C moved the diff solver into
`emit_diff.rs`). Audit baseline: 258a415 (HEAD of main). Method:
cross-commit digest A/B against a purpose-built worktree at the
pre-delta commit, the hostile torture battery at HEAD, the full
28-check gatekeeper suite, a panic/unsafe census of every added line,
and the structural audit of the new certificate seam and its ledgers.

## Verdict

**All five areas are at peak at HEAD 258a415. Zero code changes in
this cycle. The determinism claim gained its strongest evidence yet:
the pinned replay digest survived the entire twenty-eight-commit delta
byte-for-byte, including the Z9-C extraction of the shipped solver.**

## Area 1 - Stability and crash: at peak

- Hostile battery at this SHA: `torture.py` 7/7 legs PASS against the
  debug build (the battery itself grew with the delta: Z5a added the
  paste-wedge leg and four restore-tail assertions - alt-leave,
  mouse-off, paste-off, focus-off - and all pass). Legs: resize-storm,
  paste-wedge, signal-storm (SIGINT default-death rc=-2, SIGTERM
  graceful rc=0), config-churn (22 edits), restart-storm (12/12 clean),
  suspend-resume (silenced stream, resumed repaint, deadline exit).
- Panic census of the DELTA: every added line in `src/` (+9105 across
  28 commits) scanned for `unwrap()`/`.expect(`/`panic!`/`unsafe`:
  five hits, all the same Z5c documentation comment. The delta added
  zero panic surface and zero unsafe surface to production code.
- CI corroboration at the same SHA: Miri, Kani, Loom, cargo-fuzz,
  Supermassive (with the new Z9-A wakeup-floor and Z9-C emission-floor
  legs), Supermassive Kernel, CodeQL and the soak lane all green.
- Visual/performance regression risk of this audit cycle: zero engine
  code touched; the empirical no-regression proof is the digest A/B
  below. No A/B benchmark required (no binary change).

## Area 2 - Code hygiene: at peak

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
  clean at this SHA (check-all rc=0 inside the two-minute law).
- Dragon-hunt scope: perstage method, the deepest read reserved for
  the delta's engine core (the Z9-C seam, area 5 below). The rest of
  the delta is pinned-surface code: the new ledgers are enforced by
  their own census gates (unsafe-census pins 61 unsafe lines across 17
  files; lock-census pins the 121-crate graph; scope-census pins the
  12+2 dependency set), so zombie drift on those surfaces cannot
  compile past CI silently.
- No stale references: stale-hunt.py and docs-audit.py PASS at HEAD
  (the Z9 suite-count sync commit 92aa69c kept the ledger text equal
  to the tree).

## Area 3 - Optimization: at peak, skipped by rule

- The floors that matter did not stand still: night-frontier-Z9-A
  (entry 56) deepened the syscall floor into a whole-frame budget -
  steady state is now pinned at one timer wake plus one write per
  frame, measured on a real pty by `wakeup_budget.py`, and the
  emission floor (entry 57's leg) accounts every byte class of the
  stream. Both are enforced, not hoped for.
- No profile shows a hot spot in the delta: the Z9-C extraction is
  byte-identical in behavior (digest A/B) and the Z8 conservation
  ledgers are test-side. Per the owner's peak-skip rule: no change.
  The correct next optimization step remains profile-first.

## Area 4 - Security hardening (white hat): at peak

- The Z5c law is compiler-enforced at HEAD: the render core carries
  `forbid(unsafe_code)` (unsafe-census verifies "the engine render
  core is compiler-forbidden"), and Cargo lints pin
  `undocumented_unsafe_blocks = "deny"` so every remaining unsafe
  block must carry a SAFETY justification.
- Z6 hardened the supply chain with teeth: `deny.toml` gained
  `yanked = "deny"` (the 2026-08-20 arrayref malicious republish must
  never re-enter a build) and wildcard requirements are denied (no
  `*` version delegation), both enforced by the Security Audit lane.
- Locale determinism: zero `setlocale` calls in `src/` - Rust does not
  locale-format numbers, so the output law has no locale dependency by
  construction. Timestamps ride the UTC law.
- Environment parsing and the escape single-sink law: unchanged since
  hardening-4's census; no delta line touched either.

## Area 5 - LTS stability and determinism: at peak, re-measured

Everything already proven and unchanged (million-year ladder, full
termios suspend/resume, single-threaded-by-law scaling, zero-alloc
floor). What this cycle adds is measurement at the new head:

1. **The cross-commit digest A/B (the sharpened target).** A worktree
   at the pre-delta commit 1565607 was built with the same pinned
   toolchain (1.98.1) as HEAD and both binaries ran the replay oracle
   twice. All four digests are identical:
   `b3:7e938efa5efcb659dc214a9bf1fc84afdbbe8d6b8e0aab9f67b0b3606f1578d7`
   - the same value NIGHT-depth-hardening-4 recorded. The pinned
   sequence survived the paste fuse, the temporal colorimetry law, the
   no-unsafe law, the S-series ledgers and the entire Z9 program
   byte-for-byte. "The same replay sequence on every Linux distro at
   kernel 5.15+" is a measured fact at the new head, not an inherited
   claim.
2. **The cross-libc gate is alive at HEAD**: ci.yml still publishes
   the debug (glibc) digest and the reproducible-verify lane compares
   it against the static musl release binary's digest on a second
   host. Two libcs, two hosts, one digest, every push.
3. **The distro floor is structural**: the published Linux x86_64
   shapes are the fully static musl pair (v3/v4, self-contained link -
   the gnu cross-compiler step is retired), so the artifact runs on
   any distribution at kernel 5.15+ regardless of the target's glibc;
   there is no glibc symbol floor to inherit from the builder.
4. **No kernel-version traps in the runtime surface**: `src/` contains
   zero cgroup parsing (the sysstat family reads /proc only, so the
   cgroup v1/v2 split across 5.15-era distros cannot perturb it), and
   the config watcher's inotify dependency predates every kernel in
   scope by a decade.
5. **Endurance anchors deepened**: the soak lane (Z6-S5) now anchors a
   long-lived process's replay digest at t=0 and t=window with the
   /proc census closing the leak case; entry 58 adds the in-process
   state-digest canary the soaked process re-derives itself. The 24h
   horizon is honest: 150-minute hosted legs, 1440 only via
   self-hosted dispatch.

## Skipped follow-ups and why

- Renumbering the family-scoped INV labels (chroma's INV-20/21/22
  ladder versus the incubator family's INV-19/20/21/22): the ledger
  itself documents the convention (entry 50: "the chroma suite's own
  INV-20 riding the same filter") and the `lock_invNN` test filters
  deliberately span both families; renumbering four suites and every
  reference would churn the tree against the LTS law for zero
  behavioral gain.
- Replacing f32 libm calls with in-tree fixed-point math: unchanged
  from hardening-4's skip rationale - the correctly-rounded law plus
  the measured cross-libc gate already pins the output.
- Extending the digest comparison to the macOS aarch64 lane: outside
  the stated Linux 5.15+ scope (hardening-4's rationale stands).

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
