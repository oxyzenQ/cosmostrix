<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-22 (F2) — the duration dual-field, deleted; `duration_s` is the single source of truth

Owner approval 2026-09-09: F2 was the last item of the approved batch
(post-exit printer value-structs, F1 the 9x config parse, F2 this
document).

## The wart, as cataloged by the hunter-3 audit

`CloudConfig` carried TWO same-typed duration fields one line apart:

    pub duration: Option<f64>,    // raw args.duration
    pub duration_s: Option<f64>,  // validated args.duration

Both were written in `build_cloud_cfg` from the SAME source
(`args.duration` — the raw copy verbatim, the `_s` copy through
main.rs's validation gate: finite check, 0.1..=86400 range when
positive, `0` passed through as the run-forever sentinel). The values
coincided by construction, which made the redundancy "safe" — until
the one consumer that read BOTH:

    // event_loop.rs (pre-fix) — the cross-wire hazard
    let end_time = cfg.duration_s.and_then(|s| {
        if !s.is_finite() || s <= 0.0 { return None; }
        let s = cfg.duration.unwrap_or(s);   // <- reads the OTHER field
        Some(start_time + Duration::from_secs_f64(s))
    });

`duration_s` acted as the trigger (is auto-exit on?) and
`cfg.duration` then OVERRODE the value via `unwrap_or`. The audit's
verdict: dead defensiveness today (the fields always coincide), but
exactly the shape that becomes a bug the day one field gains another
writer. Prescribed fix: delete the raw field, keep the validated one.

## The fix

- `CloudConfig.duration` (the raw twin) is DELETED; `duration_s` —
  the validated value — is the single duration source of truth,
  documented at the field.
- `build_cloud_cfg` stuffs only `duration_s` (main.rs validated it
  before the struct was ever built; the raw copy carried no extra
  information).
- The end_time derivation collapses to a single-source read:
  `cfg.duration_s.filter(finite && > 0).map(start + dur)` — the
  `unwrap_or` cross-read is gone, along with its and_then scaffolding.
- 12 CloudConfig literal sites updated (1 source fixture +
  11 test fixtures, `duration: None` twin lines removed).

Consumers that were already reading OTHER sources stay untouched:
verbose startup / `--testconf` / bench guard read `args.duration`
(the Cli struct) directly — they never used the CloudConfig twin.

## Verification

- **Full suite: 2546 passed, 0 failed** (+3 new source-text contract
  tests pinning the deletion so the twin cannot silently return:
  CloudConfig declares exactly one duration field; build_cloud_cfg
  threads only the validated one; the event loop never cross-reads a
  second duration field).
- **Behavioral proof (PTY, real debug binary, timed):** `--duration
  0.6` exits cleanly at ~630 ms wall (deadline fired); the no-duration
  control runs until the external kill — `None` keeps run-forever
  behavior, and the `0` sentinel is dropped by the same `> 0.0`
  filter as before.
- **clippy `-D warnings` (all targets): clean.** **Gatekeepers:
  10/10.** **LOC guard: green** (all touched src files under the
  800 cap; the test tree is out of the guard's scope by design).
- **A/B 10 s benches (pro, dry, cinematic + monolith):**

  | scene     | side       | avg_fps | frame_ms | p99_ms | dirty/fr | entropy_bits | density_gini |
  |-----------|------------|---------|----------|--------|----------|--------------|--------------|
  | cinematic | baseline_A | 29038   | 0.034    | 0.053  | 456.3    | 5.163        | 0.640        |
  | cinematic | after_B    | 29044   | 0.034    | 0.056  | 456.6    | 5.167        | 0.639        |
  | monolith  | baseline_A | 69377   | 0.014    | 0.020  | 140.8    | 4.175        | 0.817        |
  | monolith  | after_B    | 69510   | 0.014    | 0.019  | 140.8    | 4.175        | 0.817        |

  monolith visual metrics identical to the third decimal; cinematic
  inside the documented shared-VM noise band. The deletion touches a
  pre-loop computation (end_time is derived once before the first
  frame) and a struct field the frame path never read. Full JSON:
  `benchmark/bench-labs/night_hunter22_f2/`.

## Files

- `src/cli/app.rs` — twin field deleted, `duration_s` documented as
  the single source; `clone_config` copies one field.
- `src/cli/build_cloud_cfg.rs` — construction threads only
  `duration_s`.
- `src/interactive/event_loop.rs` — end_time single-source
  derivation.
- `src/interactive/event_loop_scene_sync.rs` — fixture twin removed.
- `test/interactive/tests.rs` — +3 source-text contract tests
  (`nh22_f2_duration_single_source`).
- 10 sibling test fixtures — `duration: None` twin lines removed.
- `benchmark/bench-labs/night_hunter22_f2/` — A/B JSON + report.

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
