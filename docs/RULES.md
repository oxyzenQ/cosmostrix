# Cosmostrix Project Rules
<!-- SPDX-License-Identifier: GPL-3.0-only -->

## Source file size

All Rust source files under `src/` must stay **under 1,500 gross lines**.
This rule is enforced by `scripts/check-rs-loc.sh`, which runs as part of
`./scripts/build.sh check-all`.

### Scope

- Core source and code files: `src/**/*.rs`, `build.rs`
- Config and build files: `*.toml`, `.cargo/config.toml`, `rust-toolchain.toml`
- Scripts: `*.sh`, `scripts/*.sh`, `benchmark/*.sh`
- CI workflows: `.github/workflows/*.yml`, `.github/FUNDING.yml`

### Excluded

- Documentation: `*.md`, `docs/**/*.md`
- Text and media: `*.txt`, assets, images, videos
- Generated files: `Cargo.lock`, `target/`
- Git metadata: `.git/`

## Module organization

Prefer splitting modules by responsibility over allowing large files.

- `main.rs` should remain bootstrap and wiring only; target 100–300 LOC long-term.
- `cli.rs` may be larger if it contains mostly Clap command definitions, but must
  still stay under 1,500 LOC.
- Module directories (e.g. `src/cloud/`, `src/interactive/`) use `mod.rs` as the
  public entry point and split implementation into focused submodules.
- Tests are colocated with their module in dedicated `tests/` subdirectories.

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

All core, config, and script files must carry an SPDX license identifier.
See `scripts/check-headers.sh` for the enforced format.

## Code quality

- Clippy must pass with `-D warnings` (warnings are errors).
- `cargo fmt` must report no differences.
- All tests must pass on every commit.
- MSRV: Rust 1.81.0 (stable).

## Test discipline

Tests must verify **behavior**, never **identity**. A test assertion that
a constant value matches itself (tautology) provides zero information and
breaks the suite on every unrelated change.

### Forbidden: tautological version assertions

```rust
// FORBIDDEN — Cargo.toml always contains its own version field.
// Always true, zero information.
assert!(include_str!("../Cargo.toml").contains("version = \"5.0.1\""));

// FORBIDDEN — PKGBUILD/.SRCINFO/README contain their own version.
assert!(include_str!("../aur/cosmostrix-bin/PKGBUILD").contains("pkgver=5.0.1"));
assert!(include_str!("../aur/cosmostrix-bin/.SRCINFO").contains("pkgver = 5.0.1"));
assert!(include_str!("../README.md").contains(r#"TAG="v5.0.1""#));
```

### Allowed: dynamic version assertions

```rust
// ALLOWED — env!() injects the compile-time package version from
// Cargo.toml [package] version. Single source of truth.
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

assert!(include_str!("../Cargo.toml")
    .contains(&format!("version = \"{}\"", CURRENT_VERSION)));
assert!(include_str!("../aur/cosmostrix-bin/PKGBUILD")
    .contains(&format!("pkgver={}", CURRENT_VERSION)));
```

### Forbidden: test-on-test meta-pattern

Tests must not assert that **other test files** contain a particular
literal string. Every version bump would force manual edits across
multiple test files just to satisfy one meta-test.

```rust
// FORBIDDEN — tests that another test file contains a literal version.
let p14 = include_str!("ledger_p14_tests.rs");
assert!(p14.contains("3.1.0"), "p14 tests must assert 3.1.0");
```

### Allowed: historical CHANGELOG assertions

Asserting that a past release has an entry in CHANGELOG is legitimate —
those entries are immutable historical record and remain valid forever.

```rust
// ALLOWED — verifies CHANGELOG has an entry for a historical release.
let changelog = include_str!("../CHANGELOG.md");
assert!(changelog.contains("## v4.0.0"));
assert!(changelog.contains("## v5.0.0"));
```

### Enforcement

`scripts/check-version-anti-patterns.sh` (run by `build.sh check-all`)
scans `src/**/*.rs` for forbidden patterns and fails the build if any
are detected. The guard catches:

- `contains("version = \"X.Y.Z\"")` and `contains(r#"version = "X.Y.Z""#)`
- `contains("pkgver=X.Y.Z")` and `contains("pkgver = X.Y.Z")`
- `contains(r#"TAG="vX.Y.Z""#)` (README install tag)

If a future test genuinely needs the current package version, use
`env!("CARGO_PKG_VERSION")` — never hardcode the literal string.

## v15 Cosmic Dragon Architecture

The Cosmic Dragon release introduces several major subsystems. All new code must
follow these architectural rules.

### Atmosphere Engine (REMOVED 2026-08-05)

The atmosphere engine subsystem was fully eliminated at commit `07b44b5`
(Dragon Hunt v2 Phase 6 Tier E item 31 — final elimination). All
`src/atmosphere_*.rs` source files, all `--atmosphere-mode` /
`--atmosphere-regime` CLI flags, all `atmosphere-mode` /
`atmosphere-regime` / `adaptive-custom.*` config keys, and all
`atmosphere-*` scene-custom presets have been removed.

Historical reference (preserved verbatim, no longer describes live behavior):
- `docs/archive/specs/ATMOSPHERE_ENGINE.md` — v20 design spec
- `docs/archive/specs/CINEMATIC_BREATHING.md` — vocabulary spec
- `docs/archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md` — full
  elimination record (file list, KEPT-vs-DELETED table, backward-compat
  notes, revival guidance)

Subsystems that still share the "atmosphere" name but were NOT deleted
(because they are separate subsystems, not the v4.0.0 atmosphere engine):
- `src/chroma/post/climate.rs` — Chroma Dragon post-FX shader
  (luminance/saturation/instability). Used by
  `chroma::shaders::base::resolve_cell_color` for every cell render.
