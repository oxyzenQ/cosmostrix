<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-lts-3 — the master restart-consistency audit: 'r' restarts from zero like a fresh launch, across every type rain

Owner directive (2026-09-10): "master depth audit for all type rain,
for peak optimize code, and cinematic LTS. the first problems owner
found inconsistency restart function on type rain blackhole when
click shortkey 'r' the restarted blackhole is not real start from
zero very different from startup. owner want consistency same
behaviour startup and restart. so make sure agent depth audit for
all type rain not just blackhole. also for consistency all type
rain should support dynamic screen size."

## Root cause (the owner's black hole repro)

The 'r' shortkey handler (interactive/input.rs) ran
`cloud.reset(w, h)` — the resize-semantics reset. That path
(`reset_with_bounds`) rebuilds geometry and empties every pool, but
it deliberately preserves each choreographed family's birth state:
the black hole's `formed` flag stays true, so the restarted hole
pops in already formed instead of replaying the formation intro
(seed dot -> collapse flare -> horizon bloom -> accretion) that a
fresh startup plays — `BlackHoleRain::new()` constructs unborn, so
the first launch always runs the sequence. The same divergence
silently affected the three other choreographed families: the DNA
molecule stood already transcribed, the quasar engine burned
already lit, the neural machine ran already trained.

## The fix

`Cloud::restart_from_zero(cols, lines)` (cloud/spawn_reset.rs) — the
'r' handler now restarts as a relaunch, layering on the full reset:

