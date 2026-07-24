# dragon-multicore — Experimental Multithreaded Simulation

<!-- SPDX-License-Identifier: GPL-3.0-only -->

> **Status**: experimental branch, not for merge to main.
> **Origin**: forked from `v20.0.0` tag.
> **Goal**: test whether parallelizing the droplet simulation pass
> produces a meaningful FPS improvement.

## Hypothesis

The Dragon diff-based renderer spends most of its frame budget in the
dirty-check and IO loop (single-threaded by necessity — there's one
stdout, one cursor, one escape sequence stream). The droplet simulation
pass (`Droplet::advance`) is also serial. If we parallelize the
simulation across cores via rayon, will the gain offset the
synchronization overhead?

## What was implemented

`src/cloud/rain.rs` — the droplet simulation pass was split into two
phases:

1. **Phase A (parallel under `--features multicore`)**: compute a
   `SimUpdate` per alive droplet. Each `Droplet::advance` call only
   mutates the droplet itself — no shared state — so this is safe to
   parallelize with `par_iter_mut`.

2. **Phase B (always serial)**: apply the `SimUpdate` list. This
   touches shared `Cloud` state: `col_stat`, `droplet_free_list`,
   `set_column_spawn`, `do_glitch_span`. These mutations can't be
   parallelized without locks that would dwarf the gains.

The split is feature-flagged: `cargo build` produces the original
serial binary; `cargo build --features multicore` produces the
experimental parallel binary. Behavior is identical — only the
scheduling of Phase A changes.

## How to reproduce

```bash
# Build both binaries
cargo build --release
cp target/release/cosmostrix /tmp/cosmostrix-serial
cargo build --release --features multicore
cp target/release/cosmostrix /tmp/cosmostrix-multicore

# Run the A/B benchmark
python3 scripts/bench-multicore.py
```

Results are written to `benchmark/multicore_results.md`.

## Result on the development host (2 cores, x86-64-v4)

| Size    | Serial FPS | Multi-core FPS | Speedup |
|---------|------------|----------------|---------|
| 80x24   | 73K        | 73K            | 0.999x  |
| 200x60  | 21K        | 22K            | 1.013x  |
| 400x200 | 8K         | 8K             | 1.013x  |

**Verdict**: NEUTRAL — average speedup 1.008x, within noise.

## Why it didn't move the needle

The benchmark's own `component_timing` section tells the story:
`avg_sim_ms` is `0.0000` in both configurations. The droplet simulation
pass is already so cheap (sub-microsecond per droplet) that there is
almost nothing to parallelize. The render pass (`avg_render_ms`) is
where the time goes, and that pass is bound by shared mutable state
(the `Frame` buffer) which can't be parallelized without a fundamental
rearchitecture.

In other words: the engine isn't CPU-bound on simulation. It's
dirty-cell-bound on rendering, and that's a single-threaded problem by
construction (one terminal, one cursor).

## What might shift the balance

- **Massive column counts** (e.g. 1000+ columns on an 8K terminal).
  At 400 columns the sim pass is still sub-millisecond; at 1000+ it
  might finally become a measurable fraction of the frame.
- **More cores.** This host has 2. A 16-core machine might amortize the
  rayon startup cost better.
- **A heavier per-droplet physics step.** If we add wind-tunnel
  turbulence, particle interactions, or column-to-column coupling, the
  sim pass might become worth parallelizing.

## Decision

Do not merge to main. The experiment is honest negative result — it
confirms the Dragon's single-core design is the right call for the
terminal renderer workload. The branch is preserved as a documented
exploration so a future maintainer asking "why don't we use threads?"
has a concrete answer with numbers.

The `SimUpdate` split is a useful refactor regardless: it makes the
boundary between "pure droplet physics" and "shared-state mutation"
explicit, which is good for future maintainability. That refactor
could be cherry-picked to main without the rayon dependency if
desired.

## Files changed on this branch

- `Cargo.toml` — added optional `rayon` dependency + `multicore` feature
- `src/cloud/rain.rs` — split the simulation pass into Phase A (sim)
  + Phase B (apply), added `SimUpdate` struct
- `scripts/bench-multicore.py` — A/B benchmark harness
- `docs/DRAGON_MULTICORE.md` — this file
- `benchmark/multicore_results.md` — generated results
