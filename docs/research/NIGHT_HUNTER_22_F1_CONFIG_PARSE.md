<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-22 (F1) — the 9x startup config parse, memoized; the intro palette path divergence

Owner approval 2026-09-09: F1 (9x config parse per startup) was the
second item of the approved batch (after the post-exit printer
value-structs; F2, the duration dual-field, follows separately).

## The wart, quantified

`load_config_file` / `load_config_file_full` had no caching. A normal
interactive startup called them 9 times (11 under `--verbose`):

| # | Caller | Phase |
|---|--------|-------|
| 1 | `config_apply.rs` (`load_config_file_full`) | canonical parse + strict validation |
| 2 | `cli/canonicalize.rs` | color alias canonicalization |
| 3-6 | `main.rs` ×4 | color resolution, color-tune, rain-style, charset |
| 7-8 | `build_cloud_cfg.rs` ×2 | ambient schedule + snapback secs |
| 9 | `event_loop.rs` | initial last-applied diff baseline |
| +1-2 | `startup_verbose.rs` ×2 | verbose ambient lines |
| +1 | `verbose.rs` | color provenance |

Each call re-read the file from disk and re-parsed the text (~1 ms
total). The performance cost was trivial — but the SAME wart hid a
real defect:

## The hidden bug: the intro palette read the WRONG FILE

`event_loop_intro.rs` loaded the intro's custom palette via
`load_config_file(None)` — the DEFAULT config path — while
`config_apply` validates `--intro-color` against `args.config` (the
ACTIVE path). Divergence scenario:

    cosmostrix --config custom.toml --intro-color mypalette

with `[colors-custom.mypalette]` defined in `custom.toml`:
- startup validation: reads `custom.toml` → palette found → PASS;
- intro animation: reads `~/.config/cosmostrix/config.toml` → palette
  NOT found → silent fallback to the brand EnergyZen intro.

The user's intro color never showed and nothing reported why. This is
the F1 wart's real teeth: duplicated loads drift, and two of the nine
had already drifted onto different files.

## The fix

### 1. The loader family moved + gained a startup-parse memo

`configfile.rs` was at exactly 800 LOC (zero headroom), so the loader
family moved to a new sibling, `src/config/configfile_load.rs` (~150
LOC): path resolution stays in `configfile.rs`, the disk side (capped
read, /etc fallback, memo) lives in the new module, and both entry
points are re-exported through `configfile` so every historical
`crate::configfile::load_config_file*` path keeps resolving.

The memo (NIGHT-hunter-22, wart F1):

- Keyed on the RESOLVED path (override verbatim, or the default path
  when `None`) — different overrides never alias.
- A path-keyed map bounded by the distinct paths actually requested
  (startup: one; at most two with the watcher-resolution edge case).
- Contract: the config file is stable for the whole startup window;
  mid-run edits flow through the live-reload watcher, which reads via
  `read_config_capped` + `parse_config_text` on its own thread and
  never consults the memo — live reload is untouched.
- Post-startup consumers that want "the config as it was at startup"
  (the event loop's initial last-applied baseline, the intro palette)
  now get the SAME coherent parse validation used.
- Poisoned/contended lock degrades to a fresh parse (correctness
  first); `ParsedConfig` gained `Clone` for the hit path.

All 9-11 startup loads now perform ONE disk read + ONE parse.

### 2. The intro reads the ACTIVE config path

`run_intro_with_color_resolution`'s custom-palette branch now goes
through `intro_custom_palette(cfg)` — a small extracted helper
(`pub(super)`, testable) that reads
`cfg.config_path_for_watcher` (the path resolved from `--config` at
build time). For `--config` users this is a memo HIT (validation
already parsed that exact path); for default-path users the resolved
watcher path matches the default path whenever it exists. The
divergence scenario above now renders the custom palette as
validated.

## Call-site review (why each site is memo-safe)

- `config_apply` — first parse for its path (populates).
- `canonicalize`, `main.rs` ×4, `build_cloud_cfg` ×2, verbose family —
  same path, file untouched between calls → hits, one snapshot.
- `event_loop.rs` initial map — the baseline WANTS the startup
  snapshot (it is the diff source for the live-reload change tracker).
- `event_loop_intro` — see fix 2; wants the validation snapshot.
- `--list-*` commands — 3 loads collapse to 1 parse + 2 hits, then the
  process exits (they run pre-config-apply).
- `--show-scene` — first load for its path, then early exit.
- The watcher thread — bypasses the loader entirely.
- No test suite call site existed; the new tests use unique temp
  paths (the config_apply_tests idiom), which the path-keyed map
  keeps parallel-safe.

## Verification

- **Full suite: 2543 passed, 0 failed** (+4 new: three memo contract
  tests — one coherent parse per path even across an on-disk rewrite,
  path keying, diagnostics through the memo — and one intro palette
  path regression test asserting the `--config custom.toml
  --intro-color <custom>` scenario loads the palette and that a
  palette-less active config fails).
- **clippy `-D warnings` (all targets): clean.** **Gatekeepers:
  10/10.** **LOC guard: green** (configfile.rs 746, configfile_load.rs
  151, config/mod.rs 800 — trimmed comment prose to stay at cap).
- **PTY startup smoke (real debug binary, `--config` + `-v` +
  `--duration 0.6`):** startup parse prints once, the post-exit
  "final runtime state" section prints, exit code 0 — the whole
  startup-to-finalize chain works on the new loader.
- **A/B 10 s benches (pro, dry, cinematic + monolith):**

  | scene     | side       | avg_fps | frame_ms | p99_ms | dirty/fr | entropy_bits | density_gini |
  |-----------|------------|---------|----------|--------|----------|--------------|--------------|
  | cinematic | baseline_A | 29078   | 0.034    | 0.056  | 455.6    | 5.162        | 0.640        |
  | cinematic | after_B    | 28972   | 0.035    | 0.054  | 457.2    | 5.163        | 0.640        |
  | monolith  | baseline_A | 70313   | 0.014    | 0.019  | 140.9    | 4.175        | 0.817        |
  | monolith  | after_B    | 69500   | 0.014    | 0.021  | 140.9    | 4.175        | 0.817        |

  monolith visual metrics identical to the third decimal; cinematic
  inside the documented noise band. The memo runs before the bench
  dispatch and touches no frame path, so this is the owner-rule safety
  check. Full JSON: `benchmark/bench-labs/night_hunter22_f1/`.

## Files

- `src/config/configfile_load.rs` — NEW (loader + /etc fallback +
  startup-parse memo).
- `src/config/configfile.rs` — loaders removed (746 LOC), re-export,
  `ParsedConfig: Clone`.
- `src/config/mod.rs` — module registration (800 LOC, comment prose
  trimmed to hold the cap).
- `src/interactive/event_loop_intro.rs` — `intro_custom_palette` reads
  the active config path.
- `test/config/configfile_tests_inline.rs` — 3 memo contract tests.
- `test/interactive/tests_v51_intro_brand_pause.rs` — intro palette
  path regression test.
- `benchmark/bench-labs/night_hunter22_f1/` — A/B JSON + report.

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
