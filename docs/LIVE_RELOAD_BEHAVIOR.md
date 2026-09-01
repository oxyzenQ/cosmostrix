<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Live-Reload Behavior Research — v51

> **Status**: Option D (masterclass) IMPLEMENTED in v50.0.0-alpha.7.
> All 4 issues from the v50-beta.3 research are now fixed. The v51
> Z-master-1B audit (2026-08-30) then found and fixed 5 MORE gaps in the
> custom palette / custom scene switching paths — see "10. v51
> Z-master-1B Audit" below for the per-key matrix update.
>
> **Research date**: 2026-08-22 (v50-beta.3)
> **Implementation date**: 2026-08-22 (v50.0.0-alpha.7)
> **v51 audit date**: 2026-08-30 (Z-master-1B)
>
> **Trigger**: owner confusion about which config keys live-reload vs
> require restart. Specifically: "if I set `msg-mode = false` in config
> while cosmostrix is running, does it reload without the default
> message? If I edit `message = "hey"` mid-run, does it show 'hey'?"
> Same question for `power-dragon`, `crystal-dragon`, `async-mode`.
>
> Source of truth: `src/config/live_config/mod.rs::rebuild_cloud_config`
> (lines 560-907) is the single function that runs on every config
> file change. Any field NOT touched by this function stays at its
> startup value — the renderer never sees the new config value until
> the process is restarted.

---

## 1. Findings — Per-Key Live-Reload Matrix