- `AtmosphericEvolution` struct in `src/cloud/ecosystem.rs` — cloud
  drift/gust events (entropy_phase, density_offset, luminance_offset,
  anomaly_offset, cycle_speed).

### Live Config Reload

- Watches `config.toml` via `notify` crate (background thread).
- Full Cloud rebuild on change (not delta apply).
- Strict validation: malformed lines, unknown keys, invalid values → exit 2.
- Error message printed to stderr AFTER terminal restore (not during rain).
- Modules: `live_config.rs`, `testconf.rs` (shared validation).

### Config Validation

- `--testconf` validates all keys + values strictly.
- Startup: rejects invalid config (exit 2, same as --testconf).
- Live reload: rejects invalid config (exit 2, error printed after exit).
- Malformed lines (no `=` or empty key/value) → error.
- Unknown keys → error.
- Invalid values (out of range, unknown enum) → error.
- No silent fallback. No warnings. Errors only.

### CLI Flag Policy (v14+)

- Quit: only `q` exits. Esc, Ctrl+C, Ctrl+Z (in-app), Tab/BackTab, and all
  other unrecognized keys are silently ignored (fall through to the
  `_ => {}` catch-all in `handle_keybinding`). v25.13: SIGINT (Ctrl+C) is
  deprecated at the signal level too — only SIGTERM/SIGHUP/SIGQUIT trigger
  graceful shutdown. v30: in-app Ctrl+Z suspend keybind was removed
  (terminal-driven SIGTSTP still works via `signal_handlers.rs`); the
  explicit Tab/BackTab no-op arm was removed (now falls through to
  catch-all). The user must press `q` deliberately to quit.
- Active runtime keybinds (the complete set, see `--help` RUNTIME
  CONTROLS):
  - `q`              Quit
  - `Space`          Reset animation + restart message typewriter
  - `c` / `C`        Cycle color scheme forward / backward
  - `s` / `S`        Cycle charset preset forward / backward
  - `p`              Pause / resume
  - `x`              Cycle scene forward (uppercase `X` is a no-op since v30)
  - `Up` / `Down`    Speed up / slow down
  - `[` / `]`        Density down / up
  - `i`              Toggle live HUD (uppercase `I` is a no-op since v30)
  - `h`              Move HUD to opposite corner (uppercase `H` is a no-op since v30)
- Screensaver mode: all the above keys work normally. Only `q` exits.
- Removed legacy keybinds (now silently ignored via catch-all, were
  never documented in `--help`):
  - v30: `-` `_` `+` `=` (density aliases for `[` / `]`)
  - v30: `Ctrl+Z` (in-app suspend — OS SIGTSTP still works)
  - v30: `Tab` / `BackTab` explicit no-op arm (now catch-all; historical
    shading-mode toggle that caused phosphor ghost flood — see
    `tests.rs::tab_*` regression suite)
  - Stale doc references to `a`, `m`, `g`, `b`/`B` as "interactive" keys
    were purged from RULES.md, COSMIC_DRAGON_ARCHITECTURE.md, README.md,
    and inline comments — these were never active keybinds in v30.
- Removed flags (each has a migration error produced by the `REMOVED_FLAGS`
  table in `src/validation.rs` that intercepts the flag before clap parsing):
  - v14.0.0: `--preset`, `--profile`, `--low-power`, `--list-presets`,
    `--list-profiles`, `--show-preset`, `--dump-profile`, `--list-colors-detail`,
    `--defaults`, `--tune-visual`
  - v15.0.0: `--completions <shell>` (clap_complete dependency dropped)
  - v17.0.0: `--mouse` (effects always on; flag removed), `--info` / `-i`
    (merged into `--doctor`), `--async` / `-a` (async always on; use
    `--uniform` to disable), `--brightness` / `--saturation` (replaced by
    `--color-tune`), `--glitchpct` / `--shortpct` / `--rippct` / `--maxdpc`
    (replaced by `--glitch-level`)
  - v25.0.0: `--charset-file <path>` (replaced by `[charset-custom.<name>]`
    config blocks loaded via `--charset <name>`)
  - v25.0.0-alpha.3: `--fullwidth` (legacy horizontal-spacing mode purged)
- Android/Termux: accept Press + Repeat key events (skip Release).

### Density Map

- Per-column spawn probability weights (0.0-1.0) for monolith pillar formation.
- Config: `scene-custom.<name>.density-map = 0.1,0.5,1.0,...`
- Generator: `scripts/gen-density-presets.py` (twin-towers, cascade, throne).
- Rejection sampling in `find_inactive_lane()`.

### Config Path Whitelist (Security)

- Linux: `~/.config/cosmostrix/`, `/etc/cosmostrix/`
- macOS: `~/.config/cosmostrix/`, `~/Library/Application Support/cosmostrix/`, `/etc/cosmostrix/`
- Windows: `%APPDATA%\cosmostrix\`, `%ProgramData%\cosmostrix\`
- Rejected: current directory, `/tmp/`, `~/.local/`, `/usr/`, all others.
- Enforced by `safepath.rs`.

### Verbose Output

- Startup: full config dump to stderr (no borders, purple brand color).
- Runtime: changes tracked silently (no eprintln during rain — causes flicker).
- After exit: final runtime state printed if any value changed.
- Format: `[verbose] field: value (was old_value)`

### Install Script

- `./scripts/install` auto-detects CPU: AVX-512 → pro-linux-v4, AVX2 →
  pro-linux-v3, baseline → release.
- `--system` flag: install to `/usr/bin`. Default: `~/.local/bin`.

