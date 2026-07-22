# Cosmostrix Render Engine — Formal Architecture

<!-- SPDX-License-Identifier: GPL-3.0-only -->

This document specifies the cosmostrix diff-based terminal rendering
engine: its data structures, algorithms, complexity class, worst-case
behavior, and design rationale. It is intended as a reference for
contributors, downstream TUI authors who want to cite or adapt the
design, and reviewers evaluating cosmostrix against alternative
rendering strategies.

> **Scope**: interactive rendering path (`src/terminal.rs` `draw()`,
> `src/frame.rs`, `src/cloud/rain.rs`). Benchmark mode (no terminal
> writes) is out of scope.

---

## 1. Problem Statement

Terminal emulators are byte-stream interpreters. Every visual change
requires sending ANSI escape sequences over a PTY. For a Matrix rain
renderer running at 60 FPS on an 80×24 terminal (1,920 cells), a naive
"clear-and-redraw-everything" strategy emits roughly:

- 1,920 × (SGR fg + SGR bg + char) ≈ 1,920 × 25 bytes ≈ **48 KiB/frame**
- At 60 FPS: **2.8 MiB/s** of ANSI bytes through the PTY

Modern terminal emulators (Alacritty, kitty, wezterm) can sustain this,
but older emulators (gnome-terminal, xterm, Termux) throttle at
~200–500 KiB/s, capping effective FPS at 5–15. The CPU cost of parsing
SGR sequences in the emulator also dominates battery on laptops.

**Goal**: minimize bytes-per-frame sent to the terminal while
preserving visual fidelity (color, afterglow, depth-of-field, overlay),
so cosmostrix can hit 60 FPS on commodity terminals.

---

## 2. Strategy: Differential Rendering with Run-Length Encoding

Cosmostrix tracks the **last frame sent to the terminal** and, on each
`draw()` call, emits only the cells that differ from the last frame.
Within the diff, consecutive cells sharing the same `(fg, bg, bold)`
style tuple are batched into a single SGR + raw-character run.

### 2.1 Data structures

```text
Frame (src/frame.rs)
├── cells: Vec<Cell>              // current frame content (24 B/cell)
├── cell_gen: Vec<u32>            // generation stamp per cell
├── gen: u32                      // current generation counter
├── semantic_gen: u32             // bumped on charset/theme/shading change
├── dirty_map: BitVec             // 1 bit per cell, O(1) mark/check
├── dirty: Vec<usize>             // queue of dirty cell indices
└── dirty_all: bool               // fast-path flag for full redraw

Terminal (src/terminal.rs)
├── last: Option<LastFrame>       // snapshot of last sent frame
│   ├── cells: Vec<Cell>          // 24 B/cell, mirrors terminal state
│   ├── semantic_gen: u32         // for invalidation detection
│   └── width/height: u16         // for resize detection
├── ansi_buf: Vec<u8>             // 64 KiB cap, single write_all per frame
├── dirty_flat: Vec<usize>        // reusable sort buffer
├── row_buf: String               // reusable RLE accumulator (full redraw)
├── run_buf: String               // reusable RLE accumulator (diff redraw)
└── color_cache: Option<ColorCache>  // pre-computed SGR bytes per (fg,bg)

Cell (src/cell.rs) — 24 bytes
├── ch: char          (4 B)
├── fg: Option<Color> (5 B, discriminant + Rgb payload)
├── bg: Option<Color> (5 B)
├── bold: bool        (1 B)
└── padding           (9 B, alignment)
```

### 2.2 Cell equality — the fast path

```rust
// src/frame.rs
pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
    let cur = /* cell from previous frame, or blank */;
    if cur == cell {
        return;  // ← skip: cell unchanged, no dirty mark
    }
    self.cells[i] = cell;
    self.cell_gen[i] = self.gen;
    self.dirty_map.set(i, true);
    self.dirty.push(i);
}
```

`Cell` derives `PartialEq`, so the comparison is a 24-byte field-wise
compare. The compiler emits a branch-predictable scalar compare; on
x86-64 this is ~4 cycles per cell. For a 1,920-cell frame, the worst
case is ~7,680 cycles (~2.5 µs at 3 GHz), negligible against the
~16 ms frame budget at 60 FPS.

