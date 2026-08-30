<!-- SPDX-License-Identifier: GPL-3.0-only -->

# msg-fill-style Expansion Masterclass — Follow-up Style Candidates

> Status: RESEARCH + DECISION PACKET (2026-08-31, Z-master-1B).
> `engrave` (owner-picked from the advisor discussion) is IMPLEMENTED and
> shipped in this commit. This document presents the remaining candidates
> for the next round — **the owner decides** which (if any) land.
> Predecessor feature: `-mfs`/`--msg-fill-style` (v51, commit 65bdb1df).

## 1. Decision recorded: `engrave` (LANDED)

The owner picked `engrave` from the advisor (deepseek) discussion —
"electron CPU carving text with sparks". What shipped:

- Burn-in reveal: chars appear at full brightness instantly (80 ms/char,
  no 30% fade-in), 2x white-hot head cooling to 1.0 over 300 ms.
- Spark sidecar: 3 particles per newly revealed char, 200 ms lifetime,
  48-slot dedicated pool, rendered inside `draw_message` (on top of the
  text). Movement-gated spawning — one burst per char, never per frame.
- Key architectural lesson (recorded for every future style with
  particles): **the shared quantum pool cannot host overlay-inside
  particles.** `apply_quantum_ripple` renders BEFORE
  `post_rain_processing` → `draw_message`, and `draw_message`
  `set_force`-paints every overlay cell — a quantum-pool spark inside
  the box is overdrawn the same frame. The border-touch spark survives
  only because its particles fly UP, out of the overlay region. Any
  future in-box particle effect must render in a pass at the END of
  `draw_message` (see `cloud/message_engrave.rs`).

## 2. Ground rules for any new style (from the shipped family)

