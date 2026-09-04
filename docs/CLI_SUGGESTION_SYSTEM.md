<!-- SPDX-License-Identifier: GPL-3.0-only -->

# CLI Suggestion System — "tip: a similar value exists" / "tip: a similar argument exists"

> Status: CONSISTENCY AUDIT + STRESSTEST (v60.0.0-beta.1, Z-master-1X)
> and CLI UX CENTRALIZATION (v100.0.0-nightly.1, 2026-09-04).
> The legacy `Did you mean '<value>'?` format was scattered across
> 14+ source files with no consistency. This document records the
> consolidated "tip:" format, the helper API, and the stresstest
> coverage that verifies every CLI surface suggests on typos.
>
> 2026-09-04 update: `src/cli/ux.rs` is now THE presentation contract
> module — `format_value_suggestion` moved there from
> `cli/suggestion.rs`, and the `extract_clap_suggestion` string-parser
> (which re-appended a duplicate flag tip in main.rs) was deleted;
> clap's own render prints the argument tip exactly once. See
> `src/cli/ux.rs` module doc for the full canonical error shapes.

## 1. Two canonical formats

| Surface | Format | Example |
|---------|--------|---------|
| VALUE typo (enum values: colors, scenes, charsets, glitch-level, msg-fill-style, etc.) | `tip: a similar value exists: '<value>'` | `tip: a similar value exists: 'cinematic'` |
| ARGUMENT/FLAG typo (unknown `--foo` flags) | `tip: a similar argument exists: '<flag>'` | `tip: a similar argument exists: '--no-effects'` |

Both formats use the `tip:` prefix (matching clap's own suggestion
output) and are rendered as a newline-prefixed line so they append
cleanly to any error message.

## 2. Helper API (`src/cli/ux.rs` presentation + `src/cli/suggestion.rs` engine)

### Value suggestion

```rust
/// Returns `\n  tip: a similar value exists: '<value>'`.
/// (lives in src/cli/ux.rs — the CLI UX contract module)
pub(crate) fn format_value_suggestion(suggestion: &str) -> String
```

Used by every value-typo call site (colors, scenes, charsets,
glitch-level, monolith-size, color-bg, msg-fill-style, intro-color,
config keys). Call sites that already have the closest match via
`closest_value_match` chain directly:

```rust
let tip = crate::cli::suggestion::closest_value_match(raw, allowed)
    .map(|s| crate::cli::ux::format_value_suggestion(&s))
    .unwrap_or_default();
```

### Argument suggestion

Flag suggestions are rendered by clap ITSELF: the `suggestions`
feature puts the tip into the error's `SuggestedArg` context, and the
cosmostrix-configured `valid` style (in `cli::clap_styles()`, part of
the 2026-09-04 style harmony) renders it WHITE — the NeonWhite head
stop #DCEBFF on truecolor, matching the S-master-HUNT-5 owner color
contract for the ux-side value tips. Value tips embedded in error
messages are painted the same way automatically by the line-aware
`output::eprintln_error_labeled` renderer (any line starting with
`tip:` / `hint:` / `[possible values` inside an error/warning block).
Standalone hint lines use `output::eprintln_suggestion_line`.

History: v50.0.0-beta.7 → v100.0.0-nightly.1 previously kept a
string-parser (`extract_clap_suggestion`) that scraped clap's
rendered tip so main.rs could append a SECOND tip line — producing
the duplicate "tip: a similar argument exists" lines the owner
reported on 2026-09-04. Deleted; the render is clap's own, exactly
once.

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
similarity) renders the tip directly — no custom engine, no
re-rendering, no hand-maintained flag list.

## 4. Call sites (full sweep)

### Value suggestions (use `cli::ux::format_value_suggestion`)

| File | Surface | Candidates |
|------|---------|------------|
| `src/validation/mod.rs` | `--glitch-level`, `--monolith-size`, `--color-bg` (prevalidator) | enum values |
| `src/engine/chroma_dragon_engine/colors_custom.rs` | custom color name | defined palette names |
| `src/scene_custom/mod.rs` | `--scene-custom` | builtin + custom scene names |
| `src/config/config_apply.rs` | `intro-color`, `scene` | builtin themes + custom palettes |
| `src/scene/charset.rs` | `--charset` | `CHARSET_PRESET_NAMES` |
| `src/cli/mod.rs` | `--color` (unknown color) | builtin theme names |
| `src/config/config_hints/mod.rs` | unknown config key | top-level config keys (uses the shared `edit_distance` from `cli/suggestion.rs`) |

### Argument suggestions (rendered by clap / the ux contract)

| File | Surface | Source |
|------|---------|-------|
| `src/cli/ux.rs` (`exit_clap_error`) | every clap parse error | clap's own `SuggestedArg` context — real usage line + help footer appended |
| `src/cli/argv_expand.rs` | `-mfs…` typo (attached form) | hardcoded `--msg-fill-style` via `ux::die_input_with_usage` |
| `src/main.rs` (`prevalidate_cli_args`) | removed-flag migration hints | REMOVED_FLAGS table via `ux::die_input_with_usage` |

## 5. Stresstest coverage

The suggestion engine is stresstested at two levels:

### 5a. Unit tests (in-source, `cargo test`)

36 unit tests covering:

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

### Flag suggestion tests (structured context, `tests/clap_suggestion.rs`)

- `--test` → `SuggestedArg` context points at `--testconf` (the owner's 2026-09-04 case)
- `--no-effecs` → `SuggestedArg` context points at `--no-effects` (the v50.0.0-beta.7 rename regression)
- Distant typos (`--zzzzqqqq`) carry no `SuggestedArg` context (no noise)
- `cli/ux.rs` tests lock the render contract: real usage line (never the suggestion-narrowed `Usage: cosmostrix --testconf`), exactly one tip line, help footer present

### Negative tests (no false suggestions)

- Distance > 2 on glitch-level → no tip
- Distance > 2 on monolith-size → no tip
- Distance > 2 on color → no tip
- Distance > 2 on charset → no tip
- Distance > 2 on custom scene → no tip

### 5b. End-to-end stresstest script (`scripts/cli_suggestion_stresstest.sh`)

Z-master-1X audit added a shell-based end-to-end stresstest that runs
the actual `./target/debug/cosmostrix` binary with a battery of typo /
wrong-value / edge-case inputs and verifies the output format. This
catches integration issues the unit tests miss (clap's full error
rendering, argv expansion, the ux-contract error shapes).

Run it with:

```bash
cargo build --bin cosmostrix
bash scripts/cli_suggestion_stresstest.sh
```

18 cases covering:

- **Long-flag typos** (6 cases): `--no-effecs`, `--colr`, `--crystal-drago`,
  `--msg-fill-styl`, `--verbos`, `--power-drago` — each must produce
  `tip: a similar argument exists: '--<flag>'` and NOT contain the legacy
  `Did you mean` format.
- **Value typos** (8 cases): `neon-gren`, `vapporwave`, `cinemtic`,
  `binari`, `typewritter`, `hollogram`, `defualt`, plus the
  unknown-custom-name `cyberpuunk2077` (verifies no false tip fires for
  names not in the candidate set).
- **Case-insensitivity** (1 case): `NEON-GREEN` must be accepted (no
  "unknown color" error).
- **Too-distant values** (2 cases): `xyzabc`, `zzzzzzz` — must NOT
  produce a tip (distance > 2 threshold).
- **Short-form expansion** (1 case): `-mfss` typo must produce the
  `--msg-fill-style` tip.

Last stresstest run: 18/18 PASS. The script is part of the gatekeeper
suite (bash -n syntax-checked; run manually before releases).

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
