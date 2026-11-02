<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# Engine ultra-long-horizon endurance depth audit (NIGHT-xenric-1)

Date: 2026-11-02
Scope: the render engine's time math, accumulators, and collection
lifecycles, audited against an "ultra long running endurance" target
the owner framed as "useful for millions of years". Where the
2026-09-21 overflow audit (NIGHT-improve-10 / security-3,
`OVERFLOW_ENDURANCE_AUDIT_2026-09-21.md`) stopped at 24 h capped runs
plus indefinite-run reasoning, this round extends the horizon ladder
to literal million-year scale and adds one census the prior audit did
not run: every session-anchored float conversion (the freeze-at-36-h
class that NIGHT-lts-3 fixed for one site) re-checked repo-wide at
today's HEAD. Audit baseline: 1589276 (HEAD of main, post-AGPL
relicense). Method: pattern census over `src/engine/`,
`src/interactive/`, `src/msg_fill_style/` (`as_secs_f32`,
`as_millis`, `as_nanos`, `as_secs`, `Instant`, anchor fields,
`push`-site lifecycle), then manual triage of every hit into
bounded-delta vs session-anchored, with the horizon math recomputed
for this document.

## Verdict

**All engine time-math surfaces verified at peak at today's HEAD —
zero defects, zero code changes.** The session-anchored conversion
census finds exactly three functional consumers of process-lifetime
elapsed time, and each one already uses the correct representation
for its job (f64 bucket math, f64 phase with modulo wrap before the
f32 handoff, u32 in integer-hash-only domain). Every per-entity timer
is re-anchored or expiry-bounded, every spawn budget is
clamped-plus-remainder-capped with finite guards, every hot collection
is capped, drained, or equilibrium-bounded, and the self-healing layer
(watchdog, P2 self-healer, per-session stuck-cell sweep, byte-budget
RIS reset, I/O recovery) supplies the transient-corruption recovery
that multi-month sessions need. Per the owner's audit-if-peak-skip
rule this is a record-only audit; the evidence exists so the next
session does not re-audit the same surfaces.

## 1 — Horizon ladder (what breaks before a million years)

All values recomputed for this audit (integer counts, IEEE-754
mantissa math):

| Representation | Failure mode | Horizon | Reachable? |
|---|---|---|---|
| `Instant`/`Duration` (Linux `CLOCK_MONOTONIC`) | u64 nanos domain ends | ~585 years | OS-representable envelope; beyond it no software fix exists without an epoch-rollover scheme |
| f32 seconds, session-anchored | ULP = one 60 FPS frame (2^17 s) | ~36.4 h | ELIMINATED at HEAD — no session-anchored f32 seconds conversion remains |
| f32 milliseconds, session-anchored | sub-ms precision lost (2^24 ms) | ~4.7 h | ELIMINATED — only bounded per-entity deltas use f32/f-ms paths |
| u64 nanos into f64 | 2^53 ns precision wall | ~104 days | ELIMINATED — every `as_nanos() as f64` site consumes seconds-scale deltas, never uptime |
| u32 milliseconds cast | wrap | ~49.7 days | ELIMINATED — the only `as u32` ms site is a per-pulse age bounded by its own envelope lifetime |
| u32 generation counters | wrap | ~2.27 y at 60 FPS | by design — proactive `GEN_RESET_THRESHOLD` reset fires first (u32::MAX − 50 M), lock-test invariant |
| u32 seconds cast (`now_secs`) | wrap | ~136 years | benign — feeds a Knuth-mix integer hash (climate flicker), not phase math; wrap is invisible in the hash domain |
| f64 milliseconds, session-anchored | 2^53 ms precision wall | ~285,000 years | first math wall past the 585-year OS envelope; nothing to fix |
| u64 milliseconds / seconds | wrap | ~584.5 My / ~584.5 Gy | unreachable |
| u64 frame counters at 240 FPS | wrap | ~2.4 Gy | unreachable |

Reading of the ladder against the owner's "millions of years" bar:
inside the OS-representable envelope (0 to ~585 years of uptime) no
engine surface degrades; the first in-process math wall that could
even theoretically matter (f64-ms precision, ~285 ky) sits beyond the
point where the OS clock representation itself ends. The engine is
time-horizon-complete for any session the platform can express.

## 2 — Session-anchored conversion census (the lts-3 class, repo-wide)

Every functional consumer of `start_anchor`-anchored elapsed time,
re-verified at HEAD:

| Site | Conversion | Use | Verdict |
|---|---|---|---|
| `cloud/rain_at.rs` (`column_coherence_phase` call) | `as_secs_f64()` × freq, `% 2π`, then f32 handoff | column-coherence shimmer LUT | peak — the NIGHT-lts-3 pattern; wrapped before handoff, stays small forever |
| `type_rain/glyph/spawn_logic.rs` | `as_secs_f64()` | 10 s density-noise buckets | peak — f64 ms-grade precision to ~285 ky; bucket math immune |
| `cloud/rain_at.rs` (`now_secs`) | `as_secs() as u32` | climate flicker hash seed | peak — integer hash domain (`col×M ^ line×M ^ seed`), wrap invisible; no phase/index use |

