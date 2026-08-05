<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Live HUD Overlay

The HUD is cosmostrix's live performance overlay for interactive mode.
Toggle it with the `i` key; move it between corners with `h`. Zero cost
when off (all methods short-circuit on `visible == false`). Metrics
recompute at 1 Hz (matches htop, mangoHUD, Steam FPS counter, and
`nvidia-smi` — faster rates cause number flicker without improving
diagnostic value).

This document is the canonical reference for what each HUD line means,
why it can disagree with `--benchmark` numbers, and how to use it to
diagnose common issues.

---

## HUD Lines (top-to-bottom)

The HUD writes 8 lines into the frame buffer at the chosen corner
(left default, or right after pressing `h`). Each line is one metric.

### 1. ` fps: <N>`

**Render-work throughput** = `1000 / rolling_avg_ms`.

This is the inverse of the average wall-clock time the renderer spent
producing one frame (work_ms = the time between `cloud.rain_at()` start
and `term.draw()` finish). It is NOT the loop's frame-rate cap, and it
is NOT the same as the `avg_fps` field in `--benchmark` output.

**Why this number is often huge (e.g. `fps: 11000` when you set
`--fps 30`):** the loop sleeps between frames to maintain the `--fps`
cap. The sleep time is NOT part of `work_ms`. So if your `--fps 30`
cap means each frame takes 33.3ms total but only 0.1ms of that is
actual render work, `fps:` will show `1000 / 0.1 = 10000`. The other
33.2ms was the loop sleeping to maintain the cap. This is correct
behavior, not a bug.

**To see the actual cap**, look at the `tgt:` line below.

**Format rules:**
- `fps >= 10000` → humanized (e.g. `11.0k`, `1.2M`) to fit HUD width
- `fps >= 100` → integer (e.g. `451`)
- `fps < 100` → 1 decimal (e.g. `59.8`)

### 2. ` tgt: <N>[ idle| paused]`

**User-configured target FPS cap** (from `--fps` or config.toml `fps =`),
with an optional mode suffix indicating whether the cap is currently in
effect.

- ` tgt: 30` — active, loop targeting 30 FPS
- ` tgt: 30 idle` — adaptive idle throttle engaged (effective rate is
  `target_fps * IDLE_FPS_FACTOR`, typically 0.5×, so ~15 FPS). Triggered
  after `IDLE_THRESHOLD_SECS` (30s) of no input.
- ` tgt: 30 paused` — user pressed Space or `p`. Loop ticks at
  `PAUSE_PERIOD_MS` (250ms = 4 Hz) just to keep the event loop alive.

**Why this line exists:** before v30 (2026-08-05), the HUD only had
`fps:`. Users who set `--fps 30` saw `fps: 11000` and thought the
flag was broken. The `tgt:` line disambiguates the cap (what you
configured) from the throughput (what the renderer is actually doing).
See `docs/archive/specs/ATMOSPHERE_ENGINE.md` for the original
discussion (atmosphere engine archival — the HUD `tgt:` line was added
in the same Dragon Hunt v2 Phase 6 window).

### 3. ` p99: <ms>`

**99th-percentile frame time** in milliseconds, computed from a ring
buffer of recent frame times (stack-allocated sort, ~300ns, called
once per second).

p99 is the slowest 1% of frames — it's the metric that catches
infrequent spikes that `avg` hides. A healthy p99 is `< 2× avg`. A p99
that is `10× avg` means there are periodic stalls (GC pauses, kernel
scheduling, terminal-emulator backpressure).

### 4. ` max: <ms>`

**Maximum frame time** observed in the last 60 seconds. Auto-resets
every `MAX_RESET_INTERVAL_SECS` (60s) so a startup spike from 10
minutes ago doesn't dominate the display forever.

Use `max` together with `p99`: if `max` is much larger than `p99`,
the spike was a one-off (likely a resize event, signal, or first-frame
cold cache). If `max` is close to `p99`, the slow path is recurring.

### 5. ` rss: <size>`

**Process RSS** (resident set size) in KiB or MiB. Sampled at 1 Hz via
`memstat::current_rss_kb()` (reads `/proc/self/status` on Linux,
`task_info` on macOS, `getrusage` on BSD/Android).

On Linux, `rss` includes all resident pages (code + heap + mmap'd
files). A growing `rss` over a long session suggests a memory leak
— check `docs/ENDURANCE.md` for the leak-detection methodology.

### 6. ` cpu: <percent>`

**Process CPU%** with 2-decimal precision. Sampled at 1 Hz as the
delta between two `cpustat::current_cpu_ns()` readings (Linux:
`/proc/self/stat` field 14 + 15; macOS: `task_info` with
`TASK_THREAD_TIMES_INFO_COUNT`; BSD/Android: `getrusage(RUSAGE_SELF)`).

On a single-threaded build, `cpu:` typically reads 0-5% during active
rendering (the loop sleeps most of the frame period). On a build with
worker threads, it can briefly exceed 100% — clamped at 999.99% for
display width safety.

