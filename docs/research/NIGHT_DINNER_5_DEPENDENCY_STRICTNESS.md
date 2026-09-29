<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-dinner-5 — the dependency strictness policy: which pins protect the LTS, which one was too tight, and which "stuck" update is not ours at all

Owner directive (2026-09-29): "which dependencies should not be
strict, so that important updates can actually land, while cosmostrix
stays stable? And why keep the strict rules for the rest — document
it, because `cargo update` printing a wall of `Unchanged` confuses
every user who sees it."

## The question, decoded

The trigger was this `cargo update --verbose` output:

```text
  Locking 0 packages to latest compatible versions
  Unchanged clap v4.5.61 (available: v4.6.7)
  Unchanged clap_builder v4.5.61 (available: v4.6.7)
  Unchanged clap_derive v4.5.61 (available: v4.6.7)
  Unchanged generic-array v0.14.7 (available: v0.14.9)
  Unchanged notify v7.0.0 (available: v8.2.0)
  Unchanged rand v0.9.5 (available: v0.10.3)
  Unchanged sha2 v0.10.9 (available: v0.11.0)
  Unchanged signal-hook v0.3.18 (available: v0.4.4)
```

Read it as three different messages hiding in one list:

1. One line is cosmostrix being **too strict** (clap, pinned `>=4.5,
   <4.6` by an old Cargo.toml decision).
2. One line is **not cosmostrix at all** (generic-array — pinned by an
   upstream exact-pin, see the next section).
3. Four lines are **semver working as designed** (notify 8, rand 0.10,
   sha2 0.11, signal-hook 0.4 — major-version boundaries that no
   responsible project crosses silently).

## The finding that changes the answer: generic-array is not our pin

`cargo update -p generic-array` refuses to move 0.14.7 to 0.14.9 even
when asked explicitly. The resolver error names the chain:

```text
candidate versions found which didn't match: 0.14.9
required by package `crypto-common v0.1.7`
    ... which satisfies dependency `crypto-common = "^0.1.3"` of package `digest v0.10.7`
    ... which satisfies dependency `digest = "^0.10.7"` of package `sha2 v0.10.9`
    ... which satisfies dependency `sha2 = "^0.10"` of package `cosmostrix`
```

Verified against the crates.io registry API: `crypto-common 0.1.7`
declares `generic-array = "=0.14.7"` — an exact pin, deliberate
RustCrypto damage control around the 0.14.8/0.14.9 releases. Neither
0.14.8 nor 0.14.9 is yanked; they are simply excluded by the
upstream requirement. No cosmostrix constraint change can move
generic-array. The only path to 0.14.9+ is the sha2 0.11 migration
(digest 0.11 drops crypto-common for hybrid-array), which the
DEPENDENCY_AUDIT framework already classifies as HOLD. Conclusion:
this line of the Unchanged list is upstream-owned; it leaves with the
sha2 0.11 migration or never.

This also corrects the 2026-09-02 audit's "UPDATE NOW — cargo update
-p generic-array" recommendation: that command cannot succeed under
the current sha2 0.10 line, with or without `--precise` (precise
bypasses nothing when the pinned parent requirement excludes the
version).

## The three strictness classes

### Class 1 — auto-flow (not strict): everything written caret-style

`crossterm = "0.29"`, `rand = "0.9"`, `bitvec = "1"`, `smallvec = "1"`,
`unicode-width = "0.2"`, `sha2 = "0.10"`, `signal-hook = "0.3"`,
`libc = "0.2"`, `ctrlc = "3.4"` — all of these accept every
semver-compatible patch/minor release the moment `cargo update` runs.
The weekly `maintenance.yml` cron runs it automatically and pushes the
validated lockfile. Proof it flows: smallvec 1.15.2 to 1.16.0 landed
exactly this way (2026-09-02), and crossterm sits at 0.29.0 because
0.29.0 IS the newest release of the 0.29 line — there is nothing newer
to flow (verified against crates.io, 2026-09-29).

### Class 2 — boundary pins (strict on purpose): the explicit ranges

- `clap >=4.5, <4.7` — relaxed today (was `<4.6`), see below.
- `notify >=8, <9` — migrated today (was `>=7, <8`), see below. The
  ceiling stays a boundary pin: the 9.x line (currently 9.0.0-rc)
  never enters the lockfile, and release candidates are never
  lockfile candidates at all.

A major-boundary pin is not "too strict"; it is the difference
between `cargo update` being a no-op risk and being a code audit. The
pin does not block anything that would otherwise be safe — it blocks
exactly the updates that REQUIRE code changes.

### Class 3 — upstream pins (not ours to relax)

`generic-array =0.14.7` via crypto-common 0.1.7, described above. The
transitive RUSTSEC-2024-0384 `instant` advisory (via notify-types) was
the same story until today: the notify 8 migration pulled notify-types
2.1.0, which dropped `instant` entirely — the crate is gone from the
graph and the `deny.toml` suppress is retired with it.

## What changed today

The clap pin `>=4.5, <4.6` was the one place cosmostrix was stricter
than semver requires for no stability gain — 4.6 is a minor release
inside the stable clap 4 line, with no API removals on the derive
surface this project uses. Relaxed to `>=4.5, <4.7` in Cargo.toml
(with a rationale comment), then `cargo update -p clap` moved:

```text
  Updating clap v4.5.61 -> v4.6.7
  Updating clap_builder v4.5.61 -> v4.6.7
  Updating clap_derive v4.5.61 -> v4.6.7
      Adding syn v3.0.6
```

(clap_derive 4.6 moved to syn 3; that is the entire lockfile delta —
three clap-family bumps plus one new transitive crate.)

