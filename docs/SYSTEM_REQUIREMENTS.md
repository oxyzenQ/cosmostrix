<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# System Requirements

Cosmostrix is designed to run on a wide range of Unix-like systems, from
modern workstations to older servers and embedded devices. This document
specifies the minimum supported system configuration.

## Minimum Supported System

| Component | Minimum | Recommended | Notes |
|-----------|---------|-------------|-------|
| **Kernel (Linux)** | 2.6.27 (2008) | 5.0+ | inotify_init1 used by `notify` crate for live config reload |
| **Kernel (macOS)** | 10.12 Sierra (2016) | 12+ | FSEvents used by `notify` crate |
| **Kernel (Windows)** | 10 (1809) | 11 | Ctrl+C handler via `ctrlc` crate |
| **glibc (Linux GNU)** | 2.17 (2012) | 2.31+ | CentOS 7 baseline; fstat, prctl, tcsetattr |
| **musl (Linux musl)** | 1.2.0 (2019) | 1.2.5+ | Static binary, no glibc dependency |
| **Rust toolchain** | 1.81.0 (2024-10) | latest stable | MSRV declared in `Cargo.toml` |
| **RAM** | 8 MiB free | 16+ MiB | Peak RSS ~4.2 MiB at 120x40 |
| **CPU** | x86-64-v1 (SSE2) | x86-64-v3 (AVX2) | v3/v4 profiles auto-selected by `install.sh` |
| **Terminal** | ANSI 16-color | TrueColor (24-bit) | Auto-detected via `COLORTERM` / `TERM` |

## Kernel Version Requirements (Linux)

Cosmostrix uses these Linux syscalls, each with a minimum kernel version:

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
| `io_uring` | 5.1 (2019) | NOT used — dragon-egg experiment only | No |

**Practical minimum: Linux 2.6.27** (December 2008). This covers:
- CentOS 6+ (2.6.32)
- Ubuntu 10.04+ (2.6.32)
- Debian 6+ (2.6.32)
- RHEL 6+ (2.6.32)
- All modern distributions (kernel 4.x, 5.x, 6.x, 7.x)

**Note on `getrandom`:** The `rand` crate uses `getrandom` (kernel 3.17+)
when available, and falls back to `/dev/urandom` on older kernels. The
fallback works on all Linux versions, so `getrandom` is NOT a hard
requirement.

## C Library Requirements

### glibc (Linux GNU builds)

| glibc version | Release | Status |
|---------------|---------|--------|
| 2.17 | CentOS 7 (2012) | Minimum supported — all cosmostrix syscalls work |
| 2.19 | Debian 8 (2014) | Supported |
| 2.24 | Ubuntu 18.04 (2017) | Supported |
| 2.28 | RHEL 8 (2018) | Supported |
| 2.31 | Ubuntu 20.04 (2020) | Recommended |
| 2.34 | Fedora 35 (2021) | Supported |
| 2.36+ | Modern distros | Supported |

**Why glibc 2.17?** Cosmostrix uses only POSIX-standard libc functions
(`fstat`, `prctl`, `tcgetattr`, `tcsetattr`, `getrusage`, `uname`,
`localtime_r`, `sysconf`). These are all present in glibc 2.17, which is
the oldest glibc still in widespread server use (CentOS 7 EOL 2024).

### musl (Linux musl builds)

| musl version | Release | Status |
|--------------|---------|--------|
| 1.2.0 | 2019 | Minimum supported |
| 1.2.4 | 2023 | Recommended |
| 1.2.5+ | 2024 | Latest |

**musl builds are statically linked** — no runtime library dependency.
The resulting binary runs on any Linux kernel 2.6.27+ regardless of the
host's C library. This is the most portable cosmostrix build.

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
| `rustc` | 1.81.0 | Declared in `Cargo.toml` as `rust-version = "1.81"` |
| `cargo` | 1.81.0 | Matches rustc |
| Edition | 2021 | Declared in `Cargo.toml` |
| `rustup` | Any | Recommended for toolchain management |

Rust 1.81.0 was released on 2024-10-17. This is the MSRV — cosmostrix
compiles and passes all tests on this exact version. The CI pipeline
enforces this via a dedicated MSRV job.

**Why 1.81?** The `let-else` chains, `if-let` patterns, and error
handling idioms used throughout the codebase require 1.81+. The `notify`
6.x crate also has an MSRV of 1.81.

## CPU Architecture

| Architecture | Profile | CPU Features | Notes |
|--------------|---------|--------------|-------|
| x86-64-v1 | `release` / `pro` | SSE2 (2001) | Baseline — runs on any x86-64 CPU |
| x86-64-v3 | `pro-linux-v3` | AVX2 (2013) | Modern CPUs (Haswell+, Ryzen+) |
| x86-64-v4 | `pro-linux-v4` | AVX-512 (2017) | Server/workstation CPUs |
| aarch64 | `pro-android-aarch64` | NEON | Android/Termux, Apple Silicon |
| aarch64 macOS | `pro-macos-aarch64-native` | NEON | Apple M1/M2/M3 |
| x86-64 FreeBSD | `pro-freebsd-amd64` | native (host CPU) | FreeBSD 13+, GhostBSD |

**`install.sh` auto-detects** the CPU microarchitecture level and builds
the optimal profile (Linux only):
- AVX-512 detected → `pro-linux-v4`
- AVX2 detected → `pro-linux-v3`
- Neither → `release` (v1 baseline, works everywhere)

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

Cosmostrix auto-detects terminal capability via `COLORTERM`, `TERM`, and
TTY checks. See `docs/TERMINAL_COMPATIBILITY.md` for the full terminal
matrix.

## What's NOT Required

- **Desktop environment** — cosmostrix runs in any TTY or terminal emulator
- **GPU** — pure CPU + stdout renderer, no OpenGL/Vulkan
- **systemd** — uses `signal_hook` + `fork`, not systemd notify
- **D-Bus** — no desktop integration dependencies
- **X11/Wayland** — terminal-only, no display server
- **io_uring** — investigated (dragon-egg experiment), rejected as not
  worth the complexity at 60 FPS
- **Network** — fully offline, no telemetry or update checks by default
  (`--check-update` is opt-in)

## Verification

To verify your system meets the requirements:

```bash
# Check kernel version
uname -r  # should be >= 2.6.27

# Check glibc version (Linux GNU)
ldd --version | head -1  # should be >= 2.17

# Check Rust version
rustc --version  # should be >= 1.81.0

# Check CPU features
grep -o 'avx2\|avx512f' /proc/cpuinfo | sort -u
# avx2 → pro-linux-v3 build available
# avx512f → pro-linux-v4 build available
# (empty) → release build (v1 baseline)

# Run cosmostrix doctor for full system check
cosmostrix --doctor
```
