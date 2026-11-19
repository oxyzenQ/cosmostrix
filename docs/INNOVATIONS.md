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
the ruleset this page obeys); the [Proof Atlas](PROOF_ATLAS.md)
(entry 48) routes each claim's evidence chain in one page.

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

## The fifty-six, in dependency order

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
pool of particle values serves every effect — click ripple, shockwave
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
hash (`murmuration`), FABRIK chain dynamics (`dragon`), Keplerian
orbits (`black_hole`) — each in its own module under the cloud.
(The FABRIK mapping originally read `dna_helix`; the solver lives in
the dragon style's body chain — the code is the truth, corrected by
the conservation-ledger family landing, entry 53.)
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

night-depth-hardening-4 (2026-11-19) extends the contract across the
libc boundary. The original proof was same-host: two processes of one
build, one libm. The census of the executed replay surface found every
transcendental call on it is f32 (`powf`, `cbrt`, `sqrt`, `sin`, `cos`,
`exp` on f32 operands — the OKLab conversion in gradient/mod.rs, spawn
lanes, ecosystem drift, rain_at easing), and f32 routines in glibc
2.28+ and musl evaluate to the correctly-rounded result, so the two
libcs must agree bit-for-bit; every Linux distro at kernel 5.15 or
above ships inside that regime. ci.yml now publishes the debug build's
glibc digest as an artifact and `reproducible_verify` compares it
against the static musl release binary's digest (two libcs, two
runners, one required digest line). Debug-vs-release equality was
proven first on a developer host (both profiles print
`b3:7e938efa5efcb659dc214a9bf1fc84afdbbe8d6b8e0aab9f67b0b3606f1578d7`),
so the cross-profile half is measured and the cross-libm half is now a
standing CI gate: a future float call drifting off the f32
correctly-rounded law fails the push instead of silently breaking a
buyer's offline re-verification.

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
VERIFY_RELEASE.md is the backstop). The chain is closed end to end:
the `release.yml` publish lane exports the same commit-epoch pin and
builds `--locked` on the unix, windows, and android lanes (the
freebsd KVM lane keeps its own toolchain law), so a published
artifact embeds the pinned minute a third party rebuilds with - the
identity is now the PROCESS, not only the CI-lane artifact shape.
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
recipe once. Advanced form 2 (2026-11-18, night-depth-hardening-Z1)
turns the recipe from a manual trust exercise into a standing gate:
`release.yml` grows a `reproducible_crosscheck` job that rebuilds the
pinned v3-musl shape from the exact release commit on an independent
runner (publish lane's verbatim RUSTFLAGS composition, `SOURCE_DATE_EPOCH`
commit-epoch pin, `--locked`) and `cmp`s it against the packaged
tarball produced by `build_packages` on a different runner; the verdict
line is `REPRODUCIBLE RELEASE CHAIN (published == rebuilt): PASS` and
`publish_release` needs the job, so a release physically cannot ship an
artifact that the source does not reproduce. That closes the one seam
the first form left open — the CI two-host proof demonstrated the
PROCESS on a CI-lane artifact, while the published package was only
covered by the epoch-pin argument; now the packaged artifact itself is
byte-compared to a fresh rebuild in every release run. Honest limits:
the gate covers the Linux x86-64-v3 musl lane (the shape the recipe
rebuilds and the archive layout the PKGBUILD depends on), the freebsd
KVM lane keeps its own toolchain law, and a synchronized compromise of
the crosscheck runner AND the GPG signer remains outside the threat
model exactly as before.

night-depth-hardening-4 (2026-11-19) doubles the verify lane as the
determinism gate: the same `reproducible_verify` job now also compares
the release musl binary's `replay-digest:` against the debug glibc
build's digest (published by `build_test` as an artifact), so byte
identity of the artifact AND bit identity of the pinned render
sequence across libcs are both standing push-time gates (see
innovation 32).

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
The ledger is closed against the WHOLE output surface, not only
write(): the trace covers write, writev, pwrite64, pwritev,
pwritev2, and ioctl, and the mandatory-output ledger asserts zero
`writev`/`pwrite*` on any pty fd (write() is the only output syscall -
a scatter-gather or positioned write would bypass the floor) and
zero MUTATING tty ioctls between the first and the last frame write
(TCSETS*/TIOCSWINSZ/TIOCSCTTY and kin, plus any undecoded request -
a ledger never guesses; mode toggles live in the setup/teardown
window). Read-only queries (TCGETS*, TIOCGWINSZ) in the steady
state are OS-mandatory resize telemetry - the renderer polls the
winsize for resize detection - and are ledgered informationally,
not criminalized. Scope, stated
honestly: the input/wait/signal/timer vocabulary (read, poll,
epoll, nanosleep, signal delivery, futex) stays untraced HERE by
design - it is OS-mandatory transport residue, ptracing it slows the
traced child, and it pins no output claim. The residue is no longer
unledgered: the wakeup-floor ledger (entry 56, night-frontier-Z9)
traces exactly that vocabulary in its own leg and pins the render
thread's whole-frame budget - one blocking wait, one bounded
zero-timeout probe, one write, zero sleeps, zero yields, zero input
reads per frame.
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
with the `syscall-floor writes/frame=1.000 PASS`, `alt-output
tty=0 PASS`, and `tty-ioctl ledger mutating-steady=0 PASS` rows
naming the kernel counts; locally, `strace -f -e
trace=write,writev,pwrite64,pwritev,pwritev2,ioctl -xx -s 96 -o
/tmp/sf.trace -- ./cosmostrix --duration 8 --perf-stats` on a real
tty gives the harness the same input.

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

### 40. The measured colorimetry budgets (the ΔE-OK perceptual moat)

