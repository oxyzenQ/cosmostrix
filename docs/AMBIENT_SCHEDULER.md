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

### Interaction with Crystal Dragon (harmony state machine — masterclass)

When Crystal Dragon is enabled (`crystal-dragon = true` in config or
`--crystal-dragon` on CLI), it periodically drifts the color palette
based on system state (CPU load / clock).

**v50.0.0-beta.7 masterclass state machine**: when both ambient AND
Crystal Dragon are enabled, the two systems cooperate with a
**deterministic rhythm** via an internal state machine (no new config
keys — only `ambient-snapback-secs` which already exists).

**State machine fields** (internal to Cloud, not config):
- `drift_active: bool` — true while a drift is visible (waiting for snapback or self-reset)
- `drift_start: Option<Instant>` — when the current drift began
- `ambient_schedule_active: bool` — Z-master-1X: true when the ambient schedule has entries (set from `!ambient_schedule.entries.is_empty()` in `create_cloud`). When false, the drift gate skips the `user_override_since_ambient` check and the drift cycle self-resets (see "Ambient OFF" below).

**Drift gate** (`cloud/post_rain.rs`): drift fires only when ALL are true:
- `crystal_dragon` is enabled
- `drift_active == false` (no drift currently visible)
- `user_override_since_ambient == false` **OR** `ambient_schedule_active == false`

The last condition is the Z-master-1X fix (commit `c12580a`): `user_override_since_ambient` is forced to `true` at startup by `event_loop_setup.rs` (coredump fix, commit `2b0e28b`) and is only cleared by an ambient fire. When the ambient schedule is empty, no ambient fire ever happens, so the flag would otherwise stay `true` forever and permanently block crystal dragon drift. `ambient_schedule_active` is the authoritative signal — when false, the user-override check is skipped entirely.

When drift fires: sets `drift_active = true`, `drift_start = now`,
changes the palette, sets `user_override_since_ambient = true`.

**Snapback** (input.rs `try_auto_snapback`): counts idle from `drift_start`
(when drift began), NOT from `last_user_input_at`. When
`now - drift_start >= ambient-snapback-secs`, snapback reverts the
palette to ambient and clears `drift_active = false` + `drift_start = None`.
Snapback only runs when the ambient schedule is non-empty — it early-returns
on `schedule.entries.is_empty()` (see `input.rs:481`).

**Live-reload interaction** (Z-master-1X round 4, commit `<TBD>`): when a
live config reload fires while a drift is visible (`drift_active = true`),
`inherit_ecosystem_state` must NOT carry `drift_active` / `drift_start` to
the fresh Cloud. The reload's re-apply path sets
`user_override_since_ambient = false`, which disables the snapback
mechanism that would normally clear `drift_active`. With `drift_active`
inherited as `true` + snapback disabled, the drift gate `!drift_active`
would block all future drifts forever — the owner symptom "ambient
dominant, drift rare after live reload, restart fixes it." The sensor
state (`crystal_dragon_sensor`, `_control`, `_last_poll`) IS still
inherited (engine state survives), but the per-cycle drift bookkeeping
resets cleanly so the next poll can fire a fresh drift.

**Self-reset when ambient is OFF** (Z-master-1X round 2, commit `40bad33`):
when `ambient_schedule_active == false`, `try_auto_snapback` never runs
(no schedule → early-return), so without a self-reset path the first
drift would set `drift_active = true` and no mechanism would ever clear
it — permanently blocking all subsequent drifts. The self-reset in
`post_rain.rs` clears `drift_active` + `drift_start` + resets
`crystal_dragon_last_poll` when ALL of the following are true:
`drift_active == true`, `ambient_schedule_active == false`, and
`now - drift_start >= CRYSTAL_DRAGON_POLLING_SECS` (60s).

The 60s visibility window matches the polling cadence: drift is visible
for one poll cycle, then the cycle resets so the next poll can fire a
new drift. When ambient is ON, the snapback path clears `drift_active`
first (at `ambient-snapback-secs`, default 30s) and the self-reset is a
no-op — correct ordering (snapback at 30s < self-reset at 60s).

**Timeline** (with `ambient-snapback-secs = 10`, poll = 60s):

```
T=0:    ambient fires → palette=energyzen (scene X default)
T=60:   drift fires → palette=neon-green, drift_active=true, drift_start=T60
T=70:   snapback fires (70-60=10s >= 10) → revert to energyzen,
        drift_active=false, drift_start=None
T=120:  drift fires again → palette=ocean, drift_active=true, drift_start=T120
T=130:  snapback fires → revert to energyzen
T=180:  drift fires → ...
T=190:  snapback fires → revert
```