| Config Key | CLI Flag | Live-Reloads? | Source: `rebuild_cloud_config` line | Notes |
|------------|----------|:-------------:|:------------------------------------:|-------|
| `color` | `--color` | OK YES | 581-596 | CLI wins over config (intent preservation via `cli.color`); `--colors-custom` also blocks the key (Z-master-2-v2). |
| `charset` | `--charset` | OK YES | 605-636 | CLI wins; charset-custom blocks also re-parsed. |
| `scene` | `--scene` | OK YES | 640-716 | CLI wins; `--scene-custom` also blocks the config scene key (Z-master-2-v2); scene defaults re-applied for color/charset/speed/density. |
| `speed` | `--speed` | OK YES | 718-730 | CLI wins. |
| `density` | `--density` | OK YES | 732-745 | CLI wins; `base_density` also updated. |
| `fps` | `--fps` | OK YES | 747-759 | CLI wins; `target_fps` updated. |
| `glitch-level` | `--glitch-level` | OK YES | 767-787 | CLI wins; full preset re-derivation. |
| `color-bg` | `--color-bg` | OK YES | 790-797 | CLI wins (`cli.color_bg` guard, Z-master-2-v2); flips `default_bg`. |
| `monolith-size` | `--monolith-size` | OK YES | 800-805 | CLI wins (`cli.monolith_size` guard, v50.0.0-alpha.7 Issue #4 fix). |
| `crystal-dragon` | `--crystal-dragon` | OK YES | 809-815 | CLI wins (`cli.crystal_dragon` guard). |
| `power-dragon` | `--power-dragon` | OK YES | 821-825 | CLI wins (`cli.power_dragon` guard, v50.0.0-alpha.7). |
| `bold` | `--bold` | OK YES | 829-844 | CLI wins (`cli.bold` guard, Z-master-2-v2); range-gated (0-2). |
| `shading-mode` | `--shading-mode` | OK YES | 845-856 | CLI wins (`cli.shading_mode` guard, Z-master-2-v2); range-gated (0-1). |
| `async-mode` | `--async-mode` | OK YES | 857-861 | CLI wins (`cli.async_mode` guard, v50.0.0-alpha.7). |
| `color.tune.*` | `--color-tune` | OK YES | 868-895 | CLI `--color-tune` preserved when no `[color.tune]` block. |
| `ambient.HH-MM` | (none) | OK YES | 897-904 | Schedule re-collected; ambient thread notified. |
| `scene-custom.<name>.*` | `--scene-custom` | OK YES | 863-866 | Re-applied if the active scene-custom name matches. Every field arm honors `cli_explicit.*` (Z-master-1-v2 gap 1 + Z-master-2-v2 bold/shading-mode/colors-custom); intra-block `color`/`colors-custom` + `charset`/`charset-custom` conflicts resolve deterministically like startup (gap 3). |
| **`message`** | `-m` | X **NO** | (not handled) | Field stays at startup value. `create_cloud` re-calls `set_message` with the OLD value. |
| **`message-border`** | `-mb` | X **NO** | (not handled) | Same — stays at startup value. |
| **`msg-mode`** | `--msg-mode` | X **NO** | (not handled) | Field stays at startup value. Default fallback + gate logic only runs at startup. |
| **`intro-color`** | `--intro-color` | X **NO** | (not handled) | Intro only plays once at startup; live-reload is moot. |
| **`intro`** | `--intro` | X **NO** | (not handled) | Same — intro is a one-shot animation. |
| `colors-custom.<name>` | (none) | warning: PARTIAL | 599-603 | Only re-parsed if `custom_palette_name` was set at startup. New palettes added mid-run are not discovered. |
| `charset-custom.<name>` | (none) | warning: PARTIAL | 609-611 | Only re-parsed if the active charset name matches a custom block. New charsets added mid-run are not discovered. |

---

## 2. Owner's Confusion Points — Answered

### Q1: "If I run `cosmostrix` (no flags) then set `msg-mode = false` in config mid-run, does it reload without the default 'cosmostrix v-x' message?"

**No.** `msg-mode` is not handled by `rebuild_cloud_config`. The
renderer keeps showing the default "cosmostrix v50.0.0-beta.3" overlay
until the user presses `q` and reruns cosmostrix. This is the collision
the owner flagged: the user expects config edits to take effect
immediately (like `color`/`speed`/`fps` do), but `msg-mode` silently
ignores mid-run edits.

### Q2: "If I run `cosmostrix -m "hello"` then edit config to set `message = "hey"` mid-run, does it reload with 'hey'?"

**No.** Same reason — `message` and `message-border` are not handled by
`rebuild_cloud_config`. The renderer keeps showing "hello" until
restart. This is inconsistent with `color`/`speed`/`fps` which DO
live-reload.

### Q3: "What about `power-dragon`, `crystal-dragon`, `async-mode`?"

**Yes, all three live-reload.** They are handled in `rebuild_cloud_config`:
- `crystal-dragon` (line 809-815) — with CLI intent guard.
- `power-dragon` (line 821-825) — config-only path (no CLI guard; see Issue #1).
- `async-mode` (line 857-861) — config-only path (no CLI guard; see Issue #1).

### Q4: "And `intro-color` / `intro`?"

**No, neither live-reloads.** Both are one-shot startup settings — the
intro animation plays once at process start and never again. A config
edit to `intro-color` or `intro` has no effect until restart. This is
expected behavior (the owner already knew this), but it's inconsistent
with the live-reload keys.

---

## 3. Issues Found

### Issue #1: `power-dragon` and `async-mode` live-reload paths ignore CLI intent

`rebuild_cloud_config` reads these directly from config without
checking `cli_explicit`. If the user started cosmostrix with
`--power-dragon false` (CLI explicit), then edits `config.toml` to set
`power-dragon = true`, the live-reload will flip power-dragon to true
— overriding the user's CLI intent. This contradicts the
"CLI wins over config" contract documented for `crystal-dragon` (which
DOES have the `cli.crystal_dragon` guard).

**Root cause**: `power-dragon` and `async-mode` were config-only keys
when `rebuild_cloud_config` was written. The CLI flags
(`--power-dragon`, `--async-mode`) were added in v50-beta.3 but the
live-reload path was not updated to add the intent guard.

**Affected keys**: `power-dragon`, `async-mode`. `crystal-dragon` is
unaffected (already has the guard).

### Issue #2: `message` / `message-border` / `msg-mode` have no live-reload path

These three keys are completely absent from `rebuild_cloud_config`.
Any mid-run config edit to them is silently ignored. This is the
primary source of owner confusion — the user expects "edit config,
see change" but instead has to restart.

**Severity**: Medium. The renderer works correctly at startup; the
issue is purely UX (user expects live-reload consistency).

### Issue #3: `intro-color` and `intro` have no live-reload path (expected)

These are one-shot startup settings (intro animation plays once).
Live-reload is moot. No fix needed — but documentation should make
this explicit so users don't expect it.

### Issue #4: `monolith-size` live-reload has no CLI intent guard

Line 800-805 reads `monolith-size` from config without checking
`cli.monolith_size`. If the user started with `--monolith-size large`
(CLI explicit), a config edit to `monolith-size = "small"` would
override the CLI intent on the next reload. Same class as Issue #1.

**Severity**: Low (monolith-size rarely changes mid-session).

---

## 4. Masterclass Solution Options

The owner asked for "some solution options masterclass so it doesn't
confuse users." Here are 4 options, ranked by effort vs. impact.

### Option A — Document the behavior (zero code change, immediate)

Add a "Live-Reload Behavior" table to `docs/LTS_AUDIT_CONFIG_LIVE_RELOAD.md`
and the `--help` output. Make it crystal clear which keys live-reload
and which require restart.

**Pros**: Zero risk, immediate clarity, no maintenance burden.
**Cons**: Doesn't fix the underlying inconsistency — users still have
to restart for `message`/`msg-mode` edits.

**Effort**: 30 min (doc only).

### Option B — Fix the CLI intent guards for `power-dragon` + `async-mode` (low risk)

Add `cli.power_dragon` and `cli.async_mode` fields to `CliExplicit`
struct, then gate the live-reload reads in `rebuild_cloud_config`
with the same pattern `crystal-dragon` uses:

```rust
if !cli.power_dragon {
    if let Some(v) = cfg.get("power-dragon") {
        // ...
    }
}
```

**Pros**: Restores the "CLI wins over config" contract for all 3
boolean dragon flags. Consistent behavior.
**Cons**: Doesn't address the `message`/`msg-mode` live-reload gap.

**Effort**: 1-2 hours (add 2 fields to CliExplicit + 2 guards + tests).

### Option C — Add live-reload for `message` / `message-border` / `msg-mode` (medium effort)

Extend `rebuild_cloud_config` to handle these 3 keys:
- Re-read `message` / `message-border` from config (with CLI intent
  guard — CLI `-m`/`-mb` wins).
- Re-apply `msg-mode` gate: if `msg-mode = false` AND no CLI message,
  clear the message field.
- On the event-loop side, after `create_cloud`, call
  `cloud.set_message(new_cfg.message)` if the message changed.

**Pros**: Fully consistent live-reload. User edits config -> sees
change immediately. No more "press q and rerun" friction.
**Cons**: More surface area for bugs. The `msg-mode` gate logic in
`config_apply.rs` would need to be extracted into a shared helper so
both startup and live-reload call it. The event-loop side needs a
diff check ("did the message actually change?") to avoid redundant
`set_message` calls every reload.

**Effort**: 3-4 hours (extract gate helper + 3 rebuild_cloud_config
blocks + event-loop wiring + tests).

### Option D — Masterclass: A + B + C combined (full fix)

Do all three:
1. **A**: Document the live-reload matrix in `docs/` + `--help`.
2. **B**: Fix the CLI intent guards for `power-dragon` + `async-mode`.
3. **C**: Add live-reload for `message` / `message-border` / `msg-mode`.

**Pros**: Fully consistent. Every config key that CAN live-reload
DOES live-reload. Every key that CAN'T (intro, intro-color) is
documented as restart-only. CLI intent is preserved across reloads
for all boolean flags.
**Cons**: Largest effort. Highest regression risk (live-reload is
the most-touched code path).

**Effort**: 5-6 hours (A: 30min + B: 1.5h + C: 3.5h + integration
tests + stress tests).

---

## 5. Recommendation

**Option D (masterclass)** is the right call for a stable v50.0.0
release. The owner has already invested in live-reload
infrastructure (file watcher, SHA-512 fingerprinting, OKLab smooth
transitions on color changes) — extending it to cover the 3 missing
message keys + fixing the 2 missing CLI intent guards closes the
consistency gap that's causing user confusion.

For v50-beta.3 (current beta), **Option B** is the safest incremental
fix — it restores the CLI contract without touching the
message-reload surface area. Option C can land in a follow-up beta.

**Option A** (docs only) should ship regardless of which code option
is chosen — the live-reload matrix is essential user-facing
documentation.

---

## 6. Implementation Status — v50.0.0-alpha.7 (Option D DONE)

All 4 issues from the v50-beta.3 research are now FIXED in
v50.0.0-alpha.7. Option D (masterclass) was implemented:

### Issue #1: `power-dragon` + `async-mode` CLI intent guards — FIXED

**Was**: live-reload paths read config directly without checking
`cli_explicit`, so CLI flag `--power-dragon false` was overridden by
config edit `power-dragon = true` on next reload.

**Now**: added `power_dragon: bool` and `async_mode: bool` fields to
`CliExplicit` struct. `rebuild_cloud_config` now gates both reads with
`if !cli.power_dragon { ... }` and `if !cli.async_mode { ... }` —
mirroring the `crystal-dragon` pattern. CLI wins over config on
live-reload for all 3 boolean dragon/async flags.

### Issue #2: `message` / `message-border` / `msg-mode` live-reload — FIXED

**Was**: these 3 keys were completely absent from
`rebuild_cloud_config`. Editing config.toml mid-run had no effect
until restart. Primary source of owner/user confusion.

**Now**: `rebuild_cloud_config` handles all 3 keys with full
precedence:
1. CLI `-m` / `-mb` (always wins — `cli.message` guard skips config read)
2. `msg-mode=false` -> suppress config message (gate fires)
3. config `message-border` (wins over `message` when both present)
4. config `message` (no border)
5. default fallback `Experience a masterpiece with cosmostrix v{}` with border
   (applied on live-reload when no config message key is present —
   mirrors startup behavior at main.rs:1239-1258)

**v50.0.0-beta.7 follow-up**: the original fix preserved `base.message`
when no config key was present, which leaked stale config values (e.g.
user comments out `message = "hey"`, renderer kept showing "hey").
Now the else branch resets to `default_message_text()` + border,
mirroring the `color.tune` reset-on-comment pattern (Limitation C).
Two lock tests prevent regression:
`live_reload_no_config_message_reverts_to_default` and
`live_reload_no_config_message_clears_when_msg_mode_false`.

The `msg-mode` gate mirrors `config_apply.rs`: when `msg-mode=false`
AND message came from config (not CLI), clear it. CLI `-m`/`-mb` is
unaffected.

### Issue #3: `intro-color` / `intro` live-reload — FIXED (intro-color only)

**Was**: `intro-color` had no live-reload path. `intro` is one-shot
(expected — intro plays once at startup).

**Now**: `rebuild_cloud_config` handles `intro-color` with CLI intent
guard (`cli.intro_color`). Validates theme name on reload — invalid
themes are logged and cleared (soft-fail, unlike startup which
hard-errors + exits, to avoid crashing a running session). `intro`
remains restart-only (one-shot animation).

### Issue #4: `monolith-size` CLI intent guard — FIXED

**Was**: `monolith-size` live-reload lacked CLI intent guard. Low
severity — rarely changes mid-session.

**Now**: added `monolith_size: bool` field to `CliExplicit` struct.
`rebuild_cloud_config` gates the read with `if !cli.monolith_size` —
mirrors the `crystal-dragon` / `power-dragon` / `async-mode` pattern.
CLI `--monolith-size` wins over config on live-reload.

### Updated Per-Key Live-Reload Matrix (v51)

| Config Key | CLI Flag | Live-Reloads? | CLI Intent Guard? |
|------------|----------|:-------------:|:-----------------:|
| `color` | `--color` | OK YES | OK YES — v51: switching TO/FROM a `[colors-custom.<name>]` palette now works (custom wins on collision, startup parity; switching to a builtin clears the active palette). |
| `charset` | `--charset` | OK YES | OK YES |
| `scene` | `--scene` | OK YES | OK YES — v51: switching TO a `[scene-custom.<name>]` scene now applies base-scene + fields (incl. rain_style); switching AWAY no longer re-applies the stale custom layer; scene fps + glitch-level defaults now apply (startup parity). |
| `speed` | `--speed` | OK YES | OK YES |
| `density` | `--density` | OK YES | OK YES |
| `fps` | `--fps` | OK YES | OK YES |
| `glitch-level` | `--glitch-level` | OK YES | OK YES |
| `color-bg` | (none) | OK YES | N/A (no CLI flag) |
| `monolith-size` | `--monolith-size` | OK YES | OK YES (FIXED in alpha.7) |
| `crystal-dragon` | `--crystal-dragon` | OK YES | OK YES |
| `power-dragon` | `--power-dragon` | OK YES | OK YES (FIXED in alpha.7) |
| `async-mode` | `--async-mode` | OK YES | OK YES (FIXED in alpha.7) |
| `bold` | `--bold` | OK YES | X NO (no CLI intent gate) |
| `shading-mode` | `--shading-mode` | OK YES | X NO (no CLI intent gate) |
| `color.tune.*` | `--color-tune` | OK YES | OK YES |
| `ambient.HH-MM` | (none) | OK YES | N/A |
| `scene-custom.<name>.*` | `--scene-custom` | OK YES | OK YES |
| **`message`** | `-m` | OK YES (FIXED in alpha.7) | OK YES (`cli.message`) |
| **`message-border`** | `-mb` | OK YES (FIXED in alpha.7) | OK YES (`cli.message`) |
| **`msg-mode`** | `--msg-mode` | OK YES (FIXED in alpha.7) | OK YES (`cli.msg_mode`) |
| **`msg-fill-style`** | `-mfs` / `--msg-fill-style` | OK YES (added v51) | OK YES (`cli.msg_fill_style`) |
| **`intro-color`** | `--intro-color` | OK YES (FIXED in alpha.7) | OK YES (`cli.intro_color`) |
| **`intro`** | `--intro` | X NO (one-shot) | N/A |

### Stress Tests Added

13 new tests in `src/config/live_config/tests.rs`:
- `live_reload_message_border_from_config`
- `live_reload_message_bare_from_config`
- `live_reload_message_border_wins_over_message`
- `live_reload_msg_mode_false_suppresses_config_message`
- `live_reload_msg_mode_true_keeps_config_message`
- `live_reload_msg_mode_defaults_true_when_unset`
- `live_reload_cli_message_wins_over_config`
- `live_reload_cli_msg_mode_wins_over_config`
- `live_reload_power_dragon_respects_cli_explicit`
- `live_reload_async_mode_respects_cli_explicit`
- `live_reload_intro_color_from_config`
- `live_reload_intro_color_invalid_soft_fails`
- `live_reload_intro_color_cli_explicit_wins`

---

## 8. Known Limitations — 99% Not 100% Perfect

Live-reload is designed for stability, not perfection. The owner
accepts that live-reload will never be 100% perfect — it is **99%
reliable, stable, and production-LTS-grade**. The remaining 1%
consists of documented edge cases that users should be aware of:

### Limitation A: `--verbose | grep` pipe behavior

**Symptom**: Running `cosmostrix -v | grep "crystal"` prints the
verbose output but cosmostrix stays in interactive mode — the user
must press `q` to exit.

**Root cause**: cosmostrix does NOT check whether stdout is a TTY
before entering interactive mode. The verbose output goes to stderr
(which is also piped when stdout is piped), but cosmostrix still
enters the alternate screen + raw mode. `grep` only captures the
stderr lines that arrive before the alt-screen entry.

**Why it's not a bug**: This is by design. cosmostrix's primary use
case is interactive terminal rendering. Piping `--verbose` output
to `grep` is a diagnostic use case, not the primary flow. The
expected workflow is: run `cosmostrix -v` in a TTY, read the output,
press `q` to exit. For non-interactive diagnostics, use `--doctor`
or `--benchmark` instead.

**Workaround**: To capture verbose output without entering
interactive mode, redirect stderr to a file:

```bash
cosmostrix -v 2>/tmp/cosmostrix-verbose.log
# press q after a moment, then:
grep "crystal" /tmp/cosmostrix-verbose.log
```

### Limitation B: Multi-terminal config overwrite

**Symptom**: Terminal 1 runs `cosmostrix --dump-config --force`
(resets config.toml to the default template — all values commented
out). Terminal 2 is still running cosmostrix with the old config
loaded in RAM. Terminal 2 continues running fine — it does NOT
error on the config change.

**Root cause**: cosmostrix's file watcher detects config.toml
content changes via SHA-256 hash. When `--dump-config --force`
rewrites the file, the hash changes, and the watcher fires a
reload. The reload reads the new (default) config — but since the
default template has all values commented out, the parsed config
is empty. `rebuild_cloud_config` sees an empty config HashMap and
preserves the startup values (base.clone()). The renderer keeps
using the old values from RAM.

**Why it's not a bug**: This is the correct behavior for
live-reload. An empty config means "use defaults" — and the
defaults happen to match what was already running (because the
user's previous config set the same values). The renderer does NOT
error because there's nothing wrong: the config is valid (just
empty/default), and the renderer's current values are still
correct.

**When it WOULD error**: If the new config contained an INVALID
value (e.g. `color = "not-a-color"`), the watcher's
`validate_config_strictly` would reject it, print an error, and
keep the previous valid config. The renderer would NOT crash —
it would log the error and continue with the last-known-good
values.

**Owner's recommendation**: For multi-terminal development, treat
each `cosmostrix` process as independent. If you reset config via
`--dump-config --force`, restart all running cosmostrix instances
to pick up the fresh defaults. The live-reload system is designed
for incremental edits (change one value, see it apply), not for
wholesale config replacement.

### Limitation C: `color.tune` reset-on-comment — FIXED

**Symptom**: User sets `color.tune.brightness = 0.0` in config,
sees the rain go dark. User then comments out the line
(`# color.tune.brightness = 0.0`). The rain STAYS dark — the
brightness does not return to normal (1.0).

**Root cause**: `rebuild_cloud_config` only updated `color_tune`
when at least one `color.tune.*` key was present in the config
HashMap (`has_tune_keys` gate). When all `color.tune.*` keys were
commented out, the parser didn't see them, so the gate was false,
and the base tune (with brightness=0.0) was preserved.

**Fix (v50.0.0-alpha.7)**: removed the `has_tune_keys` gate. Now
always calls `color_tune_from_config(cfg)` — when no keys are
present, it returns `IDENTITY` (all 1.0), which is the correct
"reset to default" behavior. CLI `--color-tune` is preserved via
`cli.color_tune` guard: when CLI is explicit, config absence does
NOT reset (CLI wins).

**Behavior after fix**:
- `color.tune.brightness = 0.0` -> rain goes dark OK
- Comment out the line -> rain returns to normal (brightness=1.0) OK
- CLI `--color-tune bright=2.0` + no config block -> stays at 2.0 OK

---

## 9. v51 Z-master-1B Audit — Custom Palette / Scene Switching (2026-08-30)

Owner suspicion: "some functions in config.toml don't work at
live-reload." Deep audit of every `USER_CONFIG_KEYS` entry against
`rebuild_cloud_config` + the downstream `create_cloud` application found
FIVE real gaps — all in the custom palette / custom scene switching
paths, all confirmed by failing tests first, then fixed:

| # | Gap | Symptom | Fix |
|---|-----|---------|-----|
| 1 | `color = "<custom-palette>"` switch ignored | The block only parsed BUILTIN scheme names (unlike startup's custom-first lookup in `main.rs`), so switching to a custom palette at runtime was a silent no-op. | Custom names now load via `load_custom_palette` (custom wins on collision — startup parity). |
| 2 | Switching `color` away from an active custom palette | The stale palette stayed loaded, and `create_cloud` applies `custom_palette` AFTER the scheme — so the builtin the user switched to never rendered. | Switching to a builtin now clears `custom_palette` + `custom_palette_name`. |
| 3 | `scene = "<custom-scene>"` switch ignored | Only `scene_name` updated; rain_style/color/charset/speed/density kept the PREVIOUS scene's values whenever the ambient scene-change branch did not fire. | Custom scene names resolve `rain_style` from the base-scene (mirroring startup's `rain_style_for_custom_scene`) and mark the scene active so the scene-custom layer applies. |
| 4 | Switching `scene` away from a custom scene | The startup `scene_custom_name` tracker was immutable — the stale custom layer re-applied on top of EVERY builtin scene the user switched to. | The tracker is now the rebuilt config's value; the layer only re-applies while the custom scene is still the active scene. |
| 5 | Scene `fps` / `glitch-level` defaults never applied | Startup (`apply_default_scene_values`) applies the scene's fps + glitch presets; the live-reload scene block only applied color/charset/speed/density/rain_style. | Both arms added to the scene block (before the user-key blocks, so explicit `fps` / `glitch-level` keys still win). |

Verification: 13 new regression tests in `src/config/live_config/tests.rs`
(`rebuild_switches_color_to_custom_palette_at_runtime`,
`rebuild_switches_color_away_from_custom_palette`,
`rebuild_unknown_color_name_keeps_current_palette`,
`rebuild_switches_scene_to_custom_scene_at_runtime`,
`rebuild_custom_scene_resolves_rain_style_from_base_scene`,
`rebuild_custom_scene_without_base_scene_defaults_to_glyph`,
`rebuild_switches_scene_away_from_custom_scene`,
`rebuild_active_custom_scene_field_edit_still_reapplies`,
`rebuild_custom_scene_colors_custom_field_loads_palette`,
`rebuild_scene_switch_applies_scene_fps_default`,
`rebuild_user_fps_key_wins_over_scene_default`,
`rebuild_scene_switch_applies_scene_glitch_default`,
`rebuild_user_glitch_key_wins_over_scene_default`).

Also verified working (no change needed): `charset` switching in both
builtin/custom directions, `monolith-size`, `bold`, `shading-mode`,
`color-bg`, `crystal-dragon`, `power-dragon`, `async-mode`, `speed`,
`density`, `fps`, `glitch-level`, `color.tune.*` (reset-on-comment),
`message`/`message-border`/`msg-mode`/`msg-fill-style`, `ambient.HH-MM`,
`ambient-snapback-secs`, and `charset-custom.<name>` block edits. The only
restart-only keys remain `intro` and `intro-color` (the intro is a one-shot
animation — documented Limitation, not a gap).

---

## 10. Source-Code References (for implementer)

- `rebuild_cloud_config`: `src/config/live_config/mod.rs` (starts near the
  top of the file; the color/scene/scene-custom blocks are the v51-audited
  paths)
- Event-loop rebuild consumer: `src/interactive/event_loop_config_rebuild.rs`
  (`apply_config_rebuild` — swaps the Cloud + Frame between frames)
- `CliExplicit` struct: `src/cli/app.rs`
- `create_cloud` (applies custom_palette AFTER the scheme — the reason gap #2
  mattered): `src/cli/app.rs`
- Startup custom-first color resolution (parity reference): `src/main.rs`
- Scene-custom layer: `src/scene_custom/mod.rs`
  (`rain_style_for_custom_scene`; the block applier
  `apply_scene_custom_to_cloud_config` lives in
  `src/scene_custom/overrides.rs` since the Z-master-1-v2 refactor)
- Custom palette loader: `src/engine/chroma_dragon_engine/colors_custom.rs`
  (`load_custom_palette`, `is_colors_custom_name`)

## 11. Z-master-1-v2 Audit — Killer-Features Priority Contract (2026-09-01)

Owner suspicion: "some potential bug" in the killer features
(colors-custom / charset-custom / scene-custom) under live reload.
Depth stresstest of the scene-custom re-apply path found FOUR gaps in the
priority contract — the same bug family as FPS-F4, which had fixed the
fps field ONLY while every other scene-custom field stayed ungated:

| # | Gap | Symptom | Fix |
|---|-----|---------|-----|
| 1 | Scene-custom field layer had no CLI gates (except fps) | `cosmostrix --speed 50 --scene-custom fast` ran at 50 until the FIRST config edit, then the block's `speed` silently won. Same for `density`, `color`, `charset`, `charset-custom`, `colors-custom`, `glitch-level`, `async-mode`. | Every arm in `apply_scene_custom_field_to_cloud_config` now returns early when the matching `cli_explicit.*` flag is set (mirrors FPS-F4). |
| 2 | Base-scene inheritance layer had no CLI gates (except fps) | Same drift class via `base-scene = <name>` defaults: `--color`/`--charset`/`--speed`/`--density`/`--glitch-level` were re-overridden on every reload. | `apply_base_scene_to_cloud_config` gates each field on `cli_explicit.*` (mirrors the startup `apply_base_scene_to_args` is_explicit checks). |
| 3 | Intra-block conflict resolution was nondeterministic on reload | A block defining BOTH `color` + `colors-custom` (or `charset` + `charset-custom`) applied in HashMap iteration order at reload, while startup deterministically let `color`/`charset` win (`apply_profile_overrides` skip rule). Reload could load a palette startup never loaded. | `apply_scene_custom_to_cloud_config` pre-scans for `color`/`charset` presence and skips the losing field — startup parity, deterministic. |
| 4 | Scene switch left a stale custom palette shadowing the new scene | Switching `scene` away from a palette-owning custom scene set the builtin scheme but never cleared `custom_palette`; `create_cloud` applies the palette AFTER the scheme, so the switch was a visual no-op for color. | The builtin-scene color arm now clears `custom_palette` + `custom_palette_name` when it applies the scene color default. |

Verification: 12 regression tests in
`src/config/live_config/tests_cli_priority.rs` (extracted from
`tests.rs` to respect the 800-LOC file cap) covering each gate, both
conflict rules, and the palette-clear-on-scene-switch. Full suite
green: 1957 passed / 0 failed.

Priority contract (unchanged, now actually enforced on every path):

```text
CLI flags (cli_explicit.*)
  > config.toml keys
    > scene-custom block fields
      > base-scene inherited defaults
        > built-in defaults
```

## 12. Z-master-2-v2 Audit — CLI Intent Preservation for Config Keys (2026-09-01)

Owner suspicion: "some potential bug" in CLI + config/live-reload. Depth
audit of every CLI flag with a matching config key found FIVE flags whose
CLI intent was silently lost on the first live-reload — the exact bug
class the project had already fixed for monolith-size (Issue #4),
power-dragon, async-mode, msg-mode, intro-color, message, and color-tune
(v50.0.0-alpha.7). The pattern: `CliExplicit` grew a guard per fix, but
these five were never added:

| # | Flag | Bug (before) | Fix |
|---|------|--------------|-----|
| 1 | `--bold N` | config `bold` key overrode the CLI flag on every reload (startup gates via `config_value`) | `CliExplicit.bold` + `if !cli.bold` guard in `rebuild_cloud_config` |
| 2 | `--shading-mode N` | same class — config `shading-mode` key overrode the flag | `CliExplicit.shading_mode` + guard |
| 3 | `--color-bg X` | same class — config `color-bg` key overrode the flag | `CliExplicit.color_bg` + guard |
| 4 | `--colors-custom <name>` | a config `color` key switching to a builtin CLEARED the CLI-owned custom palette (startup never drops it — main.rs checks `--colors-custom` first) | `CliExplicit.colors_custom` + `!cli.colors_custom` gate on the color block AND the scene color-default arm |
| 5 | `--scene-custom <name>` | a config `scene` key replaced the CLI-selected custom scene AND cleared the `scene_custom_name` tracker (startup applies the CLI scene-custom layer last, so it wins) | `CliExplicit.scene_custom` + `!cli.scene_custom` gate on the scene block; the tail block still re-applies the custom scene's fields so live-editing the block keeps working |

Consequential scene-custom field gates shipped in the same pass: the
block's `bold` / `shading-mode` / `colors-custom` fields now also honor
`cli_explicit.*` (mirrors FPS-F4, extending Z-master-1-v2 gap 1 to the
newly tracked flags).

Verification: 9 regression tests in
`src/config/live_config/tests_cli_priority.rs` (7 rebuild-level +
2 `build_cli_explicit` argv-level). Suite green: 1967 passed / 0 failed.

Stale matrix rows fixed in the same pass (section 1): the
monolith-size / power-dragon / async-mode rows still described the
pre-alpha.7 behavior ("no intent gate") even though those guards landed
in v50.0.0-alpha.7 — the rows now match the code.

---
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
