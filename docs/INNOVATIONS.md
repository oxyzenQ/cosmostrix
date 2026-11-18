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

## The thirty-nine, in dependency order

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
path reaches 0.0 allocs/frame (measured once, now contract-enforced
by the zero-alloc ledger, entry 36 — the README's Philosophy section
carries the story).
Truth: `src/engine/cosmic_dragon_engine/terminal/draw.rs`,
`src/bolt/`; the batching cost model lives in
`src/cosmic_dragon_incubator/egg/threshold_sweep.rs`.
Verifier: lock invariants for the SGR color cache (Cosmic Dragon
INV-14/15), plus the `escape_ctrl` fuzz target proving emission stays
safe under adversarial input; the supermassive PTY byte-log assertions
(`scripts/supermassive/pty_assert.py`) observe the emitted control
sequences on a real terminal, where the batching result is visible,
and the syscall-floor ledger (entry 35) proves the batching lands in
exactly one kernel write() per frame.

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
shapes, the depth-bore drift drill); the syscall-floor ledger
straces the release shape and proves the kernel floor is exactly
one write() per drawn frame (entry 35); the torture battery's leg 6
proves the suspend/resume contract end to end (entry 37) and the pty
battery's leg 7 enforces the color-16 HUD purity contract (KNOWN_ISSUES,
3a); CI proves the documented Linux
kernel floor and the latest mainline in KVM micro-VMs; and the gate
suite enforces headers, language, emoji, hype vocabulary, and doc
hygiene on every push.
Truth: `test/` (mirroring `src/`), `scripts/supermassive/`,
`scripts/depth/`, `scripts/gates/`, `.github/workflows/`.
Verifier: the one-command harness table in the README's Test Results
section; the gate entry point is `scripts/gate-keepers.sh`.

### 31. The provenance depth matrix (masked embedded build signature + blake3 anchor)