Bounded-delta sites (sampled, all re-anchored or envelope-limited):
pause/resume/glyph-entry ramps (`pause_start`, `resume_start`,
`glyph_entry_time` — cleared at settle snap), border-pulse ages
(`p.birth`, drained at envelope end each frame), phosphor decay delta
(re-anchored per frame, clamped to 1/30 s, scaled by `resume_blend`),
glitch window (`last/next_glitch_time` — re-anchored on every fire,
deltas in seconds), monolith/vortex/lorenz/physarum/murmuration/black
hole spawn budgets (step deltas clamped by `max_sim_delta`), crystal
dragon poll/dwell (`last_poll` re-anchored per poll, dwell re-anchored
per theme entry), living-rain gust phases (`phase_start` re-anchored
per phase, `shift_in_time` for the §8.5 pause family).

## 3 — Collection lifecycle census (extensions over the 2026-09-21 round)

New surfaces verified this round, complementing the prior audit's
list (ring buffers, anomaly zones, warning caps, droplet free list,
diff/SGR caches — all re-confirmed unchanged):

- **Border pulses** (`cloud/message_draw.rs`): drain-and-rebuild with
  hoisted scratch + `mem::swap` — zero steady-state allocation, and
  the drain drops every pulse whose envelope ended, so the live set
  is bounded by `touch-rate × envelope lifetime`, not by session age.
- **Engrave sparks** (`src/msg_fill_style/engrave.rs`): pool-full
  truncation ("burst silently truncated"), per-frame dt capped by
  `PARTICLE_MAX_FRAME_DT_SECS` (0.25 s anti-teleport floor).
- **Message cells** (`cloud/reset_message.rs`): rebuilt from text,
  hard-capped by `MESSAGE_MAX_LEN` (200 chars); word ordinals and
  scratch arrays resize to the message, never to time.
- **Ghost events** (`cloud/ghost_events.rs`): `GHOST_MAX_ACTIVE`
  admission cap plus expiry retention.
- **Physarum trail field** (`type_rain/physarum/physarum.rs`):
  multiplicative decay `PHYSARUM_TRAIL_DECAY.powf(dt × 60 Hz ref)` on
  every cell per step — equilibrium-bounded (deposit ÷ (1 − decay)),
  frame-rate independent, no monotonic ratchet path; deposits are
  grid-indexed, cleared on resize.
- **Spawn remainder accumulators**: `SPAWN_REMAINDER_CAP` (4.0)
  clamps the carry, and every budget consumer guards
  `budget.is_finite()` before use — the inf/NaN path is dead.
- **RNG**: cloud-level `StdRng` (ChaCha family) — no short-period
  degeneracy to compound over time; entropy seeds mixed from
  startup-time nanos.

## 4 — Self-healing layer (the endurance multiplier)

Correctness under years-long uptime is not only steady-state math —
transient corruption must self-repair. Verified inventory: watchdog
(`interactive/watchdog.rs`), P2 self-healer health-score resync
(`event_loop_self_heal.rs`), stuck-cell sweep running every
interactive session by default (`enable_stuck_cell_sweep`, the
NIGHT-hunt-36 gate fix), RIS full-reset keyed to cumulative byte
budget (`terminal/tier2.rs`), write-error recovery with cumulative
write-time accounting (`terminal/io_recovery.rs`), and the §8.5
timestamp-shift family that re-anchors wall-clock subsystems across
pause/resume so no timer freezes or jumps.

## 5 — What was deliberately NOT changed (over-engineering avoided)

- No epoch-rollover scheme to extend `Instant` past ~585 years —
  the kernel clock representation ends there; application-layer
  workarounds cannot out-live the platform's own monotonic domain.
- No widening of `now_secs` to u64 — 136-year wrap in a hash-only
  domain is invisible, and the wider type would suggest a phase
  contract the consumer does not have.
- No f64 promotion for bounded-delta f32 paths (pulse ages, ramps,
  spawn budgets) — their inputs are seconds-scale by construction;
  wider types would add nothing but churn in hot loops.
- No benchmark run — this change is documentation-only (owner rule:
  docs-only changes skip the A/B benchmark), and the 2026-09-21
  record already covers the runtime soak methodology.

## Honest limits

- The ~585-year ceiling is the kernel's `CLOCK_MONOTONIC`
  representation; S3 suspend semantics and NTP steps remain kernel
  contracts (saturating arithmetic keeps the worst case a clamped
  delta, per the prior audit, unchanged).
- Hardware, not software, is the practical million-year wall: no
  terminal, host, or storage medium survives that span. The engine's
  horizon completeness claim is about math, not about machines.
- This audit re-verified the in-process surface at today's HEAD;
  /proc-sourced kernel counters remain kernel-owned, and post-exit
  scrollback remains the documented trust boundary
  (SECURITY_AUDIT.md).

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
