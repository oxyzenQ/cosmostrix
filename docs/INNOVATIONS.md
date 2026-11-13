<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# The cosmostrix Innovation Ledger

This page records the research and innovations that define cosmostrix's
three-engine renderer. It is the result of the owner's own thinking and
design research, conducted together with AI, and it is written simple
on purpose, so a new maintainer (or a curious competitor) can learn the
whole machine in one sitting. Each entry names the problem it kills,
the mechanism in one breath, and the file where the truth lives. This
page is the map, not the ledger — the source tree is the ledger, and
every claim below yields to it (see [DOC_AUDIT.md](DOC_AUDIT.md) for
the ruleset this page obeys).

One law runs through everything below: **CPU-only, single-threaded,
no daemon, no GPU** — the rain is diff-based ANSI text in a terminal
the user already owns, and every mechanism is pinned by invariant tests
instead of hope. The engine lock suites assert the two render engines'
public contracts on every commit, the supermassive PTY lanes prove the
render contract on a real terminal, and the gate suite enforces the
written law (headers, language, emoji, hype vocabulary, doc hygiene)
on every push.

## The engines, named

Three cooperating engines split the work along clean boundaries:
**the Cosmic Dragon** owns *what cells changed* (the diff-based render
path, `src/engine/cosmic_dragon_engine/`), **the Chroma Dragon** owns
*what color a cell becomes* (the OKLab pipeline,
`src/engine/chroma_dragon_engine/`, locked at Phase 9-D), and **the
Crystal Dragon** owns *what mood the rain should have* (ambient drift
and time-of-day scheduling, `src/engine/crystal_dragon_engine/`).
Around them sit the shared layers: the Central Control registries that
own every tuning parameter (`src/central_control_rains/`,
`src/central_control_power_dragon/`), the interactive runtime
(`src/interactive/`), and the config and live-reload stack
(`src/config/`). The cooperation model is told once in
[THREE_DRAGON_ENGINES.md](THREE_DRAGON_ENGINES.md).

## The thirty, in dependency order

### 1. The diff-based render path (the Cosmic Dragon)

Problem killed: the redraw-everything loop hits the terminal's ANSI
throughput ceiling, so anything cinematic is unaffordable at size.
Mechanism: a persistent frame buffer is compared against the last
emitted frame, and only changed cells are written — with semantic
generation invalidation so a clear is O(1) and a scene switch
invalidates in one step. The measured I/O reduction numbers are told
once in the README's Cosmic Dragon section.
Truth: `src/engine/cosmic_dragon_engine/frame.rs` (the double-buffered
frame), `src/engine/cosmic_dragon_engine/terminal/last_frame.rs` (the
previous-frame cache), `src/engine/cosmic_dragon_engine/terminal/draw.rs`
(the dirty-cell emit).
Verifier: the Cosmic Dragon lock suite (dirty semantics, generation
invalidation, idle-frame tracking) —
`cargo test cosmic_dragon_incubator::tests::lock -- --nocapture`.

### 2. The output path — row-RLE batching and pre-formatted SGR

Problem killed: per-cell formatting and per-frame heap allocation burn
the frame budget. Mechanism: consecutive dirty cells in a row are
batched into one escape run (row-RLE), colors are pre-formatted into
cached SGR byte strings so the hot path writes bytes rather than built
strings, and `src/bolt/` supplies branchless lookup tables; the lean
path reaches 0.0 allocs/frame (measured, told once in the README's
Philosophy section).
Truth: `src/engine/cosmic_dragon_engine/terminal/draw.rs`,
`src/bolt/`; the batching cost model lives in
`src/cosmic_dragon_incubator/egg/threshold_sweep.rs`.
Verifier: lock invariants for the SGR color cache (Cosmic Dragon
INV-14/15), plus the `escape_ctrl` fuzz target proving emission stays
safe under adversarial input; the supermassive PTY byte-log assertions
(`scripts/supermassive/pty_assert.py`) observe the emitted control
sequences on a real terminal, where the batching result is visible.

### 3. The three-layer parallax depth system

