<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Ambient Phase Scheduler

Config-driven time-of-day scene switching — replaces the archived
`adaptive-custom` subsystem (eliminated with the atmosphere engine at commit
`07b44b5`) with a simpler contract: **config-only** (no CLI flag),
**instant switch** (no blend window), and a **dynamic idle/wake scheduler
thread** (zero CPU between phase boundaries).

## Usage Quick Guide (read this first — verified 2026-09-02)

New to `ambient` + `crystal-dragon` + `power-dragon`? These are the
facts the rest of this document assumes, stated without engine jargon:

| Question | Answer |
|---|---|
| How often does crystal-dragon act? | Sensor poll every **`crystal-dragon-secs`** (default **60s**, range 0.0..=86400.0 — CLI `--crystal-dragon-secs`, config key, live-reload); each poll has a **~12% drift chance** (about one drift per 5 minutes on average at 60s — organic, not periodic). |
| What is the ambient snapback? | The ambient phase re-asserting itself after something else took over (a crystal-dragon drift, or your manual `c`/`C`/`x`/`X`/`s`/`S` shortkey). |
| Default snapback delay? | **30s** (`ambient-snapback-secs`, range 0.0..=86400.0; 0 = instant, 86400 = effectively off). |
| Does a snapback >= the poll interval still fire? | **YES.** Verified live: a 90s snapback fired at ~90s against the 60s default poll. The timer has no upper-bound bug. A long value only stretches the rhythm (see the next row). |
| What actually changes with snapback >= polling? | The drift palette **holds** the ambient palette for the whole window and **no new drift can fire** during it — the system looks "stuck on one color" until the snapback lands. At 86400 that is ~24h. |
| Recommended values when combining? | Keep `ambient-snapback-secs` **under `crystal-dragon-secs`** (<= polling-10s for margin) so each drift reverts before the next poll and the two systems take clean turns. Both knobs are live-reload-able — tune the rhythm online while watching the HUD (v80.0.0-alpha.1). |
| I set `density = 0.90` but the HUD shows ~0.65 — bug? | No. `power-dragon` (default on) throttles the *effective* density under pressure; the HUD `dsty:` line shows the effective value. Set `power-dragon = false` (or `--power-dragon false`) for the exact fixed value. |
| Do I need both dragons? | No. If you do not want drift-vs-ambient interplay, turn either one off — never required together. |

The rest of this document is the engine-level reference for those facts.

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
color = "energy-zen"           # built-in color name OR colors-custom = "<palette>"
charset = "retro"              # built-in preset    OR charset-custom = "<set>"
fps = 60                       # v80.0.0-beta.2: the ambient scene owns fps too
speed = "50"
density = "0.65"
glitch-level = "subtle"

