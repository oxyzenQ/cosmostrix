<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunt-36 A/B report — 10 s benches, release profile

Methodology: baseline binary built from clean `da449a0` (pre-fix HEAD,
git worktree at /tmp/cosmostrix-baseline-h36) → 2 runs per scene →
rebuild with the hunt-36 fix (`c523de9`) → 2 runs per scene. Command:
`TERM=dumb <bin> --benchmark --bench-duration 10 --json --scene
cinematic|monolith`. Cinematic is the glyph rain — the sole
droplet-family style the sweep serves (the style of the owner's stuck
rain glyph report); monolith is the structured-style control.

Structural expectation: benchmark mode disables the sweep
(`enable_stuck_cell_sweep = false`) and the message overlay, so the
bench's steady-state frame path executes none of the changed lines.
The only persistent cost is 8 extra bytes on the Cloud struct (4 x u16
rectangle fields, written once per relayout — a message-set / resize /
border-toggle event, never in the bench loop).

## Cinematic (glyph — the fixed style)

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 28 580 | 28 845 | +0.93 % |
| entropy bits | 5.1708 | 5.1663 | −0.09 % |
| density gini | 0.6377 | 0.6391 | +0.21 % |
| dirty cells/frame | 458.65 | 457.18 | −0.32 % |

## Monolith (structured control)

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 85 883 | 85 694 | −0.22 % |
| entropy bits | 3.2951 | 3.2953 | +0.01 % |
| density gini | 0.8961 | 0.8960 | −0.01 % |
| dirty cells/frame | 56.80 | 56.78 | −0.01 % |

## Conclusion

Performance-neutral and visual-identical within run noise (this sandbox
routinely swings ±1–4 % between identical runs — see HUNT-25's −4 %
outlier note; every delta here is under 1 %). The structural argument
settles it: the bench loop never enters the sweep (disabled in bench
mode) and never lays out a message overlay, so the two binaries share
the same steady-state path; the fix's work (rectangle bounds check +
correct pidx per swept cell) runs once per 600 frames in interactive
mode only. Correctness win: the sweep goes from never-running on
default interactive runs (the fallback message gate) to running with a
per-cell overlay exemption, and from consulting a transposed cell's
phosphor energy to the cell's own slot — pinned by 6 new regression
tests plus 2 rewritten stale tests (suite 2879 green).

Note: the sweep's actual effect is only observable in interactive runs
(msg-mode on). The e2e unit test
`hunt36_end_to_end_orphan_cleared_with_fallback_message` drives the
real rain_at pipeline with the fallback message active and verifies
the orphan glyph is cleared — the PTY-level equivalent of the cbg34
stuck-cell measurements, kept in-process because the change is
frame-buffer-local (no terminal-layer involvement).
<!-- COSMOSTRIX-DISCLAIMER -->
<!-- COSMOSTRIX-VERIFIED-A/B -->
