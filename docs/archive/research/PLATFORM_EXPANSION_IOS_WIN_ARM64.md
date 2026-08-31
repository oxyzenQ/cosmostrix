<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Platform Expansion Research — iOS & Windows aarch64

**Date**: 2026-08-23 · **Mode**: research + verified compile evidence, no
product code changes · **Owner question**: can cosmostrix expand to iOS
support and Windows aarch64/arm64?

**TL;DR**

| Target | Compiles today? | Real-world usability | Recommendation |
|--------|-----------------|----------------------|----------------|
| `aarch64-pc-windows-msvc` | **YES — verified** (`cargo check` clean, 0 errors, 2026-08-23, same 5 pre-existing warnings as x86_64) | High — Snapdragon X / Copilot+ PCs run ARM64 natively; x64 emulation works but native is faster | **GO — LANDED 2026-08-23**: release.yml matrix entry (`windows-11-arm` native runner, `pro-win-aarch64` profile) + README platform table updated. First artifact ships with the next tagged release. |
| `aarch64-apple-ios` | **YES — verified** (`cargo check` clean, 0 errors, incl. crossterm, notify-kqueue, clap) | Constrained — no App Store path for a standalone CLI; meaningful only inside terminal-emulator contexts (jailbreak/NewTerm, TrollStore terminal, or an embedding app) | **NO-GO as a shipped target**; document the VM/emulator paths that already work today |

---

## 1. Windows aarch64 (ARM64) — GO

### 1.1 Verified evidence

- `rustup target add aarch64-pc-windows-msvc` + `cargo check --target
  aarch64-pc-windows-msvc` -> **clean compile, zero errors** on 2026-08-23
  at commit `31e7401`. The only warnings are the 5 pre-existing
  cfg-related ones also present on x86_64.
- Target tier: **Tier 2 with host tools** (Rust supports building and
  running on Windows ARM64 directly).

### 1.2 Why it compiles with zero changes (verified, not assumed)

Every platform fork in the codebase is keyed on `cfg(unix)` /
`cfg(target_os = "linux")` / `cfg(not(unix))` — never on
`cfg(target_arch = "x86_64")` for behavior. The only architecture gates
are additive diagnostics:

- `main.rs` CPU-feature check: `#[cfg(target_arch = "x86_64")]` (skipped
  on ARM — correct).
- `diagnostics/mod.rs` already has an `#[cfg(target_arch = "aarch64")]`
  arm — ARM is an anticipated, first-class citizen.
- The fork-based SIGKILL guard is Unix-only; Windows uses the documented
  ConPTY auto-restore no-op (unchanged on ARM64).
- `libc` is a Unix-only dependency; nothing in the Windows path touches
  it. The 2026-08-23 `page_size` libc fix (commit `dc297b5`) specifically
  re-gated that call to Linux — Windows ARM64 inherits the clean path.
- Dependencies: crossterm 0.29 (windows-sys backend), ctrlc 3.4 — both
  support aarch64-pc-windows-msvc.

### 1.3 What shipping it takes (all CI/packaging, no product code)

1. `.cargo/config.toml`: add a `pro-win-arm64` alias mirroring
   `pro-win-amd64` with `--target aarch64-pc-windows-msvc`
   (rustflags: none — `-C target-cpu=native` is unnecessary; ARM64
   baseline is uniform enough, and Windows ARM64 devices vary).
2. `release.yml`: one matrix entry. Two runner options, both verified
   paths at GitHub:
   - **Native**: `windows-11-arm` hosted runner (available to public
     repos) — cleanest, allows running the test suite on-target.
   - **Cross**: existing `windows-latest` + `rustup target add
     aarch64-pc-windows-msvc` — VS Build Tools' ARM64 components link
     cross-built binaries without an ARM host.
3. Release packaging/checksums/GPG: same `.zip` flow as windows-x86_64.
4. AUR/docs: mention `windows-arm64` in the platform list.

Estimated effort: **half a day** (CI + one verification run on real
hardware or a Windows ARM64 VM). Risk: low — the only historically
fragile area (libc gating) was already fixed for the Windows CI break
(`dc297b5`).

### 1.4 Why it matters

