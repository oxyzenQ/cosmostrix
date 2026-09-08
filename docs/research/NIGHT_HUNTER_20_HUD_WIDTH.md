<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-20 — HUD 64-column minimum usable width (owner mandate)

Owner bug report 2026-09-08, verbatim intent: loading a scene with a
long name hard-cut the `scn:` HUD metric line — the metrics read
`scn: example_1234_test_this_long` but displayed `scn: example_1234_t`.
The owner mandated that the HUD metrics border width provide a minimum
usable width of 64 characters.

## Root cause chain

The cut was NOT the border clipping text at the terminal edge. It was
a two-part budget chain designed around the pre-hunt width cap:

1. `HUD_MAX_WIDTH = 24` (`src/interactive/hud/mod.rs`) capped the
   dynamic HUD width. `update_metrics` computes
   `current_width = max_line_len.clamp(HUD_MIN_WIDTH, HUD_MAX_WIDTH)`.
2. `set_scene_name` / `set_charset_preset` truncated their inputs to
   14 chars so the composed line (` scn: ` prefix = 6 chars +
   14 = 20) stayed inside the 24-col budget. `example_1234_t` is
   exactly 14 chars — the truncation point, not a border clip.
3. The chroma L-border's right edge sits AT `current_width`, which the
   20-char scn line itself drove — so the border column landed exactly
   where the text ended, making the truncation READ as "hardcut by
   border".

A latent sibling of the same defect survived on the `clr:` line:
`set_custom_palette_name` had NO truncation at all, so a long custom
palette name pushed its line past the width cap and the border
genuinely DID land mid-text on that row (the only real border-cut in
the codebase). This hunt closes it alongside the owner's case.

## Fix

- `HUD_MAX_WIDTH`: 24 → 64 (owner-mandated minimum usable width).
- New shared const `HUD_IDENTITY_VALUE_MAX_CHARS = 58`
  (64 − 6-char label prefixes) used by all three identity setters:
  - `set_scene_name`: 14 → 58 char truncation (UTF-8 boundary safe).
  - `set_charset_preset`: 14 → 58.
  - `set_custom_palette_name`: NEW truncation (was unbounded) — the
    invariant "no HUD line exceeds HUD_MAX_WIDTH, so the border column
    can never land mid-text" now holds for every row.
- `UPTIME_VALUE_MAX_CHARS` stays 19: the tiered ladder's mathematical
  max is 19 chars even at `u64::MAX` seconds; widening the HUD budget
  gives headroom the ladder can never use. Derivation comments in
  `src/clock/mod.rs` and `docs/HUD.md` updated to the new arithmetic.
- Geometry unchanged: `write_to_frame`, `draw_border`, the HB-01
  shrink-clear, and the border's residue-stain clearing all track
  `current_width` already; `draw_border` bounds-checks against the
  terminal width, so terminals narrower than the HUD degrade exactly
  as before (right edge omitted, text clipped at the screen edge).

## Regression tests

- `hud_set_scene_name_and_charset_preset_truncate_long_input` —
  updated to the 58-char budget + asserts `current_width` clamps at 64.
- `hud_long_scene_name_renders_in_full_owner_case` — NEW: the owner's
  exact 27-char scene name renders in full; HUD grows to 33 cols.
- `hud_custom_palette_name_truncates_to_identity_budget` — NEW: short
  name unchanged, 80-char name truncates at 58, `None` falls back to
  the builtin Debug format.

## E2E verification (PTY, real binary)

- NEW `scripts/hud_long_scene_e2e.py`: spawns the binary with a
  custom scene literally named `example_1234_test_this_long`
  (the owner's example), toggles the HUD, reconstructs the virtual
  terminal screen, and asserts screen row 8 carries the full name with
  the border column past the text (col 33 ≥ text end 32). PASS.
- `scripts/hud_order_e2e.py`: PASS (25/25 labels, exact row order) —
  after a methodology repair documented below.

## Discovery: hud_order_e2e.py asserted the wrong surface (pre-existing defect)

The row-order e2e failed identically on the BASELINE tree (b8efb15)
and the fixed tree — a pre-existing script defect, not a regression.
Root cause: it asserted the order of label occurrences in the RAW ANSI
stream. The differential renderer may paint one HUD toggle across
MULTIPLE frame flushes (each bounded by a synchronized-output
`ESC[?2026l` marker), and the flush emission order follows the dirty
cell population, not the visual row layout. Observed: the bottom HUD
rows (cid/up/screensize + border) flushed in frame A and the top rows
(fps…) in frame B, so `cid:`'s stream position sorted FIRST while the
on-screen layout was verified correct (cursor-position escapes showed
fps at row 1, scn at row 9, cid at row 23, up at row 24).

Repair: NEW shared helper `scripts/ansi_screen.py` (mini ANSI screen
reconstructor — applies CUP moves + printable chars to a virtual grid)
and both e2e scripts now assert on the RECONSTRUCTED SCREEN rows,
which is what the user actually sees. The old methodology ran red on
baseline; the new one runs green on the same binary.

## A/B benchmark (owner rule: 10 s, visual + performance)

`--benchmark --bench-duration 10 --json`, pro profile, dry benches.
Baselines captured on the pre-change tree (b8efb15, changes stashed).
Full JSON: `benchmark/bench-labs/night_hunter20/`.

| scene     | side | avg_fps | frame_ms | p99_ms | dirty/pr | entropy_bits | density_gini |
|-----------|------|---------|----------|--------|----------|--------------|--------------|
| cinematic | A    | 27,764  | 0.036    | 0.051  | 463.5    | 5.184        | 0.635        |
| cinematic | B    | 29,011  | 0.034    | 0.054  | 457.6    | 5.166        | 0.639        |
| monolith  | A    | 69,913  | 0.014    | 0.020  | 140.7    | 4.175        | 0.818        |
| monolith  | B    | 70,195  | 0.014    | 0.020  | 140.8    | 4.175        | 0.817        |

monolith is identical to the third decimal on every visual metric.
cinematic's deltas sit inside the documented same-tree noise band (a
third run landed at 28,814 fps between both sides; this host is a
shared cloud VM). The change is also structurally invisible to the
bench path: `HudState` is constructed only in the interactive event
loop — the bench dispatch never builds it.

## Files touched

- `src/interactive/hud/mod.rs` — HUD_MAX_WIDTH 64, identity budget 58,
  three setters, doc-drift fixes (22→64, 5→6 prefix chars).
- `src/interactive/hud/metrics.rs` — stale tgt comment refreshed.
- `src/clock/mod.rs` — uptime budget derivation comment refreshed.
- `test/interactive/hud/tests_chroma_metrics.rs` — 1 updated + 2 new
  tests.
- `scripts/ansi_screen.py` — NEW shared screen reconstructor.
- `scripts/hud_long_scene_e2e.py` — NEW owner-case e2e.
- `scripts/hud_order_e2e.py` — methodology repair (screen assertions).
- `docs/HUD.md`, `CHANGELOG.md` — synced.
- `benchmark/bench-labs/night_hunter20/` — A/B JSON + report.

<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying on
  any number, path, or symbol name here.
-->