# Top-level — outside any [section] block:
ambient.15-00 = afternoon
```

v80.0.0-beta.2 (S-master-LOGIC-3): the block is a COMPLETE six-dimension
profile — all fields required (incomplete blocks are rejected by
`--testconf`, startup, and live-reload). `base-scene` inheritance is
removed; custom scenes always render glyph rain.

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
state (`crystal_dragon_sensor`, `_last_poll`) IS still inherited (engine
state survives), but the per-cycle drift bookkeeping resets cleanly so
the next poll can fire a fresh drift. v80.0.0-alpha.1:
`crystal_dragon_control` is also no longer inherited — the fresh Cloud's
control is config-derived (create_cloud applies the live-reloaded
`crystal-dragon-secs`), so carrying the old control would pin the poll
cadence to the pre-edit value.

**Self-reset when ambient is OFF** (Z-master-1X round 2, commit `40bad33`):
when `ambient_schedule_active == false`, `try_auto_snapback` never runs
(no schedule → early-return), so without a self-reset path the first
drift would set `drift_active = true` and no mechanism would ever clear
it — permanently blocking all subsequent drifts. The self-reset in
`post_rain.rs` clears `drift_active` + `drift_start` + resets
`crystal_dragon_last_poll` when ALL of the following are true:
`drift_active == true`, `ambient_schedule_active == false`, and
`now - drift_start >= crystal_dragon_control.polling_secs` (the effective
`crystal-dragon-secs`, default 60s — v80.0.0-alpha.1: the window follows
the CONFIGURED cadence, not the constant).

The visibility window equals the polling cadence: drift is visible
for one poll cycle, then the cycle resets so the next poll can fire a
new drift. When ambient is ON, the snapback path clears `drift_active`
first (at `ambient-snapback-secs`, default 30s) and the self-reset is a
no-op — correct ordering (snapback at 30s < self-reset at the poll cycle,
in the default harmony configuration).

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

**Rhythm (ambient OFF)**: one poll cycle of drift visibility → reset →
next cycle → ... No palette revert (no ambient to revert to) — the previous
drift color persists until the next drift fires. v80.0.0-alpha.1: the
visibility window follows `crystal-dragon-secs` (default 60s) so the
cycle cadence always matches the poll cadence exactly — tune the knob
and the whole rhythm follows.

**Edge case: snapback >= polling**: if `ambient-snapback-secs >=
crystal-dragon-secs`, the snapback **still fires** — at exactly
`ambient-snapback-secs` after the drift began (verified live 2026-09-02:
a 90s snapback fired at ~90s against the 60s default poll;
`ambient_diag: snapback=1`, final scene reverted to the ambient phase).
There is no upper-bound bug and no "collision that starves the timer".
What actually changes is the RHYTHM, in two ways:

1. The drift palette holds the ambient palette for the whole window —
   with 86400 (the documented "effectively disabled" value) the very
   first drift holds the ambient palette for ~24 hours.
2. No new drift can fire during the window (the `!drift_active` gate).
   The next drift poll (at +`crystal-dragon-secs`) finds `drift_active`
   still true and is skipped; drift becomes eligible again only after
   the snapback clears the flag (next drift at snapback + ~poll).

This is by design — the user chose a long snapback, so drift gets a
long visible window and the next drift is delayed. To avoid this,
set `ambient-snapback-secs` to a value **less than
`crystal-dragon-secs`** (<= polling-10s for margin) for the "Ns ambient,
Ms drift" rhythm, or accept a **longer** drift window with skipped polls.
The "snapback never triggers at >= 60s" reading is a myth — do not
document it anywhere; this section is the correction. With
`crystal-dragon-secs` now tunable, BOTH sides of the inequality are
yours to place: raise the poll interval (e.g. 120) instead of lowering
the snapback, or shorten the poll (e.g. 30) for a faster rhythm —
keeping the 60s minimum-dwell floor in mind (palette flips never
faster than one per minute).

**Manual user override**: pressing `c`/`C`/`x` sets
`user_override_since_ambient = true`, which blocks drift from firing
until snapback clears it. This is the existing behavior — manual
overrides always take priority over automatic drift.

**Config surface** (v80.0.0-alpha.1): the state machine uses internal Cloud
fields (`drift_active`, `drift_start`) + the two user-facing timing knobs:
`ambient-snapback-secs` (config-only) and `crystal-dragon-secs`
(CLI `--crystal-dragon-secs` + config key, both live-reload-able). No other
persistent settings — the rhythm is fully controlled by those two values.

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
was actually in effect at session end. v80.0.0-beta.2
(S-master-LOGIC-1): the change-tracked field list is complete —
`fps` and `glitch_level` join the tracked set (both are
ambient-owned), so an ambient phase applying a scene with fps 12 /
glitch none shows `fps: 12.0 (was 60.0)` /
`glitch_level: None (was Subtle)` at exit. The `(was Xs)` suffix appears
only when a live-reload edit (or ambient apply) changed the value
mid-session:

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

**Re-apply deferral parity (v80.0.0-beta.2 S-master-HUNT)**: the rebuild
re-apply is gated on the PRE-rebuild `user_override_since_ambient`. When
the CLI deferral is still running (CLI flags present, the ambient has not
applied yet) or the user overrode with a shortkey, a config edit DEFERS
the re-application — the tracker stays armed and
`try_auto_snapback` applies the entry after `ambient-snapback-secs` of
idle, exactly like the rx-event path. Without the gate, any config edit
during the deferral window applied the ambient scene instantly (owner
bug 1: the CLI `--scene` fallback looked broken and timing-dependent).
Ambient-owned state (an entry already applied, no user override since)
still re-asserts on every rebuild — a config edit never sets
`user_override_since_ambient`, so config-vs-ambient precedence is
unchanged.

### Schedule Removal = Overlay Lift (v80.0.0-beta.1, owner contract 2026-09-01)

Removing ALL `ambient.*` keys at runtime (commenting them out) lifts the
ambient overlay — the same CLI-locked fallback contract the plain `scene`
key follows (see `docs/LIVE_RELOAD_BEHAVIOR.md` section 14):

- If the current scene is **ambient-owned** (the last visual change came
  from an ambient apply — `user_override_since_ambient == false` and the
  live scene matches the last applied entry), the scene family REVERTS to
  the locked startup resolution (CLI > config > default) — VERBATIM
  (v80.0.0-beta.2 S-master-HUNT: the cloud-level revert copies the
  startup snapshot's VALUES — scene, rain style, palette/scheme,
  charset, speed, density, glitch — instead of re-deriving them from the
  scene definition, which re-applied a scene-custom block layer over
  CLI-shadowed locks like `-c test -C test`). No exit, no rerun. Two cooperating paths enforce this, whichever sees the emptied
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
- v80.0.0-beta.1 honesty fix: the ground-truth nuke no longer fakes
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
| `src/engine/cosmic_dragon_engine/cloud/scene_runtime.rs` | `Cloud::apply_ambient_entry` — delegates to `apply_scene_runtime_with_cfg`, which handles both built-in scenes (fast path) and custom scenes (looks up the `[scene-custom.<name>]` block and applies its complete self-contained field layer — v80.0.0-beta.2: no `base-scene` inheritance, glyph rain always) |
| `src/interactive/event_loop.rs` | Spawns scheduler at startup, polls `rx` each frame, pushes reload on config change |
| `src/config/live_config/mod.rs` | `rebuild_cloud_config` collects new schedule from config map; `apply_scene_custom_to_cloud_config` re-applies the (complete) scene-custom field layer on live-reload — v80.0.0-beta.2: config wins over the locked CLI value at runtime (no cli_explicit gates) |
| `src/scene_custom/mod.rs` | `UserProfile` struct (six scene-family dimensions), `ambient_scene_fps` (built-in default or block field), `validate_scene_custom_completeness` (all six required), `resolve_rain_style` (custom scenes are always Glyph) |
| `src/interactive/event_loop_ambient.rs` | `poll_ambient_events` — rx-event apply, snapback, overlay-lift revert; v80.0.0-beta.2: returns the ambient-owned fps intent the event loop applies to the power manager + HUD (custom-scene `fps` fields and built-in scene fps defaults take effect on ambient fires; the overlay-lift revert restores the locked startup fps) |
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
