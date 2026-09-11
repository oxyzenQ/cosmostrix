# Benchmarking Guide
<!-- SPDX-License-Identifier: GPL-3.0-only -->

> Independent guide to benchmarking cosmostrix: how to run, interpret, compare, and trust the numbers. For exhaustive metric definitions, see `--help` and the `--benchmark` output itself.

## Quick Start

```bash
# Default 5s benchmark (dry, no I/O — pure engine throughput)
target/x86_64-unknown-linux-gnu/pro-linux-v3/cosmostrix --benchmark

# 10s benchmark with wet I/O (writes ANSI to /dev/null)
cosmostrix --benchmark --bench-io --bench-duration 10s

# Measure the production render path (what the terminal actually sees)
cosmostrix --benchmark --bench-io --bench-scene production-draw --bench-duration 10s

# JSON output for CI/scripts
cosmostrix --benchmark --json | jq .performance.avg_fps
```

The default benchmark runs **dry** (no I/O) — it measures pure engine throughput, not how many frames the terminal *draws*. Real interactive FPS is bounded by the terminal emulator, refresh rate, and ANSI output bandwidth. Use `i` (live HUD, lowercase only — uppercase `I` is a no-op) during a real run to see actual interactive FPS.

## Benchmark Modes

| Flag | What it does | When to use |
|------|-------------|-------------|
| `--benchmark` | Premium 5s benchmark (2s warmup + 3s measurement). Prints FPS, frame-time percentiles, dirty-cell coverage, throughput, MEMORY (RSS), CPU %, component timing, DRIFT. | Default user-facing benchmark |
| `--bench-frames N` | Legacy CI benchmark. Runs N headless frames, prints compact `BENCH:` output. | CI pipelines, frame-count-based measurement |
| `--bench-duration N` | Override duration (1s minimum, 24h maximum — the hard OS-protection ceiling, S-master-HUNT-5). Accepts `30s`, `5m`, `1h30m`, `0.5d`. | Endurance testing, drift/leak detection |
| `--bench-io` | Wet I/O — writes ANSI to `/dev/null`. Exercises kernel syscall path. | Measure real write bandwidth + latency |
| `--bench-scene NAME` | Select render path (`lean` or `production-draw`). Requires `--bench-io`. | Measure specific render path |
| `--bench-all` | Scaling sweep across 6×6 -> 200×60. Prints SCALING SUMMARY table. | See how FPS scales with screen size |
| `--screen-size WxH` | Fixed virtual screen size. Min 4×4, max 7680×4320 in bench mode. Memory scales with cells: the double frame buffer + generation maps cost roughly 48-72 bytes per cell, so the 8K cap allocates ~1.0-1.5 GiB heap — size down on memory-constrained machines (a 500×200 run needs ~12 MiB). | Benchmark at exact dimensions |
| `--save-baseline PATH` | Save JSON output to whitelist-enforced path. | Lock in regression baseline |
| `--compare-baseline PATH` | Compare current run against saved baseline. Flags >5% FPS regressions. | CI regression detection |
| `--json` | Machine-readable JSON output. | Scripts, CI, dashboards |

## `--bench-scene` Strict Validation + Reading the Report

