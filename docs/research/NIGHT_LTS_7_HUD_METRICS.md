<!-- SPDX-License-Identifier: GPL-3.0-only -->

# NIGHT-lts-7 — the HUD metrics depth audit: realtime, accurate, zero measurable overhead, stable — one e2e-harness robustness fix

Owner directive (2026-09-10): "depth audit all functions HUD metrics,
for peak optimize, realtime, accurate but not overhead, stable, lts
already"

## Audit surface

- `src/interactive/hud/` (mod.rs 975 LOC, metrics.rs, hud_init.rs,
  colors.rs) — HudState, the 1 Hz metric tick, samplers, rendering
- `src/interactive/event_loop_hud.rs` — the per-frame state push
- `src/sysstat/` (cpustat, memstat, envstat, procstat, usagestat)
- `src/interactive/activity.rs` — FrameTimeTracker (fps/p99 source)
- e2e: `scripts/hud_order_e2e.py`, `scripts/hud_long_scene_e2e.py`

## Realtime — verified

1 Hz metric tick (HUD_METRIC_INTERVAL = HUD_RSS_INTERVAL =
HUD_CPU_INTERVAL = 1 s, deliberately ALIGNED so all three fire on
the same tick — half the timestamp comparisons on the fast path).
The 1 Hz rate is the documented htop/mangoHUD/nvidia-smi convention:
faster rates flicker without diagnostic value. Palette-driven colors
refresh EVERY frame (separate from the metric tick), so a runtime
color change appears on the next frame, not up to 1 s later. The
`tgt:` line disambiguates throughput from the cap
(active/idle/paused/drain mode suffix) — the documented fix for the
"--fps 30 but HUD shows 11k fps" confusion.

## Accurate — verified, contract by contract

- cpu%: two-sample delta math (cpu_ns delta / wall_ns delta), the
  wall delta computed before the baseline swap, division-by-zero
  guarded, display-clamped, baseline kept WARM while the HUD is off
  (toggle-on shows an instant percent instead of 1 s of "—"), and
  pause-aware: the display freezes but the baseline keeps ticking so
  the first post-resume delta stays a ~1 s window instead of the
  whole idle span. Unsupported sampler -> honest "—" (never a fake
  0.00%).
- rss: /proc/self/status VmRSS through the shared procfs helper;
  None -> "—". getrusage rejected by design (ru_maxrss is a lifetime
  peak, not a window sample).
- fps: deliberately render-work throughput (1000/rolling-avg-ms over
  the fixed 60-frame ring) — documented as such, with tgt: carrying
  the cap. p99: stack-anchored 60-element sort (~300 ns, zero heap).
- dcel/tcel: 60-frame rolling average over the DirtyCellTracker ring
  plus latest total; paused frames do not push (matches the
  FrameTimeTracker pause-freeze contract).
- uptime: pause-excluded with sub-second precision (accumulated
  paused_total + the open segment), tiered compound format with
  calendar-fixed units (1mo = 30d, 1y = 365d) so every boundary is
  deterministic.
- Pause freeze (owner bug fix 2026-08-30): every metric holds its
  last active value while paused; set_metrics_paused runs BEFORE the
  metric setters on the pause frame and AFTER on the resume frame.

## Overhead — measured at zero

A/B (pro binary, 200x56 PTY, 60 FPS, continuous drain, 25 s steady
window): HUD off 4.76% vs HUD on 4.47% — the delta (-0.28 pp) is
inside run-to-run noise. Structural reasons it stays there: one
/proc read per second (~2 KiB, documented "well under 0.1% CPU"),
string setters use clear+push_str on warmed allocations, the metric
tick is a single timestamp early-exit per frame, mode suffixes are
&'static str (no per-tick allocation), and write_to_frame rides the
differential renderer (Frame::set short-circuits on content
equality; HB-01 pad-to-max covers shrink clear).

## Stable / LTS — verified by e2e

hud_order_e2e.py: PASS — 25/25 labels, screen-reconstructed row
order exact (fps/tgt/max/p99/cpu/rss/ehs/prs/scn/chr/clr/sped/dsty/
prdr/crdr/ambt/glth/ctun/mnst/rain/dcel/tcel/cid/up/size). The
screen-reconstruction methodology (not raw-stream order) is itself
the documented HUNT-20 fix. hud_long_scene_e2e.py: PASS — the
27-char scene name renders in full with the border past the text
(HUNT-20's 64-col width budget).

## The one finding (fixed)

hud_long_scene_e2e.py hardcoded BIN = target/pro/cosmostrix. On a
release-only machine the child exec'd a nonexistent path, died
instantly, and the empty reconstructed screen read as a RENDERER
defect ("scn row does not carry the full scene name") — a false
negative that cost real audit time this session. Fixed: BIN env
override, then pro, then release fallback, with a clear fatal
message when no binary exists. Both e2e scripts verified green
after the fix.

(Operational note for future harnesses, learned while measuring:
an undrained PTY blocks the child's frame writes, the event loop
stalls, and keypresses are never processed — always drain from t=0,
not just after the interaction.)

## Verdict

The HUD metrics subsystem is already at peak — zero changes to the
metric code paths. The e2e BIN-resolution fix is the product, plus
this report.
<!-- COSMOSTRIX-DISCLAIMER -->
