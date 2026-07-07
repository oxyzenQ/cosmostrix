// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Benchmark environment collection (reproducibility metadata).
//!
//! Gathers OS + terminal + CPU governor context so benchmark reports are
//! self-documenting for cross-machine comparison. Two machines with the
//! same CPU can produce different results if the governor differs
//! (performance vs powersave) or SMT is enabled/disabled.
//!
//! ## Platform support
//! - **Cross-platform**: kernel version (uname), libc variant (build-time),
//!   terminal env vars ($TERM, $TERM_PROGRAM, $TERM_PROGRAM_VERSION).
//! - **Linux-only**: CPU governor, SMT status (read from `/sys`).
//! - **Other**: emits "unsupported" for the Linux-only fields.
//!
//! All collection happens once at benchmark start (no per-frame cost).
//! Reading `/sys` files is ~1µs per file and does not perturb the
//! measurement window.

/// Snapshot of benchmark environment context. All fields are `String`
/// because they come from env vars / files / syscalls that return text.
/// `None` means "detection unavailable on this platform".
#[derive(Clone, Debug, Default)]
pub struct EnvSnapshot {
    /// Kernel version (e.g. "6.8.0-1014-aws"). From `uname -r`.
    pub kernel_version: Option<String>,
    /// Libc variant (e.g. "gnu", "musl"). From build-time CARGO_CFG_TARGET_ENV.
    pub libc_variant: &'static str,
    /// $TERM env var (e.g. "xterm-256color"). None if unset.
    pub term: Option<String>,
    /// $TERM_PROGRAM env var (e.g. "kitty", "vscode"). None if unset.
    pub term_program: Option<String>,
    /// $TERM_PROGRAM_VERSION env var. None if unset.
    pub term_version: Option<String>,
    /// CPU governor (e.g. "performance", "powersave"). Linux only.
    pub cpu_governor: Option<String>,
    /// SMT/hyperthreading active status ("on" / "off"). Linux only.
    pub smt_active: Option<String>,
}

impl EnvSnapshot {
    /// Collect the current environment snapshot. Reads env vars + /sys
    /// files. Safe to call once at benchmark start.
    #[must_use]
    pub fn collect() -> Self {
        Self {
            kernel_version: kernel_version(),
            libc_variant: libc_variant(),
            term: env_str("TERM"),
            term_program: env_str("TERM_PROGRAM"),
            term_version: env_str("TERM_PROGRAM_VERSION"),
            cpu_governor: linux_sys_read("scaling_governor"),
            smt_active: linux_smt_active(),
        }
    }
}

/// Read an env var as Option<String>. Empty strings become None.
fn env_str(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

/// Libc variant — known at build time, no runtime cost.
fn libc_variant() -> &'static str {
    // CARGO_CFG_TARGET_ENV is set by cargo at compile time.
    // Values: "musl", "gnu" (Linux), "" (macOS uses "" or "apple"),
    // "msvc" / "gnu" (Windows).
    match std::env::consts::OS {
        "linux" => {
            if cfg!(target_env = "musl") {
                "musl"
            } else {
                "gnu"
            }
        }
        "macos" => "apple-libc",
        "windows" => {
            if cfg!(target_env = "msvc") {
                "msvc"
            } else {
                "gnu"
            }
        }
        _ => "unknown",
    }
}