Problem killed: "no muddy midpoints" was a design claim. The polar
OKLab gradient's headline property was proven on one synthetic
opposing-hue pair (entry 18's INV-10), the shipped ramps were tuned by
eye, and nothing MEASURED the colorimetry of what actually ships — a
future gradient change that introduced a hue-wrap spike, an overshoot
artifact, or a chroma-collapse midpoint would have shipped while every
existing test stayed green. Mechanism (night-frontier-Z4a, Lapis 4a):
the Chroma Dragon lock suite now carries measured colorimetry on the
REAL production surfaces — all 44 themes, the real catalog stops, the
real production ramp. INV-20 pins the ΔE-OK perceptual budget: every
interpolation path between consecutive stops is sampled at
just-noticeable-difference resolution (adaptive arc-length two-pass
sampling over `gradient_path_oklab`, the production sampler's
pre-quantization math — added so the audit measures the LAW, not the
u8 display grid, which quantizes coarsely near black) and no
consecutive step may exceed one JND (ΔE-OK ≈ 0.02, the "What's My
ΔE(OK) JND?" 2026 average threshold); a ±1-channel-per-sample mirror
pin ties the shipped u8 path to the audited math. The shipped ramp's
band widths are the designed discrete aesthetic — deliberately wider
than one JND — so they are pinned separately by the measured band
ceiling (0.40; the widest band any theme ships today is 0.357,
NeonYellow). INV-21 pins the no-muddy-midpoints contracts as measured
chroma troughs: along every saturated segment's fine path the minimum
chroma keeps ≥ 0.80 of the weaker endpoint's chroma (measured worst
0.984 — polar carries chroma linearly, as designed), and every
interior entry of the shipped ramp keeps ≥ 0.50 of its neighbors'
chroma (measured worst 0.629, Snow[7]); near-gray segments are exempt
because the Cartesian fallback there is the visually correct path.
Effect: the claims are quantities now — a future theme edit, gradient
change, or floor retune that breaks perceptual continuity or collapses
a midpoint fails CI with the offending theme, segment, and ΔE value in
the message; the audit harness (`z4a_measure_...`) re-derives every
budget from the same table so a deliberate retune is a visible,
reviewable constant change, not a silent drift. Honest scope: the
budgets govern the TrueColor interpolation surface (the mode where
the gradient machinery's output is what renders); Color256/Color16
quantized ladders carry their own anchor contracts (tests_c16_anchor),
and the trough floors assert saturation retention, not a full ΔE00
appearance model.
Truth: `src/engine/chroma_dragon_engine/gradient/mod.rs`
(`delta_e_ok`, `oklab_chroma`, `gradient_path_oklab` — the
pre-quantization audit surface), `src/engine/chroma_dragon_engine/tuning.rs`
(the five Z4a constants with their measured provenance),
`test/engine/chroma_dragon_engine/tests/lock_inv20_z4a.rs` (INV-20,
INV-20b, INV-21a, INV-21b, the measurement harness),
`test/engine/chroma_dragon_engine/tests/lock.rs` (the INV table, the
9-E version bump).
Verifier: `cargo test lock_inv20_z4a` — JND continuity over ~300
traced segments, the band ceiling, both trough floors, and the
re-derivation harness must all pass; the engine version sentinel pins
"9-E (locked)".

### 41. The calibrated phosphor decay (the P1-class curve, Lapis 4c)

Problem killed: the trail afterglow was an exponential with a bare
hand-tuned rate — `PHOSPHOR_DECAY_RATE = 8.0` carried no provenance,
the per-frame decay factors were built inline in the decay pass, and
nothing said what the number MEANS physically or pinned the curve's
shape; a future retune could silently reshape the afterglow under
every test. Mechanism (night-frontier-Z4c, Lapis 4c):
`central_control_rains::phosphor_calibration` maps the decay curve to
the real CRT phosphor persistence data. The matrix-rain afterglow
emulates the P1 class — Zn2SiO4:Mn green willemite, 525 nm, the
phosphor of oscilloscopes and the green monochrome monitors the whole
aesthetic descends from, classified medium persistence (1-100 ms) by
the standard P-designation table — whose characteristic exponential
decay component sits at tau = 25 ms. A 25 ms afterglow is invisible at
terminal scale, so the engine runs phosphor time DILATED:
`PHOSPHOR_SIM_TIME_SCALE = 5.0` — one simulated second of decay
represents 5 phosphor seconds, a documented aesthetic dilation chosen
to reproduce the shipped rate BIT-IDENTICALLY (1 / (0.025 x 5.0) =
8.0 exactly in f32), so the owner's tuned trail envelope is preserved
byte for byte while the number gains its physical identity: the trail
is the P1 curve at 5x dilation (simulated tau 125 ms, simulated
half-life 86.6 ms). `PHOSPHOR_DECAY_RATE` is now DERIVED from the
calibration (a test pins the bit-identity — if the rate ever moves,
the commit must say "retune"), the per-frame decay LUT is built by
`phosphor_decay_lut` in one audited place (the same exp() law the
decay pass always ran, only the construction site moved), and the
cosmostrix parallax multipliers keep their documented meaning: each
layer's effective tau is the P1 sim tau divided by its multiplier
(back 65.8 ms, mid 108.7 ms, front 192.3 ms; half-lives 45.6 / 75.3 /
133.2 ms; bottom band accelerated by 1.8x). The module also carries
the P-designation reference classes the trail could emulate — P3
amber (~13 ms), P4 white B/W TV (~60 us), P7 radar dual-layer (long
component ~200 ms) — pinned as REAL published values with the honest
scope stated: only the P1 identity is load-bearing, and cross-class
ratios are deliberately not engine law because public persistence
tables quote decay to different brightness levels (10% vs 1%).
Effect: the afterglow claim is calibrated and pinned — tests assert
the rate identity, the half-life law (t_half = tau x ln(2), the LUT
lands exactly on 0.5 there), the 1/e tau point, the exponential law
(factor(2t) = factor(t)^2 per slot), the real reference table, and
the shipped trail envelope end to end (a tail-residual cell dies
after 13 / 22 / 38 frames per layer at 60 fps). A future retune of
tau, dilation, or shape fails with the physical consequence in the
message; a deliberate class-aware trail (amber schemes riding P3) is
now a constant change, not a research project.
Truth: `src/central_control_rains/phosphor_calibration.rs` (the law,
the reference classes, the LUT builder),
`src/central_control_rains/parallax.rs` (the derived rate, the
layer-multiplier provenance),
`src/engine/cosmic_dragon_engine/cloud/phosphor.rs` (the decay pass
consuming the calibration LUT),
`test/central_control_rains/tests_phosphor_calibration.rs` (the six
pins).
Verifier: `cargo test z4c_` — calibration identity (bit-identical
8.0), half-life law, tau point, exponential-law structure, reference
values, shipped envelope; the existing phosphor suites
(`cargo test phosphor`, 60 tests) must stay green — they pin the
same law the calibration now derives.

### 42. The wave-equation click shockwave (the solved ripple, Lapis 4b)

Problem killed: the mouse-click ripple was a DRAWING, not a wave — a
synthetic pair of kinematic rings whose radii grew linearly with
`elapsed * speed`, hand-shaped (primary bright ring + secondary echo
at 0.4x speed, an 8-cell annulus window), bounded by a pool of 4
slots so rapid clicks would not clobber each other, and living on a
wall clock (`Instant` births, a 1.8 s expiry sweep). Nothing about it
was physics: the ring had no memory, no interference, no boundary
behavior — and its equal-cell radii rendered as an ELLIPSE on a real
terminal (cells are twice as tall as wide). Mechanism
(night-frontier-Z4b, Lapis 4b): `cloud/wave_field.rs` solves the 2D
wave equation on the CELL GRID — the telegraph form
`u_tt + k*u_t + (k/2)^2*u = c^2*laplacian(u)` — with a leapfrog FDTD
scheme whose Courant numbers are pinned 11x inside the von Neumann
bound (`rx^2 + ry^2 = 0.089 <= 1` at the `MOUSE_FLASH_DT_MAX` = 1/120
sub-step ceiling; slow frames split into more sub-steps, capped at 8,
so a stall advances the wave instead of destabilizing it — and since
night-frontier-Z6 every sub-step is EXACTLY one `MOUSE_FLASH_DT_MAX`
quantum, see entry 46). A click
injects a compact impulse packet into ONE shared field; the wave
equation's LINEARITY does the rest — simultaneous clicks genuinely
interfere (the slot pool and its eviction are obsolete by physics),
the inject amplitude caps so a click storm saturates instead of
blowing up. The grid is PHYSICAL: `dy = 2*dx` (the cell aspect), so
the front advances half as many rows as columns and the ripple
renders as a CIRCLE (measured: horizontal 10-column and vertical
5-row arrivals land in the same window, ~15-16 frames for an
18.75-frame expectation — the residual gap is the coarse-grid
dispersion, documented, tolerance-pinned). First-order Mur absorbing
edges swallow the front at the screen border (no reflected ring), the
edges ride the same telegraph envelope as the interior (Mur alone
leaves a slow-decaying boundary reservoir — measured before the fix:
the field settled 2x late with a visible rim), and the restoring term
is what makes the life PROVABLE: in the pure damped wave equation the
spatial-mean mode converges to a nonzero constant (a visible residue
that never settles); the telegraph form rides `exp(-k*t/2)` on EVERY
mode, so the field provably crosses the settle floor at
`2*ln(A0/eps)/k` = 1.84 s (matching the synthetic era's 1.8 s life)
and the solver then clears itself — post-settle frames cost ZERO
work. The renderer's hot path strictly shrinks: one bounds-checked
amplitude lookup per cell replaces the per-wave distance math
(no sqrt, no loop over 4 ctx waves). Costs, kept honest: two
preallocated f32 grids (~15 KiB at 80x24), single-threaded, ~10 flops
per cell per sub-step, zero allocation after resize (a one-shot
reset surface, the same class as the phosphor arrays) — the zero-alloc
ledger's law extends here, pinned by a capacity-stability test.
Honest scope: a scalar brightness wave, not a full elastic
simulation; coarse-grid dispersion makes the front's numerical speed
slightly direction- and distance-dependent (pinned within a 35%
tolerance, not claimed exact); the field is not part of the replay
oracle's surface (clicks are never armed headless, same as the
pre-Z4b era) and the visual banding differs from the synthetic ring
by design — the crest/wake pattern is the physics, not a hand-drawn
dual ring.
Truth: `src/engine/cosmic_dragon_engine/cloud/wave_field.rs` (the
solver, the CFL law, the Mur edges, the settle lifecycle),
`src/central_control_rains/mouse.rs` (the shockwave constants: c,
k, the sub-step ceiling, the settle floor),
`src/engine/cosmic_dragon_engine/cloud/render.rs` (WaveCtx — the
solved slice the draw path borrows), `src/droplet/draw.rs` (the
per-cell amplitude read),
`src/engine/cosmic_dragon_engine/cloud/rain_at.rs` (the one step per
frame on the phosphor clock),
`test/engine/cosmic_dragon_engine/cloud/tests/tests_wave_field.rs`
(CFL margin, bounded long run, speed + isotropy arrivals, damping
envelope, superposition linearity, settle lifecycle, zero-alloc,
determinism, inject clamping),
`test/engine/cosmic_dragon_engine/cloud/tests/tests_mouse_shockwave.rs`
(Cloud wiring: injection, superposition, click storm, --no-effects
gates).
Verifier: `cargo test wave_field` and `cargo test mouse_shockwave`
— 20 pins, all asserting the physics (the superposition test is the
linearity proof: two clicks' field equals the sum of the single-click
fields to 1e-4); the full suite (3072 tests) must stay green, and the
zero-alloc ledger (entry 36) is untouched — the solver allocates
nothing in the frame path.

### 43. The paste fuse (the structural wedge kill, night-frontier-Z5a)

Problem killed: the one hostile-input wedge the repo had documented
but not closed (FUTURE_BACKLOG 5a). A dangling bracketed-paste START
marker (`ESC [200~` with no end marker — a hostile multiplexer or a
corrupted terminal; a real terminal always sends the pair) used to
wedge crossterm's internal paste buffer: every later byte, the `q`
quit key included, was swallowed into an unbounded `Vec<u8>` until the
torture watchdog SIGKILLed the session (rc=-9, run 40466357873). The
parked app-side options (cap/drain the buffer, or time out
unterminated pastes) were verified unimplementable against the actual
crossterm 0.29 source — the buffer is private, uncapped, and
undrainable. Mechanism (night-frontier-Z5a): the defense moved one
level DOWN, to the compiler — the crossterm `bracketed-paste` feature
is compiled out, so the paste-accumulation branch does not exist and a
dangling START parses as an ordinary unknown CSI sequence that is
discarded (`Err` clears the buffer); the `Event::Paste` variant
vanishes from the type system, so the wedge state is structurally
unreachable, not mitigated. The paste UX keeps its contract by
burst-rate arming: the `PasteBurstGuard` arms itself on three
plain-printable Presses inside 50 ms (far above human typing,
auto-Repeat excluded) and suppressed presses extend the window — the
original suppression law, unchanged — while the per-frame event drain
is wall-clock bounded (2 ms) against the neighboring
complete-junk-sequence storm class. Honest residual (documented in the
research doc): the first two characters of a paste stay live before
the window arms, and the wider unterminated-CSI accumulation class
remains upstream (bounded per-frame by the drain budget). Files where
the truth lives: `Cargo.toml` (the feature rationale comment),
`src/engine/cosmic_dragon_engine/terminal/mod.rs` (never enable),
`src/engine/cosmic_dragon_engine/terminal/cleanup.rs` + `restore.rs`
(the raw `ESC [?2004l` baseline reset for paste mode earlier processes
may have armed), `src/interactive/input.rs` (the self-arming guard),
`src/interactive/event_loop.rs` (the bounded drain).
`docs/research/NIGHT_FRONTIER_Z5A_PASTE_FUSE.md` is the full
investigation record. Verifier: the dedicated `paste-wedge` torture
leg (dangling START + hostile content + `q` must quit, rc=0, restore
tail intact) in `scripts/supermassive/torture.py` — 7/7 legs at the
Z5a SHA — plus the rewritten guard unit suite
(`cargo test burst`, `cargo test repeat`) and the
`benchmark/bench-labs/night_frontier_z5a/AB_REPORT.md` campaign
(steady state unchanged within noise; the A/B the parked note
demanded).

### 44. The afterglow temporal colorimetry law (INV-22, night-frontier-Z5b)

Problem killed: the temporal read of the phosphor trail — what the
decay actually does to emitted color, frame over frame — carried no
measured law. Z4a pinned the spatial palette paths, Z4c pinned the
curve's shape and half-lives, but the perceptual behavior of the melt
(the supra-JND steps, the death pop, the hue stability, the u8 floor)
was nobody's asserted quantity. Mechanism (night-frontier-Z5b): the
research MEASURED first — the charter's "temporal JND budget" framing
was honestly wrong on three counts (the melt is designed supra-JND;
chroma-magnitude shrinkage is the physics of dimming, not a defect;
the death is a designed one-frame pop) — and pinned the measured law
instead: the energy-ladder structure (monotone, terminating), the
nominal melt envelope as a twelve-entry two-sided table (layer x fps,
+10 percent band both ways), the load corners (stacked boosts, the
bottom-band emergency collapse to ~0.74 per step), the death pop
(0.135 bound), the floor pop (the u8 last-quantum vanishing, 0.14
global bound), and the hue-stability law (per-step drift <= 10
degrees, net visible-life shift <= 21 degrees — concentrated at the
deep-blue u8 foot, the medium's documented property). Files where the
truth lives:
`test/engine/chroma_dragon_engine/tests/lock_inv22_z5b.rs` (the seven
pins, every bound measured against the real pipeline),
`docs/research/NIGHT_FRONTIER_Z5B_TEMPORAL_COLORIMETRY.md` (the
investigation record, including the three charter corrections).
Verifier: `cargo test inv22` — 7 pins green; the shipped binary is
byte-for-byte unchanged (tests-only), so no A/B applies.

### 45. The unsafe law (the compiler-forbidden render core, night-frontier-Z5c)

Problem killed: the no-unsafe-in-renderer policy was a review
convention — nothing stopped the next unsafe block from landing in the
color engine, and nothing measured the OS-boundary surface the render
core legitimately needs. Mechanism (night-frontier-Z5c), two enforced
layers: `#![forbid(unsafe_code)]` on the five safe engine trees — the
Chroma Dragon, the Crystal Dragon, and the Cosmic Dragon's render core
(`cloud/`, `frame.rs`, `runtime.rs`); `forbid` cannot be overridden by
a nested allow, so unsafe there is a BUILD ERROR (a probe block was
compile-rejected during the landing). And the unsafe-census gate
(gate-keepers check 23, `scripts/gates/unsafe-census.py`) pins the
ENTIRE non-test unsafe surface line-by-line — 61 lines across 17
OS-boundary files (clock, platform guards, watchdog, reclaim, sysstat,
the terminal/tty recovery ladder, the io_uring rejection record):
growth, shrinkage, or a new file is a finding; the surface can only
change on purpose. The test harness's one legitimate unsafe
(edition-2024 `env::set_var`) moved OUT of the forbidden trees to a
single audited helper (`test/test_support::set_env_var`, mounted at
the crate root) instead of weakening the forbid. The census-first
lesson is recorded: the first grep undercounted the terminal ladder's
unsafe, so the whole-engine forbid plan was corrected to
core-forbidden + boundary-pinned — the boundary is exactly where
audited unsafe belongs. Files where the truth lives: the five module
roots (the attribute + the law comment),
`scripts/gates/unsafe-census.py` (the scanner that generated its own
pin), `test/test_support/mod.rs` (the carve-out),
`docs/research/NIGHT_FRONTIER_Z5C_UNSAFE_LAW.md` (the record).
Verifier: the compile-negative probe, the census-negative probe, the
gate-keepers 23-check sweep, and
`benchmark/bench-labs/night_frontier_z5c/AB_REPORT.md`
(performance-neutral — the attribute is a lint, zero codegen).

### 46. The wave-field energy ledger (the named Lyapunov bound, night-frontier-Z6)

Problem killed: the click shockwave's stability story (#42) was a set
of IMPLICIT bounds — CFL margin, a bounded-run test, a damping law —
with no running quantity that names the wave's energy and would NOTICE
if the solver ever started creating it. Worse, the ledger's first
empirical sweep found a real hole the implicit bounds could not see:
under ALTERNATING frame pacing (a synthetic 1/60 -> 1/45 -> 1/30 ->
1/90 cycle) the fractional-width sub-step schedule composed
amplification matrices whose product RESONATES — exponential energy
growth (~15% per 4-frame cycle), invisible to a constant-pacing
bounded-run test, and plausibly reachable in production where the
Power Dragon's throttle cycles and adaptive pacing oscillate the frame
delta. Mechanism (night-frontier-Z6, S4), three layers:
`cloud/wave_field.rs` now carries an explicit `EnergyLedger` over the
three-level leapfrog scheme's discrete Lyapunov functional — kinetic
at level n-1/2 plus the CROSS-TIME-LEVEL gradient products
`E = (1/2)*sum((u^n - u^(n-1))^2) + (rx^2/2)*<grad_x u^n, grad_x
u^(n-1)> + (ry^2/2)*<grad_y u^n, grad_y u^(n-1)>` (the textbook
discrete energy: dot the scheme with the centered velocity, the
kinetic telescopes exactly, the self-adjoint Laplacian part telescopes
into the staggered potential, the damping term `2a*||v||^2` is
positive-definite removal — the plain potential-on-current-level form
is NOT monotone, measured worst per-frame tick 1.3e-2, the cross-term
form measures exactly zero). The ledger attributes every movement to
named signed buckets (`injected` — the exact touched-set closed-form
delta per click, a fresh interior click deposits exactly
`rx^2*2 + ry^2*2` = 0.0889; `dissipated` — the per-frame delta;
`rescale` — measurement-scale reconciliation, identically zero under
the quantum clock) with the identity
`energy == injected - dissipated + rescale` holding exactly at every
point of every lifecycle. Second layer, the CAP law: every term of E
is a difference of values each bounded by INJECT_CAP, so per-cell
`E <= 2.5*cap^2 = 10.0` (measured storm operating point: 0.0002 per
cell — the cap is a rigorous ceiling, not the operating point), and
the per-frame pass plus per-inject local delta are f64 over the f32
state, zero allocation, closed form (no grid pass per click — the
click-storm cost is ~100 flops per click). Third layer, the fix the
ledger forced: the solver now runs a QUANTUM CLOCK — frame time
accumulates and every sub-step is EXACTLY `MOUSE_FLASH_DT_MAX` wide
(capped 8/frame, remainder carries over) — so the amplification matrix
is CONSTANT frame to frame and the alternating-h class is eliminated
by construction: the pathological cycle now decays monotonically to
exactly zero, settled, `dissipated == injected` to the last f64 bit.
Honest residuals, named not laundered: under click-storm saturation
the Mur boundary extrapolation reflects a sliver of oblique energy
back into the interior (measured worst per-frame tick 3.1e-2 at 80x24
saturated, decaying with the field — visible as a slightly negative
`dissipated` run); the visible timing quantization is bounded by one
quantum (8.3 ms on a 1.8 s life). Costs: one extra f64 pass over the
grid per ACTIVE frame (the settled path is untouched zero-work), zero
on settled frames, and the render path never reads the ledger.
Truth: `src/engine/cosmic_dragon_engine/cloud/wave_field.rs`
(EnergyLedger, the functional, the quantum clock, the touched-set
inject delta),
`test/engine/cosmic_dragon_engine/cloud/tests/tests_wave_field.rs`
(the Z6 pins: closed-form deposit, monotone steps, storm cap, ledger
identity, quantum-clock quanta, rescale-zero, reset semantics).
Verifier: `cargo test wave_field` — 16 wave-field pins (10 Z4b + 6
Z6), all asserting the laws; the full suite must stay green; the
replay digest is unchanged by the ledger (the render surface is
untouched — pinned by the CI replay-oracle step and by the A/B
report).

### 47. The determinism-anchored soak (the 24h canary lane, night-frontier-Z6)

Problem killed: the endurance methodology had been RETIRED — the
sampling helpers were removed as dead scripts in 936c7ba and
`docs/ENDURANCE.md` became a historical record — so the screensaver's
headline claim (stable for sessions exceeding 24 hours, no memory
growth, no handle leaks) had no standing enforcement anywhere, and the
analytic ladder (xenric-1) proved the million-year horizon on paper
with nothing empirical closing behind it. Mechanism
(night-frontier-Z6, S5): `scripts/depth/soak_determinism.sh` soaks ONE
long-lived headless process — the bench loop at `--bench-duration` up
to the parser's exact `24h` ceiling — with the WHOLE window wrapped in
a determinism anchor: the replay oracle's digest (entry 32) is taken
at t=0 and again after the soak, and the two must be byte-identical —
determinism used as the state-drift canary, the pattern this repo owns
outright: a wall-clock-independent replay that moves across a
multi-hour window names the drift nobody measured. Inside the window
the harness censuses /proc (RSS, fd count, threads) into a gitignored
CSV and enforces the pass law on all four legs: digest identical, RSS
plateau (linear regression over the final 80% of the window — the
first 20% carries allocator warm-up — below 2%/hour of the median,
the ENDURANCE acceptance line, verdict SHORT under 30-minute windows
because a trend needs a trend's worth of data), fd flat (span <= 2,
transient tolerance), and a clean exit with no panic in the logs.
Scheduled lane ONLY (`.github/workflows/soak.yml`, the estate-wide
Sunday 03:00 UTC slot plus manual dispatch) — the 2-minute per-push
law is untouched. The 24h horizon, honestly scoped: GitHub-hosted
runners cap a job at 6 hours, so the scheduled profile runs a bounded
150-minute leg while a self-hosted runner dispatches
`soak_minutes=1440` for the real 24-hour anchor — both run the SAME
pass law, the hosted profile is not a weaker one, and the lane fails
loudly on a dispatch that would exceed the hosted cap instead of
silently eating the bill. The lineage is kept honest: the removed
19-column extended CSV is named as the ancestor of the focused
5-column census, `docs/ENDURANCE.md` carries the current methodology
first and the historical record below it. Honest scope: the anchors
prove cross-process determinism across the window, not within-process
state identity at t=24h (the bench loop's own drift section
`first_half_fps`/`second_half_fps` carries the in-process half), and
the RSS law needs the window long enough to pin a trend — the lane
reports SHORT, never fakes a verdict.
Truth: `scripts/depth/soak_determinism.sh` (the harness, the pass
law), `.github/workflows/soak.yml` (the scheduled lane, the hosted
cap guard, the artifact upload on any outcome), `docs/ENDURANCE.md`
(the current methodology + the pass law table).
Verifier: the smoke run (`--minutes 2`: anchors identical, fd span 0,
exit 0, RSS verdict SHORT as designed), the regression unit check
(synthetic flat CSV 1.3%/h PASS vs a 500 kB/h leak 9.8%/h FAIL), and
the lane itself on its Sunday schedule — the artifact
(`soak-census-run-*`) carries the census + both anchor lines as the
audit record.

### 48. The Proof Atlas (the one-page evidence chain, night-frontier-Z6)

Problem killed: the evidence lived EVERYWHERE — a lock suite here, a
ci.yml job there, a bench-labs record in a third place, an audit doc
in docs/research/ — so a buyer, an acquirer, or an AI evaluator who
wanted to verify any claim had to know the tree well enough to find
the artifact without a map, and nothing structural stopped a doc
claim from drifting away from its evidence. Mechanism
(night-frontier-Z6, S6): [PROOF_ATLAS.md](PROOF_ATLAS.md) — one
master page routing every innovation entry to its
proof artifact(s) — the test file, the CI lane, the bench-labs A/B
record, the research/audit doc — with the verifier command or the
verdict line to look for, plus the standing-lanes table (what every
workflow pins and on which cadence) and the one-sitting audit recipe
(full suite, gate-keepers, the offline replay re-verification,
spot-check any row). The atlas is DOCS-ONLY by construction: no
version bump rides it, no A/B campaign applies, it changes no code
and no lane. The honesty law it carries: every row names at least
one artifact that exists in the tree today (the docs-audit gate
sweeps every relative ref on every push, so a rotting link fails
the build); where the strongest evidence is historical (a dated
research doc, an A/B snapshot) the row labels it as the past; and a
doc claim that cannot trace to an atlas row is a FINDING, not a
fact. The atlas routes — it never becomes the truth itself; `src/`
outranks it, exactly as the Innovation Ledger's own intro says of
this page.
Truth: `docs/PROOF_ATLAS.md` (the atlas, the honesty law, the lanes
table, the audit recipe), `scripts/gates/docs-audit.py` (the link
sweep that keeps every row alive).
Verifier: the link sweep (113 refs, all resolve — the audit output
below the docs-audit run is empty), the docs-audit gate on every
push, and the one-sitting recipe itself: `cargo test` green,
`./scripts/gate-keepers.sh` green, the replay digest re-verified
offline.

### 49. The never-be scope contract (the enforced non-goals, night-frontier-Z6)

Problem killed: nothing structural stopped the slow identity drift
every long-lived renderer eventually faces — a daemon mode here, an
audio visualizer there, a plugin loader after that — each a polite
feature request that imports hard-latency threads, network surface,
or third-party code into a process whose whole value is determinism,
a measured syscall floor, and a closed unsafe surface. A contract
that lives only in prose is a press release: nothing told a
contributor, an AI agent, or the owner's own future PR where the
product's edge is, and nothing failed when the edge was crossed.

Mechanism (night-frontier-Z6): the never-be contract — a closed list
of what cosmostrix will NEVER be (a daemon or background service, a
sound engine, a game, a network service, a GUI application, an ML
runtime, a plugin host, a cloud or distributed system) — written into
the README's what-this-is-not section with the reason each identity
is impossible, and enforced by `scripts/gates/scope-census.py`
(gate-keepers check 24): the direct dependency manifest is pinned
exactly (twelve runtime crates — crossterm, clap, rand, bitvec,
smallvec, unicode-width, notify, sha2, blake3, plus the per-target
signal-hook, libc, ctrlc — and two test-only dev crates, every one
local-capability), src/ is swept zero-tolerance for the capability
markers those identities need (socket APIs, embedded network stacks,
audio backends, windowing toolkits, dynamic-library loaders, ML
runtimes), and the process::Command census is pinned to its two
sanctioned sites (terminal restore; the user-invoked update check
that delegates to curl/wget — no embedded HTTP stack). Transitive
dependencies are out of the gate's contract by design: the pin is
what cosmostrix declares and calls, and the transitive graph stays
cargo-deny and cargo-audit territory. The registry's entertainment
category is discoverability routing, not a capability claim. The
contract can only change on purpose: a crossing PR fails the gate
until the owner consciously re-lands the pin.

Truth: README.md (the never-be contract table),
`scripts/gates/scope-census.py` (the manifest pin, the capability
sweep, the command census).

Verifier: the gate-keepers sweep on every push — the
`scope-census: OK` verdict line (twelve runtime plus two dev direct
dependencies exactly, zero capability markers in src/, the two-site
command census) — and the contract table in the README, which the
docs-audit gate keeps linked.

### 50. The lock census (the pre-advisory graph tripwire, night-frontier-Z6)

Problem killed: cargo-audit and cargo-deny are reactive — they catch
a crate once an advisory exists in RustSec or once a name lands a
ban list. The 2026-08-20 arrayref attack proved the gap that leaves:
a compromised maintainer republished arrayref 0.3.10 with a NEW
dependency on a freshly-published payload crate (proc-macro1, whose
build script downloaded a second stage), and the family lived on
crates.io for 86–107 minutes — inside the window where no advisory
exists to scan for. Any project running a routine `cargo update`
inside that window swallowed the payload with every check green.

Mechanism (night-frontier-Z6): the lock census —
`scripts/gates/lock-census.py` (gate-keepers check 25) pins the
STRUCTURE of Cargo.lock: the 121-name crate set and the 124-entry
package count, generated at the pin SHA. A new name (the payload
crate's entry move), a second version line of a known name (the
duplicate-entry move), or any member of the named attack family (the
belt for seats without cargo-deny) is a finding the moment it
reaches the lock — advisory or no advisory. Version drift inside the
pinned structure is deliberately not a finding: the lock pins exact
versions with checksums, every CI build is --locked, and routine
updates are how advisories get consumed. The census landed alongside
the deny.toml tightening of the same commit: yanked = deny (the
attack's republish shape), the nine-name attack family banned
name-wide, wildcard requirements denied, unknown git and registry
sources denied. The graph's structure can only change on purpose.

Truth: `scripts/gates/lock-census.py` (the name census, the entry
count, the kill list), `deny.toml` (yanked deny, the attack-family
bans, wildcard and source deny), docs/SUPPLY_CHAIN.md (§1 rule 5,
the census law).

Verifier: the gate-keepers sweep on every push — the
`lock-census: OK` verdict line (121 names across 124 entries,
exactly the pinned graph, zero malicious-family crates) — plus the
landing-time crates.io sweep (all 124 locked versions verified live
and unyanked on 2026-11-19), and `cargo deny check all` in the
`ci.yml` deny job enforcing the tightened policy upstream.

### 51. The fault-floor ledger (the kernel-side memory mirror, night-frontier-Z7)

Problem killed: the zero-alloc ledger (entry 36) proves what Rust
ASKS for, and the counting allocator cannot see what the KERNEL
actually grants. Stack growth past the warmup high-water mark,
allocator arena growth outside the counted calls, and any allocation
that bypasses the global allocator hook all surface as page faults —
a regression class every existing ledger is blind to, shipping while
INV-18 stayed green. Mechanism (night-frontier-Z7, Kernel Floor 2c):
the Cosmic Dragon lock suite grows a second ledger over the SAME
pinned window — the INV-18 rig shared verbatim (one window shape,
two axes) — and asserts the calling thread's page-fault counters read
EXACTLY ZERO minor and major faults across the 180 lean frames. The
counters come from `/proc/thread-self/stat` (Linux 4.1+), which
resolves to the CALLING THREAD's task entry: per-thread attribution
with no allocator hook and no unsafe — the FreeBSD CI incident design
read straight from the kernel instead of reconstructed from allocator
calls, so concurrent test threads cannot pollute the delta. Zero
minor faults says no new page of ANY class entered the thread's
address space during the window ("Rust requested nothing" rises to
"the kernel granted nothing"); zero major faults pins the no-paging
clause. The reader is deliberately heap-free — one `File::read` into
a fixed stack buffer, parsed in place — because the measuring path
must not fault either (the first landing's `read_to_string` reader
measured exactly one arena fault under libtest's parallel execution,
the precise blind spot this ledger exists to close). Teeth, the
house pattern: the positive control runs the identical window with
one deliberate 2 MiB fresh-page touch mid-window and requires the
minor count to rise; the bound is a floor, not an exact count,
because the exact page count of one allocation is the kernel's and
the allocator's to choose. Honest scope: the LEAN path (entry 36's
exact scope) on Linux — the kernel-floor family is Linux-anchored
(the strace syscall-floor lane is Linux-only too); other hosts keep
the INV-18 allocator contract; and exotic environments (NUMA page
balancing, memory scanners) can inject minor faults no allocator can
cause — the failure message names the counters and the suspected
environment cause. Truth: `test/cosmic_dragon_incubator/tests/fault_ledger.rs`
(INV-19, both directions), `test/cosmic_dragon_incubator/tests/alloc_ledger.rs`
(the shared pinned rig), `test/cosmic_dragon_incubator/tests/lock.rs`
(the inventory row). Verifier: `cargo test --locked lock_inv19` —
the zero window AND the fresh-page control; the MSRV lock lane runs
the filter on every push, and the supermassive arm64 lane runs it on
real ARM silicon.

### 52. The cross-architecture oracle parity (one rain, two silicons, night-frontier-Z7)

Problem killed: the D4 engine-oracle vector (entry 34) was
empirically cross-verified on the x86_64 axes only — debug-glibc vs
the pro-linux-amd64-v3-musl release shape (the ci.yml cross-libm
leg) — while the ARM release shapes shipped (pro-android-aarch64 for
Termux, pro-native-arm64-musl for arm servers, pro-macos-aarch64 for
Apple silicon) never recomputed the pinned vector on ARM silicon. If
the arch's f32 codegen diverged, the determinism contract (the
replay oracle, the soak, the D4 row a buyer quotes) would silently
not extend to the machines it ships on — and "should hold (strict
IEEE)" is not the house style. Mechanism (night-frontier-Z7): the
supermassive battery grows two legs. `sig_parity` runs `sig` on the
release shape on BOTH arches (x86_64 and the ubuntu-24.04-arm
runner) and requires exit 0, the five-of-five `SIGNATURE: AUTHENTIC`
verdict, and the D4 engine-oracle pass row — the same compile-time
pinned vector recomputed on each silicon; a mismatch reports
TAMPERED and fails the leg (either outcome is the moat working, and
the report is uploaded so the recomputed digest is quotable per
arch). `arm_lock_suite` runs the `lock_` invariant families (the
cosmic locks including the zero-alloc ledger INV-18 and the
fault-floor ledger INV-19, the chroma and crystal locks) under the
SHIPPED aarch64-unknown-linux-musl target on the ARM runner — the
deeper half `sig` cannot reach, since the oracle and replay module
tests do not carry the `lock_` prefix (the D4 vector's in-suite
cross-check stays with the ci.yml lanes). Honest scope: the android
shape (pro-android-aarch64) cannot execute in CI — a bionic-linked
binary does not run on a plain Linux arm host — so its parity
inherits from the arm64-linux proof plus strict-IEEE, and any
divergence surfaces on-device through `--doctor`'s engine-oracle row
(entry 34's embedded proof the user can run themselves); the macOS
arm shape builds in ci.yml while its runtime proof is the same
strict-IEEE argument plus the doctor row. The same landing fixes a
pre-existing workflow bug: musl_shards declared `needs: usl_compile]`
(a stray bracket made the reference a nonexistent job name; the
platform tolerated it, but the ordering was never guaranteed by
spec) — the declared dependency is now the real one, `[musl_compile]`.
Truth: `.github/workflows/supermassive.yml` (the `sig_parity` and
`arm_lock_suite` jobs), `src/bench/engine_oracle.rs` (the pinned
vector), `src/diagnostics/sig_report.rs` (the depth rows). Verifier:
the supermassive legs on every push (the `sig-parity-aarch64
exit=0 D4=pass` and `arm-lock-suite exit=0` MASS-RESULT rows), plus
`cosmostrix sig` locally on any host.

### 53. The conservation-ledger family (six solvers, six named contracts, night-frontier-Z8)

Problem killed: the wave field carried the only named discrete
quantity (entry 46) — the other five original motion-math systems
(plus the FABRIK chains, which entry 24's mapping mislabeled as
dna_helix when the solver lives in the dragon style; the code is the
truth and this landing corrects the mapping) were "original motion
math" by authorship alone: no named quantity, no measured bound,
nothing to catch a solver regression that stays inside the visual
noise. Mechanism (night-frontier-Z8, INV-20, one suite-sized fixture
beside the per-style rigs): each solver gets its honest contract —
the form the math actually supports, not the form a headline would
want:

- Keplerian orbits (black_hole): the angular-momentum ledger. The
  infall velocity update is CENTRAL (the gravity blend and the
  influence falloff scale only the magnitude; the direction stays
  radial), so under the semi-implicit Euler ordering the specific
  angular momentum L = x*vy - y*vx is conserved EXACTLY per substep
  wherever the accretion brake is absent — the closed form:
  L' = (x + v'*dt) x v' = x x v', so L' - L = x x (v' - v) =
  dt (x x a) = 0. Asserted to f32 rounding on a near-circular orbit
  seeded above the capture radius (the window guard keeps the scope
  honest), plus a cloud-level bounded-drift pass over the real pool.
  The accretion brake is the designed L sink and stays disclosed.
