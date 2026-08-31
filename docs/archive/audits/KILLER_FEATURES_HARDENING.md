# Killer Features Hardening Audit — Z-master-1B (v51.0.0-beta.1)

<!-- SPDX-License-Identifier: GPL-3.0-only -->
<!-- Copyright (C) 2026 rezky_nightky (oxyzenQ) -->

Owner directive: "peak optimize and stability/LTS, no hidden bug, potential
problems, & security risk more. special for killers feature that is
colors-custom, charset-custom, & scene-custom." This audit treats the three
custom-block systems as the product's world-class differentiators — no
competitor ships all three — and holds them to the standard that implies.

Method: line-by-line read of every file in the three subsystems
(`chroma_dragon_engine/colors_custom.rs`, `scene/charset_custom.rs`,
`scene_custom/{mod,helpers,overrides,display}.rs`) plus full call-path
tracing across the four contexts that invoke them (startup, live reload,
scene change / ambient, `--testconf`), and doc-vs-code verification for
every documented claim.

## 1. Findings fixed

### K1 — Mid-rain stderr leaks in the killer features (AB-10 class, visual integrity)

The AB-10 incident class ("a stderr line must never leak into the rain
matrix") had been fixed for colors-custom but NOT for the other two killer
features. `collect_charset_custom` re-runs on **every scene change and
every live reload**, and three warning sites still wrote directly to
stderr while the alternate screen was active:

| Site | Fires when | Symptom before the fix |
|------|-----------|------------------------|
| `charset_custom.rs` wide/zero-width skip note | user's `set` contains any wide char | `⚠ skipped 1 wide/zero-width character(s)...` printed into the rain on every scene change |
| `charset_custom.rs` builtin name-collision notice (via `warn_name_collision`) | custom charset shadows a builtin preset | 4-line warning block printed into the rain on every scene change / reload |
| `scene_custom` unknown-scene / invalid-field notes | scene layer re-applies with a bad value | same leak class |

Fix: new session gate `INTERACTIVE_SESSION_ACTIVE`
(`live_config_state.rs`), set once at the top of `run_interactive` BEFORE
the intro sequence can resolve custom blocks. New routing helper
`output::warn_runtime_or_now`: direct stderr pre-session (startup users
keep their immediate feedback, and the startup warning counter still
increments), buffered `push_runtime_warning` mid-session (drained
post-exit on the main screen). All three sites plus
`warn_name_collision` and `warn_invalid` now route through it. Startup
output is byte-identical to before; mid-session output moves to the
post-exit summary where it is readable instead of destructive.

### K2 — Runtime warning buffer spam (no dedup)

`push_runtime_warning` accepted duplicates. The `.stops` deprecation note
re-fires on every `collect_colors_custom` call and the scene-custom
re-apply note re-fires on every live reload while a custom scene is
active — a long editing session could fill the 64-slot post-exit summary
with copies of the same line. Identical messages are now deduplicated
(first occurrence wins); genuinely different messages still accumulate.

### K3 — density-map had NO entry-count cap (memory integrity)

Every other custom-block input is bounded (rain stops 64, charset 256
chars, blocks 100, names 64) — but `density-map` accepted an unbounded
comma list, and `parse_density_map` **leaks** each parsed map
(`Box::leak` into the content-dedup cache). A pasted 1M-entry CSV leaked
~8 MB per distinct value, permanently; a user tweaking the value
repeatedly at runtime grew RSS without bound. New cap
`DENSITY_MAP_MAX_ENTRIES = 1024` (real terminals are a few hundred
columns wide, so 1024 weights is already beyond any legitimate use):
values truncate to the first 1024 entries with a routed warning;
`--testconf` gained the matching ceiling warning (warning, not error —
runtime truncates, so blocking would create a testconf/runtime
divergence, matching the out-of-range-clamp precedent).

### K4 — Live-reload field validation drifted from startup (consistency)

`apply_scene_custom_field_to_cloud_config` (the live-reload arm of
scene-custom) validated three fields differently from the startup arm
(`apply_profile_overrides`) and `--testconf`:

| Field | Startup / testconf | Live reload before the fix |
|-------|--------------------|-----------------------------|
| `bold` | 0..=2, warn + skip otherwise | ANY u8 accepted; 255 silently mapped to Random |
| `shadingmode` | 0..=1, warn + skip otherwise | ANY u8 accepted |
| `async-mode` | `parse_bool` (true/false/1/0/yes/no/on/off), warn + skip on garbage | every non-true string silently meant `false` (`async-mode = "banana"` turned async off without a word) |

Strict validation rejects invalid reloads before this code runs, so the
drift was latent — but latent drift is exactly how hidden bugs are born.
All three arms now share the startup validation and warn (routed) on
invalid values. The applier takes the scene name so warnings carry
context.

### K5 — Dead display code contradicted the owner contract

`show_custom_scene_text` rendered `monolith-size` and `color-bg` rows,
but those fields are FORBIDDEN in `[scene-custom.*]` blocks by the owner
contract (`SCENE_CUSTOM_FIELDS` excludes them; `collect_custom_scenes`
never sets them) — the arms were unreachable. Removed, with a regression
test that locks the forbidden fields out of the display forever.

### K6 — Doc-vs-code contradictions (stale reference purge)

- `charset_custom.rs` module doc claimed wide/zero-width chars are
  "rejected with a clear error" — the implementation SKIPS them with a
  warning (control characters are the hard error). Doc corrected to
  match reality.
- `collect_colors_custom` / `collect_charset_custom` doc comments said
  "bounded by MAX_BLOCKS (64)" — the constant is 100. Corrected.
- Cap tests claimed "only keep the first MAX_BLOCKS entries" — HashMap
  iteration order is unspecified, so "which" is random; only "at most"
  is guaranteed. Comments corrected.

### K7 — Allocation churn on the hot event path (peak optimize)

`is_colors_custom_name` (the boolean "is this a custom palette name?"
probe) rebuilt the full palette BTreeMap on every call — and it runs on
every scene change and live reload via the scene-custom layers. Most
configs define zero custom palettes. The probe now short-circuits when
the config contains no `colors-custom.*` keys at all (mirroring
charset-custom's existing `contains_key` pre-check).

## 2. Verified safe — investigated, no change needed

| # | Question investigated | Verdict |
|---|----------------------|---------|
| V1 | Direct `cfg.get("scene-custom.<name>...")` lookups look case-sensitive while collection is case-insensitive — bug? | **Safe**: `configfile::parse_config_text` lowercases ALL section names and keys at parse time (configfile.rs:217, 229), so mixed-case TOML block names arrive pre-normalized. The case-insensitive collect paths are defensive depth, and the in-repo tests that exercise them construct the map directly. |
| V2 | `parse_density_map` leaks `Box::leak` slices — leak? | **Accepted design, now bounded**: content-deduplicated (one leak per distinct value); with K3 the worst case is 1024 x 8 B + key per distinct value. The poisoned-mutex one-shot path leaks without dedup but is reachable only after a prior panic in another thread. |
| V3 | Input bounds coherent? | Yes after K3: names <= 64, blocks <= 100, rain stops <= 64, charset <= 256 chars, density-map <= 1024 entries, weights clamped [0,1]. No unbounded allocation remains in any killer feature. |
| V4 | Panic surfaces? | `parse_hex_color` slice indexing is guarded by length + ASCII-hexdigit checks; charset control chars are hard errors; guarded `.expect()` calls only run after the matching prefix check proves they cannot fail. No new unwrap was introduced. |
| V5 | Security surface? | No `unsafe` anywhere in the three subsystems, no path handling, no shell-outs, no format-string injection (all messages are built with `format!` on literals). Inputs are bounded and validated; supply chain (dependency advisories/licenses/sources) is covered by the existing cargo-audit / cargo-deny / CodeQL gates. |
| V6 | Per-frame cost? | All killer-feature lookups run at event frequency (startup / scene change / reload), never per frame; the render loop holds no references into the config maps. K7 removes the largest event-frequency allocation. |

## 3. Tests added (+6, suite 1829 -> 1835)

- `push_runtime_warning_dedups_identical_messages` (live_config_state)
- `warn_runtime_or_now_buffers_while_session_active` (live_config_state)
- `parse_density_map_caps_entries_at_max` / `parse_density_map_at_cap_is_not_truncated` (scene_custom)
- `show_custom_scene_text_never_renders_forbidden_fields` (scene_custom)
- `is_colors_custom_name_false_when_no_blocks_defined` (colors_custom pre-check)

## 4. Files touched

`src/config/live_config_state.rs` (session gate + dedup + tests),
`src/config/live_config/mod.rs` (re-exports), `src/interactive/event_loop.rs`
(gate set), `src/output/mod.rs` (`warn_runtime_or_now`, collision routing),
`src/scene/charset_custom.rs` (routing + doc fixes), `src/engine/chroma_dragon_engine/colors_custom.rs`
(doc fix + pre-check + test), `src/scene_custom/{mod,helpers,overrides,display}.rs`
(routing, range parity, scene-name context, density cap, dead-arm removal),
`src/testconf/field_validation.rs` (density-map ceiling),
`docs/RULES.md` (bounds table + routing section), this document,
`CHANGELOG.md`.

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
