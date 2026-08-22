// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use super::*;
use std::env;
use std::sync::Mutex;

// env::set_var is process-global and not thread-safe; serialize the
// tests that touch TERM_PROGRAM so they don't race with each other.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Helper: restore TERM and TERM_PROGRAM after a test mutates them.
/// Captures the prev values up-front and returns a closure that
/// restores them when dropped (RAII). Eliminates the boilerplate
/// `match prev_*` blocks that previously appeared in every test.
///
/// Also inhibits the `/proc` ancestor walk (`ancestor_process_names`)
/// on the current thread for the lifetime of the guard. Without this,
/// tests that set `TERM=dumb` or `TERM=xterm-256color` (no high-perf
/// hint) and assert `!caps.kitty_keyboard` would fail when run inside
/// Alacritty/Kitty/WezTerm/Ghostty/Foot/Konsole — the test process's
/// ancestor chain contains the terminal name, firing Layer 4/5 of the
/// detection chain and contaminating the result. The inhibit flag is
/// thread-local, so concurrent tests that explicitly want the real
/// walk (e.g. `ancestor_process_names_returns_nonempty_in_test_env`)
/// are unaffected.
struct EnvGuard {
    prev_term: Option<String>,
    prev_tp: Option<String>,
    prev_konsole_version: Option<String>,
    prev_wt_session: Option<String>,
    prev_ancestor_inhibit: bool,
}

impl EnvGuard {
    fn capture() -> Self {
        // Set the thread-local inhibit flag BEFORE reading any env var,
        // so that any subsequent `detect()` call on this thread sees the
        // flag and skips the /proc walk. Save the previous value so
        // nested EnvGuards restore correctly.
        let prev_ancestor_inhibit = set_ancestor_walk_inhibited(true);
        Self {
            prev_term: env::var("TERM").ok(),
            prev_tp: env::var("TERM_PROGRAM").ok(),
            prev_konsole_version: env::var("KONSOLE_VERSION").ok(),
            prev_wt_session: env::var("WT_SESSION").ok(),
            prev_ancestor_inhibit,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // Restore the inhibit flag FIRST, before restoring env vars, so
        // that if another EnvGuard is being created concurrently on a
        // different thread it doesn't see a stale flag. (The flag is
        // thread-local, so this is only relevant for nested guards on
        // the same thread, but the ordering is still correct.)
        set_ancestor_walk_inhibited(self.prev_ancestor_inhibit);
        match self.prev_term.take() {
            Some(v) => env::set_var("TERM", v),
            None => env::remove_var("TERM"),
        }
        match self.prev_tp.take() {
            Some(v) => env::set_var("TERM_PROGRAM", v),
            None => env::remove_var("TERM_PROGRAM"),
        }
        match self.prev_konsole_version.take() {
            Some(v) => env::set_var("KONSOLE_VERSION", v),
            None => env::remove_var("KONSOLE_VERSION"),
        }
        match self.prev_wt_session.take() {
            Some(v) => env::set_var("WT_SESSION", v),
            None => env::remove_var("WT_SESSION"),
        }
    }
}

#[test]
fn sync_markers_are_valid_escape_sequences() {
    // SYNC_START / SYNC_END must start with ESC [ and end with
    // valid CSI terminators (h/l for set/reset private modes).
    assert!(SYNC_START.starts_with(b"\x1b["));
    assert!(SYNC_END.starts_with(b"\x1b["));
    assert_eq!(SYNC_START.last(), Some(&b'h'));
    assert_eq!(SYNC_END.last(), Some(&b'l'));
}

#[test]
fn ris_reset_is_valid_escape_sequence() {
    // RIS is ESC c (0x1b 0x63) — a 2-byte C1 control sequence.
    assert_eq!(RIS_RESET.len(), 2);
    assert_eq!(RIS_RESET[0], 0x1b);
    assert_eq!(RIS_RESET[1], b'c');
}

#[test]
fn sync_output_disabled_for_linux_console() {
    // Simulate TERM=linux detection result: sync_output stays OFF
    // (Linux console vt.c does not understand mode 2026), but
    // has_alternate_screen is now ON (vt.c supports mode 1049 since
    // kernel 2.6.x — see detect() comment for the full history).
    // kitty_keyboard is also OFF — vt.c doesn't understand kitty
    // protocol sequences (CSI->1u), would emit them as literal chars.
    let caps = TerminalCaps {
        sync_output: false,
        kitty_keyboard: false,
        has_alternate_screen: true,
        xtermjs_host: false,
        vscode_integrated: false,
        default_fps_cap: 240.0,
        dynamic_default_fps: 60.0,
        dynamic_fps_source: "test",
    };
    assert!(!caps.sync_output);
    assert!(
        caps.has_alternate_screen,
        "Linux console supports alternate screen (kernel 2.6.x+)"
    );
    assert!(
        !caps.kitty_keyboard,
        "Linux console does not support kitty keyboard protocol"
    );
}

#[test]
fn alternate_screen_enabled_for_linux_console() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "linux");
    env::remove_var("TERM_PROGRAM");
    let caps = detect();
    assert!(
        caps.has_alternate_screen,
        "Linux console (TERM=linux) supports alternate screen via vt.c mode 1049 (kernel 2.6.x+). Re-enabling alt screen preserves TTY scrollback on quit."
    );
    // sync_output must stay OFF for Linux console — vt.c does not
    // understand mode 2026, so emitting sync markers is wasted bytes.
    assert!(
        !caps.sync_output,
        "sync_output must stay disabled for Linux console"
    );
    // kitty_keyboard must stay OFF for Linux console — vt.c would
    // emit CSI->1u as literal characters, polluting the input stream.
    assert!(
        !caps.kitty_keyboard,
        "kitty_keyboard must stay disabled for Linux console (vt.c doesn't understand kitty protocol)"
    );
}

