# Charset "minimal" Masterclass Replacement — Research (Z-master-1B)

<!-- SPDX-License-Identifier: GPL-3.0-only -->
<!-- Copyright (C) 2026 rezky_nightky (oxyzenQ) -->

Status: **RESEARCH ONLY — owner decides, no code changed.** The preset name
stays `minimal`; only the glyph set it maps to is up for replacement.

Owner mandate: the current `minimal` charset is "bad, not masterclass" —
propose alternatives worthy of the preset slot.

## 1. Why the current set fails the masterclass bar

Current definition (`src/scene/charset.rs:221`):

```
.:-=+*·•○●◦◌◍◉◎◇◆□■        (17 glyphs)
```

Five concrete defects:

1. **Six unrelated families in one pool.** ASCII operators (`.:-=+*`),
   typographic dots (`·•`), circles at three fills (`○●◦`), exotic circles
   (`◌◍◉◎`), diamonds (`◇◆`), squares (`□■`). "Minimal" should be ONE
   family done perfectly; this is a junk drawer.
2. **No readable progression.** A masterclass minimal set is an ordered
   ramp (faint → dense). The current set contains four *separate*
   two-step ramps (`·→•`, `○→●`, `◇→◆`, `□→■`) shuffled together by the
   renderer — the eye reads noise, not structure.
3. **Uncontrolled ink density.** `■`/`●` are near-solid ink, `.`/`:` are
   hairlines. Random mixing produces flickering density spikes — the
   opposite of calm minimalism.
4. **Weakest glyph-coverage picks in the pool.** `◌` (U+25CC DOTTED
   CIRCLE — a combining-mark rendering aid, visually noisy at 1 cell),
   `◍` (U+25CD), `◉` (U+25C9), `◎` (U+25CE) are the least-supported
   Geometric Shapes in monospace fonts; on fallback fonts they render
   with inconsistent optical size next to the ASCII glyphs.
5. **Operators dilute the concept.** `=+*` are syntax from other
   charsets, not minimal shapes — they make the pool read like a broken
   `hacker` preset.

## 2. Design criteria for the replacement

- **One family** — every glyph belongs to a single visual idea.
- **An ordered ramp** — even shuffled by the renderer, the pool reads as
  a gradient (faint → dense), so trails gain a second depth dimension on
  top of the OKLab color gradient.
- **Bulletproof coverage** — glyphs that every terminal font ships:
  prefer ASCII, Block Elements (already proven in-tree by the `blocks`
  preset, 0x2580–0x259F), and the core Geometric Shapes (U+25A0–U+25CF
  basics).
- **Single-width guaranteed** — every candidate is East-Asian-Width
  Narrow/Ambiguous (width 1 under the project's `unicode-width` default
  config), and the build-time `width() == Some(1)` filter guards the
  pool regardless.
- **5–6 glyphs** — small enough to feel curated, large enough for
  visible variation along a falling trail. (zen = 1, binary = 2, minimal
  should sit just above.)

## 3. Options

### Option A — "Ink Ramp" (circles, faint → dense)  **[recommended]**

```
· • ○ ◎ ●
```

One shape — the circle — in five ink states: hairline dot, small bullet,
hollow ring, bullseye, solid disc. The set IS a gradient; the renderer's
random sampling turns every trail into a boiling, depth-shifting column
of circles — organic, dreamlike, unmistakably intentional. Multiplying
the ink ramp with the tail→head color gradient gives trails TWO depth
dimensions (ink + color) from a five-glyph pool — maximal result from
minimal means, which is the definition of the masterclass move.

- Coverage: `·` U+00B7, `•` U+2022, `○` U+25CB, `◎` U+25CE, `●` U+25CF —
  core Latin-1 + Geometric Shapes basics, present in every mainstream
  monospace font (DejaVu, Cascadia, JetBrains Mono, Fira Code, Menlo,
  Consolas, Noto Sans Mono).
- Risk: `◎` is the least-common of the five; if the owner wants maximum
  safety the set still works as `·•○●` (four glyphs).
- Rain character: "bubbling ink".

### Option B — "Shade Ramp" (Block Elements)

```
░ ▒ ▓ █
```

The canonical terminal density ladder — the same glyphs every masterclass
TUI (htop, lazygit, ncmpcpp) uses for graded fill. Pure ink language, no
shapes at all; combined with the color gradient it reads as anti-aliased
dissolving rain. This is the most "boring but strong" option: Block
Elements are the single most portable glyph range in terminals (progress
 bars everywhere rely on them), and the range is already proven in-tree
