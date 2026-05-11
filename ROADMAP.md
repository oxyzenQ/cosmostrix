# Cosmostrix Roadmap — Perceived Quality Engineering

> **Core Principle:** Cosmostrix has passed the "fast enough" threshold.
> The differentiator now is not raw FPS — it is **how it feels while running**.
> This is the territory of pacing, smoothness, polish, and cinematic engineering.

---

## Evolution Summary

| Arc | Focus | Status |
|-----|-------|--------|
| **1 — Hardening** | Survival under abuse | Done |
| **2 — Stabilization** | Perpetual smoothness | Done |
| **3 — Performance Maturity** | Elite-tier efficiency | Done |
| **4 — Production Polish** | Completeness feel | Done (CLI, help, defaults) |
| **5 — Render Smoothness** | Perceived quality | In Progress |
| **6 — Runtime Hardening** | Endurance under load | Planned |
| **7 — Cinematic Polish** | Premium animation feel | Planned |

---

## ARC 5 — RENDER SMOOTHNESS (Phase A)

> Humans don't see FPS. Humans see consistency.
> 60 FPS with high jitter feels cheap. Stable frametime feels premium smooth.

### 5.1 Frame Pacing Consistency

- **Spin-sleep hybrid pacing**: Replace naive `poll_event(timeout)` sleep with
  a spin-sleep approach — busy-wait the last ~0.5ms for sub-millisecond
  accuracy, then yield for the remainder. Eliminates OS scheduling jitter.
- **Sleep drift correction**: Track cumulative drift between target and actual
  frame times. Distribute corrections across frames to avoid single-frame jumps.
- **Pacing accumulator**: Use a phase-accumulator model where the target cadence
  is an ideal clock, and the renderer chases it. Prevents phase creep.
- **Oversleep compensation**: When OS sleep overshoots, absorb the error into
  the next frame's budget rather than shifting the schedule.

### 5.2 Delta-Time Normalization

- **Smooth delta-time**: Apply exponential moving average to raw delta-time
  before feeding simulation. Reduces per-frame jitter propagation into physics.
- **Clamped simulation step**: Cap per-frame simulation advance to prevent
  teleportation on frame spikes (already partially implemented via `max_sim_delta`).
- **Sub-frame interpolation for animation subsystems**: Long-timescale systems
  (color ecosystems, atmospheric evolution) should interpolate rather than step.

### 5.3 Monotonic Timing Hardening

- **Single `Instant` per frame**: Capture time once at loop top, derive all
  timing from it. Eliminates drift between multiple `Instant::now()` calls.
- **Consistent timing model**: Ensure every subsystem (spawn, advance, decay,
  glitch, atmospheric) derives from the same frame timestamp.

---

## ARC 6 — RUNTIME HARDENING (Phase B)

> A premium renderer must run for hours without degradation.

### 6.1 Resize Stability

- **Zero-flash resize**: Eliminate full redraw flash on resize. Transition the
  existing frame into the new dimensions — crop or extend — then resume
  differential rendering without a full clear.
- **Resize coalescing**: Already partially implemented. Ensure rapid resize
  storms are collapsed into a single transition.
- **Dimension-preserving state migration**: Cloud state (droplets, entropy)
  should survive resize without full reset where possible.

### 6.2 Adaptive Throttling

- **Idle detection**: Reduce update pressure when user is away (no input for N
  seconds). Lower FPS, reduce atmospheric subsystem tick rates.
- **Active restoration**: Instantly restore full quality on user interaction.
- **Power-aware scheduling**: Integrate with `--low-power` philosophy at
  runtime — detect battery/thermal pressure and adapt dynamically.

### 6.3 Long-Endurance Stability

- **Allocation audit**: Zero steady-state allocations. All rendering should be
  reuse-based after initial setup.
- **Terminal state drift detection**: Periodic ANSI state validation to catch
  desync before it becomes visible.
- **Timing drift guard**: Detect and correct cumulative timing drift over
  multi-hour sessions.
- **Memory budget enforcement**: Hard cap on transient allocations per frame.

### 6.4 Terminal Compatibility

- **Graceful degradation matrix**: kitty, wezterm, alacritty, ghostty, foot,
  gnome-terminal, tmux, SSH, low refresh rate terminals.
- **Feature capability detection**: Probe terminal capabilities at startup and
  disable features the terminal cannot support.
- **ANSI safety net**: Detect broken ANSI state and force a full redraw.

---

## ARC 7 — CINEMATIC POLISH (Phase C)

> The psychological gap between "technically correct" and "premium smooth".

### 7.1 Animation Refinement

- **Velocity smoothing**: Apply easing curves to droplet acceleration instead
  of linear gravity. Feels more organic.
- **Stream death refinement**: Fade-out instead of abrupt stop. Droplets
  should decelerate gracefully at end-of-life.
- **Color propagation smoothing**: Interpolate color transitions across
  temporal color ecosystem ticks rather than stepping.
- **Rain acceleration easing**: Gradual ramp-up on start, not instant full speed.

### 7.2 Temporal Coherence

- **Frame-blend for low FPS**: At low FPS targets (<=30), blend consecutive
  frames for perceived smoothness rather than stepping discretely.
- **Transition interpolation**: All cross-fades (profile changes, color scheme
  cycles, density shifts) should use smoothstep or eased interpolation.

### 7.3 Deterministic Timing

- **Replay consistency**: Same seed + same config should produce identical
  output frame-for-frame (within floating-point tolerance).
- **Stable pacing cadence**: Frame timing should be deterministic enough that
  visual rhythm is predictable and rhythmic, not stochastic.

---

## Explicitly Deferred

The following are intentionally NOT in scope:

- Plugin system / Lua scripting
- Networking / multi-user
- GUI mode
- Config ecosystem / theme marketplace
- WASM / browser targets

These can dilute renderer excellence. The focus is on being the best
terminal renderer — not a platform.