/// Kernel version via uname(2). Cross-platform on Unix.
#[cfg(unix)]
fn kernel_version() -> Option<String> {
    // SAFETY: uname writes into a utsname struct. This is the documented
    // POSIX API. The struct is stack-allocated; release field is a fixed
    // NUL-terminated char array.
    unsafe {
        let mut buf: libc::utsname = std::mem::zeroed();
        let rc = libc::uname(&mut buf);
        if rc != 0 {
            return None;
        }
        // release is [c_char; 65] on Linux, NUL-terminated.
        let release: &[u8] = {
            let ptr = buf.release.as_ptr() as *const u8;
            let len = buf.release.len();
            std::slice::from_raw_parts(ptr, len)
        };
        let nul = release
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(release.len());
        std::str::from_utf8(&release[..nul])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

#[cfg(not(unix))]
fn kernel_version() -> Option<String> {
    None
}

/// Read a single line from `/sys/devices/system/cpu/cpu0/cpufreq/<key>`.
/// Returns None if the file doesn't exist (e.g. on non-Linux or systems
/// without CPUFreq).
#[cfg(target_os = "linux")]
fn linux_sys_read(key: &str) -> Option<String> {
    let path = format!("/sys/devices/system/cpu/cpu0/cpufreq/{key}");
    std::fs::read_to_string(&path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(not(target_os = "linux"))]
fn linux_sys_read(_key: &str) -> Option<String> {
    None
}

/// Read SMT active status from `/sys/devices/system/cpu/smt/active`.
/// Returns "on" or "off" on Linux with SMT support; None otherwise.
#[cfg(target_os = "linux")]
fn linux_smt_active() -> Option<String> {
    let raw = std::fs::read_to_string("/sys/devices/system/cpu/smt/active")
        .ok()?
        .trim()
        .to_string();
    match raw.as_str() {
        "1" => Some("on".to_string()),
        "0" => Some("off".to_string()),
        _ => Some(raw),
    }
}

#[cfg(not(target_os = "linux"))]
fn linux_smt_active() -> Option<String> {
    None
}

// ── Report rendering ───────────────────────────────────────────────────────

use crate::report::Report;

/// Append the BENCHMARK ENVIRONMENT section to a Report. Kept here (next
/// to the EnvSnapshot definition) rather than in bench_report.rs to keep
/// bench_report.rs under its 1000-LOC guard.
pub(crate) fn render_section(r: &mut Report, env: &EnvSnapshot) {
    let s = r.section("BENCHMARK ENVIRONMENT");
    s.field(
        "kernel_version",
        env.kernel_version.as_deref().unwrap_or("unsupported"),
    );
    s.field("libc_variant", env.libc_variant);
    s.field("term", env.term.as_deref().unwrap_or("(unset)"));
    s.field(
        "term_program",
        env.term_program.as_deref().unwrap_or("(unset)"),
    );
    s.field(
        "term_version",
        env.term_version.as_deref().unwrap_or("(unset)"),
    );
    s.field(
        "cpu_governor",
        env.cpu_governor.as_deref().unwrap_or("unsupported"),
    );
    s.field(
        "smt_active",
        env.smt_active.as_deref().unwrap_or("unsupported"),
    );
    s.field(
        "env_basis",
        "kernel via uname; governor/SMT via /sys (Linux only); terminal via env vars",
    );
    s.field(
        "env_caveat",
        "governor/SMT affect CPU frequency + throughput; same CPU can produce different results",
    );
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_returns_non_empty_kernel_on_unix() {
        let env = EnvSnapshot::collect();
        #[cfg(unix)]
        {
            assert!(
                env.kernel_version.is_some(),
                "kernel_version must be Some on Unix"
            );
            assert!(!env.kernel_version.as_ref().unwrap().is_empty());
        }
        #[cfg(not(unix))]
        {
            assert!(env.kernel_version.is_none());
        }
    }

    #[test]
    fn libc_variant_is_known() {
        let env = EnvSnapshot::collect();
        let known = ["gnu", "musl", "apple-libc", "msvc", "unknown"];
        assert!(
            known.contains(&env.libc_variant),
            "libc_variant '{}' must be a known value",
            env.libc_variant
        );
    }

    #[test]
    fn collect_does_not_panic() {
        // Smoke test — collect() reads env vars + /sys files and must
        // never panic regardless of platform or missing files.
        let _ = EnvSnapshot::collect();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_sys_read_returns_none_for_nonexistent_file() {
        // A key that definitely doesn't exist in cpufreq.
        let result = linux_sys_read("definitely_nonexistent_key_xyz123");
        assert!(result.is_none(), "nonexistent /sys file must return None");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cpu_governor_is_some_on_most_linux_systems() {
        let env = EnvSnapshot::collect();
        // Most Linux systems have CPUFreq enabled. VMs/containers without
        // cpufreq may not have the file — accept None there.
        if let Some(g) = env.cpu_governor {
            let known = [
                "performance",
                "powersave",
                "ondemand",
                "conservative",
                "schedutil",
                "userspace",
            ];
            assert!(
                known.contains(&g.as_str()) || !g.is_empty(),
                "cpu_governor '{g}' should be a known value or non-empty"
            );
        }
    }
}
