// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

# Zactrix 50k Performance Lab

Branch: `zactrix-50k-lab`

Starting commit: `e7253e7` (`perf(v4.8): reduce redundant color pipeline work`)

Final commit: the commit containing this lab record on `zactrix-50k-lab`.

Base branch context:

- `zactrix-50k-lab` was created from `zactrix-20k-lab`.
- `zactrix-20k-lab` remains at `e7253e7` and was not rewritten.
- `main` was not touched or merged during this lab pass.
- No version bump, tag, release, or AUR metadata change was made.

## Goal

Explore whether Cosmostrix can honestly reach a stable 50k+ FPS synthetic
benchmark ceiling without reducing visual work, dirty-cell coverage, active
streams, terminal writer honesty, or renderer correctness.

50k FPS is treated as a stretch target. The actual goal is bottleneck discovery
and a safe Pareto point, not a suspicious peak number.

## Baseline

Environment baseline on this machine was lower than the earlier expected
26k-28k range. The invariants stayed healthy, so this was treated as the local
branch/machine baseline rather than a visual-work collapse.

| Run | avg_fps | median_fps | p95 ms | p99 ms | dirty % | streams | stability |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 1 | 21242.6 | 21681.9 | 0.052 | 0.055 | 7.22 | 41 | excellent |
| 2 | 21092.9 | 21592.4 | 0.054 | 0.059 | 7.22 | 41 | excellent |
| 3 | 21199.7 | 21571.2 | 0.052 | 0.055 | 7.22 | 41 | excellent |
| 4 | 21024.7 | 21655.8 | 0.052 | 0.055 | 7.22 | 41 | excellent |
| 5 | 21214.7 | 21423.4 | 0.053 | 0.056 | 7.22 | 41 | excellent |

Baseline mean avg_fps: `21154.9`

Invariant labels:

- `actual_execution`: `single-threaded-renderer`
- `terminal_writer`: `single-owner`
- `compute_parallelism`: `disabled`
- `active_frame_ratio`: `100.0%`

## Attempts

### Rejected: Frame Dirty Epoch Stamps

Hypothesis: replace `BitVec` dirty de-duplication with a `Vec<u32>` epoch stamp
to avoid clearing dirty bits each frame.

Result: no gain; mean avg_fps dropped to about `20709`. The larger per-cell
stamp footprint did not beat the existing compact `BitVec` path.

Decision: rejected and reverted.

### Rejected: Monolith Stale-Only Cleanup

Hypothesis: draw current Monolith cells first, mark current occupancy, and clear
only previous cells that are not redrawn this frame.

Result: Monolith residue tests passed and dirty ratio stayed honest at `7.22%`,
but benchmark mean stayed around `20756`, below baseline. The mark bookkeeping
cost more than it saved for the current 120x40 synthetic workload.

Decision: rejected and reverted.

### Rejected: Edge-Fade Line Cache

Hypothesis: cache `viewport_edge_fade(line, lines)` per terminal line and reuse
it from `DrawCtx`.

Result: exact-value tests passed and invariants stayed unchanged, but 5-run
mean was effectively identical to baseline (`~21155`). The added state did not
provide a measurable Pareto win.

Decision: rejected and reverted.

### Rejected: Non-TTY Progress Elapsed Gate

Hypothesis: in non-interactive benchmark runs, skip the per-frame elapsed-time
calculation used only by the live progress UI.

Result: benchmark fields stayed honest, but 5-run mean was about `20667`, below
baseline on this machine. The extra timestamp read was not the limiting factor.

Decision: rejected and reverted.

## Final Metrics

No code optimization was accepted in this pass. Final renderer performance is
therefore expected to match the baseline within normal run-to-run noise.

Final validation on the same machine showed lower avg_fps than the initial
baseline, while all visual-work invariants remained identical. Because the only
accepted repository change is this documentation file, the lower final mean is
recorded as measurement/load variance rather than a renderer regression.

| Run | avg_fps | median_fps | p95 ms | p99 ms | dirty % | streams | stability |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 1 | 20103.1 | 20415.7 | 0.058 | 0.065 | 7.22 | 41 | excellent |
| 2 | 20713.5 | 21030.5 | 0.053 | 0.056 | 7.22 | 41 | excellent |
| 3 | 20655.8 | 21115.3 | 0.054 | 0.056 | 7.22 | 41 | excellent |
| 4 | 20400.5 | 21576.4 | 0.056 | 0.069 | 7.22 | 41 | excellent |
| 5 | 20847.9 | 21469.8 | 0.054 | 0.066 | 7.22 | 41 | excellent |

Final mean avg_fps: `20544.2`

## Tier Result

- Bronze (`32k-39k`): not reached
- Silver (`40k-49k`): not reached
- Gold (`50k+`): not reached

This pass did not find a safe code change that moved the branch toward 50k FPS
without either adding complexity for no gain or losing benchmark confidence.

## Accepted Changes

- Added this lab record documenting baseline measurements and rejected
  optimization attempts.

No renderer code, benchmark counters, visual density, active stream count,
terminal writer policy, or compute-parallelism policy was changed.

## Rejected Categories

- Benchmark-only visual shortcuts: rejected by policy.
- Dirty-ratio reduction by skipping visual work: rejected by policy.
- Worker threads, Rayon, or parallel terminal writing: rejected by policy.
- Unsafe/SIMD: rejected by policy for this lab pass.
- State/cache changes without measurable win: rejected after measurement.

## Future Candidates

Future investigation should start with symbolized profiling rather than broader
structural edits. The current pro binary is stripped enough that `perf report`
on this pass only produced address-level samples, which limited attribution.

Useful next steps:

- Build a one-off local profiling binary with symbols and frame pointers.
- Attribute time inside Monolith color selection, frame writes, and phosphor
  bookkeeping before editing.
- Keep the invariant guardrails: dirty ratio near `6.8%-7.6%`, active streams
  near `40-42`, `single-owner` terminal writer, and `disabled` compute
  parallelism.

## Safety Confirmation

- `main` was not touched.
- `main` was not merged.
- `zactrix-20k-lab` was not rewritten.
- No version bump was made.
- No tag or release was created.
- No AUR release metadata was touched.
- No generated benchmark logs, CSV files, or videos were committed.
