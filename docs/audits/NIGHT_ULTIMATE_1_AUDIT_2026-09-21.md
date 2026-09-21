<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Comprehensive Peak Audit — NIGHT-ultimate-1

Date: 2026-09-21
Scope: the owner's "security / mitigate / LTS / other comprehensive
aspects — should be complete peak" request. This is the umbrella audit
that closes the remaining robustness surfaces NOT already covered by
the prior dedicated audits: runtime panic paths, integer
division/modulo, narrowing casts, unsafe-block soundness, and
allocation explode-at-the-limit guards. Audit baseline: 8d67509
(HEAD after NIGHT-improve-10 / security-3).
Method: automated runtime-vs-test classification of every
panic-adjacent token across all 303 files under `src/` (brace-matched
`#[cfg(test)]` region tracking, comment/string-stripped token scan),
followed by site-level source reads of every surviving runtime
candidate. Owner rules honored: audit-if-peak-skip (no
over-engineering), micro-commit per task, no benchmark for a
docs-only change.

## Verdict

**All surfaces verified at peak — zero defects, zero code changes.**
Every runtime panic candidate is either compile-time-evaluated
(`const _: () = assert!`), dev-build-only (`debug_assert!`),
infallible-by-construction (`Uniform::new` over constant valid
ranges), or gated by an explicit upstream predicate, dispatch guard,
or loop-range contract that makes the panic unreachable. Integer
division and modulo carry zero literal-zero divisors and zero
unguarded variable divisors. The three unsafe blocks are
SAFETY-documented and sound. Allocation sizing is clamped at both
ends (floor 1 column, interactive 1024x500 cap, bench 8K ceiling).
Together with the prior audits referenced in section 6, the
security / mitigation / LTS posture is at complete peak.

## 1 — Runtime panic surfaces (unwrap / expect / panic / assert)

Classifier tally over `src/**` (test regions excluded): 1,354
test-classified hits, 222 runtime-class hits. Of the runtime class:

| Class | Count | Representative sites | Verdict |
|---|---|---|---|
| Compile-time const assertions | ~170 | `central_control_rains/style_rain.rs` physics-constant locks (`const _: () = assert!(...)`) | peak — evaluated at compile time; this IS the hardening technique |
| Infallible range constructions | ~20 | `Uniform::new(0.0, 1.0)`, `Uniform::new_inclusive(0, 23)` etc. across `cloud/mod.rs`, `ecosystem.rs`, `spawn_reset.rs`, `runtime_controls.rs` | peak — constant ranges that cannot fail; the expect documents the invariant |
| `debug_assert!` invariant locks | ~15 | `terminal/sgr_format.rs`, `frame.rs`, `rain_at.rs`, `monolith_helpers.rs`, `hud/colors.rs` | peak — compiled out of release builds; dev-mode tripwires only |
| Predicate-gated expects | 4 | `scene_custom/mod.rs:266/267/451/452` — `key.split_once('.').expect(...)` reached only after `is_profile_config_key` / `is_scene_custom_config_key` performed the identical split successfully (lines 292, 298, 424, 430) | peak — unreachable-by-predicate |
| Dispatch-gated expects | 1 | `bench/run_bench.rs:25` — `cfg.bench_frames.expect(...)` called only inside `if let Some(_bench_frames) = args.bench_frames` (`bench/dispatch.rs:58`) | peak — unreachable-by-dispatch |
| Flow invariants | 4 | `terminal/draw.rs:132/289` ("set above" / "checked above"), `live_config/watcher.rs:255` ("checked is_err above"), `output/report.rs:46` ("just pushed") | peak — Option populated a few lines above in the same expression flow |
| Structural match invariants | 1 | `live_config_trace.rs:162` — `(None, None) => unreachable!()` over keys drawn from `old.keys().chain(new.keys())` | peak — a key in the union exists in at least one map by set semantics |
| Fail-fast static init | 1 | `theme/mod.rs:279` — duplicate-alias `assert_eq!` inside `LazyLock` init over the static `THEMES` table | peak — boot-time integrity lock on compile-time data, fires identically in every environment |
| Bench-internal invariants | 2 | `cloud/spawn.rs:356` (char_pool >= 2 after empty check), `cloud/living_rain.rs:197` (validated constants) | peak — invariant stated and established in the same function |