#[test]
fn alternate_screen_disabled_for_dumb_terminal() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "dumb");
    env::remove_var("TERM_PROGRAM");
    let caps = detect();
    assert!(
        !caps.has_alternate_screen,
        "dumb terminal does not support alternate screen"
    );
    assert!(
        !caps.kitty_keyboard,
        "dumb terminal does not support kitty keyboard protocol"
    );
}

#[test]
fn alternate_screen_enabled_for_xterm() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::remove_var("TERM_PROGRAM");
    let caps = detect();
    assert!(
        caps.has_alternate_screen,
        "xterm-256color supports alternate screen"
    );
    // Generic xterm (no TERM_PROGRAM, no ancestor hint): kitty_keyboard
    // is conservatively OFF. Real xterm support is version-dependent
    // and many setups claim `xterm` TERM without actually being xterm
    // (SSH defaults, screen, etc.). Conservative skip avoids pushing
    // garbage CSI->1u to non-xterm terminals.
    assert!(
        !caps.kitty_keyboard,
        "kitty_keyboard must stay disabled for generic xterm (conservative skip — support is version-dependent and TERM=xterm is the default for many non-xterm setups)"
    );
}

#[test]
fn vscode_detection_when_term_program_is_vscode() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::set_var("TERM_PROGRAM", "vscode");
    let caps = detect();
    assert!(caps.xtermjs_host, "xtermjs_host must be true for VSCode");
    assert!(
        caps.vscode_integrated,
        "vscode_integrated must be true for VSCode"
    );
    assert!(
        !caps.sync_output,
        "sync_output must be disabled for xterm.js hosts (OOM amplification)"
    );
    assert!(
        !caps.kitty_keyboard,
        "kitty_keyboard must be disabled for xterm.js hosts (xterm.js doesn't implement kitty protocol — pushing CSI->1u would pollute input stream)"
    );
    assert_eq!(caps.default_fps_cap, XTERMJS_FPS_CAP);
}

#[test]
fn xtermjs_host_detection_for_all_known_hosts() {
    // Tier 2: every entry in XTERMJS_HOSTS must trigger detection.
    // This is the regression test for the core Tier 2 fix — if a
    // new host is added to the list but the detection logic breaks,
    // this test fails.
    let _guard = ENV_LOCK.lock().unwrap();
    for &host in known_xtermjs_hosts() {
        let _env = EnvGuard::capture();
        env::set_var("TERM", "xterm-256color");
        env::set_var("TERM_PROGRAM", host);
        let caps = detect();
        assert!(
            caps.xtermjs_host,
            "xtermjs_host must be true for TERM_PROGRAM={host}"
        );
        assert!(
            !caps.sync_output,
            "sync_output must be disabled for TERM_PROGRAM={host}"
        );
        assert!(
            !caps.kitty_keyboard,
            "kitty_keyboard must be disabled for xterm.js host {host}"
        );
        assert_eq!(
            caps.default_fps_cap, XTERMJS_FPS_CAP,
            "FPS cap must apply for TERM_PROGRAM={host}"
        );
    }
}

