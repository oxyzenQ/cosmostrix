<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# System Requirements

cosmostrix is designed to run on a wide range of Unix-like systems, from
modern workstations to older servers and embedded devices. This document
specifies the minimum supported system configuration.

## Minimum Supported System

| Component | Minimum | Recommended | Notes |
|-----------|---------|-------------|-------|
| **Kernel (Linux)** | 2.6.27 (2008) | 5.0+ | inotify_init1 used by `notify` crate for live config reload |
| **Kernel (macOS)** | 10.12 Sierra (2016) | 12+ | FSEvents used by `notify` crate |
| **Kernel (Windows)** | 10 (1809) | 11 | Ctrl+C handler via `ctrlc` crate |
| **Rust toolchain** | 1.98.1 (2026-09) | latest stable | MSRV declared in `Cargo.toml` |
| **RAM** | 8 MiB free | 16+ MiB | Peak RSS ~4.2 MiB at 120x40 |
| **CPU** | x86-64-v1 (SSE2) | x86-64-v3 (AVX2) | v3/v4 profiles auto-selected by `scripts/build.sh release` |
| **Terminal** | ANSI 16-color | TrueColor (24-bit) | Auto-detected via `COLORTERM` / `TERM` |

> **Libc note (night-dragon-maintenance-6):** the Linux x86_64 release
> packages are fully static musl builds — the shipped binaries carry their
> own libc, so no host glibc/musl version applies to them. The glibc
> (Linux GNU) build support was retired; see
> [C Library Requirements](#c-library-requirements) for the historical
> floor of the retired gnu builds.

## Kernel Version Requirements (Linux)

cosmostrix uses these Linux syscalls, each with a minimum kernel version:

| Syscall / Feature | Min Kernel | Used For | Required? |
|-------------------|-----------|----------|-----------|
| `fstat` | 1.0 (1994) | `--dump-config` redirect detection | Yes |
| `prctl(PR_SET_PDEATHSIG)` | 2.1.57 (1997) | SIGKILL terminal guard child | Yes (Linux-only) |
| `prctl(PR_SET_NAME)` | 2.6.9 (2004) | Name the guard child process | Yes (Linux-only) |
| `inotify_init1` | 2.6.27 (2008) | Live config reload (`notify` crate) | Yes (live reload) |
| `signalfd` | 2.6.22 (2007) | Signal handling (`signal_hook` crate) | Yes (Unix) |
| `eventfd` | 2.6.22 (2007) | `mio` event loop (crossterm) | Yes |
| `epoll` | 2.6 (2003) | `mio` event loop (crossterm) | Yes |
| `timerfd` | 2.6.25 (2008) | `mio` timer (crossterm) | Yes |
| `getrandom` | 3.17 (2015) | `rand` crate entropy | Fallback to `/dev/urandom` |
| `io_uring` | 5.1 (2019) | NOT used — cosmic-dragon-egg experiment only | No |

**Practical minimum: Linux 2.6.27** (December 2008). This covers:

- CentOS 6+ (2.6.32)
- Ubuntu 10.04+ (2.6.32)
- Debian 6+ (2.6.32)
- RHEL 6+ (2.6.32)
- All modern distributions (kernel 4.x, 5.x, 6.x, 7.x)

**CI proof at the boundary (night-dragon-maintenance-2):** the floor is
not only documented, it is executed. The `supermassive-kernel.yml`
kernel-floor lane boots the REAL intrepid GA 2.6.27 kernel in a KVM
micro-VM on every push and runs the static binary's full headless E2E
battery inside it, as root - the "runs on any Linux kernel 2.6.27+"
promise proven at the exact boundary kernel, live, per commit. The
kernel-best lane proves the other envelope on the newest mainline
stable kernel.

**Kernel/ISA interplay (read before choosing a build):** the lane's
payload is the baseline x86-64 (SSE2) static musl build - the exact
pairing of this section's kernel floor with the CPU minimum below. A
v3-baked binary cannot execute on kernels older than 2.6.35: those
kernels never enable YMM state in XCR0, so the first VEX-encoded
instruction raises #UD (SIGILL) even on an AVX2-capable CPU under KVM
host-passthrough. The v3/v4 release shapes therefore carry a
practical kernel floor of 2.6.35+ on top of the code's 2.6.27 syscall
floor (see the CPU Architecture table).

**Note on `getrandom`:** The `rand` crate uses `getrandom` (kernel 3.17+)
when available, and falls back to `/dev/urandom` on older kernels. The
fallback works on all Linux versions, so `getrandom` is NOT a hard
requirement.

## C Library Requirements

### Static musl (Linux release builds)

The Linux x86_64 release packages are statically linked against musl.
The binary embeds its own libc, so there is no host C library requirement
and no glibc/musl version matrix to check: the same binary runs on any
Linux kernel 2.6.27+ regardless of the host distro's libc (glibc hosts,
Alpine, containers, minimal base images). This is the only Linux x86_64
shape that ships.

