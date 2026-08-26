<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Live HUD Overlay

The HUD is cosmostrix's live performance overlay for interactive mode.
Toggle it with the `i` key. Zero cost when off (all methods short-circuit
on `visible == false`). Metrics recompute at 1 Hz (matches htop,
mangoHUD, Steam FPS counter, and `nvidia-smi` — faster rates cause
number flicker without improving diagnostic value).

v50.0.0-beta.6 HUD expansion: the HUD grew from 9 rows to 18 rows,
adding 7 owner-mandated metric lines (ehs / prs / sped / dsty / scn /
chr / clr) plus 2 dragon indicator lines (prdr / crdr). The `h` shortkey
that previously toggled the HUD position (left <-> right corner) was
**completely removed** as unused maintenance cost; the HUD now always
renders flush-left at column 0 (the previous default). There is no `h`
binding — it is silently ignored (catch-all).

This document is the canonical reference for what each HUD line means,
why it can disagree with `--benchmark` numbers, and how to use it to
diagnose common issues. New users: start with the [Quick Reference](#quick-reference)
table and the [How to Read the HUD in 10 Seconds](#how-to-read-the-hud-in-10-seconds)
section. Veterans jump straight to [Diagnostic Recipes](#diagnostic-recipes)
or [Common Misreadings](#common-misreadings--pitfalls).

---

## Quick Reference

At-a-glance table for users who just pressed `i` and need to know what
each line means without reading the full reference below.

| Row | Label        | Unit           | What it tells you in one sentence                                                                  |
|-----|--------------|----------------|----------------------------------------------------------------------------------------------------|
| 0   | `fps:`      | FPS (number)   | Render-work throughput = `1000 / avg_work_ms`. Often 10-100× higher than `tgt:` because loop sleep is excluded. |
| 1   | `tgt:`      | FPS (number)   | **Target** FPS cap from `--fps` / `config.toml`. The cap you configured, with optional `idle` / `paused` mode suffix. |
| 2   | `max:`      | ms             | Maximum frame time observed in the last 60 seconds. Auto-resets so a startup spike does not dominate forever. |
| 3   | `p99:`      | ms             | 99th-percentile frame time. The slowest 1% of recent frames — catches spikes `avg` hides.          |
| 4   | `cpu:`      | percent        | Process CPU usage. 0-5% typical single-threaded; can briefly exceed 100% on multi-threaded builds. |
| 5   | `rss:`      | KiB / MiB      | Process resident set size (memory). Watch for steady growth -> possible leak.                       |
| 6   | `ehs:`      | 0-100 (int)    | **Endurance Health Score** — long-endurance process stability from RSS variance + frame jitter + ctxt-switch rate. 100 = stable, <50 = degraded. |
| 7   | `prs:`      | 0.00-1.00      | **Effective Pressure** — drives spawn rate, sim factor, self-healer. 0.0 = no pressure, 1.0 = max throttle. |
| 8   | `sped:`     | chars/sec (1dp) | **Speed** — chars-per-second. User adjusts via `Up`/`Down`. Confirms the actual sanitized value (matches `--speed`). |
| 9   | `dsty:`     | multiplier (2dp) | **Density** — droplet density multiplier. User adjusts via `[`/`]`. Label is `dsty` (NOT `den`) per owner mandate. |
| 10  | `scn:`      | string         | **Scene name** — current scene (e.g. `cinematic`, `matrix`, or a custom scene). Confirms `x` cycle position. |
| 11  | `chr:`      | string         | **Charset preset** — current charset (e.g. `binary`, `zen`). Confirms `s`/`S` cycle position. |
| 12  | `clr:`      | string (Debug) | **Color scheme** — active scheme name via Debug format (e.g. `NeonGreen`, `FancyDiamond`). Confirms `c`/`C` cycle. |
| 13  | `up:`       | MM:SS / Xh:MM / Xd:YYh | Session uptime since process start.                                                       |
| 14  | (no label)   | WxH auto/fix   | Terminal size in columns × rows, plus `auto` (follows resize) or `fix` (`--screen-size`).          |
| 15  | `cid:`      | hex short SHA  | Build commit id (7-char git short SHA). Lets you verify the exact build without quitting cosmostrix. |

**Symbol legend:**

| Symbol / Suffix | Meaning                                                                                          |
|-----------------|--------------------------------------------------------------------------------------------------|
| `idle`         | After `tgt:` — adaptive idle throttle engaged (no input for 30s; effective FPS = `tgt × 0.5`).   |
| `paused`       | After `tgt:` — user pressed `p`; loop ticking at 4 Hz just to keep event loop alive.             |
| `ms`            | Milliseconds (frame time unit). 1ms = 0.001s. A 60 FPS target = 16.67ms budget per frame.        |
| `KiB` / `MiB`   | 1024 bytes / 1024² bytes (binary, NOT decimal SI units).                                         |
| `%`             | Percent of one CPU core. 100% = one full core. Multi-threaded spills can exceed 100%.            |
| `—` (em dash)   | Metric unavailable: unsupported platform (non-unix for `cpu:`) or pre-delta window (first ~1s).  |
| `auto` / `fix`  | After screensize: `auto` = follows terminal resize, `fix` = `--screen-size WxH` locked.          |
| `cid:`          | Commit id line — 7-char lowercase hex git short SHA injected at compile time by `build.rs`. Falls back to `unknown` for tarball builds without `.git`. |

---

## Annotated HUD Layout

What you actually see in the top-left corner after pressing `i`. All 16
rows are visible at once; this mockup annotates each:

```text
┌─────────────────────────┐
│ fps: 451      ◄── 0.  render-work throughput (NOT the cap)
│ tgt: 60       ◄── 1.  your --fps cap, "60" = sixty FPS target
│ max: 1.204ms  ◄── 2.  worst frame in last 60s (auto-resets)
│ p99: 0.832ms  ◄── 3.  slowest 1% of frames (spike detector)
│ cpu: 1.43%    ◄── 4.  process CPU% (one core = 100%)
│ rss: 8.2MiB   ◄── 5.  process memory (leak detector)
│ ehs: 87       ◄── 6.  endurance health score (0-100, 100=stable)
│ prs: 0.12     ◄── 7.  effective pressure (drives spawn+sim+self-healer)
│ sped: 14.0    ◄── 8.  chars/sec speed (Up/Down adjustable)
│ dsty: 1.00    ◄── 9.  density multiplier ([/]) — `dsty` per owner mandate
│ scn: cinematic ◄── 10. scene name (x cycle confirmation)
│ chr: binary   ◄── 11. charset preset (s/S cycle confirmation)
│ clr: NeonGreen ◄── 12. color scheme (c/C cycle confirmation)
│ up: 03:42     ◄── 13. session uptime (MM:SS under 1h)
│ 200x50 auto   ◄── 14. terminal size + mode (auto/fix)
│ cid: 6ed244b  ◄── 15. build commit id (verify without quitting)
└─────────────────────────┘
```

**Color gradient (top dim -> bottom bright):** the HUD mirrors a falling
rain droplet — the bottom row (`cid`) earns the brightest `head` stop
(rain leading character), the top row (`fps`) is the dimmest `tail`
(rain trailing fade). The `cid` line earns the head position because the
build identity is the most definitive info the owner reads to verify
which commit is running. See [HUD Color Scheme](#hud-color-scheme)
below for the full palette mapping.

**Width is dynamic:** the HUD grows to fit the longest line (capped at
22 cols, floored at 12 cols). High-FPS values like `fps: 11000` push
the width out; short values like `fps: 30` let it shrink. The 7 new
metric rows (ehs/prs/sped/dsty/scn/chr/clr) are all ≤ 18 chars so
they never dominate the width budget.

---

## How to Read the HUD in 10 Seconds

1. **Check `tgt:` first** — confirms your `--fps` setting was applied.
   If you ran `--fps 30` and `tgt:` shows `30`, the cap is in effect.
   If `tgt:` shows `30 idle`, the idle throttle kicked in (no input
   for 30s) — effective rate is ~15 FPS. If `tgt:` shows `30 paused`,
   you pressed `p` — press again to resume.

2. **Compare `fps:` to `tgt:`** — `fps:` is render-work throughput
   (how fast the renderer *could* draw), `tgt:` is the cap (how fast
   it *is* drawing). If `fps:` >> `tgt:`, the renderer has huge
   headroom and the loop is sleeping. If `fps:` ≈ `tgt:`, the
   renderer is the bottleneck (work_ms approaches the frame period).

3. **Check `p99:` and `max:` together** — `p99:` catches recurring
   spikes, `max:` catches one-off spikes. If `max:` >> `p99:`, the
   worst frame was a fluke (resize, signal, cold cache). If `max:` ≈
   `p99:`, the slow path is recurring — investigate.

4. **Watch `rss:` over time** — a flat `rss:` is healthy. Steady growth
   across minutes suggests a memory leak; see `docs/ENDURANCE.md` for
   the leak-detection methodology.

5. **Glance at `cpu:`** — single-threaded builds typically read 0-5%
   during active rendering (the loop sleeps most of the frame). If
   `cpu:` reads >50% on a single-threaded build, the renderer is CPU-
   bound. On multi-threaded builds, brief spikes above 100% are normal.

The remaining sections go deeper into each line, edge cases, and
diagnostic recipes for specific symptoms.

---

## HUD Lines (top-to-bottom)

The HUD writes 16 rows into the frame buffer at the top-left corner
(column 0). Each row is one metric. Rows 0-5 are the performance core
(unchanged from v50), rows 6-12 are the 7 owner-mandated HUD expansion
metrics (ehs / prs / sped / dsty / scn / chr / clr), and rows 13-15
are session/diagnostic/build identity (up / screensize / cid).

### 1. `fps: <N>`

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

- `fps >= 10000` -> humanized (e.g. `11.0k`, `1.2M`) to fit HUD width
- `fps >= 100` -> integer (e.g. `451`)
- `fps < 100` -> 1 decimal (e.g. `59.8`)

### 2. `tgt: <N>[ idle| paused]`

**User-configured target FPS cap** (from `--fps` or config.toml `fps =`),
with an optional mode suffix indicating whether the cap is currently in
effect. The label `tgt` is short for **target** — it is the frame-rate
ceiling the loop is aiming for, distinct from `fps:` which is the
render-work throughput (often far above the cap because loop sleep is
excluded from `work_ms`). Read the two lines together: `tgt:` is the
goal, `fps:` is the headroom.

- `tgt: 30` — active, loop targeting 30 FPS
- `tgt: 30 idle` — adaptive idle throttle engaged (effective rate is
  `target_fps * IDLE_FPS_FACTOR`, typically 0.5×, so ~15 FPS). Triggered
  after `IDLE_THRESHOLD_SECS` (30s) of no input.
- `tgt: 30 paused` — user pressed `p`. Loop ticks at
  `PAUSE_PERIOD_MS` (250ms = 4 Hz) just to keep the event loop alive.

**Why this line exists:** before v30 (2026-08-05), the HUD only had
`fps:`. Users who set `--fps 30` saw `fps: 11000` and thought the
flag was broken. The `tgt:` line disambiguates the cap (what you
configured) from the throughput (what the renderer is actually doing).
See `docs/archive/specs/ATMOSPHERE_ENGINE.md` for the original
discussion (atmosphere engine archival — the HUD `tgt:` line was added
in the same Dragon Hunt v2 Phase 6 window).

### 3. `max: <ms>`

**Maximum frame time** observed in the last 60 seconds. Auto-resets
every `MAX_RESET_INTERVAL_SECS` (60s) so a startup spike from 10
minutes ago doesn't dominate the display forever.

Use `max` together with `p99`: if `max` is much larger than `p99`,
the spike was a one-off (likely a resize event, signal, or first-frame
cold cache). If `max` is close to `p99`, the slow path is recurring.

### 4. `p99: <ms>`

**99th-percentile frame time** in milliseconds, computed from a ring
buffer of recent frame times (stack-allocated sort, ~300ns, called
once per second).

p99 is the slowest 1% of frames — it's the metric that catches
infrequent spikes that `avg` hides. A healthy p99 is `< 2× avg`. A p99
that is `10× avg` means there are periodic stalls (GC pauses, kernel
scheduling, terminal-emulator backpressure).

### 5. `cpu: <percent>`

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

### 6. `rss: <size>`

**Process RSS** (resident set size) in KiB or MiB. Sampled at 1 Hz via
`memstat::current_rss_kb()` (reads `/proc/self/status` on Linux,
`task_info` on macOS, `getrusage` on BSD/Android).

On Linux, `rss` includes all resident pages (code + heap + mmap'd
files). A growing `rss` over a long session suggests a memory leak
— check `docs/ENDURANCE.md` for the leak-detection methodology.

### 7. `up: <duration>`

**Session uptime** since the HUD was created (process startup). Format:

- `< 1h`: `MM:SS` (e.g. `59:03`)
- `< 1d`: `Xh:MM` (e.g. `1h:03`)
- `>= 1d`: `Xd:YYh` (e.g. `2d:03h`)

### 8. `<W>x<H> <mode>`

**Terminal size** as `columns x rows`, with mode:

- `auto` — dynamic (follows terminal resize)
- `fix` — fixed via `--screen-size WxH` (ignores resize)

### 9. `cid: <short-SHA>`

**Build commit id** — the 7-character lowercase hex git short SHA
injected at compile time by `build.rs` (via `git rev-parse --short=7
HEAD`, exposed as the `COSMOSTRIX_GIT_SHA` env var). Falls back to the
literal string `unknown` for tarball/release builds that had no `.git`
directory available at compile time.

**Why this line exists:** the owner needs to verify which exact commit
is running without quitting cosmostrix. Previously, checking the build
version required `q` to exit, then `git log -1` or `cosmostrix
--version`, then re-launching — disruptive to long-running sessions.
The `cid:` line puts the answer in the corner at all times when the
HUD is visible.

**Why the text never changes:** the SHA is baked into the binary at
compile time. The line is set once in `HudState::new()` and only its
color is refreshed by `refresh_colors` every frame (it occupies the
head stop — palette last-stop, the brightest position — because the
build identity is the most definitive info the owner reads to verify
which commit is running).

**Cross-reference:** the same SHA is printed by `cosmostrix --version`
and emitted in `--benchmark` JSON output as the `git_sha` field.

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
 max: 0.150ms
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
(green rain -> green HUD, amber rain -> amber HUD) instead of washing
out to grey.

### Rain-aesthetic gradient (top dim -> bottom bright)

The 16 HUD lines form a vertical brightness gradient that mirrors a
falling rain droplet — the bottom lines (`cid`, `screensize`, `up`) are
the brightest `head` (palette last-stop, the rain's leading bright
character), and the top lines (`fps`, `tgt`) are the dimmest `tail`
(palette index 1, the rain's trailing fade). Mid lines span `trail` and
`mid` so the eye reads the HUD as a small rain column hanging in the
corner, not as a flat block of equally-bright text.

| Row | Line         | Color level | Palette position         |
|-----|--------------|-------------|--------------------------|
| 0   | `fps`        | dim         | palette index 1 (tail)   |
| 1   | `tgt`        | dim         | palette index 1 (tail)   |
| 2   | `max`        | trail       | palette index n/4        |
| 3   | `p99`        | trail       | palette index n/4        |
| 4   | `cpu`        | mid         | palette index n/2 (body) |
| 5   | `rss`        | mid         | palette index n/2 (body) |
| 6   | `ehs`        | mid         | palette index n/2 (body) |
| 7   | `prs`        | mid         | palette index n/2 (body) |
| 8   | `sped`       | trail       | palette index n/4        |
| 9   | `dsty`       | trail       | palette index n/4        |
| 10  | `scn`        | trail       | palette index n/4        |
| 11  | `chr`        | trail       | palette index n/4        |
| 12  | `clr`        | trail       | palette index n/4        |
| 13  | `up`         | head        | palette last stop        |
| 14  | `screensize` | head        | palette last stop        |
| 15  | `cid`        | head        | palette last stop        |

This inverts the original pre-v50-alpha.4 mapping where `fps`/`tgt`/`max`
were the brightest at the TOP. The owner explicitly flagged the inversion:
"rain tail is dim head is white" — the bright head must lead at the
bottom, matching a real falling rain stream.

### Instant palette refresh (no delay on runtime changes)

Color refresh is split out of the 1 Hz metric tick — `HudState::refresh_colors`
runs every frame (cheap: 4 `brighten_color` calls ≈ 2 µs) so a runtime
palette change is reflected on the very next frame, with no perceptible
delay. The 1 Hz rate limit only governs text reformatting (p99 sort,
`format!` calls, RSS string) — that's what causes the numbers to update
once per second, but the COLORS track the rain immediately.

This matters for: `c`/`C` key color cycling, `--crystal-dragon`,
live-config reload (`config.toml` edit while running), and scene
transitions (`x` key). All of these change the palette at runtime; the
HUD must keep up without a visible "lag" where the rain has new colors
but the HUD still shows the old palette.

Brightening is hue-preserving (not a white blend) so the HUD stays
readable on dark rain palettes without desaturating the rain's color
identity. The `dim` (tail) level is brightened just like the others —
TARGET_V = 200 guarantees readability on a black background even when
the palette's tail stop is very dark (e.g. RGB(0,50,0) is boosted to
RGB(0,200,0), preserving the green hue).

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

## Diagnostic Recipes

Symptom -> likely cause -> what to check -> action. Use this table when
the HUD is showing something unexpected and you need a starting point.

| Symptom                                            | Likely cause                                         | What to check                                             | Action                                                                                          |
|----------------------------------------------------|------------------------------------------------------|-----------------------------------------------------------|-------------------------------------------------------------------------------------------------|
| `fps:` shows a huge number (e.g. `11000`) with `--fps 30` | Loop sleeping to maintain cap (NOT a bug)      | `tgt:` line — should show `30`                            | None needed. Read `tgt:` to verify the cap, ignore `fps:` for cap verification.                 |
| `tgt:` shows `30 idle`                             | No input for 30s, idle throttle engaged              | Recent keyboard/mouse activity                            | Press any key or click to return to active mode; `tgt:` reverts to `30`.                        |
| `tgt:` shows `30 paused`                           | `p` was pressed                                      | Recent key presses                                        | Press `p` again to resume.                                                                      |
| `p99:` >> `avg` (e.g. p99=10ms, avg=0.5ms)         | Periodic stalls (GC, kernel scheduling, terminal backpressure) | Recent terminal activity, other running processes | Investigate with `strace`/`perf` on Linux; check terminal GPU acceleration settings.            |
| `max:` >> `p99:` (e.g. max=50ms, p99=2ms)          | One-off spike (resize, signal, first-frame cold cache) | Whether a resize or signal happened around the spike time | Safe to ignore unless it recurs. `max:` auto-resets every 60s.                                  |
| `rss:` grows steadily over minutes                 | Possible memory leak                                 | `rss:` trend over 5-10 minutes                            | See `docs/ENDURANCE.md` for leak-detection methodology. Run `--benchmark` for a fixed-duration sample. |
| `cpu:` shows `—` (em dash)                         | Unsupported platform (non-unix) or pre-delta window  | Platform; time since HUD toggle-on                        | On unix, the em dash should disappear within 1s. On non-unix, `cpu:` is permanently unavailable. |
| `cpu:` > 100%                                      | Multi-threaded build spilling onto another core      | Build flags (single-threaded vs multi-threaded)           | Brief spikes are normal. Sustained >100% suggests worker threads are saturated.                 |
| `up:` shows wrong uptime                           | `session_start` set at HUD creation, not process start | Process start time vs HUD toggle-on time                  | `up:` measures time since the `HudState` was constructed (process startup), not since `i` press. |
| Screensize shows `200x50 fix` when terminal resized | `--screen-size WxH` was passed, locking the size     | CLI flags / config.toml                                   | Remove `--screen-size` to let the size follow terminal resize (`auto` mode).                    |
| HUD does not appear after pressing `i`             | HUD is off; or terminal width too small for the HUD_MIN_WIDTH floor (12 cols) | Check terminal size, or toggle off/on again          | The HUD always renders at column 0 (top-left). If invisible, verify the terminal width is ≥ 12 cols. |
| HUD numbers flicker / change too fast              | Expected at 1 Hz — if faster, check for a regression | `HUD_METRIC_INTERVAL` constant in `src/interactive/hud/mod.rs` | 1 Hz is the world-class standard (htop, mangoHUD). Do not increase the rate.                    |
| HUD colors look grey / washed out                  | Palette has very dim stops; brighten fallback engaged | Active palette (`c`/`C` to cycle, or check config)        | Pure-black palette stops fall back to neutral grey RGB(120,120,120). Use a palette with non-black stops. |

---

## Common Misreadings & Pitfalls

Explicit list of ways users get confused by the HUD. Each entry states
the wrong reading, the correct reading, and why the difference matters.

### Misreading 1: "fps: 11000 means my `--fps 30` flag is broken"

**Wrong:** `fps:` is the loop's frame-rate cap.
**Correct:** `fps:` is render-work throughput = `1000 / work_ms`. The
loop sleeps between frames to maintain the `--fps` cap, and sleep time
is NOT part of `work_ms`. So a 30 FPS cap with 0.1ms render work shows
`fps: 10000` — the renderer could draw 10000 frames/sec if unconstrained,
but the cap holds it to 30.
**Why it matters:** users file bug reports about `--fps` being ignored
when it is actually working correctly. Always cross-check `tgt:` to
verify the cap.
**Fix:** v30 (2026-08-05) added the `tgt:` line specifically to
disambiguate this. See [HUD vs `target_fps`](#hud-vs-target_fps-the---fps-confusion).

### Misreading 2: "cpu: 0.50% means the renderer is doing nothing"

**Wrong:** `cpu:` measures total process activity.
**Correct:** `cpu:` measures process CPU% as a fraction of one core.
A single-threaded build at 0.5% means the renderer is doing 0.5% of
one core's work — the other 99.5% is loop sleep (maintaining the cap)
or kernel-side I/O. On a 60 FPS cap with 0.1ms render work, the loop
is active 0.3% of the time (0.1ms / 33.3ms).
**Why it matters:** users think the renderer is stalled when it is
actually just idle-throttled by design.

### Misreading 3: "rss: 8.2MiB is a memory leak"

**Wrong:** Any non-zero `rss:` indicates a leak.
**Correct:** `rss:` is the resident set size — all resident pages
including code, heap, and mmap'd files. A flat `rss:` around 8-15 MiB
is normal for cosmostrix. A leak shows as STEADY GROWTH across minutes
or hours, not as a fixed elevated value.
**Why it matters:** users file leak reports for normal startup RSS.
Check the trend, not the absolute value.

### Misreading 4: "max: 50ms means the renderer is slow"

**Wrong:** `max:` reflects current renderer performance.
**Correct:** `max:` is the worst frame time in the last 60 seconds.
A single resize event or signal can produce a 50ms spike that has
nothing to do with render performance. Compare `max:` to `p99:` — if
`p99:` is low (e.g. 2ms) and `max:` is high (e.g. 50ms), the spike was
a one-off, not a recurring slow path.
**Why it matters:** users optimize for a one-off spike that will never
recur. `max:` auto-resets every 60s to surface only recent peaks.

### Misreading 5: "tgt: 30 idle means the renderer is broken"

**Wrong:** `idle` suffix means the renderer crashed or stalled.
**Correct:** `idle` means the adaptive idle throttle engaged after 30s
of no input. The loop is intentionally running at half-rate (`target_fps
× IDLE_FPS_FACTOR`, typically 0.5×) to save CPU when no one is watching.
Any input (key press, mouse click, mouse move) returns to active mode.
**Why it matters:** users think the program is hung. Press any key to
verify it snaps back to `tgt: 30` (no suffix).

### Misreading 6: "screensize 80x24 auto changes when I resize my window"

**Wrong:** `auto` means it auto-detects once at startup.
**Correct:** `auto` means the size follows terminal resize events in
real time. `fix` means `--screen-size WxH` was passed and the size is
locked (resize events are ignored). If you see `fix` and want resize
to work, remove `--screen-size` from your CLI / config.
**Why it matters:** users think resize is broken when they accidentally
passed `--screen-size`.

### Misreading 7: "the HUD colors are wrong — they don't match my palette"

**Wrong:** HUD colors are hardcoded.
**Correct:** HUD colors come from the active rain palette, hue-preserving
brightened via HSV value scaling. A green rain produces a green HUD; an
amber rain produces an amber HUD. If the HUD looks grey, the palette's
tail stop is probably pure black (RGB 0,0,0), which falls back to
neutral grey RGB(120,120,120) because hue cannot be preserved on black.
**Why it matters:** users think the HUD is decoupled from the palette.
Cycle palettes with `c`/`C` to see the HUD track the rain's color.

### Misreading 8: "p99: 0.000ms means frames are taking zero time"

**Wrong:** `p99: 0.000ms` means the renderer is infinitely fast.
**Correct:** `p99: 0.000ms` usually means the ring buffer is empty
(no frames have been recorded yet) or all recorded frames rounded to
0.000ms at 3-decimal precision. This is common in the first second
after HUD toggle-on. Once frames accumulate, `p99:` shows real values.
**Why it matters:** users think the HUD is broken. Wait 1-2 seconds
for the ring buffer to fill.

---

## See Also

- [`docs/BENCHMARKING.md`](BENCHMARKING.md) — `--benchmark` report
  fields and methodology
- [`docs/PERFORMANCE_ACROSS_SCALES.md`](PERFORMANCE_ACROSS_SCALES.md) —
  scaling audit from 6×6 to 400×200
- [`docs/ENDURANCE.md`](ENDURANCE.md) — endurance testing methodology
  (uses HUD `rss` and `p99` for leak / stall detection)
- [`docs/RULES.md`](RULES.md) — project conventions and CLI flag policy
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
