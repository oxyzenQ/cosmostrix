<!-- SPDX-License-Identifier: GPL-3.0-only -->

# cosmostrix Insights — Living Idea Journal

> This file documents the moments when cosmostrix's features were born —
> not from issue trackers or user requests, but from the owner's lived
> experience with the renderer running in the background of daily life.
> Each entry records **when**, **where**, and **what sparked** the idea.
> Source code is truth; this file is the story behind the truth.

---

## Why This File Exists

Most features in most projects come from external pressure: bug reports,
user requests, roadmap planning. cosmostrix's most distinctive features
came from a different source — **the owner watching the rain fall and
noticing what was missing**.

This is not romanticism. It is a documented pattern:

1. The renderer runs in the background of daily life (laptop open, rain
   falling, owner doing other things).
2. The brain's Default Mode Network (DMN) — the network active when not
   focused on a specific task — processes the visual in the background.
3. An insight "pops" in a relaxed moment: wifi offline, just woken up,
   drinking coffee.
4. The insight is noted immediately (this file) before it fades.

This file exists to **preserve the pattern** — so future contributors
understand that cosmostrix's feature pipeline is not a backlog to burn
down, but a living observation of what a terminal rain renderer can
become when you let it breathe.

---

## Entry Format

Each entry follows the same structure:

- **Date**: when the insight occurred
- **Context**: what the owner was doing (the "relaxed moment")
- **Observation**: what the owner noticed
- **Feature**: what it became in the codebase
- **Status**: implemented / researching / future / rejected

---

## Insight 1 — Border-Touch Glow (the "wifi offline" moment)

**Date**: 2026-08-26
**Context**: Wifi went down. The owner was listening to music with
cosmostrix running fullscreen on the laptop. No coding possible — the
brain relaxed. Eyes drifted to the rain. The message border (`-mb`) was
active. Rain droplets were hitting the top border... but nothing
happened. No glow. No signal. The border just sat there while the rain
passed through it invisibly.

**Observation**: *"Why doesn't the border glow when rain touches it?"*
The rain head is bright (palette last-stop, usually white). The border
is a hard line. The contrast is there — but the renderer structurally
prevents any visual contact (the overlay draws last, occluding the rain
cell at the border). The "touch event" doesn't exist; it has to be
synthesized.

**Feature**: `docs/research/RAIN_BORDER_TOUCH_GLOW_AUDIT.md` — 5-option
masterclass audit (A: Overflow Bleed, B: Border-Head Bloom, C: Edge-Touch
Pulse, D: Glow Halo Above Border, E: Splash Spark). The audit
established the trigger condition (droplet head transitions from
`< start_line` to `== start_line`) + the 4 LTS invariants that any
touch-glow design must respect (top corners stay dark, triangle wave on
edges, no lone bright heads).

**Status**: Audit complete. Options A–E documented. The `BorderPulse`
mechanism (Option C variant) was implemented in the border gradient
system (beta.4). See `detect_border_touch` in `cloud/rain.rs`.

---

## Insight 2 — Particle Spark (the "just woken up" moment)

**Date**: 2026-08-27
**Context**: The owner had just woken up. Still in bed, still groggy.
The laptop was on, cosmostrix still running from the night before — rain
falling on the screen, message border active. Eyes half-open, brain not
yet in "problem-solving mode". The rain was hitting the border. The
border pulse (from Insight 1) was there... but it was just a glow. A
flat single-cell brightness. It didn't *feel* like rain hitting
something. It felt like a light turning on.

**Observation**: *"Where's the splash? Where's the spark?"* Real rain
hitting a surface doesn't just glow — it *splashes*. Particles fly
upward and outward. The mouse-click quantum ripple (20 particles, 4s
lifetime) was too big. The border pulse was too small. What was missing
was the **middle ground**: a small particle burst, sized for a single
rain drop, not a click.

**Feature**: `docs/research/RAIN_BORDER_TOUCH_SPARK_RESEARCH.md` —
Option F (Particle Spark) with 3 sub-variants:
- F1 Micro-Spark: 3 particles, 250ms, no trail ("tic")
- F2 Splash Crown: 6 particles, 350ms, 1-cell trail ("plash") — **owner-selected**
- F3 Spark + Ring: 6 particles + mini flash ring ("kapow")

Implemented as `spawn_border_spark` in `cloud/spawn.rs` (F2 variant:
6 particles, upward semicircle fan, `·` glyph, `head_rgb` color, 1-cell
trail, 350ms lifetime). Shares the `QuantumParticle` pool — zero new
allocation. Corner-skip guard preserves LTS invariants.

**Status**: Implemented (commit `ae995a4`). Owner-rated 9/10.

---

## Insight 3 — The "living project" realization

**Date**: 2026-08-27
**Context**: The owner was reflecting on how Insights 1 and 2 emerged
—not from a roadmap, not from user requests, but from *living with the
renderer*. The wifi-offline moment. The just-woken-up moment. The
realization: cosmostrix is not a backlog to burn down. It is a living
observation of what a terminal rain renderer can become.

**Observation**: *"The best features come from watching the rain, not
from planning the rain."* The pattern: (1) run cosmostrix in the
background of daily life, (2) let the brain process it in relaxed
moments, (3) note insights immediately, (4) implement when ready. This
is not a methodology to enforce — it is a rhythm to protect.

**Feature**: This file (`INSIGHTS.md`). A living journal that records
the *when* and *where* of each insight, so the pattern is visible to
future contributors and the owner can recall the rhythm.

**Status**: This file is the implementation. Future insights will be
appended below.

---

## Insight 4 — HUD Sci-Fi Dashboard (the "pre-flow-focus" moment)

**Date**: 2026-09-02 (insight the night before; noted ~12:50 before
entering the day's coding flow)

**Context**: Morning session. The owner had just returned to the laptop
after breakfast, about to enter flow-focus, when the idea surfaced —
recorded immediately (this entry) and deliberately deferred so the
stable-release line keeps priority.

**Observation**: *"The HUD could sit bottom-center as an elegant
triangle — FPS indicator on top, the other metrics below — dressed like
a sci-fi space dashboard: rounded corners, separate panels, that
aesthetic."* Owner posture: leaning both ways ("agak tolak dan agak mau
test implement") — research first, nothing in the backlog.

**Feature**: `docs/research/HUD_LAYOUT_MASTERCLASS_RESEARCH.md` —
4-option masterclass audit (A: Apex Pendant — the literal vision;
B: Sci-Fi Panel Grid; C: Bottom Console Bar; D: Style-Only Evolution
at the current corner) + 5 documented rejected directions (GPU/kitty
protocol, app-level transparency, true full-triangle, position toggle,
scanline layer) + cross-cutting finishers. Key findings: the glyph
vocabulary (`╭╮╰╯`, `╱╲`, `▲▼`) is fully gate-clean under the
symbol-only rule (U+2500..U+25FF allowed); rounded corners are already
house style (message border + HUD `╯` corner); the hard cost is the
bottom-center placement itself (message-box collision geometry,
dynamic-width jitter, test churn) — not the styling.

**Status**: Research complete, awaiting owner decision (decision menu
in the research doc §11). No code changed. Priority LOW — after the
stable release.

---

## Future Insights

When a new insight arrives — in a relaxed moment, from watching the
rain — append it here using the entry format above. Do not edit past
entries (they are historical record). Do not plan insights (they
arrive on their own schedule). Just note them honestly.

The pattern to protect:
1. **Run cosmostrix** — let it live in the background.
2. **Do not force insights** — let the DMN work.
3. **Note immediately** — before the moment fades.
4. **Implement when ready** — no deadline, no pressure.

---

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