| Rule | Why |
|------|-----|
| Stateless reveal math preferred (pure function of elapsed time) | Zero per-frame bookkeeping, trivially testable, no state to reset on restart/resize/style-switch. 6 of 7 shipped styles comply. |
| If stateful: ONE bounded sidecar struct, pre-allocated pool, O(active)/frame, `--no-effects` gate, reset in `reset_message` + restart paths | The `EngraveState` contract (48-slot pool, movement-gated spawn). |
| Particles inside the box render at the END of `draw_message` | Draw-order constraint (see §1). |
| Default stays `typewriter`, bit-identical pre-v51 | LTS guarantee — every new style is opt-in. |
| New values must touch ALL 9 surfaces | clap enum, `as_str`, `verbose_label`, help_detail block, argv_expand value lists (x2), config_apply error list, configfile comment, dump-config comment, testconf validation (+ docs: README x2, CHANGELOG, this file's style table). |
| Per-cell color changes need a `CellReveal` extension | `CellReveal` currently carries `visible`/`factor`/`slide_rows` only. A tint/ember field is the ONE structural extension point — shared by every future color-shifting style. |

## 3. Candidates (from the advisor discussion, grounded in this codebase)

### A. `hologram` — projected hologram with scanline (RECOMMENDED first)

- **Look**: text flickers in (per-char brightness noise for the first
  ~150 ms after reveal, deterministic from elapsed time), then a bright
  horizontal scanline sweeps down the box once; settled text gets a
  subtle 1-2% brightness ripple (breathing) that stops after ~2 s.
- **Stateless?** YES — fully. Flicker = hash(content_idx, elapsed/40ms
  bucket) modulated factor; scanline position = elapsed/scan duration;
  ripple = sin(elapsed) * small amplitude. No sidecar, no pool.
- **Cost**: ~60-80 LOC in `msg_fill_style.rs` + the 9-surface sweep +
  tests. Cheapest candidate by far.
- **Risk**: low. No color change needed (brightness-only — the existing
  `factor` path already handles > 1.0 boost).
- **Name check**: `hologram` (advisor also offered `holo`, `project`).

### B. `glitch` — cyberpunk distortion settle (RECOMMENDED second)

- **Look**: characters do NOT appear left-to-right. Each char's reveal
  time is a deterministic scramble (hash of its index), each newly
  revealed char flickers between 2-3 wrong glyphs for ~90 ms before
  settling on the true one (Matrix-decode feel).
- **Stateless?** YES with one caveat: the glyph substitution needs the
  *intended* char (available: `mc.val`) plus a deterministic
  pseudo-random pick — hash(content_idx, bucket) into a tiny fixed
  wrong-glyph table. The reveal ORDER can stay index-based but
  visually scrambled via per-cell reveal times
  `(hash(idx) % spread) * X ms`.
- **Cost**: ~80-100 LOC. The only wrinkle: `content_reveal` currently
  returns visibility/factor, not a substitute glyph — needs either a
  `glyph_override: Option<char>` field on `CellReveal` (clean) or the
  substitution done in `message_draw.rs` (less clean).
- **Risk**: medium-low. Glyph substitution interacts with
  `visible_content_cells` test helpers (they compare `cell.ch ==
  mc.val`) — tests must sample after the settle window.

### C. `scorch` — burnt-in text with embers and smoke (highest wow)

- **Look**: chars appear in an ember tint (orange/red) at the head,
  cooling to the palette color over ~400 ms; occasional smoke particles
  drift upward from the head; the cell behind the head briefly chars
  (dimmer + bold).
- **Stateless?** NO — needs BOTH extensions:
  1. `CellReveal` tint field (ember → palette color blend over time),
  2. a smoke sidecar (slow upward gray particles, ~2-3 active, can
     clone the `EngraveState` pattern — pool 16, lifetime ~700 ms).
- **Cost**: ~150-180 LOC + the biggest doc/test surface. The tint
  field is PERMANENT new API surface every future style shares.
- **Risk**: medium. Color blending must route through
  `scale_msg_content_fg`'s pipeline (chroma first, legacy fallback) —
  the blend target (ember orange) is a fixed RGB, unlike every current
  style which only scales the palette head color.
- **Name check**: `scorch` (advisor also offered `burn`, `ember`).

### D. `cascade` — per-column waterfall reveal (defer)

- **Look**: columns light up left-to-right; within each column, glyphs
  drop in top-to-bottom (multi-line messages only — on the common
  single-line overlay it degenerates into a fast left-to-right wipe,
  nearly indistinguishable from `typewriter` at speed).
- **Stateless?** YES — `reveal_at = (col_offset * col_ms) + (row * row_ms)`.
  No sidecar.
- **Cost**: ~50-60 LOC, cheapest after hologram.
- **Risk**: low, but **payoff is low too** — most overlays in practice
  are 1-2 lines (default overlay is one line). Advisor offered
  `cascade`/`waterfall`.
- **Recommendation**: defer until multi-line overlays are common
  (e.g. if a future default overlay gains a tagline row).

## 4. Decision matrix

| Candidate | Stateless | LOC est. | New API surface | Visual distinctness | Verdict |
|-----------|-----------|----------|-----------------|---------------------|---------|
| hologram  | YES | ~60-80 | none | high (scanline + flicker) | **Land first** |
| glitch    | YES | ~80-100 | `CellReveal.glyph_override` | high (scramble decode) | Land second |
| scorch    | no (smoke sidecar) | ~150-180 | `CellReveal` tint + sidecar | highest (color + particles) | Land when the tint API is wanted anyway |
| cascade   | YES | ~50-60 | none | low on 1-line overlays | Defer |

## 5. Preview / verification path

Unlike the charset research (where `[charset-custom.<name>]` shadowing
allowed live A/B against real rain), msg-fill-style has no custom
override — candidates are only previewable once implemented. The
implementation order above is deliberately cheap-first: hologram alone
is a ~1-hour change with zero structural impact, so the owner can
"try before committing to the family direction" — if hologram + glitch
land and feel right, scorch's tint API becomes the natural follow-up.

Recommended acceptance ritual per style (mirrors engrave's test set):
pacing test, brightness/glyph assertion at exact elapsed values,
no-effects test (if stateful), restart re-arm test (if stateful),
live-reload + clap + argv + testconf coverage.

## 6. Style table (current, post-engrave)

| Style | Text reveal | Border | Sidecar |
|-------|-------------|--------|---------|
| `typewriter` | 80 ms/char + 100 ms fade-in (30→100%) | lags text (t^1.5) | none |
| `fade` | instant, block alpha 0→100% (800 ms) | fades with the block | none |
| `words` | 200 ms/word + 150 ms fade-in | lags word progress | none |
| `slide` | 60 ms/char, rises from 1 row below | lags text (t^1.5) | none |
| `pulse` | typewriter + 1.5x scanner cursor | lags text (t^1.5) | none |
| `instant` | full brightness at t=0 | clockwise draw over 1 s | none |
| `engrave` | 80 ms/char burn-in, 2x hot head, 300 ms heat trail | lags text (t^1.5) | 48-slot spark pool (`message_engrave.rs`) |
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
