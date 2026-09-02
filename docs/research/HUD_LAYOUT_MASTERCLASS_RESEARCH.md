<!-- SPDX-License-Identifier: GPL-3.0-only -->

# HUD Layout & Sci-Fi Dashboard Style — Masterclass Research

> Source code is truth; cross-check the referenced files before relying on
> this analysis for implementation decisions. This document is an internal
> research artifact, not a contract. **No code was changed to produce it.**
> Research scope: internal codebase only (per owner directive) — no external
> sources were consulted; the external AI-chat summary the owner pasted is
> evaluated purely against internal facts in section 3.

## 1. Executive Summary

The owner (chat with DeepSeek, 2026-09-02 ~12:50, pre-flow-focus moment)
proposed a HUD restyle: move the metrics overlay to **bottom-center**, give
it a **triangular-but-elegant silhouette**, keep **FPS on top with the other
metrics below**, and dress it as a **sci-fi space dashboard** (rounded
corners, panel separation, semi-transparent background, scanline effect).
Status per the owner: "agak tolak dan agak mau test implement" — leaning
both ways, research first, nothing in the backlog.

**Research verdict:**

1. **The glyph vocabulary is 100% policy-clean.** The symbol-only output
   rule (`scripts/check-symbol-only-output.sh`) *explicitly allows*
   U+2500..U+25FF (box drawing + geometric shapes) — so `╭ ╮ ╰ ╯`, `╱ ╲`,
   and `▲ ▼` all pass the CI gate without any exemption. Rounded corners
   are in fact **already the project's house style**: the `-mb` message
   border draws a `╭╮╰╯─│` rounded rectangle, and the HUD's own L-border
   already terminates in a `╯` corner. The owner's instinct is not a new
   visual language — it is an extension of an existing one.
2. **The styling half of the idea is cheap; the positioning half is the
   expensive part.** Bottom-center anchoring is technically a *local*
   change (`Frame` exposes `pub width/height`, so `write_to_frame` can
   compute the anchor itself), but it creates three real problems: a
   geometry collision with the centered `-mb` message box on nearly every
   realistic terminal height (section 6), horizontal jitter from the
   dynamic panel width (section 7 cross-cutting), and a rewrite of every
   position-coupled HUD test.
3. **A true full-triangle silhouette is mathematically impossible** with
   24 text metric rows (the bottom rows need ~10+ columns of text). Only a
   chamfered/tapered silhouette (wide top, narrowing bottom — a "pendant")
   can read as triangular.
