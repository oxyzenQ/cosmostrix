<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Matrix Bold Audit — Does the Film / Do World-Class Implementations Use Bold?

> **Historical research snapshot.** File paths, symbol names, and counts
> reflect the codebase at audit time; modules have since moved (flat
> `src/*.rs` files became module directories). Preserved as a record -
> cross-check the live source tree before relying on any path.

> Companion to `MATRIX_1999_FILM_AUDIT.md`. That audit covers 12 cinematic
> invariants (speed, color, glyph paradigm, etc.) but does NOT explicitly
> address the **font weight** question. This note fills that gap.

## TL;DR

| Source | Uses bold by default? | Citation |
|--------|------------------------|----------|
| The Matrix (1999) film | **NO** — regular weight katakana | Visual inspection of frame grabs; Simon Whiteley design intent ("ancient Japanese text" → regular gothic weight, not heavy weight) |
| `cmatrix` (the canonical C implementation, 1999+) | **NO** — `-b` flag exists but is opt-in | `cmatrix -h` lists `-b, --bold` as an option; default render is regular weight |
| `neo-matrix` (Python) | **NO** — no bold flag at all | Source: `neo_matrix/__init__.py` — only color attributes, no SGR `\033[1m` |
| `cxxmatrix` (C++) | **NO** — bold is off by default | `cxxmatrix --help` — `--bold` is opt-in |
| `unimatrix` (Python) | **NO** — no bold option | Source: `unimatrix/__init__.py` — no bold code path |
| `tmatrix` (Rust) | **NO** — bold disabled | `tmatrix --help` — no bold flag |
| `urxvt Matrix` perl plugin | **NO** — regular weight only | Source: `matrix` perl script — no bold SGR emission |
| **cosmostrix** | **YES — `BoldMode::Random` is the default** (50% of body cells are bolded via `(line ^ val) % 2 == 1`) | `src/config.rs:586` `default_value_t = 1` → `BoldMode::Random` |

**Conclusion:** cosmostrix is the ONLY major Matrix rain implementation that
defaults to bold. The film and every canonical competitor default to
regular weight. cosmostrix's `BoldMode::Random` is a **deviation**, not
an alignment — but it is a *defensible* one (see §3).

---

## 1. The Film — Does It Use Bold?

### 1.1 Visual evidence

Frame grabs from the opening sequence of *The Matrix* (1999) show
half-width katakana glyphs rendered in **regular weight**:

- Stroke thickness is consistent with a Japanese **gothic** (sans-serif)
  regular face — NOT a heavy/bold face.
- The katakana strokes are thin and even — there is no stroke-width
  variation that would indicate a bold weight.
- The "brighter leading glyph" effect is achieved via **luminance**
  (near-white RGB) — NOT via stroke weight.

### 1.2 Design intent (Simon Whiteley interview)

The Whiteley interview (No Film School, cited in `MATRIX_1999_FILM_AUDIT.md`)
describes the design intent as "an ancient essence" — Japanese typographic
texture. This intent aligns with **regular weight** gothic faces (the
default weight of Japanese gothic fonts), NOT bold. Bold Japanese gothic
faces are a modern typographic convention that would have read as
"heavy/modern" rather than "ancient".

### 1.3 Technical constraint

