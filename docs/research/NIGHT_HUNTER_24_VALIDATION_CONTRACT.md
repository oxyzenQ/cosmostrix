<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-24 (F-24-1) — the colors-custom load contract lived on only one side

Owner approval 2026-09-09: fresh sweep of the `--testconf`/validation
layer — the habitat where the hunter-23 drift species lived. Four
findings; one hidden defect (split-verdict, repro'd end-to-end), three
same-species cleanups (probe-fold, stale doc, twin predicate).

## The defect (F-24-1)

The runtime constructor `CustomPaletteDef::to_palette`
(chroma_dragon_engine/colors_custom.rs) requires **at least 2
parseable rain stops** and hard-errors below that. No validation
surface ever checked the count: `validate_colors_custom_value`
checked hex FORMAT only, and the reference probes
(`validate_field_value_with_cfg`) checked KEY EXISTENCE only
(bg || rain || stops contains_key, hand-copied three times).

One config therefore produced four different verdicts:

| surface | verdict for a bg-only / single-stop block |
|---|---|
| `--testconf` | PASS — "config is valid" |
| startup `--colors-custom z` / `color = z` (main.rs) | fatal exit — `to_palette` Err AFTER validation promised the file was fine |
| startup `intro-color = z` (event_loop_intro.rs) | silent brand-palette fallback (documented as "validation failed silently at startup") |
| live-reload `color = z` (live_config/mod.rs), scene-runtime ambient (scene_runtime.rs, overrides.rs) | silent no-op — debug trace only ("keeping current") |

That is the F-23-1 multi-surface disagreement signature, surviving at
the VALUE boundary instead of the NAME boundary. The silent branches
are the HUD-honesty violation family: the user asks for `z`, nothing
happens, no error, the `clr:` HUD never moves.

End-to-end repro (pre-fix, pro binary): a two-line config
(`color = "z"` + `[colors-custom.z] bg = "#0a0a12"`) passed
`--testconf` with exit 0 and then died at startup with the raw
`to_palette` error.

## The fix — validation asks the loader

Single-source contract helpers in colors_custom.rs (next to the
constructor they consult):

- `colors_custom_load_error(cfg, name)` — returns the exact
  `to_palette` error the runtime would produce, so the validation
  layer and the loader cannot drift apart again.
- `validate_colors_custom_blocks(cfg)` — the colors-custom analogue
  of the scene-custom completeness mandate: every DEFINED block must
  satisfy the load contract, referenced or not (an unreferenced
  deficient block is dead weight; a referenced one is the
  split-verdict defect).

The three hand-copied any-of-3 probes in field_validation.rs
(`colors-custom`, `intro-color`, `color` branches) are REPLACED by
canonical `is_colors_custom_name` + the load-contract check (F-24-2
fold — the same duplication shape that caused F-23-1). The
block-level gate runs beside the scene-custom completeness gate in
both `validate_config_strictly` (startup + live-reload watcher) and
`--testconf`'s report loop.

Post-fix end-to-end (pro binary, same repro config):

```
error: testconf: colors-custom.z: custom palette needs 'rain' field with at least 2 hex colors
error: testconf: color = z: color 'z' references a custom palette that cannot build: ...
testconf: FAIL ... EXIT=2
error: invalid config — colors-custom.z: custom palette needs 'rain' field ...
EXIT=2
```

Control (rain-only, 2 stops, bg optional — the F-23-1 acceptance):
`--testconf` PASS, timed run exits at the requested duration. The
live-reload watcher rejects a deficient edit through the same strict
gate (old config kept, rejection logged) — no more silent
"keeping current".

## Same-species cleanups

- **F-24-2**: the any-of-3 probe fold described above (three copies →
  canonical calls).
- **F-24-3**: colors_custom.rs module doc claimed
  `ambient.22-00 = sunset` as a palette use — ambient keys name a
  SCENE (palette references travel through a scene-custom block's
  `color` field). Stale example corrected.
- **F-24-4**: `config_hints::is_valid_colors_custom_field_str` was a
  hand-written twin of configfile's private
  `is_valid_colors_custom_field`, "kept in sync via tests" — the
  exact root-cause shape of F-23-1. The canonical function is now
  `pub(crate)` and the twin is deleted.

## Known edge (documented, not changed)

Block names longer than `COLORS_CUSTOM_MAX_NAME_LEN` (64) pass the
key-shape check (`is_valid_custom_name` has no length cap) but are
silently skipped by `collect_colors_custom`. A REFERENCE to such a
name now rejects cleanly ("unknown block" — the canonical helper
does not see the entry). An unreferenced oversized block remains
inert dead weight; flagging it would need a dedicated length check
in the block gate, deliberately out of scope for this fix.

## Verification

- 2561 tests pass (2551 + 10 new: 3 inline contract-helper tests,
  7 validation-surface tests incl. a flipped drift-pin — the old
  `only_bg_field_still_accepted` test had pinned the defect).
- check-all -q exit 0; gate-keepers 10/10.
- PTY smoke on the real pro binary: deficient config → testconf
  FAIL exit 2 + startup clean rejection exit 2 (identical message);
  valid rain-only control → PASS + 619 ms timed exit.
- A/B 10 s benches (stash methodology, 3 run pairs): visual metrics
  identical to the 3rd–4th decimal (entropy/gini/dirty); fps deltas
  inside the run-to-run noise band with interleaved runs (cinematic
  spread 1.96 %, monolith 4.93 % incl. one 66.7 K outlier run; the
  other two after-runs interleave baseline). Structural argument:
  the bench path never executes validation code — the change is
  cold-startup-only. Artifacts: benchmark/bench-labs/night_hunter24_1/.
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