4. **Two of the four sci-fi finishers from the chat are not implementable
   as described**: app-level semi-transparent backgrounds (ANSI background
   colors are opaque — only the *terminal's own* opacity applies) and GPU /
   kitty-graphics rendering (violates the project's universal-terminal
   layering). Honest substitutes exist for the first; the second is
   rejected outright.

Four implementable options are presented (A: Apex Pendant — the owner's
literal vision; B: Sci-Fi Panel Grid — the compact dashboard; C: Bottom
Console Bar; D: Style-Only Evolution at the current top-left position),
plus a documented rejected-directions list (E). The agent recommendation,
pending owner decision: **Option D now** (zero-risk taste upgrade) if the
bottom-center placement is negotiable, **Option B** if it is not.
The decision menu for the owner is section 11.

---

## 2. Trigger & Source

- **Date**: 2026-09-02 (owner insight the night before; noted before
  entering the day's coding flow).
- **Context**: owner returning to the laptop after breakfast, about to
  enter flow, deferred the idea to research (correctly — the S-master
  stable-release line has priority).
- **External chat summary (DeepSeek, pasted by the owner)**: claims that
  rounded corners are achievable via Unicode box-drawing (`╭╮╰╯`), that a
  top FPS box + metrics below + triangle bottom is buildable, that no
  extra library is needed, and that a GPU (kitty graphics protocol) route
  exists for smoother rendering. Cross-checked against the codebase in
  section 3 — verdict: the Unicode claims hold (and understate how much
  of this is already in the codebase); the transparency and GPU claims do
  not hold for this project (section 7-E).
- **Priority posture**: research-only, nothing implemented, no backlog
  entry (matches the owner's "catat dulu" instinct and DeepSeek's
  LOW-priority advice; see `INSIGHTS.md` Insight 4 entry).

---

## 3. Current-State Audit (verified @ main 9f756c89)

### 3.1 What the HUD is today

| Property | Value | Source |
|---|---|---|
| Toggle | `i` key; zero cost when off (`visible == false` short-circuit) | `hud/mod.rs` |
| Rows | 24 metric rows + 1 border row = 25-row footprint | `hud_init.rs` |
| Position | flush-left, column 0, rows 0..=23 (+ border row 24) | `hud_init.rs` `write_to_frame` |
| Width | dynamic, min 12 (`HUD_MIN_WIDTH`), cap 24 (`HUD_MAX_WIDTH` const) | `hud/mod.rs` L93, L102 |
| Metrics cadence | 1 Hz recompute (`HUD_METRIC_INTERVAL`) | `hud/mod.rs` L75 |
| Colors | 24-stop chroma gradient, dim tail (top, row 0 fps) to bright head (bottom, row 23 screensize), hue-preserving `brighten_color` floor | `hud/colors.rs` `compute_chroma_gradient_24` |
| Border | L-shape: right `│` at col = current width, bottom `─` row 24, corner `╯`; edge fade toward bg (FADE_MAX 0.6, the owner "semi black/fade biar elegant" mandate, rated 9/10); residue-clearing on shrink (owner 8/10 bug, fixed) | `hud_init.rs` `draw_border` |
| Frame writes | `frame.set()` (dirty-skip — unchanged cells are never re-sent); called after `cloud.rain_at()` and before `term.draw()` | `event_loop_sim_draw.rs` L87-92 |
| Row order | fps / tgt / max / p99 / cpu / rss / ehs / prs / scn / chr / clr / sped / dsty / prdr / crdr / ambt / glth / ctun / mnst / dcel / tcel / cid / up / screensize (v80.0.0-beta.1 owner reorder + Z-master-1X round 5) | `docs/HUD.md`, `hud/mod.rs` L46-50 |

Two facts that matter for this research:

- **FPS is already the top row.** The owner's "paling atas itu indicator
  fps, bawahnya metrics indicator lain" is *already the current row
  order*. The delta is purely position (bottom-center) + shape + styling.
- **`h` position-toggle history.** v50.0.0-beta.6 *purged* the `h`
  shortkey (left/right corner toggle) as unused maintenance cost
  (`docs/HUD.md` L13-18). Any new position feature must be a committed
  default, not a resurrected toggle — see INV-6.

### 3.2 Rounded corners are already house style

- The `-mb` message overlay draws a centered box with a
  `╭╮╰╯─│` rounded border (see `RAIN_BORDER_TOUCH_GLOW_AUDIT.md` §2.1;
  `reset_message.rs` BD-01 fixed a 1-cell corner asymmetry the owner
  reported — the owner has already eyeballed rounded corners on this
  project and approved them visually).
- The HUD L-border already uses `╯` as its corner glyph
  (`hud_init.rs` L374).
- Therefore "can the terminal do round corners?" is answered *yes, and
  the project is already doing them on two surfaces*. The only new
  question is layout.

### 3.3 External chat claims vs internal facts

| DeepSeek claim | Internal verdict |
|---|---|
| Rounded corners via `╭╮╰╯` work in modern terminals | **True** — and already shipped here (message border, HUD corner) |
| No extra library needed | **True** — the HUD renders through the cell/frame-buffer pipeline; styling is pure glyph + color choices |
| Best on Alacritty/kitty/GNOME Terminal | **Consistent with `TERMINAL_COMPATIBILITY.md`** — Excellent tier; the Basic tier (Linux console) may tofu the corner glyphs (see INV-3) |
| Background semi-transparan | **Not app-controllable** — ANSI/ECMA-48 bg is opaque; only the terminal's own opacity (kitty/ghostty/wezterm config) composites. See 7-E.2 |
| GPU rendering / kitty graphics protocol (vexart) | **Rejected for this project** — universality violation, dual pipeline. See 7-E.1 |
| `▲`/`▼` for the triangle | **Allowed** (U+25B2/25BC ∈ U+25A0..25FF, gate-clean) but width-ambiguous — prefer structural box-drawing for edges, geometric glyphs only as accent (INV-2) |

---

## 4. Symbol-Only Policy Verification (the gate that decides glyphs)

`scripts/check-symbol-only-output.sh` (v80.0.0-beta.2 owner rule) bans
U+2300-23FF, U+2600-27BF, U+2B00-2BFF, VS16, ZWJ, astral emoji — and
**explicitly allows** (script header, L39-47): "U+2500..U+25FF box
drawing + geometric (banner rules, rain art)". Consequence table:

| Glyph | Codepoint | Block | Gate | Notes |
|---|---|---|---|---|
| `╭ ╮ ╰ ╯` | U+256D-2570 | Box Drawing | **pass** | same class as the shipped `╯` |
| `─ │` | U+2500, U+2502 | Box Drawing | **pass** | already in the HUD border |
| `╱ ╲` | U+2571, U+2572 | Box Drawing | **pass** | diagonal edges for the taper (Option A) |
| `▲ ▼` | U+25B2, U+25BC | Geometric Shapes | **pass** | accent only — see INV-2 |
| `·` | U+00B7 | Latin-1 | **pass** | already used by border-touch halo |

No exemption entry is needed for any option in this document. The sci-fi
aesthetic is fully expressible inside the existing glyph policy — a
non-obvious and load-bearing finding: the owner's two mandates
(symbol-only diagnostics + sci-fi HUD) are **not** in conflict.

---

## 5. Hard Constraints & Invariants (any option must respect)

- **INV-1 — Symbol-only gate**: all output glyphs ∈ gate-allowed classes
  (section 4). Verified for every option below.
- **INV-2 — Width stance**: the project treats ambiguous-width glyphs as
  width 1 (unicode-width non-CJK; `charset.rs` L147 filter, nabla `∇`
  precedent at L230-233). `▲▼` are East-Asian-Width Ambiguous — same risk
  class as the shipped `minimal` rain preset. Policy for the HUD
  (diagnostics layer): structural edges use box-drawing (unambiguous
  width 1); `▲▼` appear only as *decorative accents* whose tofu/wide
  rendering cannot break metric alignment.
- **INV-3 — Terminal compat layering** (`TERMINAL_COMPATIBILITY.md`):
  diagnostics must stay readable on the Basic tier (Linux console / vt.c
  font has partial glyph coverage — the nabla is a documented miss). Rule:
  metric *text* stays ASCII in every option; only frame/edge glyphs may
  degrade. Corners degrading to tofu is cosmetic, never functional.
- **INV-4 — Dirty-cell economy**: HUD writes via `frame.set()` (never
  `set_force`); steady-state HUD cost must remain near-zero. A
  **fixed-width** panel footprint is strongly preferred (see
  cross-cutting X-1) because a dynamically re-centered panel re-dirties
  the whole block whenever a value length changes at 1 Hz.
- **INV-5 — Message-box collision geometry**: the `-mb` box is
  screen-centered (`reset_message.rs` L102-103) and drawn *inside*
  `cloud.rain_at()`; the HUD writes *after* it, so the HUD occludes the
  message box wherever they overlap (same as today). Section 6 quantifies.
- **INV-6 — No position-toggle resurrection**: the `h` shortkey was
  purged (v50.0.0-beta.6, "unused maintenance cost"). A new position must
  ship as ONE committed default; evaluation happens in a branch, not via
  a config key or runtime toggle (a `hud_position` config key drags the
  full config cascade — validation, template, help, live-reload, docs,
  tests — for an evaluation feature).
- **INV-7 — LOC caps**: `hud/mod.rs` is LOC_EXEMPT (single cohesive
  dashboard); `hud_init.rs` / `colors.rs` / `metrics.rs` sit under the
  800-line cap — new geometry belongs in `hud_init.rs`, new text
  assembly in `metrics.rs`.
- **INV-8 — Minimum terminal 80x24** (`SYSTEM_REQUIREMENTS.md`): the HUD
  must never panic; `frame.set()` silently clips out-of-bounds cells, and
  all anchor math must be saturating. A 25-row block on a 24-row terminal
  fills the full height wherever it is anchored (same as today).
- **INV-9 — Existing HUD contracts preserved**: 1 Hz metric cadence,
  per-frame `refresh_colors`, pause-freeze semantics, dirty-cell
  (dcel/tcel) tracking, and the 24 metric *contents* themselves (all
  owner-mandated over v50..v80). No option below drops a metric.
- **INV-10 — Zero cost when off**: `visible == false` short-circuits
  everything; all options keep this.

---

## 6. The Collision Geometry Problem (the central honest finding)

All numbers assume: terminal `H` rows, `-mb` message box height `B`
(content + 2 border rows + 2 padding rows; typical B = 5..9 for short
messages), HUD footprint 25 rows (24 metrics + border).

| Layout | HUD vertical span | Overlaps centered `-mb` box iff |
|---|---|---|
| Today: top-left corner | rows 0..24 | `H < 50 + B` **vertically** — but horizontally the corner (cols 0..24) only meets the centered box when the box starts left of col 25, i.e. on terminals narrower than `~2×25 + box_w` ≈ 80 cols. On ≥ 80-col terminals the corner is clean *by geometry*, which is why the current position never fights the message box in practice. |
| Option A (25-row block, bottom-center) | rows H-25..H-1 | `H < 50 + B` → **overlap on essentially every real terminal** (H < 55..59 for typical B). The HUD occludes the lower rows of the message box (HUD writes last). |
| Option B (13-row grid panel, bottom-center) | rows H-13..H-1 | `H < 26 + B` ≈ H < 31..35 → clean on terminals ≥ ~36 rows; marginal at 30-35. |
| Option C (2-row bar) | rows H-2..H-1 | never (bottom edge is below any centered box on H > 2B+4). |

Reading: the owner's literal vision (all 24 rows stacked at bottom-center)
collides with the centered message box *by construction* — a 25-row
monument covers half-to-all of any realistic terminal, and the middle of
the screen is exactly where the `-mb` box lives. This is not a bug, it is
geometry: corner anchors and center anchors partition the screen, and the
message box already owns the center. The options below differ mainly in
how much height they concede to buy back compatibility with `-mb`.

Acceptance levers (owner's call, not the agent's): (a) accept the
occlusion — HUD wins over the message box, message reveal styles keep
animating underneath (invisible until the HUD moves/shrinks/toggles);
(b) treat `-mb` + HUD-on as a rare combo in the owner's actual flow
(corner-case posture); (c) compact the HUD so the collision threshold
drops below common terminal heights (Options B/C); (d) keep the corner
position (Option D).

---

## 7. Masterclass Options

Layout hint for every mockup: `dim` (tail) rows at the top, `bright`
(head) rows at the bottom — the existing 24-stop gradient is positional
and survives every option (recomputed for the new row count).

### Option A — Apex Pendant (the owner's literal vision)

Bottom-center, all 24 metric rows, FPS at the (wide) top, silhouette
chamfering from the panel width down to a narrow bright foot — a
pendant / nose-cone read, not a mathematically-true triangle (impossible:
`200x50 auto` needs 11 columns at the foot; a 1-cell apex cannot hold
text). Two taper sub-variants:

```text
        A1: diagonal edges (╱╲)              A2: stepped edges (─)
        ╭──────────────────────────╮         ╭──────────────────────────╮
        │ fps: 451      tgt: 60    │  dim    │ fps: 451      tgt: 60    │
        │ max: 1.204ms  p99: 0.832 │         │ max: 1.204ms  p99: 0.832 │
        │ cpu: 1.43%    rss: 8.2MiB│         │ cpu: 1.43%    rss: 8.2MiB│
        │ ehs: 87  prs: 0.12       │         │ ehs: 87  prs: 0.12       │
        │ scn: cinematic           │         │ scn: cinematic           │
        │ chr: binary  clr: Neon.. │         │ chr: binary  clr: Neon.. │
        ╱ sped: 14.0   dsty: 1.00  ╲         ────────────────────────────
        ╱ prdr: on  crdr: off      ╲         │ prdr: on  crdr: off      │
        ╱ ambt: off glth: default  ╲         ────────────────────────────
        ╱ cid: 6ed244b  up: 03:42  ╲         │ cid: 6ed244b  up: 03:42  │
        ╰── 200x50 auto ───────────╯         ╰── 200x50 auto ───────────╯
                    ▼  (accent, optional)              (foot narrows by steps)
```

The gradient story strengthens here: the brightest head rows are exactly
the narrowing foot — "the rain column condenses into a point of light",
and the screensize anchor (row 23, owner mandate "visual anchor at the
very bottom") becomes literally the last row on the physical screen.

- **Mechanics**: anchor row = `frame.height.saturating_sub(25)`;
  per-row width = panel width − taper inset(row); `start_col` per row =
  centered on the *panel* (not the row) so the text grid stays stable;
  chamfer rows use per-row inset with `╱╲` (A1) or short `─` stubs (A2);
  HB-01 trailing-cell clearing generalizes to per-row widths (the
  residue-clearing pattern already exists in `draw_border`'s shrink path).
- **Glyphs**: `╭╮╰╯─│` + `╱╲` (all gate-clean, width-1) + optional `▼`
  accent (INV-2: decorative only). A2 is the conservative sub-variant —
  pure box-drawing, zero ambiguous glyphs, best Basic-tier degradation.
- **Perf**: same cell count as today; **panel width must be fixed**
  (cross-cutting X-1) or every 1 Hz value-length change re-centers and
  re-dirties the whole pendant.
- **Cost**: `hud_init.rs` `write_to_frame` + `draw_border` rewrite
  (~150-250 LOC), ~200 lines of position-coupled test updates in
  `hud/tests.rs`, `scripts/hud_order_e2e.py` re-check; `metrics.rs` text
  assembly unchanged (rows keep their current one-metric-per-line text).
- **Trade-offs**: owner-vision fidelity 10/10; collision per section 6
  (occludes `-mb` on nearly every terminal height); tallest footprint
  (full-height on 80x24 minimum terminals — same as today, but now also
  in the horizontal center).

**Verdict**: the art-directed maximum. Choose it only with eyes open
about INV-5 occlusion and only with the fixed-width rule X-1.

### Option B — Sci-Fi Panel Grid (the compact dashboard)

Bottom-center, all 24 metrics preserved but packed into a 3-column grid
(8 metric rows + a bright FPS header strip + a footer strip), full
rounded `╭╮╰╯` frame, fixed width (~42 cols). This is the literal
"dashboard space" reading of the owner's words — and it is the layout
that makes "FPS on top, metrics below" true *by structure* rather than by
row order:

```text
              ╭──────── fps: 451 ── tgt: 60 ─────────╮   bright header
              │                                       │
              │ ehs: 87       prs: 0.12     scn: cine │
              │ chr: binary   clr: NeonGrn  sped: 14  │
              │ dsty: 1.00    prdr: on      crdr: off │
              │ ambt: off     glth: def     ctun: def │
              │ mnst: normal  dcel: 57/2.9  tcel: 1.9K│
              │ max: 1.2ms    p99: 0.832ms  cpu: 1.43%│
              │ rss: 8.2MiB   cid: 6ed244b  up: 03:42 │
              │                                       │
              ╰──────────── 200x50 auto ──────────────╯   bright footer
                             ▼  (accent, optional)
```

- **Mechanics**: `metrics.rs` `update_metrics` re-assembles text into
  grid rows (three `format!`-joined cells per row, fixed per-cell width —
  labels are 3-5 chars, values bounded by the existing truncation
  setters); `hud_init.rs` draws the rounded rect + header/footer strips;
  gradient recomputed as N-stop over the new row count
  (`compute_chroma_gradient_24` → `_13`); `HUD_MAX_WIDTH` const bumps
  24 → ~44.
- **Glyphs**: pure `╭╮╰╯─│` — zero ambiguous glyphs, best compat.
- **Perf**: ~420 cells + frame vs ~600 today — slightly *cheaper*;
  fixed width = stable footprint, no re-center dirty storms.
- **Cost**: ~250-350 LOC across `metrics.rs` + `hud_init.rs` +
  `colors.rs` (gradient stop count), the same ~200-line test update
  class as A, plus the largest *documentation* cascade (HUD.md's
  line-by-line reference rewrites from rows to grid cells).
- **Trade-offs**: collision threshold drops to H ≥ ~36 rows (clean on
  typical 40+ row terminals; marginal 30-35); height 13 rows; the
  "rain column" vertical gradient poetry compresses to 13 stops (still
  smooth — interpolation handles it); row-order semantics change (the
  v80.0.0-beta.1 owner reorder becomes a grid-zone order: health /
  identity / controls / dragons / efficiency / footer).

**Verdict**: the honest dashboard. Best function-per-risk of the
bottom-center family; the only option that keeps all 24 metrics *and*
clears the message box on common terminals.

### Option C — Bottom Console Bar (minimal horizontal strip)

1-2 rows at the bottom-center edge, FPS prominent left, a rotating or
curated subset of metrics inline; true game-HUD "status bar" reading:

```text
 ╭─ fps: 451 │ tgt: 60 │ p99: 0.832ms │ ehs: 87 │ scn: cinematic ──╮
 ╰─ cid: 6ed244b ─────────────── 200x50 auto ────────────────────────╯
```

- **Mechanics**: `metrics.rs` packs a curated subset (rotation adds a
  timer + selection state); `hud_init.rs` draws the bar; everything else
  shrinks accordingly.
- **Perf**: ~120 cells — cheapest of all.
- **Cost**: ~200-280 LOC, but the *content* sacrifice is the real price:
  the 24 owner-mandated at-a-glance rows collapse to a summary, and the
  "FPS top / metrics below" vertical reading is lost (FPS becomes
  leftmost, not topmost).
- **Trade-offs**: zero collision (bottom edge), but function loss —
  the HUD's documented diagnostic purpose (line-by-line reference in
  HUD.md, diagnostic recipes) degrades to a summary bar. Rotation
  reintroduces the number-flicker problem the 1 Hz cadence was chosen to
  prevent.

**Verdict**: rejected by the agent for function loss; kept in the menu
because it is the only zero-collision bottom-center layout.

### Option D — Style-Only Evolution (position unchanged: top-left)

Keep the corner anchor (all of today's geometry guarantees), upgrade the
visual language: complete the L-border into a full rounded frame
(`╭╮` added at the top, `╰` at the bottom-left, short top/left edge
segments with the existing fade), optional `▼` tail accent under the
bottom border, optional scanline tint (X-3), optional frosted tint
(X-2):

```text
 ╭───────────────────────────╮
 │ fps: 451                  │  dim
 │ tgt: 60    ...            │
 │ (24 metric rows, unchanged)│
 │ 200x50 auto               │  bright
 ╰──────────────▼────────────╯   tail accent, optional
```

- **Mechanics**: `draw_border` extension only (~50-90 LOC): today the
  top/left edges are "implied by the screen edge" — the frame either
  insets the HUD by 1 cell (cleanest, touches the row/col math once) or
  draws the top/left segments over the existing footprint with the same
  edge fade. Text assembly, position, tests' cell-row expectations for
  metrics rows: unchanged.
- **Glyphs**: `╭╮╰╯─│` + optional `▼` accent.
- **Perf**: +~50 border cells, one-time writes (dirty-skip) — negligible.
- **Cost**: the smallest of any option; no INV-5 change; no jitter
  (corner anchor, grow-rightward as today).
- **Trade-offs**: delivers the sci-fi *finish* but not the bottom-center
  *placement* or the triangular *silhouette* — the position ask is
  effectively deferred/rejected, honestly labeled as such.

**Verdict**: the zero-risk taste upgrade. If the owner's core desire is
the *finish* (rounded, elegant, space-dashboard dressing) rather than
the *placement*, this satisfies it with ~5% of the blast radius.

### Option E — Rejected directions (documented, with reasons)

1. **GPU rendering / kitty graphics protocol** (the chat's "vexart"
   suggestion): violates the terminal-compat layering (INV-3) — the
   kitty protocol works only in kitty/ghostty-class emulators, while the
   HUD is a *diagnostics* surface that must render everywhere the rain
   renders, down to the Basic tier. It would fork the HUD into two
   implementations (cell pipeline + graphics protocol), permanently
   doubling the maintenance surface, and it bypasses the dirty-cell
   frame economy (INV-4) that keeps the HUD near-free. Rejected on
   internal grounds alone; the external claim was not otherwise
   verified (research scope: internal only).
2. **App-level semi-transparent background**: ANSI/ECMA-48 background
   colors carry no alpha — a cell bg is opaque by standard. The only
   real transparency is the *terminal's own* window opacity (kitty
   `background_opacity`, wezterm, etc.), which the user configures and
   which cosmostrix cannot influence; today's `bg: None` passthrough
   already benefits from it. The honest in-app substitute is the
   **frosted tint** (X-2): a fixed dark blend that *reads* as tinted
   glass on every terminal, opaque in truth.
3. **True full-triangle silhouette**: a 1-cell apex cannot hold metric
   text (the shortest footer line `200x50 auto` is 11 cells; `up:` and
   `cid:` rows ~9-10). Only chamfered tapers (Option A) or grid panels
   (B) can carry a triangular *feeling* without truncating diagnostics.
4. **Runtime position toggle / `hud_position` config key**: the `h`
   shortkey precedent (purged v50.0.0-beta.6 as unused maintenance cost)
   plus the config-cascade cost (~200-300 LOC across validation,
   template, help, live-reload, docs, tests) for what is an evaluation
   question, not a user feature. Evaluate in a branch; ship one default
   (INV-6).
5. **Scanline as a first-class effect layer**: the frame pipeline's
   `Cell` has no faint/dim attribute (ch/fg/bg/bold only); a real
   scanline layer would either extend `Cell` (frame-pipeline-wide change
   for a cosmetic) or be emulated with alternating row bg tints. The
   emulation is cheap and optional (X-3) but must never default-on while
   the 24-stop chroma gradient carries the vertical rhythm — two
   competing vertical textures fight each other.

---

## 8. Cross-Cutting Finishers (apply to A/B/C/D individually)

- **X-1 — Fixed panel width (strongly recommended for any centered
  layout)**: today's width is dynamic (12-24 cols, grows with values
  like `fps: 11000`). A center-anchored dynamic panel shifts its
  `start_col` on every 1 Hz value-length change — visible horizontal
  jitter plus a full-block dirty re-send. Fix: lock the panel to its cap
  (A/D: 24 cols; B: ~42 cols) and let text left-align inside; the
  footprint becomes constant, the center never moves, and HB-01's
  clear-on-shrink logic simplifies to "always pad to cap". Cost: some
  padding cells; dirty-skip makes them free after the first frame.
- **X-2 — Frosted tint (optional)**: HUD cells' `bg` set to a fixed
  10-15% blend toward black (the `blend_toward_bg` helper already
  exists and powers the border fade). Reads as tinted glass everywhere;
  replaces the impossible true transparency. Trade-off: a solid
  rectangle has more visual weight than today's `bg: None` passthrough
  — on transparent terminals it *reduces* the see-through effect.
  Owner's call.
- **X-3 — Scanline tint (optional, off by default)**: alternate row bg
  luminance ±2-3% (odd/even rows). Zero struct change; per-row bg values
  only. Risk: competes with the 24-stop gradient's vertical rhythm;
  recommend testing *after* the layout decision, never bundled with it.
- **X-4 — Triangle tail accent (optional)**: a single `▼` (or a 3-row
  `╲╱` taper) under the bottom border. Decorative only (INV-2) — tofu on
  the Basic tier is harmless. Cheap (≤ 3 cells), carries the "segitiga"
  cue in every option including D.
- **X-5 — Panel separation**: thin `─` divider rows between metric
  zones (performance core / health / identity / controls / dragons /
  efficiency / footer) — the "separate small panels" sci-fi cue without
  new geometry. Costs 1 row per divider in tall layouts; in Option B the
  grid gutters provide it for free.

---

## 9. Decision Matrix

Scale 1-10 (10 = best / most). "Collision" = `-mb` message-box occlusion
risk (section 6). "Test churn" = higher is better (less churn).

| Dimension | A Pendant | B Grid | C Bar | D Style-only |
|---|---|---|---|---|
| Owner-vision fidelity (position+shape) | 10 | 8 | 4 | 3 |
| Sci-fi finish (rounded/panels/accents) | 9 | 10 | 8 | 7 |
| `-mb` collision safety | 2 | 7 | 10 | 10 |
| Jitter risk (needs X-1) | high → low w/ X-1 | low (fixed by design) | low | none (corner) |
| Perf (steady-state) | 10 | 10 | 10 | 10 |
| Implementation cost (LOC) | ~150-250 core | ~250-350 core | ~200-280 core | ~50-90 core |
| Test churn | high (~200 lines) | high (~200 lines + grid semantics) | medium | low (~30 lines) |
| Docs cascade size | medium (HUD.md position notes) | large (HUD.md full rewrite) | large (HUD.md full rewrite) | small |
| Glyph policy risk | 0 (clean) | 0 (clean) | 0 (clean) | 0 (clean) |
| Basic-tier degradation (cosmetic only) | corners/diagonals tofu | corners tofu | corners tofu | corners tofu |
| Keeps all 24 metrics at a glance | 10 | 10 | 2 | 10 |
| LTS / universality posture | 8 | 9 | 9 | 10 |

---

## 10. Agent Recommendation

**Primary: Option D (style-only evolution) — if the bottom-center
placement is negotiable.** The research shows the owner's sci-fi
*finish* (rounded corners, elegant fade, optional tail accent) rides on
infrastructure the project already owns (rounded-border house style,
edge-fade helper, dirty-cell economy) at ~5% of the blast radius of a
reposition. The bottom-center placement is where all the real cost
lives (collision, jitter, test churn) — and its benefit is aesthetic
placement, not function.

**If bottom-center is non-negotiable: Option B (panel grid).** It is the
only layout that keeps all 24 owner-mandated metrics, honors "FPS on
top, metrics below" structurally, clears the message box on common
terminal heights (≥ ~36 rows), and fixes the jitter problem by design
(fixed width). Option A (pendant) is the art-directed maximum — choose
it only if the owner explicitly accepts the `-mb` occlusion trade and
the full-height footprint on smaller terminals.

**Sequencing advice** (matches the owner's LOW-priority instinct and the
stable-release line): land nothing before the current stable-release
track completes; when picked up, implement the chosen option behind a
branch first, PTY-verify with the existing `scripts/repro_driver.py`
harness patterns, and only then commit to the default — no config key,
no toggle (INV-6).

---

## 11. Open Questions for the Owner (the decide/approve menu)

1. **Placement**: commit to bottom-center, or keep the top-left corner?
   (D vs A/B/C — the single highest-leverage decision.)
2. **If bottom-center**: is the `-mb` occlusion on < ~36-row terminals
   acceptable, or is `-mb` + HUD-on a combo you actually use?
3. **Shape**: pendant taper (A), grid panel (B), console bar (C), or
   frame + tail accent only (D)?
4. **Taper variant** (A only): diagonal `╱╲` edges (more literal
   triangle, Basic-tier tofu on diagonals) or stepped `─` chamfer
   (conservative, pure box-drawing)?
5. **Tail accent `▼`** (any option): include or skip?
6. **Frosted tint X-2**: tinted-glass look, or keep `bg: None`
   passthrough (lets your terminal's own opacity show through)?
7. **Scanline X-3**: worth a post-layout test, or skip entirely?
8. **Fixed width X-1**: approve locking the panel width (kills jitter,
   costs padding)?
9. **Timing**: after the stable release, as DeepSeek advised — agree?

---

## 12. Pre-Existing Doc Drift Noticed During Research (no code changed)

Future doc-cascade material; recorded here so it is not lost:

- `hud/mod.rs` module doc still says "writes a compact 5-line overlay"
  (v16-era wording; the HUD renders 24 rows since v50/Z-master-1X).
- `hud/mod.rs` L96-101 comment says the width cap was "bumped 20 → 22"
  while the const is `HUD_MAX_WIDTH: u16 = 24`; `docs/HUD.md` L129 also
  says "capped at 22 cols". Code is truth: 24.
- `hud_init.rs` L129-131 references a `HUD_DISPLAY_MAX_HZ` rate limit
  in a doc comment; no such constant exists in the codebase (stale claim
  — verify before citing in future docs).
- `hud/mod.rs` `refresh_colors` doc block still narrates a "16-stop"
  gradient while the implementation is 24-stop
  (`compute_chroma_gradient_24`, divisor 23).

---

## 13. References

- `src/interactive/hud/mod.rs` — HudState, row order, HUD_MIN/MAX_WIDTH,
  design-constraint docs
- `src/interactive/hud/hud_init.rs` — `write_to_frame` (L143-225),
  `draw_border` (L282-393), FADE_MAX, residue clearing
- `src/interactive/hud/colors.rs` — `compute_chroma_gradient_24`,
  `brighten_color`
- `src/interactive/hud/metrics.rs` — 1 Hz metric tick + text assembly
- `src/interactive/event_loop_hud.rs` — per-frame setter plumbing
- `src/interactive/event_loop_sim_draw.rs` L87-92 — call order
  (refresh_colors → write_to_frame, before `term.draw()`)
- `src/engine/cosmic_dragon_engine/cloud/reset_message.rs` L96-103 —
  message-box centering math
- `src/engine/cosmic_dragon_engine/frame.rs` L72-73 — `pub width/height`
- `src/scene/charset.rs` L141-150, L226-233 — width filter + nabla
  ambiguous-width precedent
- `scripts/check-symbol-only-output.sh` L39-47 — glyph allowlist
  (U+2500..U+25FF)
- `docs/TERMINAL_COMPATIBILITY.md` — glyph policy + Basic-tier limits
- `docs/HUD.md` — HUD reference + `h` shortkey purge history
- `docs/SYSTEM_REQUIREMENTS.md` — 80x24 minimum terminal
- `docs/research/RAIN_BORDER_TOUCH_GLOW_AUDIT.md` §2.1 — message border
  `╭╮╰╯─│` house style
- `INSIGHTS.md` — Insight 4 entry (this research's trigger record)

---

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
