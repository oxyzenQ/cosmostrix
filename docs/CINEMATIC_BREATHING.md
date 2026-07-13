<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cinematic Breathing Language

## Status

This is the authoritative vocabulary document for v5.0.0 Nightfall and all
future releases. All references to visual rhythm, atmospheric transitions,
and scene behavior in documentation, help text, and source comments must
use the terms defined here. New breathing vocabulary must be added to this
document before implementation.

## What is Cinematic Breathing?

Cosmostrix is not a static screensaver. Its visuals have rhythm, weight, and
intention. When rain falls across the terminal, it does not simply repeat the
same frame at a constant rate — it breathes. Cinematic breathing is the
vocabulary we use to describe how the rain lives on screen.

The concept draws from film editing, where pacing is not an accident but a
deliberate creative choice. A director does not cut randomly between scenes;
each transition carries meaning. Similarly, Cosmostrix treats visual state
changes as intentional acts with rhythm and purpose. The rain accelerates,
slows, thickens, thins, brightens, and dims — not at random, but according
to a shared language that both developers and users can reference and
understand.

Cinematic breathing applies to three layers of the visual experience. The
first layer is the base rain itself: how fast columns fall, how dense they
are, how characters cycle through their glyphs. The second layer is
atmospheric modulation: the subtle (or not-so-subtle) shifts that the
controlled atmosphere system applies on top of the base rain. The third
layer is scene identity: how different scenes like monolith, matrix, and
signal each express a distinct visual character through their own pacing
and structural choices.

This document defines the vocabulary for all three layers. It establishes a
contract: when documentation or code says "whisper," every reader and
developer shares the same mental model of what that means. When a user
activates a pulse regime, they can expect a specific kind of intensity
shift — not a surprise, but a promise.

## Breathing Vocabulary

### Rest

**The baseline state.** Rain falls at default speed and density. No
atmosphere effects are active. The visual equivalent of breathing normally
— steady, even, unremarkable in the best sense.

**Visual description:** Columns descend at their configured speed with
uniform density. Colors follow the selected palette without modulation.
Glitch intensity is at the chosen level. The screen looks alive but calm,
like a quiet forest with leaves falling at a natural rate. There is no
sense of acceleration or deceleration, no brightness shifts, no density
waves. This is the state the user expects when they launch Cosmostrix
without any atmosphere flags.

**Technical note:** All parameters sit at their configured values. The
atmosphere controller, if active, targets the calm regime. Speed
multiplier is 1.0. Density multiplier is 1.0. Brightness multiplier is
1.0. No visual parameter deviates from its explicit or default setting.

**Example:** Running `cosmostrix --scene classic` with no atmosphere
flags produces Rest. The `atmosphere-calm` preset also produces Rest
because it maps to `mode: disabled, regime: calm`, which means zero
atmosphere modulation.

### Pulse

**A temporary increase in intensity.** Speed rises, density may shift,
colors may brighten slightly. Then it returns to rest. Like a heartbeat —
a single, purposeful contraction and release.

**Visual description:** The rain briefly comes alive with more energy.
Columns accelerate, maybe by 10-20% above their base speed. Colors gain a
subtle brightness lift, as if a soft light passed behind the screen. Then,
over a breath cycle, everything eases back to the resting state. The user
should feel a gentle rhythm, not a jarring spike. A pulse should feel
inviting, like a warm breeze through the rain.

**Technical note:** The pulse regime modulates speed and brightness
multipliers within whisper-bounded ranges. The atmosphere controller
interpolates from the current state to the pulse target, holds briefly,
then interpolates back. The transition follows the pacing contract — no
instant jumps.

**Example:** The `atmosphere-pulse` preset demonstrates this regime.
Activate it with `--scene-custom atmosphere-pulse` and watch for periodic
waves of subtle intensity.

### Whisper

**The most subtle atmosphere effect.** Barely perceptible shifts in color
weight or cell brightness. Only noticeable if you watch carefully. Does
not change structure.

**Visual description:** Imagine the difference between a room lit by a
candle and the same room lit by the same candle a few minutes later, as
the flame subtly shifts. The rain is still the same rain. The columns fall
at the same speed. But there is an almost imperceptible warmth that comes
and goes, like the light itself is breathing. A user who glances at the
screen will not notice anything different. A user who watches for ten
seconds will begin to sense that something is alive.

**Technical note:** Whisper is not a regime — it is a modulation bound.
All non-calm atmosphere regimes operate within whisper-bounded ranges,
meaning their visual impact is capped at a level that preserves the rain
character. The `runtime_application: whisper` label in diagnostics
indicates this safety bound is active.