Problem killed: binary theft and fake-claim forks — a rebrand that
ships the renderer under someone else's name, or a "modified build"
nobody can tell apart from the official one. The first draft embedded a
plaintext manifest, and the owner rejected it on a real threat: a thief
can run `strings`/`rg` over the artifact, find the wording, and patch
it out. Mechanism (advanced form): the ownership claim
"rezky_nightky (oxyzenQ) 2026" exists in the binary ONLY as opaque
64-bit integers — two independent XOR-masked word arrays (the Signature
manifest, the Copyright authors string) masked by a splitmix64 stream
keyed by per-layer seeds, with no printable four-byte run anywhere in
the masked data, so artifact-side greps find nothing. `-V/--version`
materializes the plaintext in one unmask pass per layer and verifies it
against compile-time FNV-1a 64-bit digests computed directly from the
masked words (the plaintext never exists as a literal in the module, so
it cannot leak into `.rodata` through const evaluation), reporting
`status=verified` or `status=tampered` on its `Depth-matrix:` line; the
installer's release law runs the built binary and requires the
materialized identity plus a verified status. Advanced form 3 adds the
cryptographic seal and the interactive surface: a BLAKE3 anchor over
the framed clear payload of both layers (domain tag + length-prefixed
manifest + length-prefixed authors) is embedded as an opaque
`[u8; 32]` and recomputed through the blake3 crate from the SAME
materialized bytes the `-V` line displays, and the new `cosmostrix sig`
command renders the owner's cross-tool depth report (zejtron
parity: D0 runtime-identity, D1 embedded-marker, D2 anchor-integrity,
D3 binary-self-hash, D4 engine-oracle) with a `SIGNATURE: AUTHENTIC (5/5 depths
verified)` verdict and exit code 2 on tamper, so scripts can gate on
it. Advanced form 4 (2026-11-13) retires the `--sig` flag spelling —
`sig` is a SUBCOMMAND dispatched pre-clap (zero arguments; extra
arguments get a clap-shaped refusal, exit 0 authentic / 2 tamper) —
and brands the surfaces: the `sig` banner rides the bold brand purple
report-title tier and the `-V` Depth-matrix line rides the ok-green
tier, both TTY-gated so piped output stays byte-for-byte plain (the
machine-friendly canon). D3 blake3-hashes the running executable image (~10 MB in tens of
milliseconds on the pure-Rust implementation; the crate's C/asm lanes
are deliberately off so every cross-compile lane in the release matrix
keeps building without a target C toolchain) — an honest FINGERPRINT:
a self-hash cannot embed its own digest, so `pass` means "the image was
read in full and hashed", and the identity check is the comparison
against a published release hash. The Python generator mirrors BLAKE3
spec-exactly (pure Python, zero dependencies) and pins six vectors
that were cross-validated byte-for-byte against the Rust crate — a
mirror that caught a real `block_len` drift (hardcoded 64 instead of
the actual block size, which only bites on partial final blocks and
empty inputs) before it could ship a wrong anchor. Four traps are part of
the lesson: an optimizer folds a constant-argument recompute back into
the embedded digest unless the read passes through
`std::hint::black_box`; a constant referenced by two functions can be
cloned into separate `.rodata` copies, letting a display patch and an
integrity check disagree — hence one unmask pass per call feeding both
display and fold; the authors env expansion (`CARGO_PKG_AUTHORS`) is a
plaintext identity carrier of its own, so the Copyright line
materializes from the masked layer instead; and the mask algorithm must
be mirrored byte-exactly between the generator and the const code,
which is pinned by fingerprint cross-check tests. Honest limits: this
is tamper evidence, not DRM — once `-V` prints, the plaintext lives in
process memory and forensics can extract it; the defense targets the
lazy grep-and-strip rebrand and survives source rebrands by people who
do not recognize the constants, while a determined attacker who fully
understands the module can regenerate everything (the legal layer in
NOTICE and TRADEMARK.md is the backstop). The constants are generated
by `scripts/release/generate-provenance-words.py`, the committed truth
source that carries the identity in plaintext by design, the same way
NOTICE does. End-to-end probes on the real release binary pin the full
matrix: pristine verifies AUTHENTIC 5/5 with zero identity fragments
at rest (`oxyzenQ` survives only inside repo/branding contexts - the
public URL, the repo slug, scene lore - never beside the year), a
patched masked word flips `-V` to `status=tampered` AND `sig` to exit
2 TAMPERED, and a patched anchor byte (LLVM constant-pool merging
splits the array into separate halves; the probe locates the head)
fails the same way. The generic `Official Build by` marker stays
plaintext by design: it is identity-free and fingerprint-bound.
Truth: `src/diagnostics/signature.rs` (the masked layers, the
single-pass materialization, the dual fingerprints, the anchor),
`src/diagnostics/sig_report.rs` (the five-depth report and the binary
self-hash),
`scripts/release/generate-provenance-words.py` (constant and anchor
generation),
`src/diagnostics/info.rs` (`version_report` renders the view),
`src/cli/argv_expand.rs` (the `sig` subcommand dispatch, argv[1] only
— extra arguments refused, `--sig` retired),
`scripts/package/install-on-my-machine.sh` (the release law's runtime
signature gate + the five-depth gate).
Verifier: `cosmostrix -V | grep Depth-matrix` must show
`status=verified` and a `b3:` anchor on any genuine build;
`cosmostrix sig` must end `SIGNATURE: AUTHENTIC (5/5 depths verified)`
and exit 0 (exit 2 plus `SIGNATURE: TAMPERED` on any patched depth);
and `strings ./cosmostrix | grep rezky_nightky` must find NOTHING (the
mark is masked at rest); the unit suites pin the materialization
oracles, the strings-invisibility floor, the const-vs-runtime
fingerprint equality, the anchor equality against the generator's
hex oracle, the framing layout, and the tamper sensitivity
(`cargo test diagnostics::signature diagnostics::info
diagnostics::sig_report`).

### 32. The deterministic replay oracle (byte-identical render sequences)