| musl version (baked in at build time) | Release | Notes |
|---------------------------------------|---------|-------|
| 1.2.x (rustup target toolchain) | current | Statically linked into every release binary |

### glibc (retired Linux GNU builds — historical reference)

The glibc-linked (gnu) builds were retired in night-dragon-maintenance-6.
No glibc-linked release package ships. The floor below is kept as the
historical record of the retired shapes:

| glibc version | Release | Status (historical) |
|---------------|---------|---------------------|
| 2.17 | CentOS 7 (2012) | Was the minimum supported — all cosmostrix syscalls worked |
| 2.31 | Ubuntu 20.04 (2020) | Was the recommended floor |
| 2.36+ | Modern distros | Were supported |

**Why glibc 2.17 (historical)?** cosmostrix uses only POSIX-standard libc
functions (`fstat`, `prctl`, `tcgetattr`, `tcsetattr`, `getrusage`, `uname`,
`localtime_r`, `sysconf`). These are all present in glibc 2.17, which was
the oldest glibc still in widespread server use when the gnu builds shipped.
Local host builds against a glibc host toolchain still work for development;
they are not release shapes.

### macOS

| macOS version | Darwin kernel | Status |
|---------------|---------------|--------|
| 10.12 Sierra | 16.x | Minimum (FSEvents) |
| 10.15 Catalina | 19.x | Supported |
| 12 Monterey | 21.x | Recommended |
| 13+ Ventura | 22.x+ | Supported |

macOS uses `mach` APIs for memory/CPU stats (`task_info`, `mach_timebase`).
These are available on all macOS versions. The `#allow(deprecated)` in
`memstat.rs` and `cpustat.rs` handles the libc 0.2.x deprecation of
the old `mach` shim in favor of `mach2`.

### FreeBSD

| FreeBSD version | Status | Notes |
|---------------|--------|-------|
| 13.x | Supported | libexecinfo in base |
| 14.x | Supported | libexecinfo in base |
| 15.0+ | Supported (requires libexecinfo) | libexecinfo removed from base system |
| GhostBSD 15 | Supported (requires libexecinfo) | Based on FreeBSD 15 |

**FreeBSD 15 / GhostBSD 15: `libexecinfo` was removed from the base system.**
The Rust standard library links against `-lexecinfo` for backtrace support.
Without it, every Rust build (not just cosmostrix) fails with:

```
ld: error: unable to find library -lexecinfo
```

**Fix — build libexecinfo from source inline (takes ~10 seconds):**

```bash
cd /tmp && mkdir -p libexecinfo-build && cd libexecinfo-build
cat > execinfo.c << 'SRC'
#define _GNU_SOURCE
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int backtrace(void **buffer, int size) {
    void *fp = __builtin_frame_address(1);
    int i = 0;
    while (i < size && fp) {
        buffer[i++] = __builtin_return_address(0);
        fp = __builtin_frame_address(0) ? *(void **)fp : NULL;
    }
    return i;
}

char **backtrace_symbols(void *const *buffer, int size) {
    if (size <= 0) return NULL;
    char **strings = calloc(size, sizeof(char *));
    for (int i = 0; i < size; i++)
        asprintf(&strings[i], "%p", buffer[i]);
    return strings;
}

void backtrace_symbols_fd(void *const *buffer, int size, int fd) {
    char **strings = backtrace_symbols(buffer, size);
    if (!strings) return;
    for (int i = 0; i < size; i++) {
        write(fd, strings[i], strlen(strings[i]));
        write(fd, "\n", 1);
        free(strings[i]);
    }
    free(strings);
}
SRC
cat > execinfo.h << 'HDR'
#ifndef _EXECINFO_H_
#define _EXECINFO_H_
int backtrace(void **buffer, int size);
char **backtrace_symbols(void *const *buffer, int size);
void backtrace_symbols_fd(void *const *buffer, int size, int fd);
#endif
HDR
cc -c -O2 -o execinfo.o execinfo.c && \
ar rcs libexecinfo.a execinfo.o && \
sudo cp libexecinfo.a /usr/lib/ && \
sudo cp execinfo.h /usr/local/include/ && \
sudo ldconfig && \
echo 'libexecinfo installed successfully'
```

After that, `cargo pro-freebsd-amd64` (or `cargo build --release`) works
normally. This is a one-time system setup — all Rust projects benefit.

## Rust Toolchain (MSRV)

| Component | Version | Notes |
|-----------|---------|-------|
| `rustc` | 1.98.1 | Declared in `Cargo.toml` as `rust-version = "1.98"`; pinned in `rust-toolchain.toml` |
| `cargo` | 1.98.1 | Matches rustc |
| Edition | 2021 | Declared in `Cargo.toml` |
| `rustup` | Any | Recommended for toolchain management |

Rust 1.98.1 was released on 2026-09-01. This is the MSRV — cosmostrix
compiles and passes all tests on this exact version. The CI pipeline
enforces this via a dedicated MSRV job.

