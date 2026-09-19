<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-cybersecurity-1 A/B report — 10 s benches, release profile

Methodology (same as hunt-36/37): baseline binary built from clean
`f70be1b` (HEAD before the fix) → 2 runs per scene → rebuild with the
cybersecurity-1 fix → 2 runs per scene. Command:
`TERM=dumb <bin> --benchmark --bench-duration 10 --json --scene
cinematic|monolith`. Cinematic is the glyph rain (droplet-family
control); monolith is the structured-style control.

Structural expectation: every changed production line is in the
report/listing family (`--show-scene` / `--list-*` output builders,
their escape_ctrl sink guards) or in comments (posix_time SAFETY
twins). The bench steady-state frame path executes none of the
changed code. The only shared code is `escape_ctrl` itself — a
per-report-call Cow fast path that the frame loop never calls.

## Cinematic (glyph control)

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 28 902.76 | 29 218.29 | +1.09 % |
| entropy bits | 5.1675 | 5.1584 | −0.18 % |
| density gini | 0.6386 | 0.6408 | +0.34 % |
| dirty cells/frame | 456.58 | 453.49 | −0.68 % |

## Monolith (structured control)

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 85 926.51 | 85 347.27 | −0.67 % |
| entropy bits | 3.2965 | 3.2950 | −0.05 % |
| density gini | 0.8958 | 0.8961 | +0.04 % |
| dirty cells/frame | 56.77 | 56.77 | −0.00 % |

## Conclusion

Performance-neutral and visual-identical within run noise (this
sandbox routinely swings ±1–4 % between identical runs; every delta
here is at or under ~1 %, and the two scenes disagree on the fps
sign — the signature of noise, not a systematic cost). The
structural argument settles it: the bench loop never enters the
report family, so the two binaries share the same steady-state path.

The security win, proven end-to-end on this host: a hostile
`scene-custom.evil.rain = "glyph<ESC>[2Jx"` config value echoed a raw
`033` escape byte through `--show-scene` on the baseline binary
(od-verified) and renders as the visible `glyph\u001b[2Jx` literal on
the fixed binary. 4 regression tests pin the sink contracts
(2 display-sink, 2 hidden-block-warning).

<!-- COSMOSTRIX-DISCLAIMER -->
<!-- COSMOSTRIX-VERIFIED-A/B -->
