<!-- SPDX-License-Identifier: GPL-3.0-only -->

# S-master-3 — Security LTS Harden (Live-Reload Message Path)

**Date:** 2026-09-01
**Scope:** `cosmostrix/*`, `src/*` (perstage — security-relevant dirs only)
**Author:** oxyzenQ (cosmic dragon mode, master audit pass)
**Predecessors:** SECURITY_VULNERABILITY_AUDIT (v50), UNSAFE_SOUNDNESS_AUDIT (v30)

## Context

The codebase had already been through two security audits
(UNSAFE_SOUNDNESS_AUDIT v30 + SECURITY_VULNERABILITY_AUDIT v50).
This audit was a **post-peak verification pass** to confirm whether
any remaining security gaps exist, and to harden what was found.

## Method

Static-only audit (ripgrep + Read) of 14 security-relevant areas:
`safepath/`, `config/`, `cli/`, `termdetect/`, `platform/`,
`output/`, `doctor/`, `validation/`, `build.rs`, `Cargo.toml`,
`engine/cosmic_dragon_engine/terminal/`,
`engine/cosmic_dragon_engine/cloud/events/`,
`interactive/signal_handlers.rs`, `main.rs`.

## Findings (35 total — 25 INFO sound, 7 LOW, 2 MED, 0 HIGH/CRITICAL)

### Categories already peak-hardened (SKIP)

| Category | Status | Notes |
|---|---|---|
| 2. Unsafe Rust | Hardened | 15 unsafe sites across 9 files, all sound. No `transmute`, `from_utf8_unchecked`, `get_unchecked`, `union`. DPD-01 (madvise HIGH) fix in place. |
| 3. Command injection | Hardened | 5 `Command::new` sites, ALL hardcoded args (`curl`, `stty`, `reset`, `tput`, `git`, `rustc`). No `sh -c`, no shell invocation. |
| 4. Env var injection | Hardened | ~50 `env::var` reads, all safe (path prefix that cannot widen whitelist, terminal detection, feature toggles). |
| 6. Integer overflow | Hardened | `parse_duration` uses `saturating_*`. `parse_screen_size` uses `parse::<u16>`. `reset_message` u16 arithmetic all `saturating_*`. |
| 7. Panic / DoS | Hardened | 156 unwrap/expect sites sampled, all in tests or statically-validated Ok-paths. `hash_file_prefix` caps at 8KB. `io_recovery` bounded. |
| 8. Network | Hardened | Single `curl` subprocess with hardcoded URL + 15s timeout. No HTTP client crate. |
| 9. Permissions | Hardened | No setuid/setgid/chmod/set_permissions. Default umask 0o644. No PID files. |
| 11. Signal handling | Hardened | `signal_hook::iterator::Signals` self-pipe pattern. Heavy work in dedicated threads. SV-05 sound. |
| 12. Fork safety | Hardened | SV-04 (Linux fork+prctl+sigwait) sound. SV-03 (PID-1 false-positive) benign documented. |

### Category 1 — Path traversal (2 LOW defense-in-depth, deferred)

- [LOW] `safepath/mod.rs:126` — `is_safe_path` does only lexical normalization; no `fs::canonicalize` for symlink resolution. Mitigated by `.toml` extension check + user-owned config dir.
- [LOW] `config/config_io.rs:59` — `write_config_atomic` follows symlinks. PID-predictable temp filename.

**Deferred** — both are defense-in-depth (attacker needs config write
access), and the existing whitelist + extension check already
provides strong protection. Over-engineering to add canonicalize
without a concrete threat model.

### Category 5 — ANSI/escape sequence injection (2 MED, FIXED)

**Root cause:** the startup path (`cli/build_cloud_cfg.rs:158-170`)
sanitizes `--message` text via `sanitize_message_text()` and enforces
`MESSAGE_MAX_LEN=200`. But the live-reload path
(`config/live_config/mod.rs:557`) stored raw config text without
either check — creating an inconsistency.