`--bench-scene` is **strict** — only two values accepted, typos are rejected (not silently fallback'd). This is part of the cosmostrix honesty contract: no hidden flags, no hidden behavior.

| Value | What it measures |
|-------|------------------|
| `lean` (default) | The `emit_cell_lean` path — per-dirty-cell SGR emission. Fastest path cosmostrix uses in interactive mode. |
| `production-draw` | The full `Terminal::draw` redraw path — `MoveTo` per row + `ColorCache` SGR + BOLT bold escape. Mirrors what the terminal actually receives during interactive rendering. |

Two-layer validation: parse-time (clap `value_parser`, rejects invalid values with "did you mean?" tip) + runtime (`validate_bench_scene`, called at the top of all 3 benchmark entry points). `production-draw` requires `--bench-io` (the production draw path routes through `BenchIoWriter`).

The `--benchmark` report is organized into sections: `BENCHMARK ENVIRONMENT` (system info, git SHA, Rust version, profile), `RENDERER` (engine config, `gpu_usage: not_applicable`), `CONFIG` (CLI flags + config file), `PERFORMANCE` (FPS, frame-time percentiles, jitter, stability), `MEMORY` (RSS), `CPU` (process CPU %), `COMPONENT TIMING` (per-subsystem frame budget), `DRIFT` (first-half vs second-half FPS delta), `RESOURCE` (energy/power on Linux), `THROUGHPUT` (glyphs/sec, ANSI bytes/sec).

## What Runs in Benchmark Mode (Critical Path Only)

Benchmark mode measures the **critical path only**: the rain simulation plus the three dragon engines (cosmic render engine, chroma color engine, crystal climate drift). Every cosmetic and protective system is out of the measurement:

| Skipped in bench | Why |
|------------------|-----|
| HUD, intro, message overlay, terminal interaction | Bench paths return before the interactive loop starts |
| Ghost events (cinematic event engine) | Opt-in; only the interactive loop enables them |
| Anomaly zones, border-cross cosmetics, CRT vignette | Pure visual cosmetics — gated on `!bench_mode && effects_enabled` (v80.0.0-alpha.1: also off under `--no-effects`) |
| Emergent storytelling moments | Cinematic density/luminance/speed perturbation — gated on `!bench_mode && effects_enabled` |
| Cursor hover glow | Cosmetic; bench has no mouse events (mouse_col sentinel) — and off under `--no-effects` |
| Crystal dragon palette drift | Forced off: palette rebuilds inject p99/max timing spikes (deterministic climate drift still runs) |
| Idle FPS throttle, self-healer, perf_pressure clamps, madvise, xterm.js cap | Interactive-only power management — never engages in bench paths |

The `cosmetics_skipped` CONFIG line lists the gated set, and `power_dragon` / `crystal_dragon` / `msg_mode` / `no_effects` disclose the effective config state (all four also appear in `--json` output). None of these keys change benchmark numbers — they exist so you can verify your config took effect.

### Which color pipeline the benchmark measures (chroma dragon audit)

The Chroma Dragon engine is **active during `--benchmark`** whenever the resolved color mode is truecolor: the benchmark renders every cell through the same `is_chroma()` branches the interactive loop uses (OKLab palette, climate post-FX, halos, perceptual blend). Only Crystal Dragon *palette drift* is forced off for p99 determinism — the engine itself never is. The report's CONFIG block answers the question without reading source:

- `color_pipeline: chroma_dragon` — the Chroma Dragon engine colored this run.
- `color_pipeline: legacy_rgb` — the terminal could not represent truecolor, so the legacy sRGB-linear path was measured (auto fallback: tty consoles, 256-color-only terminals, unknown/unset TERM).
- `chroma_in_benchmark: chroma enabled (...)` — plain-text status line for the same fact.

Truecolor resolves from `COLORTERM=truecolor/24bit`, `TERM` carrying `-direct`/`-truecolor`, or `TERM` naming a truecolor-native terminal (alacritty, xterm-kitty, xterm-ghostty, wezterm, foot, contour — NIGHT-research-1; covers SSH/`sudo` sessions where `COLORTERM` was stripped). Run `cosmostrix --doctor` to see which pipeline your environment resolves; `--color-mode 24` forces truecolor for A/B comparisons of the two pipelines on the same machine.

**task-17 emission note:** the benchmark I/O writer mirrors the
production emission boundary. A `--color-mode 16` run writes classic
`3x`/`9x` codes and a `--color-mode 256` run writes `38;5;N` indexed
sequences (OKLab-nearest), exactly like the interactive renderer — a
pre-task-17 comparison of 16/256 runs against truecolor runs is
therefore not byte-comparable. Visual metrics stay in-family across
modes (monolith 10s: entropy 4.840/4.838/4.838 for 16/256/24; Color16
color-transition delta runs higher because 16 discrete colors jump
farther). The dry-run `ansi_bytes_per_second` remains the disclosed
19-bytes/cell truecolor-based estimate (see its `basis` field); real
bytes per frame are measured by the wet I/O scenes.

**task-18/19 + NIGHT-research-4/5/6 style signatures:** the five
structured flagship styles benchmark like this (10s, 120x40,
truecolor, 5s discarded warmup): `vortex` sits in the
structured-performance class with monolith (42K fps, ~278 dirty
cells/frame, sim 0.013ms) while carrying the highest frame entropy
of any style (6.31) and the most even density (gini 0.468); `flux`
(task-19, supersedes the rejected ripple style) sits in the
structured class at ~32K fps with the LOWEST dirty-cell count of
any style (~102/frame — fluid particles move coherently with the
flow, so the diff engine barely works), entropy 5.73 and gini
0.627, drift under 0.4%; `lorenz` (NIGHT-research-4) runs at ~52.3K
fps / ~138 dirty / entropy 5.87 / gini 0.597; `cosmic_dragon`
(NIGHT-research-5) is the FASTEST style at ~126.8K fps with the
fewest dirty cells (~75/frame — only three serpentine chains are
on screen), entropy 5.15 and the most concentrated density of the
structured flagships (gini 0.731); `physarum` (NIGHT-research-6)
runs at ~63.3K fps with ~100 dirty cells/frame, entropy 5.74 and
gini 0.624 — its (entropy, gini) point lands near flux (emergent
networks spread particles across the viewport much like a fluid)
while its motion signature remains 100% distinct. The PIC/FLIP
solver itself costs ~0.006ms/frame (fixed 60Hz stepping, one step
per bench frame). Per-style benchmarks are directly comparable via
`--scene <name> --benchmark`. The NIGHT-lts-1 master depth audit
(2026-09-10, stages 1-7) re-benched all fourteen styles at the
default 80x24 profile with per-stage A/B evidence — see
`docs/research/NIGHT_LTS_1_STAGE*.md` for those numbers, the
noise-yardstick methodology, and the audit's codegen controls
(the numbers in this section above are the historical 120x40
truecolor signatures and remain valid for that profile).

**NIGHT-research-11 (2026-09-11, the black hole soft-light cap and
all-lanes consistency):** the soft-head cap (one extra ladder step
per glyph head — a pure level re-grade), the halo pool doubling
(2 riders per lane per column, one rider per lane per two columns:
roughly 2x the halo population — more RK4 integrations, projections
and draw cells per frame), the lockstep pace (the lanes advance at
the ring's tier-0 mean motion — same per-frame work, different
phase), and the round-robin tag split (a simpler spawn branch). 10s
A/B at 120x40 wet IO vs the parent commit (before 88ae1d7, after
136ca38, dev profile, same two-run discard-warmup protocol):
sorgonemous_intrascals avg fps 2320.0 -> 1955.8 (-15.7%,
confirmation run 1960.6 — call it -15.5%), median 2356.3 ->
1979.9, p99 frame time 0.519 -> 0.594 ms, p99.9 0.782 -> 0.759 ms,
avg dirty cells/frame 181.3 -> 214.7 (+18.4%, dirty ratio 3.78% ->
4.47%), frame time stability excellent on both sides (drift -0.37%
vs -0.11%), allocator flat (heap retained 0 B, peak RSS 8.97 ->
8.96 MiB — the doubled pool rides in the same allocation budget,
the Vec grows at reset). The cost is mechanical and owner-directed:
the density ask doubles the stream population, so the advance pass
integrates ~2x the riders and the draw pass paints ~2x the lane
cells (the +34 dirty cells/frame); the soft-head cap and the
lockstep re-branch are level/branch changes with no measurable
cost. At 1956 fps the scene still sits far above any terminal's
display rate — the headroom is the honest price of the
all-lanes-consistent read. Visual signature shifts track the
intent: entropy 6.01 -> 6.05 and density gini 0.5655 -> 0.5544
(the denser, evenly split lanes spread the composition wider and
more evenly across the viewport — the consistent-rings read), color
transition delta 0.00 -> 0.00 (the bench palette holds steady).

**NIGHT-research-12 (2026-09-11, the black hole sparse lower echo,
soft ball, rise hold, and arc concentration):** the crown-dominant
lane re-split (the same halo pool, the rider population moved
between lanes — 0.28 per crown, 0.08 per lower arc, zero new
motes), the soft ball (the annulus band re-grading plus the
soft-head cap after the Doppler lobe — pure level changes), the
rise hold (a distance clamp on the far-side ladder input — one
min() per tier-0 far-side head), and the concentration pair (the
halo-specific tight entry scale plus the halved wobble — the same
per-rider math, smaller amplitudes). 10s A/B at 120x40 wet IO vs
the parent commit (before 9fa6f56, after 7646c6e, dev profile,
same two-run discard-warmup protocol): sorgonemous_intrascals avg
fps 1957.4 -> 1931.4 (-1.3%, confirmation run 1941.3 — call it
about -1%, within the run-to-run spread), median 1980.3 -> 1959.5,
p99 frame time 0.588 -> 0.623 ms (confirmation 0.586 — flat), avg
dirty cells/frame 214.2 -> 215.2 (+0.5%), dirty ratio 4.46% ->
4.48%, allocator flat (heap retained 0 B, peak RSS 8.94 -> 9.32 /
9.02 MiB). The round is mechanically honest-flat: the density
re-cut moves riders between lanes without changing totals, the
brightness changes are level re-grades, and the tighter geometry
trims a few off-arc wanderer cells while the denser crowns add a
few — the dirty count nets out. Visual signature shifts track the
intent: entropy 6.04 -> 6.08 and density gini 0.5570 -> 0.5454
(the crowns concentrate onto their tight arcs while the lower echo
thins — the composition reads as a dense crown family over a rare
mirrored echo, slightly more even overall).

**NIGHT-research-25 (2026-09-11, the flux cap pin round):** the
audit's honest no-op — the flux style was already Hot-capped by
construction (the speed ladder reads Ghost/Mid/Hot with no Core
rung; the comet trail only descends), the only scene of the
thirteen needing no brightness change; the round pins the cap so
the guarantee lives in contract, not accident (step_down_level
promoted to the ladder surface, one new ceiling contract sweeping
any speed, direction, and magnitude). The pin touches no runtime
path (visibility only). 10s A/B at 120x40 wet IO (dev profile,
two-run discard-warmup): flux avg fps 3623.3 -> 3636.4 (+0.4%,
within the run-to-run spread), median 3688.1 -> 3689.0, p99
0.3368 -> 0.3252 ms, avg dirty cells/frame 16.0 -> 16.1, entropy
3.70 -> 3.71, gini 0.8937 -> 0.8931, heap retained 0 B, peak RSS
9.05 -> 9.04 MiB — mechanically identical, as a pin-only round
should read.

**NIGHT-research-24 (2026-09-11, the solar flare apex soft-light
round):** the tier-three pin-and-finish opener — the apex
condensation glow's step-up saturated at Hot (the black hole's
NR11 cap precedent). The audit's finding: the corona was
flash-gated in three of four Core paths (the 0.6 s eruption
window, the ladder's fresh-flare rung, the footpoint landing
punch), but the apex glow was flux-gated alone — a heavily-fed
loop stepped Hot to Core and held it through the whole cooling
plateau (the 0.38/s decay from FLUX_MAX crosses HOT at about
3.4 s). The glow now lifts only the lower rungs (the new
loops::apex_step_level), a Core base passes through untouched
(the flash windows keep their Core), and the whole pass-B
decision extracted into the pure loops::arc_cell_level — no RNG
change, no allocation, same pools. 10s A/B at 120x40 wet IO (dev
profile, two-run discard-warmup): solar_flare avg fps 2304.6 ->
2327.0 (+0.9%, confirmation run 2325.1 — flat), median 2330.6 ->
2358.7, p99 0.5261 -> 0.5364 ms, avg dirty cells/frame 116.5 ->
114.2 (-2.0% — the softer apex composition changes fewer drawn
contents through the frame-equality fast path), entropy 5.93 ->
5.90, gini 0.5705 -> 0.5796, heap retained 0 B, peak RSS 8.92 ->
8.94 MiB, drift +0.39% -> +0.09% (stable).

