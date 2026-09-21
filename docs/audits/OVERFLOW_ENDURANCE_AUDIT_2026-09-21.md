<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Overflow / Explode-Data Endurance Audit — NIGHT-improve-10 & security-3

Date: 2026-09-21
Scope: every counter, accumulator, buffer, time computation, and
floating-point drift state that grows or advances during a running
session — the overflow/explode-data class the owner flagged
("mitigate overflow/explode data when reach the limit; cosmostrix
should be useful and have long endurance for LTS mode"). Audit
baseline: d2006c2 (HEAD after the NIGHT-improve-8 follow-up).
Method: surface-by-surface source audit (types, arithmetic style,
growth-vs-bound lifecycle) against worst-case long-session math —
24 h capped runs for benchmark/duration paths, indefinite
screensaver runs for everything else. Owner rules honored: audit if
all peak then skip (no over-engineering), micro-commit per task,
no benchmark for a docs-only change.

## Verdict

**All surfaces verified at peak — zero defects, zero code changes.**
Every integer accumulator that a long session advances is either
u64 (wrap horizons from ~42,700 years up), u32 with a proactive
reset threshold well before wrap plus a lock-test-verified invariant,
or saturating arithmetic. Every container that a long session grows
is bounded by a fixed ring/cap/expiry/dedup contract. Every
Duration construction from runtime values is floored upstream so the
`from_secs_f64` inf-panic path is unreachable. Every floating-point
drift state is sinusoidal-with-wrapped-phase, derived-from-bounded-
averages, or EMA — none is a linear accumulator. The endurance story
is already backed by `docs/ENDURANCE.md` (RSS/fd/handle stability
methodology for 24 h+ sessions). Per the owner's audit-if-peak-skip
rule this is a record-only audit: the evidence below exists so the
next session does not re-audit the same surfaces.

## 1 — Integer overflow surfaces

| Surface | Type / mechanism | Worst-case wrap horizon | Verdict |
|---|---|---|---|
| `total_ansi_bytes` (terminal/mod.rs) | u64 | ~42,700 years at 13.7 MB/s (the VSCode/xterm.js Tier-1 ceiling — the highest documented sustained rate); native terminals are far below | peak |
| `flush_count`, `frames`, `drawn_frames`, `idle_frames`, `overshoot_frames`, `dirty_sum`, `dirty_samples` (terminal/mod.rs, PerfCounters) | u64 + `saturating_add` on every increment | ~2.4 billion years at the 240 FPS cap | peak |
| `bytes_since_ris`, `backpressure_skips`, `ris_resets` (terminal/mod.rs) | u64; `bytes_since_ris` is reset by the RIS path at 50 MB by design | not reachable | peak |
| `tty_recoveries` (terminal/mod.rs) | u32 | event-driven (one per PTY recovery); 4.29 billion recoveries required | peak |
| content/dirty generation counters (frame.rs) | u32 `wrapping_add` + `GEN_RESET_THRESHOLD = u32::MAX - 50_000_000` proactive reset (~2.1 years at 60 FPS, ~3-month margin, one O(N) memset amortized over years) + invariant locked by the cosmic dragon lock suite | never wraps — reset fires first | peak |
| `semantic_gen` (frame.rs) | u32 `wrapping_add` | increments only on semantic invalidation (charset/theme/live-reload changes) — not per-frame; ~2.27 B changes required | peak |
| alloc counters (diagnostics/alloc_trace.rs) | AtomicU64 `fetch_add`, `saturating_sub` for retained | centuries | peak |
| `as_secs() as u32` (rain_at.rs:1514) | u32 cast of seconds since `start_anchor` | 136 years; feeds animation phase math only (cosmetic modulus), no capacity or indexing use | benign by horizon |
| `now_secs` vs droplet `u32` timestamps | same 136-year class | benign | peak |

## 2 — Unbounded-growth surfaces (the "explode data" class)

- **FrameTimeTracker** (interactive/activity.rs): fixed `[f64; 60]`
  ring buffer, allocation-free by construction — the per-frame push
  in `update_perf_stats` is O(1) overwrite, not growth.
- **PerfCounters** (interactive/event_loop_ctx.rs): scalars only
  (u64 quintet + f64 trio) — no per-frame Vec anywhere in the perf
  path.
- **Emergent storytelling moments** (cloud/post_rain.rs →
  ecosystem.rs): `expire_moments(now)` retains-by-expiry on every
  tick, spawn is cooldown-gated (duration + 60 s), and the whole path
  is disabled in bench mode and under `--no-effects`.
- **Anomaly zones** (cloud/phosphor.rs, post_rain.rs): spawn refuses
  at `ANOMALY_MAX_ZONES`, `retain` drops expired zones per tick,
  resize clears.
- **Phosphor active set** (cloud/phosphor.rs): BitVec
  `phosphor_in_active` membership dedup before push (a cell can only
  be active once), `swap_remove` lifecycle, cleared on resize —
  bounded by grid cell count.
- **Runtime warnings + verbose diags** (config/live_config_state.rs):
  `MAX_RUNTIME_WARNING_LOG = 64` slot cap + first-occurrence-wins
  dedup (the documented anti-spam contract — a long session under
  pressure cannot fill the buffer with re-fires).
- **Droplet free list** (cloud/rain_at.rs): recycle semantics —
  pushes return indices removed by retire; bounded by droplet
  capacity, not by session length.
- **Benchmark buffers** (bench/bench_scale.rs etc.): sized by the
  fixed `SCALE_SIZES` list; sample counts bounded by the 24 h
  `--bench-duration` ceiling.
- **LastFrame diff cache / color cache / SGR cache** (terminal/):
  keyed by palette/grid size — resized with the terminal, never grown
  by session time.

## 3 — Time and duration arithmetic

- **Duration parsing** (cli/cli_parse.rs `parse_secs_f64`): f64
  throughout — component-level `is_finite` + negative rejection, and
  the accumulated total is re-checked `is_finite` before the ceiling.
  The 24 h hard ceiling (`DURATION_MAX_SECS`, S-master-HUNT-5) is
  enforced INSIDE the parser ("no caller can bypass it") and doubled
  at the CLI spec layer (`HumanSecs max: 86400.0`). A hostile
  `99999999h` input saturates f64 to inf during accumulation and is
  rejected as non-finite — integer overflow is structurally
  unreachable on this path.
- **`Duration::from_secs_f64(1.0 / effective_fps(...))`**
  (event_loop.rs): the division-by-zero inf-panic path requires
  effective_fps == 0.0. Unreachable: `PowerManager::new` floors
  `base_target_fps` at 1.0 (`n.max(1.0)`), the live-reload setter
  re-floors (`fps.max(1.0)`), the paused branch returns the constant
  `1000 / PAUSE_PERIOD_MS`, the power-dragon branch clamps to an
  explicit `OUTPUT_DRAIN_FPS_FLOOR`, and the idle multiplier
  (`IDLE_FPS_FACTOR = 0.5`) is a crate const with no config/CLI
  exposure. Worst case is a 1 s frame period, not inf.
- **`--duration` auto-exit** (event_loop.rs:98):
  `start_time + Duration::from_secs_f64(s)` with `s <= 86400` from
  the capped parser — Instant add-overflow needs a centuries-scale
  monotonic domain; unreachable.
- **Sim delta cap** (event_loop_sim_draw.rs): the
  `f64::clamp(min > max)` panic case is explicitly sanitized (uses
  `sim_max_s` as the effective lower bound when inverted).
- **Instant vs anchors**: every hot-path comparison uses
  `saturating_duration_since`; the remaining plain `duration_since`
  sites are now-vs-earlier-anchor on monotonic `Instant`s (panic-free
  by construction) or `SystemTime::duration_since(UNIX_EPOCH)` where
  the Err arm (clock jumped before epoch) is handled with
  `unwrap_or(0)`.
- **HUD session clock**: u64 seconds tiered compound format with
  `saturating_sub` pause accounting — 584-billion-year domain.
- **Exit-summary divisions**: behind `frames.max(1)` guards; f64
  division cannot panic (worst case is a printed inf/NaN, and the
  guards keep even that unreachable).

## 4 — Floating-point long-run drift

- **EntropyDrift** (ecosystem.rs): `entropy_phase %= 1.0` every
  tick — the phase is wrapped BEFORE the `sin` evaluation, so the
  argument stays in [0, 2π] for the entire life of the process; the
  derived offsets (`density/luminance/anomaly_offset`) are
  sinusoidal × fixed ranges — bounded by construction, with no f32
  large-argument precision loss even after months of uptime.
- **RendererMemory** (ecosystem.rs): fixed `[f32;
  MEMORY_HISTORY_SAMPLES]` ring arrays + `history_idx` wrap; the
  pressures are recomputed from bounded history averages
  (derived, not accumulated) — cannot drift.
- **PowerManager EMA family** (pressure/drain backoff): exponential
  moving averages with increment/decay constants — converging by
  construction; a sustained overshoot saturates at the documented
  backoff maximum, which is exactly the intended degradation
  behavior.
- **f64 perf sums** (`work_sum_s`, `pressure_sum`,
  `utilization_sum`): IEEE-754 saturating-to-inf semantics (no UB,
  no panic); consumed only through averages behind count guards.

## 5 — What was deliberately NOT changed (over-engineering avoided)

- No widening of u64 counters to u128 — the shortest wrap horizon in
  the entire per-frame accumulator family is ~42,700 years at the
  worst documented byte rate.
- No added `checked_*` on the generation counters — they already
  carry the wraparound-safe reset threshold, the memset amortization
  argument, and a lock-test-verified invariant; layering checked
  arithmetic on top would add branches to the hottest O(1) path in
  the renderer for zero reachable benefit.
- No restructuring of the warning buffer into a ring — the 64-slot
  cap + dedup contract is documented and sufficient (warnings are
  rare, human-scale events, not per-frame data).
- No clamp added to `as_secs() as u32` (136-year horizon, cosmetic
  modulus math only).
- No benchmark run — this change is documentation-only (owner rule:
  docs-only changes skip the A/B benchmark).

## Honest limits

- The kernel-side counters cosmostrix reads via /proc (jiffies, page
  faults, context switches) are kernel-owned; any wrap there is a
  kernel contract, not an application surface.
- A system suspended across a clock-domain boundary (S3 sleep) can
  stretch `Instant` semantics; the hot paths already use saturating
  arithmetic, so the worst observable effect is one clamped delta,
  not a panic.
- This audit covers the in-process surface. Post-exit scrollback and
  terminal-side capture remain the documented anti-copy trust
  boundary (SECURITY_AUDIT.md, NIGHT-improve-8) — unchanged here.
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