- PIC/FLIP (flux): the pool ledger — the active count IS the
  bookkeeping truth (spawn +1, retire -1, nothing else) and the
  velocity clamp is a hard ceiling; both asserted every frame.
- Physarum stigmergy (physarum): the mass-balance budget — the trail
  total must sit inside the closed-form window every frame (growth
  bounded by the per-agent deposit budget at pace_max, shrink
  bounded by the documented decay floor); a breach names unaccounted
  mass creation or a decay that broke the frame-rate-independence
  law (NIGHT-hunter-10).
- Boids (murmuration): the flight-band ledger — every ACTIVE bird
  sits inside the documented speed band after every frame, and no
  bird moves farther in one step than the band allows (active
  frames paired only; dormant slots carry no flight contract).
- Lorenz RK4 (lorenz): the absorbing-set contract — the honest
  scope: a chaotic system has NO monotone energy ledger; the
  contract is the measured state envelope (with margin) plus
  absorption promptness (no mote outlives its lifetime).
- FABRIK chains (dragon): the spacing ledger — the constraint pass
  lands every segment at EXACTLY DRAGON_SEGMENT_SPACING from its
  predecessor (closed-form projection), so chain integrity is an
  exact per-frame invariant, not an aesthetic.

Teeth, the house pattern: the kepler drift helper must DETECT a
deliberately injected non-central kick — a ledger that cannot see a
violation cannot enforce one. Honest scope: the ledgers read state
through the family's existing pub(crate) surface (flux motes, lorenz
motes, birds, dragons were already exposed; physarum's trail field
joins the same convention — it was the style family's only private
state); the runtime render path is untouched, so entries 32 and 36
see no new surface; the bounds are physics bounds (f32, libm, and
codegen tolerant — they hold on the musl shards and the arm64 parity
lane by form), not bit-exact pins. Truth:
`test/engine/cosmic_dragon_engine/cloud/tests/tests_conservation_z8.rs`
(the family), `src/engine/cosmic_dragon_engine/cloud/type_rain/physarum/physarum.rs`
(the field-visibility join). Verifier: `cargo test --locked lock_inv20`
— the eight family pins plus the chroma suite's own INV-20 riding
the same filter; the MSRV lane, the musl shards, and the arm64
parity lane run the filter on every push.