**NIGHT-research-23 (2026-09-11, the lorenz soft-light round):**
one rung, Core to Hot — the cheapest fix in the audit's entire
sweep: level_for_z sent every lobe-peak excursion (the peaks
cluster near z=40) to Core, holding a standing Core cluster at
both butterfly tips; the peaks now read the warm Hot ceiling,
the depth read survives (Hot wings, Mid transition, Ghost saddle
bridge). LORENZ_Z_HOT stays live for the black hole ring's
locked composition. No RNG change, no allocation. 10s A/B at
120x40 wet IO (dev profile, two-run discard-warmup): lorenz avg
fps 8328.9 -> 8731.0 (+4.8%), median 8565.7 -> 8843.9, p99
0.1526 -> 0.1467 ms, avg dirty cells/frame 56.9 -> 53.7 (-5.6%
— the peaks' softer composition changes fewer drawn contents
through the frame-equality fast path), entropy 5.19 -> 5.15,
gini 0.7331 -> 0.7386, heap retained 0 B, peak RSS 9.05 -> 8.95
MiB, drift improving (-2.00% -> -0.58%).

**NIGHT-research-22 (2026-09-11, the neural soft-light round):**
the burst-lifted pulse heads cap at the warm Hot ceiling (the
retired flaring branch held Core on every riding signal for the
whole 1.8 s burst window — about ten times the black hole's whip
flash; the pulse-head ladder extracted into the pure
pulse_head_level for the pin) and the output band's step-up
stops at Hot (a standing Hot answer node holds; a fired flash
keeps its white through the step). No RNG change, no allocation.
10s A/B at 120x40 wet IO (dev profile, two-run discard-warmup):
neural avg fps 13718.1 -> 13412.9 (-2.2%), median 13421.0 ->
13529.2, p99 0.1179 -> 0.1006 ms (-14.7%), avg dirty cells/frame
26.4 -> 30.7 (+16% — an honest mechanical cost: the softer ladder
walks more rungs as the fired flashes decay, the retired Core
saturation collapsed several rungs into one, so more decaying
cells cross level boundaries per frame), entropy 4.42 -> 4.57,
gini 0.8266 -> 0.8149 (the busier decay transitions spread the
dirt more evenly), heap retained 0 B, peak RSS 9.02 -> 8.98 MiB,
drift improving (-6.23% -> +1.84%). The NIGHT-research-10 class:
a visual ask with a measurable mechanical price, recorded
honestly — p99 improved and the machine still clears 13K fps.

