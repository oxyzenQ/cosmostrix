<!-- SPDX-License-Identifier: GPL-3.0-only -->

# src/ Root Rules — Single-File Policy

> **Owner mandate 2026-08-19**: `src/` root must contain ONLY `main.rs`.
> All other modules live in subdirectories. This rule exists to keep the
> codebase navigable for beginners, contributors, and maintainers.

## The Rule

**`src/` root must contain exactly one `.rs` file: `main.rs`.**

No new `.rs` files may be placed directly at `src/` root. All new modules
MUST be organized into subdirectories.

## Why

- **Navigability**: A new contributor opening `src/` sees a single entry
  point (`main.rs`) + logically grouped directories. They don't need to
  scan 30+ flat files to understand the codebase structure.
- **Maintainability**: Each directory is a self-contained subsystem with
  its own `mod.rs` + submodules + `tests.rs`. Adding a new feature means
  adding to an existing directory or creating a new one — never adding a
  flat file at the root.
- **Consistency**: All existing modules already follow this pattern (bench/,
  clock/, config/, cosmic_dragon_engine/, chroma_dragon_engine/,
  crystal_dragon_engine/, etc.). This rule codifies the convention.

## Current Structure (2026-08-19)

```
src/
├── main.rs                    <- ONLY .rs file at root (entry point)
├── RULES.md                   <- this file
├── bench/                     <- benchmark subsystem (17 bench_*.rs files)
├── bolt/                      <- cross-cutting utility module
├── central_control_power_dragon/  <- power management, self-healer, thermal
├── central_control_rains/     <- rain visual tuning constants
├── chroma_dragon_engine/      <- coloring engine (palette, shaders, post-FX)
├── cli/                       <- CLI args, parsing, help, app struct, UX contract (ux.rs)
├── clock/                     <- wall-clock helpers (Howard Hinnant style)
├── config/                    <- config.toml parsing, live-reload, hints
├── cosmic_dragon_engine/      <- rendering engine (cloud, frame, terminal, runtime)
├── cosmic_dragon_incubator/   <- experimental / concluded work
├── crystal_dragon_engine/     <- ambient intelligence (palette drift, scheduler)
├── diagnostics/               <- diagnostics, alloc_trace, info, humanize
├── docs_tests/                <- integration tests for docs/README consistency
├── doctor/                    <- --doctor diagnostics subsystem
├── droplet/                   <- droplet rendering (parallax, brightness)
├── interactive/               <- event loop, HUD, input handling (intro runner glue: event_loop_intro.rs)
├── intro_style/               <- cinematic intro styles (one file per style: cosmic.rs / logo.rs + mod.rs dispatch)
├── msg_fill_style/            <- message overlay reveal styles (one file per style: typewriter/fade/words/slide/pulse/instant/engrave.rs + mod.rs dispatch)
├── output/                    <- output, report, verbose, message (ux.rs moved to cli/)
├── platform/                  <- platform detection, panic hook, update
├── safepath/                  <- path validation (security)
├── scene/                     <- scene + charset + charset_custom
├── scene_custom/              <- custom scene definitions
├── sysstat/                   <- CPU/memory/env/usage stats
├── termdetect/                <- terminal capability detection
├── testconf/                  <- --testconf validation
├── tests/                     <- crate-level integration tests
├── theme/                     <- theme/cosmostrix-pro theme system
├── types/                     <- constants, cell, rain_style, renderer_info
└── validation/                <- input validation
```

## How to Add a New Module

1. **Create a new subdirectory**: `src/<module_name>/`
2. **Create `mod.rs`**: `src/<module_name>/mod.rs` with the module's code
   (or a thin re-export shim if the module has submodules)
3. **Declare in `main.rs`**: `mod <module_name>;` (+ `pub(crate) use
   <module_name>::{...};` if re-exporting submodules)
4. **Add tests**: `src/<module_name>/tests.rs` (Pattern C — dedicated
   tests/ file)

**NEVER**: create `src/<module_name>.rs` as a flat file at root.

## Exceptions

There are NO exceptions. Even small utility modules (e.g., `bolt/` at 225
LOC) live in their own directory. The cost of one extra directory is
negligible; the benefit of a flat-file-free root is significant.

## Re-export Pattern

When a module needs to be accessible as `crate::<name>::Foo` from other
modules, use the re-export pattern in `main.rs`:

```rust
// main.rs
mod my_group;
pub(crate) use my_group::{submodule_a, submodule_b};
```

This allows `crate::submodule_a::Foo` to resolve even though the file
lives at `src/my_group/submodule_a.rs`. All existing call sites continue
to work unchanged.

## Enforcement

- **Code review**: PRs that add `.rs` files to `src/` root must be rejected.
- **CI**: the `check-rs-loc.sh` script can be extended to verify root
  cleanliness (future enhancement).
- **This file**: serves as the canonical reference for the policy.

## File Permission Rule (v50.0.0-beta.4)

**Standard Unix file permissions for cosmostrix**:

| Type | Mode | Examples |
|------|------|----------|
| Directories | 755 | `src/`, `docs/`, `scripts/`, `benchmark/` |
| Shell scripts | 755 | `*.sh` (build.sh, gate-keepers.sh, install.sh, etc.) |
| Python scripts | 755 | `*.py` (bench-runner.py, visual-mode-audit.py, etc.) |
| Rust source | 644 | `*.rs` |
| Config files | 644 | `*.toml`, `*.yaml`, `*.yml`, `*.json` |
| Documentation | 644 | `*.md`, `*.txt` |
| Lockfiles | 644 | `Cargo.lock` |
| Assets | 644 | `*.png`, `*.gif`, `*.csv` |
| Binaries | 755 | compiled `cosmostrix` binary (not tracked in git) |

**For AI agents** (per DeepSeek advice):

> When modifying file permissions, set folders to 755, files to 644,
> and binaries to 755. NEVER use `chmod 777` or `chmod 755 -R` on
> everything. Use `git update-index --chmod=-x <file>` to fix tracked
> files that were accidentally marked executable.

**How to verify**: `git ls-files --stage | awk '$1 == "100755" {print $4}'`
should only show `.sh` and `.py` files. Any other file at 100755 is a
bug — fix with `git update-index --chmod=-x <file>`.
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