Problem killed: single-layer rain reads flat, and more than three
layers collapses perceptually in a 24-row terminal. Mechanism: far,
mid, and near layers with independent speed, brightness, length,
density, and phosphor-decay multipliers, plus per-layer contrast
reduction for depth-of-field blur; every tuning parameter has one home
in the Central Control Rains registry.
Truth: `src/central_control_rains/mod.rs` (the single source of truth),
`src/droplet/draw.rs` (the per-layer contrast LUT).
Verifier: `test/central_control_rains/`; the tuning bible is
[CENTRAL_CONTROL_RAINS_USAGE.md](CENTRAL_CONTROL_RAINS_USAGE.md).

### 4. Phosphor persistence (CRT afterglow)

Problem killed: instant cell clears read as a screensaver, not a
cinema. Mechanism: per-layer decay multipliers with bottom-row
acceleration leave a ~400 ms afterglow per glyph, and the anomaly pass
keeps the decay honest under pressure.
Truth: `src/engine/cosmic_dragon_engine/cloud/phosphor.rs`,
`src/engine/cosmic_dragon_engine/cloud/phosphor_anomaly.rs`.
Verifier: [RENDER_ENGINE.md](RENDER_ENGINE.md) and the scene residue
tests under `test/engine/cosmic_dragon_engine/cloud/tests/`.

### 5. The living rain field (value-noise density and wind gusts)

Problem killed: uniform random columns read mechanical. Mechanism:
Perlin-style value-noise density scatters the rain organically while
gust cycles accelerate columns in waves, so the field breathes instead
of cascading on a metronome.
Truth: `src/engine/cosmic_dragon_engine/cloud/living_rain.rs`,
`src/engine/cosmic_dragon_engine/cloud/rain_at.rs` (the render loop
entry).
Verifier: [RENDER_ENGINE.md](RENDER_ENGINE.md) and the supermassive
soak lane on a real PTY.

### 6. The border-touch family (glow pulse and splash spark)

Problem killed: rain visibly hits the message border and nothing
happens, because the overlay structurally occludes the contact.
Mechanism: the touch event is synthesized at the moment a droplet head
reaches the border row — classified by position, not by glyph — the
border gradient pulses, and a six-particle splash crown spawns from the
shared particle pool with zero new allocation.
Truth: `src/engine/cosmic_dragon_engine/cloud/border_touch.rs`
(`detect_border_touch`), `src/engine/cosmic_dragon_engine/cloud/spawn.rs`
(`spawn_border_spark`).
Verifier:
`test/engine/cosmic_dragon_engine/cloud/tests/tests_border_gradient.rs`;
the research records are
[research/RAIN_BORDER_TOUCH_GLOW_AUDIT.md](research/RAIN_BORDER_TOUCH_GLOW_AUDIT.md)
and
[research/RAIN_BORDER_TOUCH_SPARK_RESEARCH.md](research/RAIN_BORDER_TOUCH_SPARK_RESEARCH.md),
and the origin story is [INSIGHTS.md](../INSIGHTS.md), insights 1-2.

### 7. The quantum particle pool

Problem killed: input-driven effects that allocate per particle break
the zero-alloc law and can spawn without bound. Mechanism: one bounded
pool of particle values serves every effect — click ripple, dual-ring
shockwave, border spark — and spawning reuses pooled slots.
Truth: `src/engine/cosmic_dragon_engine/cloud/state.rs` (the particle
struct), `src/engine/cosmic_dragon_engine/cloud/mod.rs` (the pool).
Verifier: the quantum tests
`test/engine/cosmic_dragon_engine/cloud/tests/tests_quantum*.rs`.

### 8. Ghost-kanji cinematic events

Problem killed: static rain lacks narrative punctuation across long
sessions. Mechanism: probabilistic ghost characters fade in and out
during the rain, colored through the Chroma post stage so they always
match the active scene's palette.
Truth: `src/engine/cosmic_dragon_engine/cloud/ghost_events.rs`,
`src/engine/cosmic_dragon_engine/cloud/events/ghost.rs`,
`src/engine/chroma_dragon_engine/post/ghost.rs`.
Verifier: [RENDER_ENGINE.md](RENDER_ENGINE.md) and the supermassive
soak lane.

### 9. The entropy drift ecosystem