Problem killed: a fixed-seed benchmark that still cannot PROVE its
content — the headless loop mixed two clocks (state anchors stamped
from real `Instant::now()` at construction/reset, simulation advanced
on the virtual `sim_now` ladder), so microsecond real-clock residue
could flip a spawn boundary, an RNG reseed rung, or the 10s
density-noise bucket, and two runs of the SAME seed silently diverged
frame-by-frame. Visual claims (`frame_entropy`, density gini) were
sampled, not pinned. Mechanism (night-frontier-Z1, Proof Moat 1a):
`Cloud::anchor_virtual_clock(origin)` rewrites every wall-clock-derived
state anchor (glitch cycle, spawn integration, reseed ladder, phosphor,
quantum, `start_anchor`, the ecosystem/gust/drift accumulators, the
event scheduler) onto one caller-supplied origin, so every `elapsed`
inside the sequence is exact `k * period` arithmetic — zero real-clock
residue. On top of that, `--bench-replay` renders the pinned sequence
(compile-time seed, pinned 120x40 virtual screen — never the env, the
terminal, or the host — and every host-dependent surface such as
Crystal Dragon ambient polling force-disabled) and reports blake3
over EVERY frame via a canonical cell serializer (pinned color-tag
ordinals, exhaustive match, `width`/`height`/frame-number headers),
one `replay: frame=<k> hash=b3:` line per 25 frames plus one folded
`replay-digest: b3:` line whose fold commits to sequence length AND
order. Two runs of the same invocation must be byte-identical —
asserted in-process by the replay unit tests (independent Clouds,
independent RNG streams) and across processes by the CI replay-oracle
step, which runs the built binary twice and diffs the digest lines.
Effect: visual regression detection is per-byte ("this frame differs"),
the benchmark's `frame_entropy`/density-gini metrics become pinned
values instead of samples, and a buyer can re-verify the render
contract offline with two invocations and one `diff`. Honest scope:
the oracle pins the core render path (spawn lanes, movement, color,
phosphor, quantum, climate drift) for a fixed configuration — message
overlays, ghost-kanji events, and click effects are not part of the
replay surface (the headless loop never arms them; ghost events age on
wall-clock `.elapsed()` by design), and a legitimately different
engine release produces different digests — the digests are
version-bound, exactly like the provenance anchor.
Truth: `src/bench/frame_digest.rs` (the canonical serializer, the
color-tag law, the length-and-order fold), `src/bench/bench_replay.rs`
(the pinned sequence, the report format, the input echo),
`src/engine/cosmic_dragon_engine/cloud/spawn_reset.rs`
(`anchor_virtual_clock` — the clock-mixing kill), `src/bench/dispatch.rs`
(the `--bench-replay` route), `src/config/mod.rs` (the flag).
Verifier: the replay unit tests
(`cargo test bench::bench_replay::replay_tests` — cross-run identity,
input sensitivity, fold length/order binding, anchor completeness,
digest teeth); the CI replay-oracle step (two processes of the debug
build must end on the same `replay-digest:` line); and locally,
`cosmostrix --bench-replay | grep replay-digest` twice must print the
same hex.

### 33. The reproducible release chain (byte-identical two-host builds)

Problem killed: process identity was unclaimed. The provenance depth
matrix (entry 31) proves an ARTIFACT is intact and the replay oracle
(entry 32) proves the RENDER CONTRACT — but nothing proved that the
published hash is the hash anyone can rebuild from source. A
"modified build without a trace" — patched between build and publish,
or built from a tree that is not the tagged source — was invisible to
both existing proofs. Mechanism (night-frontier-Z1, Proof Moat 1b):
the last wall-clock leak in the binary is closed
(`COSMOSTRIX_BUILD_TIME` read `SystemTime::now()` per build; it now
honors `SOURCE_DATE_EPOCH` — the reproducible-builds.org convention —
with a loud cargo warning on an invalid pin instead of a silent
mis-pin), and the CI runs a standing two-host proof on every push:
`reproducible_build` is a two-runner matrix that each compiles the
pinned release shape (`pro-linux-amd64-v3-musl`: static musl,
x86-64-v3, LTO fat, codegen-units 1) from the lockfile with NO shared
cache, the same `SOURCE_DATE_EPOCH` (the commit timestamp), and the
release lane's exact RUSTFLAGS composition (`-C target-cpu=x86-64-v3`
plus `--remap-path-prefix` redaction of `${HOME}/.cargo` and `${PWD}`
plus `-Cforce-frame-pointers=yes`); `reproducible_verify` downloads
both artifacts and requires byte identity (`cmp`). Local re-verify on
one host (two cold builds, byte-identical): `bash
scripts/release/repro-build.sh` — opt-in, never wired into check-all,
because CI already burns the two full LTO builds per push. Honest
limits: the local script proves same-host process identity (one
checkout path, one machine — the CI two-runner job is the stronger
cross-host proof); the standing proof covers the Linux x86-64-v3 musl
lane, the exact shape the release ships, not every matrix lane; and
the two runners are independent machines only as far as GitHub
hardware isolates them — a hypothetical synchronized compromise of
both runners is outside the threat model (the GPG layer in
VERIFY_RELEASE.md is the backstop).
Truth: `build.rs` (`resolve_source_date_epoch` — the epoch law, the
warning fallback, the rerun-if-env trigger), `.github/workflows/ci.yml`
(`reproducible_build`, `reproducible_verify`), and
`scripts/release/repro-build.sh` (the local verifier).
Verifier: the CI `reproducible_verify` job must print
`REPRODUCIBLE RELEASE CHAIN: PASS` on every push (two hosts, one
`cmp`); the build-script test suite pins the epoch resolver
(`source_date_epoch_law_unset_invalid_and_valid` via the standalone
`rustc --edition 2024 --test build.rs` runner); and the doc section
[VERIFY_RELEASE.md](VERIFY_RELEASE.md) 4a tells the rebuild-yourself
recipe once.

