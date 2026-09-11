<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-research-13: the thirteen-scene masterclass audit

The owner's verdict after the fourth polish round: the black hole
(sorgonemous_intrascals) is 10/10, locked. The question that
follows: which of the remaining thirteen rain styles needs the
same treatment next. This document is the audit that answers it.
No code changed in this round — this is the map, not the
roadwork; each recommended round follows the black hole protocol
(baseline 10s bench, change, post-commit A/B, a BENCHMARKING.md
record, and visual contract tests in the mirrored tree).

## The standard

Four owner rounds (NIGHT-research-9 through 12) distilled into
seven dimensions:

1. Soft-light cap — no standing Core-white glyph. Core is the
   100% palette + 55% white blend rung (MONOLITH_CORE_WHITE_BLEND,
   the documented eye-strain source); standing surfaces must
   compose to the Hot ceiling (85%, no blend), with Core reserved
   for transient flashes. The mechanism is soft_head_level.
2. Lockstep pace — no per-lane or per-entity speed multipliers;
   one shared law, one simulation clock, matched densities.
3. Concentration — riders dense and on-path: tight entry, small
   wobble, no scattered flying-out reads.
4. Sparse elegance — secondary structures few but substantive.
5. Physics correctness — integrators and motion laws that hold
   up (RK4, closed forms, corotation by construction).
6. Visual test contracts — brightness ladders and draw-site
   composition pinned in the mirrored test tree.
7. Honest A/B records — every visual round benchmarked before and
   after, wins and costs both recorded.

## The lock verification

Before the audit, the black hole lock was verified on f6466ad
(the NR12 code; the A/B doc commit is docs-only): full suite 2800
green, cargo fmt clean, clippy -D warnings clean, gate-keepers
12/12 (yamllint and actionlint skip in the sandbox; CI covers
both). The two-run 10s protocol at 120x40 wet IO: median fps
1954.9 / 1962.2 (NR12 exit 1959.5 — identical), avg dirty cells
per frame 215.8 / 215.3 (exit 215.2), frame entropy 6.09 (6.08),
density gini 0.5420 (0.5454), heap retained 0 B, peak RSS 9.18 /
9.29 MiB, p99 0.644 / 0.600 ms, drift -5.3% / +3.2% both read
stable. Avg fps read 1876.0 / 1892.7 against the exit's 1931.4 —
a 2-3% run-to-run spread on a loaded sandbox; every
machine-independent signature metric (median, dirty, entropy,
gini, heap) is identical. The lock holds. The record lives in
docs/BENCHMARKING.md next to the NR12 A/B entry.

## The scoreboard

Scores are against the black hole's 10/10, not against a generic
quality bar: every scene below is already physics-correct,
LTS-audited, and carries an honest A/B record. The debt is
concentrated in light composition and pinning, not motion.

| Scene | LOC | Tests | Score | The standing offender |
| --- | --- | --- | --- | --- |
| quasar | 2076 | 25 | 6.0 | Core-bright core cell painted every frame, forever |
| dragon | 979 | 13 | 5.5 | Three permanent Core-white heads, 20 s lifetimes |
| vortex | 585 | 10 | 5.0 | Per-mote pace spread; loose scattered arms |
| monolith | 1247 | 32 | 6.5 | Standing Hero Core + 55% white blend, tests defend it |
| physarum | 927 | 15 | 6.0 | The vein network (the signature) burns standing Core |
| aeolian | 1428 | 27 | 7.0 | Drop heads Core for most of every fall |
| dna_helix | 2093 | 42 | 7.5 | Three standing Core sites (drops, fork wake, rung faces) |
| murmuration | 1339 | 27 | 6.5 | Speed band + 1.2 s panic floor hold standing Core |
| neural | 2612 | 16 | 6.5 | 1.8 s burst-wide Core heads + output band steps to Core |
| lorenz | 707 | 10 | 6.5 | Standing Core cluster at each lobe tip |
| solar_flare | 1968 | 22 | 7.0 | Apex glow holds standing Core up to ~3.4 s |
| flux | 1058 | 21 | 7.0 | None (already Hot-capped); the cap is unpinned |
| glyph | 496 | ~49 | 8.0 | Soft by design; one pure-white clamp edge, unpinned |

## Per-scene findings

### The astro family

