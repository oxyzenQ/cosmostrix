<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->

# Night Frontier Z5b — INV-22: the afterglow's temporal colorimetry law

Owner: oxyzenQ | Session: night-frontier-Z5 | Date: 2026-11-19
Reference: github.com/oxyzenQ/zelynic (mature infra lineage)

## The gap

Z4a (INV-20/21) pinned the SPATIAL colorimetry — the palette
interpolation paths. Z4c (Lapis 4c) pinned the phosphor decay's SHAPE —
the P1-class curve, the half-lives. Nothing pinned the TEMPORAL
colorimetry: what the decay actually does to emitted color, frame over
frame, in perceptual units. The research charter proposed a "temporal
ΔE JND budget on the decay path"; the measurement BEFORE pinning
(the session's own discipline: measure first, pin what is true) found
that the charter's pretty framing was wrong in three specific ways,
and built the honest law from the measured truth instead.

## What the measurement found (the charter's corrections)

1. **The decay is DESIGNED supra-JND.** The trail is a visible fast
   melt, not an invisible drift: the worst per-step ΔE-OK at the
   nominal envelope runs 0.03 (front layer at 120 fps) to 0.37 (back
   layer at 24 fps), and under the stacked self-healer boosts plus the
   bottom band the emergency collapse reaches ~0.74 per frame. A
   JND-capped decay law would freeze the aesthetic — rejected. The
   honest form: pin the melt ENVELOPE (the designed quantities) in
   both directions, so a retune is a conscious, reviewed change.
2. **Chroma-magnitude shrinkage is the physics of dimming**, not a
   defect. A first attempt pinned the chroma-plane displacement per
   step; the direct sweep showed it riding the melt (0.03 nominal,
   0.12 at the 24 fps corner) — proportional to the intended
   luminance step, meaningless as a separate JND claim. The
   perceptually meaningful axis is HUE (the "which color" axis), and
   that is what the law pins.
3. **The death is a designed pop.** The last alive energy (7) renders
   a ghost ~0.13 ΔE-OK from the black background before the cell
   blanks at `PHOSPHOR_DEAD_THRESHOLD` = 6 — six times the mid-tone
   JND, for exactly one frame. Dying at energy 1 would extend a
   near-invisible tail for the sake of one imperceptible frame. The
   pop is now a pinned designed quantity, not an accident nobody
   measured.

## The structure the law sits on

The per-cell decay is a u8 ENERGY ladder —
`floor(energy * exp(-rate * dt))` per frame, death at `<= 6` — and the
rendered color is the pristine base color re-scaled by `energy / 255`
EVERY frame (`phosphor_ghost_brightness` -> `apply_brightness_rgb`),
never an iterated color rounding. The colorimetry of the trail is
therefore the colorimetry of the energy ladder plus one fixed-point
scale per frame. The rate product is
`PHOSPHOR_DECAY_RATE (8.0, the calibrated P1 curve at 5x dilation) *
layer_mult * bottom_mult * boost * adaptive * tuning`; the law pins
the corners of that product at the reference tuning and documents how
the envelope scales.

## INV-22 — the six pins (all measured, `lock_inv22_z5b.rs`)

1. **Monotone + terminating ladder** — every emitted channel
   non-increasing frame over frame; every path terminates (< 600
   frames). Full stride-8 cube x 3 layers x the 24-120 fps ladder.
2. **Nominal melt envelope** — the twelve-entry (layer, fps) table of
   worst per-step ΔE-OK, asserted in BOTH directions (+10 percent
   band): a retune that exceeds a ceiling fails, and so does one that
   collapses the designed melt.
3. **Load corners** — the stacked-boost corner (1.2 x 1.5) and the
   bottom-band emergency stack (x 1.8 on top) pinned at their measured
   worsts (0.18-0.82 across the fps ladder): the survival trade is
   bounded and known.
4. **Death pop** — ΔE(ghost at energy 7, black) <= 0.135.
5. **Floor pop** — any step that vanishes the emitted color to black
   is bounded at 0.14 (measured global worst 0.1286 at the
   bottom+boost corner; 0.067 at the nominal corners — the
   (1,1,1) -> black last-quantum step, Oklab's near-black foot making
   1/255 register large).
6. **Hue stability** — through the visibly colored regime (chroma >
   0.06): per-step hue drift <= 10 degrees (measured worst 9.0 at
   24 fps), net visible-life shift <= 21 degrees (measured worst
   20.2), concentrated at the deep-blue u8 foot — the medium's
   documented property, now a measured bound. Bright-chroma paths
   drift far less.

Plus the LUT contract ride-along: `phosphor_decay_lut` must equal the
raw exponential law layer by layer, so the temporal pins and the Z4c
calibration can never drift apart.

## Honest scope notes

- The pins live at the reference tuning (decay_mult 1.0);
  `set_phosphor_tuning`'s terminal-aware multiplier scales the rate
  and therefore the envelope — the shape of the law is invariant, and
  the per-terminal tuning has its own sweep tests
  (`tests_phosphor_sweep.rs`, `tests_phosphor_adaptive.rs`).
- The 1 fps legal config floor is the deliberate slow-cadence regime
  (one frame = one second of decay, by request); the law's envelope is
  the interactive 24-120 fps ladder the scenes and ambient scheduler
  actually run.
- The pins are tests-only: the shipped binary is byte-for-byte
  unchanged (the release build never compiles `#[cfg(test)]` code), so
  no A/B campaign is required by the benchmark law — the A/B the
  charter imagined would compare identical binaries.

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