### 34. The embedded engine oracle (the capability depth, D4)

Problem killed: the lock suites lived in CI, but the binary a buyer
runs carried no engine proof — the depth matrix (entry 31) certified
WHO built the artifact and that it is intact, yet nothing in the
artifact demonstrated WHAT the engine actually draws. A tampered
binary with pristine identity layers (or a fork claiming parity) had
no in-binary surface to contradict it. Mechanism
(night-frontier-Z1, Proof Moat 1c): a pinned synthetic sequence — a
60x20 monolith, TrueColor/Green, binary charset, 16 frames at the
anchored 60 fps virtual clock, every wall-clock anchor rewritten by
the entry-32 law and Crystal Dragon ambient polling forced off —
renders in-memory and folds (the entry-32 serializer) into one
blake3 digest, compared against a compile-time pinned
`[u8; 32]` vector. The depth ladder rises from an identity proof to
a capability proof: `cosmostrix sig` reports the fifth row
`D4 engine-oracle` (pass names the recomputed digest; a mismatch
names the divergence and flips the verdict to TAMPERED, exit 2), and
`--doctor` carries the same verdict as an `Engine oracle` section so
a user can quote a verified render contract next to the build
identity. Cost discipline, kept honest: milliseconds,
single-threaded, no terminal I/O, no threads; the only heap footprint
is the pinned synthetic arena plus one reused scratch buffer. The
vector is empirically cross-verified: the debug-glibc build and the
`pro-linux-amd64-v3-musl` release lane recompute the SAME digest
(IEEE float semantics hold across profile, libc, and target-cpu), so
one vector serves the test build and the shipped shape. Honest
limits: the vector is version-bound — it pins the pinned-toolchain
render of the pinned configuration, and it is regenerated deliberately
when the engine legitimately changes (the module test fails until it
does, so a stale vector can never pass CI silently); a binary built
outside the pinned lane that renders differently reports
engine-oracle FAIL — that is the moat working (the row certifies the
release-lane render contract), not a false alarm; and the oracle
covers one pinned configuration, not every scene and style — the
breadth contract stays with the lock suites and the replay oracle.
Truth: `src/bench/engine_oracle.rs` (the pinned sequence, the vector,
the regeneration law), `src/bench/frame_digest.rs` (the shared
serializer and fold), `src/diagnostics/sig_report.rs` (the D4 row,
the 5/5 verdict), `src/doctor/mod.rs` (the Engine oracle section).
Verifier: `cosmostrix sig` must end
`SIGNATURE: AUTHENTIC (5/5 depths verified)` with the D4 row naming
`b3:d00a460a... recomputes in-memory`; `--doctor` must show
`engine_oracle: pass`; the engine-oracle unit tests pin the vector
(`cargo test engine_oracle` — pin currency, cross-run
determinism, the manual fold, and the non-degenerate vector); the
installer release law gates on the 5/5 line.

### 35. The syscall-floor ledger (the kernel-floor proof)

