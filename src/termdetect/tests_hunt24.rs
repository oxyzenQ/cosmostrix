// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! S-master-HUNT-24: CPU-renderer / console-TTY effects-gate detection
//! tests. Extracted from `tests.rs` to keep that file under the 800-LOC
//! hard cap (see `src/RULES_LOC.md`). Pure test motion — same fixtures
//! via `super::tests::EnvGuard`.

use super::tests::EnvGuard;
use super::*;
use std::env;
use std::sync::Mutex;

// env::set_var is process-global and not thread-safe; serialize the
// tests that touch TERM/TERM_PROGRAM so they don't race with the other
// env-mutating suites.
static ENV_LOCK: Mutex<()> = Mutex::new(());

// ── S-master-HUNT-24: CPU-renderer / console-TTY effects gate tests ──

#[test]
fn cpu_rendered_gate_vte_version_env() {
    // GNOME Terminal / kgx / Xfce / Mate: no TERM_PROGRAM, generic TERM —
    // VTE_VERSION is the only reliable marker. Effects must auto-disable.
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::remove_var("TERM_PROGRAM");
    env::remove_var("KONSOLE_VERSION");
    env::remove_var("WT_SESSION");
    env::set_var("VTE_VERSION", "7603");
    let caps = detect();
    assert!(
        caps.cpu_rendered,
        "VTE_VERSION must flag the VTE family as CPU-rendered"
    );
    assert!(!caps.console_tty);
    assert_eq!(caps.effects_gate_source, "VTE_VERSION");
    // VTE was already on the standard tier; the fps default is unchanged.
    assert_eq!(caps.dynamic_default_fps, 60.0);
}

#[test]
fn cpu_rendered_gate_foot_term() {
    // foot: TERM=foot (its terminfo) or TERM_PROGRAM=foot. Both must flag
    // CPU-rendered AND drop the old 144 FPS high-perf default.
    let _guard = ENV_LOCK.lock().unwrap();
    for (term, tp) in [("foot", None), ("foot-extra", Some("foot"))] {
        let _env = EnvGuard::capture();
        env::set_var("TERM", term);
        match tp {
            Some(v) => env::set_var("TERM_PROGRAM", v),
            None => env::remove_var("TERM_PROGRAM"),
        }
        env::remove_var("KONSOLE_VERSION");
        env::remove_var("WT_SESSION");
        env::remove_var("VTE_VERSION");
        let caps = detect();
        assert!(
            caps.cpu_rendered,
            "foot ({term}) must be flagged CPU-rendered (HUNT-24 reclassification)"
        );
        assert_eq!(
            caps.dynamic_default_fps, 60.0,
            "foot must no longer get the 144 FPS high-perf default"
        );
        // Standard-tier phosphor tuning applies (same as VTE).
        assert!((caps.phosphor_decay_mult - 1.3).abs() < 1e-6);
    }
}

#[test]
fn cpu_rendered_gate_console_tty_linux() {
    // Raw Linux console: TERM=linux — console_tty + cpu_rendered both set,
    // effects hard-off regardless of any other env marker.
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "linux");
    env::remove_var("TERM_PROGRAM");
    env::set_var("VTE_VERSION", "7603"); // even with VTE markers, console wins
    let caps = detect();
    assert!(caps.console_tty, "TERM=linux must flag console_tty");
    assert!(caps.cpu_rendered, "console tty implies cpu_rendered");
    assert!(caps.effects_gate_source.contains("console tty"));
}

#[test]
fn cpu_rendered_gate_dumb_terminal() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "dumb");
    env::remove_var("TERM_PROGRAM");
    env::remove_var("VTE_VERSION");
    let caps = detect();
    assert!(caps.console_tty, "TERM=dumb must flag console_tty");
    assert!(caps.cpu_rendered);
}

#[test]
fn cpu_rendered_gate_xtermjs_host() {
    // Electron/xterm.js hosts already carry their own detection — the
    // effects gate must honor it.
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::set_var("TERM_PROGRAM", "vscode");
    env::remove_var("VTE_VERSION");
    env::remove_var("KONSOLE_VERSION");
    let caps = detect();
    assert!(caps.xtermjs_host);
    assert!(caps.cpu_rendered, "xterm.js host must flag cpu_rendered");
    assert_eq!(caps.effects_gate_source, "xtermjs_host");
}

#[test]
fn cpu_rendered_gate_gpu_terminals_stay_enabled() {
    // GPU/composited terminals (and unknown terminals) keep effects on —
    // the static gate must stay conservative; unknowns are covered at
    // runtime by the dynamic congestion gate, not by a guess here.
    let _guard = ENV_LOCK.lock().unwrap();
    for term in [
        "alacritty",
        "xterm-kitty",
        "xterm-ghostty",
        "xterm-256color",
    ] {
        let _env = EnvGuard::capture();
        env::set_var("TERM", term);
        env::remove_var("TERM_PROGRAM");
        env::remove_var("VTE_VERSION");
        env::remove_var("KONSOLE_VERSION");
        env::remove_var("WT_SESSION");
        let caps = detect();
        assert!(
            !caps.cpu_rendered,
            "{term} must NOT be flagged CPU-rendered (effects stay on)"
        );
        assert!(!caps.console_tty);
        assert_eq!(caps.effects_gate_source, "none — effects on");
    }
}