**NIGHT-research-21 (2026-09-11, the murmuration soft-light round):**
the two institutionalized standing-Core sources cap at the warm
Hot ceiling (the retired 21-plus speed band a wheeling flock
routinely held, and the panic floor's 1.2 s unconditional Core
for every startled bird) — Core survives only in the predator
flash's one glyph; and the jitter halves (16.0 -> 8.0, the
NIGHT-research-12 halved-wobble precedent — the audit's largest
scatter constant), the same two rolls per bird. 10s A/B at
120x40 wet IO (dev profile, two-run discard-warmup): murmuration
avg fps 3140.6 -> 3203.8 (+2.0%), median 3148.8 -> 3149.6, p99
0.4061 -> 0.3778 ms (-7.0%), avg dirty cells/frame 117.3 ->
111.7 (-4.8% — the tighter flock trims wandering cells, the
NIGHT-research-16 concentration-signature class), entropy 5.26
-> 5.26, gini 0.7243 -> 0.7246, heap retained 0 B, peak RSS
8.92 -> 8.96 MiB, drift improving (+1.74% -> +0.27%). The
signature shifts track the concentration intent; zero
allocation, same pools, same roll count.

**NIGHT-research-20 (2026-09-11, the dna_helix soft-light round):**
three standing Core sites re-graded to the Hot warm ceiling — the
fresh nucleotide heads (Core stood for the first 20% of every
drop's lifetime), the replication wake (the fork's 2.6 charge
stamp decayed through Core for ~1.6 s per written rung; now a
~0.35 s fresh-write blink above the new 2.3 bound, then the warm
wake), and the rung front-face step-up (a standing Hot rung no
longer composes Core; the blink keeps its flash across the whole
face). No RNG change, no allocation. 10s A/B at 120x40 wet IO (dev
profile, two-run discard-warmup): dna_helix avg fps 7469.1 ->
7814.4 (+4.6%), median 7682.5 -> 7930.0, p99 0.1774 -> 0.1735 ms,
avg dirty cells/frame 55.6 -> 51.9 (-6.7% — the softer soup heads
and capped wake change fewer drawn contents through the
frame-equality fast path), entropy 4.55 -> 4.46, gini 0.8298 ->
0.8382, heap retained 0 B, peak RSS 8.98 -> 9.04 MiB, drift
improving (-2.97% -> +0.57%). The level re-grades pay a mechanical
dividend: the same cells repaint softer and the diff engine finds
fewer of them changed.

