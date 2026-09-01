<!-- SPDX-License-Identifier: GPL-3.0-only -->

# v51.1 CLI-Locked Fallback — Internal Research Audit

> Task: owner special directive (2026-09-01) — "premature logic at usage
> comment out vs in at config.toml". Owner repro:
> `cosmostrix -v -s -C minimal --scene crystal-dragon -mfs words`,
> runtime config edit `# scene = cinematic` → `scene = cinematic`
> (live-reload good) → back to `# scene = cinematic` (engine stayed on
> cinematic — should return to crystal-dragon without exit + rerun).
> Scope: masterclass audit to peak — precision, harmony, stability/LTS;
> no over-engineering; update all docs/reference so nothing stays stale.

## 1. The premature logic (root cause)

Two cooperating defects, both in the live-reload priority layer:

**Defect 1 — the beta.6 "CLI retired" zeroing.**
`apply_config_rebuild` set `base_cfg.cli_explicit =
CliExplicit::default()` before every rebuild (v50.0.0-beta.6 "temporal
precedence: runtime config > scene defaults, CLI retired"). Consequences:

- Every CLI lock was silently destroyed at the FIRST config edit.
- All 21 per-key guards inside `rebuild_cloud_config` (Bug 3, Issue #4,
  alpha.7 fixes, Z-master-2-v2 hardening) became dead code in production
  — the unit tests call the function directly with live flags, so the
  guard tests passed while the production path never executed a guarded
  branch. Two priority models half-shipped on top of each other: the
  guards said "CLI blocks config", the zeroing said "config blocks CLI".
  Neither was the owner's actual contract.

**Defect 2 — the runtime scene sync contamination.**
`sync_base_cfg_with_runtime_scene` fired whenever the config `scene` key
was absent, copying the RUNTIME scene's managed defaults
(color/charset/speed/density/rain_style) into `base_cfg` — the rebuild
base that is supposed to hold the locked startup values. After a
config-driven scene switch (key present → sync skipped), commenting the
key out triggered the sync with the runtime scene still set to the
config-driven scene: the base was permanently contaminated, so the
rebuild could never fall back to the CLI-locked scene. The owner's exact
symptom.

## 2. The v51.1 contract (owner's abstract rule, formalized)

```text
Startup:  CLI > config.toml > scene defaults > built-in defaults
Runtime:  config key > CLI lock > scene defaults > built-in defaults
```

- The CLI value is LOCKED (the pristine startup snapshot, never mutated).
- A config key PRESENT at runtime overrides the lock (the file edit is
  the most recent user intent — same temporal logic as shortkey c/s/x).
- A config key ABSENT → fall back to the locked startup value (CLI
  first, then config@startup, then defaults) — no exit, no rerun.
- Shortkeys and ambient fires keep their existing runtime semantics
  (scene family survives unrelated config edits via the sync).
- Scene-managed defaults sit BELOW the CLI lock
  (`config key > CLI lock > scene default`) — startup parity, and the
  Z-master-1/2 field gates (scene-custom / base-scene layers) remain
  valid and are now actually live in production.

## 3. Changes (9 files, +20 net tests)

| File | Change |
|------|--------|
| `src/config/live_config/mod.rs` | Removed all 16 top-level blocker guards (key present wins); kept the scene-block inner gates (CLI lock > scene defaults); `color.tune`/`message`/`msg-mode` rewritten to the fallback pattern (alpha.7 reset-on-comment survives for lockless runs); contract documented on the function. |
| `src/interactive/event_loop_config_rebuild.rs` | Zeroing REMOVED; `startup_cfg` param; `resolve_scene_base_action` delta rule (present → apply / just-removed → restore / never-present → sync); revert event traced. |
| `src/interactive/event_loop_scene_sync.rs` | New `SceneBaseAction` enum + pure resolver; new `restore_locked_scene_family` (exact managed-field rollback); 9 tests incl. the owner's end-to-end scenario. |
| `src/interactive/event_loop.rs` | `startup_cfg = cfg.clone()` immutable locked layer; `cli_has_any_override` via `CliExplicit::any()`. |
| `src/cli/app.rs` | `CliExplicit::any()` — the ambient startup deferral was an inline OR-chain of 15 of the 21 flags; six flags (`--bold`, `--shading-mode`, `--color-bg`, `--colors-custom`, `--scene-custom`, `-mfs`) did not defer ambient. Struct doc updated to the contract. |
| `src/cli/cli_explicit.rs`, `src/cli/build_cloud_cfg.rs` | Stale "CLI > config.toml > scene priority" comments updated to the locked-fallback contract. |
| `src/config/live_config/tests_cli_fallback.rs` (new) | 11 per-key fallback tests (fps/bold/color+palette/scene-custom/message/msg-mode/msg-fill-style/color-tune/charset/dragons+async/startup-effective layer). |
| `tests.rs` / `tests_cli_priority.rs` / `tests_rejection_msg.rs` / `tests_msg_fill_style.rs` | 14 old-contract guard tests rewritten to the new key-present-wins semantics. |

Behavior changes vs pre-v51.1 (all in the owner's intended direction):

1. Scene family reverts to the CLI-locked (startup) scene when the config
   `scene` key is commented out — the owner's bug.
2. `color.tune` / `message` / `msg-mode` with a CLI lock survive key
   removal (previously reset to default/identity on the first reload —
   the alpha.7 guards were dead, so `--color-tune` and `-m` were lost on
   ANY config edit).
3. Runtime config keys override CLI flags while present (previously
   inconsistent: the zeroing made config win anyway, but the dead guards
   and their tests claimed the opposite).
4. The six missing flags now defer ambient at startup (documented
   "ANY CLI flag" rule).

## 4. Verification

- **Unit**: 2015 passed / 0 failed (was 1995; +20 net).
- **Live PTY proof** (real binary, real watcher, `COSMOSTRIX_LIVE_RELOAD_DEBUG=1`,
  graceful `q` exit so the trace buffer drains):
  - phase 1 (uncomment): `apply scene='cinematic'` → `Cloud rebuilt —
    speed=9.00 density=0.750 fps=30.00`
  - phase 2 (re-comment): `scene key removed — reverting to the locked
    startup scene 'crystal-dragon' (runtime was 'cinematic')` →
    `Cloud rebuilt — speed=30.00 density=0.780` (crystal-dragon profile
    returned)
  - phase 3 (unrelated `fps = 45`): `apply fps='45'` → `Cloud rebuilt —
    speed=30.00 density=0.780 fps=45.00` (scene stays locked)
  - Same script on the pre-v51.1 tree: phase 2 FAILS (stays cinematic,
    no revert) — bug reproduced, then fixed.
- **10s monolith 80x24 A/B** (machine drifted ±2% during the session, so
  a same-tree control pair was taken): visual parity — entropy -0.10%,
  gini +0.03%, color_transition identical, streams 23 identical, dirty
  -0.15%, allocs/deallocs bit-stable 563/553; avg_fps +2.69% vs the
  same-state control. Raw JSON in `benchmark/bench-labs/v51_1_cli_fallback/`.

## 5. LTS notes

- The live-reload layer now has ONE priority model (the v51.1 contract)
  instead of two half-shipped ones; every guard that remains is live in
  production and covered by a test that exercises the same path
  production takes (flags alive, no zeroing).
- `startup_cfg` is a one-time clone at startup (no per-frame cost; the
  rebuild path is reload-only, never on the render hot path).
- The revert event is traced (`scene key removed — reverting to the
  locked startup scene ...`) so future debug sessions see the fallback
  directly in the live-reload trace buffer.
- Docs synced: LIVE_RELOAD_BEHAVIOR.md section 1 + 6 + 11 + 12 + new
  section 13; CHANGELOG; the `CliExplicit` doc comments in app.rs,
  cli_explicit.rs, build_cloud_cfg.rs.
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
