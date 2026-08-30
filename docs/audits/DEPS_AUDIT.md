<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Dependency Mastery Audit — Supply Chain & Bloat

**Date**: 2026-08-23 · **Scope**: every direct dependency (runtime + dev +
platform-gated), the full transitive tree, CI tooling pins · **Method**:
usage-tracing every dep to its production call sites via ripgrep, feature
analysis via `cargo tree -e features`, supply-chain posture review against
the project's existing controls (committed Cargo.lock, `--locked` CI builds,
`cargo deny`, daily `cargo audit`).

**TL;DR**: **10 direct dependencies, all justified — zero useless ones.**
Every dep traces to real production call sites; the two "heaviest per line
of functionality" candidates (bitvec, sha2) are both load-bearing and stay.
One real trim landed: proptest's default features pulled fork/timeout/
tempfile machinery that the property tests never use. One real supply-chain
fix landed: the CI python lint tools (ruff, codespell) were installed
**unpinned**, which is what let a newer local ruff surface rule drift
against code that CI had just accepted — both tools are now pinned to
verified versions.

---

## 1. Runtime dependency inventory (10 direct, 59 transitive = 69 crates)

| Dependency | Version | Production use | Verdict |
|-----------|---------|----------------|---------|
| `clap` | 4.5.61 | CLI parsing, `--help`, did-you-mean | **Keep.** Already minimal: `default-features = false` + explicit features (std, color, help, usage, error-context, derive). The derive adds proc-macro crates at compile time only. |
| `crossterm` | 0.29.0 | Raw mode, alt screen, events, mouse, resize | **Keep.** Core substrate; features already trimmed (bracketed-paste, events, windows only). |
| `rand` | 0.9.5 | `StdRng` (seeded, deterministic — benchmark reproducibility) + 2 `rand::rng()` sites | **Keep.** The tree (rand_core, rand_chacha, ppv-lite86, zerocopy) is the standard StdRng stack; ThreadRng adds no extra crate. Default features are all in use. |
| `bitvec` | 1.1.1 | `glitch_map`, `phosphor_fresh`, `phosphor_in_active` bitsets + `BitSlice` borrows in the shader hot path | **Keep** (revisited — see §3). 5 crates, but 3 membership bitsets + the only safe bit-slice borrow API for the shader path. Hand-rolling a `Vec<u64>` bitmap in the per-cell hot path is new code for zero user-visible gain. |
| `smallvec` | 1.15.2 | Inline dirty-index list (256 slots, zero heap at common terminal sizes) | **Keep.** Directly serves the zero-per-frame-alloc guarantee. |
| `unicode-width` | 0.2.2 | Wide-char guard (Bug #11), terminal column math | **Keep.** Correctness-critical; rust-official adjacent. |
| `notify` | 6.1.1 | Live-reload watcher | **Keep.** Features already per-platform tuned (macos_fsevent / kqueue); the Cargo.toml comment documents why each feature exists. |
| `sha2` | 0.10.9 | Config SHA-512 fingerprinting (live-reload change detection, dump/testconf fingerprints) | **Keep** (owner-mandated). RustCrypto, pure Rust, no unsafe. Cryptographic strength is an owner contract (FNV-1a was deliberately removed); a lighter hash would violate it. |
| `signal-hook` (unix) | 0.3.18 | Self-pipe signal handling | **Keep.** The iterator pattern is the verified-safe design (SV-05). |
| `libc` (unix) | 0.2.189 | All FFI (termios, fork guard, sysconf, getrusage, madvise, localtime_r) | **Keep.** rust-lang official; gated to `cfg(unix)`. |
| `ctrlc` (windows) | 3.4 | Windows Ctrl+Break handler | **Keep.** Windows-only, small, standard. |

## 2. Dev dependency trim (landed)

`proptest` was declared with default features, which pull `fork` +
`timeout` -> `rusty-fork` + `tempfile` + `wait-timeout` into every test
build. The property suite (`src/tests/property.rs`) uses only the
`proptest!` macro with primitive strategies — no forking, no timeouts, no
tempfiles. Trimmed to `default-features = false, features = ["std"]`:
property suite still 8/8 green, dev tree drops the fork machinery
(rusty-fork, tempfile and friends no longer build). Dev-only edge count:
76 crates total, down from ~80.

## 3. Revisited candidates — why they stay

- **bitvec (5 crates for "a bitset")**: initially the strongest bloat
  candidate. Usage audit changed the verdict: three separate membership
  bitsets on `Cloud` (glitch, phosphor-fresh, phosphor-active) sized to
  the full grid, plus `BitSlice` borrowed into `ShaderCtx` where the
  per-cell shader indexes it directly. A hand-rolled replacement is
  ~30 lines of bit math in the hottest path in the program — the classic
  "save a dep, buy a bug" trade. For LTS: keep the audited crate.
- **sha2 (6 crates for config hashing)**: cryptographic strength is an
  owner contract (the comment in Cargo.toml documents FNV's deliberate
  removal). RustCrypto is the canonical Rust implementation. Keep.
- **rand's ThreadRng**: 2 call sites (`rand::rng()`). Could be refactored
  to the seeded `StdRng` and default features trimmed, but ThreadRng adds
  zero extra crates — there is nothing to save.

## 4. Supply-chain posture

**Existing controls (verified)**: Cargo.lock committed; CI builds with
`--locked`; `deny.toml` enforces a license allowlist with a
confidence threshold; CI runs `cargo deny check all`; a daily
`cargo audit` job exists. All 10 direct deps are maintained by
high-reputation orgs (rust-lang, rust-random, RustCrypto, crossterm-rs,
clap-rs, notify-rs, bitvec/ferrilab) — no abandoned or single-unknown-
maintainer crates in the tree.

**Gap found and fixed — CI tool pinning**: the "Project lint" CI job
installed ruff and codespell **unpinned** (`pip install codespell ruff`),
and the Guard workflow installed codespell unpinned. ruff's default rule
set gains rules across releases, so CI lint behavior silently changes
whenever ruff ships — observed 2026-08-23 when a newer local ruff flagged
PLW1510/FURB167 against code CI had just accepted. Both workflows then
pinned `codespell==2.4.3` / `ruff==0.16.4` (both verified passing on the
current tree), with the upgrade procedure documented inline: fix new
findings locally first, then bump both pins in the same commit.

**Policy update 2026-08-30 (owner decision)**: the pins above were
removed again. Owner policy for `.github/*` is now "dynamic latest,
minimal maintenance, boring but strong" — CI-installed deps resolve their
latest upstream release at run time, and the trade-off (a new tool
release can turn the gate red until findings are fixed in-tree) is
accepted as cheaper than carrying and bumping version pins forever.
Same-run-time-version parity for local dev is documented in
CONTRIBUTING.md; the full policy lives in
`docs/workflow/ABOUT_CI.md` (Dependency version policy).

**Recommendation (not landed, needs owner decision)**: `cargo update`
currently floats minor/patch versions within the lock's semver ranges on
every manual unlock. For a strict-LTS posture, a scheduled
`cargo update && cargo audit && cargo deny` PR per release cycle
(rather than ad-hoc local updates) would make dependency movement fully
auditable. The daily cargo audit already covers the advisory side.

## 5. Final tree

- Runtime: **69 unique crates** (10 direct + 59 transitive), unchanged —
  the runtime tree was already minimal; all trims were in dev/CI scope.
- Dev: **76 crates** including the trimmed proptest.
- CI: lint tools pinned; build remains `--locked` + `cargo deny`.

---

Copyright (C) 2026 rezky_nightky (oxyzenQ). All rights reserved.
cosmostrix and the cosmostrix logo are trademarks of rezky_nightky (oxyzenQ).
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