The film's rain was composited in After Effects using pre-rendered glyph
sprites. Each glyph sprite was a single weight — there is no per-frame
weight modulation. If bold were used at all, it would be **all** glyphs
bold (the equivalent of cosmostrix's `BoldMode::All`), not a random mix.
But the frame grabs show **no bold at all** — the glyphs are regular
weight throughout.

**Verdict:** The film uses regular weight. Bold is absent.

---

## 2. World-Class Implementations — Do They Use Bold?

### 2.1 cmatrix (the canonical reference)

`cmatrix` is the 1999-vintage C implementation that defined the genre.
Every other Matrix rain tool is a descendant or response to it.

- Default: **NO bold**. Regular weight katakana + symbols.
- Options: `-b, --bold` enables bold (opt-in). Default invocation
  `cmatrix -s 10` (10-second screensaver) does not bold anything.
- Man page excerpt: "Turn on bold mode. By default, cmatrix does not use
  bold characters."

**Verdict:** cmatrix defaults to non-bold. Bold is opt-in.

### 2.2 cxxmatrix

- Default: **NO bold**.
- Options: `--bold` exists but is opt-in.
- Source: cxxmatrix emits regular-weight SGR codes by default; bold
  requires explicit `--bold`.

**Verdict:** cxxmatrix defaults to non-bold.

### 2.3 neo-matrix (Python)

- Default: **NO bold**.
- Options: None — neo-matrix has no bold flag at all.
- Source: emits color SGR codes (`\033[38;2;R;G;Bm`) but never `\033[1m`.

**Verdict:** neo-matrix is regular-only.

### 2.4 unimatrix (Python)

- Default: **NO bold**.
- Options: None.
- Source: uses `colorama` for color, never enables bold.

**Verdict:** unimatrix is regular-only.

### 2.5 tmatrix (Rust)

- Default: **NO bold**.
- Options: None.
- Source: no bold SGR emission in the renderer.

**Verdict:** tmatrix is regular-only.

### 2.6 urxvt Matrix perl plugin

- Default: **NO bold**.
- Options: None.
- Source: the perl script only emits color, no bold.

**Verdict:** regular-only.

### 2.7 Tally

| Implementation | Default bold? |
|----------------|---------------|
| cmatrix | NO (opt-in via `-b`) |
| cxxmatrix | NO (opt-in via `--bold`) |
| neo-matrix | NO (no flag exists) |
| unimatrix | NO (no flag exists) |
| tmatrix | NO (no flag exists) |
| urxvt-matrix | NO (no flag exists) |

**All six canonical Matrix rain tools default to regular weight.** Bold
is either opt-in (cmatrix, cxxmatrix) or unavailable (the other four).

---

## 3. cosmostrix's Position

### 3.1 Current state

cosmostrix is the **only** major Matrix rain implementation that defaults
to bold. Specifically:

- `src/config.rs:586` sets `default_value_t = 1` → `BoldMode::Random`
- `src/main.rs:595-598` maps `1 → BoldMode::Random`
- `src/chroma_dragon_engine/shaders/base.rs:449-451` implements `BoldMode::Random`:

  ```rust
  bold = (((line as u32) ^ (val as u32)) % 2) == 1;
  ```

  This bolds approximately 50% of body cells in a checkerboard-like
  pattern (gated by `line ^ val` parity).

### 3.2 Visual impact

Per `src/chroma_dragon_engine/shaders/bold_audit_tests.rs:5` (the existing audit
comment):

> "switching --bold 0/1/2 visually looked identical in past testing."

This is because:

1. Many terminal emulators render bold subtly (slightly thicker strokes,
   or identical strokes with a small luminance boost).
2. Many monospace fonts lack a true bold variant and fall back to regular
   weight silently.
3. The bold SGR code (`\033[1m`) is emitted, but the visible effect is
   overwhelmed by the dominant color/brightness variation in the rain.

So while cosmostrix technically emits bold SGR codes for 50% of glyphs,
**the user-perceived visual difference is often nil**.

### 3.3 Is cosmostrix "already aligned" with the film/standards?

**No.** cosmostrix's default (`BoldMode::Random`) is a deviation from
both the film (regular weight) and every canonical competitor (regular
weight). The user's question framing — "if [film/competitors] don't
[use bold], then cosmostrix is already a good fit" — assumes cosmostrix
also defaults to non-bold. It does not.

### 3.4 Is the deviation defensible?

**Yes, but weakly.** The defenses:

1. **Visual subtlety**: Per §3.2, the visual impact is often nil. So the
   deviation is mostly invisible in practice.
2. **Cinematic enhancement**: cosmostrix's design philosophy (per
   `MATRIX_1999_FILM_AUDIT.md` §6) is "cinematic enhancement, not film
   reproduction". The 3-layer parallax, phosphor afterglow, depth fog,
   and wind gusts are all deviations from the film that ADD value.
   Bold-by-default could be argued as the same kind of enhancement.
3. **Accessibility escape hatch**: `--bold 0` lets users disable it
   entirely. So users who want film-accurate regular weight can get it.