Problem killed: a renderer that looks identical at minute 1 and minute
600 feels dead. Mechanism: slow autonomous luminance shifts, density
migration, and anomaly pressure fluctuations ride a self-resetting
cycle on top of behavior profiles, so long sessions drift
atmospherically without user input.
Truth: `src/engine/cosmic_dragon_engine/cloud/ecosystem.rs`
(`EntropyDrift`, `BehaviorProfile`).
Verifier: [RENDER_ENGINE.md](RENDER_ENGINE.md); the long-run evidence
lives in [ENDURANCE.md](ENDURANCE.md).

### 10. Smooth pause — the exponential-decay easing family

Problem killed: a hard pause or resume snap destroys the cinematic
illusion. Mechanism: one unified easing family governs pause
coast-down, resume ramp, and glyph scene entry; resume is
phase-continuous per droplet (brightness ramp and row-advance phase
preserved frozen mid-step), and a cell a structured family still draws
is never decayed into a ghost by the afterglow pass.
Truth: `src/engine/cosmic_dragon_engine/cloud/pause.rs`.
Verifier: the paused-mode contract in [HUD.md](HUD.md); the
NIGHT-hunter research records in [research/](research/).

### 11. The Chroma Dragon OKLab pipeline (Phase 9-D)

Problem killed: raw RGB interpolation produces muddy midpoints on
hue-crossing gradients and hard seams on palette switches. Mechanism:
palette interpolation in OKLab (perceptually uniform), hue-preserving
polar gradients, palette-relative brightness floor, body-tail
brightness continuity, head halo with subpixel hue jitter, temporal
column coherence, and palette transitions that sweep perceptual space
instead of snapping.
Truth: `src/engine/chroma_dragon_engine/palette/mod.rs` (the pipeline),
`src/engine/chroma_dragon_engine/tuning.rs` (the tuning constants).
Verifier: the Chroma Dragon lock suite (invariants INV-01 through
INV-19 across `test/engine/chroma_dragon_engine/tests/lock.rs` and
`lock_inv13_19.rs`) —
`cargo test chroma_dragon_engine::tests::lock -- --nocapture`.

### 12. The central color theme registry

Problem killed: theme lists that live in several places drift apart
(`--list-colors` versus `--color` versus scenes). Mechanism: one theme
array is the single source, and every surface — listing, selection,
palette construction — auto-discovers from it; adding a theme is one
variant plus one entry.
Truth: `src/engine/chroma_dragon_engine/catalog.rs` (the facade) with
`src/engine/chroma_dragon_engine/catalog/themes.rs` (the array), and
`ColorScheme` in `src/engine/cosmic_dragon_engine/runtime.rs`.
Verifier: the Chroma lock full-catalog build sweep (INV-02); the
add-a-theme recipe is told once in [README.md](README.md).

### 13. Color tune — the per-channel multipliers

Problem killed: infinite aesthetic variants would otherwise mean
infinite theme entries. Mechanism: five per-channel multipliers (sat,
bright, head, body, tail; 0.0-3.0, default identity) ride the Chroma
pipeline, turning every built-in theme into a family of variants.
Truth: `src/engine/chroma_dragon_engine/tuning.rs`.
Verifier: the `--color-tune` contract in the README CLI reference and
[FAQ.md](FAQ.md).

### 14. Crystal Dragon ambient drift

Problem killed: a fixed palette ignores the machine's mood. Mechanism:
system sensors (CPU, clock) collapse to a point, point-based
temperature grouping (Cold, Medium, Hot) selects a palette family,
probabilistic weighting picks the theme, and a ~300 ms OKLab wave
transition moves through the Chroma Dragon.
Truth: `src/engine/crystal_dragon_engine/` (`sensor/`, `point_system/`,
`palette_groups/`, `ambient/`), with the canonical header in
`src/engine/crystal_dragon_engine/mod.rs`.
Verifier: `test/engine/crystal_dragon_engine/ambient/tests.rs`; the
complete source-as-truth reference is
[CRYSTAL_DRAGON_ENGINE.md](CRYSTAL_DRAGON_ENGINE.md).

### 15. The ambient scheduler (time-of-day scenes and snapback)