**NIGHT-research-19 (2026-09-11, the aeolian soft-light round):**
the drop kinetic ladder's top rung re-graded Core to the Hot warm
ceiling (the rain half matching the instrument half's own
knots-only-Core policy; AEOLIAN_SPEED_CORE retired with the rung),
so the falling glyphs read the warm streak from the accelerating
band to terminal while the interference knots keep the scene's
only white; the comet trails compose one rung dimmer behind the
softer heads. No RNG change, no allocation. 10s A/B at 120x40 wet
IO (dev profile, two-run discard-warmup): aeolian avg fps 7599.5
-> 7944.2 (+4.5%, confirmation 7870.0), median 7430.3 -> 7721.2,
p99 0.1706 -> 0.1728 ms, avg dirty cells/frame 74.8 -> 71.0
(-5.1%, stable across the confirmation run — the re-graded heads
and dimmer trails change fewer drawn contents through the
frame-equality fast path), entropy 5.65 -> 5.63, gini 0.6285 ->
0.6312, heap retained 0 B, peak RSS 9.05 -> 9.01 MiB, drift
improving (+2.63% -> +0.78%). A level re-grade with a small
mechanical dividend: the same cells repaint softer and the diff
engine finds fewer of them changed.

**NIGHT-research-18 (2026-09-11, the physarum soft-light round):**
the trail ladder's top rung re-graded Core to the Hot warm
ceiling — the saturated-vein zone (equilibrium above the retired
0.30 bound) merged with the sustained-vein zone, so the whole
signature surface reads warm-gold instead of standing white
blend; PHYSARUM_BRIGHTNESS_HOT retired with the rung, its tuning
knowledge folded into the MID/deposit docs. A pure level
re-grade on the same drawn cells — no RNG change, no allocation.
10s A/B at 120x40 wet IO (dev profile, two-run discard-warmup):
physarum avg fps 7945.7 -> 7968.1 (+0.3%, flat), median 8106.3
-> 8145.6, p99 0.1529 -> 0.1495 ms, avg dirty cells/frame 78.5
-> 78.4, entropy 5.45 -> 5.43, gini 0.6890 -> 0.6923, heap
retained 0 B, peak RSS 9.03 -> 9.06 MiB, drift stable (-0.38%
-> +0.47%). Mechanically honest-flat: the standing vein cells
repaint at the palette's bright stop, nothing else moves.

**NIGHT-research-17 (2026-09-11, the monolith soft-light round):**
the Hero head rung re-graded Core to the Hot warm ceiling
standing, with Core reserved for the fresh cascade's ~1.5 s
arrival-reveal window (the dragon entry-reveal precedent; the
reveal countdown stamps at activation and decays on advance's
capped clock) — a pure level re-grade on the same drawn cells,
the draw stream and RNG untouched, the cap at the monolith draw
site while color_for_level stays the shared ladder for the five
sibling scenes. 10s A/B at 120x40 wet IO (dev profile, two-run
discard-warmup): monolith avg fps 6699.3 -> 6679.3 (-0.3%,
flat), median 6660.1 -> 6721.8, p99 0.1939 -> 0.1918 ms, avg
dirty cells/frame 107.3 -> 107.1, dirty ratio 2.24% -> 2.23%,
entropy 3.92 -> 3.92, gini 0.8945 -> 0.8946, heap retained 0 B,
peak RSS 8.94 -> 8.96 MiB, drift stable (-0.54% -> -0.71%).
Mechanically honest-flat: the standing Hero heads repaint at the
warm stop instead of the white-blended stop, nothing else moves.

**NIGHT-research-16 (2026-09-11, the vortex lockstep round):** the
per-mote spin/fall multipliers (0.85-1.15 / 0.80-1.25) retire for
lockstep pace — every mote at the same radius advances identically
(the black hole's all-lanes-consistent precedent), two RNG draws
retire with the rolls, the K/r law relabels honestly as a flat
rotation curve (VORTEX_KEPLER_K -> VORTEX_ROTATION_K), and the
radius ladder's Core rung re-grades to Hot (the ceiling by
construction, not by pass-ordering accident). 10s A/B at 120x40
wet IO (dev profile, two-run discard-warmup): vortex avg fps
7712.9 -> 7817.8 (+1.4%), median 7367.7 -> 7581.8, p99 0.1733 ->
0.1704 ms, avg dirty cells/frame 90.0 -> 88.2 (-2.0% — the
tighter arcs trim off-arm wanderer cells), entropy 5.68 -> 5.67,
gini 0.6342 -> 0.6361 (the arms concentrate — the signature the
owner asked for), heap retained 0 B, peak RSS 9.01 -> 8.91 MiB.
The signature shifts track the concentration intent; the round is
otherwise mechanically flat.

