<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Ambient Phase Scheduler

Config-driven time-of-day scene/parameter switching — replaces the archived
`adaptive-custom` subsystem (eliminated with the atmosphere engine at commit
`07b44b5`) with a simpler contract: **config-only** (no CLI flag),
**instant switch** (no blend window), and a **dynamic idle/wake scheduler
thread** (zero CPU between phase boundaries).

## Config Format

```toml
# ambient.<HH-MM> = <color>, <scene>, [key=value, ...]
ambient.00-00 = cosmos, monolith, speed=15, density=1.2
ambient.06-00 = aurora, matrix, speed=60, density=0.5
ambient.22-00 = neon, monolith, speed=10
```

### Fields

- **`HH-MM`** (key suffix): 24-hour time, zero-padded (`00-00` to `23-59`).
  The phase becomes active at this wall-clock minute and stays active until
  the next entry's boundary.
- **Positional 1** (`color`): built-in scheme name (52 themes) OR a
  `colors-custom.<name>` palette name. Optional — if omitted, color is
  sticky (keeps previous value). If the first positional is a valid scene
  name but NOT a valid color name (e.g. `monolith`), it's treated as scene
  (color omitted) — this lets you write `ambient.12-00 = monolith, speed=15`.
- **Positional 2** (`scene`): built-in scene name (`matrix`, `monolith`,
  `signal`, `cinematic`, `cosmos`, `calm`, `storm`, `neon`, `hacker`,
  `low-power`, `classic`, `carbonic`, `cosmic_dragon`, `matrix_film`).
  Optional — if omitted, scene is sticky.
- **Optional `key=value` pairs**:
  - `speed` — float in `[1.0, 100.0]` (asymmetric vs top-level `speed`
    which is integer; float allows future lerp extension).
  - `density` — float in `[0.01, 5.0]`.
  - `fps` — integer in `[1, 120]`.
  - `charset` — built-in charset name OR `charset-custom.<name>`.
  - `glitch-level` — one of `none`, `subtle`, `default`, `intense`.

### Sticky Semantics

Fields not specified in a phase entry keep the previous value (the engine
does NOT reset unspecified fields to defaults when transitioning between
phases). This matches the archived `adaptive-custom` contract.

Example: if `ambient.06-00 = aurora, matrix, speed=60` fires, and the next
entry `ambient.22-00 = neon` (no scene, no speed) fires, the scene stays
`matrix` and speed stays `60` — only color changes to `neon`.

## Behavior

### Dynamic Idle/Wake Scheduler

The scheduler thread does NOT poll on a fixed interval. Instead it:

1. Computes `time_to_next_phase` (seconds until the next entry's `HH:MM`
   boundary, with midnight wrap-around).
2. Sleeps for that duration via `Condvar::wait_timeout` (capped at 1 hour
   for reload responsiveness).
3. On wake, fires the new phase via mpsc channel to the event loop.
4. Returns to step 1.

Between phase boundaries, the thread is parked — **zero CPU usage, zero
wakeups**. The OS only schedules it when:

- The timeout expires (a phase boundary was reached), OR
- The condvar is notified (live-reload path pushed a new schedule).

This is the design the owner explicitly requested:

> "dynamic clock, bro — it doesn't have to stay awake continuously; idle
> when the time is approaching, then a few seconds before, automatically
> wake up — so CPU usage doesn't stay high all the time when the user
> uses the ambient config"

### Instant Switch

There is no blend window. When the scheduler fires a phase entry, the
scene/color/charset/speed/density/glitch-level are applied immediately via
`Cloud::apply_ambient_entry`. The only visual smoothing comes from the
existing `transition_chars` (glyph warm-start) and `transition_rain_style`
(pool reset) — those exist for correctness (preventing ghosting), not for
cinematic blending.

The owner explicitly chose instant switch over the archived 5-minute
smoothstep blend window:

> "use instant switch for the blend window"

### Live Reload

Editing `ambient.*` keys in `config.toml` triggers an immediate re-parse
via `collect_ambient_schedule`. The new schedule replaces the old one
atomically (mutex swap), and the scheduler thread is woken via condvar to
recompute the next phase boundary. If the new schedule's currently-active
phase differs from the previously-applied one, the thread fires it on the
next loop iteration (no boundary wait).

### CLI Flag

**None.** Ambient is config-only — there is no `--ambient` CLI flag. This
matches the archived `adaptive-custom` contract (which was also config-
only) and is consistent with cosmostrix's naming convention: time-driven
schedulers have no CLI analog (you can't "pass --ambient 12-00" at startup
because the scheduler runs continuously).

## Module Map