Shows `—` (em dash) on unsupported platforms (non-unix) or during the
brief pre-delta window (first ~1s after HUD toggle-on).

### 7. ` up: <duration>`

**Session uptime** since the HUD was created (process startup). Format:
- `< 1h`: `MM:SS` (e.g. `59:03`)
- `< 1d`: `Xh:MM` (e.g. `1h:03`)
- `>= 1d`: `Xd:YYh` (e.g. `2d:03h`)

### 8. `<W>x<H> <mode>`

**Terminal size** as `columns x rows`, with mode:
- `auto` — dynamic (follows terminal resize)
- `fix` — fixed via `--screen-size WxH` (ignores resize)

---

## HUD vs `--benchmark` — Why They Disagree

The HUD and `--benchmark` measure DIFFERENT things:

| Metric | HUD | `--benchmark` |
|--------|-----|---------------|
| `fps` / `avg_fps` | `1000 / work_ms` (render-work throughput, sleep excluded) | `frames / wall_clock_s` (true throughput, sleep included) |
| `target_fps` | shown as `tgt:` | printed in CONFIG section |
| Frame time source | rolling ring buffer (recent frames) | full-session aggregate |

**Typical disagreement:** with `--fps 60` on a fast machine, the HUD
`fps:` line may show `5000` (because render work takes 0.2ms), while
`--benchmark` `avg_fps` reports `~60` (because the loop sleeps to
maintain the cap). Both are correct — they measure different things.

The HUD `fps:` line is most useful for catching RENDER SLOWDOWNS (when
`work_ms` spikes, `fps:` drops). The `--benchmark` `avg_fps` is most
useful for measuring USER-PERCEIVED throughput (what the user actually
sees on screen).

---

## HUD vs `target_fps` (the `--fps` confusion)

**Common confusion (the one that prompted the `tgt:` line in v30):**

User runs `cosmostrix --fps 30`, presses `i`, sees:

```
 fps: 11000
 tgt: 30
 p99: 0.150ms
 ...
```

The `fps: 11000` is correct — it's the render-work throughput. The
loop is doing 0.09ms of work per frame and then sleeping 33.2ms to
hit the 30 FPS cap. The `tgt: 30` confirms the cap is in effect.

Without the `tgt:` line (pre-v30), the same display showed only
`fps: 11000` and the user thought `--fps 30` was broken.

**Rule of thumb:**
- Use `tgt:` to verify your `--fps` setting was applied.
- Use `fps:` to see how much headroom the renderer has. If `fps:`
  drops close to `tgt:`, the renderer is the bottleneck (work_ms
  approaches the frame period). If `fps:` is 100× `tgt:`, the
  renderer is idle most of the frame.

---

## HUD Color Scheme

HUD colors come from the active rain palette, hue-preserving brightened
via HSV value scaling so the HUD follows the rain's actual color scheme
(green rain → green HUD, amber rain → amber HUD) instead of washing
out to grey.

Each line uses a different palette position:
- `fps`, `tgt`, `max` — head (brightest palette stop, brightened)
- `p99`, `cpu` — mid (middle palette stop, brightened)
- `rss` — trail (1/4 palette position, brightened)
- `up`, `screensize` — dim (palette index 1, brightened)

Brightening is hue-preserving (not a white blend) so the HUD stays
readable on dark rain palettes without desaturating the rain's color
identity.

---

## Design Constraints

1. **Zero cost when off.** `visible == false` short-circuits all HUD
   work. The `i` key press is the only thing that activates it.

2. **Metrics at 1 Hz.** p99 sort + string formatting only every 1000ms.
   Faster rates (e.g. the previous 4 Hz cadence) made FPS/p99 visibly
   flicker 4×/sec which read as "wasteful" even though CPU cost was
   negligible (~30 µs/s).

3. **Frame buffer integration.** HUD cells written via `frame.set()`
   (not `set_force`) so unchanged cells are NOT marked dirty — the
   terminal skips re-sending them. When metrics are stable, only the
   uptime seconds change between frames.

4. **Dynamic width.** Lines are formatted WITHOUT fixed-width padding.
   The HUD width grows/shrinks to fit the longest line. Capped at
   `HUD_MAX_WIDTH` (22 cols) to prevent the HUD from eating the whole
   terminal. Floor at `HUD_MIN_WIDTH` (12 cols) for short values.

5. **No `\x1b[2K` line clear.** The HUD writes only `current_width`
   characters per line — rain on the rest of the line is preserved.
   This was the root cause of the historical "blank space above rain"
   bug: `\x1b[2K` cleared all columns, not just the HUD area.

---

## See Also

- [`docs/BENCHMARKING.md`](BENCHMARKING.md) — `--benchmark` report
  fields and methodology
- [`docs/PERFORMANCE_ACROSS_SCALES.md`](PERFORMANCE_ACROSS_SCALES.md) —
  scaling audit from 6×6 to 400×200
- [`docs/ENDURANCE.md`](ENDURANCE.md) — endurance testing methodology
  (uses HUD `rss` and `p99` for leak / stall detection)
- [`docs/RULES.md`](RULES.md) — project conventions and CLI flag policy
