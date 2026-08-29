// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for `/proc` ancestor walk hardening and dynamic FPS source
//! attribution. Extracted from `tests.rs` to keep the source file
//! under the 800-LOC cap. Pure code motion — no behavior change.

use super::tests::EnvGuard;
use super::*;
use std::env;
use std::sync::Mutex;

// env::set_var is process-global and not thread-safe; serialize the
// tests that touch TERM_PROGRAM so they don't race with each other.
static ENV_LOCK: Mutex<()> = Mutex::new(());

// ── hardening: /proc ancestor walk tests ──

#[test]
#[cfg(target_os = "linux")]
fn parse_proc_ppid_extracts_correct_field() {
    // Real /proc/<pid>/stat format: "pid (comm) state ppid pgrp ..."
    // The comm field can contain spaces and parens — we parse from
    // the right of the LAST ')' to avoid ambiguity.
    let line = "1234 (zsh) S 1 1234 1234 34816 1234 4194304 12345 1 1";
    assert_eq!(parse_proc_ppid(line), Some(1));

    let line2 = "5678 (alacritty) S 1 5678 5678 34816 5678 4194304 999 1 1";
    assert_eq!(parse_proc_ppid(line2), Some(1));

    // comm with spaces (e.g., a script named "my script")
    let line3 = "9999 (my script) S 1234 9999 9999 34816 9999 4194304 1 1 1";
    assert_eq!(parse_proc_ppid(line3), Some(1234));

    // comm with a paren inside (rare but possible)
    let line4 = "9999 (foo) bar) S 1234 9999 9999 34816 9999 4194304 1 1 1";
    assert_eq!(parse_proc_ppid(line4), Some(1234));
}

#[test]
#[cfg(target_os = "linux")]
fn parse_proc_ppid_returns_none_on_malformed() {
    assert_eq!(parse_proc_ppid(""), None);
    assert_eq!(parse_proc_ppid("no parens here"), None);
    assert_eq!(parse_proc_ppid("(missing pid) S"), None);
    assert_eq!(parse_proc_ppid("(missing ppid) S not_a_number"), None);
}

#[test]
fn ancestor_matches_high_perf_detects_terminal_names() {
    // Direct match: "alacritty" contains "alacritty"
    assert!(ancestor_matches_high_perf(&["alacritty".to_string()]));
    assert!(ancestor_matches_high_perf(&["kitty".to_string()]));
    assert!(ancestor_matches_high_perf(&["ghostty".to_string()]));
    assert!(ancestor_matches_high_perf(&["foot".to_string()]));
    assert!(ancestor_matches_high_perf(&["wezterm".to_string()]));
    assert!(ancestor_matches_high_perf(&["konsole".to_string()]));

    // Case-insensitive: "Alacritty" contains "alacritty"
    assert!(ancestor_matches_high_perf(&["Alacritty".to_string()]));
    assert!(ancestor_matches_high_perf(&["KITTY".to_string()]));
}

#[test]
fn ancestor_matches_high_perf_finds_terminal_in_chain() {
    // The real scenario: cosmostrix → zsh → alacritty. The ancestor
    // walk returns ["zsh", "alacritty"] — "alacritty" matches.
    let chain = vec!["zsh".to_string(), "alacritty".to_string()];
    assert!(
        ancestor_matches_high_perf(&chain),
        "ancestor chain containing alacritty must match"
    );

    // tmux scenario: cosmostrix → zsh → tmux → alacritty
    let tmux_chain = vec![
        "zsh".to_string(),
        "tmux".to_string(),
        "alacritty".to_string(),
    ];
    assert!(
        ancestor_matches_high_perf(&tmux_chain),
        "ancestor chain through tmux to alacritty must match"
    );
}

#[test]
fn ancestor_matches_high_perf_rejects_non_terminal_chains() {
    // cargo test scenario: cosmostrix_test → cargo → zsh → sshd
    // No high-perf terminal in the chain → no match.
    let chain = vec!["cargo".to_string(), "zsh".to_string(), "sshd".to_string()];
    assert!(
        !ancestor_matches_high_perf(&chain),
        "chain without a high-perf terminal must not match"
    );

    // Empty chain
    assert!(!ancestor_matches_high_perf(&[]));

    // Shell-only chain
    assert!(!ancestor_matches_high_perf(&["bash".to_string()]));
}

