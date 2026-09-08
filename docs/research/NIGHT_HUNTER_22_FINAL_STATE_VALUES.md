<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-22 — the post-exit printer value-structs (the signature family closes)

Owner approval 2026-09-09 (NIGHT-hunter-21 follow-up): the three
remaining `too_many_arguments` suppressions — the post-exit printers —
plus F1 (9x config parse per startup) and F2 (duration dual-field) were
approved as the next hunt batch. This entry covers the printers; F1 and
F2 are separate commits in the same hunt series.

## The wart, quantified

NIGHT-hunter-21 closed the loop-side signatures but deliberately left
three printers out of scope. They were a different wart family —
immutable VALUE tuples, not mutable-state coupling — but the hazard
class was the same: positional parameters the compiler cannot guard.

- `set_final_state` — **25 parameters** (color, scene, charset, speed,
  density, msg_mode, message, message_border, msg_fill_style,
  power_dragon, crystal_dragon, async_mode, intro_color,
  ambient_snapback_secs, ambient_entries, crystal_dragon_secs, fps,
  glitch_level, bold_mode, shading_mode, monolith_size, color_bg,
  color_tune). Called once at loop exit from `event_loop_finalize`.
  Same-typed hazards: `&str` × 8 (glitch/bold/shading/monolith labels
  among them — all Debug-format strings), `Option<&str>` × 2,
  `bool` × 4, `f32` × 2, `Option<f64>` × 2.
- `print_final_runtime_state` — **26 parameters** (the same 23 startup
  baselines + a `start_time: Instant` wedged between parameters 13 and
  14). Called once from `output/post_exit.rs`.
- `print_perf_report` — **10 parameters**, seven of them bare `u64`
  (enc_bytes, enc_flushes, sgr_hits, sgr_misses, tier2_skips,
  tier2_resets, tier2_bytes_since). A transposed pair of those compiles
  and silently mislabels the perf report.

## The fix

### SessionState (new module: src/interactive/final_state.rs)

One point-in-time snapshot of every live-reload-able field — the SAME
23 dimensions at the two moments that matter:

- `SessionState::from_startup(cloud_cfg, color_scheme)` — the
  resolution the session launched with (post CLI > config > scene
  layering). Replaces the 26-param call site in `post_exit.rs`.
- `SessionState::from_live(cloud, cfg, scene_name, charset_preset)` —
  the effective state at loop exit, reading the live Cloud trackers
  (the v80.0.0-beta.2 S-master-HUNT source-alignment rationale for the
  color label moved onto the constructor doc). Replaces the 25-param
  call site in `event_loop_finalize.rs`.

- `set_final_state(state: SessionState)` — 1 parameter; the struct is
  consumed by destructuring, every field moves into its `FINAL_*`
  OnceLock.
- `print_final_runtime_state(startup: &SessionState, start_time)` —
  2 parameters; the `(was X)` diff logic is unchanged, only reading
  `startup.<field>` instead of 23 named parameters.

The entire final-state family (statics, accessors, `fmt_opt_str`, the
printer) moved from `interactive/mod.rs` (794 LOC) into
`final_state.rs` (772 LOC, under the 800 cap); `mod.rs` shrinks to 175
LOC. Facade re-exports keep every historical call path resolving:
`super::set_final_state` (event_loop_finalize),
`crate::interactive::print_final_runtime_state` (post_exit), the
`#[path]` test modules' glob `super::super::*`, and
`super::fmt_opt_str` (tests_fmt_opt).

### TerminalIoStats (event_loop_finalize.rs)

The two tuple getters (`Terminal::encoding_stats` → 4×u64,
`Terminal::tier2_stats` → 3×u64) still return tuples (engine-layer API,
out of scope), but the interactive layer now captures them once as one
named bundle BEFORE `drop(term)`:

- `TerminalIoStats::capture(&term)` — destructure point for both
  tuples; fields are named from here on.
- `print_perf_report(stats, &io, elapsed_s, final_instant_fps)` —
  4 parameters, the destructure at the top of the body keeps the
  report text byte-identical.

### Suppression outcome

`too_many_arguments` in the interactive family: **3 → 0.** Combined
with hunter-21's 13 → 3, the whole interactive family now carries zero
suppressions.

## Bonus warts found on the same surface

