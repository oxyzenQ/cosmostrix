# cosmostrix Project Rules
<!-- SPDX-License-Identifier: GPL-3.0-only -->

## Source file size

All Rust source files under `src/` must stay **under 1,500 gross lines**. Enforced by `scripts/check-rs-loc.sh`, run as part of `./scripts/build.sh check-all`.

**Scope**: `src/**/*.rs`, `build.rs`, `*.toml`, `.cargo/config.toml`, `rust-toolchain.toml`, `*.sh`, `scripts/*.sh`, `benchmark/*.sh`, `.github/workflows/*.yml`, `.github/FUNDING.yml`. **Excluded**: `*.md`, `docs/**/*.md`, `*.txt`, assets, images, videos, `Cargo.lock`, `target/`, `.git/`.

## Module organization

Prefer splitting modules by responsibility over allowing large files. `main.rs` should remain bootstrap and wiring only (target 100–300 LOC long-term). `cli.rs` may be larger if it contains mostly Clap command definitions, but must stay under 1,500 LOC. Module directories (e.g. `src/cosmic_dragon_engine/cloud/`, `src/interactive/`) use `mod.rs` as the public entry point and split implementation into focused submodules. Tests are colocated with their module in dedicated `tests/` subdirectories.

## Validation

Behavior-preserving refactors must pass the full validation suite:

```bash
scripts/check-rs-loc.sh
scripts/check-headers.sh
cargo fmt --all
cargo test --all --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
./scripts/build.sh check-all
```

## License headers

All core, config, and script files must carry an SPDX license identifier. See `scripts/check-headers.sh` for the enforced format.

## Code quality

- Clippy must pass with `-D warnings` (warnings are errors).
- `cargo fmt` must report no differences.
- All tests must pass on every commit.
- MSRV: Rust 1.98.0 (pinned in `rust-toolchain.toml`).

## Test discipline

Tests must verify **behavior**, never **identity**. A tautological assertion (a constant matching itself) provides zero information and breaks the suite on every unrelated change.

**Forbidden** (tautological version assertions — Cargo.toml/PKGBUILD/README always contain their own version):

```rust
assert!(include_str!("../Cargo.toml").contains("version = \"5.0.1\""));
assert!(include_str!("../aur/cosmostrix-bin/PKGBUILD").contains("pkgver=5.0.1"));
assert!(include_str!("../README.md").contains(r#"TAG="v5.0.1""#));
```

**Allowed** (dynamic, via `env!()` — single source of truth):

```rust
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
assert!(include_str!("../Cargo.toml").contains(&format!("version = \"{}\"", CURRENT_VERSION)));
assert!(include_str!("../aur/cosmostrix-bin/PKGBUILD").contains(&format!("pkgver={}", CURRENT_VERSION)));
```

**Forbidden**: test-on-test meta-pattern — tests must not assert that *other test files* contain a particular literal string (e.g. `assert!(p14.contains("3.1.0"))`). Every version bump would force manual edits across multiple test files just to satisfy one meta-test.

**Allowed**: historical CHANGELOG assertions (e.g. `assert!(changelog.contains("## v13.0.0"))`, `assert!(changelog.contains("## v50.0.0-alpha.5"))`) — those entries are immutable historical record and remain valid forever.

**Enforcement**: `scripts/check-version-anti-patterns.sh` (run by `build.sh check-all`) scans `src/**/*.rs` for forbidden patterns and fails the build if detected: `contains("version = \"X.Y.Z\"")`, `contains("pkgver=X.Y.Z")`, `contains(r#"TAG="vX.Y.Z""#)`. If a future test genuinely needs the current package version, use `env!("CARGO_PKG_VERSION")` — never hardcode the literal string.

## Cosmic Dragon Architecture

### Atmosphere Engine (REMOVED 2026-08-05)

Fully eliminated at commit `07b44b5` (Dragon Hunt v2 Phase 6 Tier E item 31). All `src/atmosphere_*.rs` source files, `--atmosphere-mode` / `--atmosphere-regime` CLI flags, `atmosphere-mode` / `atmosphere-regime` / `adaptive-custom.*` config keys, and `atmosphere-*` scene-custom presets have been removed. Historical reference: `docs/archive/specs/ATMOSPHERE_ENGINE.md` (design spec), `docs/archive/specs/CINEMATIC_BREATHING.md` (vocabulary spec), `docs/archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md` (full elimination record). Subsystems still sharing the "atmosphere" name but NOT deleted (separate subsystems): `src/chroma_dragon_engine/post/climate/mod.rs` (Chroma Dragon post-FX shader), `AtmosphericEvolution` struct in `src/cosmic_dragon_engine/cloud/ecosystem.rs` (cloud drift/gust events).

### Live Config Reload + Config Validation

Watches `config.toml` via `notify` crate (background thread). Full Cloud rebuild on change (not delta apply). `--testconf` validates all keys + values strictly. Startup rejects invalid config (exit 2). Live reload rejects invalid config (exit 2, error printed to stderr AFTER terminal restore). Malformed lines (no `=` or empty key/value) → error. Unknown keys → error. Invalid values → error. No silent fallback. No warnings. Errors only. Modules: `live_config.rs`, `testconf.rs` (shared validation).