Problem killed: scene mood should follow the clock, but a cron-like
helper would be a daemon — banned by the one law. Mechanism:
`ambient."HH-MM" = "scene"` config entries drive a dynamic idle/wake
thread that spends zero CPU between phase boundaries; Crystal Dragon
drift overrides the ambient palette and the snapback reverts it, so
the two systems take turns; both timing knobs are tunable online
through one human-duration vocabulary.
Truth: `src/engine/crystal_dragon_engine/ambient_scheduler/mod.rs` (the
runtime); the `ambient.<HH-MM>` key parse lives in
`src/config/configfile.rs`.
Verifier: `test/engine/crystal_dragon_engine/ambient_scheduler/tests.rs`; the
timing recipes are told once in
[AMBIENT_SCHEDULER.md](AMBIENT_SCHEDULER.md), and the DST wall-clock
honesty is in the README Limitations.

### 16. The Power Dragon (adaptive throttle and byte budgets)

Problem killed: a full-rate rain wastes CPU when idle and can exhaust
weak web-hosted terminals. Mechanism: adaptive throttling reduces FPS
after input goes quiet, thermal pressure feeds the self-healer,
xterm.js hosts get hard byte-budget caps on top of their FPS cap, and
`power-dragon = false` pins the exact configured values.
Truth: `src/central_control_power_dragon/` (the budgets),
`src/central_control_power_dragon/power_manager/` (effective pressure
and FPS), `src/termdetect/` (host classification), `src/cli/app.rs`
(the cap).
Verifier: `test/central_control_power_dragon/power_manager/tests.rs`
and `test/termdetect/`; the subsystem doc is
[CENTRAL_CONTROL_POWER_DRAGON.md](CENTRAL_CONTROL_POWER_DRAGON.md).

### 17. The self-healing stack (P1, P2, and the Endurance Health Score)

Problem killed: long sessions under pressure degrade into stutter,
leaks, or a degraded terminal. Mechanism: P1 auto-downgrades the scene
to `low-power` under sustained pressure and restores when the pressure
drops; P2 runs the endurance mitigation (full redraw plus memory
reclaim hints); the Endurance Health Score (0-100) predicts trouble
from RSS, frame utilization, and context switches before it happens.
Truth: `src/central_control_power_dragon/self_healer/mod.rs`,
`src/central_control_power_dragon/endurance_health.rs`, wired through
`src/interactive/event_loop_self_heal.rs`.
Verifier: `test/central_control_power_dragon/self_healer/tests.rs`
(P1/P2 precedence) and `test/central_control_power_dragon/audit_tests.rs`;
the LTS audit record is
[archive/LTS_AUDIT_SELF_HEALING.md](archive/LTS_AUDIT_SELF_HEALING.md).

### 18. Live config reload — the hybrid watcher and the deliberate-empty law

Problem killed: config edits either require a restart or silently
misapply. Mechanism: a notify-based hybrid watcher (native plus poll
fallback) feeds a bounded channel into a strict
validate-rebuild-apply pipeline on the render thread; a zero-key parse
with comment lines is classified as a deliberate empty config
(delivered) while a whitespace-only file is transient (skipped) — the
distinction that killed the dropped-event bug family.
Truth: `src/config/live_config/watcher.rs` and
`src/config/live_config_poll/`.
Verifier: `test/config/live_config/tests.rs` and
`test/config/live_config_poll/tests.rs`; the behavior matrix and the
99% honesty contract are told once in
[LIVE_RELOAD_BEHAVIOR.md](LIVE_RELOAD_BEHAVIOR.md) and
[CONFIG_LIVE_RELOAD_DISCLAIMER.md](CONFIG_LIVE_RELOAD_DISCLAIMER.md);
the bug's story is [INSIGHTS.md](../INSIGHTS.md), insight 6.

### 19. The CLI suggestion engine (did-you-mean)

Problem killed: a typo in any surface either fails ugly or silently
misconfigures. Mechanism: edit-distance suggestions cover every option,
value, scene, color, charset, custom-name, and shorthand surface;
removed options get migration hints instead; one vocabulary is shared
by the CLI and the live-reload watcher.
Truth: `src/cli/suggestion.rs`, wired through
`src/config/config_hints/mod.rs`.
Verifier: `test/tests/clap_suggestion.rs` and the shell stress drill
`scripts/depth/cli_suggestion_stresstest.sh`; the coverage inventory is
told once in [RULES.md](RULES.md).

### 20. Message fill styles (eleven cinematic reveals)