by the `blocks` preset.

- Coverage: bulletproof — U+2591–U+2593 + U+2588.
- Risk: visually heavier than A (full blocks carry a lot of ink); at high
  density the screen darkens. Density tuning mitigates.
- Rain character: "dissolving sand / LED decay".

### Option C — "Bit Pairs" (hollow/solid geometry)

```
○ ● ◇ ◆ □ ■
```

Three shapes, each in exactly two states — hollow and solid. The pool is
a statement: everything in the rain is a bit that flips. Falling columns
toggle between outline and fill, which reads as digital animation. Six
glyphs = the largest pool of the four options.

- Coverage: U+25CB/U+25CF, U+25C7/U+25C6, U+25A1/U+25A0 — all core
  Geometric Shapes, excellent coverage.
- Risk: three families again (circle/diamond/square) — though paired
  states give it a logic the current set lacks; heavier ink than A.
- Rain character: "flipping bits".

### Option D — "ASCII Signal" (zero-unicode purism)

```
. : - =
```

Pure ASCII: the classic signal-strength ramp. Zero unicode risk — works
in a Linux text console, PuTTY with any font, serial terminals. Reads as
Morse / fading signal, pairing naturally with the `binary` preset
philosophy. The most minimal of all options by glyph weight.

- Coverage: absolute — 7-bit ASCII.
- Risk: visually the plainest; `=` and `-` can read as damage/glitch
  rather than intent on sparse trails.
- Rain character: "fading Morse".

## 4. Side-by-side rain mock

```
Option A        Option B        Option C        Option D
  ·               ░               ○               .
  •               ▒               ●               :
  ○               ░               ◇               -
  ◎               ▓               ◆               =
  ●               █               □               .
  ○               ▒               ■               :
  •               ░               ●               =
```

## 5. Decision matrix

| Criterion | A Ink Ramp | B Shade Ramp | C Bit Pairs | D ASCII Signal |
|-----------|-----------|--------------|-------------|----------------|
| One-family coherence | 5/5 | 5/5 | 3/5 | 5/5 |
| Ordered progression | 5/5 | 5/5 | 3/5 | 4/5 |
| Font coverage | 4.5/5 | 5/5 | 5/5 | 5/5 |
| Ink control (calm, not noisy) | 5/5 | 3/5 | 3/5 | 4/5 |
| Interplay with color gradient | 5/5 | 4/5 | 3/5 | 3/5 |
| Distinctive / memorable | 5/5 | 4/5 | 4/5 | 3/5 |
| **Total** | **29.5** | **26** | **21** | **24** |

**Recommendation: Option A (`·•○◎●`)** — one family, one gradient, two
depth dimensions, and the most distinctive silhouette of the four.
**Runner-up: Option B** — choose it if bulletproof coverage outranks
elegance (e.g. if reports arrive of `◎` misrendering on niche fonts).

## 6. Preview before deciding (no build needed)

Each option can be previewed RIGHT NOW against the live rain using the
charset-custom killer feature — a custom block with the same name
shadows the builtin preset (custom wins, with the Option-D collision
notice; see docs/RULES.md):

```toml
# ~/.config/cosmostrix/config.toml
[charset-custom.minimal]
set = "·•○◎●"        # or "░▒▓█", "○●◇◆□■", ".:-="
```

then run `cosmostrix --charset minimal` (or edit `charset = "minimal"`).
Compare the four candidates live before committing one to the preset.

## 7. Implementation touch points once the owner decides (for the follow-up task)

1. `src/scene/charset.rs:219-225` — the `Charset::MINIMAL` arm's glyph
   string (the only behavioral change).
2. `src/config/list_printers.rs` — the `--list-charsets` description
   line for `minimal`.
3. Tests: add a `build_chars_minimal_*` lockstep test (mirrors the zen /
   binary tests in charset.rs) asserting the exact new pool.
4. Docs: README charset table (if it enumerates glyphs), this document
   gets a "decision recorded" note, CHANGELOG entry.
5. No API/flag/format change — the preset name, `--charset minimal`,
   `[charset-custom.minimal]` shadowing, and live reload all keep
   working unchanged.

<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (src/**/*.rs) is the single source of truth.
  Always cross-check against the actual .rs files before relying
  on any specific number (test count, LOC, FPS, ms timeout), file path,
  function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