## 2 — Integer division and modulo

- **Literal-zero divisors**: zero occurrences across all runtime code
  (automated scan, `DIVZERO` class).
- **The `find_inactive_*` rotating-cursor family**: every member —
  quasar (capture x2), physarum, lorenz, solar flare (x2), dna_helix,
  neural (spawn, plasticity), aeolian, dragon, flux, vortex, black
  hole, infall — opens with `let len = ...; if len == 0 { return
  None; }` before the first `% len`. The guard is the family
  contract, verified member-by-member.
- **monolith lane pick** (`monolith.rs:549-560`): modulo expressions
  sit inside loop bodies bounded by `0..len.min(16)` and `0..len` —
  with `len == 0` both ranges are empty and the modulos never
  evaluate.
- **neural wiring** (`network.rs:487-491`): `wire_dst` carries an
  explicit `if n_next == 0 { return 0; }` guard before the modulo.
- **Floored divisors**: `shaders/base/mod.rs:445`
  (`total_cells = (total as i32).max(1)`),
  `glyph/pool_lifecycle.rs:124` (`seed_limit.max(1)`),
  `event_loop_finalize.rs:213/356` (`frames.max(1)`),
  `bench_mem.rs:84` (`if self.samples > 0`), `cli/suggestion.rs:147`
  (`if matches == 0.0`), `boids.rs:437-441` (degenerate-speed early
  return).
- **Constant divisors**: clock tiering (`clock/mod.rs` — YEAR /
  MONTH / DAY / HOUR literals), `BENCH_WARMUP_DIVISOR`,
  `DENSITY_BASE_COLS`, `SPINNER.len()`, `MEMORY_HISTORY_SAMPLES`,
  `MAX_PALETTE_SLOTS`, palette lengths behind const arrays.
- **Kernel syscall results**: `reclaim_state.rs:164` divides by
  `page_size()` (sysconf-backed, kernel-contract nonzero).
- **All remaining division sites are f32/f64**: IEEE-754 division
  cannot panic (worst case inf/NaN); the fps-floor chain that keeps
  `Duration::from_secs_f64(1.0 / effective_fps)` finite is already
  proven in the overflow audit (section 3 of
  OVERFLOW_ENDURANCE_AUDIT_2026-09-21.md).

## 3 — Narrowing casts

Runtime `as u8/u16/u32/i32/usize/f32` sites fall into five
categories, all bounded:

- **Const-table indexing**: `PARALLAX_*` lookup arrays selected by
  `self.layer as usize` (layer is a validated enum-backed index);
  `bolt/mod.rs` u8 tables indexed by construction (0..=255).
- **Validation-gated u8**: `validation/mod.rs:261-262` parses through
  `parse_canonical_u32_range(name, raw, min, max)` BEFORE `as u8` —
  the range check precedes every truncating cast.
- **Saturating float-to-int + clamp**: `sensor/mod.rs:215/263`
  (`(raw.round() as u8).clamp(POINT_MIN, POINT_MAX)` — Rust
  float-to-int casts saturate since 1.45, and the clamp follows).
- **Bounded calendar math**: `posix_time.rs` casts of hour/minute/
  second (each < 60), day-of-year, month — all bounded by their
  modulus expressions.
- **Bounded geometry**: `phosphor.rs` / `rain_post.rs` /
  `phosphor_anomaly.rs` casts of `dirty_idx % frame_width` results —
  the modulo result is strictly < frame_width, which the Frame
  constructor clamps to `MIN_TERMINAL_COLS..max_cols`.
- **Fallback-guarded**: `hud/metrics.rs:387`
  (`.unwrap_or(HUD_MIN_WIDTH as usize) as u16`).

## 4 — Unsafe-block soundness

Three unsafe sites exist in `src/` (the renderer hot path contains
none — `diagnostics/info.rs` documents this contract):

- `diagnostics/alloc_trace.rs` — `GlobalAlloc` wrapper: forwards
  every argument unchanged to `System`, adds only `Relaxed` atomic
  counter bumps. Safety obligations preserved by construction.
