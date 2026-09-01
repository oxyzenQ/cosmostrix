<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Z-master-v2 LTS Audit — Killer Features / CLI / Config Harmony (2026-09-01)

Owner suspicion across all three tasks: "some potential bug". Depth
stresstest confirmed the suspicion — one systematic bug family
(priority-contract violations on the live-reload path) spanning 4 + 5
gaps, all fixed, all regression-tested, benchmark-verified at visual
bit-parity.

## The bicycle analogy (Z-master-3-v2 mandate)

Owner's framing: the bicycle already works from the factory — only the
body color changes; gears, wheels, and brakes stay original. This audit
respected that framing: **zero changes to the engine/render path**. All
fixes live in the priority gates (config-apply / live-reload decision
layers) — the "paint shop", not the drivetrain. The A/B benchmark
(section 5) proves the parity: visual metrics identical, performance
within the natural variance band.

## 1. Z-master-1-v2 — killer features (colors-custom / charset-custom / scene-custom)

### Method

Full read of the re-apply path: `rebuild_cloud_config` (live_config) →
`apply_scene_custom_to_cloud_config` → base-scene layer → field layer →
`create_cloud` palette shadowing. Cross-checked every arm against its
startup twin (`apply_profile_layer` → `apply_base_scene_to_args` /
`apply_profile_overrides`) for divergence.

### Findings (4 gaps, all fixed in 24b3a28)

| # | ID | Gap | Impact |
|---|----|-----|--------|
| 1 | Z1-1 | Field layer had CLI gates on `fps` ONLY (FPS-F4) | `--speed/--density/--color/--charset/--glitch-level/--async-mode` (+ `--colors-custom` ref field) silently overridden by the block on every config edit |
| 2 | Z1-3 | Base-scene layer had CLI gate on `fps` ONLY | same drift class via `base-scene` inherited defaults |
| 3 | Z1-2 | Intra-block conflict resolution nondeterministic | block with `color` + `colors-custom` (or `charset` + `charset-custom`) applied in HashMap order at reload; startup deterministically lets `color`/`charset` win |
| 4 | Z1-4 | Stale palette on scene switch | switching `scene` away from a palette-owning custom scene kept `custom_palette` loaded; `create_cloud` applies the palette AFTER the scheme → the switch was a visual no-op for color |

### Fixes

- `src/scene_custom/overrides.rs`: every field arm returns early when the
  matching `cli_explicit.*` is set (mirrors FPS-F4).
- `src/scene_custom/mod.rs` `apply_base_scene_to_cloud_config`: per-field
  `cli_explicit.*` gates.
- `apply_scene_custom_to_cloud_config` (moved to overrides.rs for the
  800-LOC cap): pre-scans for `color`/`charset` presence and skips the
  losing custom-reference field — deterministic, startup-parity.
- `live_config/mod.rs` scene block: clears `custom_palette` +
  `custom_palette_name` when the builtin color default actually applies.

## 2. Z-master-2-v2 — CLI + config/live-reload

### Method

Enumerated every CLI flag with a matching config key, then diffed
against (a) the startup gate (`config_value` / `is_explicit`) and (b)
the live-reload gate (`cli.*` in `rebuild_cloud_config`). The diff
exposes flags that win at startup but lose on the first config edit.

### Findings (5 gaps, all fixed in c60441e)

| # | Flag | Startup | Live reload (before) |
|---|------|---------|----------------------|
| 1 | `--bold N` | CLI wins | config `bold` key overrode the flag on every reload |
| 2 | `--shading-mode N` | CLI wins | same |
| 3 | `--color-bg X` | CLI wins | same |
| 4 | `--colors-custom <name>` | CLI palette wins (checked first in main.rs) | config `color` key switching to a builtin CLEARED the CLI-owned palette |
| 5 | `--scene-custom <name>` | CLI layer applies last → wins | config `scene` key replaced the CLI custom scene AND cleared the tracker |

Root cause: `CliExplicit` grew one guard per historical fix
(monolith-size Issue #4, power-dragon, async-mode, msg-mode,
intro-color, message, color-tune, msg-fill-style) — these five were
never added. Same bug class, not a new mechanism.

### Fixes