Windows ARM64 machines (Surface Pro, Snapdragon X laptops, Copilot+ PCs)
run x86_64 binaries through emulation with a startup and memory overhead.
A terminal renderer that pegs a core benefits visibly from native code:
no emulation layer in the frame loop, smaller RSS, faster startup. This
is the highest-value / lowest-risk expansion available.

## 2. iOS — compiles, but no meaningful standalone distribution

### 2.1 Verified evidence

- `cargo check --target aarch64-apple-ios` -> **clean compile, zero
  errors** (2026-08-23, commit `31e7401`). Notably `notify` builds with
  its **kqueue** backend — the Cargo.toml target table has anticipated
  iOS all along (`cfg(any(... target_os = "ios"))` with the comment
  "iOS too"), and `signal-hook` + `libc` (the `cfg(unix)` deps) resolve
  for iOS.

### 2.2 The three real barriers (none of them are the compiler)

1. **There is no controlling terminal in an iOS app.** Every cosmostrix
   interactive path assumes a TTY on stdin/stdout: raw mode via
   termios, isatty probes (watchdog dead-PTY check), `/dev/tty` recovery.
   A normal iOS app has no PTY; `IsTerminal` returns false everywhere,
   and the fork guard + interactive loop exit early or never engage.
   The binary would run, immediately find no terminal, and exit.
2. **App Store distribution is closed to this shape of program.** A
   standalone CLI binary cannot be shipped; Apple's model requires an
   app bundle. Shipping *cosmostrix inside a terminal-emulator app* is
   technically conceivable (embed the render loop against an in-app
   VT100 view over a PTY pair), but that is a new app project with its
   own UI/HIG/App Review surface — not a port of this codebase, and it
   would still be unable to spawn arbitrary commands (sandbox).
3. **Where a local terminal exists, it is niche**: jailbroken devices
   (NewTerm 2 and similar provide real PTYs), TrollStore-signed
   terminal apps on old iOS versions, or an embedding app built by
   someone else. None of these are distribution channels an LTS project
   should promise support for.

### 2.3 What ALREADY works today (document this instead)

- **UTM (Linux ARM64 VM on iOS)**: the shipped
  `linux-aarch64` release runs natively inside UTM's terminal — this
  works with zero changes and is the honest answer to "cosmostrix on
  iPad/iPhone".
- **iSH (x86 Alpine emulation)**: the `linux-amd64-musl` binary can run
  under iSH's emulation; slow, but it runs. Worth a one-line mention,
  not a support promise.
- **Blink Shell + a real host**: the realistic mobile story for
  power users — run cosmostrix on a server, view from anywhere.

### 2.4 Recommendation

- Do **not** add `aarch64-apple-ios` to the release matrix. Compiles ≠
  supported; shipping an artifact that exits immediately without a TTY
  would generate bug reports, not users.
- Keep the compile-clean status recorded here (it is cheap insurance:
  any future refactor that breaks iOS compilation now has a baseline
  to diff against).
- Document UTM/iSH/Blink paths in README's platform table as
  community-tested, not supported.
- If iOS ever becomes serious, the real project is "terminal emulator
  app with an embedded cosmostrix render engine" — a product decision,
  not a port. The engine-side prerequisite (no-TTY early exit instead
  of hang) already exists.

## 3. Verification commands (reproducible)

```bash
# Windows ARM64 — clean compile evidence
rustup target add aarch64-pc-windows-msvc
cargo check --target aarch64-pc-windows-msvc

# iOS — clean compile evidence (no Xcode needed for check)
rustup target add aarch64-apple-ios
cargo check --target aarch64-apple-ios
```

Both verified green on 2026-08-23 at `31e7401` (zero errors; only the
5 pre-existing Windows cfg warnings on the msvc target, none on iOS).

## 4. Bottom line for the owner

- **Windows ARM64: yes, expand** — a half-day of CI work for a real
  hardware class, with verified zero code changes. Say the word and the
  matrix entry lands as its own micro-commit.
- **iOS: the code is already iOS-clean, but the platform has no honest
  standalone path** — document UTM/iSH instead, and keep this research
  as the "why" record.

---

Copyright (C) 2026 rezky_nightky (oxyzenQ). All rights reserved.
cosmostrix and the cosmostrix logo are trademarks of rezky_nightky (oxyzenQ).
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