- `platform/fork_guard.rs` — libc orphan-terminal-restore guard:
  TTY-gated entry, `tcgetattr` rc-checked before `assume_init`,
  `sigemptyset` initializes the sigset before use, the child never
  returns into Rust flow, and the fork-vs-prctl race plus the
  kernel reparent window are explicitly handled (capture-ppid-first
  strategy, NIGHT-hunt-47-depthbore). LTS-relevant: this is the
  crash-recovery path that restores the terminal when the renderer
  is SIGKILLed.
- `diagnostics/mod.rs` — macOS `sysctlbyname` two-call pattern
  (length query, rc-checked, then fetch into a sized buffer).

The archived `docs/archive/audits/UNSAFE_SOUNDNESS_AUDIT.md` covers
the historical verification; this audit re-verified the current
shape of all three sites.

## 5 — Allocation explode-at-the-limit guards

The "explode data when reaching the limit" class for memory:

- **Frame constructor** (`frame.rs:128`):
  `width.clamp(MIN_TERMINAL_COLS, max_cols)` with
  `MIN_TERMINAL_COLS = 1` (types/constants.rs:122) — a zero or
  hostile terminal-size report cannot produce a zero-width grid
  (division/modulo safety) or an unbounded one.
- **Interactive ceiling**: 1024 x 500 safety cap on the interactive
  path — allocation per frame is bounded regardless of what the
  terminal reports.
- **Bench ceiling**: `BENCH_MAX_COLS = 7680`
  (types/constants.rs:109) — the benchmark intentionally exceeds the
  interactive cap up to 8K UHD and no further.
- **Reset path mirrors the clamp** (`spawn_reset.rs:56`): resize
  re-clamps before any buffer sizing.
- Per-container bounded-growth contracts (rings, caps, expiry,
  dedup) are catalogued in section 2 of
  OVERFLOW_ENDURANCE_AUDIT_2026-09-21.md — not re-audited here.

## 6 — Prior-audit coverage map (not re-audited)

| Aspect | Authoritative record |
|---|---|
| Integer overflow / endurance / time arithmetic / f64 drift | docs/audits/OVERFLOW_ENDURANCE_AUDIT_2026-09-21.md (NIGHT-improve-10 / security-3) |
| Anti-copy surface, mouse capture, paste/OSC 52, terminal trust boundary | docs/SECURITY_AUDIT.md (NIGHT-improve-8 + follow-up d2006c2) |
| LTS resource-stability methodology (RSS/fd/handles, 24 h+) | docs/ENDURANCE.md, docs/audits/LTS_FINAL_AUDIT_2026-09-11.md |
| Historical security/unsafe/LTS campaigns | docs/archive/audits/ (SECURITY_VULNERABILITY_AUDIT, UNSAFE_SOUNDNESS_AUDIT, S3_SECURITY_LTS_HARDEN, LTS_DEEP_AUDIT_3STAGE, TRIPLE_ENGINE_LTS_AUDIT, and the rest) |

## 7 — What was deliberately NOT changed (over-engineering avoided)

- No conversion of invariant-backed `expect`s to `ok_or` error
  plumbing — each is guarded by a predicate, dispatch arm, or flow
  invariant verified above; replacing them would add dead error
  paths that can never execute.
- No `checked_div`/`checked_rem` on the family modulo sites — the
  `len == 0` early-exits already make zero divisors unreachable, and
  the hot loops must stay branch-minimal (the rotating-cursor scan
  is the documented amortization contract).
- No restructuring of `const _: () = assert!` physics locks — they
  are compile-time, zero-cost, and already the strongest form of the
  invariant.
- No additional clamp on narrowing casts that are already
  validation-gated, saturating, or modulo-bounded.
- No benchmark run — this change is documentation-only (owner rule:
  docs-only changes skip the A/B benchmark).

## Honest limits

- The classifier's runtime/test split is brace-matched but
  attribute-position-based; a pathological test module declared
  mid-file with runtime code after it would still be attributed
  correctly (regions are per-item), but exotic macro-generated test
  harnesses could in theory evade region tracking. Every surviving
  runtime candidate was nonetheless read at the source level, so
  the verdict does not rest on classification alone.
- `debug_assert!` sites are dev-build tripwires: a debug build can
  panic where release builds cannot. That is the intended contract
  (fail fast on invariant breach during development).
- External-boundary behavior (kernel sysconf values, libc signal
  semantics, terminal-reported sizes) is trusted per OS contract;
  the clamps in section 5 bound the application's exposure to those
  boundaries.
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
