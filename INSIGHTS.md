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

## Insight 4 — The "two clocks" harmony moment

**Date**: 2026-09-02
**Context**: The owner was combining crystal-dragon with ambient
snapback and noticed the two systems share ONE timeline: a drift
becomes visible for exactly `ambient-snapback-secs` before the
ambient phase re-asserts, and the next drift can only fire on a poll
boundary. With the poll interval fixed at 60s, the only tuning
freedom was the snapback side — half a conversation.

**Observation**: *"When two systems cooperate on one timeline, both
sides of the timing need to be tunable — otherwise the user is
negotiating with only half a voice."* The moment the poll interval
became a knob (`crystal-dragon-secs`, range 0.0..=86400.0 — the same
range contract as `ambient-snapback-secs`), the harmony guidance
stopped being a fixed rule ("keep snapback < 60") and became a
relationship ("keep snapback < polling") the user can shape online
while watching the rain.

**Feature**: `--crystal-dragon-secs` / `crystal-dragon-secs` (CLI,
config, live-reload — v80.0.0-alpha.1). The drift-cycle self-reset
follows the configured cadence; verbose, `--doctor`, `--testconf`,
the template config, and the post-exit final state all disclose the
effective value. See `docs/CRYSTAL_DRAGON_ENGINE.md` §3
"`crystal-dragon-secs` — the harmony knob".

**Status**: Implemented (v80.0.0-alpha.1; dwell floor amended by
S-master-HUNT-3 — see Insight 5). The 60s minimum-dwell anti-flicker
floor originally stayed constant on purpose; the owner later showed
that pinning made the knob a no-op below 60s, so the floor is now
min(60s, cadence) — anti-flicker for the untuned case, obedience for
the tuned one.

---

## Insight 5 — The knob that wasn't (glyph vs. position; floor vs. lock)

**Date**: 2026-09-03
**Context**: The owner tuned `--crystal-dragon-secs 6`, live-enabled
the dragon, and watched the palette change ~60s later — the exact
default the knob was supposed to replace. The same session exposed a
message box that dropped its dash ("v80.0.0 alpha.1") and a
`--no-effects` flag that silently died on the first config edit.

**Observation**: *"A knob whose floor outranks it is not a knob."*
Three of the four bugs were the same lesson in different clothes:
state classified by the WRONG coordinate. The dash was swallowed
because border membership was decided by GLYPH (what the character
looks like) instead of POSITION (where the layout put it); the
6s cadence was pinned because a safety floor behaved like a LOCK
instead of yielding to explicit user intent; the effects flag died
because only the startup path applied it (temporal, not structural).
Classification — of cells, of precedence, of ownership — is the
actual design decision; everything downstream is arithmetic.

**Feature**: `MsgChr.is_border` (positional border classification,
v80.0.0-alpha.1 S-master-HUNT-3), `min_dwell_secs =
min(60, polling_secs)` (the floor yields), and
`create_cloud`-owned `effects_enabled` (one construction site, every
path). Locked by 12 tests + a PTY replay harness.

**Status**: Implemented (v80.0.0-alpha.1, S-master-HUNT-3). Verified
live: the 6s cadence drifts at ~6s, the dash renders verbatim, a
stressed non-verbose exit stays silent.

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
