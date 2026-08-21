<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Ambient Phase Scheduler

Config-driven time-of-day scene switching — replaces the archived
`adaptive-custom` subsystem (eliminated with the atmosphere engine at commit
`07b44b5`) with a simpler contract: **config-only** (no CLI flag),
**instant switch** (no blend window), and a **dynamic idle/wake scheduler
thread** (zero CPU between phase boundaries).

## Config Format (simplified — breaking change)

```toml
# Each entry is a single scene name.
ambient.<HH-MM> = <scene-name>
```

The value is a **single scene name** — either a built-in scene
(`cinematic`, `signal`, `monolith`, etc.) or a custom scene defined via
`[scene-custom.<name>]`. All parameters (color, charset, speed, density,
fps, glitch-level, rain_style) live inside the scene itself, eliminating
the precedence confusion that plagued the multi-field format.

### Migration from the multi-field format

Previously accepted `ambient.15-00 = energy-zen, signal, speed=50, density=0.65`.
This format is now rejected with a migration error. To preserve the entry, define a
custom scene that captures the same parameters and reference it from a
TOP-LEVEL `ambient.*` key (NEVER place the `ambient.*` key inside the
`[scene-custom.<name>]` block — TOML would parse it as
`scene-custom.<name>.ambient.<HH-MM>`, which is rejected as unknown):

```toml
[scene-custom.afternoon]
base-scene = "signal"          # inherits signal's rain_style + defaults
color = "energy-zen"          # overrides signal's color
speed = "50"                   # overrides signal's speed
density = "0.65"               # overrides signal's density

# Top-level — outside any [section] block:
ambient.15-00 = afternoon
```

This separates concerns cleanly: the schedule says WHEN, the scene says
WHAT. There is no override-precedence bug surface because the scene IS
the source of truth — no field can be "lost" between the scene switch and
the override layer.

### Why the simplification?