**NIGHT-research-15 (2026-09-11, the dragon soft-light round):** the
head rung re-graded Core to the Hot warm ceiling standing, with
Core reserved for the ~1.5 s entry-reveal arrival flash (the
formation-collapse precedent) — a pure level re-grade on the same
drawn cells. 10s A/B at 120x40 wet IO (dev profile, two-run
discard-warmup): cosmic_dragon avg fps 22678.9 -> 22809.1 (+0.6%,
flat), median 23410.2 -> 23810.4, p99 0.0681 -> 0.0670 ms, avg
dirty cells/frame 31.7 -> 31.8, entropy 4.07 -> 4.06, gini 0.8706
-> 0.8709, heap retained 0 B, peak RSS 9.05 -> 8.93 MiB, drift
stable (+0.04% -> +0.94%). The cheapest tier-one round: one
ladder rung plus its draw-site flag, zero mechanical motion.

**NIGHT-research-14 (2026-09-11, the quasar soft-light round):** the
disk ladder composes through the ported soft_head_level after the
ignition cap (the inner rung, the doppler limb's step-up and the
fresh-feed charge step-up never land Core), the jet ladder caps
before the knot's step-up (the collar reads warm, the knot's
traveling pulse stays the beam's one transient Core flash), and
the core cell reads the soft warm ceiling standing with a 0.85-1.0
pulse breathing, burning Core only inside the 2.0 s flare window —
pure level re-grades, the retired Core white blend leaves every
standing surface. 10s A/B at 120x40 wet IO (dev profile, two-run
discard-warmup): quasar avg fps 12866.7 -> 12889.1 (+0.2%, flat),
median 13639.0 -> 13587.8, p99 0.1007 -> 0.1012 ms, avg dirty
cells/frame 45.1 -> 45.1, entropy 3.90 -> 3.89, gini 0.8867 ->
0.8868, heap retained 0 B, peak RSS 8.96 -> 9.03 MiB, drift stable
(+1.8% -> +0.9%). The round is mechanically honest-flat: same
cells repainted at softer levels, zero allocation, same pools —
the NR12 precedent class (the cheapest visual round in the audit's
tier one).

**NIGHT-research-13 (2026-09-11, the black hole lock verification):**
the owner locked the scene at 10/10 after four rounds; this is the
lock's stability record. Full suite 2800 green, cargo fmt clean,
clippy -D warnings clean, gate-keepers 12/12 (yamllint and
actionlint skip in the sandbox; CI covers both). Two-run 10s
protocol at 120x40 wet IO on the NR12 code (f6466ad is docs-only):
median fps 1954.9 / 1962.2 (NR12 exit 1959.5 — identical), avg
dirty cells/frame 215.8 / 215.3 (exit 215.2), dirty ratio 4.50%,
frame entropy 6.09 (6.08), density gini 0.5420 (0.5454), heap
retained 0 B, peak RSS 9.18 / 9.29 MiB, p99 frame time 0.644 /
0.600 ms, fps drift -5.3% / +3.2% both read stable. Avg fps read
1876.0 / 1892.7 against the exit's 1931.4 — a 2-3% run-to-run
spread on a loaded sandbox; every machine-independent signature
metric (median, dirty, entropy, gini, heap) is identical, so the
lock holds. The companion audit of the other thirteen rain styles
lives in docs/research/NIGHT_RESEARCH_13_SCENE_AUDIT.md.

**NIGHT-research-10 (2026-09-11, the black hole photon line, triple
crown, and head-white ladder):** the rim photon line (the annulus's
outer band flipping to Core — a level re-banding, zero new cells),
the five-lane halo re-split (the same rider population redistributed
across three crowns and two lower arcs — zero new motes), and the
two head-white ladders (the crown gain and the tier-0 near-ball
floor — per-head ladder lookups only). 10s A/B at 120x40 wet IO
vs the parent commit (before 0f8f42f, after 954e3d7, dev profile,
same two-run discard-warmup protocol): sorgonemous_intrascals avg
fps 1706.1 -> 1652.5 (-3.1%, confirmation run 1678.0 — call it
-2 to -3%), median 1737.5 -> 1708.2, p99 frame time 0.770 ->
0.677 ms (-12.1%), p99.9 1.071 -> 1.008 ms, avg dirty cells/frame
198.8 -> 217.2 (+9.3%, dirty ratio 4.14% -> 4.53%), frame time
stability excellent on both sides, allocator flat (heap retained
0 B, peak RSS 9.02 -> 9.07 MiB). The cost is mechanical and
owner-directed: the lower family's doubled share (0.18 -> 0.28 of
the halo pool) puts more riders on the arcs that cross the disk
band's zone under the shadow, so more cells change hands between
populations per frame (the +18 dirty cells), and the tier-0 floor
plus the brighter Core trails repaint more near-ball levels per
frame; nothing allocates (flat heap, same pool sizes — the
features are level and tag changes, not population changes).
Visual signature shifts track the intent: entropy 6.07 -> 6.09
(noise), density gini 0.5487 -> 0.5440 (the doubled lower arcs
spread the composition slightly wider below the shadow), color
transition delta 136.9 -> 132.9 (the crowns sit steadier at Core
white — less palette wandering on the upper family). The one-off
8.3 ms max-frame spike in the first after-run did not reproduce
(confirmation max 4.9 ms — OS jitter, not a code path).

