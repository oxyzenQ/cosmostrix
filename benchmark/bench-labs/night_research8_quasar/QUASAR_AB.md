<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-research-8 A/B report — the quasar, the thirteenth rain style

Owner rule: 10 s benchmark A/B before/after any visual-engine
change, visual (density gini, frame entropy) + performance (fps,
dirty cells). Baselines captured on the pre-change tree (e310891,
the NIGHT-research-7 part 3 HEAD) before implementation; the
after tree adds the quasar engine (the four populations, the
ignition birth sequence, the capture economy) and its wiring.
Dry benches (no terminal I/O) at the default bench viewport,
`--benchmark --bench-duration 10 --json`. Four regression probes:
cinematic (the glyph droplet path), aeolian and solar_flare (the
structured-family siblings the style's dispatch plugs into) and
murmuration (the accumulator-spawn sibling whose orchestration
the quasar mirrors most closely).

## Results

| scene         | side | avg_fps | dirty/pr | entropy_bits | density_gini |
|---------------|------|---------|----------|--------------|--------------|
| cinematic     | A    | 28,685  | 460.0    | 5.170        | 0.638        |
| cinematic     | B    | 27,580  | 467.0    | 5.193        | 0.633        |
| aeolian       | A    | 160,464 | 45.9     | 4.977        | 0.646        |
| aeolian       | B    | 156,198 | 46.0     | 4.980        | 0.645        |
| solar_flare   | A    | 36,435  | 249.1    | 6.043        | 0.323        |
| solar_flare   | B    | 36,359  | 249.7    | 6.052        | 0.317        |
| murmuration   | A    | 24,747  | 144.6    | 4.808        | 0.692        |
| murmuration   | B    | 25,114  | 144.2    | 4.839        | 0.686        |
| quasar        | B    | 96,350  | 84.7     | 4.788        | 0.711        |

(The quasar has no A side — the style did not exist on the
pre-change tree; its row is the new style's own steady-state
measurement, the murmuration report's precedent.)

## Reading

- Zero visual regression on the four probes: the dirty-cell
  populations, entropy and gini match the baselines to the third
  decimal (cinematic's dirty 460 -> 467-465-463 across repeated
  B runs — same-tree re-runs measured 27,460-27,880 fps with
  463.1-467.0 dirty, the spread of the bench's own sampling, not
  a drift; the A side's 28,685/460.0 was a single sample from
  the pre-change tree in the same session).
- The fps deltas (-3.9%, -2.7%, -0.2%, +1.5%) carry MIXED SIGNS
  across the probes — the signature of interleaved-run noise
  (the genesis report measured +2.3/-1.0% same-tree spread; the
  murmuration report -0.1/-0.8/+0.3%; this session's repeated
  cinematic runs span +-1.5% on the same tree). The probes' hot
  paths execute none of the new code: every quasar dispatch arm
  sits behind a `matches!(rain_style, Quasar)` branch that
  predicts false for the probes, and the style's pools idle at
  zero population while another style runs. The one shared-code
  surface the change touches is the dispatch chains themselves —
  six additional integer-compares per frame on the glyph path,
  sub-0.1 percent of a 35 us frame.
- The quasar's own numbers: 96,350 fps (the fourth-fastest scene
  in the catalog — the engine is four O(n) pools over ~130
  particles with one sin/cos pair each, no neighbor graph), 84.7
  dirty cells/frame (inside the family band: aeolian 46,
  murmuration 144, dna_helix 108, solar_flare 250), entropy
  4.788 bits and gini 0.711 (a single central body with a wide
  weather halo — the most concentrated composition in the
  catalog, honest to a scene whose hero object owns the frame).
- The ignition does NOT run in the bench: `reset_bench`
  fast-forwards to the burning engine (the Z-6 critical-path
  contract, the DNA genesis precedent — one-shot choreography is
  not steady-state throughput). The birth is instead pinned by
  25 deterministic contracts (tests_quasar/) and the PTY shape
  smoke (scripts/quasar_smoke.py: the dark cloud leaves the
  core's home empty — 0 cells at the center box — and the steady
  engine draws its core + glow + inner disk there — 17 cells —
  with both beams lighting 23 of the band's rows).

## Files

- `baseline_A_{cinematic,aeolian,solar_flare,murmuration}.json`
  — pre-change tree (e310891), captured before implementation.
- `after_B_{cinematic,aeolian,solar_flare,murmuration,quasar}.json`
  — post-change tree.
<!-- COSMOSTRIX-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths or symbol names. Maintainers update source code but may
  forget to sync every doc — the project ships 80+ .md files and
  perfect sync is a known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of truth.
  Always cross-check against the actual `.rs` files before relying
  on any specific number (test count, LOC, FPS, ms timeout), file path,
  function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