Problem killed: the "single-syscall flush" claim was a document.
RENDER_ENGINE.md says "256 KiB single-syscall flush" and the P3
comments say "3→1 write_all per frame", but nothing measured the
KERNEL side — a flush-path regression that split a frame across two
write() calls, or dribbled per-cell writes, would ship invisibly
while every unit test stayed green. Mechanism (night-frontier-Z2,
Kernel Floor 2a): the supermassive battery runs the release-shape
binary under strace on a real pty
(`scripts/supermassive/syscall_floor.py`) and the ledger asserts the
kernel-visible floor: the count of write() syscalls carrying the
synchronized-output frame marker (`ESC[?2026h`) equals the
`--perf-stats` `frames_flushed` count — the renderer's own ledger of
`flush_ansi()` calls. The fingerprint is exact by construction: the
ONLY writer of the sync-open marker is the combined flush buffer
(the marker rides the frame's single write_all; the exit stack emits
only the sync END, and the v50 fix removed the init-time standalone
open), so mode toggles and the restore tail can never false-positive.
The ledger also pins byte-exact identity (the sum of kernel
frame-write lengths equals `total_ansi_bytes` + 16 sync-wrapper
bytes per frame), zero failed writes, zero short writes (a partial
write IS a frame split), and no BufWriter spillover at the pinned
80x24 screen; the parser handles strace's truncated-payload and
unfinished/resumed line shapes and never guesses silently — an
unparsable write line is a FAIL, not a skip. Effect: the claim is
now a CI invariant on the exact binary shape the release ships, and
a buyer can re-run the same proof locally with one strace command.
Free with the proof: the io_uring rejection record (Retired by
verdict, below) becomes evidence-backed — one write() per frame IS
the kernel floor, so io_uring for a single write is pure overhead.
Honest scope: strace is Linux-only by definition, so the ledger
proves the x86_64 release lane (the syscall contract is
arch-shared; the ptrace tool is not); the 8-second leg renders one
pinned configuration (80x24, xterm-256color with sync output); and
the byte-identity row assumes the 16-byte sync wrapper pair that the
fingerprint itself proves present.
Truth: `scripts/supermassive/syscall_floor.py` (the transport, the
strace parser, the ledger), `.github/workflows/supermassive.yml`
(the `syscall_floor` job),
`src/engine/cosmic_dragon_engine/terminal/io_recovery.rs` (the
combined flush the fingerprint targets).
Verifier: the CI `syscall_floor` job must end `MASS-VERDICT: PASS`
with the `syscall-floor writes/frame=1.000 PASS` row naming the
kernel write count and the flush count; locally, `strace -f -e
trace=write -xx -s 96 -o /tmp/sf.trace -- ./cosmostrix --duration 8
--perf-stats` on a real tty gives the harness the same input.

### 36. The zero-alloc ledger (the enforced 0.0 allocs/frame contract)

Problem killed: "0.0 allocs/frame on the lean path" was a benchmark
number — measured in one window, quoted once in the README's
Philosophy section, and free to regress silently between releases:
any future per-frame Vec, String, or pool spill would land in the
binary while every functional test stayed green, because nothing
asserted the ABSENCE. Mechanism (night-frontier-Z2, Kernel Floor
2b): the counting global allocator (TraceAlloc, wired in
`src/main.rs` since the DeepSeek benchmark restructuring) is
promoted from a measurement tool to a CONTRACT: the Cosmic Dragon
lock suite opens a pinned steady-state window (a 200-frame warmup on
the anchored virtual clock — the entry-32 replay law — then 180
ledger frames of the exact bench lean shape: `Cloud::rain_at` plus
the lean per-cell emission plus `clear_dirty`) and asserts the
allocator delta is EXACTLY ZERO — alloc, dealloc, realloc, and
retained bytes — with per-thread attribution (the FreeBSD CI
incident design) keeping concurrent test threads out of the count.
The ledger's teeth are themselves tested: a positive-control test
runs the identical window with one intentional per-frame allocation
and requires the count to read exactly the frame count, so a
refactor that silently detached the window from the allocator fails
before any false green can ship. Effect: CI goes red on the commit
that introduces a per-frame allocation — the claim and the
enforcement are now the same mechanism. Honest scope: the contract
covers the LEAN path (sim + diff + lean emission), the same scope
the 0.0 measurement claimed; the production-draw I/O path carries
~1.1 allocs/frame by design and stays measured, not contracted; and
one-shot surfaces (startup, config parse, palette rebuild) are out
of scope — the ledger measures the per-frame steady state only.
Truth: `test/cosmic_dragon_incubator/tests/alloc_ledger.rs` (the
window, the zero assertions, the positive control),
`src/diagnostics/alloc_trace.rs` (the counting allocator, the
per-thread design), `src/main.rs` (the wiring),
`test/cosmic_dragon_incubator/tests/lock.rs` (the INV-18 table row).
Verifier: `cargo test cosmic_dragon_incubator::tests::alloc_ledger`
— the zero test must report 0/0/0 across the window and the positive
control must count exactly the frame count; the suite runs on every
commit in CI.