**NIGHT-research-9 (2026-09-11, the black hole masterclass physics
pass):** the corotation spawn (an O(1) per-mote angular-momentum
derivation), the width-capped ball, the stretched disk unit, the
proximity-ladder gain, the radial-speed brightness input, and the
see-saw window re-cut with the dynamic tilt cap. 10s A/B at
120x40 wet IO vs the parent commit (before bc132c2, after
43dafb9, dev profile, same two-run discard-warmup protocol):
sorgonemous_intrascals avg fps 2042.2 -> 2330.0 (+14.1%), median
2072.4 -> 2359.1, p99 frame time 0.648 -> 0.505 ms (-22.1%), max
frame 1.680 -> 1.297 ms, avg dirty cells/frame 184.0 -> 161.0
(-12.5%, dirty ratio 3.83% -> 3.36%), frame time stability
excellent on both sides, allocator flat (heap retained 0 B). The
win is mechanical, not noise: the width-capped ball at 120x40
drops the shadow from 11 to 9 line-height radii, and the annulus
area (the per-frame ball cell count) scales with r squared —
about a third fewer ball cells drawn and diffed every frame, with
the disk motes, halo riders, and infall glyphs at their usual
populations. Visual signature shifts track the geometry change
exactly as intended: entropy 6.00 -> 5.96 (noise), density gini
0.5642 -> 0.5758 (the smaller shadow concentrates the composition
slightly), color transition delta 0.00 on both sides.

**NIGHT-depthtest-2 (2026-09-11, CLI/config validation surface):** the
duplicate-key/section detection, the explicit `--config` read-error
path, and the FreeBSD system-path candidate are all STARTUP-time
changes (parse + validation once per process, or once per
config-save on the watcher thread); the render loop is untouched.
10s A/B at 120x40 truecolor vs the parent commit (before 2a7e406,
after ce696e3): every metric within +/-0.3% (sorgonemous_intrascals
avg fps 13882.9 -> 13901.4, dirty cells/frame 228.86 -> 228.73,
entropy 6.042 -> 6.047, gini 0.5505 -> 0.5491; monolith avg fps
36963.3 -> 36970.9, dirty 182.74 -> 182.91) — measurement noise,
no regression, visual structure signatures identical.

**NIGHT-hunter-29 (2026-09-10, the phosphor ownership rule):** the
fix removed the phosphor ghost-vs-draw strobe that had been
inflating every structured family's steady-state diff (the
afterglow pass dimmed persistently-drawn cells, the family draw
restored them — a period-2 full-population churn). 10s A/B at
120x40 truecolor, `--scene sorgonemous_intrascals`: avg fps
13028 -> 15743 (+20.8%), avg dirty cells/frame 789.4 -> 179.6
(-77.3%, dirty ratio 16.45% -> 3.74%), avg render ms 0.0230 ->
0.0062 (-73%) for +0.005ms of sim (the per-drawn-cell phosphor
zeroing). `--scene monolith`: avg fps 40234 -> 50252 (+24.9%),
dirty 270.7 -> 107.3 (-60.4%). The (entropy, gini) points shift
accordingly (black hole entropy 6.11 -> 5.94, gini 0.544 ->
0.580; monolith entropy 4.81 -> 3.92, gini 0.811 -> 0.894): the
strobe's constant whole-field flicker had artificially flattened
the density distribution and inflated the entropy — the
post-fix points are the styles' true structure signatures.

## Key Metrics

| Metric | Unit | What it tells you |
|--------|------|-------------------|
| `avg_fps` | FPS | Mean frames per second. Primary throughput number. |
| `peak_fps` | FPS | Highest instantaneous FPS. Often much higher than avg (diff engine skips frames with zero dirty cells). |
| `p99_frame_time` | ms | 99th-percentile frame time — slowest 1% of frames. Catches spikes avg hides. |
| `frame_time_stability` | label | `excellent` = p99 within 2× avg, max within 5×. |
| `fps_drift_percent` | % | (first_half − second_half) / first_half × 100. Negative = warmup; positive = throttle/leak. \|drift\| < 5% = stable. |
| `glyphs_per_second_theoretical` | glyphs/sec | Theoretical upper bound: full-frame cell count × active-frame rate. NOT actual throughput — use `dirty_glyphs_per_second` for actual rendered work. |
| `dirty_glyphs_per_second` | glyphs/sec | Changed cells per second — the work the diff engine actually emits. |
| `peak_rss` | MiB | Peak resident set size. Steady growth across runs = possible leak. |
| `avg_cpu_percent` | % | Process CPU%. ~99% = single-threaded, fully utilized. |
| `alloc_calls_per_frame` | count | Fresh allocations per frame. Higher = leaking heap. v30 baseline: 3.00. |
| `heap_retained` | bytes | Bytes allocated and never freed. Non-zero = investigate. |
| `energy_per_frame` | µJ | Energy per frame (Linux + RAPL only). Lower = more efficient. |
| `IPC` | ratio | Instructions per cycle (Linux + perf_event_open). >2.0 = healthy; >3.0 = excellent. |
| `frame_entropy` | bits | Information entropy of frame content. Higher = more visual variety. |
| `density_gini` | 0..1 | Gini coefficient of cell density. 0 = uniform; 1 = maximally concentrated. |

