<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-25 part 2 — the hot cell-draw family, one shared bundle design for the shader/render pair

Part 1 (cold families) closed the census to six remaining
`#[allow(clippy::too_many_arguments)]` suppressions, all on per-frame
hot paths, and explicitly deferred them: "those are per-frame
inner-loop signatures where a parameter-bundle design deserves
undivided attention." This commit is that attention. The six
signatures, in call-frequency order:

| Signature | Path | Old args | Bundle |
|---|---|---|---|
| `resolve_cell_color` | shaders/base/mod.rs | 8 (7 + ctx) | `CellPaint` |
| `DrawCtx::get_attr` | cloud/render.rs | 8 (7 + self) | `CellPaint` |
| `draw_solar_cell` | solar_flare/draw.rs | 8 (6 + ctx + frame) | `SolarCellPaint` |
| `render_particle_cell` | intro_style/mod.rs | 10 (7 + frame + w + h) | `ParticlePaint` |
| `post_rain_processing` | cloud/post_rain.rs | 9 (7 + self + frame) | `PostRainInputs` |
| `emit_cell_lean` | bench/bench_io.rs | 9 (8 + buf) | `StyleCursor` |

src/ now carries ZERO too_many_arguments suppressions (verified by
`rg '^\s*#\[allow\(clippy::too_many_arguments\)\]' src/` — empty).

## The pair: CellPaint, one design on both sides

`resolve_cell_color()` (the shader) and `DrawCtx::get_attr()` (the
renderer's thin wrapper) forwarded the same seven per-cell positionals
in lockstep: `palette_slot, line, col, val, loc, head_put_line,
length`. The pair carried two classic positional hazards:
`line`/`col` are both `u16`, and `head_put_line`/`length` are both
`u16` — a swapped pair at any call site compiles cleanly and silently
paints the wrong cell. The hottest call site in the engine feeds this
pair: `Droplet::draw`'s per-cell loop (every visible droplet cell,
every frame, 14 rain styles).

The bundle is the fix, and it is ONE design shared by both sides
(the whole point of the pair): named fields make every call site
self-documenting, the lint threshold drops to two arguments on both
sides, and a future per-cell input (a new shader phase) becomes a
named-field addition instead of an eighth positional on two
signatures that must be edited in lockstep.