### 37. The exact-state suspend/resume contract (the full-termios snapshot)

Problem killed: "SIGTSTP (Ctrl-Z) suspends in raw mode... Recovery is
automatic on `fg`/SIGCONT as long as nothing else wrote to the TTY" —
a README limitation whose recovery depended on the terminal staying
quiet and whose "stays in raw mode" claim was stale besides (the
suspend path's `disable_raw_mode` is a direct `tcsetattr` syscall,
immune to PTY byte jams). The real termios-axis gap: crossterm's
`disable_raw_mode` restores the termios it saved at SESSION START, so
any drift since then (another process `stty`-ing the same device, a
nested raw-mode tool) made both suspend and resume run on stale bytes.
Mechanism (night-frontier-Z3, Lapis 3b):
`src/platform/termios_snapshot.rs` captures the EXACT current termios
at SIGTSTP — before raw mode is disabled, via `tcgetattr` on the tty
device — and the SIGCONT reinit branch re-applies those precise bytes
over crossterm's canonical raw mode, so the render loop resumes on the
same termios it suspended with, byte for byte, regardless of anything
the device saw while the session was backgrounded. The escape-sequence
axis stays on the existing nonblocking restore + full repaint contract
(its own drift is covered by design: a dropped restore byte surfaces
as visual drift the repaint overwrites). A bare `kill -CONT` with no
snapshot in flight is a no-op — the pre-Z3 behavior. Honest scope: the
Windows/Termux hosts cannot be exercised in this repo's CI; the
observable contract is proven end to end by the supermassive torture
battery's suspend-resume leg (leg 6): SIGTSTP silences the stream,
shell-prompt bytes are written to the device while the session is
stopped, SIGCONT resumes with a repaint burst, and the session exits
by its own deadline with the full restore tail.
Truth: `src/platform/termios_snapshot.rs` (the capture, the exact
re-apply, the no-tty degradation), `src/interactive/signal_handlers.rs`
(the SIGTSTP capture arm), `src/interactive/event_loop.rs` (the
SIGCONT restore in the reinit branch),
`scripts/supermassive/torture.py` (leg 6, the end-to-end proof).
Verifier: the torture battery's `torture-suspend-resume` rows must all
read PASS in CI (silence, repaint, deadline exit, restore tail); the
snapshot slot contract is pinned by
`cargo test platform::termios_snapshot` (capture replaces, restore
consumes, a consumed snapshot never restores twice, and the no-tty
path degrades without panicking); the README Limitations list no
longer carries the SIGTSTP caveat — the contract lives in
[TERMINAL_LIFECYCLE_MATRIX.md](TERMINAL_LIFECYCLE_MATRIX.md) rows 5-6.

### 38. The synthetic terminal-gone quarantine (the pinned sudden-exit root cause)

