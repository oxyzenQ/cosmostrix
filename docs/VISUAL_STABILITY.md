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

## Implementation Notes

v3.6.0 does not:
- Rewrite the renderer.
- Add new scenes or color palettes.
- Change profile precedence or CLI behavior.
- Add unsafe code or heavy dependencies.
- Bump the version number.
