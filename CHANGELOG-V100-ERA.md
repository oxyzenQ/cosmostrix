# Changelog — v100 Era
<!-- SPDX-License-Identifier: GPL-3.0-only -->

The v100 era: every `v100.0.0-nightly.1` entry, verbatim, that carried
the renderer from the v80 line to the v100.0.0 stable release
(2026-09-04 to 2026-09-06). These entries accumulated under the live
[CHANGELOG.md](CHANGELOG.md) Unreleased section and were moved here
unchanged in NIGHT-docs-1 — immutable historical record, original
order preserved.

## v100.0.0-nightly.1 — The Nightly Hunts

### stability: v100.0.0-nightly.1 — NIGHT-hunter-15 'r' restart residue on glyph + the dragon_hunt milestone scene

Owner report (2026-09-06, post-e58f8b8): pressing 'r' on glyph rain did
not restart the field — the screen appeared stuck for a moment, then
rain resumed falling on top of the old glyphs, which stayed permanently
("the previous rain lingering, stuck on screen"). Every other style cleared and
restarted from the top as documented; the owner asked to fix glyph and
verify the other six styles, then commemorate the biggest bug hunt with
a milestone scene.

Root cause: the 'r' handler calls `cloud.reset()` +
`cloud.force_draw_everything()`. `reset_with_bounds` armed
`semantic_invalidate` only for STRUCTURED styles — for the droplet
family (glyph) the force flag was consumed by the HUNT-25 resync path
(`Frame::force_repaint` — re-emit current content, NO clear), so the
frame kept the pre-restart glyphs while the simulation state (droplet
pool, phosphor arrays) was wiped: no droplet owned those cells anymore
and the phosphor decay system no longer tracked them, so nothing could
ever blank them. Pre-HUNT-25 the glyph force branch called
`clear_with_bg` (which cleared the screen on restart, alongside the
resync mass-dump HUNT-25 fixed); the force_repaint swap removed the
restart clear as collateral damage. Fix: `reset_with_bounds` arms
`semantic_invalidate` for EVERY style — a hard reset is a semantic
event, the same contract scene switches already honor — so the first
post-restart frame routes through `invalidate_semantic` (full logical
clear + generation bump + terminal LastFrame resync) before the force
branch. Bare resyncs (idle resync, stuck sweep, P2 mitigation — force
flag WITHOUT a reset) keep the HUNT-25 non-perturbing contract. All
other reset() callers rebuild the Frame fresh (resize, live-reload,
intro re-read, startup, bench), where the invalidation on an
already-blank frame is a no-op.

Verification: three new regression tests in tests_restart_hunt15.rs —
the glyph restart pin (content epoch + semantic_gen must bump, every
pre-restart cell must blank; verified to FAIL on the pre-fix code), the
all-seven-styles restart contract (no cell may retain pre-restart
content unless the fresh simulation rewrote it this frame — pins the
six structured styles that already passed so none can regress), and the
bare-resync non-regression (force flag without reset preserves content
and generations — the HUNT-25 contract). End-to-end: a new committed
PTY evidence tool, scripts/nh15_restart_e2e.py, renders the ANSI stream
through the nh2 mini terminal emulator and asserts the screen blanks
to <10% within 0.3 s of the key and refills from the top (with
`--intro none` to keep the key past the startup animation): the fixed
binary clears at +0.01 s in 3/3 runs; the pre-fix binary retains the
residue (no clear within the window) in 3/3 runs — the owner's symptom
reproduced and eliminated at the real terminal boundary. Monolith,
lorenz, and the new dragon_hunt scene verified the same way.

Milestone (owner spec): the `dragon_hunt` scene — the biggest bug hunt
in cosmostrix history, the "glitch rain shift" run to ground across
HUNT-23..26 (26 rounds: output drain backoff, EMA pressure decoupling,
the phosphor park-epoch fix, the P2 resync full-body flash fix, the
MADV NUL emission fix, the amortized thaw). The Lorenz butterfly — the
engine's strange attractor, the motion the hunters chased the ghost
through — rendered in the `nebula` palette on `blocks` glyphs at cycle
position 19 (milestone group, right after cosmic-dragon). Speed 22 (a
hair under the lorenz flagship's 24: the hunt is over, the butterfly
glides), density 0.70 (flagship parity), glitch level NONE — the glitch
is dead. Catalog grows 23 -> 24 scenes; the scene-count pins, the
sorted name list, the x-cycle order pins, the --scene help list, and
the README milestone section all updated; a stale "21-scene cycle"
comment in the interactive tests corrected.

Gates: cargo fmt clean, clippy 0 warnings, 2411/2411 unit tests
(2407 + 3 restart + 1 scene pin), build.sh check-all green (cargo-audit
skipped: not installed, same as prior sessions), gate-keepers 10/10,
LOC caps respected (scene/mod.rs 729, help_detail.rs 703,
spawn_reset.rs 228). 10s A/B benchmark (cinematic + matrix scenes,
120x40, interleaved same-machine builds, warm discarded):
noise-equivalent — cinematic 6570/6490 fps (matrix 6259/6308 — the
delta flips sign across scenes, run noise), entropy 5.748/5.744 and
5.748/5.747, gini 0.6401/0.6406 and 0.6398/0.6407. Expected: the
semantic flag fires once per reset (frame 1 of a bench run, where
dirty_all was already set) and never again. dragon_hunt first baseline:
54.3K fps, p99 0.024 ms, entropy 5.883, gini 0.5925 — in family with
the lorenz flagship.

### stability: v100.0.0-nightly.1 — NIGHT-hunter-14 deep audit of the two original rain styles (glyph + monolith): warm-start free-list leak + drawn-gen wrap guard

Owner-mandated deep audit of the two ORIGINAL rain styles (Glyph and
Monolith), closing the loop on the NIGHT-hunter-10 series that audited
the five newer structured styles (vortex, lorenz, physarum,
cosmic_dragon, flux). Two defect classes found and fixed; the rest of
both styles' hot paths verified at peak (droplet draw: hoisted
head-brightness/transition-energy/frac-progress, LUT'd edge fade +
vignette, chroma-routed blend chains; monolith draw: drawn-gen skip
pass, per-stream tone hoist, direct-indexed arrays — no further gains
available without over-engineering).