**Rhythm**: 60s ambient → 10s drift → revert → 60s ambient → 10s drift
→ revert → ... Drift is visible for exactly `ambient-snapback-secs`.

**Timeline (ambient OFF)** — Z-master-1X round 2 (commit `40bad33`):

```
T=0:    startup → palette=user/config color, drift_active=false,
        ambient_schedule_active=false
T=60:   drift fires (12% chance per poll) → palette=neon-green,
        drift_active=true, drift_start=T60
T=120:  self-reset fires (120-60=60s >= 60s POLLING_SECS) →
        drift_active=false, drift_start=None,
        crystal_dragon_last_poll=T120
T=180:  drift fires again (12% chance) → palette=ocean,
        drift_active=true, drift_start=T180
T=240:  self-reset fires → cycle repeats
```

**Rhythm (ambient OFF)**: 60s drift visible → reset → 60s drift visible
→ reset → ... No palette revert (no ambient to revert to) — the previous
drift color persists until the next drift fires. The 60s visibility
window is fixed at `CRYSTAL_DRAGON_POLLING_SECS` (not configurable) so
the cycle cadence matches the poll cadence exactly.

**Edge case: snapback >= 60**: if `ambient-snapback-secs >= 60`, the
next drift poll (at +60s) finds `drift_active` still true → drift is
**skipped**. Drift fires at +120s instead (after snapback cleared the
flag). This is by design — the user chose a long snapback, so drift
gets a long visible window and the next drift is delayed. To avoid
this, set `ambient-snapback-secs` to a value **less than 60** (e.g.
10, 30, 50) for the "60s ambient, Ns drift" rhythm, or **greater than
60** (e.g. 70, 120) for a longer drift window with skipped polls.

**Manual user override**: pressing `c`/`C`/`x` sets
`user_override_since_ambient = true`, which blocks drift from firing
until snapback clears it. This is the existing behavior — manual
overrides always take priority over automatic drift.

**Why no new config keys**: the state machine uses only internal Cloud
fields (`drift_active`, `drift_start`) + the existing
`ambient-snapback-secs` config key. No new config keys, no new CLI
flags, no new persistent settings. The rhythm is fully controlled by
the existing `ambient-snapback-secs` value.

**Note**: climate drift (luminance/saturation/hue modulation, NOT
palette scheme replacement) always runs via `color_ecosystem.tick()`
regardless of any state. Only palette-scheme drift uses the state machine.

### User overrides + 30-second auto-snapback (IMPORTANT)

When the user presses `x` (scene cycle), `c` (color cycle), or `s`
(charset cycle) while an ambient phase is active, the override takes
effect immediately — but it is **temporary**. After a configurable idle
delay (default **30 seconds**), the event loop automatically re-applies
the current ambient phase, undoing the user's change. This is the
intended auto-snapback behavior (see `docs/archive/audits/AMBIENT_SCHEDULER_AUDIT.md`
§2.2 for the design rationale).

The timer resets on **every** keypress (not just `x`/`c`/`s`), so an
active user cycling through scenes will never be interrupted. The
snapback only fires after the user stops pressing keys for the
configured delay. The default is `AUTO_SNAPBACK_DELAY_SECS = 30.0` in
`src/central_control_dragon_power/mod.rs:198`.

#### Config-tunable snapback delay (v50.0.0-beta.7)

The snapback delay is now configurable via the `ambient-snapback-secs`
config key (config-only, no CLI flag):

```toml
# Default: 30 seconds (when unset)
ambient-snapback-secs = 30

# Effectively disable snapback (24h = never fires in practice)
ambient-snapback-secs = 86400

# Instant snapback (overrides revert immediately on next frame)
ambient-snapback-secs = 0

# Long grace period (2 minutes)
ambient-snapback-secs = 120
```

**Range**: `0.0..=86400.0` (0 = instant, 86400 = 24h). Values outside
this range are rejected at startup and on live-reload (fall back to
default 30s). The key is live-reloadable — editing it in config.toml
takes effect on the next frame without restart.

**Why this design**: the default 30s preserves the original v35 behavior
(no breaking change). Users who want longer grace periods (e.g. 120s)
or want to effectively disable snapback (86400s) can tune it without
code changes. This closes the deferred enhancement listed in
`docs/archive/audits/AMBIENT_SCHEDULER_AUDIT.md` §3.

**To make a permanent change**, edit `config.toml` (live-reload will
apply immediately) — the snapback only reverts user keypress overrides,
not config edits.

#### Verbose + final runtime state visibility (v50.0.0-beta.7 LTS)