**Example:** Every controlled-live atmosphere preset uses whisper-bounded
modulation. Run `cosmostrix --scene-custom atmosphere-signal` and watch the
rain carefully for several seconds to perceive the whisper effect.

### Compression

**The visual field tightens.** Density may increase subtly. The rain feels
heavier, closer. Not faster, but denser — as if the space between columns
is slowly shrinking.

**Visual description:** The screen fills. Not with speed, but with
presence. Columns do not accelerate, but the gaps between them narrow.
The rain begins to feel more immersive, like stepping from an open field
into a narrow canyon where the rain is funneled and concentrated. The
experience should feel enveloping, not claustrophobic. There is still
rhythm and space — just less of it.

**Technical note:** The compression regime primarily modulates the density
multiplier upward while keeping the speed multiplier near 1.0. The
atmosphere controller applies the density increase gradually over at
least one breath cycle. Total density remains within whisper-bounded safe
ranges.

**Example:** The `atmosphere-compression` preset demonstrates this. Run
`--scene-custom atmosphere-compression` and notice how the rain gradually
fills more of the screen without speeding up.

### Void

**A deliberate reduction.** Rain thins, speed slows, colors dim toward
dark. The visual equivalent of holding breath. Empty space becomes visible.

**Visual description:** The rain retreats. Columns thin out, their
brightness dims, and the dark background begins to dominate. The effect is
not a crash or a failure — it is intentional emptiness. Like the pause
between movements in a symphony, where the silence is as expressive as the
sound. The user should feel a sense of spaciousness, of the screen
breathing out and leaving room for the background to exist on its own
terms.

**Technical note:** The void regime reduces density and brightness
multipliers while potentially lowering the speed multiplier. The
atmosphere controller interpolates downward gradually. The visual
runtime remains protected. The effect is bounded so that rain never fully
disappears — some columns always remain visible.

**Example:** The `atmosphere-void` preset demonstrates this. Run
`--scene-custom atmosphere-void` and watch the rain gradually thin and dim.

### Signal

**A patterned interruption.** Brief structured anomalies in the rain. Not
random glitch, but intentional marks. Like a message hidden in the stream.

**Visual description:** Within the steady rain, certain columns begin to
behave differently. They might converge toward a point, or flash in a
coordinated pattern, or develop a rhythmic emphasis that the surrounding
rain does not share. The effect is subtle but structured — a watcher
might think "that column is doing something different" even if they cannot
articulate what. Unlike random glitch, signal anomalies have direction and
purpose. They feel like communication, not noise.

**Technical note:** The signal regime introduces structured visual
patterns while remaining within whisper-bounded modulation ranges. The
atmosphere controller may modulate directional convergence, column
emphasis, or coordinated brightness patterns. The effect is the most
structurally intrusive of the non-storm regimes but remains bounded.

**Example:** The `atmosphere-signal` preset demonstrates this. Run
`--scene-custom atmosphere-signal` and watch for columns that develop
coordinated behavior distinct from the surrounding rain.

### Storm

**Full intensity.** Speed high, density high, glitch elevated. The visual
equivalent of shouting. Not the default state. Only used when explicitly
requested.

**Visual description:** Everything accelerates. Columns race down the
screen at high speed, the screen fills densely with characters, glitch
intensity rises to its highest level, and the overall impression is one
of overwhelming visual energy. Storm is the antithesis of Rest — where
Rest breathes evenly, Storm roars. The screen feels urgent, electric,
almost chaotic — but still structured. It is not a malfunction; it is a
deliberate maximum.

**Technical note:** Storm is the most intense regime in the atmosphere
system. It pushes speed, density, and glitch parameters to their highest
safe values. Storm is never default and must be explicitly requested.
The atmosphere system enforces strict boundaries even during storm —
parameters are capped at safe maximums that preserve terminal readability
and performance.

**Example:** The `--scene storm` curated preset demonstrates high-intensity
visual behavior. Note that this is a curated preset (color, charset, speed,
density, glitch) and is distinct from the atmosphere storm regime. The
atmosphere storm regime is not available through any preset or profile
and is explicitly blocked at every parsing layer.

### Breath Cycle

**The natural transition between states.** Never instant. Always gradual.
A breath cycle is the minimum transition period between two visual states.

**Visual description:** When Cosmostrix moves from one visual state to
another — say, from Rest to Pulse, or from Compression back to Rest —
the transition unfolds over a perceptible period rather than snapping
between states. Think of it as a slow exhale: the rain eases into its
new character rather than jumping. A user watching the screen during a
transition should perceive smooth, continuous change rather than a sudden
shift. The exact duration of a breath cycle depends on the specific
transition and the pacing parameters in effect, but the principle is
universal: no visual state change is instant.