Problem killed: an overlay message that pops in instantly breaks the
mood. Mechanism: eleven reveal engines — typewriter, fade, words,
slide, instant, engrave, hologram, glitch, scorch, cascade, radar —
one file per style, all honoring `--no-effects`.
Truth: `src/msg_fill_style/` (the enum plus one file per style).
Verifier:
`test/engine/cosmic_dragon_engine/cloud/tests/tests_msg_fill_*.rs` and
`test/config/live_config/tests_msg_fill_style.rs`.

### 21. Terminal recovery and the fork guard

Problem killed: a crashed renderer leaves the user's terminal broken.
Mechanism: `--reset-terminal` runs layered recovery (RIS reset,
alternate-screen exit, cursor restore, terminal attributes reset,
scrollback clear); on Linux a fork-based best-effort guard restores
`termios` after SIGKILL; the RAII terminal guard and the panic hook
restore the terminal on unwind, which is why release builds keep
`panic = "unwind"` on purpose.
Truth: `src/engine/cosmic_dragon_engine/terminal/io_recovery.rs`,
`src/platform/`.
Verifier:
`test/engine/cosmic_dragon_engine/terminal/p5_tests.rs` and
`test/engine/cosmic_dragon_engine/terminal/cbg34_tests.rs`; the
contracts are [TERMINAL_KILL_CLEANUP.md](TERMINAL_KILL_CLEANUP.md) and
[TERMINAL_LIFECYCLE_MATRIX.md](TERMINAL_LIFECYCLE_MATRIX.md).

### 22. Terminal tier detection (the xterm.js OOM guard)

Problem killed: web-hosted terminals crash out-of-memory at high frame
rates — not the renderer's fault, still its user's pain. Mechanism:
startup detection classifies the host; xterm.js hosts get a hard,
non-configurable 30 FPS cap while native terminals get the full range.
Truth: `src/termdetect/`, cap applied in `src/cli/app.rs`.
Verifier: `test/termdetect/`; the compatibility matrix is
[TERMINAL_COMPATIBILITY.md](TERMINAL_COMPATIBILITY.md).

### 23. The zero-dependency benchmark stack

Problem killed: honest measurement usually drags in a JSON framework
(supply-chain surface) or fabricates metrics where the OS cannot know.
Mechanism: a hand-rolled zero-dependency JSON serializer, fixed virtual
screen sizes, compound durations, wet-I/O mode, lean versus
production-draw scenes, the scaling ladder, and baseline save/compare
with a marked regression threshold; unsupported metrics print
`unsupported` — never fake values.
Truth: `src/bench/` (including `bench_json.rs`).
Verifier: `test/bench/`; the mode catalog is told once in
[BENCHMARKING.md](BENCHMARKING.md); fair cross-tool measurement runs
through `scripts/bench/bench-runner.py`.

### 24. The fourteen rain styles (six original motion-math systems)

Problem killed: cinematic variety needs real motion models, not speed
hacks over one cascade math. Mechanism: fourteen rain styles, six of
them original motion-math systems — Navier-Stokes PIC/FLIP (`flux`),
Lorenz RK4 (`lorenz`), Physarum stigmergy (`physarum`), boids spatial
hash (`murmuration`), FABRIK chain dynamics (`dna_helix`), Keplerian
orbits (`black_hole`) — each in its own module under the cloud.
Truth: `src/engine/cosmic_dragon_engine/cloud/type_rain/` (one
directory per style).
Verifier: the style research records in
[research/](research/) (`NIGHT_LTS_1_STAGE*.md`,
`NIGHT_RESEARCH_*.md`), with the color-cap rulings pinned by the
Chroma lock suite.

### 25. The Cosmic Dragon Incubator

Problem killed: render-path experiments argued by taste instead of
cost. Mechanism: a cost-modeling module prices a change to the hot
path (the RLE threshold sweep among them) before the change is
allowed to land.
Truth: `src/cosmic_dragon_incubator/`.
Verifier: the incubator lock suite
(`test/cosmic_dragon_incubator/tests/lock.rs`, which also carries the
engine version sentinel).

### 26. The live HUD