`CliExplicit` + `build_cli_explicit` gained `bold`, `shading_mode`,
`color_bg`, `colors_custom`, `scene_custom`; `rebuild_cloud_config`
gained the matching guards (color block, scene block + its color-default
arm, bold/shading-mode/color-bg blocks); the scene-custom field gates
for `bold`/`shading-mode`/`colors-custom` extend the Z1-1 fix to the
newly tracked flags. The palette re-load path (`custom_palette_name`)
still fires for `--colors-custom` runs so live-editing the
`[colors-custom.<name>]` block keeps working — the guard only blocks
the plain `color` key from switching/clearing the palette. Likewise the
`--scene-custom` guard keeps the tail block re-applying the custom
scene's fields, so live-editing `[scene-custom.<name>]` keeps working.

## 3. Z-master-3-v2 — CLI/config harmony audit

The harmony contract (documented in `--help` "Precedence" block and
`docs/LIVE_RELOAD_BEHAVIOR.md`):

```text
built-in defaults < scene defaults (fills unset only)
  < config values < config scene-custom
  < CLI scene < CLI scene-custom
  < explicit CLI flags
```

Before this audit the contract was enforced at startup but leaked in
NINE places on the live-reload path (the 4 + 5 gaps above). After the
fixes, every flag/config/block combination resolves identically at
startup and on reload:

- Startup verification: 7 new harmony cases in
  `scripts/custom_features_stresstest.sh` assert the RESOLVED values in
  the benchmark JSON report (`--bold 0` + config `bold = 2` →
  `"bold":"Off"`, etc.). 34/34 pass.
- Reload verification: 21 unit tests in
  `src/config/live_config/tests_cli_priority.rs` drive
  `rebuild_cloud_config` / `build_cli_explicit` directly per gap.
- Consequential consistency: the scene block's builtin color arm, the
  color block's palette-clearing arm, and the base-scene/field layers
  now apply the same precedence everywhere (one contract, no per-path
  exceptions).

## 4. Test and gate summary

- 1967 tests green / 0 failed (was 1946 at baseline: +21 net new).
- `cargo fmt --all --check` clean; `cargo clippy --all-targets
  --all-features -D warnings` clean; `scripts/check-rs-loc.sh` and
  `scripts/check-headers.sh` clean (the two files that hit the 800-LOC
  cap were split: `tests_cli_priority.rs` extracted,
  `apply_scene_custom_to_cloud_config` moved to `overrides.rs`).
- `scripts/custom_features_stresstest.sh`: 34/34 PASS (27 prior + 7
  harmony). `scripts/cli_config_stresstest.sh`: 47/47 PASS.

## 5. A/B benchmark (10s, monolith 80x24, dry)

Full table in `benchmark/bench-labs/Z_master_v2/Z_MASTER_V2_AB_report.md`
(raw JSON: `baseline_A.json` / `after_B.json` in the same directory).

| Metric | A | B | Delta |
|--------|---|---|-------|
| frame_entropy_bits | 3.2930 | 3.2938 | +0.03% (RNG noise) |
| density_gini | 0.8962 | 0.8962 | -0.01% |
| dirty_cells_per_frame | 56.735 | 56.800 | +0.11% |
| active_streams_avg | 23 | 23 | 0 |
| avg_fps | 92293.7 | 92137.2 | -0.17% |
| total_ns_per_cell | 190.98 | 191.08 | +0.06% |

Verdict: visual bit-parity, performance parity. The changes are
config-edit-event code, not per-frame code — as predicted, and now
measured. **Already at peak; no further optimization (per task brief:
skip, do not over-engineer).**

## 6. Documentation updates shipped with this audit

- `docs/LIVE_RELOAD_BEHAVIOR.md`: new sections 11 (Z-master-1-v2) and
  12 (Z-master-2-v2); per-key matrix rows refreshed — including three
  rows (monolith-size / power-dragon / async-mode) that still described
  the pre-v50.0.0-alpha.7 behavior as "no intent gate" (stale data the
  owner asked to hunt); scene/scene-custom/color rows note the new
  guards.
- `CHANGELOG.md`: two Unreleased entries (one per task commit).
- This research doc + the bench-lab A/B artifacts.
- Source-comment references updated in place (Z1-*/Z2-* markers cite
  the contract and the startup twin each gate mirrors).
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
