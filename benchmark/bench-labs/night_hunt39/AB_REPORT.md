<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunt-39 / NIGHT-hunt-38-supermassive A/B report — 10 s benches, release profile

Methodology (same as hunt-36/hunt-37): baseline binary built from clean
`0641ee9` (parent of the fix, git worktree at
/tmp/cosmostrix-baseline-h39) → 2 runs per scene → rebuild with the fix
(`bb1915d`) → 2 runs per scene. Command: `TERM=dumb <bin> --benchmark
--bench-duration 10 --json --scene cinematic|monolith`. Cinematic is the
glyph rain (the droplet-family control); monolith is the
structured-style control.

Structural expectation: every changed line is config-parse-time (the
parser's separator-typo guards run once per line at load; the msg-mode
validator and the entry-count checks run once per validation; the
ambient essay only renders on rejection). The bench steady-state frame
path executes none of the changed code — the bench loop never
re-parses the config. The chroma pipeline math is untouched (see the
chroma KEY.md UNLOCK entry in `bb1915d`: one validation constant,
COLORS_CUSTOM_MAX_BLOCKS 100 → 64).

## Cinematic (glyph control)

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 28 067.59 | 28 231.56 | +0.58 % |
| entropy bits | 5.1759 | 5.1768 | +0.02 % |
| density gini | 0.6369 | 0.6364 | −0.07 % |
| dirty cells/frame | 460.75 | 461.37 | +0.13 % |
| alloc calls (10 s) | 563 | 563 | +0.00 % |

## Monolith (structured control)

| metric | baseline | after | delta |
|---|---|---|---|
| avg fps | 85 122.58 | 86 177.00 | +1.24 % |
| entropy bits | 3.2955 | 3.2934 | −0.06 % |
| density gini | 0.8961 | 0.8962 | +0.02 % |
| dirty cells/frame | 56.82 | 56.78 | −0.08 % |
| alloc calls (10 s) | 563 | 563 | +0.00 % |

## Conclusion

Performance-neutral and visual-identical within run noise (this
sandbox routinely swings ±1–4 % between identical runs; every visual
delta here is at or under 0.13 % and the two scenes disagree on the
sign, the signature of noise rather than a systematic cost). The
strongest signal is the allocation counter: 563 alloc calls on BOTH
binaries in BOTH scenes — the fix adds zero steady-state allocations,
exactly as the structural argument predicts (validation runs at
config-parse time, never per frame). The correctness win: the owner's
four manual repro classes (`set == "x"` silent accept, `set : "x"`
generic diagnostic, `msg-mode = truee` three-verdict inconsistency,
the stale base-scene ambient essay) are closed uniformly on all three
surfaces, and the four custom namespaces now share one entry budget
(min 1 / max 64) with 65-entry configs hard-rejected instead of
silently truncated.
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
