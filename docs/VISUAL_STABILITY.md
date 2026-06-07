# Visual Depth & Throughput Stability Policy

## Overview

Cosmostrix v3.6.0 introduces focused improvements to visual depth
perception and throughput stability reporting without rewriting the renderer
or changing the identity of Monolith Rain.

## Visual Depth Policy

### Background Modes

Cosmostrix supports three background rendering modes, each with distinct
visual expectations:

| Mode | Flag | Background | Visual Expectation |
|-------|------|-----------|-------------------|
| **Black** | `--color-bg black` | Solid `Color::Rgb(0,0,0)` | Cosmostrix paints every cell with a black background. Maximum depth contrast. Best for dark terminal emulators. |
| **Transparent** | `--color-bg transparent` | `None` | Cosmostrix does NOT paint any background. The terminal emulator's own background shows through. It does NOT change terminal emulator opacity. If your emulator has a white background, transparent will appear on white. |
| **Default-background** | `--color-bg default-background` | `None` | Equivalent to transparent. Cosmostrix relies on the terminal default background. Designed for user-configurable terminals. |

### Black Mode Depth Hierarchy

In black mode, the Monolith Rain visual depth hierarchy (from brightest to
dimmest) is:

1. **Hero Core** — the bottom cell of a Hero segment, white-bloomed.
   Always the brightest element on screen.
2. **Hero Hot** — upper cells of a Hero segment, bright afterglow.
3. **Body Mid** — body segment cells, the main visual body of rain.
4. **Body Dim** — fading trail edges, barely visible.
5. **Spine Ghost** — faint spine trace between segments, the most subtle element.
6. **Empty Space** — blank cells, completely black.

This hierarchy must have clear separation at each level. If any two adjacent
levels become visually indistinguishable, the depth perception degrades to a
flat "wall of grey" — this is the "muddy residue" artifact that
v3.6.0 specifically targets.

### Brightness Level Implementation

The `color_for_level()` function maps brightness levels to palette indices:

- **Ghost / Dim**: use `first_visible` (the faintest non-background palette
  entry), ensuring spine traces and dim segments are barely perceptible on
  black backgrounds. Their combined RGB sum should remain below 80 to
  prevent muddy mid-range greys.
- **Mid**: uses `2/5` of the palette range, providing clear body readability
  while maintaining separation from ghost/dim levels.
- **Hot**: uses `4/5` of the palette range, creating sharp afterglow contrast
  that separates hero segments from the body.
- **Core**: uses the brightest palette entry with an additional 10% white
  bloom, making hero tips unambiguously the brightest element on screen.

This separation is enforced by regression tests that verify strict
brightness ordering: ghost < mid < hot < core, and that ghost/dim cells
never exceed a dimness threshold on black backgrounds.

### Afterglow Contrast

The phosphor persistence system creates CRT-style afterglow by decaying
cell brightness over time after the trail passes. Key afterglow behaviors:

- **Fresh capture**: cells drawn by active droplets capture phosphor energy
  (capped at the bottom edge to prevent bright ghost residue).
- **Active trail protection**: cells within living droplet ranges are
  protected from phosphor decay, preventing the "concrete wall" artifact
  where active trail cells were progressively dimmed.
- **Blanked-cell protection**: freshly blanked cells start phosphor from
  residual energy but do NOT render ghost cells that would override the
  intentional blank.
- **Glyph threshold**: below `PHOSPHOR_GLYPH_THRESHOLD` (96/255 energy),
  the character glyph is no longer rendered, preventing stale cells from
  filling the background with dark charset glyphs while still allowing
  color-only dim patches for the final fade.
- **Bottom-row acceleration**: ghost cells in the bottom 8 rows decay 2.5x
  faster, preventing accumulation where droplets end and fewer new streams
  overwrite the residue.

On transparent backgrounds, afterglow works identically but the visual
contrast depends on the terminal emulator's own background color. Users
with light terminal backgrounds should expect dimmer afterglow perception.

### Transparent Mode Expectations

When `--color-bg transparent` is selected, Cosmostrix emits cells with
`bg: None` (crossterm ResetColor for background). The terminal emulator's
background is responsible for filling the background. This means:

- On a dark terminal, transparent looks similar to black mode.
- On a light terminal, transparent appears on a light background.
- Cosmostrix does NOT and MUST NOT override the terminal's choice.

### Guard Tests

Regression tests verify:
- Transparent mode never forces a solid black background.
- Black mode always paints solid black.
- Ghost/dim cells are never in the middle brightness range (prevents muddy grey).
- Hero/spine/trail/empty-space brightness levels are strictly ordered.
- Bottom rows do not accumulate persistent non-blank residue after normal frames.
- Clean exit leaves no persistent ghost glyphs in bottom rows.
- All benchmark output fields remain backward-compatible.