#[test]
#[cfg(target_os = "linux")]
fn ancestor_process_names_returns_nonempty_in_test_env() {
    // When running `cargo test`, the process tree is:
    //   cosmostrix-<test_binary> → cargo → <shell> → <terminal or sshd>
    // We can't assert WHICH ancestors are present (depends on the
    // caller's environment), but the walk MUST return at least one
    // name (the parent process). If this returns empty, /proc is
    // broken or unavailable, which would mean the Layer 5 fallback
    // silently degrades to 60 FPS — the exact bug we're fixing.
    let names = ancestor_process_names(10);
    assert!(
        !names.is_empty(),
        "ancestor_process_names must return at least the parent process \
         name on Linux — empty result means /proc is unavailable, which \
         would silently disable Layer 5 detection (the alacritty bug)"
    );
}

#[test]
#[cfg(target_os = "linux")]
fn env_guard_inhibits_ancestor_walk_on_current_thread() {
    // Regression guard for the "tests fail inside Alacritty/Kitty/WezTerm"
    // bug. When a developer runs `cargo test` from inside a high-perf
    // terminal, the test process's ancestor chain contains the terminal
    // name, which makes `kitty_keyboard_supported` Layer 4 and
    // `high_perf_detection_source` Layer 5 fire — contaminating tests
    // that set `TERM=dumb` / `TERM=xterm-256color` and assert
    // `!caps.kitty_keyboard` or `dynamic_default_fps == 60.0`.
    //
    // EnvGuard solves this by setting a thread-local inhibit flag that
    // makes `ancestor_process_names` return empty. This test verifies
    // that mechanism explicitly — without it, the bug would be invisible
    // on headless CI (where the ancestor walk finds no high-perf
    // terminal) and only manifest on developer machines.
    //
    // ENV_LOCK is required because EnvGuard::capture() reads env vars
    // (TERM, TERM_PROGRAM, etc.) — without the lock, a concurrent test
    // mutating env vars could cause our Drop to restore a stale value.
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    let names = ancestor_process_names(10);
    assert!(
        names.is_empty(),
        "EnvGuard must inhibit the /proc ancestor walk on the current \
         thread so detect() tests are isolated from the host's actual \
         terminal ancestry. If this fails, EnvGuard is no longer setting \
         the inhibit flag (check set_ancestor_walk_inhibited in ancestor.rs)."
    );
    // EnvGuard::Drop restores the inhibit flag to its previous value.
}

#[test]
#[cfg(target_os = "linux")]
fn ancestor_process_names_stops_at_init() {
    // Walking with max_depth=10 should never include PID 1 (init).
    // The walk stops when ppid <= 1, so "init"/"systemd" should not
    // appear in the result (we break before reading its comm).
    let names = ancestor_process_names(10);
    for name in &names {
        let lower = name.to_ascii_lowercase();
        assert!(
            !lower.contains("systemd") || lower != "init",
            "ancestor walk should stop before init/systemd (got '{name}')"
        );
    }
}

// ── dynamic_fps_source tests ──

#[test]
fn dynamic_fps_source_records_term_program_layer() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::set_var("TERM_PROGRAM", "Alacritty");
    env::remove_var("KONSOLE_VERSION");
    env::remove_var("WT_SESSION");
    let caps = detect();
    assert_eq!(
        caps.dynamic_fps_source, "TERM_PROGRAM",
        "source must identify TERM_PROGRAM as the matching layer"
    );
}

#[test]
fn dynamic_fps_source_records_konsole_layer() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::remove_var("TERM_PROGRAM");
    env::set_var("KONSOLE_VERSION", "230400");
    env::remove_var("WT_SESSION");
    let caps = detect();
    assert_eq!(
        caps.dynamic_fps_source, "KONSOLE_VERSION",
        "source must identify KONSOLE_VERSION as the matching layer"
    );
}

#[test]
fn dynamic_fps_source_records_term_substring_layer() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-ghostty");
    env::remove_var("TERM_PROGRAM");
    env::remove_var("KONSOLE_VERSION");
    env::remove_var("WT_SESSION");
    let caps = detect();
    assert_eq!(
        caps.dynamic_fps_source, "TERM substring",
        "source must identify TERM substring as the matching layer"
    );
}

#[test]
fn dynamic_fps_source_records_fallback() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::set_var("TERM_PROGRAM", "gnome-terminal");
    env::remove_var("KONSOLE_VERSION");
    env::remove_var("WT_SESSION");
    let caps = detect();
    assert_eq!(
        caps.dynamic_fps_source, "standard/unknown fallback",
        "non-high-perf terminal must record fallback source"
    );
}