### 54. The control-loop resonance census (the no-limit-cycle proof, night-frontier-Z8)

Problem killed: entry 46 FOUND a real resonance — under alternating
frame pacing the wave solver's amplification matrices composed a
~15 percent growth per 4-frame cycle — and named the production
reach: the Power Dragon's throttle cycles and adaptive pacing
oscillate the frame delta. The solver got its Lyapunov functional;
the CONTROLLERS did not. The measured drain backoff
(S-master-HUNT-23), the predictive reader backoff (entry 39), and
the perf-pressure accumulator rise and fall on the same cadence the
resonance rides, and nothing asserted their interaction cannot
compose a limit cycle. Mechanism (night-frontier-Z8): the census
drives PowerManager through the adversarial scenario matrix — the
measured pacing sawtooth (60 -> 45 -> 30 -> 90), the byte sawtooth
the reader model damps, the combined storm (sawtooth + byte flood +
a thermal pulse train), and a clean release window after each storm —
and asserts the discrete-control contract at every frame:
BOUNDEDNESS (both backoffs, the pressure accumulator, the visual EMA
inside their documented closed intervals; effective_fps between the
floor and the base; every signal finite), RELEASE (each axis decays
into the healthy band within the closed-form frame count the
documented fall rate gives, and effective_fps returns to the base
cadence), and NO AMPLIFICATION (under sustained alternation the
ratchet saturates into a period-2 ripple whose spread is exactly one
fall step — a bounded controlled response at the ceiling, not a
growing oscillation; the late window's spread must not exceed the
step the input forces). The census's honest finding, recorded here
so the shape is not re-derived: at the ceiling the rise clamps while
the fall keeps stepping, so the axis RIDES a 0.002 ripple — that is
the designed bounded response, and the contract pins its
non-amplification instead of pretending a flat line. Teeth, the
house pattern: the storm's raw unclamped accumulation must exceed
the ceiling (30 frames x 0.05 = 1.5) while the observed value reads
exactly 1.0 — the clamp is load-bearing, and the boundedness
assertions guard a real edge, not a tautology. Honest scope: the
census drives PowerManager at the unit level (deterministic,
exact); the cloud-side consumers run in their own suites; one
cfg(test) read-only accessor (reader_backoff_for_test, the
visual_pressure precedent) exposes the predictive axis. Truth:
`test/central_control_power_dragon/power_manager/tests_resonance_z8.rs`
(the census), `src/central_control_power_dragon/power_manager/mod.rs`
(the cfg(test) accessor). Verifier: `cargo test --locked
lock_resonance` (the four scenarios); the lock_ filter rides the
MSRV lane, the musl shards, and the arm64 parity lane on every push.


