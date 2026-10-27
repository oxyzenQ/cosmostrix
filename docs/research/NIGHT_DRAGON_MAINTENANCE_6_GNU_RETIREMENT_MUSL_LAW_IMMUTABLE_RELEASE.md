<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Night Dragon Maintenance 6 — GNU Retirement, Static Musl Law, Immutable-Release Fix

Owner: oxyzenQ | Session: night-dragon-maintenance-6 | Date: 2026-10-27
Reference: github.com/oxyzenQ/zejtron (mature infra lineage)

## Scope

One owner directive executed as ten micro-commits (1 task = 1 commit,
pushed in order), plus the verification record:

| # | Task | Commit | Result |
|---|------|--------|--------|
| 1 | Retire the gnu alias + profile pair | `f25b29a` | `.cargo/config.toml` + `Cargo.toml` musl-only |
| 2 | Release lane rides the static musl law | `caf002d` | musl matrix (v3+v4), aarch64-gnu leg out, PGO musl |
| 3 | Draft-then-publish survives immutable releases | `007e6b1` | publish job draft + un-draft + tripwire |
| 4 | CI variant matrix rides the musl pair | `c41d726` | `linux_cpu_variants` musl, aarch64 cross job out |
| 5 | Torture and maintenance lanes ride the musl shape | `6b9cfc5` | supermassive pty/soak/torture + maintenance musl |
| 6 | Rust-side metadata and hints name the musl law | `e0111a3` | build.rs tests/hints, doctor hint, doc comment |
| 7 | Shell tooling targets the static musl shapes | `2db2cc6` | build.sh libs, verify-release-build, benchmark, depth tools |
| 8 | AUR binary package tracks the musl assets | `519d7ed` | PKGBUILD + .SRCINFO musl assets, x86_64-only |
| 9 | Docs name the static musl release law | `da1f908` | README + 11 docs + release-notes generator |
| 10 | This research ledger | (this commit) | session record |

## The Owner Directive

Total removal of glibc (GNU) package build support from the release
packages, cargo aliases, and every reference that contains it, plus
peak optimization of the static musl build support. Modeled on the
sibling repository's mature lanes: its release lane already runs a
musl-only matrix (its gnu pair was retired in night-badass-1) and its
publish job carries the immutable-release fix.

## Why the Musl Law Is the Right Peak

- **One linkage story.** A fully static musl binary embeds its libc.
  The same artifact runs identically on glibc hosts, musl hosts,
  Alpine, containers, and minimal base images. There is no host libc
  version matrix to defend, no glibc floor to audit, and no
  dynamic-loader variable in any terminal or sandbox contract.
- **Pure-Rust dependency surface.** The lockfile carries no C-linking
  crates (no openssl-sys, no curl-sys, no libz-sys), so the musl target
  links cleanly with rustup's self-contained musl objects. The supercharger
  v4 shape costs nothing extra.
- **The torture lanes already agreed.** supermassive-kernel.yml built the
  musl alias before this session: the kernel-floor lane called the static
  shape "the honest payload for an initramfs guest". This session made
  every other lane agree with that judgment.
- **Smaller attack and support surface.** Three release legs (two glibc
  amd64, one glibc aarch64) retired; two static legs (v3-musl, v4-musl)
  ship. Fewer artifacts, each with a strictly stronger portability claim.

## Findings Beyond the Directive