## Throughput Stability Interpretation

### Why avg FPS Alone is Insufficient

Average FPS is a common but misleading performance metric:

- **A benchmark averaging 60 FPS with 2ms avg frame time can still have
  occasional 50ms spikes** (p99 = 50ms, effective 20 FPS) that cause
  visible stutter.
- A benchmark averaging 120 FPS with 8ms avg frame time and 0.1ms jitter
  is noticeably smoother than 60 FPS with 2ms jitter, even at the same
  average throughput.
- The frame time distribution matters more than the average.

### Key Stability Metrics

| Metric | What It Measures | Good Range |
|--------|----------------|------------|
| `avg_fps` | Average frames per second | Machine-dependent |
| `median_fps` | 50th percentile FPS | Close to avg_fps |
| `p95_frame_time` | 95th percentile frame time | < 2x avg frame time |
| `p99_frame_time` | 99th percentile frame time (trimmed) | < 3x avg frame time |
| `frame_time_stability` | Classification of jitter std | excellent/good/moderate/high |
| `frame_jitter` | Frame time standard deviation | < 0.5ms = smooth |

### Stability Classification

The `frame_time_stability` field provides a human-readable classification:

- **excellent**: jitter std < 0.3ms — frame pacing is nearly perfect.
- **good**: jitter std < 0.5ms — imperceptible to most users.
- **moderate**: jitter std < 2.0ms — acceptable but may show occasional micro-stutter.
- **high**: jitter std >= 2.0ms — visible stutter, investigate system load or profiling.

A healthy benchmark should show `frame_time_stability` of "good" or better,
with `p95_frame_time` less than 2x the average frame time and `p99_frame_time`
less than 3x the average. When `avg_fps` is high but `frame_time_stability`
is "moderate" or "high", the user experience will be worse than the FPS
number suggests due to uneven frame pacing.

### Dirty-Cell Ratio

The dirty-cell ratio tracks what percentage of the terminal's cells change
per frame. Lower values indicate more efficient differential rendering:

- `< 5%` — excellent, most frames update only active rain cells.
- `5-30%` — normal for active Monolith scenes.
- `> 50%` — may trigger full-redraw path in the terminal.

### Estimated Full-Redraw Ratio

This is a **threshold estimate**, NOT a literal measurement. It indicates what
percentage of frames have enough dirty cells to potentially trigger the
terminal's full-redraw rendering path (dirty cells >= total cells / 3).

It does NOT mean Cosmostrix performs a full redraw on those frames.
The terminal emulator's rendering pipeline makes its own decisions about
when to batch redraw. This metric is useful for identifying when rendering
load may spike, not for counting exact redraws.

### Throughput Metrics

| Metric | What It Measures |
|--------|-----------------|
| `glyphs_per_second` | Theoretical upper bound based on full-frame cell count and active-frame rate |
| `dirty_glyphs_per_second` | Actual cell updates per second (differential rendering efficiency) |
| `ansi_bytes_per_second` | Estimated ANSI output bandwidth |
| `active_streams_avg` | Average number of active rain streams during measurement |

The `glyphs_per_second` metric is a theoretical upper bound, not actual
rendered output. Compare it against `dirty_glyphs_per_second` to understand
how much differential rendering saves versus full-frame updates.

## Color Stability Policy

### Explicit Color Is Sticky by Default

When a user sets a color via `--color`, `--profile`, `--preset`, `--scene`,
or the config file, that color remains **permanently sticky** for the entire
session unless the user explicitly changes it. This is a UX trust guarantee:
the renderer will never silently change the color you chose.

Autonomous palette drift — where the `ColorEcosystem` spontaneously
transitions to a related color scheme (e.g., Green drifting to Green2 or
Aurora) — is **disabled by default**. The drift code path still exists in
the codebase (reserved for the future atmosphere engine in v4.0.0) but is
gated behind an explicit opt-in flag.

### How Color Can Change at Runtime

Color may only change through these explicit, user-initiated actions:

| Action | Mechanism | Sticky After? |
|--------|-----------|--------------|
| `--color sun` (CLI) | Sets initial scheme | Yes, permanently |
| `--profile nightcore` (CLI) | Profile may set color | Yes, permanently |
| `--preset cinematic` (CLI) | Preset may set color | Yes, permanently |
| `--scene monolith` (CLI) | Scene may set color | Yes, permanently |
| `c` / `C` keypress | Cycles color scheme | Yes, new scheme sticks |
| Numeric hotkeys `0-9`, `!@#$%` | Jumps to specific scheme | Yes, permanently |
| `x` / `X` keypress (scene cycle) | Scene may set color | Yes, if scene specifies one |
| `auto-color-drift = true` (config/CLI) | Enables autonomous drift | Drifts over time (opt-in only) |