1. **The RNG stream re-seeds** from `RNG_INITIAL_SEED` BEFORE the
   reset (so the reset's glitch-clock draw consumes the same stream
   position a launch's reset does). Every launch draws from the
   same deterministic stream; every restart now re-enters it —
   "from zero" in every observable sense (the same seed family; the
   exact stream position differs only by construction-time draws,
   which is invisible).
2. **The full reset** (geometry, pools, maps, LUTs, message,
   semantic invalidation, force redraw, subsystem clocks).
3. **The fresh-start state** `reset` deliberately preserves across
   resize/live-reload (the Phase D Bugs #8/#9 contract — an
   interrupt must not snap the visual climate): the time anchor
   re-captures, the ecosystem/drift accumulators reconstruct at
   their unevolved defaults, and the event scheduler's dedicated
   RNG re-seeds (`GhostEventScheduler::restart_rng` — `reset` only
   dropped the active events, so a restarted session would have
   drawn a different ghost-event sequence than a launch). A restart
   is a relaunch, not an interrupt, so from zero means from the
   unevolved state.
4. **The pause/resume easing clears** (unpaused, full rate) — a
   restart mid-pause or mid-resume-ramp no longer carries the
   throttle.
5. **The birth choreography re-arms** for the current style, the
   same contract a scene entry honors: black hole `begin_formation`,
   DNA `begin_genesis`, quasar `begin_ignition`, neural
   `begin_genesis`. The plain structured families need nothing
   beyond the full reset (pools vacant — identical to both startup
   and scene entry), and the glyph family matches a fresh launch by
   design: the empty pool fills through natural spawn, and the
   warm-start ramp belongs to style transitions, not launches.

The 'r' handler becomes `restart_from_zero` + the message
typewriter replay. Help text, README and RULES.md keybind tables
synced ("restart from zero", not "reset animation").

## The master audit table (all fourteen type rains)

| family | restart BEFORE ('r' = reset) | restart AFTER ('r' = restart_from_zero) | dynamic screen size |
|---|---|---|---|
| glyph (cinematic) | residue-free since HUNT-15; pool refills, no ramp — already the startup shape | + RNG/drift/anchor/pause fresh-start | resize rebuilds pool + column tables; draw clamps (Fraction LUTs) |
| monolith | pools vacant, geometry rebuilt — already the startup state | + fresh-start state | lanes from cols; vertical via draw-time clamps |
| vortex | same | same | motes per column; polar geometry fraction-based |
| flux | same | same | field + pools reset with both dims |
| lorenz | same | same | motes per column; attractor scales with viewport |
| dragon | same | same | pool reset; geometry fraction-based |
| physarum | same | same | particles per column; trail field re-allocates lazily with both dims |
| black hole | **formed flag survives — the hole pops in formed (the owner's repro)** | **formation replayed from the seed dot** | resize keeps steady state by documented contract |
| aeolian | pools vacant — startup state | + fresh-start state | strings/drops reset with both dims |
| solar flare | arcade wiped — startup state | same | loops/flux re-anchored by reset |
| dna helix | **formed flag survives — the molecule stands pre-born** | **genesis replayed from the soup** | rung count rebuilt from height; phase kept |
| murmuration | flock vacant — startup state | same | boid grid + pool reset with both dims |
| quasar | **lit flag survives — the engine burns pre-lit** | **ignition replayed from the cold cloud** | pools/disk rebuilt; steady state kept on resize |
| neural | **lit flag survives — the machine runs pre-trained** | **genesis replayed from the falling data** | network reset with both dims; steady state kept on resize |

The resize column: the pending-resize apply (event_loop) routes
every style through `reset` — all thirteen structured families take
their reset with the new dimensions (cols-only families rebuild
lane pools; lines enters through the step/draw structs' fraction
geometry). The dynamic-size verdict: supported everywhere, verified
mechanically by the new resize test (below). The resize path keeps
the steady state for the choreographed families by design — the
distinction between resize (an interrupt) and restart (a relaunch)
is now explicit and test-pinned on both sides.

## Tests (tests_restart_lts3.rs, 9 new + hunt15 extended)

- `restart_replays_the_black_hole_formation` — the owner's exact
  repro: formed hole + 'r' -> formation re-armed, clock at zero, the
  replay begins with the seed dot at the viewport center.
- `restart_replays_the_dna_genesis` / `restart_replays_the_quasar_ignition`
  / `restart_replays_the_neural_genesis` — formed/lit flag flips
  back, clock at zero.
- `restart_reseeds_the_rng_stream_from_zero` — three properties:
  the rewind happens (not the burned continuation), every restart
  rewinds to the same deterministic zero ('r' twice replays
  identical draws), and the post-restart draws live inside the
  RNG_INITIAL_SEED launch stream.
- `restart_clears_pause_and_recaptures_the_anchor` — mid-pause 'r'
  comes back unpaused at full rate; the anchor re-captures.
- `restart_cinematic_glyph_refills_like_a_fresh_launch` — the
  default scene: residue-free restart, natural refill, no
  transition ramp.
- `resize_up_and_down_keeps_every_style_live` — all fourteen styles:
  resize up (60x25 -> 120x45) and down (-> 48x16), live in-bounds
  content, no panic, dims tracked.
- `resize_keeps_the_steady_state_distinct_from_restart` — the
  documented resize contract pinned on the black hole, and 'r'
  still re-arms after a resize.
- `hunt15_restart_clears_old_rain_for_every_style` extended from
  seven to all fourteen styles (the seven families that joined
  after HUNT-15: black hole, aeolian, solar flare, dna helix,
  murmuration, quasar, neural).

## Performance

Zero hot-path code changed: `restart_from_zero` and
`restart_rng` are cold (the 'r' keybind and nothing else calls
them); `reset_with_bounds`, `rain_at` and every draw/advance loop
are untouched. The bench harness drives `--scene` + `--benchmark`
and never presses 'r', so an A/B would measure pure construction
noise — skipped per the task rules' waste guard (the hunter-18
precedent for keybind-path changes).

## Peak verdict

The audit found no further restart or resize inconsistency beyond
the fixed class: every family's reset already returns it to the
startup state (the HUNT-15 frame-clearance contract plus the
per-family pool/geometry resets), the four birth-choreography
families now replay their intros, and the fresh-start state (RNG,
drift, anchor, pause, event RNG) is pinned by tests. All fourteen
families carry both contracts — restart == startup, resize ==
steady-state re-anchor.
<!-- COSMOSTRIX-DISCLAIMER -->