**Quasar (6.0).** The brightest standing cell in the codebase:
the core glyph reads Core from first light forever (draw.rs,
the core-cell paint, factor 1.0, no pulse dimming), with two more
standing Core sites at the disk inner ring and the jet launch
collar, and step-up rungs that lift Hot to Core. No
soft_head_level anywhere; the ladders are inline in the draw pass
so the current shape cannot even be tested. The fix is the NR11
analog ported whole: cap the core cell, the disk inner ladder,
the jet collar, and the step-up sites at Hot; reserve Core for
the flare window, first light, and knot passage; extract the
ladders into pure functions and pin them.

**Vortex (5.0).** The lowest score in the audit, and the exact
"scattered, flying outward" complaint class: every mote rolls its
own spin (0.85-1.15) and fall (0.80-1.25) multipliers, so arms
smear radially and a shared annulus drifts apart; the arm spread
is +-0.55 rad plus rim jitter, covering roughly 190 degrees of
rim with three arms. The physics label is also wrong: the omega
law is K/r (a flat rotation curve) but is documented and tested
as "Keplerian" (Kepler's third law is r^-1.5, which the quasar
implements correctly). The head cap is de facto — absorption
deactivates motes before the draw pass, so the Core rung in the
ladder is unreachable — but nothing pins that ordering, so a pass
reorder would silently create standing Core. The round: retire
the per-mote multipliers for lockstep (the K/r differential
already provides all the shear read), pin the
heads-never-paint-Core invariant, and fix the label.

**Dragon (5.5).** The most literal offender: level_for_segment
returns Core for the head segment unconditionally, and three
dragons each carry a 20-second lifetime — three permanent
Core-white glyph heads on screen at all times, plus per-dragon
pace multipliers (0.85-1.15, the NIGHT-hunter-10 fix already
linearized the application). The round: re-grade the head rung
Core to Hot, reserve Core for the 1.5 s entry-reveal flare, and
pin the composition. One-line change, mechanically flat by the
NR12 precedent (a level re-grade repaints the same cells).

### The engine family

**Monolith (6.5).** The strain source NR11 audited and fixed for
the black hole, still live here: standing Hero heads read Core
(100% palette + 55% white blend) and the breath/hero pulse adds
up to +0.20 more white on top. Three test sites defend the wrong
direction (core must bloom beyond hot; the Core blend pinned as
intended), so the round needs the same deliberate re-pin dance
the black hole did (64 -> 72 tests). The cap must land at the
monolith draw site, not inside color_for_level — that function is
the shared ladder for lorenz, vortex, dragon, physarum and flux;
an in-function cap would silently re-grade five scenes. The
per-stream speed spread (0.78-1.36) is the widest unexamined pace
divergence in the classic family, but varied cascade speed is
deliberate classic semantics — an owner call, not a defect.