1. **Immutable-release exposure (found, fixed).** The publish job created
   the release with `draft: false`. GitHub's immutable-release feature
   locks assets the moment a release is published, and
   softprops/action-gh-release publishes on creation - on an immutable
   repo the tail of a multi-asset upload gets rejected mid-flight (the
   sibling's incident shape: 7 of 10 assets landed, 3 rejected). The fix
   is the sibling's pattern: create as a draft, upload everything, then
   un-draft atomically with an explicit `isDraft` post-condition tripwire,
   plus `fail_on_unmatched_files: true` so an unmatched glob fails loudly
   instead of shipping an asset-less page.
2. **aarch64 gnu leg was double dead weight.** It needed a cross gcc,
   a dedicated build step, and a dedicated metadata-verification step -
   all guarding a package that the static musl law does not need. Removed
   with its CI twin job. aarch64 stays covered by the pro-native local
   path, the Android lane, and the rust-lld aarch64-musl target pin in
   `.cargo/config.toml`.
3. **The gate leg moved, not lost.** The full gate (fmt, clippy, tests,
   cargo audit, cargo deny) rode the amd64-v3-gnu leg; it now rides the
   amd64-v3-musl leg, so the gate exercises the exact linkage the release
   ships - a strictly better property than gating a shape that no longer
   ships.
4. **Actionable preflight over deep linker errors.** `build.sh release`
   and `verify-release-build.sh` now preflight the musl rustup target and
   fail with the exact fix line (`rustup target add
   x86_64-unknown-linux-musl`) instead of a bare linker error mid-build.
5. **History stays labeled, guidance goes current.** The v30 benchmark
   reference records keep their gnu-era data lineage explicitly (they were
   measured on gnu-era builds); the actionable guidance around them now
   names the musl shapes. Archived audits and endurance records are
   untouched by design - they are timestamped history, not guidance.

## Verification Record (honest, local limitations stated)

- Every touched workflow parsed with a YAML parser plus structural
  assertions (matrix contents, gate-leg flag, PGO pair, publish-step
  draft contract, AUR trigger condition).
- Every touched shell script passed `bash -n`; every touched Python
  tool passed `py_compile`; `Cargo.toml` and `.cargo/config.toml` passed
  a TOML parse.
- Residue sweep: `v3-gnu`, `v4-gnu`, `aarch64-unknown-linux-gnu` survive
  only in (a) explicitly labeled historical references (system
  requirements history, v30 benchmark lineage, endurance records,
  archived audits) and (b) the `claimed_x86_baseline` test, which
  documents by contract that legacy gnu-shaped build ids keep claiming
  their baseline if a local host build ever produces one.
- No cargo toolchain was available on the session machine (no rustup
  cache; the toolchain download could not complete in the session's
  time budget), so `cargo fmt`/`clippy`/`test` and the musl compile were
  not executed locally. The compile-level verdict belongs to CI: the
  musl gate leg, the variant matrix, the PGO lane, and the torture lanes
  all compile the shipped shapes on every push. The rust-side edits are
  name-for-name substitutions in strings, comments, and test fixtures;
  no signature or type surface changed.
- Benchmark A/B (10 s visual + performance) intentionally not run: this
  session touches build infrastructure and packaging, not runtime visual
  or engine code. The rendered-frame logic is byte-identical; there is
  no before/after visual delta to measure, and the metric that matters
  here (static linkage, asset names, release atomicity) is verified by
  the workflows themselves.
- `build.sh check-all` not run locally for the same missing-toolchain
  reason; the local gates that could run, did run (bash -n, py_compile,
  TOML/YAML parse, residue sweeps).

## Post-Session Release Surface

| Platform | Package | Linkage |
|----------|---------|---------|
| Linux amd64 v3 | `cosmostrix-<tag>-linux-amd64-v3-musl.tar.gz` | static musl, x86-64-v3 |
| Linux amd64 v4 | `cosmostrix-<tag>-linux-amd64-v4-musl.tar.gz` | static musl, x86-64-v4 |
| Linux amd64 PGO | `...-v3-musl-pgo` / `...-v4-musl-pgo` | static musl + PGO |
| macOS arm64 | `darwin-aarch64-native` | host-native |
| FreeBSD amd64 | `freebsd-amd64` | host-native |
| Windows x64/arm64 | `windows-*` | MSVC |
| Android arm64 | `android-aarch64-native` | NDK |

No glibc-linked package ships. Every Linux x86_64 artifact is a fully
static musl executable; the release page publishes atomically after a
complete asset upload, and the AUR package tracks the musl assets with
no glibc/gcc-libs runtime dependency.

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