### CLI Flag Policy

- **Quit**: only `q` exits. Esc, Ctrl+C (SIGINT deprecated — only SIGTERM/SIGHUP/SIGQUIT trigger graceful shutdown), Ctrl+Z (in-app suspend removed; OS SIGTSTP still works), Tab/BackTab, and all other unrecognized keys are silently ignored (catch-all `_ => {}` in `handle_keybinding`).
- **Active runtime keybinds** (complete set, see `--help` RUNTIME CONTROLS):
  - `q` Quit · `Space` Reset animation + restart message typewriter · `c`/`C` cycle color scheme fwd/back · `s`/`S` cycle charset preset fwd/back · `p` pause/resume · `x` cycle scene forward (`X` no-op) · `Up`/`Down` speed up/down · `[`/`]` density down/up · `i` toggle live HUD (`I` no-op — see `docs/HUD.md`)
- **Screensaver mode**: all the above keys work normally. Only `q` exits.
- **Removed legacy keybinds** (silently ignored via catch-all, were never in `--help`): `-` `_` `+` `=` (density aliases for `[` / `]`); `Ctrl+Z` (in-app suspend); `h` (HUD position toggle — **completely removed**, no binding exists, silently ignored; HUD always renders flush-left at column 0 per v50.0.0-beta.6; HUD visibility is toggled with `i`, not `h`); `Tab`/`BackTab` explicit no-op arm (now catch-all; historical shading-mode toggle that caused phosphor ghost flood — see `tests.rs::tab_*`). Stale doc references to `a`, `m`, `g`, `b`/`B` as "interactive" keys were purged — these were never active.
- **Removed flags** (each has a migration error in `src/validation/mod.rs` `REMOVED_FLAGS` table): v14.0.0 (`--preset`, `--profile`, `--low-power`, `--list-presets`, `--list-profiles`, `--show-preset`, `--dump-profile`, `--list-colors-detail`, `--defaults`, `--tune-visual`); v15.0.0 (`--completions <shell>`); v17.0.0 (`--mouse`, `--info`/`-i`, `--async`/`-a`, `--brightness`/`--saturation`, `--glitchpct`/`--shortpct`/`--rippct`/`--maxdpc`); v25.0.0 (`--charset-file <path>`); v25.0.0-alpha.3 (`--fullwidth`).
- **Android/Termux**: accept Press + Repeat key events (skip Release).

### Density Map + Config Path Whitelist (Security)

**Density map**: per-column spawn probability weights (0.0-1.0) for monolith pillar formation. Config: `scene-custom.<name>.density-map = 0.1,0.5,1.0,...`. Generator: `scripts/gen-density-presets.py` (twin-towers, cascade, throne). Rejection sampling in `find_inactive_lane()`.

**Config path whitelist** (enforced by `safepath.rs`): Linux `~/.config/cosmostrix/`, `/etc/cosmostrix/`; macOS `~/.config/cosmostrix/`, `~/Library/Application Support/cosmostrix/`, `/etc/cosmostrix/`; Windows `%APPDATA%\cosmostrix\`, `%ProgramData%\cosmostrix\`. Rejected: current directory, `/tmp/`, `~/.local/`, `/usr/`, all others.

### Verbose Output + Install Script

**Verbose**: startup dumps full config to stderr (no borders, purple brand color). Runtime: changes tracked silently (no eprintln during rain — causes flicker). After exit: final runtime state section always prints (v50.0.0-beta.6) — first line is `exit_time: <YYYY-MM-DD HH:MM:SSZ>` (UTC, ISO 8601) and `duration: <Xm Ys>` showing the total process lifetime. UTC chosen for LTS stability (no DST transitions, no tzdata drift). Changed live-reload fields follow (only if any value changed during the session). Format: `[verbose] field: value (was old_value)`. The section closes with the ambient diagnostics summary.

**Install**: `./scripts/install` auto-detects CPU — AVX-512 → pro-linux-v4, AVX2 → pro-linux-v3, baseline → release. `--system` flag: install to `/usr/bin`. Default: `~/.local/bin`.

### Naming Collision Policy (v50.0.0-beta.6 Option D)

When a custom config block (`[charset-custom.<name>]`, `[colors-custom.<name>]`, `[scene-custom.<name>]`) has the same name as a builtin preset/scene/theme, **custom always wins**. A collision warning is emitted to stderr at startup so the user knows the builtin is being shadowed:

```
⚠ warning: custom charset 'zen' overrides builtin — custom wins (Option D policy)
  builtin: builtin preset (see --list-charsets)
  custom:  1 char(s) from [charset-custom.zen]
  To use the builtin, rename the custom block in config.toml.