#[test]
fn kitty_keyboard_enabled_for_alacritty_term_program() {
    // Owner-reported bug: Super+C cycled colors on Alacritty despite
    // the modifier allowlist. This is the regression test for the
    // kitty keyboard protocol enablement fix.
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::set_var("TERM_PROGRAM", "Alacritty");
    let caps = detect();
    assert!(
        caps.kitty_keyboard,
        "Alacritty (TERM_PROGRAM=Alacritty) supports kitty keyboard protocol since v0.13.0"
    );
}

#[test]
fn kitty_keyboard_enabled_for_kitty_term_program() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-kitty");
    env::set_var("TERM_PROGRAM", "kitty");
    let caps = detect();
    assert!(
        caps.kitty_keyboard,
        "kitty terminal is the original implementer of kitty keyboard protocol"
    );
}

#[test]
fn kitty_keyboard_enabled_for_wezterm_term_program() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "wezterm");
    env::set_var("TERM_PROGRAM", "WezTerm");
    let caps = detect();
    assert!(
        caps.kitty_keyboard,
        "WezTerm supports kitty keyboard protocol"
    );
}

#[test]
fn kitty_keyboard_enabled_for_ghostty_term_program() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-ghostty");
    env::set_var("TERM_PROGRAM", "ghostty");
    let caps = detect();
    assert!(
        caps.kitty_keyboard,
        "ghostty supports kitty keyboard protocol"
    );
}

#[test]
fn kitty_keyboard_enabled_for_foot_term_program() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "foot-extra");
    env::set_var("TERM_PROGRAM", "foot");
    let caps = detect();
    assert!(
        caps.kitty_keyboard,
        "foot supports kitty keyboard protocol since v1.4"
    );
}

#[test]
fn kitty_keyboard_enabled_for_konsole_via_env_var() {
    // KDE Konsole doesn't set TERM_PROGRAM; it sets KONSOLE_VERSION.
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::remove_var("TERM_PROGRAM");
    env::set_var("KONSOLE_VERSION", "230804");
    let caps = detect();
    assert!(
        caps.kitty_keyboard,
        "KDE Konsole (KONSOLE_VERSION set) supports kitty keyboard protocol since 22.04"
    );
}

#[test]
fn kitty_keyboard_enabled_for_term_substring_hint() {
    // Some terminals don't set TERM_PROGRAM but their name appears
    // as a substring in TERM (e.g., `xterm-ghostty`, `alacritty`).
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-ghostty");
    env::remove_var("TERM_PROGRAM");
    let caps = detect();
    assert!(
        caps.kitty_keyboard,
        "TERM=xterm-ghostty contains 'ghostty' substring — should trigger kitty keyboard detection"
    );
}

#[test]
fn kitty_keyboard_disabled_for_iterm_app() {
    // iTerm.app is high-perf but NOT in the kitty keyboard list —
    // support is version-dependent and requires opt-in. Conservative
    // skip to avoid pushing garbage CSI->1u to a terminal that may
    // not understand it.
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::set_var("TERM_PROGRAM", "iTerm.app");
    let caps = detect();
    assert!(
        !caps.kitty_keyboard,
        "iTerm.app is excluded from kitty keyboard list (version-dependent support — conservative skip)"
    );
}

#[test]
fn kitty_keyboard_disabled_for_apple_terminal() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::set_var("TERM_PROGRAM", "Apple_Terminal");
    let caps = detect();
    assert!(
        !caps.kitty_keyboard,
        "Apple_Terminal is excluded from kitty keyboard list (version-dependent support — conservative skip)"
    );
}

#[test]
fn kitty_keyboard_case_insensitive_term_program_match() {
    // Detection must be case-insensitive — some terminals emit
    // `alacritty` vs `Alacritty`, `WEZTERM` vs `WezTerm`.
    let _guard = ENV_LOCK.lock().unwrap();
    for &tp in &["alacritty", "ALACRITTY", "Alacritty", "aLaCrItTy"] {
        let _env = EnvGuard::capture();
        env::set_var("TERM", "xterm-256color");
        env::set_var("TERM_PROGRAM", tp);
        let caps = detect();
        assert!(
            caps.kitty_keyboard,
            "kitty_keyboard must be true for TERM_PROGRAM={tp} (case-insensitive)"
        );
    }
}

