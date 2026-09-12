<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-34 A/B report — 10 s benches, release profile

Methodology: baseline binary built from clean `371eb8a` (pre-fix HEAD,
stashed as `/tmp/cosmostrix-baseline-cbg34`) → 2 runs per scene →
rebuild with the shadow-honesty fix → 2 runs per scene. Command:
`TERM=dumb ./target/release/cosmostrix --benchmark --bench-duration 10
--json --scene cinematic|monolith`. The change adds one `bool` load +
OR per full-redraw cell (steady state: `force_full_emit = false`, the
HUNT-27 skip branch is unchanged) and re-emits every cell exactly once
per shadow reset (scene switch / restart / resize / live-reload / RIS /
backpressure recovery) — the bench loop's steady-state frames touch
none of the reset paths.

## Cinematic

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 28 516 | 28 436 | −0.28 % |
| entropy bits | 5.17 | 5.17 | +0.11 % |
| density gini | 0.64 | 0.64 | −0.20 % |
| dirty cells/frame | 458.62 | 459.79 | +0.25 % |

## Monolith

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 85 976 | 85 653 | −0.38 % |
| entropy bits | 3.30 | 3.29 | −0.05 % |
| density gini | 0.90 | 0.90 | +0.03 % |
| dirty cells/frame | 56.79 | 56.78 | −0.01 % |

## Conclusion

Performance-neutral and visual-identical within run noise (this sandbox
routinely swings ±1–4 % between identical runs — see HUNT-25's −4 %
outlier note). The structural argument settles it: the bench's
steady-state frame path executes the same skip logic with one extra
false-bool check; the full-emit path only runs on reset events that the
bench loop never triggers. Correctness win: the four owner scenarios
(intro residue, 'x'/'X' scene-switch residue, 'r' restart residue,
color-bg live-reload residue + custom-palette variant) go from
39/195/141/2554/2352 stuck cells to 0/0/≤2/0/0 — verified by
`scripts/night_cbg34_e2e.py` (PTY + mini terminal emulator).
<!-- COSMOSTRIX-DISCLAIMER -->
<!-- COSMOSTRIX-VERIFIED-A/B -->