```

This policy is consistent across all 3 systems (charset, colors, scene). Previously they had inconsistent behavior: charset was custom-wins, colors was builtin-wins, scene was builtin-wins. Now all three are custom-wins with visible warning. The user can always use the explicit flags (`--colors-custom`, `--scene-custom`, `--charset-custom`) for unambiguous intent.

### Custom Block LTS Bounds (v50.0.0-beta.6)

All 3 custom config systems use the same bounds for consistency. Max **100 blocks** per category (generous — built-in themes are ~44, built-in scenes ~10, built-in charsets ~25). Max **64 char names** (built-in names are ≤16 chars; longer = likely typo).

**Unified bounds table:**

| System | Max blocks | Max name len | Max content | Rationale |
|--------|-----------|--------------|-------------|-----------|
| colors-custom | 100 (`COLORS_CUSTOM_MAX_BLOCKS`) | 64 (`COLORS_CUSTOM_MAX_NAME_LEN`) | 64 rain stops (`COLORS_CUSTOM_MAX_RAIN_STOPS`) | OKLab engine only needs 2-16 stops; 100 blocks far exceeds realistic use |
| charset-custom | 100 (`CHARSET_CUSTOM_MAX_BLOCKS`) | 64 (`CHARSET_CUSTOM_MAX_NAME_LEN`) | 256 chars (`CHARSET_CUSTOM_MAX_LEN`) | Bounded glyph pool; prevents 10K-char paste bloat |
| scene-custom | 100 (`SCENE_CUSTOM_MAX_BLOCKS`) | 64 (`SCENE_CUSTOM_MAX_NAME_LEN`) | N/A (fields are key-value) | 100 blocks far exceeds realistic use (built-in scenes ~10) |

When a cap is hit, behavior depends on the cap type:

- **Content cap** (rain stops, charset chars): emits a runtime warning via `push_runtime_warning` (drained after Terminal::drop so it doesn't leak into the rain matrix). Example: `colors-custom: rain stops capped at 64 (extra stops ignored)`.
- **Block cap** (total blocks per category): silently skipped (no warning — the user would have to define 100+ blocks to hit this, which is almost certainly a script-generated config, not a human typo).
- **Name length cap**: silently skipped (no warning — almost certainly a typo, warning would be noise).

All 3 systems are now aligned: same max blocks (100), same max name len (64), same skip semantics. This makes the LTS contract predictable across colors, charset, and scene custom blocks.

### Dynamic `dsty:` Metric (v50.0.0-beta.6 Option D)

The `dsty:` HUD metric (row 9) is **dynamic when power-dragon is ON** — it reflects the effective density after power-dragon throttle. When power-dragon is OFF, `dsty:` is **static** (shows the user's configured density, no throttle applied).

**How it works:**

- `dsty:` = `user_density * compute_spawn_scale(pressure, aggressive)`
- `compute_spawn_scale()` is a shared function (`central_control_rains.rs`) — the **same function** used by `rain_at()` in the render path. No formula drift.
- `pressure` = `power_manager.effective_pressure()` (0.0–1.0)
- `aggressive` = `cloud.aggressive_throttle` (set by self-healer on sustained high CPU)

**Behavior table:**

| State | `dsty:` shows | Example |
|-------|--------------|---------|
| power-dragon OFF | user density (static) | `dsty: 0.75` |
| power-dragon ON, no pressure | user density (full) | `dsty: 0.72` |
| power-dragon ON, 50% pressure | throttled | `dsty: 0.45` (0.72 * 0.625) |
| power-dragon ON, 100% pressure | floored | `dsty: 0.18` (0.72 * 0.25) |
| power-dragon ON + aggressive | drops harder | `dsty: 0.40` (0.72 * 0.55) |
| CLI `--density 1.0` + max pressure | CLI caps, throttle reduces | `dsty: 0.25` (1.0 * 0.25) |

**CLI wins:** the user's configured density is the **ceiling** — the throttle only reduces below it (scale ≤ 1.0), never above it. So `--density 1.0` with max pressure shows `dsty: 0.25`, not `1.0`.

Custom blocks have a **strict field allowlist** — unknown fields are rejected as errors, NOT auto-promoted to root scope. This prevents silent side-effects like `color = green` inside `[charset-custom.quantum]` changing the global color scheme.

**Allowed fields per block type:**

| Block | Allowed fields | Source |
|-------|---------------|--------|
| `[colors-custom.<name>]` | `bg`, `rain`, `stops` (deprecated alias) | `is_valid_colors_custom_field()` |
| `[charset-custom.<name>]` | `set` only | `is_valid_charset_custom_field()` |
| `[scene-custom.<name>]` | `base-scene`, `color`, `charset`, `bold`, `colors-custom`, `charset-custom`, `shadingmode`, `glitch-level`, `fps`, `speed`, `density`, `density-map`, `async-mode` | `SCENE_CUSTOM_FIELDS` |

Any other field inside these blocks surfaces as an `unknown_key` → `--testconf` reports the error, live-reload rejects the config. The auto-promote path (which previously moved top-level keys like `color`/`intro`/`speed` from inside a custom block to root scope) is **disabled** when `current_section` starts with `charset-custom.`, `colors-custom.`, or `scene-custom.`.

Auto-promote still works for non-custom sections (e.g. `[color.tune]` — a top-level key accidentally nested under it still promotes to root). Only custom blocks are strict.
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