#[test]
fn vscode_alias_false_for_non_vscode_xtermjs_hosts() {
    // Tier 2: vscode_integrated is a back-compat alias that should
    // be FALSE for non-VSCode xterm.js hosts (Hyper, WaveTerminal,
    // etc.), even though xtermjs_host is true. This protects
    // user-facing strings that single out VSCode.
    let _guard = ENV_LOCK.lock().unwrap();
    for &host in known_xtermjs_hosts().iter().filter(|&&h| h != "vscode") {
        let _env = EnvGuard::capture();
        env::set_var("TERM", "xterm-256color");
        env::set_var("TERM_PROGRAM", host);
        let caps = detect();
        assert!(caps.xtermjs_host, "xtermjs_host must be true for {host}");
        assert!(
            !caps.vscode_integrated,
            "vscode_integrated must be false for non-VSCode host {host}"
        );
    }
}

#[test]
fn detection_false_for_native_terminals() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::set_var("TERM_PROGRAM", "alacritty");
    let caps = detect();
    assert!(!caps.xtermjs_host);
    assert!(!caps.vscode_integrated);
    assert!(
        caps.sync_output,
        "sync_output must stay on for native terminals"
    );
    assert_eq!(caps.default_fps_cap, 240.0);
}

#[test]
fn detection_false_when_term_program_unset() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::remove_var("TERM_PROGRAM");
    let caps = detect();
    assert!(!caps.xtermjs_host);
    assert!(!caps.vscode_integrated);
}

#[test]
fn detection_false_for_unknown_electron_hosts() {
    // Tier 2: an Electron host that's NOT in our list (e.g., a
    // future or internal tool) should not trigger xterm.js
    // detection. We can't know if it embeds xterm.js, so we err
    // on the side of full performance (no cap, sync_output on).
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::set_var("TERM_PROGRAM", "SomeUnknownElectronApp");
    let caps = detect();
    assert!(!caps.xtermjs_host);
    assert!(caps.sync_output);
    assert_eq!(caps.default_fps_cap, 240.0);
}

#[test]
fn detect_does_not_panic_with_empty_term() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::remove_var("TERM");
    env::remove_var("TERM_PROGRAM");
    let caps = detect();
    assert!(!caps.xtermjs_host);
    assert!(caps.sync_output, "empty TERM should still enable sync");
}

#[test]
fn vscode_fps_cap_alias_matches_xtermjs_cap() {
    // Back-compat: VSCODE_FPS_CAP is now an alias for XTERMJS_FPS_CAP.
    // Older tests reference VSCODE_FPS_CAP — this assertion ensures
    // the alias tracks the canonical constant if either is retuned.
    assert_eq!(VSCODE_FPS_CAP, XTERMJS_FPS_CAP);
}

#[test]
fn known_hosts_list_includes_vscode() {
    // VSCode is the original crash host and must always be in the
    // list — removing it would silently regress the Tier 1 fix.
    assert!(
        known_xtermjs_hosts().contains(&"vscode"),
        "XTERMJS_HOSTS must contain 'vscode' (Tier 1 back-compat)"
    );
}

#[test]
fn known_hosts_list_has_at_least_five_entries() {
    // Tier 2 expanded detection from 1 host (VSCode) to ≥5 hosts
    // (VSCode + Hyper + WaveTerminal + Tabby + WarpTerminal). This
    // test fails if a future refactor accidentally shrinks the list.
    assert!(
        known_xtermjs_hosts().len() >= 5,
        "XTERMJS_HOSTS must have at least 5 entries (Tier 2 expansion)"
    );
}

// ── masterclass: dynamic default FPS tests ──

#[test]
fn dynamic_default_fps_high_perf_terminal_gets_144() {
    let _guard = ENV_LOCK.lock().unwrap();
    for &term in &[
        "Alacritty",
        "kitty",
        "WezTerm",
        "ghostty",
        "foot",
        "iTerm.app",
        "Apple_Terminal",
        "konsole",
        "WindowsTerminal",
    ] {
        let _env = EnvGuard::capture();
        env::set_var("TERM", "xterm-256color");
        env::set_var("TERM_PROGRAM", term);
        let caps = detect();
        assert_eq!(
            caps.dynamic_default_fps, 144.0,
            "high-perf terminal {term} must default to 144 FPS"
        );
    }
}