The `--verbose` / `-v` flag now reports the **effective** ambient
configuration in two places so the user can verify what is actually in
effect, not just what was set at startup:

**1. Startup `── Ambient ──` section** (printed before the rain loop
starts):

```text
  ── Ambient ──
[verbose] [HH:MM] schedule:      3 entries [00-00→monolith, 12-00→calm, 18-00→neon]
[verbose] [HH:MM] ambient_snapback_secs: 10.0s (from config — drift visible for 10.0s before ambient reverts)
[verbose] [HH:MM] auto_snapback: 30.0s idle threshold, 10.0s snapback delay (user overrides via 'c'/'C'/'x'/'s' revert after 10.0s)
```

When `ambient-snapback-secs` is **unset** in config, the line reads:

```text
[verbose] [HH:MM] ambient_snapback_secs: 30.0s (default (unset in config) — drift visible for 30.0s before ambient reverts)
```

Before this LTS fix, the verbose output lied — it always printed the
constant `AUTO_SNAPBACK_DELAY_SECS` (30.0s) even when the user had set
`ambient-snapback-secs = 10` in config. The runtime correctly used 10s
for snapback timing, but verbose mis-reported 30s. Owner found this
while debugging crystal-dragon drift visibility.

**2. Post-exit `final runtime state` section** (printed after the rain
loop exits, always — even when no live-reload field changed):

```text
[verbose] [HH:MM] final runtime state
[verbose] [HH:MM]   exit_time:     YYYY-MM-DD HH:MM:SSZ | duration: Xm Ys
... (changed live-reload fields, if any) ...
[verbose] [HH:MM]   ambient_snapback_secs: 10.0s (config)
[verbose] [HH:MM]   ambient_entries:    3
[verbose] [HH:MM]   ambient_diag: startup=1 rx=0 reapply=0 snapback=0 ...
```

The `ambient_snapback_secs:` + `ambient_entries:` lines are
**always-printed** (not gated by change) so the user can confirm what
was actually in effect at session end. The `(was Xs)` suffix appears
only when a live-reload edit changed the value mid-session:

```text
[verbose] [HH:MM]   ambient_snapback_secs: 10.0s (config) (was 30.0s)
[verbose] [HH:MM]   ambient_entries:    3 (was 0)
```

This closes the LTS audit gap: previously, live-reload edits to
`ambient-snapback-secs` were silently lost on exit — there was no way
to verify the actual snapback delay in effect when the session ended.

#### The cinematic/monolith shared-color gotcha

If the ambient phase is `monolith` (default color: `neon-purple`) and
the user presses `x`, the first scene in the cycle is `cinematic` —
which **also defaults to `neon-purple`**. So the first `x` press may
produce **no visible color change** (only the rain style/charset changes
underneath). Press `x` again to cycle to `matrix` (green) or another
scene with a distinct color. This is not a bug — it's a consequence of
two scenes sharing the same default palette.

#### Summary of override behavior

| User action | Effect | Duration | After snapback delay |
|-------------|--------|----------|----------------------|
| Press `x` | Scene cycles to next | Immediate | Reverts to ambient phase |
| Press `c` | Color cycles to next | Immediate | Reverts to ambient phase |
| Press `s` | Charset cycles to next | Immediate | Reverts to ambient phase |
| Edit `config.toml` | Live-reload applies | Permanent | No snapback |
| Press any other key | Timer resets | N/A | Delay clock restarts |

### Live Reload

Editing `ambient.*` keys in `config.toml` triggers an immediate re-parse
via `collect_ambient_schedule`. The new schedule replaces the old one
atomically (mutex swap), and the scheduler thread is woken via condvar to
recompute the next phase boundary. If the new schedule's currently-active
phase differs from the previously-applied one, the thread fires it on the
next loop iteration (no boundary wait).

Simplification: because ambient entries are just scene names, there's
no per-field override layer — the scene IS the spec. The runtime still
keeps the `last_applied_ambient_entry` tracker (event-loop locals): it
dedups duplicate rx fires, arms the auto-snapback after a user override,
and lets the rebuild path re-apply the ambient phase to a fresh Cloud
(`event_loop_config_rebuild.rs` re-apply block) so an unrelated config
edit does not visually kick ambient off.

### Schedule Removal = Overlay Lift (v51.2, owner contract 2026-09-01)

Removing ALL `ambient.*` keys at runtime (commenting them out) lifts the
ambient overlay — the same CLI-locked fallback contract the plain `scene`
key follows (see `docs/LIVE_RELOAD_BEHAVIOR.md` section 14):

