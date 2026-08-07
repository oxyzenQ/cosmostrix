<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmostrix Future Backlog

**Purpose**: Parking lot for **new CLI flags / parameters** that were
intentionally NOT added during the v30 stabilization audit. v30 froze the
config surface; items here are saved for a future session when the owner
wants to evolve the surface again.

**Owner**: oxyzenQ
**Last updated**: 2026-08-04

## Audit Closure Status

All 39 findings from the 5-phase CONFIG_SYNC audit are CLOSED (100%).
Phase 6 dead-code sweep: 0 dead code found. Miri unsafe soundness pass:
0 unsoundness. `pub → pub(crate)` tightening: 580 warnings → 0.
Unused-deps audit (`cargo-machete`): 0 unused deps.

Full phase reports have been archived to
[`docs/archive/CONFIG_SYNC/`](../archive/CONFIG_SYNC/) and the
unsafe soundness audit to
[`docs/archive/audits/UNSAFE_SOUNDNESS_AUDIT.md`](../archive/audits/UNSAFE_SOUNDNESS_AUDIT.md).

The items below are **NEW FLAG/PARAMETER IDEAS** only — parked per owner
instruction ("don't make new flags or parameters yet because this will
become the long-term stabilization version"). The underlying CONCERNS
that motivated each flag have already been addressed in v30 via non-flag
approaches (doc comments, warning summaries, code fixes).

## Parked Flag Ideas

### `--strict-config` (from P3-5)

Promote soft `[config] warning: ...` lines to hard errors (exit 1).
v30 closure: added a startup warning SUMMARY line at the end of config
apply (commit `6fd7380`), so warnings are visible without a new flag.
**Reopen cost**: ~2h. Touches `configfile.rs::warn_invalid` (5 sites) +
`config.rs` + `main.rs`.

### `--strict-profiles` (from P2-4)

Make `[profile.<name>]` and `[scene-custom.<name>]` strict-reject like
top-level config (currently they warn-and-continue). v30 closure:
documented the divergence as intentional — profiles are override
collections and rejecting the entire config because one profile has a
typo would be hostile. (The historical reference doc that recorded this
decision was `ATMOSPHERE_ENGINE.md`, now archived at
`docs/archive/specs/ATMOSPHERE_ENGINE.md`. The intentional-divergence
rationale still applies post-elimination.)
**Reopen cost**: ~1.5h. Touches `profile.rs` + `scene_custom.rs` + clap.

### `--no-adaptive-custom-when-disabled` (from P1-#2) — CLOSED/OBSOLETE

~~Suspend `[adaptive-custom.HH-MM]` entries when `atmosphere-mode =
disabled`.~~ **OBSOLETE 2026-08-05**: both `atmosphere-mode` and
`adaptive-custom.*` config keys were eliminated at commit `07b44b5`
along with the entire atmosphere engine subsystem. There is no longer an
`adaptive-custom.*` schedule to suspend and no `atmosphere-mode` switch
to gate against. This item is closed as obsolete — do not reopen.

### `--testconf-adaptive-custom` (from P3-4) — CLOSED/OBSOLETE

~~Standalone flag to validate ONLY `[adaptive-custom.*]` blocks without
the full `--testconf` pass.~~ **OBSOLETE 2026-08-05**: `adaptive-custom.*`
config keys were eliminated at commit `07b44b5`. `--testconf` now
rejects any `adaptive-custom.*` entry with a clear migration message
("the atmosphere engine subsystem was eliminated"). There is no
remaining surface that this flag would target. Closed as obsolete.

### Case-insensitive enum unification (from P2-6, P1-#4)

Not a flag — was a code fix. v30 closure: made `testconf.rs`
case-insensitive for the 4 affected enums (intro, monolith-size,
glitch-level, color-bg) by adding `.to_ascii_lowercase()` (commit
`76115d4`). All 3 paths (CLI, testconf, runtime) now agree. Kept here
only for historical reference.

## How to Reopen an Item

1. Find the item by ID (e.g. "P3-5 `--strict-config`").
2. Read the original Phase report in
   [`docs/archive/CONFIG_SYNC/`](../archive/CONFIG_SYNC/) for full
   context (file:line citations).
3. Decide: implement / defer again / close as won't-fix.
4. If implement: create a branch, write the code, run
   `./scripts/build.sh check-all`, commit + push.