### The `auto-color-drift` Flag

A hidden CLI flag and config file key control autonomous palette drift:

```bash
# CLI (hidden flag)
cosmostrix --auto-color-drift --color sun

# Config file
auto-color-drift = true
```

When enabled, the `ColorEcosystem` may spontaneously replace the current
color scheme with an atmospherically related one every ~100 seconds on
average (governed by `AUTONOMOUS_PALETTE_DRIFT_CHANCE = 0.03` per 3-second
tick). The luminance, saturation, and hue climate continue to drift
regardless of this flag — these only modulate rendering intensity, not
the palette identity.

### Color Stability Endurance Tests

Nine deterministic tests verify color stability without wall-clock sleeping:

1. **fixed_color_sun_stays_sun_across_simulated_minutes** — Sun remains Sun
   for 10 simulated minutes (36,000 frames at 60fps).
2. **profile_color_sun_stays_sun_across_simulated_minutes** — Profile-set
   Sun remains sticky across 10 simulated minutes.
3. **default_monolith_color_does_not_drift_without_opt_in** — Green
   Monolith does not drift without opt-in.
4. **auto_color_drift_is_opt_in_only** — With drift OFF, color stays; with
   drift ON, color eventually changes to a related scheme.
5. **pressing_c_changes_color_intentionally** — User key `c` changes color
   and new color sticks.
6. **pressing_shift_c_changes_color_intentionally** — User key `C` changes
   color and new color sticks.
7. **scene_cycle_applies_scene_color_intentionally** — Scene cycling sets
   color (if scene specifies one) and it sticks.
8. **benchmark_output_fields_complete** — Verifies all required benchmark
   metrics are present in the source.
9. **endurance_color_sticky_default_off** — 30 simulated minutes (108,000
   frames) with spot-checks every 1,000 frames.

## Config Color Precedence & Honesty

### Why `color = sun` in Config May Not Win

Cosmostrix uses a 10-level precedence chain (see README). A plain config value like
`color = sun` is resolved at **step 2** (config file values). If the same config file
also contains `preset = cinematic` or `scene = monolith`, those layers (steps 3 and 4)
can override the color because they have higher precedence within the config file. The
color you see in `-i` output reflects the **final resolved** value, not the raw config
line.

This is intentional and documented, but it can surprise users who expect `color = sun`
to be the final word. The important distinction:

- **Precedence override**: `preset`/`scene`/`profile` replace the config color at
  startup. The replacement is immediate and deterministic. `auto_color_drift` remains
  `false`. The replaced color is then sticky for the entire session.
- **Autonomous drift**: `auto_color_drift = true` enables the `ColorEcosystem` to
  spontaneously replace the color scheme over time (every ~100s on average). This is
  a gradual, runtime effect, completely separate from startup precedence.

### How to Guarantee a Final Color

If you want `color = sun` to be the final resolved color, use one of these approaches:

1. **Explicit CLI flag** (highest precedence, step 10):
   ```bash
   cosmostrix --color sun
   ```
   CLI `--color` always wins over config preset, scene, and profile.

2. **Profile with explicit color** (step 5/8):
   ```ini
   profile.nightcore.color = sun
   profile = nightcore
   ```
   Profile color overrides config preset and scene because profiles resolve after
   them in the precedence chain.

3. **Avoid preset/scene that manage color**:
   Remove `preset` and `scene` from the config file (or use preset/scene variants
   that do not set color), so the raw config `color = sun` is never overridden.

### Verifying Drift State at Runtime

The `-i` (info) output includes an `auto_color_drift` field showing `true` or `false`:

```text
RUNTIME PROFILE
  ...
  color: sun
  auto_color_drift: false
```

- `auto_color_drift: false` (default) — autonomous palette drift is disabled.
  The color shown will remain sticky for the entire session.
- `auto_color_drift: true` — autonomous palette drift is enabled (opt-in).
  The `ColorEcosystem` may replace the color scheme periodically.

### Tests That Verify Precedence Is Not Drift

Three config resolution tests document this behavior:

1. **config_color_overridden_by_config_preset_is_precedence_not_drift** — verifies that
   a config `color = sun` overridden by `preset`/`scene` leaves `auto_color_drift`
   at `false`. The color change is from precedence, not autonomous drift.
