<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Live-Reload Behavior Research — v50-beta.3

> **Research date**: 2026-08-22
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
| `color` | `--color` | ✅ YES | 581-596 | CLI wins over config (intent preservation via `cli.color`). |
| `charset` | `--charset` | ✅ YES | 605-636 | CLI wins; charset-custom blocks also re-parsed. |
| `scene` | `--scene` | ✅ YES | 640-716 | CLI wins; scene defaults re-applied for color/charset/speed/density. |
| `speed` | `--speed` | ✅ YES | 718-730 | CLI wins. |
| `density` | `--density` | ✅ YES | 732-745 | CLI wins; `base_density` also updated. |
| `fps` | `--fps` | ✅ YES | 747-759 | CLI wins; `target_fps` updated. |
| `glitch-level` | `--glitch-level` | ✅ YES | 767-787 | CLI wins; full preset re-derivation. |
| `color-bg` | (none) | ✅ YES | 790-797 | Config-only; flips `default_bg`. |
| `monolith-size` | `--monolith-size` | ✅ YES | 800-805 | Config + CLI both applied (no intent gate — bug?). |
| `crystal-dragon` | `--crystal-dragon` | ✅ YES | 809-815 | CLI wins (`cli.crystal_dragon` guard). |
| `power-dragon` | `--power-dragon` | ✅ YES | 821-825 | Config-only path (no CLI guard — but CLI flag now exists; see Issue #1 below). |
| `bold` | `--bold` | ✅ YES | 829-844 | Range-gated (0-2); no CLI intent gate. |
| `shadingmode` | `--shadingmode` | ✅ YES | 845-856 | Range-gated (0-1); no CLI intent gate. |
| `async-mode` | `--async-mode` | ✅ YES | 857-861 | Config-only path (no CLI guard — but CLI flag now exists; see Issue #1). |
| `color.tune.*` | `--color-tune` | ✅ YES | 868-895 | CLI `--color-tune` preserved when no `[color.tune]` block. |
| `ambient.HH-MM` | (none) | ✅ YES | 897-904 | Schedule re-collected; ambient thread notified. |
| `scene-custom.<name>.*` | `--scene-custom` | ✅ YES | 863-866 | Re-applied if the active scene-custom name matches. |
| **`message`** | `-m` | ❌ **NO** | (not handled) | Field stays at startup value. `create_cloud` re-calls `set_message` with the OLD value. |
| **`message-border`** | `-mb` | ❌ **NO** | (not handled) | Same — stays at startup value. |
| **`msg-mode`** | `--msg-mode` | ❌ **NO** | (not handled) | Field stays at startup value. Default fallback + gate logic only runs at startup. |
| **`intro-color`** | `--intro-color` | ❌ **NO** | (not handled) | Intro only plays once at startup; live-reload is moot. |
| **`intro`** | `--intro` | ❌ **NO** | (not handled) | Same — intro is a one-shot animation. |
| `colors-custom.<name>` | (none) | ⚠️ PARTIAL | 599-603 | Only re-parsed if `custom_palette_name` was set at startup. New palettes added mid-run are not discovered. |
| `charset-custom.<name>` | (none) | ⚠️ PARTIAL | 609-611 | Only re-parsed if the active charset name matches a custom block. New charsets added mid-run are not discovered. |

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

**Pros**: Fully consistent live-reload. User edits config → sees
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

## 6. Source-Code References (for implementer)

- `rebuild_cloud_config`: `src/config/live_config/mod.rs:562-907`
- Event-loop reload consumer: `src/interactive/event_loop.rs:336-360`
- `CliExplicit` struct: `src/cli/app.rs:155-167`
- `create_cloud` (calls `set_message`): `src/cli/app.rs:277-280`
- `set_message` / `set_message_border`: `src/cosmic_dragon_engine/cloud/mod.rs:493-511`
- `apply_config_and_runtime_defaults` (startup msg-mode gate):
  `src/config/config_apply.rs:51-518`

---

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
