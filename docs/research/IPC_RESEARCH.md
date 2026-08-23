<!-- SPDX-License-Identifier: GPL-3.0-only -->

# IPC Research — Can cosmostrix reach ~3.0?

**Date**: 2026-08-23 · **Mode**: research, no code changes · **Question**:
can IPC (instructions per cycle) be lifted from the measured **2.67** (Zen 3,
Ryzen 7 5800HS, local-pro build) toward **~3.0** without any visual or
performance regression? · **Owner's framing is correct**: IPC > 1.5 already
proves the workload is cache-friendly (L1/L2 resident, no memory stalls);
the remaining ceiling is control-flow, not data.

**TL;DR**: **Yes, plausibly — via a path the project already owns but has
never benchmarked: PGO.** The measured budget shows branch mispredicts cost
an estimated 13–19% of all cycles; halving them lifts IPC to ~2.85–2.92,
and PGO's code-layout gains on top make 3.0 reachable. Zero visual change
(output is bit-identical), zero API change, and the infrastructure
(`./scripts/build.sh pgo`) already exists — the release notes even print
`pgo: no — not currently used`. Secondary gains exist in extending the
existing BOLT branchless pattern. SIMD stays rejected (existing documented
policy).

---

## 1. Where the 14.4B cycles actually go

Measured baseline (owner's benchmark, 5 s, monolith, 105×64, Zen 3):

| Metric | Value | Meaning |
|--------|------:|---------|
| cycles | 14.4B | total work |
| instructions | 38.3B | IPC 2.67 |
| branch instructions | 6.3B | 16% of instructions are branches |
| branch misses | 141.9M | 2.24% mispredict rate |
| est. mispredict cost | **1.8–2.7B cycles** | 141.9M × 13–19 cycle Zen 3 penalty = **13–19% of all cycles** |

Context for honesty: 2.67 IPC is already *high* for branchy scalar code —
typical SPECint-class workloads sit at 1.5–2.5 on Zen 3; the >3.0 band the
project's own docs call "excellent" is territory for regular,
predictable-flow code. The renderer is dominated by data-dependent
per-cell decisions, so every point of IPC above ~2.5 must come from making
those decisions cheaper, not from removing them.

## 2. The four levers, ranked by confidence

### Lever 1 — PGO (profile-guided optimization): the designed-but-unused route ★★★

The repository has a complete two-stage PGO pipeline
(`./scripts/build.sh pgo`, instrumented → profile → final) and release
profiles that report PGO status — but every benchmark on record (including
the owner's) runs a non-PGO build (`pgo: no`).

Why PGO is the right tool for THIS workload:

- The hot path is a chain of *data-dependent* branches (per-cell
  style-change checks, `CharLoc` dispatch, palette-slot/wave decisions,
  run-boundary detection). PGO feeds the compiler the real executed
  probabilities, which unlocks: hot/cold block splitting, better branch
  layout (fallthrough on the common path), partial predication (`cmov`
  conversion of highly-biased branches), and improved inlining budgets.
- It changes only compiled code shape — **output bytes are identical**,
  so the visual-identity constraint is satisfied by construction.
- Expected magnitude: industry-typical PGO gains on branchy code are
  5–15% overall; on this workload, most of the gain should come through
  exactly the mispredict + layout channels that cap IPC today.
- Verification is already built in: the benchmark's MICROARCHITECTURE
  section reports `ipc` and `branch_mispredict_rate` — a PGO vs non-PGO
  A/B is one command pair (see §4).

### Lever 2 — extend the BOLT branchless pattern ★★

The codebase already proves the technique: `BOLD_ESCAPES` replaced the
bold branch with a table lookup (docs record mispredict 0.57–2.41% on v30
when BOLT was applied). The same conversion applies to:

- The SGR emission decision (fg/bg change → select cached bytes
  arithmetically instead of branching) — the single hottest branch chain
  in `terminal/draw.rs`, executed per cell.
- `CharLoc` → palette-index selection in `resolve_cell_color`, where the
  match arms are mutually exclusive values, not actions — a candidate for
  a lookup table.

Each conversion is a small, testable, BOLT-style change; each removes a
hard-to-predict branch rather than making it predictable. Cost: touching
locked engine code (cosmic lock protocol applies; the BOLT precedent shows
the unlock pattern).

### Lever 3 — mispredict budget math (what "half" buys)

Halving the mispredict rate (2.24% → ~1.1%) recovers roughly
0.9–1.35B cycles of 14.4B → **IPC 2.85–2.92** at the same instruction
count. PGO's layout gains (fewer stalls, better µop-cache hit rate) are
what closes the remaining gap to ~3.0. This is why Lever 1 + Lever 2
compose: PGO makes the remaining branches predictable, BOLT-style tables
delete the unpredictable ones.

### Lever 4 — what NOT to do (already-evaluated dead ends)

- **Manual SIMD**: rejected by `docs/SIMD_FEASIBILITY.md` — per-cell
  workload is branch-dominated, defeats vectorization, requires new
  `unsafe`, est. 5–15% for major maintenance cost. Still correct.
- **SoA/Cell-layout restructuring**: would widen ILP but is a large
  refactor through the locked diff engine with hot-path regression risk —
  the opposite of LTS stability for a stretch metric.
- **Chasing IPC with more instructions**: IPC is a proxy, not a goal.
  `perf`-style padding (e.g., unrolling to trade instructions for IPC)
  can *raise* IPC while *slowing* the wall clock. The governing metric is
  cycles-per-frame; every recommendation above reduces it.

## 3. Honest risk assessment

- **3.0 is a stretch, not a promise.** 2.67 on branchy Zen 3 code is
  already in the top band for this class. PGO + branchless conversions
  make ~2.9–3.1 *plausible*; if the mispredict floor turns out dominated
  by genuinely random data-dependent branches (wave positions, glitch
  maps), the ceiling may land at ~2.85 — still a real cycles-per-frame
  win.
- **Scene dependence**: monolith (the benchmark default) is the most
  regular scene; cinematic/signal have more chaotic branching. Expect the
  bigger IPC deltas on the heavier scenes.
- **Measurement variance**: pin the CPU governor (the benchmark env
  section already discloses it), same terminal size, two runs, discard
  the first.

## 4. Verification protocol (all tools already in-tree)

```bash
# Non-PGO baseline (any pro profile)
cargo build --profile pro-linux-v3 ... && ./target/.../cosmostrix --benchmark

# PGO build via the existing pipeline
./scripts/build.sh pgo --auto

# Compare: PERFORMANCE.avg_frame_time (cycles/frame proxy),
# MICROARCHITECTURE.ipc + branch_mispredict_rate
# For branch-level attribution on Linux:
perf record -e branch-misses ./target/.../cosmostrix --benchmark
perf annotate --source --sort symbol
```

Success criteria, in priority order: (1) avg_frame_time drops, (2) mispredict
rate < 1.5%, (3) IPC ≥ 2.9. Criterion (1) is the one that matters — IPC is
only its proxy.

## 5. Measured result (2026-08-23, step 1 executed)

The PGO A/B from §4 ran the same day in a 2-core CI container (perf
counters unavailable, so the IPC half awaits a bare-metal re-run):

- avg_fps **+4.5%** (91,337 → 95,475, interleaved 3-run medians)
- avg_frame_time −4.5% (0.0110 → 0.0105 ms)
- **max_frame_time −35%** (0.0727 → 0.0473 ms — the jank spike nearly
  halved; this is the code-layout effect in action)
- total_ns_per_cell −5.0%; per-frame visual metrics identical
  (deterministic seed)

Conclusion: the PGO lever is confirmed real at the low end of the
estimated range, with the biggest effect on worst-case smoothness.
Remaining steps unchanged: measure IPC/mispredict on bare metal (owner's
rig — the benchmark already prints them), and if mispredicts stay >1.5%,
extend BOLT-style tables to the SGR decision and CharLoc selection.

Full data: [`benchmark/bench-labs/PGO_AB_20260823.md`](../../benchmark/bench-labs/PGO_AB_20260823.md).

## 6. Recommendation

1. Run the PGO A/B first (zero code changes, uses existing infra) and
   record results in `benchmark/bench-labs/` + `BENCH_LABS.md`.
2. If mispredicts remain > 1.5% after PGO, extend BOLT-style tables to the
   SGR decision and `CharLoc` selection (engine-unlock commits, one at a
   time, with the lock protocol's A/B evidence).
3. Keep SIMD and layout restructuring rejected for LTS; revisit only if
   the workload profile changes (e.g., an 8K-default future).

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
