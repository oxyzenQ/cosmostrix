<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Ambient Scheduler Engine — Deep Audit (v35.0.0-alpha.1)

**Repo**: cosmostrix @ v35.0.0-alpha.1
**Audit scope**: `src/ambient.rs`, `src/ambient_scheduler.rs`, `src/cloud/scene_runtime.rs::apply_ambient_entry`, `src/interactive/event_loop.rs` (ambient event path), `src/interactive/input.rs` (shortkey x/c/s), `src/cloud/rain.rs` (auto-color-drift path), `src/cloud/ecosystem.rs::ColorEcosystem::tick`.
**Trigger**: Owner report of confused behavior — after pressing `x`/`c`/`s` at runtime, the ambient scheduler no longer re-asserts the configured scene (e.g. `ambient.22-10 = aurora` does not come back at 22:10 the next day). Owner also asked whether `--auto-color-drift` + ambient can conflict, and how to harmonize them.

---

## 1. Findings — three concrete defects + one design gap

### 1.1 Defect A — Day-boundary refire is suppressed by entry-level dedup

**Symptom (owner's report)**: With a single-entry schedule
`ambient.22-10 = aurora`:

1. At 22:10 today, scheduler fires `aurora`. `last_applied = <22:10, aurora>`.
2. Owner presses `x` at 22:15 to change scene to `matrix`. Cloud is now in `matrix`.
3. At 22:10 the next day, scheduler wakes. `current_phase = <22:10, aurora>`. `last_applied = <22:10, aurora>`. **Equal → no fire.**
4. Owner expected `aurora` to be re-applied. **Bug.**

**Root cause** (`src/ambient_scheduler.rs:217-231`):

```rust
if let Some(entry) = &current_entry {
    if last_applied.as_ref() != Some(entry) {
        // ... fire ...
        last_applied = Some(entry.clone());
    }
}
```

The dedup compares the full entry (hour, minute, scene). It was added in v30.3
to handle a different bug (scene-name change for an existing slot not triggering
refire). The dedup is correct for *same-day* refire prevention, but it
incorrectly suppresses the legitimate *next-day* refire of a single-entry
schedule.

For multi-entry schedules (`ambient.22-10 = aurora` + `ambient.06-00 = morning`),
the bug is masked: at 06:00 the next day, `current_phase = <06:00, morning>`,
`last_applied = <22:10, aurora>`, different → fire. Then at 22:10 the next day,
`current_phase = <22:10, aurora>`, `last_applied = <06:00, morning>`, different
→ fire. So multi-entry schedules work, but with up to 24h delay before ambient
retakes control after a user override.

**Fix**: Track `last_fired_yday: i32` (day-of-year, -1 = never fired) in the
scheduler thread. On each wake, compute the current `yday` from localtime. If
`yday != last_fired_yday` AND `current_phase.minutes_of_day() <= now_min` (we
are at or past today's boundary), refire even if `entry == last_applied`. This
restores the per-day, per-entry fire semantics without breaking the same-day
dedup. See §2.1 for the patch.

### 1.2 Defect B — No "snap back to ambient" shortkey

**Symptom**: After pressing `x`/`c`/`s`, the owner wants to return to the
ambient-configured scene *now*, without waiting for the next boundary (which
for a single-entry schedule is up to 24h away).

**Root cause**: There is no shortkey that re-applies the current ambient phase
on demand. The only ways to return to ambient are:

1. Wait for the next boundary fire (up to 24h for single-entry schedules).
2. Save `config.toml` to trigger live-reload, which re-applies the last ambient
   entry (but only if the schedule's currently-active phase differs from the
   last-applied one — same dedup caveat).
3. Restart cosmostrix (cold-start ambient apply).

**Fix**: Add an `a` shortkey ("ambient snap-back") that looks up the current
phase from `last_ambient_schedule` and applies it immediately via
`cloud.apply_ambient_entry`. See §2.2 for the patch.

### 1.3 Defect C — Auto-color-drift palette drift can override ambient's palette

**Symptom**: With `--auto-color-drift` enabled and `ambient.22-10 = aurora`
active, the autonomous palette drift (in `cloud/ecosystem.rs::tick`) randomly
picks a new palette from the current feeling's family every ~3% tick (gated by
a cooldown). This overrides `aurora`'s palette until the next ambient boundary
re-asserts it. The owner experiences this as "ambient doesn't stick" or
"colors keep fighting my schedule".

**Root cause** (`src/cloud/rain.rs:905-909`):

```rust
if self.auto_color_drift && !self.custom_palette_active {
    if let Some(new_scheme) = maybe_drift {
        self.set_color_scheme(new_scheme);
    }
}
```

Palette drift is suppressed only for `--colors-custom` (the
`custom_palette_active` guard added in v30 Bug #4). It is NOT suppressed for
ambient-asserted palettes. So ambient and auto-drift fight over the base
palette.

**Note**: *Climate* drift (luminance / saturation / hue multipliers in
`ColorEcosystem`) is NOT affected — it layers on top of the base palette
regardless of who set it. The conflict is only about *palette scheme
replacement* (which `ColorScheme` enum variant is active).

**Fix**: Add `ambient_palette_locked: bool` to `Cloud`. Set it `true` when
ambient fires (scheduler or `a` key). Set it `false` when the user manually
overrides (presses `c`/`C` to cycle color, or `x` to change scene which may
change color). Gate the palette drift on `!ambient_palette_locked`. Climate
drift continues — this is the harmony: ambient specifies *what* palette,
auto-drift specifies *how* it varies visually. See §2.3 for the patch.

### 1.4 Design gap — Event loop dedup can falsely skip legitimate refire

**Symptom**: After fixing Defect A (scheduler refires same entry on
day-boundary), the event loop's dedup (`event_loop.rs:467`) will skip the
legitimate refire because `last_applied_ambient_entry == Some(entry)`.

**Root cause** (`src/interactive/event_loop.rs:465-471`):

```rust
if last_applied_ambient_entry.as_ref() == Some(&entry) {
    // skip duplicate
}
```

This dedup was added for the cold-start path: `apply_startup_ambient` applies
the current phase synchronously, then the scheduler thread fires the same
phase via mpsc — the event loop must skip that duplicate. But the same dedup
also incorrectly skips the day-boundary refire from Defect A's fix.

**Fix**: Add `user_override_since_ambient: bool` to `Cloud`. Set it `true`
when the user presses `x`/`c`/`s`/`C`/`S` (manual override) OR when
auto-color-drift picks a new palette. Set it `false` when ambient fires
(scheduler or `a` key). Change the dedup to:

```rust
if last_applied_ambient_entry.as_ref() == Some(&entry)
    && !cloud.user_override_since_ambient
{
    // skip duplicate
}
```

This way:

- Cold-start duplicate (no user override): skip. ✓ (existing behavior)
- Day-boundary refire after user override: `user_override = true` → don't skip → apply. ✓
- `a` key snap-back: clears `user_override`, applies entry, sets `last_applied`. Subsequent scheduler fires of the same entry: `user_override = false` → skip (correct — `a` key just applied it). ✓

See §2.4 for the patch.

---

## 2. Fix design — surgical patches

### 2.1 Patch A — Day-boundary refire in `src/ambient_scheduler.rs`

Add a `current_yday()` helper in `src/ambient.rs` (mirrors
`current_minute_of_day`).

In `scheduler_loop`, add `let mut last_fired_yday: i32 = -1;` before the loop.
After the existing `if last_applied.as_ref() != Some(entry)` fire check, add a
day-boundary refire check:

```rust
// Day-boundary refire: if we're in a new day AND the current phase's
// boundary has been crossed today, refire even if entry == last_applied.
// This handles single-entry schedules where the same entry is "current"
// across multiple days — without this, a user who presses 'x' after 22:10
// would never see aurora re-asserted at 22:10 the next day.
let today_yday = crate::ambient::current_yday();
if today_yday != last_fired_yday {
    if let Some(entry) = &current_entry {
        if entry.minutes_of_day() <= now_min {
            // Day changed AND we're past today's boundary — refire.
            // (Only fires once per day: after firing, last_fired_yday = today_yday.)
            if last_applied.as_ref() == Some(entry) {
                // Same entry, new day — this is the refire case.
                if tx.send(entry.clone()).is_err() {
                    return;
                }
                // Don't update last_applied here — it's already Some(entry).
            }
            // else: the existing != check above already fired it.
            last_fired_yday = today_yday;
        }
    } else {
        // No current phase — still mark today as "seen" so we don't loop.
        last_fired_yday = today_yday;
    }
}
```

### 2.2 Patch B — `a` shortkey in `src/interactive/input.rs`

Add a new function `handle_ambient_snapback` (separate from
`handle_keybinding` to avoid bloating its parameter list):

```rust
/// Apply the current ambient phase to the cloud immediately.
///
/// Called when the user presses 'a' (ambient snap-back). Returns true if
/// an entry was applied (caller should redraw), false if no schedule is
/// active or no current phase exists.
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_ambient_snapback(
    cloud: &mut Cloud,
    charset_preset: &mut String,
    scene_name: &mut String,
    scene_generation: &mut u64,
    last_applied_ambient_entry: &mut Option<crate::ambient::AmbientEntry>,
    schedule: &crate::ambient::AmbientSchedule,
    cfg_map: &std::collections::HashMap<String, String>,
    user_ranges: &[(char, char)],
    def_ascii: bool,
) -> bool {
    let now_min = crate::ambient::current_minute_of_day();
    let Some(entry) = schedule.current_phase(now_min).cloned() else {
        return false; // no active phase — silent no-op
    };
    *charset_preset = cloud.apply_ambient_entry(
        &entry, charset_preset, user_ranges, def_ascii, cfg_map,
    );
    *last_applied_ambient_entry = Some(entry.clone());
    *scene_name = entry.scene.clone();
    *scene_generation = scene_generation.wrapping_add(1);
    cloud.user_override_since_ambient = false;
    cloud.ambient_palette_locked = true;
    true
}
```

In `handle_keybinding`, add the override flags to `x`/`c`/`s`/`C`/`S` arms:

```rust
(KeyCode::Char('c'), _) => {
    let next = cycle_color_scheme(cloud.color_scheme(), 1);
    cloud.set_color_scheme(next);
    cloud.user_override_since_ambient = true;
    cloud.ambient_palette_locked = false;
}
// (same for 'C', 's', 'S', 'x' — see patch)
```

In `event_loop.rs`, add a 4-line check before `handle_keybinding`:

```rust
if k.code == crossterm::event::KeyCode::Char('a') {
    let cfg_map = last_applied_cfg_map.clone().unwrap_or_default();
    if super::input::handle_ambient_snapback(
        &mut cloud, &mut charset_preset, &mut scene_name, &mut scene_generation,
        &mut last_applied_ambient_entry, &last_ambient_schedule, &cfg_map,
        &user_ranges, def_ascii,
    ) {
        term.set_color_cache(ColorCache::new(&cloud.palette));
        frame = Frame::new(w, h, cloud.palette.bg);
        super::fill_terminal_bg(cloud.palette.bg);
    }
} else {
    let redraw_needed = handle_keybinding(/* ... */);
    // ...
}
```

### 2.3 Patch C — Gate palette drift on `!ambient_palette_locked`

In `src/cloud/mod.rs`, add two fields to `Cloud`:

```rust
/// v35: true when ambient scheduler has asserted a palette. Suppresses
/// auto-color-drift palette replacement (climate drift still runs).
pub(crate) ambient_palette_locked: bool,
/// v35: true when user has manually overridden scene/color/charset since
/// the last ambient fire. Cleared by ambient fire (scheduler or 'a' key).
/// Used by the event loop's ambient-event dedup to avoid falsely skipping
/// legitimate day-boundary refires.
pub(crate) user_override_since_ambient: bool,
```

In `src/cloud/rain.rs:905`:

```rust
if self.auto_color_drift
    && !self.custom_palette_active
    && !self.ambient_palette_locked
{
    if let Some(new_scheme) = maybe_drift {
        self.set_color_scheme(new_scheme);
        self.user_override_since_ambient = true;
    }
}
```

### 2.4 Patch D — Event loop dedup + post-apply flag updates

In `src/interactive/event_loop.rs:465-491`, change the dedup and add
post-apply flag updates:

```rust
if let Some(entry) = last_ambient_entry {
    if last_applied_ambient_entry.as_ref() == Some(&entry)
        && !cloud.user_override_since_ambient
    {
        // skip duplicate (cold-start or rebuild re-apply)
    } else {
        let cfg_map = last_applied_cfg_map.clone().unwrap_or_default();
        charset_preset = cloud.apply_ambient_entry(/* ... */);
        last_applied_ambient_entry = Some(entry.clone());
        scene_name = entry.scene.clone();
        scene_generation = scene_generation.wrapping_add(1);
        cloud.user_override_since_ambient = false;  // NEW
        cloud.ambient_palette_locked = true;        // NEW
        term.set_color_cache(ColorCache::new(&cloud.palette));
        frame = Frame::new(w, h, cloud.palette.bg);
        super::fill_terminal_bg(cloud.palette.bg);
    }
}
```

Also apply the same two flag updates in the startup ambient apply path and the
live-reload re-apply path (lines 240-268 and 418-448).

---

## 3. Harmony / synergy model

After the four patches, the interaction model is:

| Event | `user_override_since_ambient` | `ambient_palette_locked` | Effect |
|---|---|---|---|
| Cold start | false | false | `apply_startup_ambient` applies current phase, sets `ambient_palette_locked = true` |
| Scheduler fires (boundary) | false | true | Dedup skips if entry unchanged; otherwise applies, sets `ambient_palette_locked = true`, `user_override = false` |
| User presses `x` | **true** | **false** | Scene changes; auto-drift palette drift UNLOCKS; next ambient boundary will refire (no dedup skip) |
| User presses `c` | **true** | **false** | Color changes; auto-drift palette drift UNLOCKS |
| User presses `s` | **true** | (unchanged) | Charset changes; auto-drift palette drift state unchanged |
| User presses `a` | **false** | **true** | Current ambient phase re-applied immediately; auto-drift palette drift LOCKS |
| Auto-drift picks new palette | **true** | (unchanged) | Palette changes; next ambient boundary will refire (no dedup skip) |
| Auto-drift climate tick (no palette change) | (unchanged) | (unchanged) | Luminance/saturation/hue drift continues regardless of lock |

**Net behavior**:

- **Ambient specifies the WHAT** (which palette / scene / charset).
- **Auto-drift specifies the HOW** (climate variation: luminance, saturation, hue drift on top of the base palette).
- **User override is temporary**: ambient re-asserts at the next boundary (or via `a` key for immediate return).
- **No fighting**: when ambient is active, auto-drift's palette replacement is suppressed — only climate drift continues. When the user overrides, auto-drift's palette replacement is re-enabled (because the user took ownership, ambient will re-assert at next boundary anyway).

This is the "synergy" the owner asked about: ambient and auto-drift no longer fight; they layer.

---

## 4. Test plan

1. **`src/ambient_scheduler.rs` unit tests**:
   - `day_boundary_refire_single_entry`: synthetic clock, single entry, simulate day rollover, assert refire.
   - `day_boundary_no_refire_within_same_day`: synthetic clock, same day, assert no spurious refire.

2. **`src/cloud/tests/tests_color_stability.rs` (or new file)**:
   - `palette_drift_suppressed_when_ambient_locked`: set `ambient_palette_locked = true`, run many ticks, assert `set_color_scheme` is not called by drift.
   - `palette_drift_unlocked_when_user_overrides`: set `ambient_palette_locked = true`, simulate `c` key (clears lock), run ticks, assert drift can pick a new palette.
   - `climate_drift_continues_when_ambient_locked`: set `ambient_palette_locked = true`, run ticks, assert `luminance_climate` / `saturation_climate` / `hue_drift` evolve.

3. **`src/interactive/tests.rs`**:
   - `a_key_applies_current_ambient_phase`: set up schedule, press `a`, assert cloud's scene matches the schedule's current phase.
   - `a_key_no_op_with_empty_schedule`: empty schedule, press `a`, assert no crash, no scene change.
   - `x_key_sets_user_override`: press `x`, assert `cloud.user_override_since_ambient == true` and `cloud.ambient_palette_locked == false`.
   - `c_key_clears_ambient_lock`: set `ambient_palette_locked = true`, press `c`, assert `ambient_palette_locked == false`.

4. **`src/cloud/tests/tests_scene/transitions.rs`**:
   - `ambient_event_refires_after_user_override`: simulate scheduler firing same entry twice with `user_override_since_ambient = true` in between, assert second fire is NOT deduped.

---

## 5. Out-of-scope / future work

- **'a' key HUD feedback**: a brief flash or HUD message confirming "snapped back to ambient phase X" would improve UX. Deferred — needs HUD work.
- **Per-entry lock policy**: currently `ambient_palette_locked` is a single bool. If the user has a multi-entry schedule where some entries should allow palette drift and others shouldn't, we'd need per-entry policy. Deferred — no user request for this granularity.
- **Auto-drift feeling → ambient scene mapping**: currently `system_feeling` and `ambient` are independent. A future feature could let the ambient schedule reference feeling states (e.g. `ambient.22-10 = feeling:void`) so the two systems share vocabulary. Deferred — needs design work.

---

## 6. Verification

After applying all four patches:

- `cargo test --bins`: all 1453 existing tests still pass + new tests pass.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: clean.
- `cargo +nightly miri test --bins chroma::shaders::base::tests::column_coherence`: still 6/6 (no regression from the unrelated Miri fix).
- Manual smoke test: `cosmostrix` with `ambient.22-10 = aurora` + `--auto-color-drift`, press `x` at 22:15, press `a` to snap back, verify aurora re-applies. Wait until 22:10 next day (or simulate via test), verify aurora refires.