1. Glyph warm-start free-list violation (correctness + LTS).
   `ensure_glyph_pool_and_warm_start` (spawn.rs) seeded scene-entry
   droplets by DIRECT pool indexing (`&mut self.droplets[i]`) without
   popping their slots from `droplet_free_list`, whose contract is
   "contains exactly the dead droplet indices" (spawn_logic.rs). Under
   pool pressure a later spawn could pop an ALIVE index and silently
   overwrite a live droplet mid-fall; the old column's
   `col_stat.num_droplets` budget then leaked permanently (death
   decrements only the OVERWRITTEN droplet's new column), thinning that
   column's rain density until the next reset or scene switch. Fix: the
   warm-start loop pops its slots from the free list (invariant exact;
   `break` covers exhaustion). Five regression tests in
   tests_hunt14.rs pin the invariant, the pool conservation law
   (free-list length + alive == pool size), column-budget exactness
   under sustained spawn pressure, and exhaustion termination — the
   invariant pins verified to FAIL against the pre-fix code.

2. Monolith drawn-gen u32 wrap (LTS). The Pass 2 tag counter
   (`drawn_gen_counter`) had no wrap guard — unlike its twin in
   frame.rs (`GEN_RESET_THRESHOLD`). After a ~2.2-year continuous
   session the counter wraps back over stale tag values; a false
   "redrawn this frame" match in Pass 3 skips a needed clear_cell and
   drops that position from the diff history with no production
   recovery path (the stuck-cell sweep is debug-gated). Fix: on wrap,
   zero every tag and restart the counter at 1 (0 stays the
   "never drawn" sentinel — same semantics as Frame's guard). Two
   regression tests pin the wrap fold (tags never exceed the counter)
   and that vacated cells still clear across the wrap boundary.

Gates: cargo fmt clean, clippy 0 warnings, 2407/2407 unit tests,
build.sh check-all green, gate-keepers 10/10. 10s A/B (monolith + matrix
scenes, 120x40, interleaved same-machine builds, warm discarded):
noise-equivalent — monolith 35.5K/35.5K fps, entropy 4.838/4.838,
gini 0.8072/0.8073; matrix ~6.3K fps, entropy 5.740/5.743, gini
0.6429/0.6414. Expected: neither fix touches the steady-state hot path
(the warm start runs only on scene entry; the wrap guard is a
never-taken branch inside any realistic bench window).

### stability: v100.0.0-nightly.1 — S-master-HUNT-26 the "glitch rain shift" actually root-caused: park-epoch bug + P2 resync bomb (NIGHT-hunter-2 round 2, owner hunt 2026-09-05)

Owner report (post-e3d1834, commit-verified on Alacritty 0.17): the EMA
pressure decoupling killed the strobe, but the glitch survived in a new
shape — landing in the first ~9-40 s of a fresh session, self-healing,
absent on the monolith scene, and re-triggerable for a few seconds by
the FIRST charset/color shortkey (`s/S/c/C`) after a long clean run
(the rain sweeping left-to-right "like lightning"), with subsequent
shortkeys clean.

Empirical hunt (two new PTY tools committed alongside: a content-level
mini-terminal-emulator harness that diffs per-frame screen grids, and a
full-speed raw spool capture + offline replay — the inline harness
itself turned out to push the app into its marginal-drain regime, so
the fast-Alacritty regime needed drain-rate-true capture): the owner
symptom decomposed into three independent defects.

Defect 1 (the core, all regimes): the phosphor decay pass's park
branch gated "cell blanked this frame" on the content-EPOCH generation
(`cell_gen == gen`), which only `clear_with_bg` resets — so "this
frame" silently meant "any time since the last semantic event". Every
cell a droplet ever vacated parked at the tail-residual energy
FOREVER: the CRT afterglow never rendered at steady state (the visible
trail was only the droplet body itself), the active list grew without
bound (measured 9,500 cells at 200x56 — every vacated cell since the
last semantic event), and the next epoch bump (charset shortkey,
palette drift, ambient snapback) dumped the whole parked set as a mass
ghost flash. Fixed by making the park check — and Pass 1's full-grid
capture scan — read the per-frame dirty-generation stamp
(`Frame::cell_written_this_frame`, now stamped unconditionally in
set/set_force; the list push stays gated on !dirty_all). Vacated cells
now get the documented one-frame grace, then decay, render their
afterglow, and die on schedule; the active list is bounded by the live
trail; the afterglow is alive again (steady-state colored-ghost
population measured 50 -> 120-240; bench dirty cells 56.8 -> 140.8 per
frame at 80x24, entropy 3.29 -> 4.18, gini 0.8962 -> 0.8175 — the
designed visual finally running; avg_fps 91.5K -> 71.3K in the
synthetic bench, 500x the 144 fps target with the 10% fast-regime
emission growth buying the live trails).

Defect 2 (the 30 s cadence on healthy fast terminals): the P2
self-healer's TriggerHealthMitigation still forced
`force_draw_everything` every 30 s cooldown whenever the endurance
health score sat in the investigate band — which a healthy fast
terminal does (measured ehs 49-58 at 144 fps with drain pressure at
zero). The resync force frame then (a) flipped the droplet draw's
fractional-position skip into full-body mode (draw_everything was
wired to the raw force flag), drawing every not-yet-reached body cell
at once — a measured ~1,700-glyph one-frame flash every 30 s (the
owner's "at 57 seconds", the 9-40 s window) — and (b) left the
MADV_DONTNEED-zeroed frame cells reading as `Cell{ch:'\0'}` because
the reclaim path's gen-bump assumption stopped holding when HUNT-25
moved the glyph force to force_repaint, emitting raw NUL bytes
terminals silently drop. Fixed: droplets draw full bodies only when
the frame content was actually invalidated this frame (semantic event
or structured-style force clear — the charset/palette waves keep
their full-body redraw; pure resyncs keep the fractional skip), and
the reclaim path re-blanks zeroed cells via the new
`Frame::normalize_reclaimed_cells` so they emit as proper blanks.

Defect 3 (marginal-drain terminals): the phosphor pressure gate's
skip hysteresis (0.70/0.50) froze the decay pass under sustained
congestion while droplet tails kept blanking cells; when the EMA later
dropped below the resume threshold the pass rendered the entire
accumulated backlog within one or two frames (measured 6,151 cells at
200x56 — thousands of blank cells flashing to afterglow at once, a
2-6x frame-size burst that re-saturated the pipe and re-armed the
skip, the self-exciting loop). Fixed by the amortized thaw: on resume
every active cell is marked pending and at most
PHOSPHOR_THAW_MAX_CELLS_PER_FRAME (600) of them are written per
frame; the backlog drains as a soft fade-in over ceil(backlog/600)
frames instead of one dump. Monolith never accumulates a backlog (its
per-frame clear_cell zeroes energies and the decay pass removes
zero-energy cells silently) — matching the owner's monolith-immunity
observation.

Verified end-to-end with the new harness: fast regime (11 MB/s drain,
144 fps) — the periodic 30 s mass-glyph events and the first-switch
dump are gone (only the two startup fill-up transients remain, both
normal); marginal regime (Python-paced reader) — the freeze/thaw churn
storms and the 6,151-cell switch dumps are gone, the charset switch
now produces one 535-cell semantic frame plus a smooth ~600-cell/frame
five-frame fade. 14 new regression tests
(tests_phosphor_thaw_hunt2.rs: thaw budget, one-frame park, silent
zero-energy/fresh departures, resync keeps fractional body skip,
resync does not reseed earlier-frame writes, MADV normalization,
reskip-mid-thaw, steady-state budget-free operation; the park pin
verified to fail against the pre-fix code). One legacy test's frame
lifecycle simulation corrected (clear_with_bg is a semantic event,
not the per-frame boundary — clear_dirty is). Gates: fmt clean,
clippy 0 warnings, 2402/2402 unit tests, stresstests green.

### feat: NIGHT-hunter-9 + NIGHT-research-5 — HUD `rain:` metric + scene-custom `rain` field (seventh dimension)

Two owner-approved features landed together in 2b24898 (shared scope:
HUD + scene-custom schema + `RainStyle` label API), then hardened by a
follow-up verification pass (a613e0b + the NIGHT-research-5 pass) that
closed the test/documentation gaps the initial landing left behind.

NIGHT-hunter-9 — HUD rain style metric:

- New HUD row 19 `rain: <style>` shows the active rain style label
  (glyph, monolith, vortex, flux, lorenz, dragon, physarum), positioned
  directly above `dcel:` per owner mandate so the user reads the active
  motion DNA before the cell-efficiency metrics. `HudState` gains the
  `rain_style` field + `set_rain_style()` setter, driven from the event
  loop every frame; the HUD buffer grew 24 -> 25 rows and the chroma
  gradient now computes 25 stops.
- Verification pass added the missing unit tests (row 19 content for
  all seven labels, Glyph default, rain-above-dcel layout lock) and
  extended `scripts/hud_order_e2e.py` from 24 to 25 tracked labels.

NIGHT-research-5 — scene-custom `rain` field (seventh dimension):

- `[scene-custom.<name>]` blocks gain the `rain` field: pick any of the
  seven rain styles by canonical label, case-insensitive (e.g.
  `rain = "lorenz"`). `RainStyle` gains `from_label()` +
  `valid_labels_hint()`; `UserProfile` gains the `rain` field;
  `SCENE_CUSTOM_REQUIRED_FIELDS` / `SCENE_CUSTOM_FIELDS` /
  `PROFILE_FIELDS` all list `rain` first, so a block is now a COMPLETE
  seven-dimension profile (rain, color|colors-custom,
  charset|charset-custom, fps, speed, density, glitch-level) — the
  `rain` field is the only non-glyph source since `base-scene`
  inheritance is gone. Both the startup path (`resolve_rain_style` in
  main.rs) and the live-reload path (`scene_apply.rs` +
  `apply_scene_custom_field_to_cloud_config`) resolve the label;
  invalid labels warn with the valid-labels hint.
- Verification pass added the missing tests (label round-trip,
  custom-scene resolution including the owner's `rain = "lorenz"`
  example, case-insensitivity, retired-label fallback, live-reload
  style switch) and refreshed stale comments/docs that still described
  the retired `ripple` style or the old six-dimension schema (README,
  docs/HUD.md, docs/CRYSTAL_DRAGON_ENGINE.md, scene_custom module
  docs).

### feat: NIGHT-research-5/6 merge — `cosmic_dragon` + `physarum` land on main; seven rain styles

Branch `cosmic_dragon` merged into `main` (commit range 2d3e916..0d759c5:
NIGHT-research-5 `cosmic_dragon` serpentine dragon, the dragon-count
follow-up fixing the active count at 3 to match the three dragon
engines, and NIGHT-research-6 `physarum` slime mold). The merge is
divergent: the branch forked from the task-18 baseline (which still
carried the owner-rejected `ripple`), while main had since replaced
`ripple` with `flux` (task-19) and merged `lorenz` (NIGHT-research-4).
The union keeps every surviving style — the catalog is now seven rain
styles (glyph cascade, monolith pillars, vortex polar orbits, flux
PIC/FLIP liquid, lorenz strange attractor, cosmic_dragon serpentine
chain, physarum slime-mold networks) and 23 scenes in the interactive
cycle (cosmic_dragon at position 7, physarum at 8, classic through
curiosity renumbered 9-23). Union resolution across the style registry
(`RainStyle` enum + family helpers — the ripple modify/delete conflict
resolved to deletion per the owner's task-19 verdict), the scene
catalog (`SCENE_ORDER` union, header and catalog-count pins 21 -> 23),
the dispatch chain (`rain_at`/`spawn`/`scene_runtime`/
`runtime_controls`/`spawn_reset` style-gate arm unions), the Cloud
struct/constructor fields, the style constants (`DRAGON_*` and
`PHYSARUM_*` blocks appended after `FLUX_*`/`LORENZ_*` in
style_rain.rs, the branch's `RIPPLE_SPEED_REF_CPS` remnant dropped),
and the mirrored test tree (`tests_dragon` + `tests_physarum` beside
`tests_flux` + `tests_lorenz`). The scene-catalog pin tests and the
cycle-order tests adapt to 23 scenes; the x-cycle test now walks five
hops through all four style flagships after the core trio.
- A/B 10s @ 120x40 truecolor (after merge): cinematic 6.0K fps / 984
  dirty / entropy 5.64 / gini 0.668 (noise-equivalent to the 0dfdc24
  baseline); monolith 34.3K / 291 / 4.84 / 0.807 (no regression);
  vortex 41.7K / 278 / 6.31 / 0.468 (identical to baseline); flux
  31.6K / 102 / 5.73 / 0.627 (matches the task-19 entry); lorenz
  52.3K / 138 / 5.87 / 0.597 (healthy). New signatures:
  cosmic_dragon 126.8K fps / 74.7 dirty / entropy 5.15 / gini 0.731 —
  the fastest style in the catalog with the fewest dirty cells (three
  serpentine chains) and the most concentrated density of the
  structured flagships; physarum 63.3K fps / 99.6 dirty / entropy
  5.74 / gini 0.624 — structured-class performance whose (entropy,
  gini) point lands near flux while the emergent-network motion
  signature stays 100% distinct. No regressions on the five
  pre-existing styles.

### feat: NIGHT-research-6 — `physarum` (bio-inspired slime-mold rain, sixth style — world-first in terminal matrix rain category)

A new rain style implementing the Jeff Jones 2010 slime-mold model:
particles follow sense / decide / move / deposit rules on a
stigmergic trail field, producing emergent NETWORK patterns (vein-like
structures that self-organize from random initial conditions, with no
central planner). This is the project's first bio-inspired renderer —
bridging biology (slime mold intelligence — Physarum polycephalum
solves mazes without nervous system), computer science (stigmergy /
multi-agent swarms), and generative art (network aesthetics).

- `physarum` (scene `physarum`, palette `cosmos` + charset `binary`):
  particles sense the trail field at three sensor positions (left-
  front, front, right-front) and steer toward the strongest signal.
  Each frame they move one step in their (possibly updated) heading
  direction (wraparound toroidal substrate — particles that exit one
  side reappear on the opposite side) and deposit trail chemical at
  the new cell. Positive feedback between deposition and sensing
  creates the network — paths that get used attract more traffic,
  unused paths decay (exponential trail decay each frame, the
  negative feedback that keeps the network alive).
- Terminal-limit exploitation — the masterpiece contract: the
  terminal's discrete cell grid IS the slime-mold substrate (a 2D
  chemical concentration field, one f32 per cell). No sub-pixel
  motion, no anti-aliasing — the medium matches the algorithm exactly.
  The trail field is INTERNAL (used for sensor sampling only); the
  visible vein network emerges from the engine's existing phosphor
  decay system. Cells that particles visit often accumulate phosphor
  (existing slow fade), creating the persistent network look — the
  terminal's "slow refresh" limitation BECOMES the slime mold's
  chemical memory.
- Motion DNA — 100% distinct from cascade (`cinematic`), pillars
  (`monolith`), polar-orbit (`vortex`), water-surface (`ripple`),
  serpentine chain (`cosmic_dragon`): each particle is a glyph agent
  in a multi-agent swarm. Particle head brightness is driven by the
  trail field value at the head position (high trail = bright vein
  cell; low trail = exploring dim cell), so the network is visible via
  the heads themselves — no direct trail field iteration needed
  (keeps draw cost O(N), not O(cells)).
- Masterpiece engineering / future-proof legacy: the algorithm is
  parameter-driven (sensor angle, sensor distance, deposit amount,
  decay rate, turn speed). The same code produces vastly different
  emergent patterns — branching trees (small sensor angle), spirals
  (high turn speed), mazes (low decay), rings (high deposit). This
  file sets a reusable standard for future bio-inspired styles (ant
  colonies, flocking birds, schooling fish could all reuse the
  trail-field + sense-decide-move substrate).
- Architecture: `RainStyle::Physarum` variant added; `cloud/physarum.rs`
  (~700 LOC, mirrors vortex/lorenz/dragon structure). Scene catalog
  grows to 22 scenes; `physarum` takes cycle position 7.
- A/B 10s @ 80x24 dry (no regression on existing styles): cinematic
  24K fps / 415 dirty / entropy 5.09 / gini 0.66; monolith 92K / 57 /
  3.29 / 0.90; vortex 113K / 41 / 4.77 / 0.70; ripple 16K / 505 /
  5.53 / 0.55; cosmic_dragon 199K / 27 / 4.01 / 0.81; physarum 102K /
  52 / 4.86 / 0.69 — physarum matches the structured-family
  performance profile (102K fps, 0.014ms p99) with a distinct visual
  signature (entropy between vortex and ripple, gini between vortex
  and cinematic — the emergent network distributes particles across
  the viewport differently than any single-motion style). No
  regressions on the other five styles.

### feat: NIGHT-research-5 — `cosmic_dragon` (Chinese-mythology serpentine dragon rain, fifth rain style)

A new rain style inspired by Chinese mythology (not Western): each
dragon is a chain of segments (head + body + tail) following a
path-generating head via FABRIK distance constraints (snake
kinematics). The Chinese dragon's signature serpentine silhouette
emerges from this chain dynamic without any procedural body animation.

- `cosmic_dragon` (scene `cosmic_dragon`, palette `nebula` + charset
  `zen`): dragons fly freely, sometimes circle, then fly free again
  — the owner spec "sometimes circling, sometimes flying free anywhere". The
  head runs a two-state machine: SOAR (smooth random-walk turn rate
  from layered sine noise — two frequencies, randomized phase per
  dragon, produces organic non-repeating free flight) and CIRCLE
  (constant-magnitude turn rate producing a circular orbit; direction
  CW/CCW randomized per state entry). State transitions are
  stochastic: SOAR lasts 4-8s, CIRCLE lasts 3-6s, weighted transitions
  (after SOAR 50/50 SOAR/CIRCLE; after CIRCLE 70% SOAR / 30% CIRCLE —
  favoring free flight). Wall bounce reflects velocity and snaps to
  SOAR (escape any pinning circle).
- Motion DNA — 100% distinct from cascade (`cinematic`), pillars
  (`monolith`), polar-orbit (`vortex`), and water-surface (`ripple`):
  each dragon is a glyph chain carried by a path-following head. The
  body inherits the head's path through the FABRIK distance constraint
  (each segment maintains fixed spacing to the previous) — the
  serpentine body trails the head organically, producing the
  signature sinuous silhouette of Chinese dragons in flight.
- Brightness gradient along the body: head = Core (brightest), first
  third of body = Hot, middle third = Mid, tail third = Ghost. This
  serpentine fade is the visible signature — the head leads brightly,
  the tail fades into mist. Matrix-style glyph mutation: segments
  re-roll glyphs on cell change (mutation tied to motion, parity with
  vortex/lorenz).
- Architecture: `RainStyle::Dragon` variant added; `cloud/dragon.rs`
  (~800 LOC, mirrors vortex/lorenz structure). The chain renderer is
  agnostic to the head motion model — swapping the head state machine
  (e.g., for a bee swarm, fish school, or bird flock) is a single
  function replacement (the body FABRIK solver is unchanged). The
  pattern sets a reusable standard for future chain-based styles.
  Scene catalog grows to 21 scenes; `cosmic_dragon` (underscore)
  takes cycle position 6 — distinct from the existing `cosmic-dragon`
  (hyphen) milestone scene.
- Naming distinction: `cosmic-dragon` (hyphen) is the existing
  milestone scene (Glyph-style tribute to the temporal-prediction
  breakthrough, palette `cosmos` + charset `binary`). `cosmic_dragon`
  (underscore) is the new rain STYLE scene (Dragon-style serpentine
  chain, palette `nebula` + charset `zen`). Different visual
  concepts, different rain styles, different palettes.
- A/B 10s @ 80x24 dry (no regression on existing styles): cinematic
  24K fps / 416 dirty / entropy 5.09 / gini 0.66; monolith 90K / 57
  / 3.29 / 0.90; vortex 113K / 41 / 4.77 / 0.70; ripple 15K / 503 /
  5.53 / 0.55; cosmic_dragon 368K / 9.3 / 2.50 / 0.92 — the dragon
  is the fastest style (368K fps, fewest dirty cells) with a distinct
  visual signature (lowest entropy, highest gini = most concentrated
  serpentine chain). No regressions on the other four styles.

### feat: NIGHT-research-4 merge — `lorenz` strange attractor lands on main; five rain styles

Branch `night-research-4/lorenz-strange-attractor` merged into `main`.
The merge is divergent: task-19 had already replaced the
owner-rejected `ripple` with `flux` on main while the branch replaced
it with `lorenz`, so the union keeps BOTH styles — the catalog is now
five rain styles (glyph cascade, monolith pillars, vortex polar
orbits, flux PIC/FLIP liquid, lorenz strange attractor) and 21 scenes
in the interactive cycle (flux at position 5, lorenz at 6). Union
resolution across the style registry (`RainStyle` enum + family
helpers), the scene catalog (`SCENE_ORDER` renumbered 6-21), the
dispatch chain (`rain_at`/`spawn`/`scene_runtime`/`runtime_controls`
arm unions), the style constants (`FLUX_*` and `LORENZ_*` blocks
coexist in style_rain.rs), the architecture diagram and the mirrored
test tree (`tests_flux` + `tests_lorenz` directories, rename/rename
conflict resolved to keep both). The full-lap and scene-name pin
tests adapt to 21 scenes; the x-cycle test now walks four hops
through both new flagships.

### feat: NIGHT-research-4 — `lorenz` (strange-attractor rain), the fifth style

The owner rejected the `ripple` style (water-surface rings + splashes,
shipped in task-18 commit 0dfdc24) for not being unique or
masterpiece-grade; task-19 had already replaced it with `flux` on
main, and this branch adds `lorenz` — the project's first
strange-attractor renderer and the rarest rain-style engineering in
any terminal matrix-rain project, a real chaos-mathematics
masterpiece.

- `lorenz` (scene `lorenz`, palette `cosmos` + charset `binary`):
  glyphs ride trajectories of the canonical Lorenz strange attractor
  (sigma=10, rho=28, beta=8/3 — the foundational chaotic system
  published by Edward Lorenz in 1963 that gave the "butterfly effect"
  its name). Integration is classical fourth-order Runge-Kutta (RK4),
  chosen over Euler because the Lorenz vector field is stiff near the
  lobe crossings and Euler drifts visibly within seconds. RK4 keeps
  trajectories on the true attractor for the mote's full lifetime.
- Motion DNA — 100% distinct from cascade (`cinematic`), pillars
  (`monolith`), and polar-orbit (`vortex`): each mote is a glyph
  carried by a 3D chaotic trajectory projected to 2D, with z mapped
  to brightness depth (lobe peaks hot, saddle transitions dim). The
  attractor's two lobes (positive x = right lobe, negative x = left
  lobe) are projected to the terminal's two halves; spawns alternate
  lobes for symmetric coverage. Motes spawn at the classic textbook
  initial condition (±1, ±1, 1) — well inside the saddle region's
  unstable manifold, immediately entering the chaotic flow. A small
  per-mote perturbation (±2.0) preserves the butterfly effect (two
  motes seeded identically diverge visibly over a few seconds —
  sensitive dependence on initial conditions).
- Masterpiece engineering / future-proof legacy: this file is the
  project's first strange-attractor renderer. The architecture (RK4
  step + derivative function + project + diff cleanup) is
  attractor-agnostic — swapping the Lorenz derivative for Rössler,
  Aizawa, Thomas, or Chen is a single function replacement (each is
  a 3D ODE the same RK4 integrates unchanged). The pattern sets a
  reusable standard for future attractor styles.
- Architecture: `RainStyle::Lorenz` variant added beside task-19's
  `RainStyle::Flux` (the `RainStyle::Ripple` removal and
  `cloud/ripple.rs` deletion were already done by task-19);
  `cloud/lorenz.rs` (~560 LOC, mirrors vortex's structure). The
  family helpers are retuned: `is_droplet_family` is Glyph-only
  (lorenz is fully structured, unlike ripple which was structured-
  surface but droplet-family); `uses_spawn_remainder` covers
  Monolith + Vortex + Flux + Lorenz. `rain_at` style gates extended
  to the fourth structured family; scene catalog grows to 21 scenes
  (lorenz lands at cycle position 6, after flux at 5).
- A/B 10s @ 80x24 dry (after implementation): cinematic 23.6K fps /
  421 dirty / entropy 5.11 / gini 0.656 (no regression);
  monolith 92K / 57 / 3.29 / 0.896 (no regression); vortex 113.5K /
  41 / 4.77 / 0.697 (no regression); lorenz 113.6K / 30 / 4.39 /
  0.763 — lorenz matches vortex's structured-family performance
  profile (113K fps, 0.013ms p99) with a distinct visual signature
  (entropy between vortex and cinematic, gini between vortex and
  monolith). No regressions on the other three styles.

### feat: v100.0.0-nightly.1 — rain style 4 replacement: `flux` liquid matrix (PIC/FLIP incompressible fluid) supersedes the owner-rejected `ripple` style (task-19, owner-approved 2026-09-05)

The task-18 `ripple` water-surface style was rejected by the owner on
visual review (commit 0dfdc24): "not unique and masterpiece". Task-19
replaces it with the rarest motion DNA available: a real fluid solver.

Rarity verification (2026-09-05, web-audited): the intersection of
"matrix rain" and "real incompressible Navier-Stokes solver" is empty
across the entire ecosystem — cmatrix, unimatrix, tmatrix, the Python/
WebGL/bash remakes and the screensaver ports are all plain column
cascades, and the existing fluid-simulation projects (Unity, GPU
shaders, tutorial code) are standalone. No matrix rain renderer has
ever shipped a CFD projection in its render path. cosmostrix is first.

- `flux` (scene `flux`, palette `ocean` + charset `minimal`): the code
  rain falls through a living incompressible liquid. Every simulated
  tick runs a full PIC/FLIP particle-grid hybrid pipeline — the
  algorithm family film-VFX fluid solvers use (PIC 1957 / FLIP 1986,
  Zhu & Bridson 2005 lineage), shrunk to terminal scale:
  1. P2G: each glyph is a fluid particle splatting its momentum
     bilinearly onto a half-resolution screen-space velocity grid.
  2. Gravity on weight-carrying nodes (the fluid exists where the
     glyphs are — calm regions stay calm).
  3. Pressure projection: divergence, Jacobi Poisson solve
     (4 iterations, Neumann boundaries), gradient subtraction — the
     incompressibility constraint of the Navier-Stokes equations.
     Falling jets push neighboring fluid aside, shear layers curl
     into eddies: emergent Kelvin-Helmholtz structure, never scripted.
  4. G2P: the FLIP/PIC hybrid readback (0.9 FLIP preserves particle
     energy and detail, 0.1 PIC damps numerical instability).
- Visual identity: brightness maps particle speed (Doppler-style flow
  visualization — hot jets, dim eddies, ghost drift); comet trails
  (3-cell); matrix-style glyph mutation on cell crossing. The minimal
  charset renders the whole scene as falling nabla ∇ glyphs — the
  gradient operator the projection step literally computes. Every
  style occupies a distinct point in visual-metric space (below).
- Determinism and rate independence: fixed-step accumulator
  (FLUX_SIM_DT = 1/60 s, capped 2 steps/frame, backlog dropped on
  slow terminals — anti-teleport). The benchmark's uniform stepping
  integrates exactly one solver step per frame; 144 Hz terminals run
  identical 60 Hz physics; the resume easing slows the accumulator
  growth so an unpause wakes the liquid in cinematic slow motion.
- Architecture: `RainStyle::Flux` replaces `RainStyle::Ripple`
  (structured family — droplet family is now Glyph-only; spawn
  remainder covers Monolith | Vortex | Flux). New
  `cloud/flux_field.rs` (361 LOC — the reusable solver platform:
  P2G, projection, G2P sampling, wall/open boundaries, zero per-frame
  allocation via ping-pong pressure buffers) and `cloud/flux.rs`
  (669 LOC — mote pool, spawn accumulator, fixed-step advance, draw plus
  monolith three-pass diff cleanup). `cloud/ripple.rs` (528 LOC) and
  its surface hooks in `rain_at`/`spawn_logic` are removed; the
  `RIPPLE_*` style constants are replaced by the `FLUX_*` set in
  style_rain.rs.
  Scene catalog stays 20 scenes (`x`-cycle: cinematic -> monolith ->
  matrix -> vortex -> flux -> classic -> ...).
- A/B 10s @ 120x40 truecolor (baseline 0dfdc24 vs after): cinematic,
  monolith and vortex all noise-equivalent (entropy/gini identical to
  2-3 decimals, fps within run-range). Flux signature: 32,758 fps
  (structured class, 6.5x the rejected ripple style's 5,080), 102
  dirty cells/frame — the LOWEST of any style (fluid particles move
  coherently with the flow, so the diff engine barely works), entropy
  5.73, gini 0.626, drift +0.30%. Solver cost: ~0.006 ms/frame.
- +20 contracts (tests_flux, replacing tests_ripple's 8): scene
  resolution + cycle order, spawn density ramp to steady state,
  fixed-step determinism (4 steps / 5 frames; 500 ms stall caps at 2
  and drops backlog), gravity speed-key scaling, net-sinking majority,
  bottom-exit recycling, drawn-cell bounds, frame-stream liveness,
  style transitions both ways, active-count routing, solver numerics
  (P2G splat identity, momentum averaging, vacuum calm, gravity
  impulse survival, divergence halved by projection — THE
  incompressibility contract, wall no-through-flow, out-of-range
  clamping, non-finite splat rejection), plus compile-time pins.
- Gates: cargo fmt clean; clippy -D warnings clean; 2,323 tests
  passed; build.sh check-all EXIT 0; gate-keepers all-installed
  checks green.

### feat: v100.0.0-nightly.1 — rain styles 3 + 4: `vortex` (polar-orbit galaxy drain) and `ripple` (water-surface rain) (task-18, owner-approved 2026-09-05)

Third and fourth rain styles — different motion DNA from both existing
styles (the cascade and the pillars), landed AFTER the task-17 emission
fix so both inherit Color16/256 quantization on the wire (PTY-probed:
mode 16 emits classic `3x`/`9x` only, mode 256 emits `38;5;N` only).

- `vortex` (scene `vortex`, palette `cosmos` + charset `zen`): glyphs
  spiral inward on Keplerian orbits — angular speed ∝ 1/radius gives a
  constant cells/sec along every orbit (majestic rim at ~8s/rev, ~1
  rev/s near the core). Three slowly-precessing spawn-arm concentrations
  shear into living spiral arms via differential rotation; motes are
  absorbed at the event-horizon core and respawn at the rim. Comet
  trails (4-cell) dim one brightness step per cell; matrix-style glyph
  mutation fires when a head crosses into a new cell.
- `ripple` (scene `ripple`, palette `ocean` + charset `matrix`): the
  glyph cascade falls onto a virtual water surface 3 rows above the
  bottom. Droplet `end_line` is capped above the surface (region
  contract: droplet fall / splash rise / ring zones are disjoint by
  construction); each surface impact opens an expanding edge-on ripple
  ring (sqrt ease-out opening, cps-scaled) plus 2-4 ballistic splash
  hops, with a deterministic hash-positioned surface shimmer keeping
  the plane perceptible between impacts. **(NIGHT-research-4: this
  style is owner-rejected and replaced by `lorenz`; the entry is kept
  for historical reference.)**
- Architecture: `RainStyle` gains `Vortex`/`Ripple` + family helpers
  (`is_droplet_family` — Glyph + Ripple share the droplet pool and
  phosphor Pass 2; `uses_spawn_remainder` — Monolith + Vortex). New
  `cloud/vortex.rs` (560 LOC) and `cloud/ripple.rs` (528 LOC) follow
  the monolith drawn-cell diff-cleanup pattern; `rain_at` style gates
  extended; scene catalog grows to 20 scenes (`x`-cycle: cinematic ->
  monolith -> matrix -> vortex -> ripple -> classic -> ...).
- Both new systems reset fully on style exit (stricter than monolith's
  historical draw-history-only exit — dormant-state-proof for future
  style-agnostic readers).
- A/B 10s @ 120x40 truecolor (baseline 7df626f vs after): cinematic and
  monolith noise-equivalent (no regression). New signatures: vortex
  42,456 fps / 277.9 dirty cells / entropy 6.307 (highest of any style)
  / gini 0.468 (most even coverage) / drift +0.8%; ripple 5,080 fps /
  1,248 dirty / entropy 6.211 / gini 0.529 / drift +3.8% — every style
  now occupies a distinct point in visual-metric space.
- +18 contracts (tests_vortex + tests_ripple): scene resolution, spawn
  density target, inward convergence, core absorption, drawn-cell
  bounds, Kepler bound (compile-time const pin), style transitions
  both ways, water-line geometry, droplet end-cap, impact hooks, ring
  expiry, region-contract pins, live frame streams.

### fix: v100.0.0-nightly.1 — Color16/256/mono emission quantization: the rain renderer now honors the resolved color mode on the wire (task-17, owner-approved Step 1, 2026-09-05)

Defect (found in NIGHT-research-2's PTY probe, owner-approved fix):
the rain render path computed every color in RGB and the SGR emission
boundary formatted all of them as `38;2;R;G;B` truecolor regardless of
the session's resolved color mode. A `--color-mode 16` session on an
80x24 PTY emitted 12,470 truecolor SGRs and 0 classic sequences in
2.5s; terminals that resolve Color16 or Color256 (linux console, old
VTE, tmux without Tc) drop `38;2` entirely — palette identity was lost
and the documented Color16 wire contract (`\x1b[3Nm`, capability table
in output/mod.rs) was violated. The palette construction had quantized
correctly all along; the defect was purely at the emission boundary
(shaded cells miss the ColorCache, whose fallback formatter — and whose
build-time entries — also decoded named 16-colors back to truecolor).

Fix — quantization at exactly that boundary, nothing upstream moves:

- New `engine/chroma_dragon_engine/palette/quantize.rs`: `SgrMode`
  (inferred from the palette a ColorCache was built from — the palette
  already encodes the session mode, so no new state flows through the
  event loop), exact OKLab-nearest searches over the xterm-256 palette
  (240 candidates, indices 16..=255) and the canonical xterm base-16
  table, and a memoized `SgrQuantizer` (flat HashMap keyed by packed
  RGB; rain shading produces only a few thousand distinct colors per
  session, so the 240-candidate scan runs once per new color).
- `Terminal` and `BenchIoWriter` hold one quantizer whenever the
  session is not truecolor; `emit_sgr` quantizes (fg, bg) BEFORE the
  cache lookup and the on-the-fly fallback. Truecolor sessions hold no
  quantizer — the default wire path is byte-identical to before
  (A/B: 4 interleaved 10s monolith runs each side — entropy 4.838/4.839
  identical on both sides, gini and color-transition bands overlap,
  fps +0.5% in the fix side's favor, within run-range overlap).
- `sgr_format::write_sgr_colors_buf` formats named base-16 colors as
  their classic codes (`30-37`/`90-97` fg, `40-47`/`100-107` bg).
  Previously named colors were skipped entirely — a cache-miss cell
  with a named fg emitted a bg-only escape (no foreground at all).
- `ColorCache` entries are now built through the quantizer in the
  palette's own wire space: Color16 caches classic sequences,
  Color256 caches `38;5;N`, Mono caches `97;49` (bright-white on
  default). The duplicated build-time formatters in color_cache.rs
  were removed — one source of truth for the wire format.
- Palette quantization quality: `rgb_to_ansi256` moves from the
  rounded cube-division + cube-vs-gray RGB-Euclidean heuristic to the
  exact OKLab nearest; `rgb_to_color16` moves from a 16-entry ad-hoc
  VGA table to the canonical xterm base-16 values. The known
  RGB-Euclidean failure — dim blue (0,0,100) mapping to Black
  (invisible on the black canvas) — resolves to DarkBlue under OKLab.
  An anti-collapse floor backs this up: visibly-lit inputs (OKLab
  L >= 0.15) never quantize to Black in Classic16 mode.
- benchmark writer mirrors the production boundary: `--color-mode 16`
  and `--color-mode 256` benchmark runs now emit the wire format those
  sessions really produce (classic codes / indexed), so the I/O
  signature and per-frame byte counts reflect reality instead of
  truecolor bytes.

Live verification (PTY probe, 80x24, 2.5s, TERM=xterm-256color):
`--color-mode 16` now emits 24,408 classic `3x`/`9x` sequences and 0
truecolor (was 12,470 truecolor, 0 classic); `--color-mode 256` emits
12,543 `38;5;N` indexed and 0 truecolor; `--color-mode 24` unchanged.
Byte side effect: a 16-mode session now writes ~42% fewer ANSI bytes
than truecolor (399 KB vs 687 KB captured in the same probe window)
— shorter classic sequences are also a bandwidth win on slow links.
Benchmark signature change (honest, disclosed): 16/256-mode runs now
show the wire-correct emission; visual metrics stay in-family
(monolith 10s: entropy 4.840/4.838/4.838, gini 0.8068-0.8073 for
16/256/24; color-transition delta 125.09/96.22/97.59 — Color16 jumps
farther between its 16 discrete colors, Color256's OKLab-nearest
transitions track truecolor closely).

Known remaining (documented, out of Step-1 scope): the HUD overlay and
intro surfaces draw through crossterm's own queue and still emit
whatever crossterm chooses for their colors (1 stray indexed SGR
observed per 16-mode session vs 24,408 classic ones from the rain
path); the dry-benchmark `ansi_bytes_per_second` remains the
disclosed 19-bytes/cell truecolor-based estimate (v50 Issue 3 basis
note) — wet I/O (`--bench-scene production-draw`) is where real bytes
are measured.

Gates: cargo fmt clean; clippy -D warnings clean; 2292 tests passed
(+24 new task-17 contracts: OKLab round-trips, anchors, anti-collapse,
luminance monotonicity along hue-stable ramps, SgrMode inference,
memo stability, wire-format sweeps through the quantizer, ColorCache
entry classes per mode); build.sh check-all EXIT 0; gate-keepers 15/15.

### research: v100.0.0-nightly.1 — color space master research: OKLab confirmed peak, alternatives documented-and-rejected (NIGHT-research-3, owner hunt 2026-09-05)

Owner question: "besides OKLab/chroma dragon, what other color science
is the most valuable for cosmostrix peak? If already peak, skip and
document why OKLab is the primary." Verdict: peak — no code change to
the color engine; new docs/research/COLOR_SPACE_MASTER_RESEARCH.md
locks the rationale with measured evidence:

- Gamut mapping (the one candidate an external review rates "highest
  value") measured instead of guessed: a Python replication of the
  production gradient math (benchmark/research/oklab_gamut_probe.py,
  same matrices, polar lerp, 9 steps) compares the shipped per-channel
  clamp against a CSS-style chroma-reduction gamut map on the real
  catalog stops. Blue/Ocean/Cosmos deviate at most 0.33° hue (invisible
  under 8-bit quantization); Rainbow's 3.69° on 4/9 samples is baked
  into the hand-approved look — swapping the clamp would re-shade a
  locked theme. Revisit trigger documented: custom-palette users
  reporting muddy midpoints on saturated opposing-hue stops.
- Wide-gamut P3 corrected from "medium value" to not-actionable: SGR
  38;2 is sRGB by spec and no escape sequence requests P3 for text
  cells.
- Alternatives table (CIELAB blue curvature in cosmostrix's
  blue/cyan heartland, CIELUV, JzAzBz/ICtCp HDR-tuned, CAM16-UCS
  viewing-condition dependence, HSL/HSV, Okhsl/Okhsv picker-only,
  Oklch = already implemented as the polar path, linear sRGB for
  additive-only) with concrete rejection reasons.
- Round-trip exactness verified one-off exhaustively: a numpy f64
  replication of the OKLab transform pair round-trips all 16,777,216
  sRGB colors with max channel error 0 (the shipped f32 path is
  grid-tested at <=1 LSB, the documented f32->u8 rounding floor).
- The doc also records the architecture argument: perceptual science
  at palette-build time, integer stop-index math on the hot path
  (hue drift is an integer offset, Bayer 4x4 dither, palette-relative
  floor) — the placement is the design win, already shipped.

Docs + one benchmark/research probe script only; render loop
untouched, no A/B benchmark applicable. Gates: gate-keepers 15/15
locally, ruff clean on the new script, codespell clean.

### fix: v100.0.0-nightly.1 — six red CI checks repaired: shfmt canonical refresh, ruff findings, cross-target cfg warnings (CI repair, owner hunt 2026-09-05)

All six failing checks on the 2026-09-04/05 pushes (Build windows /
linux-aarch64 / macos / android, Gate-keepers, Project lint) traced
to four independent causes, none in the render loop:

- Space-indentation artifact: a whole-file rewrite rendered tab
  indents as 8 spaces, and every locally-run gate silently skipped
  shfmt (binary not installed) — so gate-keepers.sh (commit 927658a,
  the comment-style check wiring) plus five more scripts touched by
  the NIGHT-hunter tasks (b99800b, dd0046f) drifted to
  non-canonical formatting. Landed the documented remediation: one
  `shfmt -w scripts/*.sh` refresh under the CI-resolved shfmt
  v3.14.0 (whitespace-only except one semantically identical
  compound-command expansion in check-rs-loc.sh; 755 modes
  preserved).
- ruff 0.16.6 (CI resolves latest, unpinned by owner policy): three
  lint findings fixed — PIE810 twice in check-comment-style.py
  (tuple `startswith`), FURB122 in nh2_pty_harness.py
  (`f.writelines`), plus the same file's `ruff format` drift.
- Cross-target `-D warnings` errors invisible on a linux x86_64
  host: info.rs imported `eprintln_safe` ungated while every call
  site sits in the x86_64-only check_cpu_features (unused-import on
  all three aarch64 CI builds — now cfg(target_arch)-gated), and
  main.rs's fatal-render `let mut msg` is mutated only by the
  cfg(unix) ENXIO hint (unused-mut on the windows CI build — now
  `#[cfg_attr(not(unix), allow(unused_mut))]`, comment compressed to
  hold main.rs at exactly 800 LOC).
- Local gate gap closed so this class cannot recur silently:
  build.sh's run_cross_platform_check ran a bare `cargo check` per
  target — warnings exit 0 locally but are errors under CI's
  RUSTFLAGS=-D warnings (the f19470a6 lesson's warning arm). The
  check now carries the same strictness and the reproduce hint
  includes the flag. The local environment additionally runs the
  exact CI tool versions (shfmt v3.14.0, ruff 0.16.6, codespell
  2.4.3, shellcheck 0.10.0) so checks 1c/6b execute instead of
  warn-skip.

Verified: all five CI cross targets plus windows-gnu pass
`RUSTFLAGS='-D warnings' cargo check` (bare and COSMOSTRIX_BUILD
envs); gate-keepers 15/15 locally with shfmt/ruff/codespell/
shellcheck executing; build.sh check-all green (2268/2268 unit
tests); script file modes unchanged. No A/B benchmark: zero
render-loop changes (error-path attribute, import gate, python
tooling, shell whitespace, build-script check strictness).

### fix: v100.0.0-nightly.1 — chroma dragon survives COLORTERM-stripped sessions; truecolor-native TERM hints (NIGHT-research-1, owner hunt 2026-09-05)

Owner question: "does the chroma dragon enter the benchmark when the
user runs `cosmostrix --benchmark`, and is there an auto fallback to
legacy when the OS/terminal cannot use it (tty, non-terminal,
unsupported terminal, new unknown terminal)?"

Answers, verified live on the debug binary:
- YES, the chroma dragon is IN the benchmark: benchmark mode renders
  every cell through the same `is_chroma()` branches the interactive
  loop uses (droplet/draw.rs, rain_post.rs, phosphor.rs, ...); only
  Crystal Dragon palette drift is forced off (p99 determinism). The
  report's CONFIG block already discloses `color_pipeline:
  chroma_dragon` + `chroma_in_benchmark`.
- YES, the auto fallback works and stays conservative: tty
  (`TERM=linux`), non-terminal (unset TERM), unknown terminals, and
  256-color-only terminals all resolve `legacy_rgb` with a disclosed
  disable_reason (locked by 12 new tests).

THE GAP the hunt found (and fixed): sessions where `COLORTERM` is
stripped in transit — SSH without `SendEnv COLORTERM` (the default),
`sudo -s`, terminal versions that never set it — degraded
truecolor-NATIVE terminals to Color16 -> legacy_rgb. `TERM=alacritty`,
`xterm-kitty`, `xterm-ghostty`, `wezterm`, `foot`, `contour` (with no
COLORTERM) all rendered the flat legacy look, losing OKLab gradients,
climate post-FX, and halos — the inverse of the owner directive
"chroma dragon first -> fallback legacy rgb/srgb".

Fix: `termdetect::hosts::TRUECOLOR_TERM_HINTS` (case-insensitive TERM
substring table, mirroring `HIGH_PERF_TERM_HINTS` semantics) wired into
`cli::detect_color_mode_from_terms` (rain pipeline) and
`output::detect_color_capability` (branding/UI colors) so both
surfaces agree. Deliberately conservative entries only — terminals
truecolor by construction; `xterm`/`screen`/`tmux`/`st`/`vte` (VTE and
tmux >= 3.2 set COLORTERM themselves), Apple Terminal/iTerm2
(TERM_PROGRAM-identified, TERM=xterm-256color), and `rio` (3-letter
substring false-positive risk) are deliberately absent, so the
conservative fallback for every non-identifiable terminal is
untouched.

Startup-only detection change — the steady-state render loop is
untouched, so the A/B visual benchmark is not applicable (a
COLORTERM-truecolor session resolves the identical pipeline before
and after; verified noise-equivalent). Gates: fmt clean, clippy
-D warnings clean, 2270/2270 unit tests (12 new NIGHT-research-1
contracts in `test/engine/chroma_dragon_engine/tests/night_research1.rs`),
build.sh check-all green, gate-keepers 10/10. Docs synced:
TERMINAL_COMPATIBILITY.md (resolution chain + SSH section),
BENCHMARKING.md (which color pipeline the benchmark measures),
runtime.rs ColorPipeline detection-rule doc, output module
capability tables.

### repo: v100.0.0-nightly.1 — gate scripts resynced onto the mirrored test/ tree (NIGHT-hunter-5, owner mandate 2026-09-05)

The NIGHT-hunter-1 test relocation moved 138 .rs files (46 K LOC) from
inline `#[cfg(test)]` modules into the mirrored `test/` tree (included
back via `#[path]` attributes), but the scan scripts still described —
and in three cases still scanned — the pre-move world. Resync:

- `check-comment-style.py`: scan set extended from `src/**` to
  `src/**` + `test/**` (git-tracked globs added; docstring updated).
  Verified: 367 files scanned, 0 emphasis markers — the test tree was
  clean, now it is also guarded.
- `check-symbol-only-output.sh`: `find test -name '*.rs'` scan loop
  added (guarded by `[ -d test ]`), header scope comment updated.
  Verified: 413 files checked (was 275), no icon glyphs.
- `stale-hunt.py`: corpus extended to both trees; docstring documents
  the migration-history exemption (test/tests/mod.rs's "Previously
  these were flat files at src/ root" is intentional history). Bonus
  bug fixed while there: the CLI-surface summary printed TWICE (two
  overlapping `print()` calls, one stale) — now one line that also
  reports the file count. Verified: 367 files, stale FILE PATHS 0.
- `check-rs-loc.sh`: scope aligned with the documented policy
  (src/RULES_LOC.md: "All `.rs` files under `src/`, plus `build.rs`")
  — build.rs (795 LOC) now actually scanned; header comments state
  the test/ tree is intentionally OUT of scope (cap governs
  production source only).
- `build.sh`: stale Miri comment fixed — "unittests embedded in
  src/*.rs modules" now reads "declared from src/ modules, with many
  module bodies living in the mirrored test/ tree via #[path]
  includes".
- `gate-keepers.sh`: header descriptions for guards 11/12 updated to
  the new scan surfaces.
- Hunt findings fixed (stale path references left behind by the
  NIGHT-hunter-1 move, surfaced by the extended scanner):
  `test/engine/chroma_dragon_engine/cloud/tests/tests_scene/mod.rs`
  and `src/config/live_config_trace.rs` +
  `src/config/live_config_poll/mod.rs` pointed at `src/tests/loc.rs`
  (now `test/tests/loc.rs`); `test/engine/chroma_dragon_engine/tests/lock.rs`
  and `test/cosmic_dragon_incubator/tests/lock.rs` pointed at
  `src/engine/chroma_dragon_engine/tests/lock.rs` (now the test/ path).
  `src/RULES_LOC.md`'s generated-code exclusion note no longer
  describes a hardcoded exclusion list (the mechanism is the
  self-declaring `LOC_EXEMPT` marker).

Script-only change (comments + scan sets; zero production code
touched), so the 10 s A/B visual benchmark is not applicable. Gates:
fmt clean, clippy -D warnings clean, 2256/2256 unit tests, build.sh
check-all green (check-rs-loc/build.rs included, comment-style 367
files, symbol-only 413 files), gate-keepers 10/10.

### docs+ux: v100.0.0-nightly.1 — fatal pipe/redirect usage cataloged, frame-zero non-tty stdout warning (NIGHT-hunter-6, owner hunt 2026-09-05)

Owner report (verbatim transcript): `cosmostrix | less`,
`cosmostrix | grep test`, and `cosmostrix > test_fatal.txt` all ended
with "[terminal] stdout write failed (broken pipe) — recovered via
/dev/tty, exiting gracefully", the redirect ran 29 s, and the target
file came out as "UTF-8 text, with very long lines (65278), with no
line terminators, with escape sequences" ("dont cat/read that file").
Root cause of the 29 s mystery: the P5 stdout-health probe
(`probe_stdout_health`) only fires every
`FD_HEALTH_PROBE_INTERVAL_FRAMES` = 3600 frames, so a redirected run
dumps full-speed raw ANSI frames into the file for ~30-40 s before the
isatty check synthesizes the broken pipe that ends it — the pipe cases
exit earlier only because the reader dies and the P3 EPIPE recovery
fires. The behavior itself is the documented lifecycle contract (a
ctty session with piped stdout still starts; P3 recovers a dead
reader); the gap was that nothing TAUGHT the user, at the moment of
misuse, what the correct tool is.

Fix (docs + one surgical warning, no behavior change to the matrix):
- New `docs/USAGE_PIPE_REDIRECT.md` — the fatal-usage catalog: all
  three owner scenarios root-caused (P3/P5 mechanisms), the `cat`-the-
  dump-file hazard (RIS/DECSET replay can clear/resize/recolor the
  live terminal), the additional fatal variants found in the hunt
  (`| tee log` double-garbage, `nohup cosmostrix &` silently becoming
  the file-dump case, `setsid`/headless ENXIO fast-fail as the
  handled-by-design contrast), the safe patterns (`--benchmark` for
  pipelines, `--doctor`/`--dump-config`/`--docs` for text,
  `-v 2> file` while watching), and an exit-code table.
- `run_interactive` now warns at frame zero when stdout is not a tty
  (`watchdog::warn_if_stdout_not_terminal`): one branded stderr line
  naming the correct tool per intent and pointing at the catalog doc.
  Placed BEFORE the alternate screen is entered and before the AB-10
  runtime-warning buffering engages, so it reaches the user
  immediately. Warn-don't-refuse on purpose — refusing would break the
  documented matrix row where `| less` renders the rain through the
  pager.
- `TERMINAL_LIFECYCLE_MATRIX.md`: new row 15 + section 15 (ctty +
  piped/redirected stdout), section 12's piped-stdout paragraph
  refreshed to point at the full contract.
- README Limitations: "Interactive mode is not pipe-friendly" bullet;
  KNOWN_ISSUES.md: redirect-dump section (symptom/hazard/workaround).

Verified live on a PTY (`script`): the frame-zero warning appears on
stderr in both the `> file` and `| head -c` reproductions; the
redirect dump reproduces the owner's exact `file(1)` signature
("very long lines, no line terminators, with escape sequences").
Hunt bonus found while verifying: reader death lands in one of two
panic-free layers — mid-loop EPIPE hits the P3 recovery (exit 0,
owner transcript), while a reader that dies during setup/intro
propagates one branded `error: Broken pipe (os error 32)` (exit 1);
the catalog documents both. Gates: fmt clean, clippy -D warnings
clean, 2256/2256 unit tests (1 new: warning actionability + ASCII
contract), build.sh check-all green, gate-keepers 10/10. Steady-state
render path untouched (one isatty call + one stderr write, both before
frame 1), so the 10 s A/B visual benchmark is not applicable.

### robustness: v100.0.0-nightly.1 — --check-update survives curl-less systems via wget fallback (NIGHT-hunter-7, owner hunt 2026-09-05)

Owner suspicion: "what if the OS doesn't have curl?" — verified and
closed. Previously `Command::new("curl")` failing with `NotFound`
produced a dead-end "curl is not available on PATH" error (graceful
exit 2, never a panic, but useless on Alpine/busybox, minimal
containers, hardened and older systems). `src/platform/update.rs` now
implements a two-step fetcher strategy: curl first (unchanged argv
contract — `--silent --max-time 15`, GitHub JSON accept + UA, trailing
`--write-out "%{http_code}"` status line for exact 403/404
classification), falling back to `wget -q -O - -T 15` (the
busybox/GNU flag intersection, so Alpine works unchanged) when curl is
absent from PATH — `wget -q -O -` only exits 0 on success-class
responses, so the exit status carries the failure class (exit 8 = the
server answered 4xx/5xx; busybox collapses to 1). When neither tool is
installed, the error is actionable: it names both accepted tools and
the manual releases URL
(`https://github.com/oxyzenQ/cosmostrix/releases/latest`) instead of
a dead end. No dependency added (still std `Command`, no shell, no
auto-download — the SECURITY_AUDIT network contract is unchanged).
Documented trade-off: GNU wget has no portable total-time cap
(`--tries`/`--waitretry` are not busybox-portable), so the fallback is
bounded by `-T 15` per attempt instead of a hard 15 s cap; it only
runs when curl is absent and the check is an explicit Ctrl-C-able
user action. Verified live with stub fetchers: curl exit 6 reports
"DNS resolution failed", curl-absent + wget-present reports the
version delta (exit 0), and both-absent reports the actionable error
(exit 2). Tests: 5 new (curl argv contract, wget argv contract with a
GNU-only-flag guard, wget exit classification, no-fetcher message
actionability, curl status-line parsing); docs synced:
SECURITY_AUDIT.md (network + spawn sections), SYSTEM_REQUIREMENTS.md
(not-required network note).

### stability: v100.0.0-nightly.1 — NIGHT-hunter-2 "glitch rain shift" root-caused and eliminated (owner hunt 2026-09-04)

Owner report: periodic "rain shifts/glitches for a few seconds then
normal" on every terminal (Alacritty included — terminal-independent),
within the first minute of a fresh session, absent while the CPU was
busy with a build, and two instances started together glitched at the
same second. Introduced by S-master-HUNT-23 (the output drain backoff)
and still present after HUNT-24/25.

Reproduced and measured on a rate-limited PTY harness
(`scripts/nh2_pty_harness.py`, emulating a real terminal's drain rate):
at marginal drain the drain-loop's write-latency overshoot strobes raw
`perf_pressure` 0.0 to 1.0 with a ~1-2 s period, and every VISUAL
consumer of that raw signal strobes with it — worst offender: the
phosphor decay pass's pressure-skip hysteresis (0.50/0.70) skip/resumed
**11 times in 60 s**; each resume re-rendered the entire aged afterglow
set at once (frames ballooned to 2-6x normal; the mass repaint
re-saturated the pipe, re-arming the spike — a self-exciting loop).
Spawn-scale bands, the glitch gate (0.35), and the sim-delta cap
(clamped droplet clocks into lag-then-catch-up wobble) flapped on the
same waveform.

- New `PowerManager::visual_pressure`: an EMA of effective pressure
  (time constant `VISUAL_PRESSURE_EMA_TAU_SECS` = 2.5 s, wall-clock
  based, frame-rate independent, 250 ms per-step dt cap for stalls).
  One helper, `applied_visual_pressure(power_dragon)`, feeds BOTH
  visual consumers: the cloud pressure feed (spawn scale, phosphor
  decay ramp + skip hysteresis, glitch gate, atmospheric gate, CRT
  vignette) and the sim-delta cap. Control-side consumers (drain
  pacing, self-healer, P5 health, effects congestion gate) keep the
  raw fast-attack signal unchanged.
- Verified: marginal-drain reproduction 11 phosphor strobes -> **0**
  over 60 s; saturated-drain stress 5 MB/s: frames >150 KB 149 -> 90,
  gap p99 54 -> 44 ms, gaps >50 ms 47 -> 25. Cadence/throughput
  unchanged (86.0 -> 83.9 fps avg, noise).
- Hunt bonus (contract hole): the sim-delta cap read raw
  `effective_pressure()` UNGATED — with `power-dragon = false` it
  could still slow droplets below configured speed, violating the v80
  Option D promise ("rain stays at user-configured density/speed
  regardless of CPU pressure"). Now gated with the same helper.
- Hunt bonus (regression): the ungated `libc::ENXIO` reference in
  main.rs (task-6's headless tip) broke the `x86_64-pc-windows-gnu`
  cross-check; now `#[cfg(unix)]`-gated (the task-6 commit only ran
  the light fallback gates, not `build.sh check-all`).
- 10 s A/B benchmark (before = a4194a9, after = this tree): noise
  equivalent (avg_fps -0.2%, entropy 5.06 both, gini 0.6669 vs
  0.6653-0.6665, dirty cells 416.6 vs 414.7-416.9).
- Gates: fmt clean, clippy clean (release, all-targets), 2250/2250
  unit tests (7 new EMA contract tests + 1 new cloud-feed gate test +
  the dragon-on test updated to the smoothed-feed contract),
  build.sh check-all green (incl. all 4 CI cross targets),
  gate-keepers 10/10. Docs synced: CENTRAL_CONTROL_POWER_DRAGON.md
  (two pressure clocks + method table + lifecycle diagram),
  HUD.md (`prs:` row), atmosphere.rs PHOSPHOR_SKIP constants, and 4
  pre-existing MD038 lint errors in this file fixed
  (`code span` leading spaces from the task-5 entry).

### robustness: v100.0.0-nightly.1 — broken-pipe panic class eliminated from every reachable output path (hunt follow-up 2026-09-04)

Found while re-verifying the verbose work: Rust ignores SIGPIPE by
default, so a piped reader that exits early (`head`, `jq`, `grep`)
turns the next `println!`/`eprintln!` into a PANIC. Verified live with
three one-command repros, all aborting with exit 101:

- `cosmostrix -v 2>&1 | head -1` (verbose dump, raw `eprintln!` sites)
- `cosmostrix --benchmark ... | head -1` (bench fleet raw writes)
- `cosmostrix --doctor 2>&1 | head -2` (report.rs writer closures)

This is exactly the abort chain the v25 terminal-close coredump fix
documented — but the bulletproof `eprintln_safe!` macro only guarded
post-exit paths, and its doc still claimed "startup stderr is a
healthy TTY" (false whenever the user pipes; the doc note predates the
piped-CLI reality).

- New `println_safe!` macro: the stdout mirror of `eprintln_safe!`
  (write_fmt + discarded error + flush; zero-arg arm for bare
  newlines). Reports silently truncate at the pipe boundary and the
  process exits with its intended code — the standard Unix CLI
  behavior for closed readers. Deliberately NOT the SIGPIPE=SIG_DFL
  approach: default-disposition death would bypass the
  terminal-restore contract and leave raw mode on.
- Every reachable user-facing write converted (~120 sites across 14
  files): the verbose dump (10), the shared report writer closures
  (doctor/docs/list renderers), the whole bench fleet (helpers,
  premium, scale, run_bench, baseline, dispatch), info variant
  warnings, testconf report, signal-handler diagnostics, list
  printers, early returns, update check. Incubator research modules
  (83 sites, compiled-only, zero callers) left as-is.
- The benchmark noop-flag warning block re-rendered through
  `eprintln_warn_labeled` — it hand-rolled a plain `[warn]` prefix via
  raw `eprintln!`, visually inconsistent with every other warning in
  the binary (the `! [auto-fx] ...` family) and not write-safe; now a
  branded `!` label with the same body.
- All three repros now exit 0; `-v | head` exits with the documented
  terminal-failure code instead of 101. Suites: 28/28 + 47/47 + 34/34.
- Gates: fmt clean, clippy clean, 2242/2242 unit tests, LOC caps held
  (premium.rs recompressed to 799).

### repo: v100.0.0-nightly.1 — custom_features stresstest fixtures migrated off the removed base-scene schema (hunt follow-up 2026-09-04)

Hunt follow-up while re-validating the full stresstest fleet after the
verbose work: `custom_features_stresstest.sh` ran 24/34 — 10 failures,
all traced to ONE root cause: the fixtures still encoded the
pre-v80.0.0-beta.2 scene-custom schema (`base-scene = "..."`), a field
that strict validation now rejects ("unknown key ... removed in
v80.0.0-beta.2"). The suite predates the schema change and was never
re-based on it — it could not verify anything about the custom-feature
contract it exists to lock.

- All 9 scene-custom fixtures rewritten to the v80+ six-dimension
  self-contained schema (color|colors-custom, charset|charset-custom,
  fps, speed, density, glitch-level) — verified against the live
  validator before locking.
- Two obsolete cases re-aimed at the CURRENT contract: "missing
  base-scene" → "incomplete scene-custom (missing dimensions) → error"
  (asserts the exact missing-dimension error), "unknown base-scene" →
  "removed base-scene field → strict reject with hint" (asserts the
  v80 migration hint). The vacuous always-pass cases
  (expected-pattern "") for the empty block and the two dual-key
  conflicts now assert the real contract: empty → incomplete error;
  color+colors-custom → color wins, runs; charset+charset-custom →
  charset wins, runs (dual-key priority verified live before locking).
- Suite result: 34/34 PASS (matches the pre-v80 claim in
  `docs/research/Z_MASTER_V2_PRIORITY_AUDIT.md` — the fleet is whole
  again: suggestion 28/28, config 47/47, custom-features 34/34).
- Operational note baked into this entry: the config and
  custom-features suites drive `./target/release/cosmostrix` (fat-LTO
  build) — run `cargo build --release` before invoking them in a fresh
  sandbox; with no release binary every grep-based case fails empty.
- Gates: bash -n clean, full fleet re-run green.

### ux: v100.0.0-nightly.1 — fatal terminal-session error renders once, branded (hunt follow-up 2026-09-04)

Found while verifying task-5 in a headless environment: an unhandled
io::Error out of `run_interactive` rendered TWICE — first a plain
`error: {e}` line written directly by main, then (because `main`
returned the Err) Rust's default main-Err handler printed a second
line in Debug format: `Error: Os { code: 6, kind: Uncategorized,
message: "..." }`. Two renders, two styles, one failure — and the
second was raw Debug noise, violating main.rs's own documented
contract ("never propagating a std::io::Error that Rust would render
as a debug-looking `Error: ...`").

- main.rs: the fatal path now renders ONCE through
  `eprintln_error_labeled` (branded red, `eprintln_safe!` write —
  same bulletproof-write contract as the v25 terminal-close coredump
  fix: write_fmt with discarded errors, no panic chain), then exits
  explicitly with code 1 after the post-exit warning drain.
- Exit code 1 preserved (documented contract:
  TERMINAL_LIFECYCLE_MATRIX.md headless row).
- ENXIO (no controlling terminal — cron, ssh without -t, CI, the most
  common trigger) gains a headless tip pointing at the non-interactive
  modes: `--benchmark`, `--doctor`, `--dump-config`.
- TERMINAL_LIFECYCLE_MATRIX.md row 12 + terminal-setup section updated
  (also fixed a pre-existing ordering inaccuracy: the cleanup burst
  precedes the error line, not the other way around).
- No A/B benchmark: fatal exit path, render loop untouched.
- Gates: fmt clean, clippy clean, 2242/2242 unit tests, stresstest
  28/28, LOC OK (main.rs 798/800).

### ux: v100.0.0-nightly.1 — verbose line format unified to one value column (owner hunt 2026-09-04)

Owner hunt area: the `-v` verbose line format. Two independent defects
verified live:

1. **Ragged label gutter.** `verbose_line` padded labels with `{:<14}`
   — a MINIMUM, not a fixed width — so every label wider than 13 chars
   pushed its value out of alignment. The live startup dump showed
   three different value columns (16/17/18) and the longest labels
   (`chroma_disable_reason:` at 24) drifted to column 24.
2. **Hand-rolled final-state lines.** The post-exit `final runtime
   state` block in `interactive/mod.rs` bypassed `verbose_line`
   entirely: 25 `eprintln_safe!` calls with manual escape injection
   and manual space padding. They rendered `[verbose]` NON-bold (every
   other verbose line is bold), duplicated the format contract by
   hand, and their padding drifted across five different value columns
   (18/19/20/23/24).

- `verbose_line` gutter widened 14 → 18: covers every curated label in
  both dumps (longest:   `chroma_features:` /   `ambient_entries:` /
  `config candidates:` at exactly 18). Labels longer than 18 are a
  naming bug, not a rendering case — documented in the doc comment.
- Four overflow labels renamed to fit the gutter and gain hierarchy:
    `chroma_disable_reason:` → `disable_reason:`,
  `crystal_dragon_secs:` → `cadence_secs:` (indented under
  crystal_dragon — the value text already says "drift cadence"),
  `ambient_snapback_secs:` → `snapback_secs:` (indented under the
  snapback lines), `TERM_PROGRAM_VERSION:` → `TERM_PROG_VER:`.
- All 25 final-state lines converted to `eprintln_verbose` /
  `eprintln_verbose_purple`: bold `[verbose]` prefix, capability-aware
  colors, single 18-column gutter, `format!`-built values — the manual
  `ts`/`purple`/`reset` bindings deleted. Startup and exit dumps now
  render in one visual language, values aligned at column 20 in both.
- Contract locked by a new unit test
  (`verbose_line_aligns_short_and_long_labels_to_one_value_column`):
  a 4-char label and a 15-char label must start their value at the
  same index, exactly 36 = 10 (prefix) + 8 (timestamp) + 18 (gutter).
- Docs: `docs/AMBIENT_SCHEDULER.md` verbose examples re-rendered with
  the new labels/alignment; `--help` ambient blurb re-pointed to
  `snapback_secs`. Historical research/archive docs untouched.
- No A/B benchmark: verbose lines are pre-loop startup diagnostics and
  post-loop exit summaries; the render loop is untouched.

### ux: v100.0.0-nightly.1 — --dump-config write-I/O failure joins the die_input family (owner hunt 2026-09-04)

Owner hunt area: the `die_config` site at the `--dump-config` I/O error
arm. Verified live: a filesystem rejection of a CLI-supplied path
(Permission denied, Not a directory) rendered as a bare one-liner with
no help footer and no next-step tip, while the SAME flag's overwrite
guard — two lines earlier in the same code block — rendered a guided
5-line message with the footer. One flag, two failure shapes; the bare
shape read as a bug, and `die_config` (the config-file failure family,
footer-less by contract) was the wrong family for a filesystem error:
the config itself was valid, the write target rejected it.

- `early_returns.rs`: the `write_config_atomic` Err arm rerouted
  `die_config` → `die_input` (footer family), and the message gains
  guided remedies: verify the directory exists and is writable, the
  retry command line, and the stdout alternative
  (`cosmostrix --dump-config` prints the example config with no file
  write — useful when the disk is full or the directory is locked).
- Stresstest: 2 new cases (26 → 28) locking the guided-error shape and
  the footer. Trigger is deterministic for any user including root: a
  path whose parent component is a FILE makes the atomic write's
  `create_dir_all` fail with NotADirectory — no permission juggling.
- No A/B benchmark: the change touches only a fatal pre-render exit
  path, not the render loop.

### repo: NIGHT-hunter-1 — test files relocated into the mirrored `test/` tree (owner mandate 2026-09-04)

Owner mandate (NIGHT-hunter-1): any file whose name contains `*test*`
lives under the project-root `test/` folder. The tree mirrors `src/`
exactly (`src/A/B_tests.rs` → `test/A/B_tests.rs`), so every file stays
at a collision-free, deterministic location.

- 138 files relocated: 71 leaf test files + 7 test directory modules
  (`test/tests/`, `test/docs_tests/`, `test/config/config_apply_tests/`,
  `test/config/configfile_tests/`, `test/cosmic_dragon_incubator/tests/`,
  `test/engine/chroma_dragon_engine/tests/`,
  `test/engine/cosmic_dragon_engine/cloud/tests/` including its nested
  `tests_scene/` and `tests_monolith/` subtrees). `git mv` preserves
  history and 644 file modes.
- Declaration sites in `src/` keep module identity via house-style
  `#[cfg(test)] #[path = "..."] mod X;` attributes — the relocated
  files remain UNIT tests of the binary crate with full private-item
  access (`use super::*` still resolves), NOT cargo integration tests.
  Test count unchanged: 2241 passed / 2241.
- `src/testconf/` is the single sanctioned exception: it is a production
  runtime module (the `--testconf` flag), not test code — only its
  `tests.rs` and `tests_validation_order.rs` relocated.
- Six `include_str!` back-references and 44 CWD-relative path literals
  inside moved meta-tests (scene-coverage guards reading sibling test
  sources) re-pointed from their old neighborhoods to `src/`/`test/`
  as appropriate.
- Two declaring files crossed the 800-LOC cap from the inserted
  `#[path]` lines; compressed four redundant comment lines to restore
  `src/interactive/mod.rs` (798) and `src/config/mod.rs` (800) to cap.
- src/ drops from 121,463 to 75,165 LOC of scanned production source;
  `scripts/check-rs-loc.sh` semantics unchanged (cap governs `src/`
  only, documented in `src/RULES_LOC.md`).
- Docs: `src/RULES.md` module conventions rewritten for the new layout
  (NIGHT-hunter-1 codified), `src/RULES_LOC.md` test-file carve-out
  updated, living docs re-pointed (`docs/README.md`, `docs/RULES.md`,
  `docs/RELEASE_GUARD.md`, `docs/LIVE_RELOAD_BEHAVIOR.md`, `docs/HUD.md`,
  `docs/workflow/ABOUT_CI.md`, `docs/TERMINAL_LIFECYCLE_MATRIX.md`).
  Historical research/archive audits and prior CHANGELOG entries are
  point-in-time records and were NOT rewritten.
- No A/B benchmark: `#[path]` attributes and file locations do not
  affect codegen; the production binary is behavior-identical.
- Gates: fmt clean, clippy clean, 2241/2241 unit tests, stresstest
  26/26, LOC caps held.

### ux: v100.0.0-nightly.1 — case-insensitive flag-suggestion fallback (owner `--LIS` test report 2026-09-04)

Owner report: `--LIS` rendered tip-less while `--lis` suggests
`--list-scenes`. Root cause: clap's did-you-mean engine compares
case-SENSITIVELY (strsim Jaro, confidence > 0.7) — an all-caps
prefix of a known flag scores zero matching chars and gets no
SuggestedArg context, so the canonical render carries no tip at all.

- New fallback `cli::ux::enrich_unknown_arg_suggestion`, called at
  the top of `exit_clap_error`: when an UnknownArgument error
  carries no suggestion, the typed flag (InvalidArg context) is
  matched case-insensitively against the command's non-hidden long
  flags and the best match is injected as clap's OWN `SuggestedArg`
  context — the tip renders in clap's canonical position and white
  `valid` style, exactly once, with no custom printing and no render
  surgery. No-op for every other error kind, for errors clap already
  suggested (never a second tip), and for short/distant inputs.
- New engine pair in `cli/suggestion.rs`: `jaro_ci` (faithful
  strsim::jaro port, both sides lowercased) and
  `closest_long_flag_ci` (> 0.7 threshold, ties resolve to the LAST
  candidate — mirroring clap's ascending-sort-then-pop so a rescued
  typo suggests the same flag clap suggests for its lowercase twin:
  `--LIS` and `--lis` both point at `--list-scenes`). Safety: for
  lowercase input the scores equal clap's own, and the candidate set
  is a subset of clap's keymap, so the fallback adds signal only
  where clap was structurally silent.
- Stresstest: 3 new cases (--LIS rescues --list-scenes, --HELPSS
  rescues --help, --x stays tip-less) — 26 total, all PASS. Unit
  tests: 4 render-contract tests in cli/ux.rs + 7 engine tests in
  cli/suggestion.rs.

### ux: v100.0.0-nightly.1 — fatal-error footer consistency, config-apply error classification (owner test report follow-up 2026-09-04)

Owner report: testing commit ea05ca00 showed `--scene cosmosm`
ending with NO "For more information, try '--help'." footer while
`-C asciix` (the same error kind: unknown value + did-you-mean tip)
ended with it — the shape inconsistency the CLI UX centralization
missed. Root cause: the whole Err stream out of
`config_apply::apply_config_and_runtime_defaults` flowed through
`ux::die_config` (footer-less config family), but that stream mixes
two error families. The same class of misroute also existed on the
`--show-scene <unknown>` path (early_returns.rs), which additionally
dead-ended without a did-you-mean tip.

- New classifier `cli::ux::die_config_apply_error(e)`: config-file
  failures ("error: invalid config" prefix — malformed lines, unknown
  keys, invalid file values) keep the die_config shape; CLI
  value-validation failures (unknown `--scene` / `--scene-custom` /
  profile names, invalid `--intro-color`) now take the die_input
  shape with the help footer, same as every other typed-flag
  validator. The classification rule (stable message prefix) is
  documented and unit-tested in cli/ux.rs.
- `--show-scene <unknown>` rerouted from die_config to die_input
  (footer gained) and now carries the same did-you-mean tip the
  `--scene` path renders: `scene_suggestion_tip` made pub(crate) and
  shared by list_printers.rs, so `--show-scene cosmosm` suggests
  'cosmos' exactly like `--scene cosmosm`. One unknown-scene message
  shape across every surface.
- Stresstest: 5 new cases (scene typo footer, distant-scene
  footer-without-tip, show-scene tip, show-scene footer, malformed
  config line stays footer-less) — 23 total, all PASS.

### ux: v100.0.0-nightly.1 — CLI UX centralized into cli/ux.rs (owner mandate 2026-09-04, "simple masterclass")

Owner report: CLI UX was inconsistent, untidy, and duplicated across
surfaces — `--test` printed the tip line TWICE and a misleading
"Usage: cosmostrix --testconf"; `-g` printed a bare error with no
usage; different error kinds had different shapes; scattered error
rendering paths made maintenance risky. Refactored into ONE contract
module with a single render path per error family.

- New central file `src/cli/ux.rs` — THE contract module for every
  user-facing CLI error, tip, usage line, and help footer. Holds the
  fatal helpers moved from `output/ux.rs` (die_input, die_config,
  or_exit — re-exported as `crate::ux` so all ~50 call sites resolve
  unchanged), the new clap-error bridge, and the canonical suffixes.
  `src/output/ux.rs` deleted; `cli/` is the central CLI folder.
- `cli::ux::exit_clap_error(e, cmd)` is now the single exit path for
  clap parse errors. Fixes, with structured clap contexts (no string
  parsing):
  1. Duplicate tip: the old main.rs interceptor printed clap's error
     (which already contains the tip) and appended its own
     "tip: a similar argument exists" line scraped from the rendered
     string by `extract_clap_suggestion` — deleted along with the
     scraper (owner's `--test`/`--clr` paste showed the doubled tip).
  2. Misleading usage: clap injects the suggested flag into usage
     generation, so `--test` rendered "Usage: cosmostrix --testconf"
     (reads as if --testconf were required, and diverged from `--clr`
     which showed "Usage: cosmostrix [OPTIONS]" — same error kind,
     two shapes). The Usage context is now replaced with the real
     full usage from `Command::render_usage()` for every error kind.
  3. Shape drift: missing-value errors (`-g`) had no usage line at
     all; no error kind carried the "For more information, try
     '--help'." footer (clap cannot render it: --help is intercepted
     manually for the curated manual, so clap has no Help-action
     flag). Every fatal CLI error now ends: message + tips, real
     usage (structural errors), footer.
- Style harmony in `clap_styles()`: clap's defaults rendered tips
  GREEN, errors plain red, invalid values generic yellow — three hues
  that disagreed with the branded ux path. clap now renders errors
  bold brand red #FF5A5A, tips suggestion white #DCEBFF, invalid
  values warn yellow #FFEB3C — verified byte-identical SGR codes on
  both paths via a PTY harness.
- Suggestion consolidation: `format_value_suggestion` moved to
  cli/ux.rs (presentation); the engine stays in cli/suggestion.rs;
  the last duplicate `edit_distance` copy (config_hints) now imports
  the shared engine. `main.rs` switched to the non-consuming
  `try_get_matches_from_mut` so the Command stays available to the
  error path.
- Pre-clap unknown-flag errors (REMOVED_FLAGS migration hints, -mfs
  typo guard) route through the new `die_input_with_usage` so they
  carry the same usage + footer suffix; CLI flag NaN gates
  (--duration, --crystal-dragon-secs) misrouted through die_config
  now use die_input; the stale "exit 1" doc claim on die_config
  corrected (shipped behavior is exit 2).
- The misleading main.rs comment claiming "--help always works even
  if other flags are malformed" corrected to the real contract
  (clap-level parse errors fire first; making help win would need an
  ArgAction::Help interception — deliberately not done for behavior
  stability).
- Tests: 3 structured suggestion-context tests replace the 5 obsolete
  string-parser tests; 4 new contract tests in cli/ux.rs lock the
  render (real usage never the narrowed form, exactly one tip line,
  footer shape). Stresstests: cli_suggestion 18/18 PASS,
  cli_config 47/47 PASS with the new shapes.

### consistency: v100.0.0-nightly.1 — central_control_dragon_power renamed to central_control_power_dragon (owner mandate 2026-09-04)

Owner report: the module folder name `src/central_control_dragon_power/`
did not match its owning flag `--power-dragon` (word order inverted).
Renamed to `src/central_control_power_dragon/` (word order mirrors the
flag; aligns with the sibling `src/central_control_rains/` family).

- Folder renamed via `git mv` (history preserved); all `mod`/`use`
  paths, doc-comment intra-links, `src/RULES.md` module map,
  `CONTRIBUTING.md` layout table, and live docs updated
  (`docs/CENTRAL_CONTROL_POWER_DRAGON.md` renamed likewise, with its
  link in `docs/CRYSTAL_DRAGON_ENGINE.md` and references in
  `docs/AMBIENT_SCHEDULER.md`,
  `docs/research/V51_2_POWER_DRAGON_AMBIENT_CONTRACT.md`,
  `Cargo.toml` comment).
- Historical records untouched by design: `CHANGELOG.md` past entries
  and `docs/archive/**` keep the old name (they describe the state of
  the tree at their time).
- Pure code motion + reference sync: zero behavior change (same
  symbols, same exports, same tests).

### consistency: v100.0.0-nightly.1 — comment markdown emphasis ban (owner mandate 2026-09-04)

Owner report: source comments across `src/*` still carried
markdown-document formatting (`**test**`-style bold, `*test*`-style
italic) — raw source read like md/mdx pasted into comments. The
2026-08-19 COMMENT_STYLE resolution ("valid rustdoc, keep it") is
superseded by this owner mandate: comments are plain prose.

- Swept 378 decorative emphasis markers (bold, italic, and 5
  multi-line bold spans) across 130 file-passes in `src/**/*.rs`;
  functional rustdoc is preserved (inline code backticks, code fences
  including doctests, links, headings). Content inside doc-comment
  code fences is untouched; asterisks inside inline-code spans
  (`(channel * fi + 128)`) are untouched.
- docs/COMMENT_STYLE.md rewritten to codify the new contract
  (section 2: emphasis banned; section 2.2: plain-prose and CAPS
  warning-label alternatives; section 6: sweep findings).
- New gate: scripts/check-comment-style.py (fence-aware,
  backtick-aware, zero-tolerance) wired into gate-keepers.sh as
  check 12 — the drift cannot silently return.

### harmony: v100.0.0-nightly.1 — S-master-HUNT-25 resync redraws without render-state reset ("glitch rain shift", round 5)

Owner bug report (2026-09-04, post-09759d5): snow-ice fixed (HUNT-22/23
confirmed), but the "glitch rain shift" reproduces on ALL terminals —
including GPU-accelerated Alacritty, the owner's daily driver. Symptom:
after roughly a minute of runtime ("at certain minutes, or simply at 57
seconds from start"), the rain suddenly shifts sideways for a few
seconds, then returns to normal on its own.

- **Audit first (empirical, PTY harness at 200x60, TERM=alacritty)**:
  a 90s timed capture was replayed through a VT emulator with per-frame
  audit. Glyph positions never shift (adjacent-second occupancy-profile
  correlation r>=0.96, cross-correlation lag 0 at every 0.5s step); the
  diff-built screen state and the app's forced repaints are
  content-identical (3/12000 cells, +-1 RGB rounding); density-noise
  re-rolls and column-coherence perturbation are inert at steady state.
  The one measured anomaly: full-redraw BURSTS — 12-18 consecutive
  frames at 211-294KB (vs 107-148KB steady state, i.e. 2-3x) firing at
  t=34.5/45.4/54.5/74.5 in a 90s run, 2.4x the normal visible glyph
  count inside the burst frames.
- **Root cause**: every periodic maintenance redraw — idle resync
  (every 20s of idle), stuck-cell sweep (every 3600 frames), ANSI drift
  redraw (every 18000 frames), plus paste/focus regain — entered the
  `force_draw_everything` branch which called `frame.clear_with_bg` AND
  wiped the whole `phosphor_base_ch` array. That reset the phosphor
  decay state wholesale: thousands of afterglow cells jumped brightness
  classes at once and the following 12-18 frames re-seeded the phosphor
  system, emitting a 3-4.5MB ANSI burst into the pipe. Any terminal
  that cannot drain that instantly stalls the event loop mid-burst and
  visibly tears through the transient — reading as "the rain suddenly
  shifts for a few seconds, then normal again". Terminal-independent
  (pure output-side), landing around the first minute (the 3600-frame
  sweep at real-world effective fps) and at recurring minute intervals
  — matching the owner's timing report.
- **Fix**: resync redraws now set only the repaint flag. New
  `Frame::force_repaint()` sets `dirty_all` WITHOUT clearing cell
  content, bumping the generation, or touching phosphor bookkeeping —
  the draw pass, phosphor decay pass, and stuck-cell `set_force`
  corrections apply exactly as on a normal frame, and the emitted
  content is identical to the screen. `phosphor_decay_pass` Pass 1 now
  prefers the dirty-index scan whenever the dirty list is populated
  (full-grid scan reserved for the genuinely-cleared buffer), so resync
  frames no longer re-seed phosphor energy for every visible cell.
  Monolith keeps its historical state reset (draw history + spine
  phosphor genuinely need rebuilding); real semantic changes still go
  through `invalidate_semantic` with the full clear.
- **Verification (empirical)**: 90s PTY capture with the fix — frame
  size distribution becomes uniform (median 118KB, p99 132KB, max
  133KB vs 297KB max before; zero frames above 180KB vs 40+ before).
  The maintenance redraws are now indistinguishable from normal frames.
- 4 regression tests lock the contract (`tests_resync_hunt25.rs`):
  force_repaint preserves cells + generation; glyph resync preserves
  active phosphor base glyph + decay state; stuck-cell sweep still
  clears through the resync path; monolith force path unchanged.
- Suite: 2226 passed / 0 failed / 2 ignored. Gates: fmt clean; clippy
  --release --all-targets 0 warnings; build.sh check-all PASS;
  gate-keepers 9/9; check-rs-loc OK; perms 644. A/B benchmark 10s:
  noise-equivalent (avg_fps +0.19%, entropy +0.03%, gini -0.01%) — the
  fix is inert in bench mode by construction.

### harmony: v100.0.0-nightly.1 — S-master-HUNT-24 effects auto-gate on CPU-rendered/TTY terminals + foot/konsole high-perf reclassification (VTE/foot stuck, round 4 — strategic)

Owner bug report (2026-09-04, post-36f8620): after HUNT-23, foot and
GNOME/kgx still reproduced the snow-ice spark degradation, and a new
symptom appeared — "glitch rain" visibly drifting for a few seconds
before settling. Owner directive: effects (particles, etc.) must
auto-disable when a pure-CPU/TTY terminal is detected.

- **Audit first**: an empirical PTY harness ran the release binary at
  200x60 under hard congestion (34 KB/s drain) with sustained synthetic
  clicking. The captured 583 KB ANSI stream is cursor-consistent (zero
  wrap-pending violations, zero non-1-width runes in the rain loop) and
  the app's own screen content shows no horizontal drift — the renderer
  is not desyncing. The remaining reproductions are a LOAD problem:
  the effects layer's ANSI volume, run into a CPU renderer that cannot
  drain it.
- **Root cause**: cosmetic effects ran on every terminal regardless of
  renderer class. On pure-CPU terminals (VTE family, konsole, foot at
  fullscreen) the interaction bursts stall the pipe faster than the
  HUNT-23 drain backoff can react (0.05/unit rise), so frames stretch
  past the 250 ms particle anti-teleport cap and the sparks decay in
  giant steps ("snow ice") — and temporal effects (glitch spans, fill
  animations) render at wildly varying frame intervals, reading as
  glitch-drift. HUNT-21..23 fixed the clocks; the pipe was still being
  overfed.
- **Fix (strategy, per the owner's directive)**: cosmetic effects are
  auto-disabled at startup on CPU-rendered and TTY terminals. New
  `TerminalCaps` fields: `cpu_rendered` (detected via `VTE_VERSION`,
  `KONSOLE_VERSION`, `TERM_PROGRAM`/`TERM` foot+konsole hints, xterm.js
  hosts) and `console_tty` (`TERM=linux`/`dumb`), surfaced with an
  `effects_gate_source` string in `-v` verbose output. The gate is
  baked into `CloudConfig.effects_enabled` in build_cloud_cfg (so the
  live-reload rebuild contract from HUNT-3 keeps it off), with a
  `[auto-fx]` runtime diagnostic explaining the decision.
- **foot + konsole removed from the high-perf tier**: both are
  CPU-rendered; the 144 FPS dynamic default they received was 2.4x the
  byte rate a CPU renderer drains at fullscreen — the amplifier behind
  the owner's foot reproduction. They now take the standard 60 FPS
  tier with VTE-class phosphor tuning. Kitty-keyboard support is
  unchanged (protocol support is orthogonal to renderer class).
- **Dynamic congestion gate (safety net)**: for CPU terminals the env
  markers cannot see, the event loop watches `drain_backoff` (HUNT-23)
  and disables effects after 4 s of sustained congestion — sticky for
  the session (no flapping: a disable-enable loop would pulse the
  effects layer on a ~30 s period). Threshold 0.20, timer reset on
  clean frames; inert on `--no-effects` runs.
- **Empirical verification (and a caught wiring bug)**: the PTY harness
  re-ran on the patched binary with `VTE_VERSION` set — and caught the
  gate's first draft ANDing the "effects must be OFF" predicate
  directly into the enable expression (an inverted gate: effects stayed
  ON exactly on CPU terminals; `--no-effects` masked it in every
  unit test because it short-circuits the same expression). The
  resolver is now a named, unit-tested seam
  (`resolve_effects_enabled` — inversion-guard tests included). Final
  matrix on the fixed binary, 200x60 under congestion with sustained
  clicking: VTE env -> `effects_enabled=false`, zero particle glyphs
  in the stream; foot TERM -> same via the TERM-substring layer;
  Alacritty-like env -> effects on, click sparks present (1.7k
  particle glyphs). The rain field renders normally in all three.
- Tests: 15 new (6 termdetect gate detection, 5 static-gate
  predicate/resolver — including the wiring-inversion guard, 4
  dynamic-gate sustain/stickiness/boundary + 1 compile-time constant
  contract moved to a `const _` block). Suite: 2224 passed / 0
  failed / 2 ignored.
  A/B benchmark (10 s, headless): noise-equivalent — the gate is
  inert in bench mode by construction (effects are off there already).
- Docs synced: KNOWN_ISSUES.md (four-layer status, affected-platforms
  rewrite, workaround 4), --no-effects help (AUTO-GATE note), `-v`
  verbose `effects_gate:` line, this entry.

### harmony: v100.0.0-nightly.1 — S-master-HUNT-23 output drain backoff + P2 mitigation congestion guard (VTE/foot stuck, round 3)

Owner bug report (2026-09-04, post-d8d53a1): after HUNT-22 the
particle clock was real-time, yet on foot and GNOME/kgx the effects
still slowed over minutes, froze for a few seconds, then
auto-dismissed. The symptom had to be upstream of particle physics.

- **Root cause (three interlocking defects, all output-side)**:
  1. *Open-loop output pacing.* `effective_fps()` responded to pause
     and idle but never to the terminal's actual drain rate. On
     CPU-rendered terminals at fullscreen (VTE at the 60 FPS default,
     foot at the 144 FPS high-perf default it is classified under)
     the ANSI byte rate exceeds what the terminal drains, the PTY
     buffer fills, and the frame's `flush()` syscall blocks until the
     terminal catches up — freezing input processing and every
     effect with it. Sim-time dilation and the spawn throttle reduce
     the produced bytes, but nothing paced the output cadence.
  2. *The flush was untimed.* `last_write_ns` timed only the
     `write_all` into the 256 KB `BufWriter` — an in-memory copy for
     every normal frame. The actual blocking syscall
     (`BufWriter::flush`) was invisible, so the power system was
     blind to the exact latency signal that matters.
  3. *P2 health mitigation bomb.* `EnduranceHealth` scored the frame
     signal as ABSOLUTE milliseconds (`100 - ms*10`): anything >= 10ms
     scored zero — calibrated to Alacritty-class renderers only. A
     VTE/foot frame that healthily uses 12ms of its 16.7ms budget was
     classified "investigate" (<60) permanently, arming the P2
     self-healer every 30s cooldown. P2's "cure" is
     `force_draw_everything()` — the single largest ANSI burst the
     renderer can produce (100-400 KB) — pushed into the already
     saturated pipe: the write blocks for seconds ("stuck"), and when
     the terminal finally drains, particles that expired during the
     stall vanish in one step ("auto-dismiss"). Periodic
     stuck-then-clear every 30s, exactly as reported. Persistent
     clicking deepened the congestion and stretched frame intervals
     past the 250ms particle anti-teleport cap, so bursts decayed
     their velocity in 1-2 giant steps and hung as near-motionless
     sparks — the "snow/sleet" degradation.
- **Fix**: four changes, one closed feedback loop.
  1. `flush_stdout_timed()` — the final flush syscall's latency is
     now ACCUMULATED into `last_write_ns`, so the measured signal
     reflects the real blocking point.
  2. `PowerManager` output drain backoff — `observe_frame_end` maps
     write-latency overshoot to a `drain_backoff` scalar (rise
     0.05/unit overshoot, decay 0.002/clean frame), and
     `effective_fps` scales the non-paused cadence by up to 75%
     (floor `min(12, base)`), gated on `power_dragon` like the idle
     reduction. The output loop now settles at the terminal's
     sustainable drain rate instead of flooding it.
  3. P2 congestion guard — `TriggerHealthMitigation` skips the
     full-redraw burst when `effective_pressure >= 0.3` (output
     congestion); the madvise (P2's actual memory purpose) is kept.
     The full redraw stays reserved for its original calibration:
     pressure LOW + genuinely unhealthy process.
  4. `EnduranceHealth` frame signal is now RELATIVE — the EMA of
     `work_s / frame_period_s` (utilization), scored
     `100 - util*60` floored at 40: a busy-but-keeping-up terminal
     scores healthy, pure output saturation alone cannot arm the
     memory mitigation (RSS variance must contribute). The event
     loop also gates the write-overshoot injection on `did_draw` so
     stale latency from non-drawing frames cannot pin the backoff.
- **Verified**: 13 new unit tests — drain backoff rise/decay/gating/
  idle-composition/floor/paused/CPU-vs-write separation
  (`power_manager/tests.rs`), P2 redraw-forces-at-low-pressure vs
  skips-under-congestion (`tests_v51_2_power_dragon_gate.rs`),
  utilization scoring bands (busy terminal not "investigate", pure
  saturation not "investigate", RSS instability still reaches
  "investigate", EMA clamping) (`endurance_health.rs`), and the HUD
  `tgt: N drain` suffix. Full suite 2207 passed / 0 failed. 10s A/B
  benchmark: noise-equivalent (avg_fps +0.19%, entropy +0.03%,
  gini -0.01%, dirty cells +0.00%) — the bench path is headless (no
  terminal drain), so the backoff never engages there, as intended.
  The interactive effect: on a saturated terminal the HUD now shows
  `tgt: N drain` while cadence tracks the drain rate; blocked-write
  stalls shrink to the pipe transit time; the 30s stuck-then-clear
  cycle is gone.

Docs synced: KNOWN_ISSUES.md VTE section (three-layer root cause +
foot classification note), power manager frame-lifecycle module
docs, `last_write_ns` field doc, `OUTPUT_DRAIN_*` constants,
HUD FrameMode docs.

### harmony: v100.0.0-nightly.1 — S-master-HUNT-22 particle real-time clock (VTE stuck/hang, round 2)

Owner bug report (2026-09-04, post-b22e81a): on VTE terminals
(GNOME Terminal, Konsole) the mouse-click spark burst and the
border-touch sparks above the message box drifted slower and slower
over minutes of clicking ("becomes snow ice"), then appeared stuck
for seconds before vanishing on their own. The HUNT-21 fix
(sim_age, b22e81a) had unified particle aging with particle motion
but the symptom survived.

- **Root cause**: particle physics integrated
  `dt = min(dt_raw, 1/30, max_sim_delta) * resume_blend` per frame.
  On VTE the real frame interval is 67-200ms while the cap chain
  admits only 15-33ms (1/30 clamp, plus `max_sim_delta` pinned at
  15ms once perf pressure saturates — VTE's CPU rendering cannot hit
  the 60 FPS target, so `observe_frame_end` overshoot pins
  `perf_pressure` at 1.0 and `run_sim_and_draw` scales the sim cap to
  0.3). Each frame therefore advanced particles only 10-30% of the
  wall-clock time that actually passed: a permanent time dilation.
  The 4.0s quantum ripple stretched to 20-40 real seconds of slow
  drift, the 350ms border spark lingered ~2.3s, the velocity decay
  froze late-life particles mid-air ("stuck"), and each effect only
  ended once its diluted `sim_age` crossed the lifetime — matching
  the owner's "slow, then stuck, then disappears by itself" report
  exactly. The co-spawned flash wave aged by `now - birth` (real
  time), which is why the click RING looked normal while its sparks
  crawled: the particle family was the only transient-effect family
  still on the dilated clock.
- **Fix**: all transient particle systems (QuantumParticle
  mouse-click ripples + border-touch splash crowns, EngraveSpark,
  ScorchSmoke) now integrate REAL elapsed time bounded by the new
  `PARTICLE_MAX_FRAME_DT_SECS` (0.25s) anti-teleport cap, still
  scaled by `resume_blend` for the pause decel/resume easing:
  `dt = min(dt_raw, 0.25) * resume_blend`. Motion and `sim_age`
  share the same real clock (HUNT-21 invariant preserved), and an
  effect completes in its intended wall-clock duration at any frame
  rate. `toggle_pause` BRANCH 2 additionally shifts
  `engrave.last_update` / `scorch.last_update` forward by the pause
  duration (same §8.5 family as `last_quantum_update_time`) so
  mid-flight sparks and smoke resume without burning their
  anti-teleport budget. The rain and monolith keep the dilated
  `max_sim_delta` clock on purpose: the rain is an ambient field
  where slow motion reads as calm, while click sparks are
  interaction impulses whose perceived latency is a responsiveness
  signal.
- **Verified**: 4 new unit tests
  (`tests_quantum_hunt22.rs`) — a 10 FPS + saturated-sim-cap run
  must expire a particle within its real lifetime (fails on the old
  clamped clock, which leaves it alive at ~1.3s sim_age after 4.0
  real seconds), a 5s stall must integrate exactly the 250ms cap,
  motion must cover equal real distance at 10 FPS vs 60 FPS (~6x in
  6x the time), and unpause must shift all three particle clocks.
  Full suite 2194 passed / 0 failed. 10s A/B benchmark
  (before/after, `--benchmark --bench-duration 10 --json`):
  noise-equivalent (avg_fps -0.55%, frame_entropy +0.01%,
  density_gini -0.00001, dirty cells +0.01%) — the bench path has no
  clicks, so the particle update stays at its O(1) early-out and the
  fix is invisible to it, as intended.

Docs synced: KNOWN_ISSUES.md VTE status section (two-layer root
cause), `PARTICLE_MAX_FRAME_DT_SECS` doc comment, `sim_age` field
comments, apply_quantum_ripple / draw_engrave_sparks /
draw_scorch_smoke clock comments, toggle_pause shift comment.

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