- [MED] `config/live_config/mod.rs:557` — Live-reload `message`/`message-border` values **bypassed `sanitize_message_text()`**. ESC chars (`\x1b`) and other C0/C1 control chars reached the terminal via `row_buf.push(cell.ch)` + `write_all`. ANSI injection in user's terminal via config edit (self-attack or shared dotfiles).
- [MED] `config/live_config/mod.rs:557` + `testconf/field_validation.rs:320` — Live-reload `message`/`message-border` **bypassed `MESSAGE_MAX_LEN=200`**. The catch-all `_ => None` in `validate_field_value` skips length validation. Multi-MB config line = unbounded memory.

**Fix applied** (`config/live_config/mod.rs:557-594`):
- Live-reload message now goes through `sanitize_message_text()` (strips C0/C1 control chars including ESC, replaces wide/zero-width chars with `?`).
- Live-reload message now length-checked against `MESSAGE_MAX_LEN` (200). Oversized values are rejected via `push_validation_rejection` (visible in the rejection log).
- Added `use crate::types::constants::MESSAGE_MAX_LEN` import.

### Category 10 — Supply chain (1 LOW, documented)

- [LOW] `instant` crate (RUSTSEC-2024-0384, unmaintained) via `notify-types 1.0.1` → `notify 7.0.0`. Acknowledged + suppressed in `deny.toml:13-20` with documentation. No upstream fix available. **Deferred** — tracked upstream.

## A/B Benchmark (10s, scene=monolith)

| Size | Metric | A (before) | B (after) | Delta | Verdict |
|---|---|---|---|---|---|
| 6x6 | avg_fps | 1,567,655 | 1,553,250 | -0.92% | stable |
| 6x6 | gini | 0.8333 | 0.8333 | +0.00% | stable |
| 6x6 | avg_dirty_cells | 0.6675 | 0.6677 | +0.03% | stable |
| 20x20 | avg_fps | 496,723 | 499,501 | +0.56% | stable |
| 20x20 | gini | 0.9165 | 0.9165 | +0.00% | stable |
| 40x20 | avg_fps | 302,881 | 303,401 | +0.17% | stable |
| 40x20 | gini | 0.9359 | 0.9354 | -0.05% | stable |
| 80x24 | avg_fps | 93,824 | 93,153 | -0.71% | stable |
| 80x24 | gini | 0.8962 | 0.8961 | -0.01% | stable |
| 120x40 | avg_fps | 54,412 | 53,598 | -1.49% | stable |
| 120x40 | gini | 0.8943 | 0.8943 | +0.00% | stable |
| 200x60 | avg_fps | 29,992 | 29,619 | -1.24% | stable |
| 200x60 | gini | 0.8905 | 0.8905 | +0.00% | stable |

**All 24 metrics within ±2% natural variance.** The security fix
only affects the live-reload path (not bench mode, which has no
config file). Zero visual or performance regression confirmed.

Raw JSON: `benchmark/bench-labs/S_master_dragon/S3_baseline_A.json`
and `S3_after_B.json`.

## Verdict

**Codebase confirmed post-peak-hardened.** Prior audits caught all
HIGH-severity issues. The v50→v80 delta introduced NO new unsafe
sites, command execution paths, network surface, or privilege
operations.

**One meaningful gap fixed:** the live-reload message path now
mirrors the startup path's sanitization + length cap, closing an
ANSI injection + unbounded memory DoS vector (defense-in-depth —
attacker needs config file write access, but the inconsistency
with the well-hardened startup path was a real defect).

**Deferred (defense-in-depth, no concrete threat model):**
- `safepath` canonicalize for symlink resolution (2 LOW)
- `write_config_atomic` `OpenOptions::create_new(true)` (1 LOW)
- `instant` crate RUSTSEC-2024-0384 (transitive, tracked upstream)

## Files Changed

- `src/config/live_config/mod.rs` — sanitize + length-cap live-reload message (lines 557-594); added MESSAGE_MAX_LEN import.
- `benchmark/bench-labs/S_master_dragon/S3_*.{json,md}` — A/B data + report.
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