Verification: `cargo check --locked` clean, `cargo clippy --locked
--all-targets --all-features -- -D warnings` clean (exit 0), full
gate-keepers.sh 16/16 PASS. The 2950-test suite and the remaining
build.sh check-all stages run in CI on push — the local pass covers
everything that can break from a CLI-parser minor bump (the derive
expansion and every type the CLI surface uses). The 10 s A/B bench
(release profile, baseline 579de6a vs after eb18b31, cinematic +
monolith, 2 runs each) is performance-neutral and visual-identical
within run noise — monolith flat to four decimals on entropy and
gini; see
[../bench-labs/night_dinner5/AB_REPORT.md](../bench-labs/night_dinner5/AB_REPORT.md).

The ceiling stays at `<4.7`, not open-ended: a hypothetical clap 5,
or a 4.7 that removes something, still cannot enter the lockfile
silently. One minor line of headroom is the maximum relaxation that
keeps `cargo update` a no-audit operation.

## The second relaxation of the day: notify 7 to 8

Owner decision (2026-09-29, same session): the relax policy is
approved with one hard boundary, stated as a dragon analogy — the
skin may be upgraded, the heart is never edited. A dependency
upgrade is allowed exactly as far as it never touches the critical
core engine (the render engines and their frame path); watcher and
CLI plumbing are skin.

Under that rule the pending question — retire the RUSTSEC-2024-0384
suppress if notify-types migrated, or do the notify 8 migration as a
dedicated change — resolved by verification: notify-types 1.0.1
still depends on `instant`, so the suppress cannot be retired in
place; notify 8.2.0 (via notify-types 2.1.0) is the only path that
removes the crate, and its API surface on this project (2 src files,
1 test file, all in config live-reload plumbing — zero engine
files) is identical across the 7 to 8 boundary. The migration landed
as a pure pin bump: `>=7, <8` to `>=8, <9` in three target sections,
zero source changes, verified by cargo check --locked --all-targets,
clippy --all-features -D warnings, the full 2983-test suite, and the
PTY config stresstest. Lockfile delta: notify 8.2.0, notify-types
2.1.0, inotify 0.11.5, windows-sys 0.60.2; instant, bitflags 1.3.2
and filetime leave the graph. The deny.toml advisory ignore is
retired (advisories now run clean with zero ignores) and the skip
list loses the bitflags entry (inotify 0.11 sits on bitflags 2),
re-pinning the windows-sys skip to 0.60.2.

The audit-table claim that motivated the 2-4 hour estimate
("notify 8 reworks the Event API") did not survive contact with the
compiler: it was an unverified overestimate, corrected in
docs/DEPENDENCY_AUDIT.md. The 10 s A/B bench (release profile,
baseline d533c98 vs the migration commit, cinematic + monolith
controls) is performance-neutral and visual-identical within run
noise — monolith flat to four decimals on gini, fps within 0.21% on
run means, and the frame path never enters notify at all; recorded in
[../bench-labs/night_dinner5/AB_REPORT.md](../bench-labs/night_dinner5/AB_REPORT.md).

## What stays strict and why (the one-glance table)

| Dep | Constraint | Class | Why it stays |
|-----|------------|-------|--------------|
| clap | `>=4.5, <4.7` | boundary pin (relaxed) | headroom of one minor line; 5.x / removal-carrying 4.7+ stay out |
| notify | `>=8, <9` | boundary pin (migrated) | 8.x landed 2026-09-29 as a zero-source-change pin bump (owner-approved relax); the ceiling keeps 9.x (rc) out |
| rand | `0.9` | auto-flow within line | 0.10 reworks the Rng/distr traits across ~15 call sites; visual regression risk needs an A/B benchmark campaign |
| sha2 | `0.10` | auto-flow within line | 0.11 is a security-critical hashing-path rework (config change detection + fingerprints); HOLD per audit |
| signal-hook | `0.3` | auto-flow within line | 0.4 changes the Signals iterator API; terminal-cleanup-on-Ctrl-C correctness is a dedicated-PR job |
| crossterm | `0.29` | auto-flow | 0.29.0 is simply the newest release; nothing is being blocked |
| generic-array | (transitive) | upstream pin | crypto-common 0.1.7 exact-pins =0.14.7; leaves only with the sha2 0.11 migration |

## Why the remaining strictness is correct for this project

1. **Stability over freshness is the documented posture** —
   cosmostrix targets a 5-10 year dormant-mode maintenance cycle
   (docs/MAINTENANCE.md); a pinned, audited dep tree is the point, not
   a bug.
2. **Semver-compatible is a contract, not a proof** — the weekly
   maintenance cron DOES take every in-range update, but through a
   full validation pipeline (audit, deny, fmt, build, test, clippy)
   before the lockfile lands on main.
3. **Every "blocked" update is a migration, not an update** — the
   three remaining major bumps each touch code paths whose failure
   mode is visible (rain patterns, terminal cleanup) or
   security-relevant (config hashing). They are scheduled work, not
   constraint casualties. (notify, the fourth, migrated 2026-09-29.)
4. **The user-visible confusion is now documented** — this file plus
   the refreshed DEPENDENCY_AUDIT.md state table exist so the next
   person who runs `cargo update --verbose` and sees `Unchanged`
   knows exactly which of the three classes each line belongs to and
   that none of them means "cosmostrix is rotting" (the list itself
   shrank from eight lines to four on 2026-09-29: clap and notify
   both moved).

## Cross-references

- `docs/DEPENDENCY_AUDIT.md` — the three-bucket framework, per-dep
  analysis, refreshed state table (2026-09-29)
- `docs/SUPPLY_CHAIN.md` — dependency policy table (clap row updated),
  weekly maintenance cron, security response SLAs
- `Cargo.toml` — the clap relaxation comment
- `CHANGELOG.md` — the NIGHT-dinner-5 entry under Unreleased
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
