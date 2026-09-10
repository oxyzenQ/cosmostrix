<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-hunter-26 — resize must not restart the message reveal: the mfs "half little reload" (all styles with msg mode active)

Owner directive (2026-09-10): "owner found inconsistency on all mfs
styles when enable msg mode on cosmostrix, because owner see
during/when resize the terminal screen the mfs style like want
reload but just half little so that need shaper eyes to detect it.
owner want don't restart during resize the screen. exclude
shortkey 'r'"

## Root cause

The resize path (`handle_resize` -> `Cloud::reset` ->
`reset_with_bounds`) re-centers the message overlay for the new
dimensions via `reset_message()`. That function conflated two
concerns:

1. the geometry rebuild (message cell grid, border order, word
   ordinals, touch geometry — legitimately resize-dependent), and
2. the fresh-reveal resets (engrave spark pool wipe, scorch smoke
   wipe, border-pulse wipe, movement-detector re-arm) — which belong
   only on paths where the reveal timeline itself restarts.

The reveal timeline (`message_start_time`) was never touched by the
resize — the text kept revealing seamlessly — but the stateful
layer around it was torn down and re-armed. Two visible artifacts
made up the owner's "half little reload":

- Every in-flight engrave spark and scorch smoke puff vanished
  mid-flight (pool wiped), and every active border-touch pulse
  vanished.
- On the next frame the movement detector compared the CURRENT
  reveal head against its re-armed sentinel (`last_head == MAX`)
  and fired a spurious spark burst / smoke puff at a char that had
  been revealed long before — a phantom "re-engraving" flash.

The same artifacts fired on the 'm'/'mb' border toggle (same
`reset_message` call), which is also a layout change, not a new
reveal.

## The fix (surgical split, reveal-restart vs geometry)

- `reset_message()` now delegates its body to a new
  `relayout_message()` and layers the fresh-reveal resets (pulse
  wipe + sidecar resets) on top. Callers where the reveal timeline
  genuinely restarts are unchanged: `set_message`,
  `set_msg_fill_style`, `restart_message_typewriter` (the 'r'
  relaunch — owner-excluded from this hunt).
- The resize path (`reset_with_bounds`) and the border toggle
  (`set_message_border`) call `relayout_message()` only: the
  timeline, the sparks, the smoke, and the pulses all continue.
  A resize is an interrupt, not a replay — the same philosophy as
  the Phase D drift-state contract (Bugs #8/#9).
- The engrave/scorch movement detectors now fire on FORWARD head
  movement only (`last_head == MAX` sentinel still bursts on the
  first char of a fresh reveal). A height-truncating resize can
  clamp the reveal budget and move the head index BACKWARD — that
  is not a newly engraved char, and bursting there was the other
  half of the phantom flash.
- The border-pulse draw pass bounds-checks `msg_idx` against the
  rebuilt grid (pulses now survive layout rebuilds, so a stale
  index must be dropped, not panic).

## Proof

Nine regression tests
(`test/engine/cosmic_dragon_engine/cloud/tests/tests_msg_resize_hunter26.rs`):

- resize keeps engrave sparks + detector state; unchanged head
  fires no burst after resize
- resize keeps scorch smoke + detector state
- resize never touches the reveal timeline anchor
- backward head jump (height-truncating resize) does not burst
- border pulses survive resize with bounds safety
- border toggle keeps sidecars + timeline (same class of fix)
- `set_message`, `set_msg_fill_style`, and the 'r' restart
  (`restart_from_zero` + `restart_message_typewriter`) still fully
  re-arm the reveal — the owner-excluded paths are pinned to stay
  restarts

Full suite: 2712 passed, 0 failed (2703 pre-change + 9 new). The
nine stateless styles were already continuous by construction
(their reveal math is a pure function of elapsed time and content
index); with the sidecar fix, all eleven styles now honor the
owner's resize contract.
<!-- COSMOSTRIX-DISCLAIMER -->
