<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

# Night Frontier Z5c — the unsafe law: compiler-forbidden render core, pinned OS boundary

Owner: oxyzenQ | Session: night-frontier-Z5 | Date: 2026-11-19
Reference: github.com/oxyzenQ/zelynic (mature infra lineage)

## The law this work lands

The repo's no-unsafe-in-renderer policy was a review convention
(`docs/PHILOSOPHY.md` section 3, the SIMD rejection record). Z5c
promotes it to two enforced layers:

1. **The compiler layer.** `#![forbid(unsafe_code)]` on the five safe
   engine trees — the Chroma Dragon (`src/engine/chroma_dragon_engine/`),
   the Crystal Dragon (`src/engine/crystal_dragon_engine/`), and the
   Cosmic Dragon's render core (`cloud/`, `frame.rs`, `runtime.rs`).
   `forbid` cannot be overridden by a nested allow, so a future unsafe
   block anywhere in those trees is a BUILD ERROR, not a review
   finding. A probe unsafe in `frame.rs` was compile-rejected during
   the landing (then removed) — the teeth are real.
2. **The census layer.** `scripts/gates/unsafe-census.py` (gate-keepers
   check 23) pins the ENTIRE non-test unsafe surface line-by-line: 61
   non-comment `unsafe` lines across 17 OS-boundary files at the Z5c
   SHA (clock, platform guards, watchdog, reclaim, sysstat, the
   terminal/tty recovery ladder, the io_uring rejection record, the
   bench allocator tracing). Growth, shrinkage, or a new file on the
   surface is a finding — the surface can only change on purpose.

## What the honest census found (the research correction)

The session's first census (a truncated `rg -c` grep) undercounted:
the engine's `terminal/tty` recovery ladder
(`terminal_tty.rs`, `io_recovery.rs`) carries REAL, audited unsafe —
raw fd writes, libc calls, the write-recovery ladder the syscall-floor
ledger (entry 35) proves. The plan to forbid the whole
`src/engine/` was therefore wrong: the terminal subtree is the OS
boundary, and the boundary is exactly where audited unsafe belongs.
The landed shape forbids the render core (where the aesthetic lives —
pure computation) and pins the boundary (where the OS lives — audited
line-by-line). The lesson is the session's own law: census before
pinning, and the pin comes from the scanner itself, never from a
looser tool.

## The test-support carve-out

Rust edition 2024 marks `std::env::set_var` unsafe, and the chroma
test seat mounts INSIDE the chroma module tree (the `#[path]` test
include) — its env-isolation setup tripped the forbid. The unsafe did
not move into the forbidden tree; it moved OUT to one audited home:
`test/test_support/mod.rs`, mounted at the crate root, exposing
`set_env_var` behind a documented Safety contract. One helper, one
unsafe block, one named home — and the forbidden trees stay
unconditionally forbidden (no `cfg_attr(test)` weakening).

## Honest scope notes

- The census counts non-comment lines matching `\bunsafe\b` in `src/`
  (the shipped surface). `test/` is out of the census pin by design —
  the harness's env mutations live in the one support file; widening
  the scan to `test/` is a conscious follow-up if the owner wants the
  harness surface pinned too.
- The forbid trees cover the engine compute paths. Other unsafe-free
  subsystems (config, cli, msg_fill_style, scene, theme, rain_style...)
  are covered by the census (zero hits = pinned at zero), not by the
  compiler attribute — widening the forbid scope there is a cheap
  follow-up, deliberately not bundled into this commit (one surface
  per commit).

## Proof

- Compile-negative: an unsafe block added to `frame.rs` fails
  `cargo check --all-targets` (observed during landing).
- Census-negative: a synthetic unsafe line fails
  `unsafe-census.py` with a NEW-unsafe finding naming the file
  (observed).
- Full `cargo test` suite green (3082 tests); clippy `-D warnings`
  clean; fmt clean; permissions 644/755 hold.
- A/B: `benchmark/bench-labs/night_frontier_z5c/AB_REPORT.md` —
  performance-neutral, visual-identical within noise (the attribute is
  a lint; zero codegen).

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
