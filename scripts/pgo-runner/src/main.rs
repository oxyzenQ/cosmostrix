// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
//
// ─────────────────────────────────────────────────────────────────────────────
// PLATFORM: UNIX-only (Linux, macOS, BSD).
//   Invokes `bash scripts/build.sh pgo --auto` which uses /proc/cpuinfo
//   (Linux) or sysctl (macOS) for CPU detection. The build itself runs on
//   any platform cargo supports, but this runner assumes a bash-invocable
//   shell. Not for Windows cmd.exe.
// ─────────────────────────────────────────────────────────────────────────────

//! Tiny runner for the `cargo use-pgo` alias.
//!
//! This crate exists because cargo's `!`-prefix shell-alias feature is not
//! reliably available across cargo versions. Instead, the alias in
//! `.cargo/config.toml` is:
//!
//! ```toml
//! use-pgo = "run --manifest-path scripts/pgo-runner/Cargo.toml --release --"
//! ```
//!
//! When the user runs `cargo use-pgo`, cargo compiles this tiny binary
//! (cached after the first invocation) and executes it. This binary then
//! locates the project root (two levels up from `scripts/pgo-runner/`),
//! invokes `./scripts/build.sh pgo --auto`, and forwards its exit code.
//!
//! The `--auto` flag triggers `detect_cpu_target()` inside build.sh, which
//! probes /proc/cpuinfo (Linux) or sysctl (macOS) and selects the best
//! `-C target-cpu` value: x86-64-v4 on AVX-512, x86-64-v3 on AVX2, native
//! on ARM or older x86. This makes the PGO build bulletproof — no more
//! SSE2 regressions.

use std::path::PathBuf;
use std::process::Command;

fn main() -> ! {
    // This crate lives at <project_root>/scripts/pgo-runner/src/main.rs.
    // Walk up to find the project root (the directory containing Cargo.toml
    // for the main cosmostrix crate, not this runner's Cargo.toml).
    let runner_manifest = env!("CARGO_MANIFEST_DIR");
    let runner_path = PathBuf::from(runner_manifest);
    let project_root = runner_path
        .ancestors()
        .nth(2) // src/ -> pgo-runner/ -> scripts/ -> project_root
        .expect("pgo-runner must live at <project_root>/scripts/pgo-runner/")
        .to_path_buf();

    let build_script = project_root.join("scripts").join("build.sh");
    if !build_script.exists() {
        eprintln!("error: build.sh not found at {}", build_script.display());
        std::process::exit(1);
    }

    // Forward any extra arguments passed after `cargo use-pgo --`.
    // Example: `cargo use-pgo -- --no-cache` → build.sh pgo --auto --no-cache
    let extra_args: Vec<String> = std::env::args().skip(1).collect();

    let status = Command::new("bash")
        .arg(&build_script)
        .arg("pgo")
        .arg("--auto")
        .args(&extra_args)
        .current_dir(&project_root)
        .status();

    match status {
        Ok(s) => std::process::exit(s.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("error: failed to execute build.sh: {e}");
            std::process::exit(1);
        }
    }
}
