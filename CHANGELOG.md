# Changelog
<!-- SPDX-License-Identifier: GPL-3.0-only -->

Cosmostrix uses [SemVer](https://semver.org/) for package versions (e.g. `4.0.0`).
Git tags and GitHub Releases use a leading `v` (e.g. `v4.0.0`).
Stable releases do not use `-stable.N` suffixes.

All notable changes to this project are documented in this file.

---

## v25.0.0-alpha.7 — Full-Codebase Dead-Code Sweep (Flat src/ Files)

### Headline

Seven-commit sweep of all flat `src/*.rs` files (everything outside
`cloud/`, `config/`, and `interactive/` which were audited in
alpha.4–alpha.6). The audit targeted every `#[allow(dead_code)]`
attribute in the codebase — 40+ attrs were investigated, classified,
and either removed (genuinely dead), gated with `#[cfg(test)]`
(test-only), or confirmed as stale (stripped). After this sweep, the
codebase has **zero `#[allow(dead_code)]` attributes that paper over
genuinely dead code**. The only remaining allows are 5 legitimate
`clippy::too_many_arguments`, 2 `unreachable_patterns` catch-all
guards, 1 `private_interfaces`/`struct_excessive_bools`, 1
`clippy::needless_late_init`, 1 platform-specific stub, and 1 re-export
`unused_imports`.

### Commit 1 — Dead Functions with Zero Callers (`bb8b302`)

Removed 10 dead functions/constants with zero callers anywhere in
`src/`, `tests/`, or `benches/`:

- `ux::warn()` — non-fatal stderr helper, never adopted
- `report::Report::field_if()` — conditional field adder, zero callers
- `palette::apply_brightness(Color, f32)` — Color-enum version, _rgb
  variant is used instead
- `palette::apply_saturation(Color, f32)` — zero callers
- `constants::HEAD_FRACTION` — informational only, never enforced
- `constants::QUANTUM_RIPPLE_RAIN_INTERACTION_SECS` — "reserved for
  future use"
- `scene_custom::validate_scene_custom_name()` — speculative "Stage 3+"
- `droplet::Droplet::is_head_bright()` — replaced by inline cached
  local
- `terminal::Terminal::capabilities()` — zero callers
- `color_cache::ColorCache::len()` — gated with `#[cfg(test)]` (4 test
  callers found after initial removal)

### Commit 2 — Test-Only Function Gating (`bb1a108`)

Gated 13 test-only functions/constants with `#[cfg(test)]`:

- `central_colors::has_theme()` + `theme_count()` — test-only
- `frame::Frame::get()` — test-facing accessor
- `scene_custom::is_valid_custom_scene_name()` +
  `validate_custom_scene_name()` — test-only
- `profile::atmosphere_presets_section()` + `list_profiles_text()` —
  test-only (--list-profiles removed in v14)
- `output::BRAND_PURPLE_RGB` + `ERROR_RGB` + `WARN_RGB` — test-only
  RGB constants
- `bench_meta::DRAW_RATIO_MEANING` + `DIRTY_ALL_FRAMES_MEANING` +
  `ESTIMATED_FULL_REDRAW_MEANING` — test-only meaning strings

### Commit 3 — Dead EventCtx Fields (`8057991`)

EventCtx carried 7 fields but only `cols` and `lines` were read by
event render methods. Removed 5 dead fields (`bg`, `palette_colors`,
`now`, `message_bounds`, `has_message`) and the `'a` lifetime
parameter. Also removed the dead `msg_bounds` computation block (21
lines) and `palette_slice` bindings in rain.rs.

### Commit 4 — Dead CliExplicit Fields + Stale Allows (`17b957e`)

- Removed `CliExplicit::scene_custom` and `CliExplicit::monolith_size`
  — never read, "tracked for completeness and future use"
- Stripped stale `#[allow(dead_code)]` from `CloudConfig::atmosphere_mode`
  — IS read by event_loop.rs:367
- Removed `BenchReportData::estimated_full_redraw_frames` and
  `estimated_full_redraw_ratio_percent` — computed but never output
  by either `build_premium_report()` or `build_json_string()`. Also
  removed the computation in bench.rs (both code paths), the
  `estimates_full_redraw` import, the test data, and the
  REQUIRED_FIELDS entries. Gated `cinematic::estimates_full_redraw()`
  with `#[cfg(test)]` (only test callers remain).

### Commit 5 — Vendor Detection + Dead Methods + Stale Allows (`340957f`)

- **termdetect.rs**: Removed entire dead vendor detection system —
  `TerminalVendor` enum (9 variants), `vendor` field in `TerminalCaps`,
  30+ lines of env-var detection logic, `Display` impl. The vendor was
  detected but never read by any production code.
- **central_colors.rs**: Removed dead `AnsiWithC16` enum variant —
  never constructed by any theme, match arm was a dead branch.
- **atmosphere_adaptive.rs**: Removed dead `AdaptiveParams::identity()`
  — zero callers ("surfaced for future fallback paths").
- **charset_custom.rs**: Removed dead `CharsetCustomDef::is_empty()`.
- **colors_custom.rs**: Removed dead `CustomPaletteDef::is_empty()`.
- **terminal.rs**: Removed dead `push_u8` re-export — zero external
  callers.

### Commit 6 — Dead signal_exit Field (`e21ee03`)

Removed `signal_exit: Arc<AtomicBool>` from `Terminal` struct. The
field was stored but never read — not by any method, not by the Drop
impl. The event loop keeps its own `Arc<AtomicBool>` and polls it
directly. Constructor signature unchanged for caller compatibility
(parameter prefixed with `_`).

### Commit 7 — Dead Theme Category/Description System (`c6e1235`)

Purged the entire v14-era `--list-colors-detail` remnant:

- `ThemeCategory` enum (9 variants) + `impl ThemeCategory` (label
  method)
- `THEME_CATEGORIES` const
- `category` and `description` fields from `ThemeInfo` struct
- `detail_list_text()` function
- `detailed_color_list_includes_categories_and_canonical_themes` test
- 4 `#[allow(dead_code)]` attrs
- `category:` and `description:` field assignments from all 44 THEMES
  entries
- Invalid `#[must_use]` on test module (pre-existing, surfaced after
  allows removed)

ThemeInfo is now a 3-field struct (name, scheme, aliases). 1138 tests
(was 1139 — one test removed with `detail_list_text`).

### What Remains (Legitimate Allows)

After this sweep, 13 `#[allow(...)]` attrs remain, all legitimate:

| Attr | Count | Reason |
|------|------:|--------|
| `clippy::too_many_arguments` | 5 | Functions with many params (design choice) |
| `unreachable_patterns` | 2 | Catch-all guards for future crossterm variants |
| `clippy::needless_late_init` | 1 | Style preference |
| `private_interfaces, struct_excessive_bools` | 1 | Structural |
| `dead_code` (bench_perf non-Linux stub) | 1 | Platform abstraction |
| `unused_imports` (bench_report re-export) | 1 | `pub(crate) use` re-export pattern |
| `clippy::module_name_repetitions` | 0 | Removed with validate_scene_custom_name |

### Cumulative Impact

Across alpha.4–alpha.7 (4 audit passes):

- **cloud/** (alpha.4): −134 lines, 2 commits
- **config/** (alpha.5): −3 lines, 2 commits
- **interactive/** (alpha.6): −17 lines, 4 commits
- **flat src/** (alpha.7): −490 lines, 7 commits
- **Total**: −644 lines of dead code, 15 commits, zero behavior change

1138 tests pass (was 1140 at start — 2 dead-constant bounds-check
tests removed alongside their constants).

---

## v25.0.0-alpha.6 — Interactive Subsystem Dead-Code Audit

### Headline

Four-pass dead-code audit of the `interactive/` subsystem (13 files,
6,142 LOC: `activity.rs`, `adaptive.rs`, `bg_fill.rs`, `event_loop.rs`,
`hud.rs`, `input.rs`, `intro.rs`, `intro_cosmic.rs`, `intro_logo.rs`,
`mod.rs`, `signal_handlers.rs`, `tests.rs`, `watchdog.rs`) purged
**all 12 `#[allow(dead_code)]` attributes** from the subsystem. The
compiler was silent (zero warnings on `cargo build --release` and
`cargo clippy --all-targets`), so the audit targeted the allow attrs
directly — each one was investigated to determine whether it papered
over genuinely dead code, test-only code, or stale scaffolding. Four
commits, 1139 tests pass (was 1140 — one dead-constant bounds-check
test was removed alongside its constant).

### Pass 1 — intro_logo.rs Constants + Stale Allows (`951232e`)

`intro_logo.rs` (1,149 LOC — the largest file in the subsystem)
carried 9 `#[allow(dead_code)]` attributes on constants and functions.
Audit found 2 genuinely dead constants, 1 test-only constant, 4 stale
allows on live code, and 1 stale allow on a live function.

**Removed entirely (zero callers anywhere):**

- `FLASH_DECAY_RATE` (`f32 = 4.0`) — documented as "ignition flash
  decay rate" but never read. The ignition flash at the Phase 2
  boundary uses inline `phase_t` interpolation (lines 476, 492), not
  this constant. Aspirational scaffolding from an earlier design
  iteration.
- `FADEIN_STEPS` (`u32 = 32`) — documented as "logo appears in N
  reveal steps spread across Phase 1" but the rendering loop uses
  continuous `phase_t` interpolation with `PHASE1_FADEIN_END_MS`,
  not discrete steps. The only consumer was
  `fadein_steps_is_reasonable()` — a compile-time bounds-check test
  that verified a constant nothing reads. Removed both the constant
  and the test.

**Gated with `#[cfg(test)]` (test-only):**

- `LOGO_COLOR` (`Color` enum form) — the doc explicitly states
  "rendering uses `LOGO_COLOR_RGB` for cheaper lerp math." The enum
  form exists only so `logo_color_matches_rgb_constant()` can verify
  the two forms agree. Production rendering reads `LOGO_COLOR_RGB`
  directly.

**Stripped stale `#[allow(dead_code)]` (all have production callers):**

- `DISSOLVE_SPEED_MIN` — read at line 665 (`spawn_rain_droplet` speed
  lerp) and 3 test assertions.
- `DISSOLVE_SPEED_MAX` — same, line 665 + 3 test assertions.
- `JITTER_VX` — read at line 672 (`spawn_rain_droplet` vx computation)
  and 1 test assertion.
- `spawn_rain_droplet()` — called at line 601 (Phase 4 rain spawn
  loop) and 2 test sites. The allow was stale from when the function
  was scaffolded before the Phase 4 rendering path was wired in.

LOC impact: −27 lines. Zero behavior change.

### Pass 2 — Test-Only Accessor Gating (`eef3f0f`)

Three accessor methods carried `#[allow(dead_code)]` because they
were called only from test modules but compiled into production
builds. Replaced the allow attrs with `#[cfg(test)]` so the methods
only exist in test builds.

1. **`intro::ParticlePool::active_count()`** (`pub(super) fn`) —
   Called from 11 test sites across `intro.rs` (3), `intro_cosmic.rs`
   (4), and `intro_logo.rs` (4). ALL inside `#[cfg(test)] mod tests`
   blocks. Production rendering uses the free-list length directly
   when deciding whether to spawn particles.

2. **`activity::idle_resync_due()`** (`pub(super) fn`) — Called from
   3 test sites in `interactive/tests.rs`. Production `event_loop.rs`
   inlines the idle-and-interval check directly rather than calling
   this helper.

3. **`hud::Hud::visible()`** (`pub(crate) fn`) — Called from 4 test
   sites in `hud.rs`'s test module. Production code reads the
   `visible` field directly (6 call sites) — cheaper than a method
   call in the hot render path.

All three methods now have doc comments explaining why they are
test-only and what production code does instead.

LOC impact: +12 lines net (added doc comments). Zero behavior change.

### Pass 3 — EnduranceHealth Stale Allow (`7a56be8`)

The `EnduranceHealth` struct in `adaptive.rs` carried
`#[allow(dead_code)]` on the struct declaration. Investigation
confirmed all 7 fields are read on every platform:

- `rss_samples` — read by `rss_mean()` and `rss_variance()` in
  `recompute()`
- `rss_idx` — written by `new()` and `push_rss()` (Linux only)
- `rss_count` — read by `recompute()` guard, `rss_mean()`,
  `rss_variance()`
- `frame_jitter_ema` — read by `recompute()` and `push_frame_time()`
- `ctxt_switch_ema` — read by `recompute()` and `push_ctxt_rate()`
  (Linux only)
- `score` — read by `recompute()`, `score()`, `classification()`
- `updates` — read/written by `push_frame_time()` and `recompute()`

On non-Linux platforms, `push_rss()` and `push_ctxt_rate()` are
`#[cfg(target_os = "linux")]`-gated out, but `new()` still initializes
all fields and `recompute()` still reads them (returning early via
the `rss_count < MIN_SAMPLES` guard). The compiler sees all fields as
read — no `dead_code` warning is generated on any platform. The allow
was likely added during initial scaffolding before `recompute()` was
fully implemented.

LOC impact: −1 line. Zero behavior change.

### Pass 4 — hint_reclaim_pages Stale Allow (`596aa1c`)

The `#[cfg(not(target_os = "linux"))]` no-op stub of
`hint_reclaim_pages()` in `adaptive.rs` carried `#[allow(dead_code)]`.
The function IS called unconditionally from `event_loop.rs:528` on ALL
platforms — on non-Linux, the call resolves to this stub. It is not
dead code by any definition; the allow was stale from when the stub
was scaffolded before the `event_loop` call site landed. The `_ptr`/
`_len` parameter prefixes already suppress unused-parameter warnings.

LOC impact: −1 line. Zero behavior change.

### What Was NOT Touched

- **`#[allow(clippy::too_many_arguments)]`** on `intro.rs:423` and
  `input.rs:88`: Legitimate clippy lint suppressions for functions
  with many parameters, not dead code. Refactoring the argument lists
  is a design change, not cleanup.
- **`#[cfg(target_os = "linux")]`-gated methods** (`push_rss`,
  `push_ctxt_rate`): These are platform abstractions, not dead code.
  They compile only on Linux and are called from `event_loop.rs` under
  the same cfg gate.
- **Test files** (`tests.rs`, and the `#[cfg(test)] mod tests` blocks
  in each production file): Out of scope — tests are the consumers
  that justify the API, not targets for removal.

### Methodology

Same proven methodology as the cloud/ (alpha.4) and config/
(alpha.5) audits:

1. **Compiler-driven**: `cargo build --release` and
   `cargo clippy --all-targets --release` — both clean, zero warnings.
2. **Allow-attr sweep**: Extracted all 14 `#[allow(...)]` attrs from
   the 13 files. Categorized: 12 `dead_code`, 2
   `clippy::too_many_arguments`. The 12 `dead_code` attrs were the
   audit targets.
3. **Grep-verified**: For each `dead_code` allow, searched for callers
   across all of `src/` to classify as genuinely-dead, test-only, or
   stale-allow-on-live-code.
4. **Cascade-aware**: After gating `active_count()` as `#[cfg(test)]`,
   verified all 11 test callers are inside `#[cfg(test)]` blocks
   (confirmed — 3 files, all test modules).
5. **Test-site sweep**: After removing `FADEIN_STEPS`, verified the
   only test consumer (`fadein_steps_is_reasonable`) was removed in
   the same commit.

### Subsystem Health

After this audit, `interactive/` has **zero `#[allow(dead_code)]`
attributes**. The only remaining allows are 2 legitimate
`clippy::too_many_arguments` suppressions. The subsystem is clean.

---

## v25.0.0-alpha.5 — Config Subsystem Dead-Code Audit

### Headline

Two-pass dead-code audit of the config subsystem (7 production files,
5,751 LOC: `config.rs`, `config_apply.rs`, `configfile.rs`,
`config_hints.rs`, `live_config.rs`, `live_config_poll.rs`,
`live_config_trace.rs`) removed duplicated logic and dead test-only
code. The compiler was silent (zero warnings on `cargo build --release`
and `cargo clippy --all-targets`), so the audit used the proven
methodology from the cloud/ audit (alpha.4): compiler-driven →
grep-verified → cascade-aware → test-site sweep. Two commits, all 1140
tests still pass.

### Pass 1 — Termux Detection DRY Consolidation (`070870b`)

`configfile::is_termux_environment()` has existed since v25.2 as the
canonical Termux detection heuristic (`TERMUX_VERSION` env var or
`PREFIX` containing `"com.termux"`). Its doc comment claimed "this
matches the detection used elsewhere in the codebase (safepath.rs,
verbose.rs, event_loop.rs)" — but `safepath.rs` and `verbose.rs`
inlined their own copies of the same env-var check, and `event_loop.rs`
had no such check at all (stale reference).

Two copies of the duplicated logic existed:

- `src/safepath.rs:187-188` — inline check before pushing `/sdcard/`
  to the allowed-prefixes list
- `src/verbose.rs:237-238` — inline check for the `android:` verbose
  env-dump line

Both were semantically identical to `is_termux_environment()` (only
cosmetic difference: the originals used `.is_ok_and()` while the
canonical version uses `.map().unwrap_or(false)` — same semantics).

Both inline copies were replaced with calls to
`crate::configfile::is_termux_environment()`. This:

1. Eliminates two copies of the detection heuristic — if the env-var
   probe ever needs to change (e.g. a new Termux version sets a
   different var), it changes in one place.
2. Justifies the `pub` visibility on `is_termux_environment()` — it
   now has real external callers instead of being effectively dead
   public API.
3. Replaces the stale `event_loop.rs` reference in the doc comment
   with an accurate "single source of truth" statement.

LOC impact: −1 line net (10 insertions, 11 deletions). Zero behavior
change.

### Pass 2 — Test-Only Utility Cleanup (`9a0aba3`)

Two config utilities carried `#[allow(dead_code)]` attrs because they
were called only from test modules but compiled into production
builds. Replaced the allow attrs with `#[cfg(test)]` so the functions
only exist in test builds — eliminating dead code from production
without changing test behavior.

**`live_config::drain_validation_rejections()`** — Called from 11
sites in `live_config.rs`, ALL inside `#[cfg(test)] mod tests`. The
doc comment (v25.13, bug #15) admitted the production drain was
removed when `main.rs` switched to exit-on-first-error, but retained
the function "as a debug hook for future tooling that may want to
inspect the session log" — speculative retention of exactly the kind
the cloud/ audit rejected. Rewrote the doc comment to state plainly
that this is a test utility for verifying `validate_and_send` recorded
a rejection.

**`configfile::config_file_path_from()`** — A test-convenience wrapper
around the private `config_file_path_from_env()` that takes owned
`Option<String>` instead of `Option<&str>`. Called from 2 test sites
in `configfile.rs`. Production code (line 336) calls
`config_file_path_from_env()` directly. The wrapper exists only so
tests can pass owned strings without `as_deref()` at every call site.
Kept the existing `#[cfg(not(target_os = "windows"))]` platform gate
and `#[must_use]`; the function is now
`#[cfg(all(not(target_os = "windows"), test))]`.

LOC impact: −2 lines net (6 insertions, 8 deletions). Zero behavior
change in either test or production builds.

### What Was NOT Touched

- **`push_validation_rejection()` write-side calls**: The 3 production
  calls in `validate_and_send` (lines 491, 512, 558) push to
  `LIVE_RELOAD_VALIDATION_REJECTIONS` — a log that is now provably
  never drained in production (the only reader, `drain_validation_rejections`,
  is `#[cfg(test)]`). These are cheap writes (one mutex lock, vec push,
  capped at 64) and serve as debug breadcrumbs. Removing them is a
  behavior-adjacent change deferred to a separate audit.
- **`#[allow(clippy::too_many_arguments)]` on `live_config.rs:349`**:
  Legitimate clippy lint suppression for a function with many
  parameters, not dead code. Refactoring the argument list is a
  design change, not cleanup.
- **Latent Windows test-compilation issue**: The two tests calling
  `config_file_path_from` (lines 899, 905) are not gated with
  `#[cfg(not(target_os = "windows"))]`, so they would fail to compile
  on Windows test builds. This is pre-existing and not caused by this
  audit. Fixing it is a separate concern.
- **Test files** (`config_apply_tests.rs`, `config_apply_profiles_tests.rs`,
  `configfile_promotion_tests.rs`, `configfile_bug7_tests.rs`): Out of
  scope for a dead-code audit — tests are the consumers that justify
  the production API, not targets for removal.

### Methodology

Same proven methodology as the cloud/ audit (alpha.4):

1. **Compiler-driven**: `cargo build --release` and
   `cargo clippy --all-targets --release` — both clean, zero warnings.
2. **Grep-verified**: Extracted all `pub fn`, `pub(crate) fn`,
   `pub struct`, `pub enum`, `pub const` names from the 7 production
   files and checked each for external callers via `rg`. Found 5
   candidates with zero external refs; 4 confirmed (1 was a false
   positive matching a URL in a doc comment).
3. **Cascade-aware**: After gating `drain_validation_rejections` as
   `#[cfg(test)]`, verified no production code reads
   `LIVE_RELOAD_VALIDATION_REJECTIONS` (confirmed — the write-side
   calls are now the only production touch).
4. **Test-site sweep**: Verified all test callers of the gated
   functions are inside `#[cfg(test)] mod tests` blocks.

---

## v25.0.0-alpha.4 — Cloud Dead-Code Audit

### Headline

Two-pass dead-code audit of the `cloud/` subsystem (11,866 LOC across
14 source files plus 7 test files) removed **134 lines** of dead code
across 10 files. Every removal was confirmed unused via compiler
warnings and `rg` grep — no behavior change, all 1140 tests still pass.
The audit also corrected a misleading `#[allow(dead_code)]` attribute
on `DrawCtx::flash_time` that had been papering over a genuinely dead
field since the field was introduced.

### Pass 1 — Atmospheric Event Engine (`45c641c`)

The atmospheric event engine (`src/cloud/atmospheric_events.rs`) is the
cinematic ghost-event system that spawns fading kanji characters on dim
rain cells. The trait and lifecycle enum had aspirational scaffolding
for a 6-phase pipeline (`Idle → Pending → Spawn → Active → Decay →
Finished → Idle`) that was never implemented — only `Active` and
`Decay` are actually produced by event implementations, and
`is_finished()` drives recycling.

Changes:
- **`EventState`**: removed `Idle`, `Pending`, `Spawn`, `Finished`
  variants. Only `Active` and `Decay` are set or compared anywhere in
  the codebase (4 call sites in `atmospheric_events.rs`, 1 in
  `events/ghost.rs`).
- **`AtmosphericEvent` trait**: removed `phase_durations_ms()` and
  `memory_footprint()` methods. Both had `#[allow(dead_code)]` since
  inception and zero callers anywhere in `src/`, `tests/`, or `benches/`.
  The `GhostEvent` impl returned hard-coded stubs `(2000, 2000)` and
  `128` — not real measurements.
- **`AtmosphericEventManager`**: removed `active_count()` method.
  Compiler confirmed it is unused after the trait methods were removed.
- Removed 4 `#[allow(dead_code)]` attributes that were papering over
  the above.
- Updated module doc: replaced the aspirational lifecycle diagram with
  the actually-implemented `Active → Decay → (recycle when
  is_finished())` and a note explaining the audit.

Changes (`events/ghost.rs`):
- Removed `phase_durations_ms()` impl (returned `(2000, 2000)` stub).
- Removed `memory_footprint()` impl (returned `128` stub).

LOC impact: −28 lines. Trait surface shrinks from 8 methods to 6 —
easier to implement new event types.

### Pass 2 — Unused Accessors + Cascade Cleanup (`0832b5c`)

Four unused accessor methods plus the cascade of dead fields they were
keeping alive.

Primary removals (each had `#[allow(dead_code)]` with zero callers):
- **`LivingRain::multiplier()`** — read-only accessor for the gust
  multiplier. The draw path calls `LivingRain::sample()` which
  advances the state machine and returns the multiplier; the read-only
  accessor was never used.
- **`Cloud::cycle_profile()`** — manually cycles `BehaviorProfile` to
  the next variant. Zero callers anywhere. The autonomous ecosystem
  evolves profiles via lerp transitions, not manual cycling.
- **`DrawCtx::is_bright()` / `DrawCtx::is_dim()`** — per-cell glitch
  phase queries. A comment at `rain.rs:361` documents these were
  replaced by cached `glitch_bright` / `glitch_dim` snapshots computed
  once per `DrawCtx` construction (avoids per-cell
  `Instant::saturating_duration_since` + nanos conversion in
  `get_attr`'s glitch branch, called 100–300×/frame when glitchy).

Cascade cleanup (dead code surfaced by the above removals):
- **`BehaviorProfile::cycle()`** — was only called by
  `Cloud::cycle_profile()`. With its sole caller removed, the compiler
  flagged it as dead. Removed the 8-variant match + method.
- **`DrawCtx::last_glitch_time`, `next_glitch_time`,
  `glitch_inv_between`** — were only read by `is_bright()` /
  `is_dim()`. With both methods removed, the compiler flagged all
  three fields as never read. The `Cloud` struct keeps its own copies
  (used to compute the `glitch_bright` / `glitch_dim` snapshots in
  `rain.rs`); only the `DrawCtx` mirror copies were dead.
- **`DrawCtx::flash_time`** — had `#[allow(dead_code)]` with a
  misleading "Kept for API compatibility" comment. Verified via `rg`
  that no `ctx.flash_time` / `draw_ctx.flash_time` reads exist
  anywhere; the field was only constructed (`rain.rs:425`) but never
  consumed. The precomputed `flash_elapsed` field is what per-cell
  logic reads. Removed the field, the construction copy, and the
  misleading comment + allow attribute.
- **`std::time::Instant` import in `render.rs`** — became unused after
  the three `Instant`-typed `DrawCtx` fields were removed.

Test sites updated (6 `DrawCtx` constructions across 3 test files):
- Removed 3 glitch-timing field literals per site (18 lines total)
- Removed 1 `flash_time: None` literal per site (6 lines total)

LOC impact: −96 lines across 8 files. `DrawCtx` shrinks by 4 fields
(3 glitch timing + `flash_time`) — smaller struct, better cache
locality in the per-cell `get_attr` hot path.

### Verification

Both passes verified via:
- `cargo build --release` — OK, zero warnings (the cascade warnings
  surfaced by pass 2 were resolved in the same commit)
- `cargo test --all` — 1140 passed, 0 failed
- `cargo clippy` — clean
- `cargo fmt` — clean
- `./scripts/build.sh check-all` — All quality checks passed

### Audit Methodology

1. **Compiler-driven**: built with `cargo build --release` and captured
   all `warning: method/field/variant is never used` diagnostics. The
   compiler is the source of truth — manual `#[allow(dead_code)]`
   attributes were treated as suspect.
2. **Grep-verified**: every removal was confirmed via `rg` to have zero
   callers in `src/`, `tests/`, and `benches/`. The `ctx.field` /
   `draw_ctx.field` access patterns were distinguished from `self.field`
   access on the source struct (`Cloud`) to avoid false positives.
3. **Cascade-aware**: after each removal, rebuilt to surface newly-dead
   code that had been kept alive by the removed code. Pass 2's cascade
   cleanup (3 `DrawCtx` fields + `BehaviorProfile::cycle` + unused
   import) was discovered this way.
4. **Test-site sweep**: all `DrawCtx` construction sites in tests were
   updated to match the new field set, then re-verified by the full
   test suite.

### What Was NOT Touched

- `EventCtx` struct in `atmospheric_events.rs` retains its
  `#[allow(dead_code)]` — its fields are populated by
  `AtmosphericEventManager` but not yet consumed by the current
  `GhostEvent` impl. This is intentional scaffolding for future event
  types and was left untouched.
- `glitch_bright` / `glitch_dim` cached snapshots in `DrawCtx` — these
  ARE read per-cell at `render.rs:277,280` and are the replacement for
  the removed `is_bright()` / `is_dim()` methods.
- `Cloud::last_glitch_time` / `next_glitch_time` (on the `Cloud`
  struct, not `DrawCtx`) — used to compute the cached snapshots and to
  drive glitch spawn timing in `spawn.rs:352`.

---

## v25.0.0-alpha.3 — Legacy `--fullwidth` Parameter Purge

### Headline

The legacy `--fullwidth` CLI flag (and its `fullwidth = false` config.toml
equivalent) has been removed from the codebase. This was a hidden
(`hide = true`), never-documented, never-defaulted-true flag that doubled
the column stride for monolith streams — rendering each single-width
glyph at 2 cells visually. Its misleading help text ("Use full terminal
width") suggested a different behavior than what it actually did, and
the only mode it ever enabled was never the default. The Cosmic Dragon
principle forbids wide chars permanently; the charset is always
single-width, so the flag was a vestigial horizontal-spacing mode
without a real use case.

### Changes

- **CLI flag `--fullwidth` / `-F` removed** (`src/config.rs`). The flag
  was `hide = true` (never shown in `--help`), never defaulted to `true`,
  and never documented publicly. Users who try the old flag now get a
  clear migration error via the `REMOVED_FLAGS` table in
  `src/validation.rs` ("error: --fullwidth has been removed in
  v25.0.0-alpha.3...").

- **`CloudConfig::fullwidth` field removed** (`src/app.rs`). All call
  sites that read this field (3 in `interactive/event_loop.rs`, 3 in
  `bench.rs`, 1 in `main.rs`) were updated.

- **`Cloud::full_width` field removed** (`src/cloud/mod.rs`). The
  `Cloud::new()` constructor signature drops the `full_width: bool`
  parameter — 11 test call sites updated.

- **`DrawCtx::full_width` field removed** (`src/cloud/render.rs`). All
  DrawCtx construction sites and dead-code branches that read
  `ctx.full_width` were updated (5 sites in `cloud/rain.rs`, `droplet.rs`,
  `cloud/monolith.rs`).

- **`MonolithRain` simplified** (`src/cloud/monolith.rs`):
  - `MonolithSpawnParams::full_width` field removed.
  - `MonolithRain::reset(cols, full_width)` → `reset(cols)`.
  - `find_inactive_lane(full_width, ...)` → `find_inactive_lane(...)`.
  - `lane_is_available(lane, _full_width, ...)` → `lane_is_available(lane, ...)`.
  - `lane_count(cols, full_width)` → `lane_count(cols)` (always returns
    `cols.max(1)`).
  - `lane_col(lane, full_width)` → `lane_col(lane)` (always returns
    `lane as u16`).
  - Dead-code branches removed:
    - `if ctx.full_width && cell.col + 1 < frame.width { clear_cell(...) }`
      in `MonolithRain::draw`.
    - `if ctx.full_width && stream.col + 1 < frame.width { frame.set(...) }`
      in `draw_segments`.
    - `#[allow(clippy::too_many_arguments)]` on `find_inactive_lane`
      removed (now 6 args, below the 7+ threshold).

- **`Droplet::draw` simplified** (`src/droplet.rs`): removed the
  `if ctx.full_width && self.bound_col + 1 < frame.width` block that
  wrote a blank cell to the right of each glyph-mode droplet.

- **`Cloud::spawn_droplets` simplified** (`src/cloud/spawn.rs`): removed
  the `if self.full_width { col &= 0xFFFE; }` column-stride adjustment
  in both the main spawn loop and the warm-start seeding loop. The
  `let mut col` was demoted to `let col` since it's no longer mutated.

- **Density helpers simplified** (`src/app.rs`):
  - `auto_density_factor(cols, fullwidth)` → `auto_density_factor(cols)`.
    The `eff_cols = (cols / 2).max(1)` branch was removed; columns are
    always single-width now.
  - `effective_density(base, cols, fullwidth, auto)` →
    `effective_density(base, cols, auto)`.

- **Config schema purged** (`src/configfile.rs`): `fullwidth` removed
  from `USER_CONFIG_KEYS`. The `--fullwidth` documentation comment in
  the `--dump-config` template was replaced with a v25.0.0-alpha.3
  removal note.

- **Config apply purged** (`src/config_apply.rs`): the
  `if let Some(v) = config_value(matches, cfg, "fullwidth", "fullwidth")`
  block removed. The `fullwidth` config.toml key is no longer read.

- **`--testconf` purged** (`src/testconf.rs`): `fullwidth` removed from
  the bool-validator pattern (`"low-power" | "mouse" | "fullwidth" | ...`
  → `"low-power" | "mouse" | ...`). The two `validate_field_value`
  test cases for `fullwidth` were replaced with `auto-color-drift`
  test cases.

- **Verbose output purged** (`src/verbose.rs`): the `fullwidth: bool`
  parameter removed from `print_verbose()`, and the
  `output::eprintln_verbose("fullwidth:", ...)` line removed from the
  "Glyphs" section.

- **`--help-detail` purged** (`src/help_detail.rs`): the
  `-F, --fullwidth` entry removed from the GENERAL section.

- **Tests updated**:
  - 13 `Cloud::new(...)` call sites in test files: removed the `false`
    (full_width) 2nd positional arg via a Python script
    (`scripts/remove_fullwidth_arg.py`).
  - 5 `monolith_rain.reset(40, false)` call sites: simplified to
    `monolith_rain.reset(40)`.
  - 4 `find_inactive_lane(false, false, ...)` call sites: simplified to
    `find_inactive_lane(false, ...)`.
  - 8 `DrawCtx { ..., full_width: false, ... }` literals in test files:
    removed the field.
  - 4 `CloudConfig { ..., fullwidth: false, ... }` literals in test
    files: removed the field.
  - `config_apply_tests.rs`: removed `fullwidth` from the
    `dump_should_contain_all_keys` test's expected-keys list.

### Why Remove It

1. **Misleading help text**: "Use full terminal width" suggested a
   different behavior (expand to full width) than what it actually did
   (halve the effective column count by doubling the column stride).
2. **Never the default**: The flag was `false` by default and never
   enabled automatically — no user could have been relying on it
   without explicitly opting in.
3. **Hidden from help**: `hide = true` meant it never appeared in
   `--help` or `--help-detail` until this commit removed it.
4. **Cosmic Dragon principle**: The codebase has a hard rule that
   "the charset is always single-width" — wide chars are forbidden
   permanently. The `--fullwidth` flag violated this principle in
   spirit by introducing a horizontal-spacing mode that mimicked
   wide-char rendering.
5. **Code complexity**: The flag's `bool` threaded through 6 structs
   (Args, CloudConfig, Cloud, DrawCtx, MonolithSpawnParams,
   MonolithRain) and gated dead-code branches in 5 functions. Removing
   it eliminates ~50 lines of conditional logic without changing any
   user-visible behavior.

### Migration

- **CLI users**: Remove `--fullwidth` / `-F` from your scripts. The
  flag was hidden and never documented, so this should affect no one.
  If you do try the old flag, you'll get a clear error message
  explaining the removal.
- **Config.toml users**: Remove `fullwidth = false` (or `true`) from
  your config. The key is no longer recognized; if left in, it will
  be reported as an unknown key (with a did-you-mean hint if you
  typo'd).
- **Visual behavior**: No change. The flag was always `false` by
  default, so removing it changes nothing about how cosmostrix renders.
  Monolith streams continue to render at the natural single-cell
  stride, exactly as they always have when `--fullwidth` was not
  passed.

### Verification

- `cargo build`: OK
- `cargo test --all`: 1140 passed, 0 failed
- `cargo clippy --all-targets`: clean
- `cargo fmt`: clean
- `./scripts/build.sh check-all`: All quality checks passed

---

## v25.0.0-alpha.2 — Cross-Scene Performance Audit (Monolith-Style Optimizations)

### Headline

A deep audit of the cloud subsystem compared every Glyph scene
(cinematic, matrix, signal, classic, calm, storm, cosmos, neon, hacker,
low-power, cosmic_dragon, carbonic) against the lightweight Monolith
baseline. Four surgical optimizations were applied to the shared
post-process pipeline that runs for **every** scene — preserving each
scene's visual identity while reducing per-frame work that previously ran
unconditionally. No scene's look, color, density, or rhythm was altered;
only redundant computation was removed.

### Audit Findings — Why Monolith Is Light

Monolith's lightweight reputation comes from six architectural properties:

1. **Sparse lane model** — at most 35% of lanes active per frame, vs the
   Glyph pool's `1.5 × cols` droplets (300 at 200 cols).
2. **Fixed `[Segment; 9]` array per stream** — zero heap allocation,
   compact 8-field stream struct vs Glyph's 15-field Droplet.
3. **Single delta update** — `stream.head += delta`, no gravity/turbulence
   sinusoidal overlay or head/tail crawling state machine.
4. **Low-energy `BehaviorProfile::Monolith`** — speed 0.5×, turbulence
   0.3×, anomaly 0.4×, reducing downstream post-process load.
5. **Previous-cells swap pattern** — O(active) cleanup via Vec swap, vs
   per-droplet tail-crawl `frame.set_force()` calls.
6. **Dedicated spine phosphor cleanup** — `clear_spine_phosphor()` is
   Monolith-specific; the generic Pass 2 of `phosphor_decay_pass` is
   structurally redundant for that scene family.

Most of these are fundamental to Monolith's visual identity (structured
braille pillars, cinematic pacing) and cannot be ported to Glyph scenes
without changing their look. The audit therefore targeted the **shared
post-process pipeline** that runs for every scene — the four spots below
are pure overhead reduction with zero perceptual impact.

### Changes

- **`apply_crt_vignette` — dirty-cell intersect instead of full-row scan**
  (`src/cloud/rain.rs`). Previously iterated every cell in the top and
  bottom `CRT_VIGNETTE_HEIGHT` rows (`O(cols × CRT_VIGNETTE_HEIGHT × 2)`
  per frame), checking each for a foreground to dim. Now iterates only
  the dirty cells drawn this frame that fall inside the vignette bands
  (`O(dirty_count)`). At 200×60 with 30% rain density, this drops ~2000
  cell reads to ~60 — a 30× reduction on sparse scenes (calm, low-power)
  and a 5-10× reduction on dense scenes (hacker, carbonic). Visual
  equivalence holds because the dim is idempotent: cells not redrawn
  this frame retain their previously-dimmed state from the prior frame,
  and a factor-0.9 dim of an already-0.9 cell is well below the 5% JND
  perceptual threshold.

- **`apply_crt_vignette` — skip under sustained performance pressure**
  (`src/cloud/rain.rs`, `src/constants.rs`). Added
  `CRT_VIGNETTE_PERF_THRESHOLD = 0.5` — when `perf_pressure` exceeds
  this threshold, the vignette (cosmetic-only post-process) is dropped
  entirely to preserve rain throughput. The threshold sits above
  `GLITCH_THRESHOLD` (0.35) so the vignette survives a bit longer than
  the glitch effect before being dropped, matching the relative
  perceptual importance of each feature.

- **`phosphor_decay_pass` Pass 2 — skip for Monolith scenes**
  (`src/cloud/phosphor.rs`). Pass 2 iterates `self.droplets` to protect
  active trail cells from phosphor decay. Monolith keeps `self.droplets`
  cleared (see `spawn.rs:33`), so this loop was a no-op every frame for
  Monolith scenes — the per-iteration branch overhead plus Vec::iter()
  setup was wasted work. Monolith has its own dedicated spine phosphor
  cleanup via `clear_spine_phosphor()` (called from `rain.rs:519-529`),
  so Pass 2 is structurally unnecessary for that scene family.

- **`apply_quantum_ripple` — O(1) early-out when no particles active**
  (`src/cloud/rain.rs`, `src/cloud/mod.rs`). Added a
  `quantum_active_count` field to `Cloud`, tracked incrementally
  (incremented on spawn, decremented on expiry/deactivation). The
  function now returns immediately when the count is zero — the common
  case in interactive rendering (no recent clicks) and the universal
  case in benchmark mode. Previously iterated the 64-element particle
  pool every frame regardless, doing palette color decode + Instant
  math for each non-active entry. The early-out skips all of that.

- **Replaced `apply_crt_dim_row` with `apply_crt_dim_cell`** — same
  brightness math (integer `(color * fi + 128) >> 8`), same skip-blank
  behavior, just narrowed to a single cell instead of a full row scan.
  Called from the new dirty-cell intersect loop in `apply_crt_vignette`.

### What Was NOT Changed

The audit explicitly rejected these "tempting" optimizations because
they would alter scene identity:

- **Reduce Glyph droplet pool size** — would reduce rain density, the
  defining feature of scenes like `hacker` (0.95) and `carbonic` (0.95).
- **Apply `BehaviorProfile::Monolith` to Glyph scenes** — the profile's
  speed/turbulence/anomaly multipliers define the cinematic feel of
  Monolith; applying them to Glyph would make every scene look like
  Monolith.
- **Skip gravity/turbulence in Droplet::advance** — the sinusoidal
  velocity overlay is what makes Glyph rain feel organic vs the
  mechanical constant-speed Monolith streams.
- **Replace Droplet's 15-field struct with a compact variant** — the
  fields are all actively used by the 12-effect visual pipeline
  (head bloom, parallax, fog, edge fade, vignette, mouse glow, click
  flash, rain shadow, etc); removing any would break an effect.

### Verification

- `cargo build`: OK
- `cargo test --all`: 1140 passed, 0 failed
- `cargo clippy`: clean
- `cargo fmt`: clean
- `./scripts/build.sh check-all`: All quality checks passed

---

## v20.1.0 — Legacy / Backward-Compat Purge

### Headline

The remaining backward-compatibility shims accumulated across v14–v20 are
gone. `base-scene`, `preset`, the `[profile.<name>]` fallback, the pre-v10
`config` filename fallback, and several dead-code helpers have been
removed. Custom scenes are now first-class citizens with a single config
namespace (`[scene-custom.<name>]`) and a single set of recognized fields.
`--testconf` now flags any leftover `base-scene`, `preset`, or
`[profile.<name>]` keys as unknown, prompting users to migrate.

### Removed

- **`base-scene` field** — no longer recognized in `[scene-custom.<name>]`
  or `[profile.<name>]` blocks. `--testconf` flags it as unknown.
- **`preset` field** — same treatment as `base-scene`. The
  `apply_profile_preset` deprecation-warning path is gone.
- **`[profile.<name>]` fallback for `--scene-custom`** — the loader now
  resolves only `[scene-custom.<name>]` blocks. Users with legacy
  `[profile.<name>]` blocks must rename the prefix.
- **`DEPRECATED_PROFILE_FIELDS` const** — replaced by direct rejection.
- **`UserProfile.preset` field** — struct field removed.
- **`apply_profile_preset` / `apply_profile_scene` helpers** — deleted.
- **`apply_legacy_config` stub** — empty function removed from
  `config_apply.rs` (no callers, no body).
- **`LEGACY_CONFIG_KEYS` const** — empty array removed from
  `configfile.rs`; `known_keys()` and `is_known_key()` no longer chain it.
- **`CONFIG_FILE_NAME_LEGACY` (`"config"`) constant** — pre-v10 fallback
  filename removed. `default_config_file_path()` now returns only the
  `config.toml` path.
- **`from_profile` parameter on `show_custom_scene_text`** — the
  `PROFILE (legacy)` migration marker is gone; the function renders only
  the `CUSTOM SCENE:` header.
- **`auto_density_factor(_, _lines, _)` parameter** — the unused `lines`
  arg was kept only for backward compat with callers; signature is now
  `auto_density_factor(cols, fullwidth)`. `effective_density` likewise
  loses its `lines` parameter.
- **`is_head_bright` legacy helper** in `droplet.rs` — dead-code wrapper
  deleted.
- **testconf `base` / `preset` → `scene` alias mapping** — `base` and
  `preset` are no longer treated as scene-name aliases during value
  validation.
- **Test scaffolding** — `nightcore_config()` and `atmosphere_config_profile`
  helpers migrated from `[profile.<name>]` to `[scene-custom.<name>]`.
  `profile_base_monolith_is_silently_dropped` renamed to
  `profile_base_monolith_is_unknown_key` (asserts new rejection behavior).

### Kept (NOT legacy shims)

The following "legacy"-labelled items are intentional, active code or
stable downstream contracts — they are NOT removed:

- `bench_helpers.rs` `COSMOSTRIX_BENCH_COLS` / `COSMOSTRIX_BENCH_LINES`
  env vars — active CI integration, not a deprecated feature.
- `bench_report_tests.rs` "backward-compat" field contract — enforces the
  stable benchmark JSON schema for downstream parsers.
- `cli_parse.rs` bare-number duration (`90` → 90 secs) — active UX
  feature, not a deprecated alias.
- `#![allow(deprecated)]` in `memstat.rs` / `cpustat.rs` /
  `diagnostics.rs` — required because libc 0.2.x deprecates the macOS
  `mach_timebase_info` shims in favor of `mach2`. System-level, not
  project-level legacy.
- "Legacy themes" comment in `central_colors.rs` — describes migrated
  theme names that are still active.
- `monolith.rs` / `cloud/mod.rs` / `app.rs` "legacy behavior" comments
  describing `None` density-map → uniform distribution — describes the
  active default behavior, not a shim.

### Migration

If your `config.toml` contains any of these keys, run
`cosmostrix --testconf` to find them, then:

| Old                                | New                                       |
|------------------------------------|-------------------------------------------|
| `[profile.<name>]`                 | `[scene-custom.<name>]`                   |
| `profile.<name>.base-scene = X`    | (delete — set fields directly instead)    |
| `profile.<name>.preset = X`        | (delete — set fields directly instead)    |
| `scene-custom.<name>.base-scene`   | (delete — set fields directly instead)    |
| `scene-custom.<name>.preset`       | (delete — set fields directly instead)    |
| `~/.config/cosmostrix/config`      | `~/.config/cosmostrix/config.toml`        |

---

## v20.0.0 — Temporal-Prediction Milestone (cosmic_dragon)

### Headline

The Cosmic Dragon now sees its own future. Three levers — `PREDICTION_HORIZON`
raised from 4 to 12, skip-draw on small droplet advances, and a new
`set_persistent()` mechanism in `frame.rs` — collapsed the dirty-cell
ratio from 18.33% to 0.39% on the cinematic baseline (200×60). Average
FPS climbed from 7,843 to 29,773 (+280%), and total drawn cells per run
dropped from 1.035B to 3.3M (−99.6%). The IO bottleneck is effectively
gone: `avg_io_ms` fell from 0.0428 to 0.0014.

### New Scene — `cosmic_dragon`

A new built-in scene commemorates the temporal-prediction breakthrough.
It uses the `deepspace` palette, `binary` charset, speed 12, density
0.65 — a deep, futuristic look that mirrors the cinematic base while
standing apart as a milestone reward. It is not part of the interactive
`x`/`X` cycle; invoke it explicitly with `cosmostrix --scene cosmic_dragon`.

This is the twelfth built-in scene, joining cinematic, matrix, monolith,
signal, classic, calm, storm, cosmos, neon, hacker, and low-power.

### Trade-off

Gini coefficient drops from 0.702 to 0.248 on the cinematic baseline
because skip-draw reduces the structured pillar formations. This is
acceptable for the cinematic scene (which favors breathing room over
dense structure) and is the explicit reason `cosmic_dragon` exists as
a separate scene rather than a replacement.

### Internal

- `src/cloud/rain.rs`: temporal-prediction loop with skip-draw gating.
- `src/droplet.rs`: `PredictedState` struct + `predicted_clean` flag.
- `src/frame.rs`: new `set_persistent(col, line)` method — marks a cell
  dirty for the current generation without recomputing its content.
- `src/scene.rs`: `cosmic_dragon` scene registered as a milestone entry.
- `src/help_detail.rs`: `--scene` documentation updated.
- `README.md`: scene count and curated list updated.
- `src/configfile.rs`: config template scene list updated.
- `src/atmosphere_custom.rs`: doc comment scene count updated.

---

## v15.0.0 — The Cosmic Dragon (Pre-Release Polish)

### Breaking Changes

- **`--completions <shell>` removed.** The flag, its handler, the
  `clap_complete` dependency, the AUR `PKGBUILD` install step, and the
  `scripts/install.sh` completion-installation phase are all gone. Shell
  completion scripts are no longer shipped. Users who relied on this
  feature should generate completions externally (e.g. via
  `clap_complete` in a downstream tool) or write them by hand from
  `--help-detail`. This drops one transitive crate and removes a
  maintenance surface that was rarely used.

- **`--help` no longer prints the `cosmostrix 14.0.0` / about header.**
  The help output now opens directly with `USAGE:` for a cleaner first
  impression. The header is reserved for `-V` / `--version` only. If
  you scripted against the old help layout, update your parser to skip
  the now-absent two header lines.

### Added

- **`-V` description line.** The `-V` / `--version` output now includes
  the one-line package description right under the version header:

  ```text
  cosmostrix: v14.0.0
  Professional-grade cinematic Matrix rain renderer for serious terminal environments.
  Build: …
  ```

  Both header lines are rendered in brand purple (`#A855F7`) on a TTY.
  When piped or redirected, the output is fully plain text — no ANSI
  escape codes leak into scripts or log files.

- **`BRAND_PURPLE` canonical constant** in `src/output.rs`. All CLI
  helper text (help, verbose, errors, version, list printers, doctor
  report titles, help-detail section headings) now flows through one
  of the centralized brand constants (`BRAND_PURPLE`, `BRAND_BOLD`,
  `ERROR`, `ERROR_BOLD`, `WARN`, `WARN_BOLD`, `RESET`). No hardcoded
  `\x1b[1;35m` / `\x1b[38;2;168;85;247m` strings remain in CLI code.

### Changed

- **Atmosphere default reverted to `disabled`.** Cosmostrix no longer
  silently shifts color schemes based on the local time of day when
  the user hasn't explicitly enabled the atmosphere engine. The
  adaptive color phases (Deep Void, Compression, Pulse, Calm, Signal)
  are still available — opt in via `atmosphere-mode = controlled-live`
  in `config.toml`. This fixes a regression introduced in commit
  `5172f39` where `cosmostrix -v` showed `color_scheme: Cosmos` at
  startup but `color_scheme: Neon` in the final runtime state.

- **`ux::die_config` now routes through `output::eprintln_error_labeled`.**
  Both fatal exit-2 paths (`die_input` and `die_config`) share the same
  truecolor-red branded error treatment. Previously `die_config` used
  basic ANSI red (`\x1b[31m`) with a comment about truecolor
  invisibility on some terminals — but `die_input` already used
  truecolor, so the inconsistency wasn't protecting anything.

- **`scripts/install.sh` renumbered.** The install pipeline is now
  4 steps instead of 5 (the completion-installation step was removed).

### Removed (dead code)

- **`src/cosmic_dragon_engine/`** — 7-file pure re-export namespace wrapper
  that nothing in the codebase ever consumed. Only the
  `mod cosmic_dragon_engine;` declaration in `main.rs` referenced it; the
  submodules all carried `#![allow(unused_imports)]` because Rust's
  own warning system flagged every re-export as unused.

- **`profile::dump_profile_text` and `profile::push_field`** —
  retained since v14 with `#[allow(dead_code)]` comments saying
  "test-only in v14", but a full audit found zero callers anywhere
  (production or test). The `--dump-profile` flag was removed in v14
  and these helpers were its rendering backend; with no replacement
  caller, they are deleted.

- **`HudState::reset_max`** — `#[allow(dead_code)]`-tagged HUD helper
  for clearing the peak frame-time counter. Never called by any
  keybinding or runtime path. Deleted.

- **`terminal.rs:153` literal `b"\x1b[?2026h"`** — replaced with
  `crate::termdetect::SYNC_START`. The same constant was already
  exported and used elsewhere in the file; the literal was a direct
  duplicate.

### Fixed

- **Live-reload error visibility** — errors detected by the config
  watcher during alternate-screen mode are now printed to stderr
  after `Terminal::drop` restores the terminal, via a global
  `LIVE_RELOAD_ERROR` mutex. Previously these errors were swallowed
  by the alternate screen and never visible to the user.

- **`--testconf` validates `[adaptive-custom]` entries** — flexible
  `HH-MM` time ranges (e.g. `20-23` or `20:00-23:00`) are now parsed
  and validated at startup and on live reload, not just at
  `--testconf` time. The validation also catches typos in phase color
  names (e.g. `greens3` → `neon`).

- **Screensaver mode no longer exits on every keypress** — the
  screensaver-exit check was moved after `handle_keybinding`, so
  recognized keys (c/s/x/i/etc.) still work in screensaver mode.
  Only unrecognized keys exit. Esc and Ctrl+C no longer exit at all
  (only `q` quits) — this matches the documented policy.

- **Android/Termux `i` key no longer exits** — `KeyEventKind::Release`
  events are now skipped on Android (only `Press` + `Repeat` accepted),
  so tapping `i` to toggle the HUD doesn't cause an immediate exit.

- **`--colormode` screen-size guard** — `parse_screen_size` now
  rejects dimensions below 4x4 (previously accepted `12x1` silently,
  which crashed the renderer).

### Documentation

- `docs/RULES.md` updated with the v15 Cosmic Dragon architecture and the
  1200-LOC per-file cap is now enforced.
- `docs/ATMOSPHERE_ENGINE.md` documents the 5-phase adaptive breath.
- `docs/ATMOSPHERE_EXPANSION.md` covers custom time mapping via
  `[adaptive-custom.HH-MM]` blocks.

### Performance

- **Peak Monolith optimization**: `dirty_map` migrated from `BitVec`
  to `Vec<u8>` (+2.5% FPS), phosphor dirty buffer reuse gives -44%
  allocations and -28% ns/cell. The Phosphor LUT optimization was
  attempted and honestly reported as a 7.7% regression — reverted.
- **Live config reload** uses a full Cloud rebuild via mpsc channel
  with strict validation, replacing the previous delta-apply approach.
- 864 tests pass. `cargo clippy --all-targets --all-features -- -D
  warnings` is clean. `cargo fmt --all -- --check` is clean.

---

## v14.0.0 — Scene-Custom Migration (Breaking CLI)

### Breaking Changes

This major release removes the legacy `--preset`, `--profile`, and
`--low-power` flag family and replaces them with two clear flags:
`--scene` (built-in themes) and `--scene-custom` (user-defined themes
from `[scene-custom.<name>]` blocks in `config.toml`).

**Removed flags** (each now produces a migration error pointing to its
replacement):

| Removed flag | Replacement |
|---|---|
| `--preset <name>` | `--scene <name>` (all 8 presets migrated to scenes) |
| `--profile <name>` | `--scene-custom <name>` (rename `[profile.X]` → `[scene-custom.X]` in config) |
| `--low-power` | `--scene low-power` |
| `--list-presets` | `--list-scenes` |
| `--list-profiles` | `--list-scenes` |
| `--show-preset <name>` | `--show-scene <name>` |
| `--dump-profile <name>` | `--show-scene <name>` |

### Added

- **`--scene-custom <NAME>`** — Apply a user-defined custom scene from
  `config.toml`. Looks up `[scene-custom.<name>]` blocks. Backward
  compatibility: if the name only exists as a `[profile.<name>]` block,
  the profile is loaded with a deprecation warning guiding migration
  (rename prefix only — fields are identical).

- **`--show-scene <NAME>`** — Display full configuration details for a
  built-in or custom scene. Built-in scenes show all field values plus
  `rain-style`. Custom scenes show all 12 possible fields. Legacy
  `[profile.<name>]` entries are surfaced with a migration note.

- **8 new built-in scenes** (migrated from presets): `classic`,
  `cinematic`, `calm`, `storm`, `cosmos`, `neon`, `hacker`, `low-power`.
  Combined with the original three (`matrix`, `monolith`, `signal`),
  `--scene` now accepts 11 built-in names. `SCENE_ORDER` stays three-entry
  to preserve interactive cycling behavior.

- **`--list-scenes` supercharged** — Now shows two groups:
  `BUILT-IN SCENES` (11 entries) and `CUSTOM SCENES (from config)`
  (loaded from `[scene-custom.<name>]` blocks).

- **Config namespace `[scene-custom.<name>]`** — New config block syntax
  for user-defined scenes. Field set is identical to `[profile.<name>]`,
  so migration is a pure prefix rename. Recognized by `--testconf`.

### Migration Guide

1. **`--preset storm`** → **`--scene storm`** (no behavior change)
2. **`--profile nightcore`** → **`--scene-custom nightcore`** (rename
   `[profile.nightcore]` → `[scene-custom.nightcore]` in config.toml)
3. **`--low-power`** → **`--scene low-power`**
4. **`preset = cinematic`** in config → **`scene = cinematic`** (with
   deprecation warning during transition)
5. **`profile = nightcore`** in config → **`scene-custom = nightcore`**
   (with deprecation warning during transition)

### Why

`--preset` and `--profile` were a source of confusion. "What's the
difference between a preset and a profile?" had no clean answer. The new
design answers a simpler question instead: "Do you want a built-in theme
or your own theme?" Two flags, two answers, no overlap.

---

## v13.6.0 — CLI Simplification Stage 1 + Background Mode Cleanup

### Stage 1: CLI Simplification (additive, zero breaking changes)

Three additive improvements to the CLI, paving the way for future
preset/profile unification (Stage 2-3 deferred to v13.7.0):

**New: `low-power` preset** — `cosmostrix --preset low-power` now
applies FPS 30, speed 5, density 0.5. This is the preset equivalent
of the existing `--low-power` flag. Both produce identical values;
the flag is kept for backward compatibility. The preset gives users
a consistent `--preset` interface for all curated configurations.

**New: `--uniform` flag** — `cosmostrix --uniform` disables the
default async variable column pacing, making all columns move at the
same speed. This is the inverse of the hidden `--async` flag (which
is default-on). `--uniform` is visible in `--help` under the new
ADVANCED heading. If both `--async` and `--uniform` are passed,
`--uniform` wins (async off).

**Help restructure: ADVANCED heading** — `--monolith-size` moved
from COMMON OPTIONS to a new ADVANCED heading. `--uniform` is also
in ADVANCED. This keeps COMMON OPTIONS focused on the most-used
flags while making advanced tuning discoverable.

### Background Mode Cleanup

User-facing behavior change: cosmostrix no longer paints a solid black
background by default. The new default is `--color-bg default-background`,
which follows the terminal emulator's configured background. If your
terminal is set to cyan, dark gray, or a wallpaper image, cosmostrix
will blend with it instead of covering it with `#000000`.

### Changed

- **`--color-bg` default is now `default-background`** (was `black`).
  Cosmostrix no longer emits `48;2;0;0;0m` background codes per cell by
  default — only foreground ANSI sequences. This saves ~12 bytes/cell
  in interactive mode and makes the renderer blend seamlessly with
  terminal themes. Users who want the old behavior can pass
  `--color-bg black` explicitly.

### Removed

- **`--color-bg transparent`** variant. It was a duplicate of
  `default-background` (both set `palette.bg = None` and
  `default_background = true`). The `ColorBg::Transparent` enum variant,
  the `transparent` parse arm in profile/config_apply, the
  `make_cloud_transparent_bg` test helper, and the
  `transparent_color_bg_does_not_force_solid_black` test (duplicate of
  `default_background_mode_keeps_bg_none`) have all been removed.
  Existing configs that set `color-bg = transparent` will now print an
  error and fall back to the new default.

### Migration

- If you previously passed `--color-bg transparent`, use
  `--color-bg default-background` (or simply omit the flag — it's the
  new default).
- If you previously relied on the implicit solid-black background, pass
  `--color-bg black` explicitly to restore the old behavior.
- Config files with `color-bg = transparent` must be updated to
  `color-bg = default-background` (or removed entirely).

---

## v13.4.0 — Screen Size + Duration Features

New feature release. Adds `--screen-size WxH` for fixed virtual screen
size and `--duration` for human-readable benchmark duration. HUD now
shows screen size on the 6th line.

### Features

**`--screen-size WxH`** (e.g. `--screen-size 120x40`, `--screen-size 12x12`):
- **Benchmark mode**: override terminal size. Replaces
  `COSMOSTRIX_BENCH_COLS`/`COSMOSTRIX_BENCH_LINES` env vars.
  Example: `cosmostrix --benchmark --screen-size 12x12 --json`
- **Interactive mode**: render to fixed virtual size. Ignores terminal
  resize events. If screen-size exceeds terminal, prints warning and
  clips to top-left.
- **Without `--screen-size`**: dynamic (current behavior — follows
  terminal resize).
- Minimum: `1x1`. Maximum: `65535x65535` (u16 range).
- Case-insensitive: `120x40` or `120X40` both work.

**`--duration` compound format** (e.g. `--duration 6s`, `--duration 1h30m`):
- Human-readable duration: `6s`, `30m`, `1h`, `1h30m`, `2h15m30s`
- Long forms: `6sec`, `30mins`, `1hour` also accepted
- Bare number: `--duration 90` = 90 seconds (backward compat)
- Minimum: `1s`. No maximum cap (user responsibility for long runs).
- Alias for `--bench-duration` — `--duration` takes precedence if both
  are specified.
- `--bench-duration` 600s max cap REMOVED — unlimited endurance runs.

**HUD screen size line**:
- 6th HUD line shows current screen size: `120x40` (dynamic) or
  `120x40*` (fixed via `--screen-size`).
- Updates on terminal resize (dynamic mode) or stays constant (fixed mode).

### Implementation

- `src/cli_parse.rs` (new, 175 LOC): `parse_duration()` + `parse_screen_size()`
  with 16 unit tests covering all formats + edge cases.
- `src/config.rs`: added `--duration` + `--screen-size` CLI args.
- `src/app.rs`: added `screen_size: Option<(u16, u16)>` to CloudConfig.
- `src/main.rs`: `resolve_bench_duration_args()` — `--duration` takes
  precedence over `--bench-duration`.
- `src/bench.rs`: `bench_dimensions()` accepts CLI screen-size;
  removed 600s max cap from `resolve_bench_duration()`.
- `src/interactive/event_loop.rs`: fixed vs dynamic size logic;
  resize events ignored in fixed mode; warning on size > terminal.
- `src/interactive/hud.rs`: 6th HUD line for screen size;
  `set_screen_size()` method.

### Usage Examples

```bash
# Benchmark at fixed 12x12 size (tiny terminal = max FPS)
cosmostrix --benchmark --screen-size 12x12 --json

# Benchmark for 10 minutes
cosmostrix --benchmark --duration 10m --json

# Benchmark for 1h30m (endurance test)
cosmostrix --benchmark --duration 1h30m --json

# Interactive at fixed 80x24 (ignores terminal resize)
cosmostrix --screen-size 80x24

# Interactive dynamic (current behavior — follows terminal)
cosmostrix
```

All 747 tests pass (731 existing + 16 new cli_parse tests). Clippy + fmt clean.

---

## v13.3.1 — Cosmic Dragon Performance Merge (18 Cosmic Dragon Eggs + P1/P2/P3)

Performance-only patch release. Merges the `cosmic-dragon-experimental`
branch: 8 commits containing 18 "cosmic dragon egg" micro-optimizations
plus 3 P-tier optimizations (P1/P2/P3). No color/render quality
changes — all commits are pure performance work.

### P1: Gate Component Timing Behind Flag

`cloud.rain_at()` skips 2 `Instant::now()` calls (t1, t2) when
`enable_component_timing` is false. Interactive mode leaves it off;
`--benchmark` and `--perf-stats` enable it. Saves ~40ns/frame.

### P2: Halve spin_wait Instant::now() Calls

`activity.spin_wait()` cached `now` for both deadline + limit checks.
Saves ~250µs/frame in interactive mode (50% reduction in spin timing).

### P3: Combined flush_ansi + io_uring Dead End

- `terminal.flush_ansi()` combines `SYNC_START + ansi_buf + SYNC_END`
  into a single `write_all` via reusable buffer. Reduces syscalls 3→1.
- `cosmic_dragon_egg_io_uring.rs` proves io_uring NOT worth it: `write()`
  syscall is 306ns/call = 0.0018% CPU at 60 FPS. Dead end.

### 18 Cosmic Dragon Eggs: Eliminate Redundant Bounds Checks + Option Allocs

All 18 eggs: `.get(i)` → direct `[]` indexing when `i` was already
bounds-checked. 2-3 cycles faster per call.

| Eggs | File | Pattern |
|------|------|---------|
| #1-5 | frame.rs, terminal.rs | set(), set_force(), diff path, dirty_map, clear_dirty() |
| #6-7 | frame.rs | cell_gen_at_index(), get() test accessor |
| #8-10 | phosphor.rs | phosphor_fresh, phosphor_in_active |
| #11-13 | render.rs | get_char(), col_stat, edge_fade LUT |
| #14-18 | render.rs, spawn.rs, monolith.rs | glitch_map, color_map, palette_slices |

Eggs #19-21 attempted but caused regression — compiler was already
optimal in those paths. Reverted.

### Performance Gains

| Size | v13.3.0 avg FPS | v13.3.1 avg FPS | Delta |
|------|----------------:|----------------:|------:|
| 4×4 | 765,235 | 790,770 | +3.3% |
| 80×24 | 105,256 | 108,572 | +3.2% |
| 120×40 | 51,236 | 51,865 | +1.2% |
| 200×60 | 28,138 | 28,582 | +1.6% |

Peak 4×4: 1,082,251 → 1,122,334 FPS (+3.7%)

### What Did NOT Change

- No color palette changes
- No rendering logic changes
- No visual quality changes
- 731 tests pass (729 existing + 2 cosmic dragon egg tests)
- `supercharger.c`/`supercharger.rs` are research artifacts, gated
  behind feature flag, NOT compiled by default

---

## v13.3.0 — Encoding Instrumentation (SGR cache hit-rate + ANSI bytes/frame)

Adds empirical measurement instrumentation to the diff-based rendering
engine. The `--perf-stats` exit report now includes an ENCODING section
showing actual measured ANSI bytes per frame, total bandwidth, and SGR
cache hit rate — replacing the previous estimate-based numbers.

### New Metrics (ENCODING section in --perf-stats report)

- **total_ansi_bytes**: cumulative ANSI bytes flushed to stdout across
  all frames. Measured in `Terminal::flush_ansi()` by summing
  `ansi_buf.len()` before each clear.
- **frames_flushed**: number of `flush_ansi()` calls (= number of frames
  actually drawn to the terminal).
- **avg_bytes_per_frame**: `total_ansi_bytes / frames_flushed`. Replaces
  the previous `ANSI_BYTES_PER_CELL_ESTIMATE` heuristic with actual
  measurement.
- **bandwidth**: `total_ansi_bytes / elapsed_seconds` in KiB/s. Shows
  real terminal I/O load.
- **sgr_cache_hits / sgr_cache_misses**: atomic counters in `ColorCache`
  incremented on every `sgr_for_cell()` call. Hit = palette color found
  in cache; miss = fell back to on-the-fly `write_sgr_colors_buf`.
- **sgr_cache_hit_rate**: `hits / (hits + misses) * 100%`. High rate
  (>90%) confirms the cache is effective.

### Implementation

**Option A — SGR cache counters** (`src/color_cache.rs`):
- Added `sgr_hits: AtomicU64` and `sgr_misses: AtomicU64` fields to
  `ColorCache`.
- `sgr_for_cell()` increments the appropriate counter on every call.
  Uses `Ordering::Relaxed` (~2ns overhead on x86) — eventual accuracy
  is sufficient for the perf report.
- New `cache_stats()` method returns `(hits, misses)`.
- 6 new unit tests covering: zero-initialization, hit on palette color,
  miss on non-palette color, miss on non-palette bg, hit on reset/blank,
  accumulation across calls.

**Option B — ANSI bytes/frame counter** (`src/terminal.rs`):
- Added `total_ansi_bytes: u64` and `flush_count: u64` fields to
  `Terminal`.
- `flush_ansi()` accumulates `ansi_buf.len()` into `total_ansi_bytes`
  and increments `flush_count` BEFORE clearing the buffer. Sync wrapper
  bytes (12 bytes when `sync_output` is enabled) are NOT counted —
  only actual frame content.
- New `encoding_stats()` method returns
  `(total_ansi_bytes, flush_count, sgr_hits, sgr_misses)`.
- Called from the `--perf-stats` exit path in `event_loop.rs`, captured
  BEFORE `drop(term)` to avoid losing the stats.

### Code Refactor

- Extracted `push_u8`, `push_u16`, `write_sgr_colors_buf` from
  `terminal.rs` into new `src/sgr_format.rs` (106 LOC). These are pure
  SGR formatting functions with no dependency on the `Terminal` struct.
  This keeps `terminal.rs` under its 1000-LOC guard (now 947 LOC).
- Updated `docs/RENDER_ENGINE.md` future-work section: SGR cache
  instrumentation marked as DONE.

### Why This Release

The RENDER_ENGINE.md spec claimed "~95% SGR cache hit rate" without
empirical evidence. v13.3.0 makes that claim **measurable and
defensible**. Run `cosmostrix --perf-stats`, interact for a few seconds,
press `q`, and the ENCODING section shows the actual hit rate and bytes
per frame.

This also replaces the `ANSI_BYTES_PER_CELL_ESTIMATE` heuristic in the
benchmark report with actual measured bytes per frame — the estimate
was ~19 bytes/cell, but with RLE batching the real number is typically
much lower (often 1-5 bytes/cell for stable rain).

All 729 tests pass (723 existing + 6 new counter tests). Clippy + fmt
clean.

---

## v13.2.0 — Render Engine Formal Specification + Competitor Benchmark

Documentation release formalizing cosmostrix's position as the
definitive diff-based terminal rendering engine. No runtime behavior
changes — purely additive documentation and tooling.

### Documentation

**Formal render engine specification** (`docs/RENDER_ENGINE.md`):
- 9-section formal architecture document covering: problem statement,
  strategy (differential rendering + RLE), data structures, complexity
  analysis, output encoding details, alternative-engine comparison,
  measured performance, failure modes, and future work.
- Includes BibTeX citation block for academic reference.
- Documents the existing `terminal.rs` `draw()` implementation:
  - Cell equality fast path (24-byte derived `==`, ~4 cycles/cell)
  - Dirty tracking via BitVec + dirty queue (O(1) mark, O(dirty) flush)
  - Run-length encoding on both full-redraw and diff-redraw paths
  - SGR state tracking across runs (`cur_fg`/`cur_bg`/`cur_bold`)
  - `ColorCache` pre-computed SGR bytes per `(fg, bg)` pair
  - `semantic_gen` counter for charset/theme invalidation
  - `force_draw_everything()` escape hatch for overlay cleanup
- Compares cosmostrix's diff-based engine against 5 alternatives:
  full redraw (cmatrix), per-droplet cursor targeting, ANSI scroll
  regions, Sixel/graphics protocol, PTY multiplexer — with explicit
  trade-off analysis for each.

**Competitor benchmark script** (`scripts/bench-compare.sh`):
- Side-by-side resource comparison: cosmostrix vs cmatrix vs unimatrix.
- Uses `/usr/bin/time -v` inside a PTY (`script`) to measure CPU time
  and peak RSS under identical terminal conditions.
- Outputs a Markdown table suitable for pasting into `benchmark/README.md`.
- Honest about limitations: terminal-bound renderers cannot be
  benchmarked for FPS via subprocess (FPS is determined by the terminal
  emulator, not the process). The script measures **resource
  efficiency** — the defensible axis for diff-based vs full-redraw
  engine evaluation.
- Gracefully handles missing competitors (cmatrix/unimatrix) with
  clear install instructions.

### README / Docs Cross-References

- `README.md` Documentation section: added link to RENDER_ENGINE.md.
- `benchmark/README.md`: added "Competitor Comparison" section
  pointing to `scripts/bench-compare.sh` and `docs/RENDER_ENGINE.md`.

### Why This Release

Cosmostrix's `terminal.rs` `draw()` function has been at masterclass
level since v10.x — RLE on both paths, SGR state tracking, color
cache, direct ANSI byte buffer, no-heap integer formatting. But this
was implicit knowledge scattered across code comments. v13.2.0 makes
it **explicit and defensible**:

1. **RENDER_ENGINE.md** makes the design citation-worthy — downstream
   TUI authors can reference cosmostrix as a reference implementation
   of diff-based terminal rendering.
2. **bench-compare.sh** provides empirical evidence — without
   competitor data, claims of "masterclass" are marketing, not
   engineering.
3. The formal spec also serves as onboarding for new contributors:
   instead of reverse-engineering `terminal.rs`, they read one
   document that explains the why behind every design choice.

---

## v13.1.2 — HUD Toggle-Off Residue Fix

Bug-fix release addressing a visual residue issue: when toggling the
live HUD off (pressing `i` again), stale HUD text + black background
cells remained visible in regions where the rain didn't actively write
this frame.

### Bug Fixes

**HUD toggle-off now clears residue via force_draw_everything()**:
- The rain renderer is diff-based (`frame.set()` skips cells whose
  content matches the previously-sent state). When HUD turns off, the
  frame buffer still contains the 5×15 HUD cells (text + black bg). On
  the next frame, only cells the rain actively writes get refreshed —
  cells in dead zones (no active droplet, no glitch, no phosphor decay
  this frame) keep their stale HUD content, leaving visible "residue".
- The fix calls `cloud.force_draw_everything()` when toggling OFF. This
  triggers `frame.clear_with_bg()` on the next rain update, which:
  1. Sets `dirty_all = true` (forces every cell to be re-sent)
  2. Resets all cells to the bg color
  3. The rain then redraws active cells on the clean canvas
- Net effect: HUD cells are guaranteed to be cleared, regardless of
  whether the rain happens to write them this frame. The user sees a
  clean toggle-off with no leftover text.
- Toggling ON does not need force_draw — the HUD writes via `set()`
  which marks cells dirty because content differs from rain. Toggle ON
  was already working correctly.

### Code Changes

- `src/interactive/event_loop.rs`: HUD toggle handler now captures the
  return value of `hud_state.toggle()` (new visibility state) and calls
  `cloud.force_draw_everything()` only when turning OFF. Added detailed
  comment explaining the residue mechanism and why force_draw is needed.

---

## v13.1.1 — Android HUD Toggle Fix

Bug-fix release addressing a critical Android/Termux regression: pressing
the HUD toggle key caused cosmostrix to self-exit instead of showing the
live metrics overlay.

### Bug Fixes

**HUD toggle key changed from `?` to `i`**:
- On Android/Termux soft keyboards, the `?` character may arrive with
  unexpected modifier bits or as a different keycode entirely. When the
  event did not match the HUD toggle arms, it fell through to the
  screensaver exit path (`if cfg.screensaver { cloud.raining = false;
  break; }`), causing cosmostrix to quit instead of toggling the HUD.
- The fix replaces `?` (and the previous `/`-with-Shift fallback arms)
  with a simple lowercase printable letter `i` (uppercase `I` also
  accepted). Every Android keyboard sends simple printable letters
  reliably; the modifier-bit ambiguity is eliminated entirely.
- All docs, help text (`--help-detail`), README, ROADMAP, and
  RELEASE_CANDIDATE updated to reflect the new key.

### Documentation

- `docs/ROADMAP.md`: added `P2-fix` row noting the v13.1.1 key change.
- `docs/RELEASE_CANDIDATE.md`: HUD smoke-test steps updated to press `i`.
- `src/help_detail.rs`: RUNTIME CONTROLS table updated.
- `README.md`: keyboard shortcuts table + benchmark section updated.

---

## v13.1.0 — Shell Completions + Verbose + Help-Detail Polish

UX polish release. Adds shell completions, a verbose diagnostic flag,
strict .toml enforcement for `--config`, and clearer help text.

### Features

**Shell completions** (bash, zsh, fish, elvish):
- New `--completions <shell>` flag generates a shell completion script
  on stdout. Pipe to your shell's completions directory.
- AUR `PKGBUILD` and `scripts/install.sh` auto-install bash + zsh
  completions during package install.
- Built with `clap_complete = "4"`.

**Verbose diagnostic output** (`--verbose`):
- Prints 30+ diagnostic fields to stderr before launching: config path,
  resolved values, terminal detection, atmosphere state, color tune,
  charset source, profile, etc.
- For power users debugging config/loading issues.

### Bug Fixes / Behavior Changes

**Strict .toml extension check for `--config`**:
- Previously `--config` would silently accept non-.toml files. Now it
  enforces the .toml extension and exits with a clear error.

**Invalid config values now say "error:" not "warning:"**:
- Invalid config values are no longer silently ignored. They now print
  `error: invalid <field>='<value>' (allowed: ...)` to stderr so users
  immediately know cosmostrix didn't load their custom config.

**Help-detail DIAGNOSTICS section**:
- `--verbose` and `--completions <shell>` added to `--help-detail`
  DIAGNOSTICS section with clear usage examples.
- `--dump-config` text updated: "warn cleanly and are ignored" →
  "error: messages are printed to stderr" (reflects the actual behavior).
- `--config` text: removed "Falls back to legacy 'config' (no extension)"
  since we now require .toml.

### Documentation

- `docs/ROADMAP.md`: removed future roadmap sections (secret — kept
  private until features ship). Only completed history remains.
- Test suite reduced from 882 to 723 by removing 159 doc-content tests
  (-2178 LOC). The docs-tests module now only verifies asset integrity
  and metadata, not prose content.

---

## v13.0.0 — Alive Rain + Depth-of-Field + Security

Visual quality + security hardening release. The rain now feels alive
throughout the trail (not just at the head), background rain appears
out-of-focus like film Matrix depth-of-field, the message typewriter
glows in per character, and file-reading CLI flags are restricted to
safe paths.

### Visual Quality

**Character cycling** (alive rain):
- Trail characters now have a 2% chance per decay step to mutate to a
  new random glyph from the char pool. At 60fps with ~1000 active trail
  cells, ~20 characters change per second — subtle enough to feel
  organic, frequent enough to make the rain feel "alive" throughout.
- Previously only the head character cycled (every 100ms); the trail
  was static after spawn. Now matches the film Matrix effect where
  background characters subtly shift.
- New constant: `TRAIL_CYCLE_PROBABILITY = 0.02` in constants.rs.

**Depth-of-field** (perceptual blur):
- Layer 0 (background) foreground color is blended 35% toward black,
  creating a "foggy/out-of-focus" look. The terminal equivalent of
  depth-of-field: instead of blurring pixels (impossible in text), we
  reduce fg-bg contrast so background rain reads as "behind a haze".
- Layers 1-2 stay sharp. 3-tier depth hierarchy: sharp foreground →
  clear midground → hazy background.
- New constant: `PARALLAX_CONTRAST_REDUCTION = [0.35, 0.0, 0.0]`.

**Typewriter fade-in glow** (masterclass upgrade):
- Each newly revealed message character now fades in from 30% to 100%
  brightness over 100ms (3 frames at 30ms/char reveal rate). Creates a
  premium "glow-in" effect — characters appear to illuminate rather
  than snap into existence.
- Previously characters popped in at full brightness (hard pop-in).

**Space key restarts typewriter**:
- When the user presses Space to reseed the rain, the message typewriter
  also restarts from the beginning. Rain reseed + message types out
  from scratch — consistent cinematic replay on every restart.
- New method: `cloud::restart_message_typewriter()`.

### Security

**Safe path validation** (`--config` and `--charset-file`):
- Prevents cosmostrix from being used as an arbitrary file reader.
  Before: `--charset-file /etc/shadow` would read and display shadow
  file contents as charset characters. `--config /proc/self/environ`
  would parse environment variables as config.
- Now: `is_safe_path()` validates the path before reading. Allowed:
  home directory (`~`), current directory (`.`), `/etc/cosmostrix/`,
  `/tmp/` (for testing/scripts). Rejected: `/etc/shadow`, `/proc/*`,
  `/sys/*`, `/root/*`, `/var/log/*`, etc.
- New file: `src/safepath.rs` (117 LOC) with 6 unit tests.

**System-wide config fallback**:
- `load_config_file()` now falls back to `/etc/cosmostrix/config.toml`
  when no user-level config exists. Search order: user config → legacy
  filename → /etc system default.

**Message length limit**:
- `--message` text is now limited to 200 characters. Prevents layout
  overflow from excessively long messages. Clear error message on
  violation.
- New constant: `MESSAGE_MAX_LEN = 200`.

### PKGBUILD Cleanup

- Removed hardcoded config.toml from AUR package. Clean install: only
  binary + license + docs. No config files installed — cosmostrix
  ships sensible built-in defaults and generates a config on demand
  via `cosmostrix --dump-config`.

---

## v11.1.0 — Benchmark Depth & Theme Tuning

Closes the "real metrics, not gimmick" gap and pushes the benchmark to
S-tier (DeepSeek 9.8/10 → 10/10). The premium benchmark (`--benchmark`)
now reports RSS memory, CPU usage, sub-component timing, long-run drift,
build/environment metadata, page faults + context switches, and an
explicit GPU-not-used declaration. A live HUD overlay brings the same
metrics into interactive runs. JSON output mode enables CI parsing.
Theme tuning makes the 43 built-in palettes more visually distinct.

### New Features

**RSS memory tracking** (P0-A, commit 34f22df):
- `--benchmark` now emits a `MEMORY` section with `peak_rss`, `avg_rss`,
  `rss_samples`, `rss_basis`, and `rss_caveat`.
- Zero new dependencies. Linux samples `/proc/self/status`; macOS uses
  `mach_task_basic_info` via `libc`. Other platforms emit `unsupported`.
- The benchmark report honestly states "RSS includes shared pages; treat
  as order-of-magnitude footprint" so users do not over-interpret.

**Tail frame-time metrics** (P0-B, commit 3afac82):
- Added `p99_9_frame_time` (1-in-1000 worst frames) and `max_frame_time`
  (single worst spike) to the `PERFORMANCE` section, plus
  `max_frame_time_meaning`.
- `max_frame_time` captures what users perceive as jank — page faults,
  OS scheduling glitches — that p99 smooths over.
- PERFORMANCE section reordered for monotonic display:
  avg → p95 → p99 → p99.9 → max.

**Sub-component timing** (P1-A, commit 6bc5035):
- New `COMPONENT TIMING` section: `avg_sim_ms`, `avg_render_ms`,
  `avg_io_ms`, plus maxes and `sim/render/io_share_percent`.
- `sim_ms` = atmosphere events + spawn rate + droplet physics.
- `render_ms` = phosphor decay + anomaly zones + atmospheric fx.
- `io_ms` = dirty checks + clear_dirty + bookkeeping. Honestly labeled
  "NO terminal write in benchmark mode" — not real IO.
- Distinguishes "benchmark mainan" from "profiling tool".

**`--bench-duration N` flag + drift detection** (P1-B, commit 9e94527):
- New `--bench-duration <1-600>` flag (default 5s). Use with `--benchmark`
  for long-run drift / leak / thermal-throttle detection.
- New `DRIFT` section: `first_half_fps`, `second_half_fps`,
  `fps_drift_percent`, `drift_interpretation`, `drift_basis`.
- Interpretation: `> +10%` = degraded; `< -10%` = improved (warmup
  insufficient); otherwise `stable`.

**Live HUD overlay** (P2, commit 12a1d2f):
- Press `?` during any interactive run to toggle a top-right overlay
  showing `fps`, `avg`, `p99`, `max`, `rss` in real time.
- Zero cost when off (all methods short-circuit on `visible == false`).
- 4 Hz render rate, 1 Hz RSS sampling. ANSI-only output bypasses the
  frame buffer to keep rain renderer's dirty tracking clean.

**CPU usage % tracking** (P3, commit aeafdd3):
- New `CPU` section in `--benchmark`: `avg_cpu_percent`,
  `peak_cpu_percent`, `cpu_samples`, `cpu_basis`, `cpu_caveat`.
- Linux samples `/proc/self/stat` (utime + stime); macOS uses
  `mach_task_basic_info` (`time_value_t` seconds + microseconds).
  Other platforms emit `unsupported`.
- `cpu_caveat` honestly states "~100% = one core saturated; >100% would
  indicate multi-threading or measurement error" (single-thread by design).

**`--color-tune` runtime theme adjustment** (Q2, commit ce0d191):
- New `--color-tune saturation=X,brightness=Y` flag. Keys: `saturation`/
  `sat`, `brightness`/`bright`. Range 0.0–3.0 (1.0 = identity).
- Linear-RGB transforms (no HSL round-trip): saturation scales distance
  from Rec. 601 luminance; brightness multiplies each channel.
- Turns the 43 built-in themes into 43 × infinite variants without adding
  new presets. Background color is also tuned for visual consistency.

**Build metadata + CPU model** (peak, commit 7db64b9):
- `SYSTEM` section expanded from 3 to 12 fields: now includes
  `rustc_version`, `git_sha`, `cpu_baseline`, `target_features`, `lto`,
  `panic`, `strip`, `pgo`, and `cpu_model` (runtime-detected chip name).
- `cpu_model` reads `/proc/cpuinfo` (Linux) or `machdep.cpu.brand_string`
  via sysctl (macOS). Lets users compare reports across machines.

**Resource usage via getrusage** (peak, commit 7db64b9):
- New `RESOURCE` section: `minor_faults`, `major_faults`,
  `voluntary_ctxt`, `involuntary_ctxt` + `*_meaning` fields.
- Cross-platform via `getrusage(RUSAGE_SELF)` — no permissions required.
- Covers the scheduling-pressure story without `perf_event_open` (which
  is Linux-only and permission-gated).

**GPU-not-used declaration** (peak, commit 7db64b9):
- New `gpu_usage: not_applicable` + `gpu_basis` fields in the RENDERER
  section. Explicitly declares that cosmostrix creates no GPU context
  (no OpenGL/Vulkan/Metal/DirectX/WebGPU). Closes the "does cosmostrix
  use GPU?" question definitively in the report itself.

**Benchmark environment (reproducibility)** (peak, commit 7db64b9):
- New `BENCHMARK ENVIRONMENT` section: `kernel_version`, `libc_variant`,
  `term`, `term_program`, `term_version`, `cpu_governor`, `smt_active`
  + `env_basis` + `env_caveat`.
- Cross-platform: kernel via `uname`, libc variant from build-time,
  terminal from env vars. Linux-only: CPU governor + SMT from `/sys`.
- Lets users compare reports across machines knowing the OS/governor/
  terminal context. Two machines with the same CPU can produce
  different results if the governor differs.

**JSON output mode** (peak, commit 7db64b9):
- New `--json` flag. Use with `--benchmark` for machine-readable JSON
  output (single line, parseable by CI/scripts).
- Manual JSON serializer — zero new dependencies (no serde).
- Mirrors the text report's 13 sections: status, system, renderer,
  config, environment, performance, memory, cpu, resource,
  component_timing, drift, throughput, timing.
- Option fields emit `null` when None; NaN/Inf emit `null` defensively.

**Async mode now default ON + improved distribution** (organic, commit pending):
- `--async` default flipped from `off` → `on`. The rain now feels
  organic out of the box — columns fall at desynchronized speeds
  instead of uniform pacing.
- Speed distribution improved from flat `uniform[0.33, 1.0]` (mean 0.665)
  to `max(two uniforms)` — a triangular distribution skewed toward 1.0
  (mean ~0.78). Most columns run near full speed with occasional slow
  streams, which feels more natural than the previous flat distribution.
- Naming clarified in `--help-detail`: "async" means "asynchronous
  column pacing", NOT Rust async/await. Cosmostrix remains single-threaded.
- `async-mode = true` now appears in `--dump-config` and `config.toml`
  with a clarification comment.
- Config file key `async-mode` added to `USER_CONFIG_KEYS` so it's
  recognized by the parser (previously only settable via CLI flag).
- `config_apply.rs` now reads `async-mode` from config files.
- Runtime toggle `a` still works for A/B comparison.

### Theme Audit

**5 near-duplicate themes tuned** (commit 304a07b):
- Programmatic audit (pairwise average per-stop RGB distance < 30)
  identified 5 pairs that were too similar. All tuned to be visually
  distinct:
  - `green3`: deep teal-shifted forest green (was nearly identical to `green`)
  - `saturn`: amber-gold (was too close to `venus`)
  - `comet`: deep-blue ion trail to cyan-white head (was too close to `uranus`)
  - `meteor`: burning rock with ionized plasma tail (was too close to `sun`)
  - `pluto`: nitrogen-ice blue dwarf (was too close to `mercury`)
- Theme descriptions in `--list-colors-detail` updated to match.
- Audit runs as an informational test (`palette::audit_tests::
  audit_near_duplicate_themes`) — future theme additions can re-run it.
- After tuning: no pairs remain under threshold. Closest is
  `galaxy` ↔ `andromeda` at 30.0 (both purple-cosmic, meant to be related).

### CI Fixes

- `libc` moved from `cfg(target_os = "linux")` to `cfg(unix)` so macOS
  builds can use `mach_task_basic_info` (commit 22fa131).
- macOS Mach API migrated from removed `task_basic_info` /
  `TASK_BASIC_INFO` to modern `mach_task_basic_info` /
  `MACH_TASK_BASIC_INFO` (commit 4e76fda).
- `cpustat.rs` macOS branch fixed: `time_value_t` is a struct
  `{seconds, microseconds}`, not `u32` — removed incorrect
  `mach_timebase_info` conversion (commit 58ebedb).
- `diagnostics.rs` macOS `sysctlbyname` fixed: `null()` → `null_mut()`
  for `*mut c_void` params + removed unused `c_char` import (commit 4726d9a).
- `.codespellrc` added `numer`, `denom` to ignore-words-list (legitimate
  Mach timebase field names, not typos).

### Internal

- 7 new source files: `memstat.rs`, `cpustat.rs`, `usagestat.rs`,
  `envstat.rs`, `bench_mem.rs`, `bench_cpu.rs`, `bench_comp.rs`,
  `bench_progress.rs`, `bench_meta.rs`, `bench_json.rs`, `color_tune.rs`,
  `interactive/hud.rs`.
- `bench.rs` extracted `BenchProgress` + `ComponentTimer` + `RssTracker`
  + `CpuTracker` to keep the file under its 900-LOC guard.
- `bench_report.rs` extracted meaning constants + helpers to `bench_meta.rs`
  + BENCHMARK ENVIRONMENT rendering to `envstat.rs` to keep under 1000 LOC.
- `FrameTimeTracker` gained `p99_ms()` accessor for the live HUD.
- `cloud/rain.rs` instrumented with 2 `Instant::now()` markers per frame
  for sim/render split (~40ns overhead, negligible).
- 845 → 864 tests (clippy + fmt clean on every commit).
- Zero new runtime dependencies.

---

## v12.0.0 — Protocol Engine

**Released: 2026-07-08**

Major release introducing terminal protocol intelligence and color pipeline
optimization. The engine now detects the terminal emulator at startup and
adapts its output strategy accordingly.

### New Modules

- **`src/termdetect.rs`** — Terminal vendor detection (kitty, wezterm, alacritty,
  foot, iTerm2, Windows Terminal, tmux, Rio) via environment variables.
  Enables synchronized output (`ESC[?2026h` / `ESC[?2026l`) for tear-free
  frame delivery. Safe on all terminals — unsupported ones ignore the sequences.
- **`src/color_cache.rs`** — Pre-formatted ANSI SGR byte cache for palette colors.
  Eliminates ~300-400 per-cell encoding calls per full-redraw frame.
  Linear-scan lookup optimized for small palettes (7-20 colors).
- **`src/ux.rs`** — Unified CLI user-experience output. Single source of truth
  for error/warning formatting. Fixes double-print bug on validation errors.

### Performance

- Synchronized output: terminal buffers entire frame, flushes atomically
- Color byte cache: `extend_from_slice` replaces `push_u8` arithmetic
- Zero regression on benchmark throughput (engine already at 50K+ FPS)

### UX Improvements

- All error messages: single clean line with `error:` prefix
- All warnings: consistent `warning:` prefix (was mixed `config:` / `warning:`)
- Exit codes: 2 for invalid input, 1 for config/runtime failure

---

## v11.0.0 — Cinematic Peak

Visual quality push to peak cinematic Matrix rain. Pure tuning — no
architecture changes, no new dependencies. Every change is a constant
value adjustment or small feature addition.

### Visual Quality Improvements

**Cosmos palette brightened** (v10.0.0):
- Old: `[17,18,19,54,55,56,57,93,129,189,225]` — avg 30.3% luminance
- New: `[20,27,33,57,63,93,99,129,141,189,225]` — avg 45.5% luminance
- Replaced 3 darkest entries with vibrant blue/purple mid-range colors.

**Head white blend raised 12% → 45%** (v10.0.0):
- Glyph mode (droplet.rs): HEAD_WF 31 → 115
- Monolith mode (monolith.rs): CORE_WF 26 → 115
- Head is now OBVIOUSLY brighter than body — film-quality head pop.

**Parallax layer 0 raised 0.55 → 0.70** (v10.0.0):
- Background rain now visible (was near-invisible after dimming).
- 3-tier depth hierarchy: bright head → mid body → dim-but-visible background.

**Phosphor decay faster** (v11.0.0):
- PHOSPHOR_DECAY_RATE: 3.0 → 5.0 (afterglow 1094ms → ~400ms)
- PHOSPHOR_TAIL_RESIDUAL: 160 → 120 (63% → 47% initial brightness)
- Trail is now crisp and energetic — matches film Matrix energy.

**EdgeFade bottom min raised 0.20 → 0.45** (v11.0.0):
- Bottom border brightness: 7% → 16% (visible, was near-invisible).
- Cinema framing preserved without over-aggressive dimming.

**Fog min factor raised 0.35 → 0.45** (v11.0.0):
- Border rows brighter, less aggressive vignette.
- Combined with EdgeFade: bottom border now ~20% brightness (was 7%).

**Monolith Ghost/Dim level raised** (v11.0.0):
- Old: `first_visible` (index 1, 4-33% luminance)
- New: `last/5` (index 2, ~42% luminance for cosmos)
- Ghost trace now visible after dimming (~25% perceptual brightness).

**Default density raised 0.75 → 0.85** (v11.0.0):
- More columns active, denser rain — matches film Matrix density.
- Updated: scene.rs, config.toml, dump_config_text, all test assertions.

**Head shimmer period 0.12s → 0.10s** (v11.0.0):
- Character changes 10/sec (was 8.3/sec) — more chaotic, film-like.

### New Features

**`--charset-file PATH`** (v11.0.0):
- Load custom characters from a file. Overrides `--charset` preset.
- File format: one char per line, or single line of characters.
- UTF-8 supported (kanji, Latin, symbols).
- Wide/zero-width characters (emoji, CJK fullwidth) are automatically
  filtered with a warning — prevents screen corruption.
- Usage: `cosmostrix --charset-file ~/my-chars.txt`

### Bug Fixes

**`--charset-file` wide-char crash** (v11.0.0):
- Emoji (🐺) and CJK fullwidth characters caused screen corruption
  (jitter, column misalignment) because the renderer is column-based
  and assumes 1 cell per character.
- Fix: filter wide/zero-width characters using `unicode_width` crate
  (same filter as built-in charset presets). Warns on stderr with
  skipped character codepoints.

---

## v10.0.0 — Peak Performance & Stability

Major performance optimization and stability hardening release.
+76.5% FPS improvement over v5.0.3 baseline through three optimization
phases plus a brutal pre-release audit. Lightning feature removed per
user request (never reached satisfying visual feel). License enforced
as GPL-3.0-only across all 171 source/doc/config files.

### Performance — Phase A: Hot-Path Optimization (+73.8% FPS)
- `phosphor_active` O(1) dedup via `phosphor_in_active` BitVec —
  eliminated 5K-100K wasted ops/frame from linear `contains()` scan
- `head_brightness()` hoisted out of per-line loop — eliminated 4K
  redundant `Instant::elapsed()` + `exp()` calls/frame
- `is_bright()` / `is_dim()` cached in `DrawCtx` — eliminated 100-300
  per-cell calls/frame when glitchy
- `viewport_edge_fade()` precomputed as LUT per terminal resize —
  eliminated 300-1000 float divisions/frame
- `phosphor_fresh` incremental clear — replaced O(W×H) `fill(false)`
  with ~200 bit clears
- `monolith_breathing_factor` computed once per stream, passed to both
  `draw_spine` and `draw_segments` — eliminated redundant cross-module
  call
- `zactrix monolith_*` functions marked `#[inline]` for cross-module
  inlining

### Performance — Phase 2: Structural (+1.6% FPS)
- Spawn free-list: `droplet_free_list: Vec<usize>` replaces O(N) linear
  scan with O(1) pop/push lifecycle
- Terminal flat dirty pairs: single `Vec<usize>` + single sort replaces
  nested `Vec<Vec<usize>>` — better cache locality, no per-row realloc

### Stability — Pre-Release Audit Fixes
- **CRITICAL**: Panic hook no longer writes to stdout (was racing with
  `Terminal::drop`'s BufWriter flush, leaking rain onto user's main
  terminal screen)
- **HIGH**: Added SIGQUIT to graceful shutdown signal set (was defaulting
  to core dump, bypassing all cleanup)
- **HIGH**: Added `debug_assert!` guard + `.min(255)` clamp in
  `fill_color_map` u8 cast (prevents latent panic if palette > 257 colors)
- **MEDIUM**: `Instant::now() - UPDATE_INTERVAL` → `checked_sub()` in
  bench.rs (prevents panic at boot epoch in containers/VMs)
- **MEDIUM**: `term_reinit.swap(false, Acquire)` → `AcqRel` for correct
  RMW memory ordering
- **MEDIUM**: `validate_err(...).unwrap()` → `.unwrap_or(s)` for
  defense-in-depth
- **LOW**: `tp + 1` → `tp.saturating_add(1)` in droplet/rain hot path

### Dead Code / Bloat Removal
- Deleted `column_transition_delay_ms: Vec<u16>` field (never read)
- Deleted `EVENT_MAX_CONCURRENT` constant (never referenced)
- Removed stale `#[allow(dead_code)]` on `EVENT_RNG_XOR` (is used)

### Feature Removal
- **Lightning system completely removed** (~3000 lines deleted). The
  atmospheric lightning feature (Storm Mode, Weather Director, bolt
  families, illuminate, global pulse) never reached a satisfying visual
  feel after multiple tuning iterations. Removed entirely rather than
  shipped in a poor state. Ghost event (phosphor ghost kanji) retained
  as a separate atmospheric feature.

### License
- Enforced `GPL-3.0-only` across all 171 source/doc/config files
- Fixed `scripts/check-headers.sh` stale `EXPECTED_LICENSE` variable
- Extended `check-headers.sh` to scan `*.md` files (was .rs/.sh/.toml/
  .yml only)
- Updated `LICENSE` body: removed "or (at your option) any later version"

### Benchmark
```
v5.0.1 baseline:    avg_fps 21,359  | frame_time 0.046ms | p99 0.058ms
v5.0.3:             avg_fps 27,869  | frame_time 0.035ms | p99 0.046ms
v10.0.0:            avg_fps 39,147  | frame_time 0.025ms | p99 0.030ms
Gain (v5.0.3→v10):  +40.5% FPS      | -28.6% frame time  | -34.8% p99
Gain (v5.0.1→v10):  +83.3% FPS      | -45.7% frame time  | -48.3% p99
```

---

## v5.0.0 — Nightfall

Cinematic UX + Product Identity Release. Polishes discoverability,
error messages, help text, and configuration UX to product-grade quality.
Establishes the cinematic breathing language as an authoritative
reference for how visual transitions and atmospheric effects should feel.
No renderer hot-path rewrite. No benchmark output field changes.
No 50k FPS promise. Terminal writer remains single-owner.
Benchmark honesty preserved.

### Added
- `--show-preset <NAME>` flag: display full preset details including
  description, overridden parameters, and effective values for any
  named preset. Makes preset behavior inspectable without running the
  renderer. Commit `e9f7b3b`.
- `config/cosmostrix.example.toml`: well-commented example configuration
  file with documented defaults and three profile examples (calm-night,
  cinematic, and dense-stress) ready to copy into `~/.config/cosmostrix/
  config.toml`. Commit `e9f7b3b`.
- `docs/CINEMATIC_BREATHING.md`: authoritative cinematic breathing
  vocabulary and pacing contract defining eight terms (Rest, Pulse,
  Whisper, Compression, Void, Signal, Storm, Breath Cycle), eight pacing
  rules, naming conventions, a 10-layer state hierarchy, and six
  anti-patterns. Commit `6289f41`.
- Cinematic breathing vocabulary with formal definitions for all
  atmospheric intensity levels, establishing a shared language for
  future development and documentation.
- Pacing contract: no instant visual state changes, default state is
  always Rest, Storm is never a default, transitions must be perceptible
  as breathing rather than flickering.
- `--profile` help text now includes `(see --list-profiles)` cross-
  reference so users know where to find available profiles.

### Changed
- Error messages follow a consistent pattern: `error: unknown <type>
  '<value>'` followed by a discovery hint line suggesting the
  appropriate `--list-*` flag. This applies to `--preset`, `--scene`,
  `--color`, `--charset`, and `--profile` validation errors.
- `--color` validation exit code changed from 2 to 1 for consistency
  with other user-input validation errors.
- `--charset` error message changed from "unsupported" to "unknown" for
  consistency with all other validation error messages.
- `--color` error message changed from inline parenthetical format to
  a separate hint line for consistency with other discovery hints.
- `docs/ROADMAP.md` updated with v5.0.0 Nightfall active development
  section, phase table, and cinematic breathing language reference.
- `docs/V5_NIGHTFALL_PLAN.md` created as the full v5.0.0 planning
  document covering scope, non-goals, release safety, phase plan, and
  Android/Cosmostrix Live boundaries. Commit `dc27e6f`.
- `docs/cosmostrix-next-vision.md` created for future sibling product
  (Cosmostrix Live) exploration as an explicitly exploratory document.
  Commit `dc27e6f`.
- `--help` output reorganized with a DISCOVERY section grouping
  `--list-presets`, `--list-profiles`, `--list-scenes`, and
  `--show-preset` for better scannability.

### Fixed
- `--profile` help text previously lacked a cross-reference to
  `--list-profiles`, making profile discovery unintuitive for new
  users. Now includes `(see --list-profiles)` hint.
- `--charset` error message used "unsupported charset" instead of
  "unknown charset", breaking the consistent `error: unknown <type>`
  pattern. Now uses the consistent format.
- `--color` error message used an inline parenthetical hint instead of
  a separate discovery hint line, inconsistent with all other
  validation errors. Now uses a separate line.

### Release Safety
- All v4.9.0 release guard mechanisms inherited and active.
- Terminal writer remains single-owner.
- Benchmark honesty preserved: no fake benchmark progress, no cherry-
  picked runs, no omitted metrics.
- No renderer hot-path changes.
- No benchmark output field changes.
- Terminal lifecycle contract remains authoritative.
- 993 deterministic tests passing.
- No new dependencies.

---

## v4.9.0

The Wolf: Release Guard + Terminal Runtime Contract. Hardens the release
process with mandatory pre-tag gates, automated benchmark reporting,
terminal lifecycle documentation, and doctor/report polish. No renderer
hot-path behavior changes and no benchmark output field changes.

- Release guard foundation (Phase 1): 10-gate (now 11-gate) pre-tag
  checklist in `docs/RELEASE_GUARD.md` ensures benchmark reports,
  version metadata, docs guards, CI, and terminal lifecycle verification
  all pass before any release tag is created. Commit `cf63254`.
- Benchmark report automation (Phase 2): `scripts/release-benchmark-report.sh`
  implements full 5-run benchmark collection and Markdown report generation
  with invariant validation. Commit `f3b6b63`.
- Terminal lifecycle matrix (Phase 3): `docs/TERMINAL_LIFECYCLE_MATRIX.md`
  documents expected cleanup behavior across 14 terminal lifecycle paths
  including normal exit, SIGINT, SIGTERM, SIGHUP, SIGTSTP/SIGCONT, SIGKILL,
  `--reset-terminal`, Windows Terminal, tmux, ssh, headless, benchmark mode,
  and doctor mode. Commit `294ad65`.
- Doctor/report polish (Phase 4): `--doctor` output now includes lifecycle
  contract fields (`signal_exit`, `sigkill`, `terminal_writer`). `--reset-terminal`
  help text clarified as destructive recovery. Commit `43e3dc9`.
- Terminal cleanup honesty preserved:
  - Normal exit (q/Esc): non-destructive mode/style restore.
  - `--reset-terminal`: explicit destructive recovery (clears screen,
    purges scrollback, resets modes).
  - SIGINT/SIGTERM/SIGHUP: catchable cleanup with viewport clear.
  - SIGKILL: cannot be caught or guaranteed. Fork guard is best-effort,
    Linux-only.
- Release guard Gate 7 (terminal lifecycle verification) requires
  `--doctor` lifecycle contract fields, manual exit testing, and
  SIGKILL honesty.
- 944 deterministic tests, all passing.
- Terminal writer remains single-owner.
- `compute_parallelism` remains `disabled`.
- `actual_execution` remains `single-threaded-renderer`.
- No new dependencies.

## v4.8.0

Zactrix Integration + Terminal Cleanup Hardening. Color pipeline optimization
from the zactrix lab with signal-exit terminal cleanup fixes. No default
visual behavior change and no active parallel compute.

- Integrated accepted zactrix color pipeline optimization from lab source
  `e7253e7` (`zactrix-20k-lab`) via manual adaptation. No direct lab merge.
  Commit `ce8dc81`.
- Single RGB decode path and integer brightness blend path replace
  redundant per-cell color computation, reducing pipeline overhead while
  preserving identical visual output.
- Cached binary pool detection avoids redundant `contains_key` lookups
  during stream spawning.
- `set_force` cleanup optimization removes unnecessary work on cells
  that are already marked dirty.
- 50k FPS lab (`zactrix-50k-lab`) documented as not reached and not a
  release promise. Rejected optimization attempts stay rejected.
- Signal-exit cleanup hardening (Phase 4): signal handler threads no
  longer race on stdout with the main loop's buffered writer.
- Visible residue fix (Phase 4B): catchable SIGTERM/pkill-TERM now clears
  the alternate screen viewport before switching back, preventing rain
  frame glyphs from bleeding into the main screen. Fork-guard child
  process silenced on normal parent exit to prevent stdout races.
- Cross-platform signal cleanup imports fixed for Windows CI.
- Windows Terminal `--reset-terminal` issue #15 verified clean.
- 891 deterministic tests, all passing.
- Terminal writer remains single-owner.
- `compute_parallelism` remains `disabled`.
- `actual_execution` remains `single-threaded-renderer`.
- No version bump until this release prep.
- No new dependencies.

## v4.7.0

Profile Ecosystem. Documentation, validation UX, and release-candidate smoke
coverage for the profile system with no default visual behavior change and no
active parallel compute.

- Profile ecosystem contract documenting profile precedence
  (CLI > profile > config > defaults), profile resolution, and mutation
  semantics.
- Profile examples documentation with ready-to-copy config snippets for
  common profile use cases.
- Config dump and `--list-profiles` enhanced with profile documentation
  pointers to `PROFILE_ECOSYSTEM.md` and `PROFILE_EXAMPLES.md`.
- Profile validation UX polish with clear, actionable error messages:
  unknown profiles mention `--list-profiles`, invalid fields/values show
  expected formats, storm rejection is explicit.
- Unknown profile actionable error: both CLI and config paths produce
  clear diagnostics pointing to `--list-profiles`.
- Storm unavailable: error messages and config dump consistently state
  that storm is unavailable.
- Profile RC smoke coverage in `scripts/rc-smoke.sh` with 11 profile-related
  checks and `docs/RELEASE_CANDIDATE.md` updated with v4.7 profile checklist.
- Default remains disabled/protected/identity. No live atmosphere enabled by
  default.
- Terminal writer remains single-owner. Compute parallelism remains disabled.
- No zactrix-20k-lab merge.
- 858 deterministic tests, all passing.

## v4.6.0

Controlled Atmosphere Expansion. Docs, test infrastructure, and CLI
discoverability release with no default visual behavior change and no active
parallel compute.

- Controlled atmosphere expansion contract with state matrix (identity, whisper,
  shadow, protected) and six regimes (calm, pulse, signal, compression, void,
  monolith-pressure). Storm is intentionally unavailable.
- Preset registry with six controlled atmosphere presets: atmosphere-calm
  (identity), atmosphere-pulse, atmosphere-signal, atmosphere-compression,
  atmosphere-void, atmosphere-monolith-pressure (all whisper). Presets are
  opt-in only.
- Preset UX documentation, config/profile examples, and config dump atmosphere
  lines for discoverability.
- `--list-profiles` enhanced with controlled atmosphere preset section showing
  mode, regime, and shadow level for each preset.
- RC smoke script hardened with six atmosphere checks (preset listing, storm
  rejection, controlled-live field verification, disabled+non-calm identity,
  color sun sticky).
- 800 deterministic tests, all passing.
- Default remains disabled/protected/identity. No live atmosphere enabled by
  default.
- Storm unavailable. Terminal writer remains single-owner.
- No zactrix-20k-lab merge.

## v4.5.0

Zactrix Foundation + Depth Regression. Architecture and test infrastructure release with no default visual behavior change and no active parallel compute.

- Split Zactrix Engine architecture into core/cache/render/system/scheduler/metrics modules.
- Added honest ZACTRIX SYSTEM diagnostics (runtime_mode, cpu_budget, render_plan, compute_parallelism, idle_policy).
- Added depth regression lab for Monolith Rain visual stability (15 categories, deterministic guards).
- Split docs, monolith, and scene regression tests into focused module directories to keep all files under 1000 LOC.
- Added roadmap closure docs covering v4.6/v4.7/v4.8/v5 release trajectory.
- No default visual behavior change.
- No active parallel compute.
- Terminal writer remains single-owner.

## v4.0.1

Fixed version output build label to include the optimized CPU tier, matching doctor/benchmark diagnostics.

- `cosmostrix -V` / `--version` now reports the canonical build label (e.g. `linux-x86_64-v3`) from `COSMOSTRIX_BUILD`, consistent with `--doctor`, `--benchmark`, and `--info`.
- Added `canonical_build_label()` as the single source of truth for the build label across all output paths.
- Added deterministic tests to prevent this mismatch from returning.

## v4.0.0

Full Atmosphere Engine groundwork and signature Monolith Rain maturation release.

Highlights:
- Signature Monolith Rain as the production default, with refined sparse data pillars, subtle phase variation, clean afterglow, and bounded residue behavior.
- Zactrix Core / Zactrix Engine / Zactrix Cache groundwork for adaptive rendering architecture, while terminal writes remain single-owner.
- Atmosphere engine internal model, verifier, controlled-live config gate, visual whisper, shadow metrics, and A/B safety smoke tests.
- Terminal compatibility lab, doctor guidance, reset safety, color capability diagnostics, and clean terminal recovery.
- User scene/profile config with controlled atmosphere profile keys.
- Benchmark/endurance/report hardening with honest planned-vs-actual execution diagnostics.
- README demo refresh with GIF-first v4 preview, MP4 link, and binary/retro posters.
- Canonical metadata alignment across Cargo, README, runtime identity, and AUR packaging.
- Release-candidate smoke script and release checklist.

Safety/defaults:
- Default runtime remains protected and identity: `application_mode = disabled`, `effective_runtime = identity`, `shadow_risk = identity`.
- `auto_color_drift` remains off by default.
- `storm` is not config-safe in controlled-live config/profile mode.
- No actual multithreaded terminal rendering; benchmark reports planned engine mode honestly.

## v3.9.0

Internal v4.0.0 ground-work phase. No public API or visual behavior changes.

- Atmosphere visual whisper engine with bounded A/B smoke testing
- Whisper wiring guard and runtime shadow metrics
- Zactrix Core eBPF-inspired architecture discipline
- Self-referential guard string avoidance pattern
- Phase 10.5: atmosphere config honesty + profile smoke hardening (27 new tests)
- Added v4 demo poster and MP4 assets for README preview
- Made the v4 README demo GIF-first and removed the obsolete demo GIF
- Replaced single v4 demo poster with binary and retro themed demo screenshots
- 568 deterministic tests, all passing

## v3.1.0

**Monolith Rain Engine.** Plain `cosmostrix` now launches signature Cosmostrix
Monolith Rain: sparse structured vertical data pillars with segmented blocks,
subtle spines, visible gaps, and a clear brightness hierarchy. Classic Matrix
glyph rain remains available with `cosmostrix --scene matrix`.

## v2.2.0

**Stability, maintainability, and supply-chain hardening release.** No visual
or CLI behavior changes.

- All `*.rs` files are under 1,000 gross lines (enforced by `check-rs-loc.sh` in `check-all`)
- Module splits: `src/cloud.rs` → `src/cloud/` (8 modules), `src/interactive.rs` → `src/interactive/` (6 modules), `src/main.rs` → `src/app.rs` + `src/cli.rs` + `src/info.rs` + `src/main.rs`
- Cloud tests split into `tests/mod.rs` (core) and `tests/tests_phosphor.rs` (phosphor/ghost)
- Added endurance testing documentation ([ENDURANCE.md](docs/ENDURANCE.md)) and resource summary script
- Added supply-chain hardening policy ([SUPPLY_CHAIN.md](docs/SUPPLY_CHAIN.md))
- Added terminal stability audit ([STABILITY_AUDIT.md](docs/STABILITY_AUDIT.md))
- Added SIMD feasibility audit ([SIMD_FEASIBILITY.md](docs/SIMD_FEASIBILITY.md))
- Engine module splits: `cloud/mod.rs` → `scene_runtime.rs` + `runtime_controls.rs` (scene switching and runtime controls extracted from core module)
- Fixed clippy module-inception and unused import warnings
- Regression suite passes, clippy clean, fmt clean

## v2.1.0

**Visual contrast & readability overhaul** — body glyphs are now clearly readable
with stronger head/body/trail hierarchy while preserving the calm cinematic identity.

- Tuned exponential trail decay (K: 3.0 → 1.8) for readable body glyphs across the full trail length
- Raised parallax brightness (far: 35→55%, mid: 80→90%) so depth layers are visible, not invisible
- Increased phosphor residual energy (120→160) for more visible CRT afterglow fadeout
- Extended head linger duration (100→300ms) for smoother cinematic head fade
- Added head self-bloom (12% white blend) making the head clearly the brightest element
- Softer head brightness mapping (0.5+0.5×hb → 0.7+0.3×hb) preventing abrupt head disappearance
- Raised luminance climate minimum (60→75%) and saturation minimum (50→70%) to prevent muddy/dim periods
- Raised fog vignette minimum (25→35%) to keep edge glyphs faintly visible
- Reduced far-layer glyph dimming (30→15%) — already dim from parallax brightness
- TrueColor green palettes now use 24-bit RGB gradients instead of ANSI 256-color indices, with proper bright green head instead of cyan-white
- Reduced profile luminance offsets (Monolith: -0.1→0, Void: -0.2→-0.1, Decay: -0.15→-0.05, Static: -0.25→-0.1)

**Safety & hardening fixes:**

- Tab key safely ignored (was toggling shading mode, causing ghost background glyph flood)
- Paste safety (bracketed-paste burst suppression ignores shortcut letters during paste)
- Pause/resume with cinematic smoothstep easing (no snap on resume)
- Color and charset transitions use cinematic top-to-bottom wave propagation
- Mouse mode default-off, opt-in with `--mouse`
- Bottom-row phosphor decay acceleration prevents "concrete wall" accumulation
- Ghost glyph threshold prevents stale charset from filling background on full redraw
- Safe terminal cleanup on all exit paths (RAII guard + `--reset-terminal`)

## v2.0.0

- Fixed stale glyph artifacts in the top visible rows during charset and theme changes.
- Fixed long-idle rain/trail resync issues with wall-clock redraw scheduling and focus/input redraw resync.
- Clarified benchmark dirty-cell and color-mode metrics so differential rendering reports are easier to interpret.
- Fixed direct-color auto-detection for `xterm-direct` and `tmux-direct`.
- Removed unused low-value support code while preserving rendering behavior.
- Completed 10h+ visual soak checks across Alacritty, Konsole, and WezTerm.
- Resource monitoring found no memory, file descriptor, thread, swap, CPU, or IO leak during the release soak.