2. **profile_color_resolves_sun_after_preset_and_scene** — verifies that a profile
   with `color = sun` correctly overrides preset/scene color per the precedence chain.
3. **cli_color_wins_over_config_preset_and_scene** — verifies that `--color sun` on
   the CLI always wins, regardless of config preset or scene.

## Implementation Notes

v3.6.0 does not:
- Rewrite the renderer.
- Add new scenes or color palettes.
- Change profile precedence or CLI behavior.
- Add unsafe code or heavy dependencies.
- Bump the version number.

v3.7.0 does not:
- Retune any visual parameters.
- Change terminal reset/cleanup behavior.
- Change x/X scene cycling semantics.
- Change config precedence semantics.
- Add unsafe code or heavy dependencies.
- Bump the version number.
- Grow src/config_apply.rs beyond its existing LOC budget.

v3.7.0 polish adds:
- `auto_color_drift` visibility in `-i` runtime profile output.
- Config precedence clarity documentation explaining why config `color = sun` may
  be overridden by preset/scene, and how to guarantee a final color.
- Three config resolution tests verifying precedence is distinct from drift.

## v3.9.0 Monolith Subtlety Policy

Ultimate Subtle Monolith Rain uses small bounded motion and depth variation to
make the signature scene feel more organic without changing its identity.
Organic does not mean chaotic: stream motion texture, lane breathing, hero pulse,
and local spine cadence must stay deterministic under seeded RNG and bounded by
Zactrix Core helper tests.

Depth changes must preserve clean empty space. Hero, hot body, mid body, dim
trail, spine ghost, gap, and blank background remain separate visual roles.
Subtle breathing may make active streams feel alive, but it must not create
full-height spine walls, over-bright white spam, muddy grey residue, or bottom
edge buildup.

Zactrix Core may guide these decisions through compact probes, maps, filters,
verifiers, and bounded history, but it remains internal architecture guidance.
It is not Linux eBPF, not a public API, and not the v4.0.0 Full Atmosphere
Engine.

## v4.0.0 Atmosphere Application Policy

The Phase 4 atmosphere application adapter must preserve visual stability:

- Atmosphere modulation is disabled by default (application_mode = disabled).
- Identity modulation produces output identical to v3.9.0.
- Non-Calm applications produce bounded modulation only in internal/test mode.
- Color change is always forbidden in the adapter output.
- Terminal behavior is never affected by atmosphere.
- The adapter does not introduce muddy residue, uncontrolled brightness, or
  visual noise. All values are clamped by the verifier before reaching the
  adapter, and the adapter itself produces only bounded scale factors.
- Clean empty space must remain clean. Hero, hot body, mid body, dim trail,
  spine ghost, gap, and blank background roles are unchanged.

## v4.0.0 ControlledLive Modulation Policy

The Phase 6 ControlledLive mode provides an even more restrictive modulation
path than InternalVerified, designed for internal-only live atmosphere
variation while keeping the v3.9.0 visual identity intact.

- ControlledLiveBounds are tighter than conservative bounds: speed ±4%,
  density ±4%, brightness ±3%, glitch_pressure ≤ 0.2.
- ControlledLive is NOT exposed via public CLI. Only reachable through
  internal/test code paths.
- Calm regime always produces identity regardless of application mode.
- ControlledLive modulation is always more restrictive than InternalVerified
  modulation for the same application — deviation from identity is strictly
  smaller in all parameters.
- The effective runtime values under ControlledLive are extremely close to
  base values: speed and density deviate by at most ±4% from config base.
- Color change and terminal effects remain permanently false in ControlledLive.

## v4.0.0 Visual Whisper Policy

The Phase 7 visual whisper adapter provides the most restrictive modulation
layer in the atmosphere pipeline. It converts verified atmosphere modulation
into ultra-subtle visual-safe whisper values that are strictly tighter than
ControlledLive bounds in every parameter.

- VisualWhisperBounds are tighter than ControlledLiveBounds: speed ±2%,
  density ±2%, brightness ±1.5%, trail_energy ±2%, glyph_pulse ±2%,
  glitch_pressure ≤ 0.05.
- The visual whisper adapter is internal/test-only. Non-identity whisper is
  never produced in the default production runtime path.
- Disabled mode always produces identity whisper.
- Calm regime always produces identity whisper.
- No color changes. No terminal effects. No persistent config mutation.
- Clean empty space must remain clean: no muddy residue, no white spam,
  no full-height spine wall, no bottom buildup.
- The whisper adapter is a pure read-only transform — it never mutates
  persistent configuration state or terminal state.
- Visual whisper is deterministic: same input always produces same output.
