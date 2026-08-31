<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Theme Catalog File-Split — Research Report (Z-master-1B)

> Source code is truth; cross-check the referenced files before relying on
> this analysis for implementation decisions. This document is an internal
> research artifact, not a contract.

**Date:** 2026-08-30
**Auditor:** oxyzenQ (Cosmic Dragon mode)
**Status:** RESEARCH ONLY — no coding yet. Owner to decide Option A / B / C.
**Scope:** `src/engine/chroma_dragon_engine/catalog/` color registry — should the
44-theme data file split into per-theme files (`chroma_cosmos.rs`,
`chroma_aurora.rs`, `legacy_<name>.rs`), split by family, or stay as one
file? Plus: which OTHER `src/` registries deserve the same treatment?

---

## 1. Owner's Request

> "owner want to split into each colors files on src/engine/chroma_dragon_engine/
> catalog/ each files like this: chroma_cosmos.rs, chroma_aurora.rs, for
> legacy legacy_x.rs. or keep default. not just catalog, because src/* need
> to alternative options? because owner which files need to split and keep
> it also want plug and play also flexible for future and reduce cost
> maintenance. after done don't forget to update all docs/reference,
> commented code reference should need update to avoid stale/outdate data."

**Three questions to answer:**

1. Split `catalog/themes.rs` into per-theme files, or keep the monolith?
2. What does `legacy_<name>.rs` mean for themes (there are no legacy themes today)?
3. Do OTHER `src/` registries (scenes, charsets, styles) need the same split?

---

## 2. Executive Summary

| Question | Answer |
|----------|--------|
| Per-theme files (Option A)? | **Rejected** — zero runtime/compile benefit, ~60% more boilerplate, no true plug-and-play (Rust has no file auto-discovery), breaks the "one place to read all 44 themes" audit surface that the LTS lock depends on. |
| Family split (Option B)? | **Deferred with a trigger** — split by FAMILY (not per theme) only when `themes.rs` passes ~1500 LOC (about 25 more themes than today's 44). |
| Keep default (Option C)? | **RECOMMENDED** — the file is pure data, already LOC-exempt by design, already the single source of truth in one grep-able place. |
| `legacy_<name>.rs`? | **Convention reserved, not needed today.** All 44 themes are live; `legacy.rs` is the fallback RGB math pipeline (a different thing, locked, keep). |
| Real plug-and-play? | **Already exists**: `[colors-custom.<name>]` in config.toml adds palettes with zero recompile (`colors_custom.rs`, 644 LOC). The compiled registry is the curated builtin set — its 7-point sync checklist is a deliberate drift-detection design, not accidental friction. |
| Other `src/` registries? | **Keep all consolidated** — scenes, charsets, styles, temperature groups all follow the same one-enum + one-registry + test-mirror pattern, each well under the 800 hard cap. Splitting only colors would break pattern uniformity. |

---

## 3. Current Architecture — What Actually Exists

The color theme system is NOT one file. It is a pipeline of six cooperating
registries, and the data file the owner wants to split is only one of them.

```text
CLI / config.toml (-c, --color, --colors-custom, ambient)
        |
        v
theme/mod.rs (399 LOC)             <- name -> scheme resolution
  THEMES: &[ThemeInfo]             <- 44 names + aliases (e.g. "neon-green")
  THEME_COUNT = 44                 <- manual const
  canonical_name_for_input()       <- alias resolver (config parse time only)
        |
        v
cosmic_dragon_engine/runtime.rs    <- ColorScheme enum, 44 variants
        |
        v
chroma_dragon_engine/catalog.rs (215 LOC)   <- registry LOGIC
  ThemeDef / ThemeColors types, build_colors(),
  graceful degradation (unknown scheme -> greyscale, never panic)
        |
        v
chroma_dragon_engine/catalog/themes.rs (948 LOC)  <- registry DATA (the split target)
  THEMES: &[ThemeDef]  <- 44 entries, pure data, LOC_EXEMPT marker
        |
        v
chroma_dragon_engine/palette/*     <- stops -> palette construction
crystal_dragon_engine/palette_groups/mod.rs (129 LOC)
  <- temperature partition: Cold(14) / Medium(14) / Hot(14) + Reserved(2)
```

### 3.1 themes.rs by the numbers (measured, not estimated)

| Metric | Value |
|--------|-------|
| Total lines | 948 (880 of them theme data) |
| Entries | 44 `ThemeDef` |
| Smallest entry | 16 lines |
| Largest entry | 91 lines (`Spectrum20` — explicit RGB + c16 + ansi tiers) |
| Average entry | 20 lines |
| Logic in file | zero — static data only |
| LOC status | over the 800 hard cap, but carries the `LOC_EXEMPT: pure data file` marker per `src/RULES_LOC.md` "When NOT to Split" |

### 3.2 ThemeColors definition formats (why entries differ in size)

- `Stops { stops, steps }` — gradient stops, c16/ansi auto-derived (space themes).
- `StopsWithC16 { stops, steps, c16, ansi }` — hand-tuned all tiers (Green family).
- `RgbWithC16 { rgb, c16, ansi }` — exact colors, no interpolation (Spectrum20).

---

## 4. The Real Friction: 7-Point Sync for Adding ONE Theme

The owner's goal is "plug and play, flexible, reduce maintenance cost". The
honest measurement of that cost is the checklist for adding one new theme
today. A file split does not remove a single item from it:

| # | File | What must be edited | Nature |
|---|------|---------------------|--------|
| 1 | `src/engine/cosmic_dragon_engine/runtime.rs` | add `ColorScheme` variant | required |
| 2 | `src/engine/chroma_dragon_engine/catalog/themes.rs` | add `ThemeDef` entry | required (the data itself) |
| 3 | `src/theme/mod.rs` | add `ThemeInfo` + bump `THEME_COUNT` | required (name/aliases) |
| 4 | `src/theme/tests.rs` | extend the hardcoded 44-variant array | deliberate drift detector |
| 5 | `src/engine/chroma_dragon_engine/catalog.rs` | tests assert `theme_count() == 44` + a hardcoded scheme array | deliberate drift detector |
| 6 | `src/engine/crystal_dragon_engine/palette_groups/mod.rs` | assign the theme to a temperature group | required for Crystal Dragon drift |
| 7 | `README.md` (2 places) + `--list-colors` help text | update the "44 built-in themes" counts | docs |

Points 4 and 5 look like duplication but are intentional safety nets — the
`every_scheme_has_a_theme` test comment says so explicitly: "This catches
'forgot to add theme after adding enum variant' bugs." The sync checklist is
the drift detector; the file layout is not the bottleneck.

**Conclusion for the split question: relocating point 2 into 44 separate
files leaves points 1, 3, 4, 5, 6, 7 exactly where they are. The
maintenance cost of adding a theme is unchanged.**

---

## 5. Option A — Per-Theme Files (Owner's Proposal)

### 5.1 What it would look like

```text
src/engine/chroma_dragon_engine/catalog/
  mod.rs            <- types + build_colors() + the 44-entry index array
  chroma_green.rs   <- ~35 lines: header + imports + pub const THEME: ThemeDef
  chroma_aurora.rs
  chroma_cosmos.rs
  ... (41 more)
```

The registry becomes `pub static THEMES: &[ThemeDef] =
&[green::THEME, aurora::THEME, cosmos::THEME, ...];` — a 44-entry index
list that must be maintained by hand, plus 44 `mod` declarations.

### 5.2 Cost / benefit, quantified

| Dimension | Effect |
|-----------|--------|
| Boilerplate | 44 files x (SPDX header + 2 imports + wrapper) ~= 440 extra lines vs today (948 -> ~1500 total). ~60% growth in line count for identical data. |
| True plug-and-play | **Not achievable in Rust.** Every module must be declared (`mod chroma_x;`) or indexed; there is no "drop a file in the folder and it registers". Achieving that would need `build.rs` glob codegen — non-idiomatic, invisible to rustfmt/clippy/rust-analyzer, and a build-step liability. |
| Compile time / runtime | No change. The concatenated slice is built the same way at compile time. |
| Audit surface | **Worse.** Today the LTS lock suite and human auditors read ALL 44 themes in one file, in display order. After the split, "what colors ship?" is spread over 44 files + an index. |
| Diff noise | Theme tuning touches one file either way — neutral. |
| LOC policy | 44 files x ~35 lines is far BELOW the 500 soft target — technically compliant but a gratuitous split of a cohesive data file, exactly the case `src/RULES_LOC.md` "When NOT to Split" exists to prevent. |
| Engine lock | `catalog.rs` is on the locked list (`RULES.md` "When to Follow This Protocol"); restructuring the registry is "modernization without measurable benefit", which the UNLOCK protocol lists as NOT acceptable on its own. |
| Naming collision risk | `chroma_<name>.rs` per theme collides conceptually with the family of engine module names (`chroma_dragon_engine/...`); `legacy_<name>.rs` would be dead naming today (see section 7). |

### 5.3 When Option A WOULD make sense

- If themes carried per-theme LOGIC (custom shaders per theme, per-theme
  timing constants) — they do not today; data only.
- If external contributors shipped themes as PRs that constantly conflict
  in `themes.rs` — merge conflicts in a single sorted data file are rare and
  trivial to resolve (adjacent-line conflicts only).
- If the count grew past ~150 themes — see the trigger in Option B.

---

## 6. Option B — Family Split (Middle Ground)

Split by THEME FAMILY, not by theme. Natural families already exist in the
display order:

| File | Themes | ~Lines |
|------|--------|--------|
| `chroma_classic.rs` | Green, Green2, Green3, Carbon, Gold, Gray, Snow, ... | ~250 |
| `chroma_neon.rs` | Neon, NeonGreen, NeonPurple, NeonWhite, NeonBlue, NeonRed, NeonOrange, NeonYellow, NeonCyan | ~180 |
| `chroma_space.rs` | Stars, Mars, Venus, Mercury, Jupiter, Saturn, Uranus, Neptune, Pluto, Moon, Sun, Cosmos, Nebula, Aurora | ~300 |
| `chroma_nature.rs` | Fire, Ocean, Forest, Rainbow, Vaporwave, FancyDiamond, Spectrum20, EnergyZen, ... | ~220 |

Still one index array in `catalog.rs` (`&[classic::THEMES, neon::THEMES,
space::THEMES, ...].concat()` cannot be const — the index would list all 44
or use nested slices at build time via a small const-fn or macro).

**Verdict: correct answer at the RIGHT trigger point, premature today.**
Family split keeps related themes adjacent (a human tuning "all neon themes"
edits one file) at 4 files instead of 44, with negligible boilerplate.

**Recommended trigger: split by family when `themes.rs` passes ~1500 LOC**
(= ~25 more themes than today). Below that, one file is cheaper to read,
grep, diff, and keep SPDX-consistent than four.

---

## 7. The `legacy_<name>.rs` Question

There are NO legacy color themes today — all 44 `ColorScheme` variants are
live and selectable. Two existing things sound like "legacy" but are not
theme files:

1. `src/engine/chroma_dragon_engine/legacy.rs` (346 LOC) — the legacy sRGB-linear
   MATH fallback used when the terminal is not TrueColor (`Color256` /
   `Color16` / `Mono`). It is equations, not theme data, it is on the
   engine lock list, and it has a bit-exact parity test contract. Keep as-is.
2. `ColorPipeline::LegacyRgb` in `runtime.rs` — the pipeline selector enum,
   not a theme registry.

**Reserve the convention, do not create the files:** if a theme is ever
RETIRED but must keep resolving for old configs (aliases keep working,
`--list-colors` hides it), the right shape is:

- move its `ThemeDef` into `catalog/legacy_<name>.rs`,
- keep its `ThemeInfo` + aliases in `theme/mod.rs` (resolution must not
  break — LTS guarantee for user config.toml files),
- exclude it from `compact_list_text()` listing via a `legacy: true` flag
  on `ThemeInfo`,
- document the retirement in the same commit (docs + KEY.md unlock log).

That is a future protocol, documented here so the naming is reserved. Today
it would be four empty files of dead convention.

---

## 8. Real Plug-and-Play Already Exists (the hidden answer)

The owner's "plug and play" goal is already served — at RUNTIME, without
recompiling anything:

```toml
# config.toml — zero recompile, zero Rust knowledge
[colors-custom.sunset]
bg = "#0a0a12"
rain = "#1a0033", "#4d0080", "#9933ff", "#cc66ff", "#e6b3ff", "#f2ccff", "#ffffff"
```

`src/engine/chroma_dragon_engine/colors_custom.rs` (644 LOC) loads these, and
`--list-colors` prints them alongside the builtins ("CUSTOM COLOR PALETTES
(from config)" section). Custom palettes even compose with
`--color-tune`. For a user who wants a new color scheme, the plug-and-play
path is a config edit — not a Rust file.

The compiled `THEMES` registry is the CURATED builtin set. Its barriers are
deliberate curation friction (temperature-group assignment, lock-suite
invariants, visual-identity audit), not accidental bureaucracy. Splitting
files would not lower any of those deliberate barriers.

---

## 9. "Not Just Catalog" — src/ Registry Survey

Every registry-like subsystem in `src/`, measured against the same
questions:

| Registry | File | LOC (cap 800) | Pattern | Verdict |
|----------|------|---------------|---------|---------|
| Theme color data | `chroma_dragon_engine/catalog/themes.rs` | 948 (LOC_EXEMPT, pure data) | `THEMES: &[ThemeDef]` | keep; family-split at ~1500 |
| Theme metadata + aliases | `theme/mod.rs` | 399 | `THEMES: &[ThemeInfo]` | keep |
| Theme lookup mirror | `theme/tests.rs` | 267 | hardcoded 44-variant array | keep (drift detector by design) |
| Scenes | `scene/mod.rs` | 528 | `SCENES: &[SceneInfo]` | keep |
| Charsets | `scene/charset.rs` | 321 | const tables | keep |
| Temperature groups | `crystal_dragon_engine/palette_groups/mod.rs` | 129 | enum partition 14/14/14+2 | keep |
| Message fill styles (v51) | `msg_fill_style/` directory (mod.rs 415 + one file per style, engrave.rs 441 largest) | 8 files, largest 441 | enum + per-style modules (owner-mandated one-file-per-style refactor, post-research) | keep — per-style split already done |
| Rain style | `types/rain_style.rs` | 20 | enum, one file | keep |
| Custom palettes (runtime) | `chroma_dragon_engine/colors_custom.rs` | 644 | config-driven | keep |
| Color cache | `chroma_dragon_engine/color_cache.rs` | 603 | cold-path cache | keep |
| Shortkeys | `interactive/input.rs` + `event_loop.rs` | 536 + 821 (LOC_EXEMPT: coupled loop state) | match on key events | keep — not a registry; unrelated to this split question |

**The uniform finding:** `src/` already converged on one registry idiom —
ONE enum, ONE consolidated data table, ONE metadata table, deliberate test
mirrors as drift detectors. Every file is under the hard cap (or exempt
with a marker). Splitting the color catalog alone would make colors the
ONLY subsystem with a different idiom — the opposite of "reduce
maintenance cost": maintainers would hop between two conventions.

---

## 10. Recommendation

1. **Keep `catalog/themes.rs` as the single 44-theme data file (Option C).**
   It is pure data, LOC-exempt by policy, and the one-place audit surface
   the LTS lock relies on.
2. **Adopt the family-split trigger** (Option B) — documented in this
   report: split into `chroma_{classic,neon,space,nature}.rs` when the file
   passes ~1500 LOC. Add this trigger line to `src/RULES_LOC.md` only when
   the owner ratifies it (it is a policy change).
3. **Reserve the `legacy_<name>.rs` retirement protocol** (section 7) —
   convention documented here; no files created today.
4. **Point future "new color scheme" asks at `[colors-custom]`** — the
   runtime plug-and-play path that already exists.
5. **Do not touch the locked engine files.** Any catalog restructuring is
   an UNLOCK event requiring the A/B + lock-suite protocol
   (`src/engine/chroma_dragon_engine/RULES.md`), which no benefit in Option A
   justifies today.

### FUTURE_BACKLOG candidate (documented, rejected for now)

A `declare_theme!` macro generating enum variant + both registries + count
from ONE table would collapse the 7-point sync checklist to ~2 edits. It is
rejected for v51 because it rewrites the shape of `ColorScheme` (public
contract of a locked engine), forces a lock-suite rewrite, and delivers
zero runtime gain. Revisit if theme additions become a monthly event or the
enum passes ~64 variants.

---

## 11. Stale References Found During This Audit (fixed in this commit)

| File | Stale data | Fix |
|------|-----------|-----|
| `src/engine/chroma_dragon_engine/catalog.rs` (test comment) | "ColorScheme has exactly 52 variants. If a 53rd is added..." | actual count is 44; corrected to 44 / 45th |
| `src/engine/chroma_dragon_engine/README.md` §7 | "Catalog registry (`catalog.rs`, 1134 LOC)" | pre-v50.0.0-beta.7 number; now catalog.rs 215 LOC + catalog/themes.rs 948 LOC (data) |
| `src/engine/chroma_dragon_engine/README.md` §7 | "44 builtin themes, each with head/body/tail RGB stops" | rewrote to match the actual `ThemeColors` tiers (stops / c16 / ansi) |

Historical log entries in `KEY.md` / `RULES.md` UNLOCK log were left
untouched — point-in-time records are not stale data.

---

**Task:** Theme catalog file-split research (Z-master-1B).
**Status:** RESEARCH ONLY — no coding. Owner to ratify Option C + the Option B trigger + the legacy retirement protocol.
**Artifacts:** This report; comment/doc freshness fixes listed in section 11.
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