**Why not SIMD?** The 24-byte Cell layout doesn't fit cleanly into a
single SSE/AVX lane. Compacting to 16 bytes (pack `Color` to `u32`
with a high-bit "is_set" flag) would enable `__m128i` compare, but
the early-exit on the `ch` field (first 4 bytes) means most
comparisons return on byte 1 anyway. The realistic gain is <10% on
the hot path, not worth the loss of `Option<Color>` ergonomics.

### 2.3 Dirty tracking — O(1) mark, O(dirty) flush

Each `frame.set()` that actually changes a cell:
1. Sets the bit in `dirty_map` (BitVec, 1 bit/cell — 240 B for 1,920 cells)
2. Pushes the cell index onto `dirty: Vec<usize>`

The renderer does **not** scan all cells on `draw()`. It iterates
`dirty` directly. Worst case `dirty.len() == cells.len()` triggers a
full-redraw fast path (`dirty_all` flag).

```rust
// src/terminal.rs draw()
let dirty_count = frame.dirty_indices().len();
let dirty_is_large = dirty_count >= total_cells / DIRTY_THRESHOLD_RATIO;
let do_full_redraw = !can_reuse_last || frame.is_dirty_all() || dirty_is_large;
```

`DIRTY_THRESHOLD_RATIO` (in `constants.rs`) is set so that once >X% of
cells are dirty, the per-cell overhead of the diff path exceeds the
amortized cost of a full redraw. The threshold was tuned empirically.

### 2.4 Run-Length Encoding on the diff path

```text
For each dirty cell (sorted row-major):
  1. Compare against last.cells[idx] — skip if equal (already sent)
  2. Start a run: record (fg0, bg0, bold0)
  3. Scan forward while:
     - next dirty index == prev + 1 (contiguous)
     - same row (no wrap)
     - same (fg, bg, bold)
  4. Emit:
     - MoveTo(x0, y0)  [only if cursor not already there]
     - SGR(fg0, bg0)   [only if (fg0,bg0) != current SGR state]
     - bold toggle     [only if bold0 != current bold state]
     - raw chars       [run_buf as UTF-8 bytes]
  5. Update cur_fg, cur_bg, cur_bold, cur_pos
```

**Critical optimization**: SGR state (`cur_fg`, `cur_bg`, `cur_bold`)
is tracked across runs within a single `draw()` call. If run N+1 has
the same colors as run N, no SGR escape is emitted — only the raw
characters. This is where the bandwidth win comes from: a column of
50 green-on-black characters that are all dirty emits:

- Without state tracking: 50 × `\x1b[38;2;0;255;0m\x1b[48;2;0;0;0m<ch>` ≈ 1,250 bytes
- With state tracking: 1 × SGR + 50 × `<ch>` ≈ 60 bytes (20× reduction)

### 2.5 Color cache

`ColorCache` (built once after palette initialization) maps
`(Option<Color>, Option<Color>) → &[u8]` SGR byte sequences. The
diff path's `emit_sgr()` checks the cache first; only cache misses
fall through to `write_sgr_colors_buf()` (which formats the SGR
on-the-fly via `push_u8` — no heap allocation).

For the 43 built-in palettes, the cache hit rate varies significantly
by scene and visual effects enabled. Measured on AMD Ryzen 7 5800HS
with `--perf-stats` (v13.3.0+):

| Configuration | Hit Rate | Notes |
|---------------|---------:|-------|
| Monolith scene, Cosmos palette, Subtle glitch (3%) | 18.1% | DoF + phosphor generate many intermediate shades |
| Matrix scene, Cosmos palette, Default glitch (10%) | 38.2% | More palette-color cells, but glitch still misses |

The Monolith scene's lower hit rate is **expected, not a bug**: depth-of-field
blends layer-0 colors 35% toward black, phosphor afterglow generates
intermediate shades between palette entries, and glitch generates random
colors. All of these produce colors that aren't palette entries, so they
miss the cache and fall through to `write_sgr_colors_buf()` (which is
still allocation-free — just slower than a cache hit).

The Matrix scene hits higher (38.2%) because it uses more palette-color
cells directly (classic glyph rain), but still misses on glitch (10%)
and bold/shading variations.

The cache remains valuable because the **palette-color cells** (rain
heads, bright trail cells) still hit, and those are the most frequently
re-rendered cells. The miss path is optimized: `write_sgr_colors_buf`
uses `push_u8` (no heap alloc) and writes directly into `ansi_buf`.

### 2.6 Semantic generation counter

