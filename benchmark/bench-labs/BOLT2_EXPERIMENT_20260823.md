<!-- SPDX-License-Identifier: GPL-3.0-only -->

# BOLT-2 Experiment — Branchless Style-Change Detection (dragon-explor-v2)

**Date**: 2026-08-23 · **Branch**: `dragon-explor-v2` (off main — main is
frozen for the LTS release) · **Goal**: reduce branch mispredicts in the
per-cell SGR emission path (the primary remaining IPC lever identified in
`docs/research/IPC_RESEARCH.md` after the bare-metal PGO verdict).

## What was changed

The per-cell style-change detection in both render loops
(`terminal/draw.rs` full-redraw + differential) previously branched on
`cell.fg != cur_fg || cell.bg != cur_bg || cell.bold != cur_bold` — each
`Option<Color>` compare is itself a discriminant branch + payload compare,
giving 6+ data-dependent compare-branches per cell.

BOLT-2 replaces the chain with `style_change_flags()`: a branchless
computation of both change flags via `u8::from(bool)` (setcc + or on x86),
consumed as a 2-bit bitmap (bit 0 → SGR emit, bit 1 → BOLT bold table).
Same family as the proven `BOLD_ESCAPES` pattern.

## Results (interleaved A/B, 4 rounds per arm, 5 s benchmark, CI container)

| Arm | avg_fps median | max_frame_time |
|-----|---------------:|---------------:|
| main (d96f82d release) | 91,133 | 0.042–0.048 ms |
| BOLT-2 | 90,786 (−0.38%) | 0.046–0.051 ms |

**Verdict: NEUTRAL on this container — not merged to main.**

- Throughput: −0.38% median, within the container's noise band (BOLT-2's
  round-2 outlier 88.6K demonstrates the spread; main's own spread was
  604 FPS vs BOLT-2's 2,640).
- Tail latency: the apparent −44% max_frame_time improvement from the
  first non-interleaved run did NOT survive interleaving — both arms
  show 0.04–0.05 ms when measured back-to-back.
- Output bit-exactness: visual identity metrics are IDENTICAL across arms
  (avg_dirty_cells 56.8 = 56.8, entropy 3.29/3.30, gini 0.896x — third
  decimal varies with sample count only), and the emitted byte logic is
  unchanged (the flags map 1:1 onto the original branch conditions).

## Why neutral, honestly analyzed

1. **The container cannot measure the mechanism.** IPC and mispredict
   rate need `perf_event_open`, blocked here. The branch-removal effect
   BOLT-2 targets is invisible to avg_fps on a 2-core shared VM where
   scheduler noise (±3%) exceeds the expected single-digit-percent gain.
2. **The converted branches may not be the hot mispredicts.** The owner's
   rig measured 146.9M branch misses at 2.47%, but that count includes
   the SIMULATION branch mix (spawn decisions, wave positions, glitch
   maps), not just the draw loop's compare chain. Without `perf annotate`
   attribution, converting draw-loop branches is a hypothesis test, and
   this container can only fail to reject "no change".
3. **LLVM may already be predicating these compares.** With
   `codegen-units=1` + fat LTO, the compiler is free to emit `cmp+setcc`
   for the short boolean chains itself; the source-level rewrite may
   produce identical machine code. (Checkable on the owner's rig via
   `objdump` diff of the two draw functions.)

## What this experiment establishes

- The BOLT-2 rewrite is **safe** (bit-exact output, full suite green,
  clippy clean) and **available** on this branch if the owner's rig
  shows it matters.
- The decisive experiment runs on bare metal:

```bash
git checkout dragon-explor-v2
cargo build --release
# interleave vs the main build; watch MICROARCHITECTURE.ipc and
# branch_mispredict_rate — NOT avg_fps (too noisy a proxy for this)
perf record -e branch-misses ./target/release/cosmostrix --benchmark
perf annotate --source --sort symbol   # confirm the draw-loop branches
```

Decision rule: merge to main only if bare-metal shows mispredict rate
drop ≥ 0.3 points (e.g. 2.47% → ≤ 2.17%) with IPC up correspondingly,
and no throughput regression beyond noise. Otherwise the branch stays as
a documented dead end — same treatment as the io_uring rejection.

## Status

- `dragon-explor-v2`: BOLT-2 committed, full suite 1649/0/2, gatekeepers
  8/8, NOT merged to main.
- Awaiting owner's bare-metal `perf` run to accept or reject.
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
