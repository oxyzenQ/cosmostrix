<!-- SPDX-License-Identifier: MIT -->

# Zactrix Integration Audit

v4.8.0 Phase 1 is the Zactrix lab integration audit. It starts from the
released v4.7.0 mainline and prepares a safe path for render efficiency
work without changing runtime visuals, benchmark semantics, or terminal
ownership.

This is not a direct lab branch merge. This is not a 50k FPS promise.

## Baselines

| Track | Branch / Commit | Role |
|-------|-----------------|------|
| Main baseline | `main` / `52a558b` | Released v4.7.0 stability baseline |
| Accepted candidate source | `zactrix-20k-lab` / `e7253e7` | Reduce redundant color pipeline work |
| Boundary evidence | `zactrix-50k-lab` / `23c97a6` | 50k ceiling research closure document |

## Accepted Candidate

The accepted optimization candidate is:

- reduce redundant color pipeline work

The candidate appears in `e7253e7` and changes these relevant files:

- `src/palette.rs`
- `src/droplet.rs`
- `src/frame.rs`
- `src/cloud/monolith.rs`
- `src/cloud/phosphor.rs`
- `src/cloud/rain.rs`
- `src/cloud/render.rs`
- related depth and documentation guard tests

The safe-looking pieces are small and scalar:

- decode terminal colors once, then reuse RGB tuples through brightness and
  blend steps
- use fixed-point RGB helpers for brightness and white/core blending
- cache whether the current character pool is binary inside draw context
- provide a forced frame-cell write helper for known cleanup paths

These are Phase 2 adaptation candidates. They still require visual parity,
depth regression, benchmark honesty, and current-main conflict review before
they can land.

## Mainline Adaptation Notes

The v4.7.0 mainline includes profile ecosystem and release-candidate guard
work that was not the focus of the performance lab. Integration should adapt
individual changes onto current main instead of merging either lab branch.

Known adaptation pressure:

- `src/docs_tests/zactrix.rs` is already large, so new guards should use a
  separate test module.
- benchmark fields must remain stable and honest.
- terminal execution remains single-threaded for rendering.
- no worker-thread or parallel terminal write experiment is part of Phase 1.
- AUR metadata, version files, and release tags are out of scope.

## 50k Lab Closure

`zactrix-50k-lab` is documentation evidence only for this integration branch.
It records that 50k FPS was not reached and that extra attempts beyond the
accepted color-pipeline candidate were neutral or slower.

Rejected 50k attempts stay rejected:

- Frame dirty epoch stamps
- Monolith stale-only cleanup
- Edge-fade line cache
- Non-TTY benchmark progress elapsed gate

The closure document is kept in `docs/ZACTRIX_50K_LAB.md` so future work can
see the boundary evidence without replaying rejected experiments.

## Invariants

Phase 1 and later integration work must preserve these benchmark and runtime
invariants:

- dirty ratio roughly 6.8%-7.6%
- active_streams_avg roughly 40-42
- active_frame_ratio 100%
- actual_execution single-threaded-renderer
- terminal_writer single-owner
- compute_parallelism disabled

Optimization must not reduce visual density, reduce active streams, fake
dirty-cell ratios, remove benchmark fields, spawn worker threads, or parallelize
terminal writes.

## Integration Rules

- no direct merge from lab branches
- cherry-pick or adapt only clean changes
- visual equivalence required
- benchmark honesty required
- no version bump during Phase 1
- no v4.8.0 tag or release during Phase 1
- no generated benchmark dumps, logs, or videos in git

## Phase 1 Decision

Code integration is deferred to Phase 2. Phase 1 records the audit, imports the
50k closure evidence, and installs documentation guards so the v4.8 path stays
honest before any render-path optimization lands.