Some changes invalidate the **meaning** of a cell, not just its
content. For example, switching charset from `binary` to `katakana`
means a cell containing `'1'` may now need to display a different
character even if `'1'` was the "same" logical position. The
`semantic_gen` counter handles this:

```rust
// src/frame.rs
pub fn invalidate_semantic(&mut self, bg: Option<Color>) {
    self.semantic_gen = self.semantic_gen.wrapping_add(1);
    self.clear_with_bg(bg);  // bump gen, set dirty_all
}
```

On `draw()`, if `last.semantic_gen != frame.semantic_gen`, a full
redraw is forced. This prevents stale glyphs from lingering after a
charset or theme switch.

### 2.7 Force-draw escape hatch

Some operations require unconditional full redraw even when no
semantic change occurred: window resize, focus regain, HUD toggle-off.
The `cloud.force_draw_everything()` method sets a flag that, on the
next `rain_at()` call, triggers `frame.clear_with_bg()` — bumping the
generation counter and setting `dirty_all = true`.

This is the escape hatch that prevents "stale cell residue" bugs
(e.g., HUD text remaining visible after toggle-off in regions the
rain didn't write this frame).

---

## 3. Complexity Analysis

| Operation | Time | Space |
|-----------|------|-------|
| `frame.set(x, y, cell)` — no change | O(1) | O(1) |
| `frame.set(x, y, cell)` — changed | O(1) | O(1) amortized |
| `frame.clear_with_bg()` | O(W×H) | O(1) |
| `frame.invalidate_semantic()` | O(W×H) | O(1) |
| `terminal.draw()` — diff path | O(D log D) + O(D) | O(D) |
| `terminal.draw()` — full redraw | O(W×H) | O(1) |

Where:
- W, H = terminal dimensions
- D = number of dirty cells (D ≤ W×H)

The O(D log D) on the diff path comes from sorting `dirty_flat`. In
practice, dirty cells are usually already near-sorted (rain advances
top-to-bottom, left-to-right), so the sort is close to O(D).

**Worst case**: theme switch triggers `invalidate_semantic()` →
`clear_with_bg()` → `dirty_all = true` → full redraw O(W×H). At
200×60 = 12,000 cells, this is ~12,000 cell copies + ~12,000 SGR
emissions (with RLE, fewer). Measured at ~8 ms on a 3 GHz CPU, well
within the 16 ms frame budget.

---

## 4. Output Encoding Details

### 4.1 ANSI byte budget per cell

| Scenario | Bytes per cell |
|----------|----------------|
| Unchanged (skipped) | 0 |
| Changed, same style as previous run | 1–4 (UTF-8 char only) |
| Changed, new style, cache hit | ~20 (SGR + char) |
| Changed, new style, cache miss | ~25 (SGR + char) |
| Full redraw, RLE batched (N cells same style) | ~20 + N (amortized ~1/cell) |

### 4.2 SGR emission format

Cosmostrix emits combined fg+bg SGR in a single escape:

```text
\x1b[38;2;R;G;B;48;2;R;G;Bm
```

This is 19 bytes for true-color fg+bg. The `ColorCache` pre-formats
these for all palette color pairs, so the hot path is a hashmap lookup
+ `extend_from_slice`.

For ANSI-256 colors (when true-color isn't supported), the format is:

```text
\x1b[38;5;V;48;5;Vm
```

For reset/default colors: `\x1b[39;49m` (6 bytes).

### 4.3 Cursor movement

`MoveTo(x, y)` is emitted as `\x1b[Y+1;X+1H` (variable length, 6–10
bytes). The diff path tracks `cur_pos` and skips the MoveTo if the
cursor is already at the target position (e.g., consecutive runs on
the same row). This saves ~8 bytes per skipped MoveTo.

### 4.4 Synchronized output

On terminals that support `ESC[?2026h` (kitty, wezterm, foot), each
frame is wrapped in sync markers:

```text
\x1b[?2026h<frame bytes>\x1b[?2026l
```

This tells the terminal to buffer the frame and present it atomically,
eliminating visible tearing. The overhead is 12 bytes/frame, negligible.

---

## 5. Why Not Alternatives?

### 5.1 Full redraw (cmatrix, unimatrix)

Clear screen + emit every cell every frame.

- **Pro**: dead simple, no state, no bug class from stale tracking
- **Con**: 5–20× more bandwidth; caps FPS at ~15 on gnome-terminal
- **When it wins**: very short runs (<5 seconds) where the steady-state
  bandwidth savings don't amortize the state-tracking overhead

Cosmostrix chose diff-based because the target use case is long-running
(24/7 screensaver, multi-hour hacking session). The bandwidth savings
compound over time.

### 5.2 Per-droplet cursor targeting

Track each droplet's head + tail position; emit only 2–3 cursor moves
+ character writes per droplet per frame.

- **Pro**: minimal bandwidth (~3 cells/droplet, no equality check)
- **Con**: cannot represent background haze, phosphor afterglow, or
  depth-of-field — these require writing cells that aren't droplet
  heads/tails
- **When it wins**: classic matrix rain with no visual effects

Cosmostrix's signature Monolith Rain + depth-of-field + phosphor
afterglow all require writing non-droplet cells, which per-droplet
targeting cannot express.

### 5.3 ANSI scroll regions

Set per-column scroll regions; let the terminal emulator handle motion.

- **Pro**: zero CPU cost for motion (offloaded to terminal)
- **Con**: only uniform vertical motion; no glitch, no color cycle,
  no horizontal motion, no overlay
- **When it wins**: 1990s BBS-era matrix rain on serial terminals

Modern terminals still support scroll regions, but the visual
limitations make it unsuitable for cosmostrix's feature set.

### 5.4 Sixel / kitty graphics protocol

Render rain as a bitmap; send as an image.

- **Pro**: pixel-perfect, true Gaussian blur for depth-of-field
- **Con**: terminal support is spotty (kitty/wezterm yes,
  gnome-terminal/xterm/Termux no); feels like an image, not terminal
  text; loses the "character grid" aesthetic
- **When it wins**: art installations, not CLI tools

Cosmostrix targets universal terminal support, which rules out
graphics protocols.

### 5.5 PTY multiplexer (tmux-style)

Spawn N threads, each renders one column, mux output.

- **Pro**: parallelism
- **Con**: ANSI escape interleaving corrupts; no shared palette state;
  synchronization nightmare; the bottleneck is PTY bandwidth (single
  writer), not CPU, so parallelism doesn't help
- **When it wins**: never, for terminal rendering

---

## 6. Measured Performance

Internal benchmark (`cosmostrix --benchmark --json`) and interactive
`--perf-stats` on AMD Ryzen 7 5800HS, Linux 6.18, 120×40 terminal:

### Headless Engine Ceiling (`--benchmark --json`)

| Metric | Value |
|--------|------:|
| Avg FPS (no terminal I/O) | 28,029 |
| Peak FPS | 39,971 |
| Total frames in 10s | 280,293 |
| p99 frame time (ms) | 0.043 |
| Peak RSS | 4.4 MiB |

### Interactive Encoding by Scene (`--perf-stats`, 60 FPS)

| Metric | Monolith | Matrix | Notes |
|--------|---------:|-------:|-------|
| Avg dirty cells/frame | 332.8 | 1,103.5 | Matrix is 3.3× denser |
| **Avg ANSI bytes/frame** | **7,134.8** | **31,537.9** | Matrix is 4.4× more bandwidth |
| Naive full-redraw equivalent | ~48 KB | ~48 KB | Same terminal size |
| **RLE compression vs naive** | **6.7×** | **1.5×** | Monolith benefits more from RLE |
| Bandwidth to terminal | 418.1 KiB/s | 1,847.4 KiB/s | Matrix nears gnome-terminal limit |
| SGR cache hit rate | 18.1% | 38.2% | Matrix uses more palette colors |
| Avg frame time | 0.109 ms | 0.450 ms | Both well under 16.67ms budget |
| Max frame time | 0.303 ms | 2.474 ms | No visible jank |
| Endurance health | 89.8/100 | 57.8/100 | Matrix triggers "investigate" |

**Key takeaways**:

- **Engine ceiling 28K FPS = 467× headroom** over 60 FPS target. Visual
  effects consume <0.25% of frame budget.
- **Monolith is the efficiency champion**: 6.7× RLE compression, 418 KiB/s
  bandwidth. Sparse structured rain means most cells are stable per frame,
  so diff-based + RLE shines.
- **Matrix is bandwidth-heavy**: 1.8 MiB/s (4.4× Monolith). Dense classic
  rain means more dirty cells per frame. Still under Alacritty/kitty
  capacity (~10 MiB/s), but approaches gnome-terminal's limit (~2 MiB/s).
- **Both scenes stay under 16ms** frame budget — no visible jank even
  at peak (max 2.47ms << 16.67ms).
- **SGR cache hit rate is scene-dependent**: 18–38% measured. Not the
  ~95% originally estimated, but the miss path is allocation-free so
  the cost is acceptable. See §2.5 for analysis.

For competitor comparison data (cosmostrix vs cmatrix vs unimatrix),
see `scripts/bench-compare.sh` and the results table in
`benchmark/README.md`.

---

## 7. Failure Modes and Defenses

### 7.1 Stale cell residue

**Symptom**: overlay text or old glyphs remain visible after the
overlay is removed or the scene changes.

**Cause**: diff-based rendering only refreshes cells the rain actively
writes. Cells in "dead zones" (no active droplet this frame) keep
their previous content.

**Defense**: `force_draw_everything()` escape hatch. Called on HUD
toggle-off, focus regain, resize, paste events. Triggers
`frame.clear_with_bg()` which sets `dirty_all = true`, forcing every
cell to be re-sent.

### 7.2 Ghost glyph flood

**Symptom**: background fills with random characters after a full
redraw.

**Cause**: `phosphor_base_ch[]` (the afterglow character buffer) is
separate from `frame.cells[]`. A full redraw that clears `cells` but
not `phosphor_base_ch` exposes stale afterglow characters as visible
background.

**Defense**: `force_draw_everything()` in `cloud/rain.rs` explicitly
clears `phosphor_base_ch` (or calls `reset_phosphor_state()` for
Monolith scene) before the redraw pass.

### 7.3 Semantic invalidation lag

**Symptom**: after a charset switch, some cells still show the old
charset's characters.

**Cause**: diff-based rendering compares cell content, not cell
"meaning". A cell containing `'1'` in the binary charset and `'1'`
in the hex charset would be considered "unchanged" even though the
intended display differs.

**Defense**: `semantic_gen` counter. Charset/theme/shading changes
bump the counter, which forces a full redraw via `invalidate_semantic()`.

### 7.4 Resize race

**Symptom**: garbage characters at terminal edges after a resize.

**Cause**: crossterm reports the new size, but the terminal emulator
may not have finished resizing its internal buffer. Raw values can be
degenerate (0×0 or 65535×65535) during the transition.

**Defense**: resize values are clamped to `MIN_TERMINAL_COLS..
MAX_TERMINAL_COLS` and `MIN_TERMINAL_LINES..MAX_TERMINAL_LINES` before
use. A debounce window (`RESIZE_DEBOUNCE_MS`) coalesces rapid resize
events (e.g. window drag) into a single `cloud.reset()`.

---

## 8. Future Work

Identified but not yet implemented:

1. **Per-row hash fast path**: 64-bit hash per row, updated
   incrementally. Skip entire row on hash match. Estimated 10–20%
   CPU reduction in idle mode, low impact in active mode.

2. **Cell compaction to 16 bytes**: pack `Color` to `u32` with
   high-bit "is_set" flag. Enables SIMD compare (`__m128i`).
   Estimated 2–3× faster `frame.set()` hot path, but early-exit on
   `ch` field limits real-world gain to <10%.

3. **SGR cache hit-rate instrumentation** — **DONE in v13.3.0**.
   The `ColorCache` now tracks atomic hit/miss counters, exposed via
   `Terminal::encoding_stats()` and the `--perf-stats` exit report's
   ENCODING section. The `Terminal` also tracks total ANSI bytes flushed
   and frame count, so the report shows actual `avg_bytes_per_frame`
   and `bandwidth (KiB/s)` instead of the previous estimate.

4. **Adaptive dirty threshold**: dynamically tune
   `DIRTY_THRESHOLD_RATIO` based on observed full-redraw frequency
   and dirty cell distribution. Currently static.

These are listed for transparency; no commitment to implement.

---

## 9. Citation

If you adapt or reference the cosmostrix render engine design in
academic work or another project, please cite:

```bibtex
@software{cosmostrix,
  author       = {rezky\_nightky (oxyzenQ)},
  title        = {Cosmostrix: Professional-grade cinematic Matrix rain renderer},
  year         = {2026},
  url          = {https://github.com/oxyzenQ/cosmostrix},
  note         = {Diff-based terminal rendering engine, v14.x},
}
```

For the specific design rationale in this document, link to
`docs/RENDER_ENGINE.md` at the specific commit you reference.
