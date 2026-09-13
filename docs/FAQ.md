<!-- SPDX-License-Identifier: GPL-3.0-only -->

# cosmostrix FAQ — Q/A

Answers to recurring configuration and rendering questions, written
once so the answer never drifts. Each entry names the source files
that implement it — source code is the single source of truth; if a
doc and the code disagree, the doc is wrong.

Index:

- [Colors & OKLab](#colors--oklab)
- [Custom blocks](#custom-blocks)
- [Rendering engine](#rendering-engine)
- [Config validation](#config-validation)

## Colors & OKLab

### Q: Does cosmostrix use OKLab when I define a custom color palette in config.toml?

A: YES. A `[colors-custom.<name>]` block routes through exactly the
same OKLab polar gradient engine as every built-in theme — there is
one palette pipeline, not two:

```text
[colors-custom.<name>] rain stops
  -> CustomPaletteDef::to_palette()            (colors_custom.rs)
  -> colors_from_stops(TrueColor, stops, 9)    (chroma palette/mod.rs)
  -> gradient_from_stops_oklab(stops, 9)       (chroma gradient/mod.rs)
  -> 9 perceptually-uniform palette samples
```

The user's raw stops (2–9 of them) are gradient control points; the
engine resamples the gradient to exactly 9 samples
(`COLORS_CUSTOM_PALETTE_STEPS` = 9) before the palette is used. This
eliminates banding on long rain trails and puts custom palettes on
identical footing with built-in themes like `synthwave` or `cosmos`
(the byte-identity is pinned by the
`to_palette_matches_builtin_gradient_path` test). Before the
masterclass round this went through, `to_palette()` returned the raw
stops verbatim — a 3-stop palette stayed 3 entries while every
built-in theme carried 9 OKLab-interpolated entries; that asymmetry
is gone.

### Q: Why is the rain-stop maximum 9?

A: Because 9 is exactly what the engine keeps. The OKLab resample
step produces 9 samples regardless of how many stops you write, so
stops beyond 9 are provably discarded input. Before NIGHT-hunt-37
(2026-09-13) the documented limit was 64 and the collector enforced
it as a silent truncation — a 67-stop block ran with 64 stops and no
error. The contract is now: min 2 stops (a gradient needs two
colors), max 9, and an over-limit block is a HARD error on every
surface (startup exit 2, `--testconf` exit 2, live-reload watcher
reject), never a silent cap. 7 stops is the sweet spot the config
template suggests.

### Q: What is OKLab and why does cosmostrix interpolate in it?

A: OKLab (Björn Ottosson, 2020) is a perceptually uniform color
space: Euclidean distance between two colors matches how different
humans perceive them. Interpolating in plain sRGB/linear-RGB rotates
hue through the desaturated center of the RGB cube on hue-crossing
gradients (red to green, blue to yellow), producing muddy brown/gray
midpoints. OKLab keeps the midpoint saturated.

cosmostrix additionally interpolates the chroma axes polar (chroma
magnitude lerped linearly, hue rotated through the shortest arc)
rather than Cartesian — on opposing-hue gradients the Cartesian
midpoint passes near (a, b) = (0, 0) = gray; polar stays saturated.
This matches the W3C CSS Color Module Level 4 default for `oklch`
interpolation. Polar is the sole production gradient path (the
legacy sRGB-linear and Cartesian variants were removed in v30).
Full rationale with examples: the module docs of
`src/engine/chroma_dragon_engine/gradient/mod.rs`.

### Q: Does OKLab apply to built-in themes too?

A: Yes. Every gradient-style built-in theme is declared as
`ThemeColors::Stops { stops, steps: 9 }` in
`src/engine/chroma_dragon_engine/catalog/themes.rs` and routes
through the same `colors_from_stops` →
`gradient_from_stops_oklab` chain (see `catalog.rs`). Fixed-list
themes (explicit color tables) skip the gradient step by design.

### Q: Where else does OKLab appear?

A: Four production surfaces, all in the Chroma Dragon engine:

1. Palette gradients (this doc's main subject).
2. The intro cinematic — color transitions during the singularity to
   burst to morph to rain sequence use `oklab_blend_rgb`
   (`gradient/mod.rs`, driven by `intro_colors.rs`).
3. Palette transition smoothing — when the active palette changes
   (live reload, ambient phase switch, crystal drift), the old and
   new colors are blended in OKLab and L is smoothed over the
   transition table (`shaders/transition/mod.rs`).
4. The 300 ms top-to-bottom palette wave — the Crystal Dragon's
   ambient/theme switches trigger the Chroma Dragon's OKLab wave
   transition (`shaders/transition/`, documented in
   `docs/CRYSTAL_DRAGON_ENGINE.md`).

### Q: What happens to my custom palette on a terminal without TrueColor?

A: The palette is still built through OKLab first; the final samples
are then quantized to the terminal's mode (`colors_from_rgb` in
`chroma palette/mod.rs`): Color256 maps each sample to the nearest
xterm-256 entry, Color16 maps to the nearest of 16, Mono collapses
to white. Custom palettes are user-defined 24-bit hex by definition,
so TrueColor is the intended mode — on weaker modes you get the
closest representable rendering of the same perceptual gradient, and
mono loses the palette entirely (by design: mono rain is white).

## Custom blocks

### Q: What makes a custom block "complete"?

A: Since NIGHT-hunt-37 the contract is strict and identical on every
surface (startup, `--testconf`, the live-reload watcher):

- `[colors-custom.<name>]` must define BOTH `bg` and `rain` (the
  deprecated `stops` alias satisfies the rain slot, but `rain` and
  `stops` together is an overload error). Values must be valid hex.
- `[charset-custom.<name>]` must define `set`, non-empty.
- `[scene-custom.<name>]` must define all seven dimensions: `rain`,
  `color` or `colors-custom`, `charset` or `charset-custom`, `fps`,
  `speed`, `density`, `glitch-level`.
- A header-only block (every field line commented out or missing) is
  a hard error — the parser records custom-block headers precisely
  so these blocks cannot hide.
- Duplicate keys, duplicate section headers, and unknown fields are
  hard errors (unknown keys carry did-you-mean hints).
- Names: 1–64 chars, letters/digits/`-`/`_`. Since NIGHT-hunt-39
  (2026-09-13) the entry budget is min 1 / max 64 per namespace:
  every block needs at least one field entry (the completeness rules
  above), and at most 64 blocks fit per namespace — the SAME 1..=64
  policy the ambient scheduler uses for its entries.
- `ambient.<HH-MM>` schedules: 1..=64 entries (0 entries simply turns
  the scheduler off, not an error); 65+ is a hard error on every
  surface, never the silent 256-truncate the old collector applied.

The bounds table with rationale lives in `docs/RULES.md` ("Custom
Block LTS Bounds"). The validators: `colors_custom::strictness`,
`testconf::custom_block_headers`, `scene_custom` completeness, and
the per-namespace name-length and block-count checks.

### Q: I edited config.toml while cosmostrix was running and it kept the old colors. Why?

A: Mid-run edits are validated by the live-reload watcher with the
same strictness as startup; a rejected edit keeps the last valid
config running (the rejection is logged — run with `--verbose` to
see it). Keys that cannot change mid-run (documented in
`docs/LIVE_RELOAD_BEHAVIOR.md`) need a restart. The honest
limitations of live reload are documented in
`docs/CONFIG_LIVE_RELOAD_DISCLAIMER.md`.

## Rendering engine

(NIGHT-docs-4, 2026-09-13 — the recurring "is it real?" questions,
answered once from source.)

### Q: Does cosmostrix really have an independent rendering engine?

A: YES — in the strongest sense: the entire paint pipeline is
authored in this repo; there is no TUI framework underneath. The
Cargo manifest pulls no ratatui/tui-rs/ncurses — the only terminal
crate is `crossterm`, and its role is bounded to setup, teardown and
input (see the next question). The rendering substrate itself is
`src/engine/cosmic_dragon_engine/` with four cooperating subsystems:

| Subsystem | Location | Role |
|-----------|----------|------|
| Cloud simulation | `cloud/` | rain simulation, monolith, phosphor decay, the render pipeline that decides each cell's glyph and color |
| Frame buffer | `frame.rs` | differential 2D cell grid with double-buffered, generation-based dirty tracking (O(1) per-frame clear) |
| Terminal output | `terminal/` | raw-mode guard, alternate screen, RLE-batched ANSI diff emission, 64 KiB buffered writer, `/dev/tty` fallback, I/O recovery |
| Runtime types | `runtime.rs` | the `ColorScheme`/`ColorMode`/`ColorPipeline` vocabulary |

The frame path, file by file (paths verified 2026-09-13):

```text
event loop            src/interactive/event_loop.rs
  -> run_sim_and_draw src/interactive/event_loop_sim_draw.rs
     -> cloud.rain_at(frame, now)   simulation writes Cell values into Frame
     -> term.draw(frame)            only DIRTY cells re-emitted, RLE-batched
        -> one write() syscall per frame (64 KiB buffer)
```

`Terminal::draw` is a two-path strategy: full redraw when dimensions
or semantics changed (no blanket clear on semantic-only changes —
that flickered), and the diff path otherwise: dirty indices grouped
by row, sorted, scanned for contiguous same-style runs so cursor
movement and SGR changes are minimized.

### Q: Is crossterm the renderer?

A: No. crossterm owns terminal *setup and teardown* (raw mode,
alternate screen, line-wrap — `terminal/cleanup.rs`) and *input event
decoding* (`crossterm::event` in the interactive loop). The hot draw
path never routes through crossterm's command traits: `terminal/
sgr_format.rs` formats ANSI SGR bytes directly into a `Vec<u8>` with
the branchless `bolt` number formatter, and `terminal/draw.rs`
appends raw `\x1b`-prefixed sequences run-by-run. crossterm command
enums appear only on cold paths (mode switches, cleanup). That is
why the `--benchmark` HUD reports dirty-cell counts and write sizes
from first-party instrumentation — no third-party render layer sits
in between to ask.

### Q: Does the Cosmic Dragon really render?

A: Yes — the Cosmic Dragon Diff-Based Rendering Engine IS the
renderer described above; "Cosmic Dragon" is its name, not a
metaphor. Every glyph you see is computed by `cloud/` (position,
character, phosphor state), colored by the Chroma Dragon (see below),
written into the `Frame` grid, and emitted by `Terminal::draw`.
Do not confuse it with `src/cosmic_dragon_incubator/` — a small
experimental namespace (~200 LOC), NOT a peer engine (the
naming-disambiguation note lives at the top of
`src/engine/cosmic_dragon_engine/mod.rs`).

### Q: What do the other dragons do — which of them renders?

A: Exactly ONE dragon paints. The others feed it:

- **Cosmic Dragon** (`src/engine/cosmic_dragon_engine/`) — THE
  renderer: simulation + frame buffer + ANSI terminal pipeline.
- **Chroma Dragon** (`src/engine/chroma_dragon_engine/`) — the color
  engine. It decides WHICH color each cell gets
  (`shaders/base.rs::resolve_cell_color`), builds every palette
  through OKLab (see the Colors section above), and quantizes to the
  terminal's color mode. It never writes to the terminal — it hands
  `crossterm::style::Color` values to the Cosmic Dragon's frame cells.
- **Crystal Dragon** (`src/engine/crystal_dragon_engine/`) — ambient
  intelligence: the time-of-day ambient scheduler and the palette
  drift engine. It decides WHEN the palette changes; it does not
  render. Its 300 ms palette wave is executed by Chroma's transition
  shaders, then painted by Cosmic.
- **Power Dragon** (`src/central_control_power_dragon/`) — adaptive
  throttle: bands density and reduces FPS under CPU pressure. It
  controls the frame RATE, not the frame CONTENT.

So the pipeline's division of labor is: Crystal decides when,
Chroma decides what color, Power decides how fast, Cosmic paints.
Each engine directory carries its own `README.md`/`RULES.md` and the
locked `KEY.md` (chroma) — the engine topology is documented in
`src/engine/mod.rs` and `src/engine/cosmic_dragon_engine/mod.rs`.

## Config validation

(NIGHT-hunt-38-supermassive, 2026-09-13 — the typo classes the owner
found by manual testing on commit 6198431, now closed.)

### Q: I typo'd the separator (`set == "x"` or `set : "x"`). What happens?

A: Both are rejected as malformed lines with a targeted `# ERROR:`
note — the same verdict on all three surfaces (`--testconf` exit 2,
startup exit 2, live-reload watcher reject). Before the fix these two
typos of the SAME mistake behaved differently: the `:` form errored
(no `=` in the line), but the `==` form SILENTLY PASSED — the parser
split at the first `=`, stored `= "x"` as the value, and a
`[charset-custom]` block happily ran with garbage glyphs while
`--testconf` said PASS. The `==` typo on an `ambient` key was even
worse: the stray `=` made the validator misfire the "legacy
multi-field format" migration essay for a format the user never
wrote. A QUOTED value whose content starts with `=` (`set = "=x"`)
remains legal — the guard inspects the raw value before
quote-stripping (the bug #19 quoting invariant).

### Q: I typo'd a boolean (`msg-mode = truee`). What happens?

A: Rejected, uniformly: `--testconf` fails with `expected true/false
(or yes/no, on/off, 1/0)`, startup exits with code 2, and the
live-reload watcher rejects the edit. Before the fix `--testconf`
passed `truee` (no validator arm for `msg-mode`), while the runtime
printed a bare one-line error and KEPT RUNNING with the default —
two different verdicts for one typo. The accepted vocabulary matches
`parse_bool_config` exactly: true/false, yes/no, on/off, 1/0,
case-insensitive.

### Q: Why did my old ambient multi-field entry stop working, and what do I migrate to?

A: The multi-field format
(`ambient.15-00 = neon-purple, signal, speed=50, density=0.65`) was
removed in favor of a single scene name. The rejection message
includes a copy-paste migration recipe. One caveat the NIGHT-hunt-38
audit caught: the recipe used to recommend `base-scene` — a field
REMOVED in v80.0.0-beta.2 — so following it produced a fresh error.
The recipe now shows the complete-block contract (all seven
`[scene-custom]` fields required). If you see the essay, migrate to a
`[scene-custom.<name>]` block and reference it by name at top level.
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
