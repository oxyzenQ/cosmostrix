# Contributing to cosmostrix
<!-- SPDX-License-Identifier: GPL-3.0-only -->

> cosmostrix is a professional-grade cinematic Matrix rain renderer built in Rust. Contributions are welcome — especially bug fixes, platform compatibility improvements, and documentation. See [TRADEMARK.md](TRADEMARK.md) §4a for the contribution fork policy.

## 1. Getting Started

**Prerequisites**: Rust 1.97.1 (pinned in `rust-toolchain.toml` — `rustup` auto-installs it); Git; Linux/macOS recommended (Windows works but some scripts need Git Bash); `shellcheck` (for the `./scripts/build.sh check-all` gatekeeper).

**Clone + Build**:

```bash
git clone https://github.com/oxyzenQ/cosmostrix.git
cd cosmostrix
cargo build                # debug build
cargo build --release      # optimized release build
```

**Verify your build**:

```bash
cargo test --all --locked      # run full test suite
./scripts/build.sh check-all   # full gatekeeper (fmt + clippy + test + audit)
```

All 11 gatekeeper checks must pass before any commit.

## 2. Coding Conventions

**Rust**: formatting via `cargo fmt --all` (enforced by gatekeeper); linting via `cargo clippy -- -D warnings` (all warnings are errors); source cap 1,500 LOC per `.rs` file (1,000 for `cloud/mod.rs` — split large files using the `#[path = "..."] mod` pattern); no production `unwrap()` (all `.unwrap()` calls must be in `#[cfg(test)]` modules — production code uses `?`, `Option`, or `match Ok/Err`); no `unsafe` without SAFETY comment (every `unsafe` block must document why it's sound).

**Shell scripts**: all `scripts/*.sh` must pass `shellcheck`; every file must have the copyright + SPDX header.

**Python scripts**: all `scripts/*.py` must pass `ruff check` + `ruff format --check`.

## 3. Commit + PR Guidelines

**Commit message format**:

```
type(scope): summary line

Body explaining what + why (not how).
```

Types: `fix`, `feat`, `refactor`, `docs`, `chore`, `perf`, `test`. Examples: `fix(visual): internal independent QA — H1 resize color cache`, `refactor(split-E1): extract sanitize_message_text from main.rs to src/message.rs`, `docs(bench): add v50 reference matrix`.

**Before committing**: (1) Run `./scripts/build.sh check-all` — all checks must pass; (2) Run `cargo fmt --all` if formatting issues; (3) Verify no debug `eprintln!` / `println!` in production code paths (use `push_runtime_warning` for diagnostics during rain — see AB-10).

**Pull request checklist**:

- [ ] All tests pass (`cargo test --all --locked`)
- [ ] Gatekeeper passes (`./scripts/build.sh check-all`)
- [ ] No new `unwrap()` in non-test code
- [ ] No new `unsafe` without SAFETY comment
- [ ] No `eprintln!`/`write_fmt` in rain-active code paths (use buffer)
- [ ] File LOC stays under 1,500 (1,000 for `cloud/mod.rs`)
- [ ] SPDX header on new files
- [ ] Commit message follows the format above

## 4. Forking Policy

- **Contribution forks** (PRs back to upstream): allowed without permission. Keep the cosmostrix name/logo/branding unchanged. See [TRADEMARK.md §4a](TRADEMARK.md).
- **Non-contribution forks** (rebrand, relaunch, derivative product): require owner discussion first. See [TRADEMARK.md §4b](TRADEMARK.md).

## 5. Architecture Quick Reference

| Subsystem | Module | Purpose |
|-----------|--------|---------|
| Cosmic Dragon | `src/cosmic_dragon_engine/frame.rs`, `src/cosmic_dragon_engine/terminal/`, `src/cosmic_dragon_engine/runtime.rs` | Diff-based rendering engine |
| Chroma Dragon | `src/chroma_dragon_engine/` | OKLab color engine |
| Cloud | `src/cosmic_dragon_engine/cloud/` | Rain simulation + spawn + render |
| Droplet | `src/droplet/mod.rs` | Per-droplet visual effects pipeline |
| Power | `src/central_control_dragon_power/` | Self-healer + power management |
| Ambient | `src/crystal_dragon_engine/ambient*/mod.rs` | Time-of-day scene scheduling |
| Live reload | `src/config/live_config*/mod.rs` | Config file watcher + rebuild |
| Interactive | `src/interactive/` | Event loop + HUD + input + intro |
| Config | `src/config/configfile.rs`, `src/config/*.rs` | TOML parser + validation |

Full audit: [`docs/audits/COSMIC_DRAGON_AUDIT.md`](docs/audits/COSMIC_DRAGON_AUDIT.md)

Copyright (C) 2026 rezky_nightky (oxyzenQ). All rights reserved.