**Technical note:** The atmosphere controller interpolates between the
current parameter state and the target parameter state. The interpolation
follows a pacing curve (currently linear or eased) over a minimum number
of frames. The breath cycle concept applies regardless of the specific
interpolation method — what matters is that the transition is gradual
and perceptible, not instantaneous.

**Example:** Switching from `--scene-custom atmosphere-pulse` to `--scene-custom
atmosphere-void` would trigger a transition from the pulse state through
rest to the void state, with each phase taking at least one breath cycle.

## Pacing Contract

The pacing contract defines the rules that all visual state changes must
follow. These rules are not suggestions — they are invariants that must
hold across all presets, scenes, atmosphere regimes, and configuration
combinations.

**No visual state change is instant.** All transitions between visual
states are gradual. This applies to speed changes, density changes,
brightness changes, color weight shifts, and any other visual parameter
that the user can perceive. When a parameter changes, it moves toward its
target value over a transition period, not in a single frame. The only
exceptions are parameters that are not visually perceptible (such as
internal bookkeeping counters or diagnostic fields).

**The minimum transition period is one breath cycle.** No visual state
transition may complete in less than one breath cycle. This ensures that
every change is perceptible as a transition rather than a flicker. The
breath cycle duration is a pacing constant that may be tuned in future
releases, but the principle that transitions have a minimum duration
does not change.

**Atmosphere effects never surprise the user.** They are opt-in or
clearly signaled. A user who launches Cosmostrix with no atmosphere
flags will never see atmospheric modulation. A user who activates an
atmosphere preset or profile does so explicitly through a named,
documented option. There is no hidden atmosphere activation path, no
secret intensity shift, no background visual effect that the user did
not request.

**The default state is always Rest.** No atmosphere effect activates
without user consent. When a user runs `cosmostrix` with no flags, they
get Rest: the baseline rain at its configured speed, density, and color.
This is not a special case — it is the default. Every other visual state
requires an explicit user action to reach.

**Storm is never default.** It requires explicit `--scene storm` or
an equivalent deliberate user action. Storm will never activate as a
side effect of another configuration, as a fallback, or as a "better
experience" recommendation. Storm is a conscious choice.

**Whisper is the safest atmosphere effect.** It changes perception
without changing structure. A whisper-bounded modulation might shift
brightness by a few percent or adjust speed by a fraction, but it never
alters the fundamental character of the rain. The columns still fall the
same way, the glyphs still cycle the same way, the scene still looks
like itself. Whisper is the guardrail that makes atmosphere modulation
safe for all presets and scenes.

**Compression and Void are intermediate.** They change density and speed
but preserve the rain character. A compressed screen still looks like
rain. A voided screen still has rain falling, just less of it. These
effects shift the intensity of the experience without changing its
nature.

**Signal is the most structurally intrusive.** It adds pattern to
randomness. Where other effects modulate parameters (speed, density,
brightness), Signal modulates structure — introducing coordinated behavior
that breaks the uniform randomness of the base rain. This makes Signal
the most noticeable and most "risky" atmosphere effect in terms of
visual identity, even though it remains whisper-bounded.

## Naming Conventions

These conventions govern how presets, scenes, and atmosphere effects are
named throughout Cosmostrix. Consistency in naming helps users form
correct expectations and helps developers maintain a coherent codebase.

**Preset names are single words, lowercase, evocative:** classic,
cinematic, calm, monolith, storm, cosmos, neon, hacker. Preset names
should suggest the visual character without overpromising. "cinematic"
suggests a particular mood; it does not promise 24fps film grain.
"storm" suggests intensity; it does not promise thunder sound effects.

**Scene names describe atmosphere:** monolith, matrix, signal. Scene
names identify the structural character of the rain. Each scene has a
distinct visual identity defined by its column structure, glyph behavior,
and pacing rhythm.

**Atmosphere preset names follow the pattern `atmosphere-<regime>`.**
The prefix makes it clear that the preset is an atmosphere configuration.
The regime name identifies the visual behavior: calm, pulse, signal,
compression, void, monolith-pressure. This naming pattern is consistent
across all six atmosphere presets and must be maintained for any future
atmosphere presets.

**Profile names are user-defined, no restriction.** Users may name their
profiles anything that the config parser accepts (letters, digits, hyphens,
underscores). Profile names are not part of the product vocabulary and do
not need to follow any convention.

**No preset or scene name may imply a promise the renderer cannot keep.**
A preset called "60fps-guaranteed" would violate this rule because
Cosmostrix does not guarantee frame rates. A scene called "photorealistic"
would violate this rule because the renderer produces terminal character
rain, not photorealistic images. Names must be honest about what the
software can deliver.