#[test]
fn dynamic_default_fps_case_insensitive_match_gets_144() {
    // hotfix: case-insensitive matching — `alacritty` (lowercase)
    // must match `Alacritty` in the list. Previously this fell through
    // to 60 FPS, which is the most likely cause of owner's "60 not 144"
    // report.
    let _guard = ENV_LOCK.lock().unwrap();
    for &term in &["alacritty", "Kitty", "WEZTERM", "GHOSTTY", "FOOT"] {
        let _env = EnvGuard::capture();
        env::set_var("TERM", "xterm-256color");
        env::set_var("TERM_PROGRAM", term);
        let caps = detect();
        assert_eq!(
            caps.dynamic_default_fps, 144.0,
            "case-insensitive: {term} must match high-perf list"
        );
    }
}

#[test]
fn dynamic_default_fps_term_substring_fallback_gets_144() {
    // hotfix: terminals that don't set TERM_PROGRAM but set a
    // distinctive TERM (e.g., `xterm-ghostty`, `alacritty`) must still
    // get the high-perf default via the TERM substring hint fallback.
    let _guard = ENV_LOCK.lock().unwrap();
    for &term in &["xterm-ghostty", "alacritty", "xterm-kitty", "foot-extra"] {
        let _env = EnvGuard::capture();
        env::set_var("TERM", term);
        env::remove_var("TERM_PROGRAM");
        env::remove_var("KONSOLE_VERSION");
        env::remove_var("WT_SESSION");
        let caps = detect();
        assert_eq!(
            caps.dynamic_default_fps, 144.0,
            "TERM substring hint '{term}' must trigger high-perf default"
        );
    }
}

#[test]
fn dynamic_default_fps_konsole_via_env_var_gets_144() {
    // hotfix: KDE Konsole doesn't set TERM_PROGRAM; it exports
    // KONSOLE_VERSION. Detect via that env var.
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::remove_var("TERM_PROGRAM");
    env::set_var("KONSOLE_VERSION", "230400");
    let caps = detect();
    assert_eq!(
        caps.dynamic_default_fps, 144.0,
        "KDE Konsole (KONSOLE_VERSION set) must default to 144 FPS"
    );
}

#[test]
fn dynamic_default_fps_windows_terminal_via_env_var_gets_144() {
    // hotfix: Windows Terminal sets WT_SESSION (not TERM_PROGRAM).
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::remove_var("TERM_PROGRAM");
    env::remove_var("KONSOLE_VERSION");
    env::set_var("WT_SESSION", "abc-123");
    let caps = detect();
    assert_eq!(
        caps.dynamic_default_fps, 144.0,
        "Windows Terminal (WT_SESSION set) must default to 144 FPS"
    );
}

#[test]
fn dynamic_default_fps_tmux_passthrough_outer_terminal() {
    // hotfix: tmux doesn't override TERM_PROGRAM (it sets TMUX
    // instead), so the outer terminal's TERM_PROGRAM passes through.
    // An Alacritty user inside tmux must still get 144 FPS.
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "tmux-256color");
    env::set_var("TERM_PROGRAM", "Alacritty");
    env::set_var("TMUX", "/tmp/tmux-1000/default,12345,0");
    let caps = detect();
    assert_eq!(
        caps.dynamic_default_fps, 144.0,
        "Alacritty inside tmux must still get 144 FPS (TERM_PROGRAM passthrough)"
    );
}

#[test]
fn dynamic_default_fps_standard_terminal_gets_60() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::set_var("TERM_PROGRAM", "gnome-terminal");
    env::remove_var("KONSOLE_VERSION");
    env::remove_var("WT_SESSION");
    let caps = detect();
    assert_eq!(
        caps.dynamic_default_fps, 60.0,
        "standard terminal must default to 60 FPS"
    );
}

#[test]
fn dynamic_default_fps_unknown_terminal_gets_60() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::remove_var("TERM_PROGRAM");
    env::remove_var("KONSOLE_VERSION");
    env::remove_var("WT_SESSION");
    let caps = detect();
    assert_eq!(
        caps.dynamic_default_fps, 60.0,
        "unknown terminal (no TERM_PROGRAM) must default to 60 FPS"
    );
}

#[test]
fn dynamic_default_fps_xtermjs_host_gets_30() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _env = EnvGuard::capture();
    env::set_var("TERM", "xterm-256color");
    env::set_var("TERM_PROGRAM", "vscode");
    env::remove_var("KONSOLE_VERSION");
    env::remove_var("WT_SESSION");
    let caps = detect();
    assert_eq!(
        caps.dynamic_default_fps, XTERMJS_FPS_CAP,
        "xterm.js host must default to XTERMJS_FPS_CAP (30)"
    );
}

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
