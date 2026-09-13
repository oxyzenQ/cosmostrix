<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunt-37 A/B report — 10 s benches, release profile

Methodology (same as hunt-36): baseline binary built from clean
`d8dbf1c` (parent of the fix, git worktree at
/tmp/cosmostrix-baseline-h37) → 2 runs per scene → rebuild with the
hunt-37 fix (`6198431`) → 2 runs per scene. Command:
`TERM=dumb <bin> --benchmark --bench-duration 10 --json --scene
cinematic|monolith`. Cinematic is the glyph rain (the droplet-family
control); monolith is the structured-style control.

Structural expectation: every changed line is config-parse-time
validation (strictness checks, header recording, the required-bg load
contract, the shared splitter). The bench steady-state frame path
executes none of the changed code — the only shared code is the
collector's splitter, which runs once per config load, never per
frame. The chroma pipeline math (OKLab gradient, floor, continuity,
halo, shaders) is untouched (see the chroma KEY.md UNLOCK entry in
`6198431`).

## Cinematic (glyph control)

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 28 675.97 | 28 425.23 | −0.87 % |
| entropy bits | 5.1654 | 5.1774 | +0.23 % |
| density gini | 0.6392 | 0.6364 | −0.44 % |
| dirty cells/frame | 457.59 | 461.35 | +0.82 % |

## Monolith (structured control)

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 85 330.50 | 86 209.49 | +1.03 % |
| entropy bits | 3.2916 | 3.2950 | +0.10 % |
| density gini | 0.8963 | 0.8961 | −0.03 % |
| dirty cells/frame | 56.74 | 56.77 | +0.05 % |

## Conclusion

Performance-neutral and visual-identical within run noise (this
sandbox routinely swings ±1–4 % between identical runs — see the
hunt-36 report's note; every delta here is at or under ~1 %, and the
two scenes disagree on the sign, the signature of noise rather than a
systematic cost). The structural argument settles it: the bench loop
never enters the validation layer, so the two binaries share the same
steady-state path bit-for-bit. The correctness win: the owner's
67-stop repro goes from silently running (with the palette silently
truncated at 64 stops) to a hard exit-2 error naming the block, the
count and the ceiling; incomplete blocks (missing bg, missing set,
header-only) and the rain/stops overload are rejected identically on
all three surfaces (startup, --testconf, live-reload watcher).

<!-- COSMOSTRIX-DISCLAIMER -->
<!-- COSMOSTRIX-VERIFIED-A/B -->