### 55. The information ledger (the field's named entropy and Gini bands, night-frontier-Z9)

Problem killed: the benchmark's two visual metrics — `frame_entropy_bits`
and `density_gini` (`src/bench/bench_visual.rs`) — were TELEMETRY: a
solver regression that stayed inside the visual noise (one noise octave
silently dropped by a refactor, a decay LUT reshaped by a retune, a
spawn drift that slowly concentrates the rain into a few columns)
passed every functional test and every lane, because nothing asserted
the INFORMATION SHAPE of the rendered field; long-session drift — the
exact class the screensaver claim and the entropy drift ecosystem
(entry 9) live or die by — was guarded only by a human eye.

Mechanism (night-frontier-Z9, INV-21): each of the fourteen rain
styles is driven through the shared ledger rig (the INV-18 window
shape, one variable varied — the style) on the anchored virtual clock
(the Z1 replay law), and its dirty-cell column distribution is
measured per frame with the EXACT math the production sampler uses.
Four named quantities are pinned per style and asserted both
directions: the window-average entropy band, the window-average Gini
band, the per-frame minimum-entropy floor, and the sampled-frame
floor (a solver that stops emitting diffs — a dead field, the worst
long-session failure — cannot hide inside the averages; Aeolian
legitimately idles 42 of 300 anchored frames, and that count is
pinned too). The bands are re-derived from the same measurement the
rig reruns anywhere — the replay law makes the entropy sequence
exactly reproducible, so these are derivable contracts, not sampled
statistics; margins are physics bounds (+/- max(10 percent, 0.25
bit) on the averages, 0.05 on the Gini, 0.15 bit on the per-frame
floor), the conservation family's cross-arch precedent. Teeth: the
port-sync cross-check asserts the ledger's math equals the production
sampler's finalize output to f64 rounding (a `bench_visual` drift
from the ledger is named, not silent), and the collapse positive
control feeds a single-column distribution and a uniform distribution
through the same checker the contract runs — one must REJECT, one
must PASS. The landing's bonus micro-pin: the PRNG cycle-horizon law
— every random stream draws from rand 0.10 StdRng (ChaCha12,
documented 2^128 period), the worst-case LTS year (bench-max screen,
FPS cap, 24/7) consumes 60.77 bits of it, and the named 2^66 margin
is asserted; determinism cannot degrade into visible repetition over
any service lifetime. Honest scope: the bands pin the LEAN
bench-parity window (Green/Mono, binary charset, 120x40, 60 fps
anchored, Crystal Dragon off) — the same scope every ledger in the
suite pins; Monolith and Aeolian carry zero per-frame entropy floors
by measurement (their motion math legitimately concentrates every
change into one column at some anchored frames), so their average
bands are the load-bearing contract; a deliberate solver retune is a
visible band-table re-derivation in the same commit, never a silent
drift.