**No name may reference a specific FPS target or hardware capability.**
Names like "120hz" or "gpu-accelerated" are forbidden because they
imply performance characteristics that depend on the user's hardware and
terminal. A name must be meaningful regardless of where Cosmostrix runs.

## State Hierarchy

Visual state in Cosmostrix resolves through a layered system. Higher layers
override lower layers. The complete hierarchy, from lowest to highest
priority, is:

1. **Built-in defaults** — The values compiled into the binary via clap
   default values. These are the fallback when no other layer provides a
   value.
2. **Config file values** — Global settings from the user's config file
   (`~/.config/cosmostrix/config.toml`).
3. **Config preset** — A preset applied via the config file's `preset`
   key.
4. **Config scene** — A scene applied via the config file's `scene` key.
5. **Config profile** — A user-defined profile applied via the config
   file's `profile` key.
6. **CLI preset** — A preset applied via `--scene <name>` on the
   command line.
7. **CLI scene** — A scene applied via `--scene <name>` on the command
   line.
8. **CLI profile** — A user-defined profile applied via `--scene-custom
   <name>` on the command line.
9. **Low-power values** — When `--scene low-power` is active, a separate set
   of conservative values is applied for fields not already set by a
   higher layer.
10. **Explicit CLI flags** — Individual flags like `--speed 20` or
    `--color purple` always win over every other layer.

Higher layers override lower. No layer may break the pacing contract. Even
when a higher layer sets an aggressive value (such as `--scene storm` with
its high speed and density), the pacing contract still applies: transitions
are gradual, storm is never default, and the atmosphere controller respects
breath cycle minimums.

The key insight of this hierarchy is that user intent flows upward. The
further up the stack a value is set, the more deliberate and explicit the
user's intent. An explicit CLI flag is the most deliberate action a user
can take. A config file setting is a persistent preference. A built-in
default is a sensible starting point. The hierarchy respects this gradient
of intent.

## Anti-patterns

Cinematic breathing is defined as much by what it is not as by what it
is. These anti-patterns describe visual behaviors that would violate the
cinematic breathing contract. Any visual change that matches an
anti-pattern is a bug, not a feature.

**NOT random visual flicker.** If the rain brightness shifts unpredictably
frame-to-frame with no rhythm or pattern, that is flicker, not breathing.
Breathing has direction and intention. Flicker is noise. Cosmostrix must
never produce visual output that a user would describe as "flickery"
unless they have explicitly configured glitch intensity to a high level.

**NOT unannounced mode switches.** If the rain suddenly changes character
without any user action or visible transition, that is an unannounced mode
switch. Every visual state change must either be triggered by an explicit
user action or be part of a documented, predictable atmosphere pattern.
The user should never be surprised by a visual change they did not request.

**NOT gradual slowdown that looks like performance degradation.** If the
rain slows down over time because the system is under load, that is
performance degradation, not a Void effect. Cinematic breathing changes
are intentional and bounded. Performance issues are bugs. A user must be
able to distinguish between "the rain is slowing because I chose void"
and "the rain is slowing because my terminal is struggling."

**NOT color changes that look like terminal errors.** If a color shift
resembles a terminal error state (inverted colors, sudden red flashes,
corrupt-looking palettes), the cinematic breathing contract is violated.
Color modulation must be subtle, bounded, and aesthetically intentional.
It must never look like something went wrong.

**NOT density drops that look like renderer bugs.** If the rain thins
out in a patchy, uneven way that looks like columns are failing to
render, that is a bug, not a Void effect. Void reduces density uniformly
and gradually. A user watching a Void transition should think "the rain
is thinning," not "something is broken."

**NOT any visual change the user did not request or cannot understand.**
This is the overarching anti-pattern. If a visual change occurs and the
user has no way to know why it happened, no way to stop it, and no
documentation that explains it, then the cinematic breathing contract
has failed. Every visible change must be traceable to a user action, a
documented atmosphere effect, or a clearly communicated system behavior.

## Future Direction

This vocabulary will expand as Cosmostrix Live and future renderer
experiments mature. New breathing terms may be needed to describe visual
effects that do not fit neatly into the current vocabulary — for example,
if a future version introduces spatial effects (rain responding to cursor
position), temporal effects (rain that accelerates based on time of day),
or interactive effects (rain that responds to system load or notifications).

When new terms are needed, they must be added to this document before
implementation. The process is: propose the term with a clear definition,
visual description, and technical note; add it to the Breathing Vocabulary
section; update the Pacing Contract if the new term implies new rules;
add static tests to `src/docs_tests/v5_nightfall.rs` to guard the new
content; and only then implement the behavior.

This ensures that the cinematic breathing language remains a shared,
documented contract rather than an ad hoc collection of feature names. The
vocabulary grows deliberately, not accidentally.