**Neural (6.5).** Pulse heads hold Core for the whole 1.8 s burst
window (about ten times the black hole's whip flash) and the
output band steps a standing Hot to Core; the streamer sway is
0.8 cells (roughly 15x the black hole's converged wobble). The
round: cap the burst-lifted heads and the output step-up at Hot,
and pin the ladder.

**Aeolian (7.0).** Split verdict: the instrument half already
implements the knots-only-Core policy (the exact NR11 rule, in
prose and in code), while the rain half's kinetic ladder sends
drop heads to Core at |vy| > 3.6 with terminal velocity 4.0 —
gravity drives every free fall past the threshold within about
1.5 s, so most of each drop's visible flight stands Core-white.
The round: cap the drop ladder at Hot to match the instrument
half's own policy.

**Flux (7.0).** Already compliant by construction: the speed
ladder tops at Hot and grep confirms zero Core at any draw site —
the only scene in the codebase besides the black hole that can
say this. The gap is contractual: the ceiling is unpinned, so a
regression could reintroduce Core silently. The round is tests
only (the NR11 pure-ladder contract).

### The bio and math family

**DNA helix (7.5).** The strongest scene in the audit, and the
only one whose tree already pins a brightness ladder — but it
pins the wrong contract (charge 2.0 must equal Core). Three
standing Core sites: fresh nucleotide heads read Core for the
first 20% of their lifetime (about 3 s per drop), the fork wake
stamps charge 2.6 that decays through Core for ~1.6 s per written
rung, and the rung front-face steps Hot to Core after every
replication sweep. The round: cap all three sites at Hot (the
fresh-write blink stays Core), re-pin the ladder test.

**Murmuration (6.5).** Two institutionalized standing-Core
sources: the speed band (any bird above 21 speed reads Core, and
a wheeling flock routinely holds that band) and the panic floor
(panicked birds return Core unconditionally for 1.2 s — a mass
standing-Core event every startle). The per-bird jitter (16.0) is
the largest scatter constant in the audit, about 290x the black
hole's halo wobble in comparable terms. The round: cap
speed_level at Hot for both the band and the panic window, keep
Core for the 0.8 s predator flash only, and halve the jitter the
way NR12 halved the wobble.

**Physarum (6.0).** The signature surface is the offender: the
vein network equilibrates above the 0.30 trail threshold and
reads standing Core — the scene's own hero structure, burning
white-blended continuously (the code's own comment documents the
grading at the 60 Hz reference). Per-particle pace multipliers
(0.85-1.15) scale the turn, the move, and the deposit — the exact
class NR11 retired. The round: cap the trail ladder's top rung at
Hot so the veins read warm-gold, and pin the composition.

**Lorenz (6.5).** The best physics and the cleanest concentration
in the audit (RK4 at dt two orders below the stability bound,
canonical parameters compile-time-pinned, zero scatter constants
— the attractor IS the path), with the cheapest fix in the
entire audit: level_for_z returns Core above z=38 and every
lobe-peak excursion lives there, so both butterfly tips hold a
standing Core cluster. One rung, Core to Hot, plus one ladder
test.

### The classic family

**Glyph (8.0).** The head is already the soft kind by
construction: the head cell takes the palette's last stop with no
white blend, the self-bloom is deliberately in-hue (green to
brighter green, never white), and the 15% head halo fades toward
the background. Two gaps: the front-layer self-bloom (0.234 x
1.20) can clamp channels to pure white, and nothing pins the
composition. The round: cap the clamp, pin the head contract,
and give the family its own tests_glyph tree (it is the only
major style without one).

**Solar flare (7.0).** The closest to parity: Core is
flash-gated in three of four paths (the 0.6 s eruption window),
the rain is sparse-and-substantive (8% of columns, every landing
charges the flux economy), and one genuine ladder test exists.
The gap is the apex glow — a heavily-fed loop steps Hot to Core
and holds it for up to ~3.4 s. The round: saturate the apex
step-up at Hot and pin the corona composition.

## Cross-cutting findings

1. soft_head_level exists only in the black hole family. Twelve
   of thirteen scenes have no soft-light ceiling of any kind.
2. Eleven of thirteen scenes paint standing Core somewhere. The
   exceptions: flux (Hot-capped by construction) and glyph (soft
   by design, one clamp edge).
3. The visual-contract gap is the systematic hole: the black
   hole carries roughly twenty brightness and composition pins;
   the other thirteen scenes combined carry two that point the
   wrong direction (dna helix pins Core as the contract;
   monolith pins the Core bloom).
4. Per-entity pace multipliers (the class NR11 retired) survive
   in six scenes: vortex (spin + fall), physarum (turn + move +
   deposit), lorenz (dt), dragon (dt), aeolian (per-drop), and
   neural (pulse and streamer spreads).
5. Physics and A/B honesty are strong everywhere — every scene
   has a correct integrator or closed form, and an honest record.
   The debt is light composition, not motion. The NR13-class
   rounds are level re-grades: mechanically flat, zero
   allocation, the cheapest visual wins in the repo.

## The recommended order

Tier one, the eye-strain parity round (the NR11 analog, biggest
win per line changed): quasar, then dragon, then vortex (vortex
carries the lockstep + concentration round; the other two carry
soft-light caps).

Tier two, the standing-Core cleanup sweep: monolith (with the
three test re-pins), physarum, aeolian, dna_helix, murmuration,
neural, lorenz (lorenz is the cheapest single line in the audit).

Tier three, the pin-and-finish round: solar_flare (apex cap),
flux (pin the existing cap), glyph (clamp + head contract + the
tests_glyph tree).

A shared soft_head_level helper (one function lifted from the
black hole family into a shared site) would let tier one and tier
two ride the same mechanism the black hole already proved — but
each scene's ladder is local, so the ports stay per-scene and the
A/B stays per-round, per the one-task-one-commit protocol.
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