Truth: `test/cosmic_dragon_incubator/tests/info_ledger.rs` (the band
table, the port, the checker, the horizon law),
`test/cosmic_dragon_incubator/tests/alloc_ledger.rs` (the
style-parameterized rig — one window shape, three ledger axes),
`test/cosmic_dragon_incubator/tests/lock.rs` (the INV-21 row).

Verifier: `cargo test --locked lock_inv21` — the 14-style band sweep,
the port-sync cross-check, the collapse control, and the horizon law
must all pass; the full suite (3105 tests) stays green, and the
MSRV lane, the musl shards, and the arm64 parity lane run the filter
on every push.


### 56. The wakeup-floor ledger (the whole-frame syscall budget, night-frontier-Z9)

Problem killed: entry 35 proved the OUTPUT floor — exactly one
write() per frame — and its own honest scope left the
input/wait/signal/timer vocabulary untraced ("OS-mandatory transport
residue"). The energy claim actually lives there: between frames the
process must sleep, and a regression that swapped the pacing wait for
a busy loop, added a stray per-frame sleep, or started reading the
input fd every frame would ship invisibly while every unit test and
every lock suite stayed green — the C-states story (the process
spends most of its wall time asleep) had no kernel-visible proof at
all.

Mechanism (night-frontier-Z9, Kernel Floor 2d):
`scripts/supermassive/wakeup_budget.py` runs the release-shape binary
under `strace -f -ttt` on a real pty (80x24, xterm-256color, 60 fps,
`--intro none` so the window is pure rain steady state — an existing
flag, no new surface) and pins the RENDER THREAD's measured per-frame
budget with per-TID attribution: exactly one write() (entry 35's
floor, re-pinned inside this window), exactly one blocking wait (the
pacing epoll_wait with a nonzero timeout), at most 1.5 zero-timeout
probes per frame (the designed near-deadline/drain probe, bounded not
free), ZERO sleeps and ZERO sched_yields, and ZERO reads on the tty
input fd — every in-window read is the Power Dragon's /proc
telemetry, counted informationally at its named cadence (measured
8.01/s, ceiling 20/s). The wait-family census pins epoll_wait as THE
mechanism: any ppoll/poll/select inside the window is a finding — the
wait mechanism changed under the ledger and the budget must be
re-derived on purpose. Wake precision is measured from the -ttt
timestamps: each blocking wait sleeps its requested timeout to a
p95 of 1.61 ms locally (ceiling 5 ms; the final half-millisecond is
the designed spin-wait, the pacing law, not a defect). The auxiliary
threads are NAMED, not criminalized: the watchdog thread's one-second
clock_nanosleep per interval (measured 1.00 sleep-sec/sec, by design,
entry 17's self-healing stack) is ledgered informationally — the
residue entry 35 left untraced is now a measured, named row. Teeth,
proven by injection: a synthetic window plus a render-thread sleep,
a foreign wait family, an input read, a busy-loop of extra blocking
waits, and an unparsable line each FAIL the ledger — a clean window
passes; unparsable lines are findings, never skipped (the entry-35
parser law). Honest scope: the budget pins the lean steady state of
the release lane on Linux (strace is Linux-only by definition, same
as entry 35); input-driven windows (a user typing) and the pause/
resume paths are out of scope — the leg renders one pinned
configuration with no input; the watchdog's cadence is pinned
informationally because auxiliary threads are OS-mandatory by
design.

Truth: `scripts/supermassive/wakeup_budget.py` (the transport, the
per-TID parser, the ledger, the bounds),
`.github/workflows/supermassive.yml` (the `wakeup_floor` job),
`src/interactive/event_loop.rs` (the spin-sleep pacing the ledger
measures), `src/engine/cosmic_dragon_engine/terminal/io_recovery.rs`
(the combined flush whose fingerprint the window keys on).

Verifier: the CI `wakeup_floor` job must end `MASS-VERDICT: PASS`
with the rows `writes/frame=1.000 PASS`, `blocking-waits/frame` in
[0.90, 1.25] PASS, `probe-waits/frame` under 1.5 PASS,
`render-sleeps=0 render-yields=0 PASS`, `input-reads=0 PASS`, and
`wake-precision-p95` under the 5 ms ceiling; locally,
`python3 scripts/supermassive/wakeup_budget.py --binary
<release-shape> --trace /tmp/wf-trace.log --log /tmp/wf-capture.log
--mass /tmp/mass-wakeup.log --label wakeup-floor-8s --duration 8
--budget 30` on a real tty reproduces the same rows.

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
