<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-23 (F-23-1) — the intro-color gate read the wrong key; the watcher's 8-param signature

Owner approval 2026-09-09: sweep the post-exit/final-state family and the
live-reload rebuild path for fresh prey. Two findings; one hidden defect
(end-to-end repro), one flow-elegance fix.

## Sweep scope and what checked out CLEAN

Post-exit family (final_state.rs, event_loop_finalize.rs,
output/post_exit.rs, main.rs drain): the SessionState from_live field
sources were verified against the ambient apply path — cloud-sourced
fields (color/speed/density/glitch) are exactly the fields
apply_scene_runtime writes to the cloud; cfg-sourced fields are exactly
the ones ambient never writes. The finalize handoff feeds
ctx.config.current (post-reload). No drift found — the hunter-22
value-struct refactor held.

Live-reload rebuild path (watcher.rs, live_config/mod.rs,
event_loop_config_rebuild.rs): the rebuild path's many S-master/HUNT
fixes held under review; the debounce-window-loses-rapid-second-edit
case is covered by the polling heartbeat's burst mode by design.

## F-23-1: the intro-color custom-palette gate (hidden defect, repro'd)

Both validation gates for intro-color built their custom-palette probe
as:

    cfg.contains_key(&format!("colors-custom.{v}.bg"))

Two defects, both reproduced end-to-end on the real binary:

1. **bg-only**: `bg` is OPTIONAL in the palette schema — `to_palette`
   requires only `rain` with >= 2 hex stops. A rain-only palette
   (`[colors-custom.mine] rain = "..."`) is a fully valid palette that
   `color = mine` loads from the same file — but `intro-color = mine`
   was a HARD STARTUP ERROR ("not a builtin theme or custom palette",
   exit). On live-reload the same probe soft-fail CLEARED the value.

2. **case-sensitive probe vs lowercased keys**: the parser lowercases
   every key, but the probe embedded the intro-color VALUE verbatim —
   `intro-color = MINE` probed `colors-custom.MINE.bg` and hard-failed
   although `load_custom_palette` normalizes case and would load.

3. **Surface disagreement** (the drift that hid it): testconf's own
   probe is any-of-3 fields (bg OR rain OR stops) on the lowercased
   name — so `cosmostrix --testconf` said "valid", then the run died
   at config_apply. The comment claiming "Same logic as
   config_apply.rs" was false in both directions.

The canonical helper `is_colors_custom_name` (case-insensitive both
sides, block-cap aware — the same function the `color =` key gate uses
at 7 call sites) replaces the probe at both gates. The intro's own
resolution chain (resolve_intro_palette_source → intro_custom_palette →
load_custom_palette) was already correct — only the gates were broken.

Repro matrix (real debug binary, before → after):

| config                                     | before            | after   |
|--------------------------------------------|-------------------|---------|
| rain-only palette + `intro-color = mine`   | hard error, exit  | accept  |
| bg-ful palette + `intro-color = MINE`      | hard error, exit  | accept  |
| `intro-color = doesnotexist`               | hard error, exit  | same    |
| mid-run switch to rain-only `mine2`        | silent clear      | apply + final-state diff |

## F-23-2: handle_notify_event's 8-param signature (elegance)

The live_config family's last `too_many_arguments` allow carried the
watched file TWICE — `target_file: &Arc<PathBuf>` for the
touches-target filter and `path: &Path` for the snapshot/read, same
file, two views (the F2 dual-field shape as positional parameters). A
future call site could pass different paths and the event filter would
silently diverge from the read path. Bundled into a `WatchSession`
struct (file + tx + debounce + dedup + burst state): 8 params → 2, the
allow deleted, the handler made module-private (single caller).
Zero-behavior-change refactor; the live-reload e2e below exercises the
whole path on the real binary.

## Verification

- **Full suite: 2551 passed, 0 failed** (+5: two startup-gate tests
  [rain-only accepted, mixed-case accepted], three live-reload-gate
  tests [rain-only kept, mixed-case kept, unknown still cleared]).
- **Live-reload e2e (PTY, real debug binary):** startup with
  `intro-color = mine` (rain-only), mid-run edit to `mine2` (new
  rain-only block) — accepted, no rejection, exit 0, and the post-exit
  final runtime state honestly reports `intro_color: "mine2" (was
  "mine")`. Before the fix the mid-run edit was silently cleared.
- **clippy `-D warnings` (all targets): clean. Gatekeepers: 10/10.
  LOC guard: green** (config_apply.rs 798 — comment prose trimmed to
  hold the cap).
- **A/B 10 s benches (pro, dry, cinematic + monolith):**

  | scene     | side       | avg_fps | frame_ms | p99_ms | dirty/fr | entropy_bits | density_gini |
  |-----------|------------|---------|----------|--------|----------|--------------|--------------|
  | cinematic | baseline_A | 29091   | 0.034    | 0.056  | 456.6    | 5.164        | 0.640        |
  | cinematic | after_B    | 29165   | 0.034    | 0.055  | 458.3    | 5.167        | 0.639        |
  | monolith  | baseline_A | 69911   | 0.014    | 0.019  | 140.8    | 4.175        | 0.818        |
  | monolith  | after_B    | 70270   | 0.014    | 0.020  | 140.8    | 4.175        | 0.818        |

  monolith visual metrics identical to the third decimal; cinematic
  inside the documented noise band. Both fixes run at startup /
  live-reload time; the frame path is untouched.

## Files

- `src/config/config_apply.rs` — startup gate uses the canonical
  helper (comment prose trimmed for the LOC cap).
- `src/config/live_config/mod.rs` — live-reload gate uses the canonical
  helper.
- `src/config/live_config/watcher.rs` — WatchSession bundle; the
  8-param signature and its allow deleted; handler module-private.
- `src/testconf/field_validation.rs` — stale "Same logic" comment
  corrected (the gates now agree).
- `test/config/config_apply_tests/v50_beta3_cli_flags.rs` — 2 startup
  gate tests.
- `test/config/live_config/tests.rs` — 3 live-reload gate tests.
- `benchmark/bench-labs/night_hunter23/` — A/B JSON.

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