- If the current scene is **ambient-owned** (the last visual change came
  from an ambient apply — `user_override_since_ambient == false` and the
  live scene matches the last applied entry), the scene family REVERTS to
  the locked startup resolution (CLI > config > default). No exit, no
  rerun. Two cooperating paths enforce this, whichever sees the emptied
  file first:
  - the ground-truth nuke in `event_loop_ambient.rs`
    (`revert_ambient_owned_scene`, driven by the per-frame file re-read
    while ambient is actively applied), and
  - the live-reload rebuild
    (`resolve_scene_base_with_ambient` upgrading `SyncRuntime` to
    `RestoreLocked` via `ambient_removed_between_maps`).
- If the user overrode with a shortkey (`x`/`c`/`s`) after the last
  ambient apply, their scene SURVIVES the removal (shortkeys are the
  runtime top priority).
- v51.2 honesty fix: the ground-truth nuke no longer fakes
  `user_override_since_ambient = true` when it clears ambient state —
  the flag keeps reporting the true owner so a later rebuild can still
  resolve the overlay lift correctly.

Commenting the entries back in recovers exactly like startup: the
scheduler refires the current phase (AB-09 identity reset on empty) and
ambient takes over again on the next poll.

### CLI Flag

**None.** Ambient is config-only — there is no `--ambient` CLI flag. This
matches the archived `adaptive-custom` contract (which was also config-
only) and is consistent with cosmostrix's naming convention: time-driven
schedulers have no CLI analog (you can't "pass --ambient 12-00" at startup
because the scheduler runs continuously).

## Module Map

| Module | Responsibility |
|--------|----------------|
| `src/engine/crystal_dragon_engine/ambient/mod.rs` | Parser, `AmbientEntry` / `AmbientSchedule` structs (`AmbientEntry` is just `{hour, minute, scene}`), `current_phase` / `next_phase` / `seconds_to_next_phase` helpers, strict validation (`validate_ambient_entries`), wall-clock helpers (`current_minute_of_day`, `current_second_of_minute`) |
| `src/engine/crystal_dragon_engine/ambient_scheduler/mod.rs` | Dynamic idle/wake scheduler thread, `AmbientSchedulerHandle`, `spawn_ambient_scheduler`, `reload` |
| `src/engine/cosmic_dragon_engine/cloud/scene_runtime.rs` | `Cloud::apply_ambient_entry` — delegates to `apply_scene_runtime_with_cfg`, which handles both built-in scenes (fast path) and custom scenes (looks up `[scene-custom.<name>]` block, applies `base-scene` defaults first, then the block's own overrides) |
| `src/interactive/event_loop.rs` | Spawns scheduler at startup, polls `rx` each frame, pushes reload on config change |
| `src/config/live_config/mod.rs` | `rebuild_cloud_config` collects new schedule from config map; `apply_scene_custom_to_cloud_config` calls `scene_custom::apply_base_scene_to_cloud_config` for base-scene inheritance on live-reload |
| `src/scene_custom/mod.rs` | `UserProfile` struct (with `base_scene` field), `apply_base_scene_to_args` inheritance layer, `rain_style_for_custom_scene` + `resolve_rain_style` + `apply_base_scene_to_cloud_config` helpers |
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
| **DST spring-forward** (2:00 AM -> 3:00 AM) | `current_minute_of_day()` returns wall-clock local time. Entries in the skipped hour (02:00–02:59) are never fired. Acceptable. |
| **DST fall-back** (2:00 AM repeat) | Entries in the repeated hour (01:00–01:59) fire twice. Acceptable — `apply_ambient_entry` is idempotent. |
| **Midnight wrap** | Handled in `AmbientSchedule::seconds_to_next_phase` — `(24*60 - now_min + next_min) * 60`. |
| **Invalid scene name** | Strict reject via `--testconf` (exit 2). Same behavior as `colors-custom` / `scene-custom`. |
| **Legacy multi-field format** | Strict reject via `--testconf` (exit 2) with a full migration message showing how to convert to `[scene-custom.<name>]` + `base-scene`. Live-reload silently drops the entry (no crash). |
| **Live-reload adds new entry** | Scheduler thread wakes (condvar), recomputes, fires current phase if changed. |
| **Live-reload removes all entries** | Scheduler goes idle. Existing scene/params retained (sticky). User can manually cycle via `x`/`X` keys. |
| **Custom scene referenced by ambient is later renamed** | `--testconf` catches this at validation time. At runtime, the ambient event is a no-op (unknown scene name -> `apply_scene_runtime_with_cfg` returns current charset preset unchanged). |

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