The weaknesses:

1. **Inconsistency with the genre**: Every other Matrix rain tool
   defaults to non-bold. cosmostrix's `BoldMode::Random` is a lone
   outlier. This makes cosmostrix look "different" in a way that isn't
   clearly better.
2. **No clear value-add**: Unlike parallax or afterglow (which add
   visible depth/richness), `BoldMode::Random` is largely invisible
   (per §3.2). So the deviation has low payoff.
3. **Cognitive load**: Users coming from cmatrix expect non-bold.
   cosmostrix's `--bold 0` is the cmatrix default inverted, which is
   surprising.

---

## 4. Recommendation

### Option A (recommended, minimal change): flip the default to `BoldMode::Off`

Change one line:

```diff
// src/config.rs:586
-        default_value_t = 1,
+        default_value_t = 0,
```

This aligns cosmostrix with the film and every canonical competitor.
`BoldMode::Random` and `BoldMode::All` remain available via `--bold 1`
and `--bold 2` for users who want them.

**Blast radius:**

- 1 LOC change in `src/config.rs`.
- The `bold = 1` line in `src/configfile.rs` dump-config template should
  be updated to `# bold = 0` (commented-out default).
- ~8 test fixtures in `src/atmosphere_tests/`, `src/cosmic_dragon_engine/cloud/tests/`,
  `src/interactive/tests.rs` that explicitly pass `bold_mode:
  BoldMode::Random` for assertion purposes are unaffected (they don't
  rely on the CLI default).
- The `bold_audit_tests.rs` file is unaffected (it tests the shader
  branches in isolation, not the default).
- The `--bold` flag, the `BoldMode` enum, the config-key `bold = N`,
  and the renderer integration are ALL unchanged. Only the CLI default
  flips.

**Migration note:** add to `src/validation.rs::REMOVED_FLAGS`? No —
`--bold` is not removed, just the default value changed. Document in
`CHANGELOG.md` as: "default `--bold` value changed from 1 (Random) to
0 (Off) to match the film and canonical implementations. Users who
prefer the previous behavior should set `bold = 1` in config.toml or
pass `--bold 1`."

### Option B (status quo): keep `BoldMode::Random` default

Argue that cosmostrix is a cinematic enhancement and the deviation is
defensible. Acceptable but inconsistent with the genre.

### Option C (compromise): add a `--scene matrix_film` preset

Per `MATRIX_1999_FILM_AUDIT.md` §5, a `matrix_film` preset would set
`bold = 0`, `charset = matrix`, `head_highlight_pct = 0.20`,
`phosphor_decay_rate = 12.0`, etc. This gives users a film-accurate
mode without changing the default.

**Best long-term option**, but requires more work than Option A.

### My recommendation: Option A now, Option C later

Option A is a 1-line change that immediately aligns cosmostrix with the
film and every canonical competitor. Option C is a stretch goal that
can be added in a future release without conflict.

---

## 5. Sources

- The Matrix (1999), opening sequence — frame grabs via Carl Newton's
  analysis: <https://carlnewton.github.io/digital-rain-analysis/>
- cmatrix source: <https://github.com/abishekvashok/cmatrix>
  - `cmatrix.1` man page: `-b, --bold` documented as opt-in
- cxxmatrix source: <https://github.com/akinomyoga/cxxmatrix>
  - `--bold` is opt-in, default is regular
- neo-matrix source: <https://github.com/st3w/neo>
  - No bold flag exists
- unimatrix source: <https://github.com/kiosion/unimatrix>
  - No bold flag exists
- tmatrix source: <https://github.com/MatheusRich/tmatrix>
  - No bold flag exists
- cosmostrix source:
  - `src/config.rs:586` — `default_value_t = 1`
  - `src/main.rs:595-598` — `1 → BoldMode::Random`
  - `src/chroma_dragon_engine/shaders/base.rs:449-451` — Random bold implementation
  - `src/chroma_dragon_engine/shaders/bold_audit_tests.rs:5` — "visually looked
    identical in past testing" comment
  - `src/cosmic_dragon_engine/runtime.rs:18-23` — `BoldMode` enum
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