**Why 1.98.1?** Pinned to a specific patch release instead of `stable`
to guarantee dormant-mode reproducibility — a future `stable` release
cannot silently break the build. Bumped from 1.98.0 to 1.98.1 to pick
up the latest patch release (2026-09-01). The 1.98.0 minor release
(2026-08-20) added the `clippy::manual_slice_fill` lint (satisfied in
the `tier2.rs` reset path) and continued LLVM/codegen refinements;
1.98.1 carries those forward and adds patch-level stability fixes.
The prior 1.97.x pins fixed an LLVM miscompilation present since Rust
1.87 plus a segfault in 1.97.0; those fixes are preserved in 1.98.1.

## CPU Architecture

| Architecture | Profile | CPU Features | Notes |
|--------------|---------|--------------|-------|
| x86-64-v1 | `release` / `pro` | SSE2 (2001) | Baseline — runs on any x86-64 CPU |
| x86-64-v3 | `pro-linux-amd64-v3-musl` | AVX2 (2013) | Modern CPUs (Haswell+, Ryzen+) — static musl release shape; needs kernel 2.6.35+ (YMM state) |
| x86-64-v4 | `pro-linux-amd64-v4-musl` | AVX-512 (2017) | Server/workstation CPUs — static musl release shape; needs kernel 2.6.35+ (YMM state) |
| aarch64 | `pro-android-aarch64` | NEON | Android/Termux, Apple Silicon |
| aarch64 macOS | `pro-macos-aarch64-native` | NEON | Apple M1/M2/M3 |
| x86-64 FreeBSD | `pro-freebsd-amd64` | native (host CPU) | FreeBSD 13+, GhostBSD |

**`./scripts/build.sh release` auto-detects** the CPU microarchitecture
level and dispatches to the matching named cargo alias (the custom build
path — never a bare `cargo build --release`):

- AVX-512 detected -> `cargo pro-linux-amd64-v4-musl`
- AVX2 detected -> `cargo pro-linux-amd64-v3-musl`
- Neither -> `cargo pro-native` (host-optimal codegen, baseline-safe)

On FreeBSD, use `cargo pro-freebsd-amd64` directly.

The runtime CPU check in `info.rs::check_cpu_features()` verifies the
CPU supports the compiled target level. If not, it prints a clear error
and exits (instead of crashing with SIGILL).

## Memory Footprint

| Screen Size | Peak RSS | Notes |
|-------------|----------|-------|
| 80x24 | 3.7 MiB | Minimal terminal |
| 120x40 | 4.2 MiB | Default benchmark size |
| 200x60 | 4.5 MiB | Large terminal |
| 240x80 | 5.2 MiB | Ultra-wide |

Memory is stable under load — 60-second endurance tests show zero growth
(alloc/dealloc balance = +7 over 3.9M operations, well within noise).

## Terminal Requirements

| Feature | Minimum | Recommended |
|---------|---------|-------------|
| ANSI escape sequences | 16-color | TrueColor (24-bit) |
| Alternate screen | Required | Required |
| Raw mode | Required | Required |
| Cursor movement | Required | Required |
| Unicode | UTF-8 locale | UTF-8 locale |
| Sync output (ESC[?2026h) | Optional | Optional (tear-free) |
| Bracketed paste | Optional | Optional |
| Focus events | Optional | Optional |
| Mouse | Optional (always-on glow + click wave) | Optional |

cosmostrix auto-detects terminal capability via `COLORTERM`, `TERM`, and
TTY checks. See `docs/TERMINAL_COMPATIBILITY.md` for the full terminal
matrix.

## What's NOT Required

- **Desktop environment** — cosmostrix runs in any TTY or terminal emulator
- **GPU** — pure CPU + stdout renderer, no OpenGL/Vulkan
- **systemd** — uses `signal_hook` + `fork`, not systemd notify
- **D-Bus** — no desktop integration dependencies
- **X11/Wayland** — terminal-only, no display server
- **io_uring** — investigated (cosmic-dragon-egg experiment), rejected as not
  worth the complexity at 60 FPS
- **Network** — fully offline, no telemetry or update checks by default
  (`--check-update` is opt-in; it needs `curl` or `wget` on PATH and
  reports an actionable error with the manual releases URL when neither
  is installed)
- **Root privileges** — runs as a regular user by design; root execution
  is an unsupported use case: warning-guarded locally, `--check-update`
  hard-refused at euid 0 (policy: `docs/SECURITY_AUDIT.md`, "Running as
  Root")

## Verification

To verify your system meets the requirements:

```bash
# Check kernel version
uname -r  # should be >= 2.6.27

# Check libc (informational only — release binaries are static musl and
# carry their own libc, so the host version does not matter)
ldd --version | head -1

# Check Rust version
rustc --version  # should be >= 1.98.1

# Check CPU features
grep -o 'avx2\|avx512f' /proc/cpuinfo | sort -u
# avx2 -> pro-linux-amd64-v3-musl build available
# avx512f -> pro-linux-amd64-v4-musl build available
# (empty) -> release build (v1 baseline)

# Run cosmostrix doctor for full system check
cosmostrix --doctor
```

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