Problem killed: a renderer running for hours gives the user no feedback
about what it is doing to the machine. Mechanism: a dashboard overlay,
toggled with `i`, reports real-time FPS, p99 frame time, frame-time
distribution, RSS, the Endurance Health Score, and build information,
with the gradient header drawn through the Chroma pipeline.
Truth: `src/interactive/hud/` (`mod.rs`, `hud_init.rs`, `metrics.rs`,
`colors.rs`), wired through `src/interactive/event_loop_hud.rs`.
Verifier: `test/interactive/` and [HUD.md](HUD.md) (the per-line
reference, including the paused-mode contract).

### 27. The cinematic intro

Problem killed: a cold binary start drops the user into the rain with
no framing. Mechanism: `--intro cosmic|logo|none` (default logo) plays
a branded reveal in every mode, with graceful degradation — skipped on
terminals smaller than 10x5, auto-skipped on low-detail renderers, and
interruptible with `q`; the intro palette is independent of `-c/--color`
unless `--intro-color` overrides it.
Truth: `src/intro_style/` (one module per style).
Verifier: `test/intro_style/`; the option contract is in the README
CLI reference.

### 28. The safepath gate

Problem killed: flags that read files from disk are a file-path
surface in a tool the user runs on their own TTY. Mechanism: every
config path is validated through one gate (`is_safe_path`,
`validate_config_path`) before the file system is touched.
Truth: `src/safepath/`.
Verifier: `test/safepath/`; the wiring note lives in `src/main.rs`.

### 29. Doctor and diagnostics

Problem killed: "does this machine run cosmostrix well?" answered by
guesswork. Mechanism: `--doctor` prints a compatibility report
(environment, locale, charset, terminal), and the diagnostics module
reports CPU feature detection and the binary variant so a user can
quote a real setup instead of describing one.
Truth: `src/doctor/`, `src/diagnostics/`.
Verifier: `test/doctor/`; the flag contracts are in the README CLI
reference.

### 30. The verification machinery

Problem killed: claims that cannot be re-run are marketing. Mechanism:
the engine lock suites assert public contracts on every commit, backed
by the compiled-in test tree; the PTY truth harnesses run the real
binary on a real pty (smoke, byte-log assertions, five hostile load
shapes, the depth-bore drift drill); CI proves the documented Linux
kernel floor and the latest mainline in KVM micro-VMs; and the gate
suite enforces headers, language, emoji, hype vocabulary, and doc
hygiene on every push.
Truth: `test/` (mirroring `src/`), `scripts/supermassive/`,
`scripts/depth/`, `scripts/gates/`, `.github/workflows/`.
Verifier: the one-command harness table in the README's Test Results
section; the gate entry point is `scripts/gate-keepers.sh`.

## Retired by verdict

Not every idea earned its place. These were evaluated and rejected,
and the records are kept so the same debate does not rerun:

- **GPU image-mode rendering** — rejected because it breaks the
  character-grid aesthetic (every glyph must stay a real, selectable
  codepoint). Record: `docs/archive/cosmic_dragon/EXPLORATION.md`.
- **The atmosphere engine subsystem** — eliminated; the elimination
  record and the historical spec are linked from the note in
  [README.md](README.md) (the docs index).
- **The `--low-power` CLI flag** — removed; the `low-power` scene
  remains as the self-healer's P1 target.
- **v1/v2 x86_64 and glibc-linked Linux release shapes** — retired;
  the fully static musl v3 shape is the shipped floor (the note in the
  README's Optimized local builds section).

## Reading order for a new maintainer

1. The README — the product and its honesty contracts.
2. [README.md](README.md) (the docs index) — the registry of every doc.
3. [RENDER_ENGINE.md](RENDER_ENGINE.md) and
   [COSMIC_DRAGON_ARCHITECTURE.md](COSMIC_DRAGON_ARCHITECTURE.md) —
   the render path.
4. [THREE_DRAGON_ENGINES.md](THREE_DRAGON_ENGINES.md) — the
   cooperation model.
5. [CRYSTAL_DRAGON_ENGINE.md](CRYSTAL_DRAGON_ENGINE.md) and
   [AMBIENT_SCHEDULER.md](AMBIENT_SCHEDULER.md) — ambient intelligence.
6. This page — the innovation map you just read.
7. [RULES.md](RULES.md) and [../CONTRIBUTING.md](../CONTRIBUTING.md) —
   the law you code under.

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