Problem killed: "Windows / Termux: the `i` key may cause sudden exit"
— KNOWN_ISSUES' longest-standing "root cause (suspected)" entry. The
exit was the event loop treating ONE terminal-gone error
(`is_terminal_gone`: EIO/EBADF/BrokenPipe from crossterm's poll/read)
as a dead PTY; on Windows and Termux the documented delivery pattern
is a key event followed immediately by a SYNTHETIC EOF or transient
I/O error — a single error from a perfectly alive terminal. Mechanism
(night-frontier-Z3, Lapis 3a): `src/interactive/terminal_gone.rs`
splits the two error classes on PERSISTENCE — a real dead terminal
errors persistently, a synthetic one errors once — and quarantines a
terminal-gone error only when it lands within 150 ms of an accepted
key event AND is the first in that window. Any error outside the
window, or a second one inside it, is persistent real death and exits
exactly as fast as before (the post-SIGHUP fast-exit contract is
untouched; a truly dead fd errors on the immediate re-poll and exits
within milliseconds). Write-side failures are never quarantined — a
write error is the kernel reporting a broken fd from the syscall
itself. Honest limits: Windows ConHost and Termux cannot be exercised
in this repo's CI; the quarantine covers the documented
single-error-after-key pattern on both platforms by mechanism, pinned
by unit tests, with non-regression proven by the supermassive PTY
battery (a real EIO mid-run still exits; the quit paths are
unchanged).
Truth: `src/interactive/terminal_gone.rs` (the guard, the verdict
state machine, the four tests), `src/interactive/event_loop.rs` (the
four quarantine call sites, the key-activity arming).
Verifier: `cargo test interactive::terminal_gone` — one
error-after-key is absorbed, two are not, spontaneous errors exit
immediately, non-gone errors always propagate; the pty smoke legs and
the torture battery must stay green (no regression on real
terminal-gone handling).

### 39. Reader-aware emission (the consumer-side cost model)

Problem killed: the terminal as a black box. The producer-side sweep
(entry 25's incubator) priced the hot path in producer terms — bytes
out, cells touched — while the consumer's parse cost stayed invisible,
so congestion was learned only AFTER the pipe filled: the HUNT-23
drain backoff is reactive by construction, and the VTE fullscreen
entry documents exactly that weakness ("VTE's internal buffering
creates a feedback delay that prevents perfect stabilization").
Mechanism (night-frontier-Z3, Lapis 3c — the one NEW mechanism of this
lapis):
`src/central_control_power_dragon/reader_cost.rs` models the CONSUMER:
terminal families differ in what bounds their ANSI consumption
(byte-bound CPU parsers — the VTE family, konsole, foot; heap-bound
xterm.js; op-bound GPU renderers), each family gets a sustainable
byte-rate policy floor derived from the repo's own documented
arithmetic (the Tier-2 section's 7 MB/s xterm.js number, HUNT-24's
"2.4x overdrain" CPU derivation), and one frame's byte volume —
which the Z2 syscall-floor ledger already made kernel-exact — maps to
a predicted consumer parse time. `PowerManager::observe_reader_cost`
compares the prediction against the frame period and drives a reader
backoff through the SAME rise/fall discipline as the measured drain
backoff: the output cadence paces to the family's sustainable rate
BEFORE the pipe backs up. Zero new surface: the family resolves from
the existing HUNT-24 termdetect sensors (no new detection layers),
the lever is the existing FPS pacing, and unarmed managers (the
pre-3c shape, and every test harness) no-op. Honest limits: the
constants are policy floors, not claimed measurements — the reactive
drain backoff remains the source of truth for REAL latency, and the
model's only job is to engage the same lever earlier; the
calibration rows pin the model against the byte volumes the
supermassive legs captured (no false throttling of documented-healthy
volumes, no blind throttling of the documented VTE saturation), and
the static RLE threshold is validated against the consumer-side
byte crossover rather than made adaptive (the hot path stays
branch-free).
Truth: `src/central_control_power_dragon/reader_cost.rs` (the family
model, the predicted parse, the calibration rows, the threshold
validation), `src/central_control_power_dragon/power_manager/mod.rs`
(the reader backoff, the effective_fps stronger-of rule),
`src/interactive/event_loop_post_draw.rs` (the per-frame feed),
`src/engine/cosmic_dragon_engine/terminal/io_recovery.rs` (the
per-frame byte volume, exact by the Z2 ledger).
Verifier: `cargo test central_control_power_dragon::reader_cost`
(family ordering, monotonicity, calibration verdicts, the threshold
validation, the sensor mapping) and
`cargo test central_control_power_dragon::power_manager` (the wiring
teeth: an over-budget frame paces the CPU family and not the GPU
family, an unarmed manager no-ops, healthy frames decay to full
cadence); the CI pty/torture lanes must stay green (the model must
never false-throttle the documented-healthy 80x24 shapes).

## Retired by verdict

Not every idea earned its place. These were evaluated and rejected,
and the records are kept so the same debate does not rerun:

- **GPU image-mode rendering** — rejected because it breaks the
  character-grid aesthetic (every glyph must stay a real, selectable
  codepoint). Record: `docs/archive/cosmic_dragon/EXPLORATION.md`.
- **io_uring** — rejected twice, on cost and then on proof. The
  measured-cost egg benchmark
  (`src/cosmic_dragon_incubator/egg/io_uring_rejected.rs`) priced it:
  at 60 FPS one write() costs ~0.006% CPU and the theoretical ring
  savings are negligible. The syscall-floor ledger (entry 35) then
  proved the deeper point: the steady state is EXACTLY one write()
  per frame — the minimum any tty writer can emit — so io_uring for
  a single write replaces one syscall with ring setup plus a
  submission: strictly more overhead for the same operation. The
  record stands on proof, not preference.
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