1. **Duplicated color-tune label format** — the
   `sat={:.2} bright={:.2} head={:.2} body={:.2} tail={:.2}` literal
   was hand-rolled twice (event_loop_finalize.rs + post_exit.rs). One
   shared `color_tune_label()` helper serves both constructors, so the
   startup and exit sections can never diverge in label shape (the
   `(was X)` diff depends on that).
2. **Duplicate startup-scene source** — post_exit.rs re-derived the
   startup scene from `args.scene` while the identical resolution
   already lived on `cloud_cfg.scene_name` (same derivation in
   build_cloud_cfg). `print_post_exit_verbose` dropped its `args`
   parameter entirely; the snapshot reads the config field. Same
   disease family as F2 (dual-source fields).
3. **`FINAL_GLIITCH_LEVEL` typo** — private static, renamed to
   `FINAL_GLITCH_LEVEL` while it was moving anyway.
4. **Pre-existing gatekeeper debt** — hunter-21's event_loop_ctx.rs
   doc comments carried 4 bold/italic markdown-emphasis markers,
   violating the 2026-09-04 comment-style rule (the gatekeeper was
   red at HEAD; verified by stashing the working tree). Rewritten as
   plain prose.

## Verification

- **Full suite: 2539 passed, 0 failed** (+3 new:
  `session_state_from_startup_maps_config_fields`,
  `session_state_from_startup_labels_custom_palette`,
  `session_state_from_live_reads_cloud_not_config`). The existing
  round-trip test was extended to pin the FULL 23-field storage
  mapping (the v50 accessor family previously had zero round-trip
  coverage) and the full 23-accessor default block.
- **clippy `-D warnings` (all targets): clean.**
- **LOC guard: green** — final_state.rs 772, mod.rs 175, main.rs 800
  (unchanged), event_loop_finalize.rs under cap.
- **Gatekeepers: 10/10 PASS** (after the passing comment-style fix
  above).
- **Behavioral equivalence argument:** every printer field mapping is
  a 1:1 rename (`startup_X` param → `startup.X` field); the diff
  gating logic, the always-print ambient/cadence lines, and the perf
  report text are byte-identical. The round-trip test pins the
  storage mapping; the constructor tests pin both production builders.
- **A/B 10 s benches (pro, dry, cinematic + monolith):**

  | scene    | side       | avg_fps | frame_ms | p99_ms | dirty/frame | entropy_bits | density_gini |
  |----------|------------|---------|----------|--------|-------------|--------------|--------------|
  | cinematic | baseline_A | 29159   | 0.034    | 0.053  | 457.3       | 5.161        | 0.640        |
  | cinematic | after_B    | 28953   | 0.035    | 0.056  | 458.3       | 5.167        | 0.639        |
  | monolith  | baseline_A | 68170   | 0.015    | 0.023  | 140.8       | 4.175        | 0.817        |
  | monolith  | after_B    | 70054   | 0.014    | 0.020  | 140.8       | 4.175        | 0.817        |

  monolith: every visual metric identical to the third decimal; fps
  +2.8% (noise). cinematic: fps −0.7%, entropy/gini/dirty stable to
  ~0.2% — inside the documented shared-VM noise band (27.7K–29.3K
  across consecutive same-binary runs). The bench dispatch never
  constructs the post-exit family (it runs before any terminal/loop
  exists), so this is the owner-rule safety check, not the behavioral
  proof — the behavioral proof is the test suite above. Full JSON:
  `benchmark/bench-labs/night_hunter22/`.

## Files

- `src/interactive/final_state.rs` — NEW (statics + SessionState +
  constructors + set/get accessors + fmt_opt_str + the printer).
- `src/interactive/mod.rs` — family extracted; facade re-exports.
- `src/interactive/event_loop_finalize.rs` — TerminalIoStats,
  4-param printer, SessionState::from_live handoff.
- `src/output/post_exit.rs` — from_startup snapshot; `args` param
  dropped.
- `src/main.rs` — call-site update (unchanged LOC count).
- `src/interactive/event_loop_ctx.rs` — comment-style prose fix
  (pre-existing gatekeeper failure).
- `test/interactive/tests_final_state_cases.rs` — full-coverage
  round-trip + 3 new constructor tests.
- `benchmark/bench-labs/night_hunter22/` — A/B JSON + report.

<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying on
  any number, path, or symbol name here.
-->
