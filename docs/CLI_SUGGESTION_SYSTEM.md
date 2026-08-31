<!-- SPDX-License-Identifier: GPL-3.0-only -->

# CLI Suggestion System — "tip: a similar value exists" / "tip: a similar argument exists"

> Status: CONSISTENCY AUDIT + STRESSTEST (v60.0.0-beta.1, Z-master-1X).
> The legacy `Did you mean '<value>'?` format was scattered across
> 14+ source files with no consistency. This document records the
> consolidated "tip:" format, the helper API, and the stresstest
> coverage that verifies every CLI surface suggests on typos.

## 1. Two canonical formats

| Surface | Format | Example |
|---------|--------|---------|
| VALUE typo (enum values: colors, scenes, charsets, glitch-level, msg-fill-style, etc.) | `tip: a similar value exists: '<value>'` | `tip: a similar value exists: 'cinematic'` |
| ARGUMENT/FLAG typo (unknown `--foo` flags) | `tip: a similar argument exists: '<flag>'` | `tip: a similar argument exists: '--no-effects'` |

Both formats use the `tip:` prefix (matching clap's own suggestion
output) and are rendered as a newline-prefixed line so they append
cleanly to any error message.

## 2. Helper API (`src/cli/suggestion.rs`)

### Value suggestion

```rust
/// Returns `\n  tip: a similar value exists: '<value>'`.
pub(crate) fn format_value_suggestion(suggestion: &str) -> String
```

Used by every value-typo call site (colors, scenes, charsets,
glitch-level, monolith-size, color-bg, msg-fill-style, intro-color,
config keys). Call sites that already have the closest match via
`closest_value_match` chain directly:

```rust
let tip = crate::cli::suggestion::closest_value_match(raw, allowed)
    .map(|s| crate::cli::suggestion::format_value_suggestion(&s))
    .unwrap_or_default();
```

### Argument suggestion

Flag suggestions are rendered inline via `eprintln!` with ANSI color
wrappers in `main.rs` and `argv_expand.rs` — no helper function is
needed because the format string is always `tip: a similar argument
exists: '--<flag>'`.

## 3. Engine: `closest_value_match` + `edit_distance`

```rust
/// Levenshtein edit distance (shared engine).
pub(crate) fn edit_distance(a: &str, b: &str) -> usize

/// Closest candidate within edit distance 2 (case-insensitive), or None.
pub(crate) fn closest_value_match(input: &str, candidates: &[&str]) -> Option<String>
```

Policy: distance <= 2 catches typos (transposition = distance 2 in
plain Levenshtein, single insertion/deletion/substitution = distance
1) without suggesting unrelated values. Case-insensitive. Ties
resolve to the FIRST candidate at the best distance.

For FLAG suggestions, clap's own `suggestions` feature (jaro
similarity) is reused via `extract_clap_suggestion()` — no
duplicate engine.

## 4. Call sites (full sweep)

### Value suggestions (use `format_value_suggestion`)

| File | Surface | Candidates |
|------|---------|------------|
| `src/validation/mod.rs` | `--glitch-level`, `--monolith-size`, `--color-bg` (prevalidator) | enum values |
| `src/engine/chroma_dragon_engine/colors_custom.rs` | custom color name | defined palette names |
| `src/scene_custom/mod.rs` | `--scene-custom` | builtin + custom scene names |
| `src/config/config_apply.rs` | `intro-color`, `scene` | builtin themes + custom palettes |
| `src/scene/charset.rs` | `--charset` | `CHARSET_PRESET_NAMES` |
| `src/cli/mod.rs` | `--color` (unknown color) | builtin theme names |
| `src/config/config_hints/mod.rs` | unknown config key | top-level config keys |

### Argument suggestions (inline `eprintln!`)

| File | Surface | Source |
|------|---------|-------|
| `src/main.rs` | unknown `--foo` flag | `extract_clap_suggestion` (reads clap's own tip) |
| `src/cli/argv_expand.rs` | `-mfs…` typo (attached form) | hardcoded `--msg-fill-style` |

## 5. Stresstest coverage

The suggestion engine is stresstested across 36 unit tests covering:

### Value suggestion tests (`closest_value_match`)

- Single-char typo (transposition, substitution)
- Case-insensitivity
- Distance > 2 ignored (no false suggestions)
- Empty input rejected
- Nearest candidate preferred on ties

### Per-surface render tests

- `--glitch-level subtle` typo → `tip: a similar value exists: 'subtle'`
- `--monolith-size large` typo → `tip: a similar value exists: 'large'`
- `--color-bg default-background` typo → `tip: a similar value exists: 'default-background'`
- `--color cosmos` typo → `tip: a similar value exists: 'cosmos'`
- `--color gren` → `tip: a similar value exists: 'green'`
- `--color nebla` → `tip: a similar value exists: 'nebula'`
- `--color vaporwav` → `tip: a similar value exists: 'vaporwave'`
- `--scene cinemtic` → `tip: a similar value exists: 'cinematic'`
- `--scene afternon` → `tip: a similar value exists: 'afternoon'`
- `--charset binari` → `tip: a similar value exists: 'binary'`
- `--charset katakan` → `tip: a similar value exists: 'katakana'`
- custom color `cyberpunk_207` → `tip: a similar value exists: 'cyberpunk_2077'`
- custom scene `afternon` → `tip: a similar value exists: 'afternoon'`
- config key `colr` → `tip: a similar value exists: 'color'`

### Flag suggestion tests (`extract_clap_suggestion`)

- `--no-effecs` → `tip: a similar argument exists: '--no-effects'`
- `--msg-fill-styl` → `tip: a similar argument exists: '--msg-fill-style'`
- `--crystal-drago` → `tip: a similar argument exists: '--crystal-dragon'`
- No suggestion when clap finds no close match

### Negative tests (no false suggestions)

- Distance > 2 on glitch-level → no tip
- Distance > 2 on monolith-size → no tip
- Distance > 2 on color → no tip
- Distance > 2 on charset → no tip
- Distance > 2 on custom scene → no tip

## 6. Migration from `Did you mean`

The legacy `Did you mean '<value>'?` format was replaced across 14+
files. The migration was mechanical:

1. Added `format_value_suggestion` helper in `src/cli/suggestion.rs`.
2. Replaced every `format!("\n  Did you mean '{s}'?")` call site
   with `format_value_suggestion(&s)`.
3. Updated flag suggestion lines in `main.rs` and `argv_expand.rs`
   to use `tip: a similar argument exists: '--<flag>'`.
4. Updated all test assertions from `contains("Did you mean 'X'?")`
   to `contains("tip: a similar value exists: 'X'")`.
5. Updated all doc comments referencing "Did you mean" to the new
   format.

The `closest_value_match` engine and `edit_distance` implementation
are unchanged — only the output format was unified.

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