Wet (`--bench-io`) = writes ANSI to `/dev/null`; dry = no I/O (pure engine throughput). `lean` = dirty-cell-only emission (fastest); `production-draw` = full `Terminal::draw` path.

## Component Timing + Wet I/O + Scaling + Baseline

**Component Timing**: the `COMPONENT TIMING` section breaks the frame budget into per-subsystem costs — rain simulation, phosphor decay, color resolution (Chroma Dragon), BOLT formatting, ANSI emission, I/O write. Use this to identify which subsystem dominates frame time when profiling.

**Wet I/O vs Dry**: dry benchmarks measure pure compute throughput. Wet benchmarks (`--bench-io`) additionally exercise the kernel syscall path by writing ANSI bytes to `/dev/null`, surfacing `write_bandwidth` (MiB/s), `avg_write_latency` (µs), `backpressure_events` (write stalls — non-zero = terminal can't keep up), `effective_write_fps`, `total_bytes_written`.

**Scaling (`--bench-all`)**: runs the benchmark across a sweep of screen sizes (6×6 -> 200×60) and prints a SCALING SUMMARY table showing how FPS, dirty-cell ratio, and throughput scale with cell count. Use this to verify the diff engine's O(dirty_cells) claim holds at scale — dirty-cell ratio should drop as screen size grows (most cells unchanged per frame).

**Baseline Save & Compare**: `--save-baseline PATH` writes the JSON output to a whitelist-enforced path (`~/.config/cosmostrix/` or `/etc/cosmostrix/`). `--compare-baseline PATH` compares the current run against the saved baseline and flags any metric that regressed by more than 5%. Use in CI to catch performance regressions before they ship.

## Microarchitecture & Energy (Linux only)

Two additional sections require elevated privileges: `MICROARCHITECTURE` (Linux `perf_event_open` syscall — CPU cycles, retired instructions, branch instructions, branch misses, IPC, branch mispredict rate) and `ENERGY` (Linux RAPL powercap sysfs — total energy, avg power, energy per frame, energy per cell). Both are entirely opt-in — cosmostrix never silently probes privileged interfaces. See [BENCHMARK_ADVANCED.md](BENCHMARK_ADVANCED.md) for setup instructions.

## Reproducibility + Honesty Contract

**Reproducibility checklist**: same commit (`git rev-parse HEAD`), same profile (`pro-linux-v3` / `pro-linux-v4` / `nitro-pgo`), same `--bench-duration`, `--screen-size`, `--bench-scene`. Pin CPU governor (`cpupower frequency-set -g performance`), disable turbo if comparing across machines. Close other CPU-bound processes. Run twice — discard the first (warmup fills caches); use the second as the reported number. For wet benchmarks, ensure `/dev/null` is on tmpfs (default on Linux). For energy benchmarks, unplug laptop charger (battery gives cleaner RAPL) or pin to a desktop CPU with stable power.

**Honesty contract**: benchmark FPS is **synthetic uncapped throughput** measured in a headless simulation. It is NOT a release promise. The actual runtime target is the configured FPS (dynamic default: 60 on standard terminals, 144 on high-refresh; override with `--fps`). The terminal emulator's ANSI parse speed is the ceiling — no amount of SIMD, GPU, or C supercharger can fix a slow terminal. Do not chase raw FPS; frame-time stability and p99 latency matter more. The `RENDERER` section always reports `gpu_usage: not_applicable` — cosmostrix is CPU-only by design (see [PHILOSOPHY.md](PHILOSOPHY.md)). `--doctor` carries the same field for consistency.

## Diagnostic Recipes

- **"FPS is lower than expected"**: check `frame_time_stability` — if `medium`/`high`, look at `max_frame_time` for spikes. Check `fps_drift_percent` — positive drift = throttle/leak. Verify CPU governor is `performance`.
- **"RSS grows over time"**: check `alloc_calls_per_frame` and `heap_retained`. Steady growth in `peak_rss` across multiple `--bench-duration 60s` runs = leak. Use `--bench-duration 5m` to confirm.
- **"Wet bandwidth is low"**: check `backpressure_events` — non-zero = kernel pipe full. Check `avg_write_latency` — >1ms suggests `/dev/null` is not on tmpfs.
- **"IPC is below 2.0"**: verify the binary is built with `pro-linux-v3` or `pro-linux-v4` profile (AVX2/AVX-512). `cargo build --release` without a profile gives baseline SIMD.
- **"Energy per frame is high"**: check CPU governor — `powersave` inflates energy-per-frame. Verify RAPL is reading the right socket (multi-socket systems).
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