Performance contract: all fields are `Copy` scalars, so the struct is
`Copy` (16 bytes: u8 + 2×u16 + char + CharLoc + 2×u16, padded). LLVM's
SROA (scalar replacement of aggregates) dissolves it at the call site
into the same register-level parameter passing the seven positionals
used. The shader body destructures once at the top and reads the
seven scalars exactly as before — zero algorithm change, zero
allocation, zero indirection. The 10 s A/B benches (run after commit,
per the owner's sequencing rule) confirm: performance-neutral within
noise.

## The family: four more bundles, same pattern, hazard-specific docs

- **`SolarCellPaint`** (solar_flare) — the old six positionals had the
  same `col`/`line` u16 pair hazard plus a float `factor` neighboring
  the u8 `level` palette stop. Four call sites in the flare surface,
  dim-surface, core, descent, and comet-trail paths now construct
  named fields.
- **`ParticlePaint`** (intro) — seven positionals with `x`/`y` both
  f32 and a subtle same-typed neighbor swap: the logo trail passes
  `trail_brightness` where the head passes `life_t`. Four call sites
  (logo head, logo trail, cosmic head, cosmic trail).
- **`PostRainInputs`** (cloud post-pass) — eight positionals with the
  densest hazard cluster: `now`/`t1` both `Instant` plus three
  adjacent bools (`time_for_glitch`, `glitch_due`, `in_transition`).
  A swapped Instant pair compiles and silently mis-times the glitch
  windows; a swapped bool pair compiles and silently reschedules
  events. One call site in `rain_at.rs`.
- **`StyleCursor`** (bench io) — different shape: three `&mut`
  positionals (`cur_fg`, `cur_bg`, `cur_bold`) became one struct
  owned by the caller, passed as a single `&mut`. This also documents
  why the refs existed at all (borrow-splitting around `&self.color_cache`),
  and `emit_cell_lean` drops to 6 args with no allow. The signature
  history comment (9 → 7 → 8 → 6) is updated in place.

## The LOC split (an en-route structural fix)

The `CellPaint` struct + its doc pushed shaders/base/mod.rs over the
800-line hard cap (834). Split rather than trim:

- The three shader constants (`TRAIL_EXP_LUT`,
  `SHORT_DROPLET_LUMINANCE_REMAP_THRESHOLD`, `BAYER_4X4`) moved to
  helpers.rs — they were the oldest residents and helpers.rs is the
  established home for shared shader resources. `TRAIL_EXP_LUT` rides
  a `pub(crate) use` re-export so every `crate::...::base::TRAIL_EXP_LUT`
  path (and the docs referencing it) keeps resolving.
- The test-support helpers (`make_test_shader`, `slot_array`) moved to
  a new `test_util.rs` (cfg(test) module), joined by a new
  `test_paint()` fixture constructor that keeps the 47 shader test
  call sites one line each.

mod.rs lands at 768 lines. helpers.rs 232, test_util.rs 84.

## The hunt: findings beyond the owner's ask (verified)

1. **The neural commit shipped a red test (verified).**
   NIGHT-research-9 (5bb2bfd) added the neural scene to the cycle at
   the src level but missed part of the test update:
   `scene::tests::cycle_scene_forward_order` at HEAD still expected
   `cycle_scene("quasar", 1) == "classic"` while src cycles
   quasar → neural. Verified by stash-and-run: the test FAILS at
   HEAD (3 passed, 1 failed in the filter). The missed assertions
   (`quasar → neural`, `neural → classic`, the name-list entry, the
   count detector) ride this commit; the tree is green again
   (2671 tests, 0 failed).
2. **The neural research doc shipped without the standard disclaimer
   (verified by gate-keepers).** docs/research/NIGHT_RESEARCH_9_NEURAL.md
   was the only .md of 185 missing the "source code is truth"
   disclaimer block — gate-keepers FAILED 15/16 on the inherited tree.
   Fixed via the prescribed `inject-disclaimer.sh` (17 lines); the
   gate now reads 16/16.
3. **An anti-finding, documented to prevent re-litigation.** The
   obvious next optimization after the bundle — hoisting the
   per-droplet `ShaderCtx` construction out of the per-cell loop in
   `Droplet::draw` — was measured manually: noise-scale, contradictory
   deltas across scenes. The `#[inline]` pair already folds the chain;
   the compiler hoists what is loop-invariant. The result is recorded
   in `get_attr`'s doc comment so nobody burns a day re-measuring it.
4. **Considered and skipped (over-engineering guard).** Four
   `too_many_arguments` allows remain in test/interactive/ — stable
   test fixtures (`call_handle_keybinding_with_scene`, duplicated
   deliberately per file, marked "test fixture, stable"). Not hot
   path, not production surface; bundling them churns four green test
   files for zero hazard reduction. The frame primitives
   (`set`, `set_force`, `index`, `get`) already sit at 2-3 parameters
   with `Cell` as a value object — peak, leave them.

## Verification

- `./scripts/build.sh check-all -q` exit 0, well under the 2-minute
  local budget (warm cache): fmt + clippy `-D warnings` (all-targets,
  all-features) + cargo test + cargo-audit (the 1 pre-existing allowed
  unmaintained-crate warning) + headers + LOC.
- `./scripts/gate-keepers.sh` 16/16 after the disclaimer fix (tools:
  shellcheck, shfmt, actionlint, yamllint, codespell, ruff, markdownlint
  via npx).
- The three PTY smoke scripts (genesis/neural/quasar) were reformatted
  to `ruff format` shape — required by the gate-keepers ruff check
  (CI parity: "Project lint (codespell + ruff)"); no logic changes,
  byte-for-byte identical subprocess arguments.
- 2671 tests pass, 0 failed (up from 2561 at hunter-25 part 1: the
  four research-7/8/9 rounds added 110, plus the cycle-order repair
  restores the two neural assertions this commit carries).
- A/B 10 s benches per the owner's rule (run after commit, reported
  in the changelog entry): visual metrics (density gini, frame
  entropy) and performance metrics (fps, dirty cells) both neutral
  within noise — the expected result for a pure signature refactor
  whose bundle scalar-replaces to the old parameter passing.

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