The multi-field format had a fundamental precedence bug:
`apply_ambient_entry` applied the scene's managed defaults first (e.g.
signal's `speed=14.0`), then tried to override with the entry's `speed=50`.
In practice the override was silently lost in some code paths, producing
confusing verbose output where `speed=12.0` (signal's default) appeared
instead of the user's `speed=50`. Live-reload compounded the bug by
rebuilding the Cloud from base config and losing the ambient overrides
entirely.

The simplification eliminates this entire class of bugs by removing
the override layer. The scene IS the spec — there's nothing to lose.

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

> "dynamic clock — it doesn't have to stay awake continuously; idle
> when the time is approaching, then a few seconds before, automatically
> wake up — so CPU usage doesn't stay high all the time when the user
> uses the ambient config"

### Instant Switch

There is no blend window. When the scheduler fires a phase entry, the
scene is applied immediately via `Cloud::apply_ambient_entry` (which
delegates to `Cloud::apply_scene_runtime_with_cfg`). The only visual
smoothing comes from the existing `transition_chars` (glyph warm-start)
and `transition_rain_style` (pool reset) — those exist for correctness
(preventing ghosting), not for cinematic blending.

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

Simplification: because ambient entries are just scene names, there's
no override layer to re-apply after a live-reload rebuild. The previous
`last_applied_ambient_entry` tracker (which re-applied overrides
after Cloud rebuild) is no longer needed — the scene IS the spec, so
firing the active phase on the next scheduler iteration is sufficient.

### CLI Flag

**None.** Ambient is config-only — there is no `--ambient` CLI flag. This
matches the archived `adaptive-custom` contract (which was also config-
only) and is consistent with cosmostrix's naming convention: time-driven
schedulers have no CLI analog (you can't "pass --ambient 12-00" at startup
because the scheduler runs continuously).

## Module Map

| Module | Responsibility |
|--------|----------------|
| `src/crystal_dragon_engine/ambient/mod.rs` | Parser, `AmbientEntry` / `AmbientSchedule` structs (`AmbientEntry` is just `{hour, minute, scene}`), `current_phase` / `next_phase` / `seconds_to_next_phase` helpers, strict validation (`validate_ambient_entries`), wall-clock helpers (`current_minute_of_day`, `current_second_of_minute`) |
| `src/crystal_dragon_engine/ambient_scheduler/mod.rs` | Dynamic idle/wake scheduler thread, `AmbientSchedulerHandle`, `spawn_ambient_scheduler`, `reload` |
| `src/cosmic_dragon_engine/cloud/scene_runtime.rs` | `Cloud::apply_ambient_entry` — delegates to `apply_scene_runtime_with_cfg`, which handles both built-in scenes (fast path) and custom scenes (looks up `[scene-custom.<name>]` block, applies `base-scene` defaults first, then the block's own overrides) |
| `src/interactive/event_loop.rs` | Spawns scheduler at startup, polls `rx` each frame, pushes reload on config change |
| `src/config/live_config.rs` | `rebuild_cloud_config` collects new schedule from config map; `apply_scene_custom_to_cloud_config` calls `scene_custom::apply_base_scene_to_cloud_config` for base-scene inheritance on live-reload |
| `src/scene_custom/mod.rs` | `rain_style_for_custom_scene` + `resolve_rain_style` + `apply_base_scene_to_cloud_config` helpers |
| `src/profile/mod.rs` | `UserProfile.base_scene` field + `apply_base_scene_to_args` inheritance layer |
| `src/config/configfile.rs` | `is_known_key` dispatch + `AMBIENT_CONFIG_KEY_HINT` constant |
| `src/config/config_hints/mod.rs` | Mis-nest detector for `scene-custom.<name>.ambient.<HH-MM>` |
| `src/testconf/mod.rs` | Strict validation of `ambient.*` entries (scene name must be built-in OR a defined `[scene-custom.<name>]` block) |
| `src/cli/app.rs` | `CloudConfig.ambient_schedule` field |

## Edge Cases

| Edge case | Handling |
|-----------|----------|
| **Empty schedule** (no `ambient.*` keys) | Scheduler thread idles (sleeps 60s, polls for new entries). Zero events fired. Existing scene/params retained. |
| **Single entry** | Fires at startup (wrap-around: the entry is treated as "yesterday's last active phase" before its boundary, and as "today's active phase" at/after its boundary). This means a single `ambient.03-17 = hacker-mode` is active ALL DAY — it's the only phase, so it carries over from yesterday via midnight wrap-around. This is correct by design (ambient is a 24-hour schedule, not a one-shot timer). If you want the scene to activate only after a specific time, use at least two entries — e.g. `ambient.03-16 = cinematic` then `ambient.03-17 = hacker-mode`. |
| **Two entries same time** | Configfile parser is `HashMap::insert` (last-writer-wins). One entry survives. |
| **DST spring-forward** (2:00 AM → 3:00 AM) | `current_minute_of_day()` returns wall-clock local time. Entries in the skipped hour (02:00–02:59) are never fired. Acceptable. |
| **DST fall-back** (2:00 AM repeat) | Entries in the repeated hour (01:00–01:59) fire twice. Acceptable — `apply_ambient_entry` is idempotent. |
| **Midnight wrap** | Handled in `AmbientSchedule::seconds_to_next_phase` — `(24*60 - now_min + next_min) * 60`. |
| **Invalid scene name** | Strict reject via `--testconf` (exit 2). Same behavior as `colors-custom` / `scene-custom`. |
| **Legacy multi-field format** | Strict reject via `--testconf` (exit 2) with a full migration message showing how to convert to `[scene-custom.<name>]` + `base-scene`. Live-reload silently drops the entry (no crash). |
| **Live-reload adds new entry** | Scheduler thread wakes (condvar), recomputes, fires current phase if changed. |
| **Live-reload removes all entries** | Scheduler goes idle. Existing scene/params retained (sticky). User can manually cycle via `x`/`X` keys. |
| **Custom scene referenced by ambient is later renamed** | `--testconf` catches this at validation time. At runtime, the ambient event is a no-op (unknown scene name → `apply_scene_runtime_with_cfg` returns current charset preset unchanged). |

## Diagnostics

Set `COSMOSTRIX_LIVE_RELOAD_DEBUG=1` in the environment to see ambient
scheduler events:

- `[live-reload-trace] ambient: reloaded schedule with N entries` — fired
  on every live-reload that changes the schedule.
- `[live-reload-trace] ambient: schedule changed (was X entries, now Y) — pushing to scheduler thread` — fired when event loop pushes the new schedule.
- `[live-reload-trace] ambient: received phase event HH:MM (scene=<name>)` — fired when the scheduler thread sends a phase event.
- `[live-reload-trace] ambient-scheduler: firing phase HH:MM (scene=<name>)` — fired inside the scheduler thread when it sends an event.

## See Also

- [Atmosphere Engine (archived)](archive/specs/ATMOSPHERE_ENGINE.md) — the
  historical design spec for the atmosphere engine subsystem, including
  the original `adaptive-custom` design that ambient replaces.
- [Atmosphere Subsystem Archival](archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md) —
  the full elimination record (file list, KEPT-vs-DELETED table, revival
  guidance).
- [Rules](RULES.md) — project conventions and CLI flag policy.