| Module | Responsibility |
|--------|----------------|
| `src/ambient.rs` | Parser, `AmbientEntry` / `AmbientSchedule` structs, `current_phase` / `next_phase` / `seconds_to_next_phase` helpers, strict validation (`validate_ambient_entries`), wall-clock helpers (`current_minute_of_day`, `current_second_of_minute`) |
| `src/ambient_scheduler.rs` | Dynamic idle/wake scheduler thread, `AmbientSchedulerHandle`, `spawn_ambient_scheduler`, `reload` |
| `src/cloud/scene_runtime.rs` | `Cloud::apply_ambient_entry` — applies an entry to the live Cloud with sticky semantics |
| `src/interactive/event_loop.rs` | Spawns scheduler at startup, polls `rx` each frame, pushes reload on config change |
| `src/live_config.rs` | `rebuild_cloud_config` collects new schedule from config map |
| `src/configfile.rs` | `is_known_key` dispatch + `AMBIENT_CONFIG_KEY_HINT` constant |
| `src/config_hints.rs` | Mis-nest detector for `scene-custom.<name>.ambient.<HH-MM>` |
| `src/testconf.rs` | Strict validation of `ambient.*` entries (color/scene/charset/glitch-level/speed/density/fps ranges) |
| `src/app.rs` | `CloudConfig.ambient_schedule` field |

## Edge Cases

| Edge case | Handling |
|-----------|----------|
| **Empty schedule** (no `ambient.*` keys) | Scheduler thread idles (sleeps 60s, polls for new entries). Zero events fired. Existing scene/params retained. |
| **Single entry** | Scheduler fires it on startup (it's the current phase). Then sleeps 24h (capped to 1h, so polls hourly — but the phase is already applied, so it no-ops). |
| **Two entries same time** | Configfile parser is `HashMap::insert` (last-writer-wins). One entry survives. |
| **DST spring-forward** (2:00 AM → 3:00 AM) | `current_minute_of_day()` returns wall-clock local time. Entries in the skipped hour (02:00–02:59) are never fired. Acceptable. |
| **DST fall-back** (2:00 AM repeat) | Entries in the repeated hour (01:00–01:59) fire twice. Acceptable — `apply_ambient_entry` is idempotent. |
| **Midnight wrap** | Handled in `AmbientSchedule::seconds_to_next_phase` — `(24*60 - now_min + next_min) * 60`. |
| **Invalid color/scene name** | Strict reject via `--testconf` (exit 2). Same behavior as `colors-custom` / `scene-custom`. |
| **Invalid `speed=15.5` range** | Strict reject via `--testconf` (exit 2). |
| **Live-reload adds new entry** | Scheduler thread wakes (condvar), recomputes, fires current phase if changed. |
| **Live-reload removes all entries** | Scheduler goes idle. Existing scene/params retained (sticky). User can manually cycle via `x`/`X` keys. |

## Speed Type Asymmetry (Intentional)

Top-level `speed` (CLI `--speed`, config `speed =`) is an **integer** in
`[1, 100]`. Ambient `speed` is a **float** in `[1.0, 100.0]`.

This asymmetry is inherited from the archived `adaptive-custom` subsystem:

- Top-level `speed` is a **snap** — applied once at startup, no
  interpolation needed. Integer is simpler for users.
- Ambient `speed` could be **lerped** across a blend window in a future
  extension. Fractional values are essential for smooth transitions
  (e.g., `speed=15.5` produces a perceptibly different blend than
  `speed=15` or `speed=16`).

Currently ambient uses instant switch (no lerp), but the float type is
preserved so a future "blend mode" can be added without a breaking config
migration.

## Diagnostics

Set `COSMOSTRIX_LIVE_RELOAD_DEBUG=1` in the environment to see ambient
scheduler events:

- `[live-reload-trace] ambient: reloaded schedule with N entries` — fired
  on every live-reload that changes the schedule.
- `[live-reload-trace] ambient: schedule changed (was X entries, now Y) — pushing to scheduler thread` — fired when event loop pushes the new schedule.
- `[live-reload-trace] ambient: received phase event HH:MM (color=..., scene=...)` — fired when the scheduler thread sends a phase event.
- `[live-reload-trace] ambient-scheduler: firing phase HH:MM (...)` — fired inside the scheduler thread when it sends an event.

## See Also

- [Atmosphere Engine (archived)](archive/specs/ATMOSPHERE_ENGINE.md) — the
  historical v20 design spec for the atmosphere engine subsystem, including
  the original `adaptive-custom` design that ambient replaces.
- [Atmosphere Subsystem Archival](archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md) —
  the full elimination record (file list, KEPT-vs-DELETED table, revival
  guidance).
- [Rules](RULES.md) — project conventions and CLI flag policy.
