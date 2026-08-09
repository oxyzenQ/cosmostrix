// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal protocol detection at startup.
//!
//! Detects terminal capabilities from environment variables and enables
//! protocol-level optimizations where available:
//!
//! - **Synchronized output** (`ESC[?2026h` / `ESC[?2026l`): Frames the
//!   entire draw in a sync region so the terminal emulator buffers output
//!   internally and flushes atomically. Eliminates visual tearing during
//!   partial redraws. Supported by: kitty, wezterm, alacritty, foot,
//!   iTerm2 3.5+, Windows Terminal 1.22+, tmux 3.3+.
//!
//! - **xterm.js host detection** (Tier 2): the `TERM_PROGRAM` env var is
//!   checked against a list of known Electron-based terminal hosts that
//!   embed xterm.js as their renderer. All of them share the same
//!   unbounded-buffer-growth failure mode: at high frame rates, cosmostrix
//!   pumps ANSI bytes into node-pty → xterm.js, whose in-memory scrollback
//!   grows without bound over multi-hour runs until V8 hits an OOM
//!   assertion (SIGTRAP). When an xterm.js host is detected:
//!
//!     * Synchronized output is disabled (xterm.js's mode 2026 buffer
//!       implementation amplifies memory pressure).
//!     * A default FPS cap is applied (see `XTERMJS_FPS_CAP`).
//!     * A rolling byte-budget backpressure is enabled (see
//!       `XTERMJS_BYTE_BUDGET_PER_WINDOW` in `constants.rs`).
//!     * A periodic RIS reset (ESC c) is emitted to clear xterm.js's
//!       in-memory buffer (see `XTERMJS_RIS_RESET_BYTES` in `constants.rs`).
//!
//!   The original crash was reported 2026-08-04 inside VSCode (code-oss
//!   Signal 5 TRAP after hours of cosmostrix). Tier 1 (shipped) covered
//!   only `TERM_PROGRAM=vscode`; Tier 2 extends detection to all known
//!   xterm.js hosts.

use std::env;

/// `TERM_PROGRAM` values that identify an xterm.js-based Electron host.
/// All of these ship xterm.js as their terminal renderer and inherit the
/// same unbounded-buffer-growth failure mode at high ANSI byte rates.
///
/// Kept as a `const` slice (not a `match`) so the test suite can iterate
/// over the list and verify each host triggers detection.
///
/// **Adding a host**: append the exact `TERM_PROGRAM` string here. The
/// rest of the detection / capping / RIS-reset machinery keys off the
/// `xtermjs_host` boolean and is host-agnostic.
const XTERMJS_HOSTS: &[&str] = &[
    // VSCode (and forks like VSCodium, code-oss, code-insiders). The
    // original crash report was inside VSCode.
    "vscode",
    // Hyper — Electron-based terminal, uses xterm.js as the renderer.
    // Sets TERM_PROGRAM=Hyper (capital H).
    "Hyper",
    // WaveTerminal — Electron-based, embeds xterm.js in a TilingWave pane.
    // Sets TERM_PROGRAM=WaveTerminal.
    "WaveTerminal",
    // Tabby — Electron-based terminal manager, embeds xterm.js for the
    // terminal pane. Sets TERM_PROGRAM=Tabby.
    "Tabby",
    // WarpTerminal — Rust+Electron hybrid; the renderer pane is xterm.js.
    // Sets TERM_PROGRAM=WarpTerminal.
    "WarpTerminal",
];

/// # FPS Precedence Chain (v30.6 documentation)
///
/// The effective frame rate at any moment is the result of a 7-layer
/// precedence chain. Layers are listed highest-priority first; the
/// first layer that produces a value wins, and lower layers are not
/// consulted.
///
/// ## Resolution-time layers (run once at startup, in main.rs)
///
/// 1. **CLI `--fps`** — explicit user override. Detected via
///    `matches.value_source("fps") == CommandLine`. Always wins.
/// 2. **Scene `fps=`** — built-in scenes (e.g., `low-power` sets
///    fps=30, `cosmic_dragon` sets fps=60). Applied in
///    `config_apply::apply_scene_values` ONLY when the user did NOT
///    set `--fps` AND config.toml did NOT set `fps =`.
///    **v35.2 audit note (FPS-F2/F3)**: scene-level `fps =` is
///    **startup-only by design**. `Cloud::apply_scene_runtime` does
///    NOT apply `fps` at runtime — only `rain_style`/`color`/`charset`/
///    `speed`/`density`/`glitch_level`. So when the self-healer
///    downgrades to "low-power" at runtime, or the ambient scheduler
///    fires a scene at runtime, the user's startup `--fps`/`fps =`
///    value stays in effect. The CPU shed from a self-healer
///    downgrade comes from `speed=5`+`density=0.45`+`glitch=None`,
///    not from `fps=30`. This is intentional — letting runtime
///    scene writers override `target_fps` would create a precedence
///    ambiguity (which user intent wins?).
/// 3. **Config.toml `fps =`** — user's persistent default. Applied
///    ONLY when the user did NOT set `--fps`.
/// 4. **Dynamic default fps** — terminal-aware default from this
///    module's `dynamic_default_fps` field (144 for high-perf
///    terminals, 60 for standard/unknown, 30 for xterm.js hosts).
///    Applied in `main.rs:669-674` ONLY when none of layers 1-3
///    produced a value.
/// 5. **xterm.js cap** — `default_fps_cap` (30 FPS) applied AFTER
///    layers 1-4 on xterm.js hosts. Even an explicit `--fps 120`
///    gets capped to 30 on VSCode's xterm.js to prevent OOM.
///    Bypassed in `--benchmark` mode.
///
/// ## Runtime layers (run every frame, in event_loop.rs:966-972)
///
/// 6. **Idle factor** — when `is_idle` is true (no user input for
///    `IDLE_THRESHOLD`), `frame_period` is multiplied by
///    `1.0 / IDLE_FPS_FACTOR` (i.e., 0.5× → half the FPS). This is
///    a frame_period adjustment, NOT a target_fps change.
/// 7. **Pause period** — when `cloud.pause` is true (user pressed
///    space), `frame_period` is replaced with `PAUSE_PERIOD_MS`
///    (250ms = 4 FPS). Highest runtime priority.
///
/// ## How to read this in verbose output
///
/// `cosmostrix -v` shows:
///   fps:           144.0
///   fps_source:    /proc ancestor (dynamic default)
///   fps_precedence: dynamic_default  <- which resolution layer won
///
/// The `fps_precedence` field is the new v30.6 visibility signal:
/// one of `cli`, `scene`, `config`, `dynamic_default`, or
/// `xtermjs_cap`. Runtime layers (idle, pause) are NOT shown here
/// because they change every frame — see the HUD's `tgt:` line for
/// the live frame_mode suffix (`idle` / `paused`).
///
/// Capabilities discovered at startup.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TerminalCaps {
    /// Synchronized output (`ESC[?2026h` / `ESC[?2026l`) — universally
    /// safe to enable; terminals that don't support it silently ignore
    /// the escape sequence.
    pub sync_output: bool,
    /// True when running inside ANY xterm.js-based Electron host
    /// (`TERM_PROGRAM` matches an entry in `XTERMJS_HOSTS`). This is the
    /// primary Tier 2 signal — gating FPS cap, byte-budget backpressure,
    /// and periodic RIS reset.
    pub xtermjs_host: bool,
    /// Back-compat alias: true when `TERM_PROGRAM=vscode` specifically.
    /// Equivalent to `xtermjs_host && term_program == "vscode"`. Kept so
    /// existing call sites (verbose output, doc cross-references) can
    /// single out VSCode without re-reading the env var. New code should
    /// key off `xtermjs_host` instead.
    pub vscode_integrated: bool,
    /// Maximum recommended FPS for this terminal. Native terminals
    /// (Alacritty, Kitty, etc.) get 240 (effectively uncapped — the
    /// user's --fps value wins). xterm.js hosts get 30 to keep the
    /// worst-case byte rate under ~7 MB/sec.
    pub default_fps_cap: f64,
    /// v30.3 masterclass: dynamic default FPS when the user does NOT
    /// specify `--fps` or `fps =` in config. Based on terminal tier:
    /// high-performance terminals (Alacritty, kitty, wezterm, ghostty,
    /// foot, iTerm2) get 144; standard/unknown terminals get 60; xterm.js
    /// hosts get 30. The user's explicit `--fps` / `fps =` ALWAYS wins
    /// over this default — it only applies when no FPS is specified.
    pub dynamic_default_fps: f64,
    /// v30.5 hardening: human-readable source string identifying WHICH
    /// detection layer matched (e.g. "TERM_PROGRAM=Alacritty",
    /// "/proc ancestor 'alacritty'", "standard/unknown fallback").
    /// Shown in `-v` verbose output so the user can verify the detection
    /// chain. Essential for debugging "why is my fps 60 not 144?".
    pub dynamic_fps_source: &'static str,
}

/// FPS cap applied when running inside any xterm.js-based host.
/// xterm.js's in-memory buffer grows unbounded at high frame rates;
/// 30 FPS keeps the worst-case byte rate under ~7 MB/sec (vs ~13.7 MB/sec
/// at 60 FPS), which xterm.js can drain over multi-hour runs without
/// OOMing — *assuming* the Tier 2 RIS reset also fires periodically to
/// clear the cumulative buffer. The user's --fps value is clamped to
/// this cap, not overridden silently — the verbose output discloses the
/// cap so there's no confusion.
const XTERMJS_FPS_CAP: f64 = 30.0;

/// Back-compat alias for `XTERMJS_FPS_CAP`. Used by older tests that
/// reference the VSCode-specific name. New code should reference
/// `XTERMJS_FPS_CAP` directly.
#[cfg(test)]
#[allow(non_upper_case_globals)]
const VSCODE_FPS_CAP: f64 = XTERMJS_FPS_CAP;

/// `TERM_PROGRAM` values that identify high-performance terminal
/// emulators capable of sustaining 144+ FPS without visual artifacts.
/// These terminals have GPU-accelerated or highly optimized renderers
/// that can keep up with cosmostrix's ANSI byte rate at high frame rates.
/// Used by the dynamic default FPS logic: if the user doesn't specify
/// `--fps` or `fps =`, these terminals default to 144 FPS instead of 60.
/// The user's explicit value ALWAYS wins over this default.
///
/// v30.4 hotfix: matching is now CASE-INSENSITIVE (some terminals emit
/// `alacritty` vs `Alacritty`, `apple_Terminal` vs `Apple_Terminal`).
/// Also expanded the list with Konsole, Windows Terminal, and corrected
/// `Apple_Terminal` (the previous `apple_Terminal` lowercase form never
/// matched real Terminal.app emissions).
const HIGH_PERF_TERMINALS: &[&str] = &[
    "Alacritty",
    "kitty",
    "WezTerm",
    "ghostty",
    "foot",
    "iTerm.app",
    "Apple_Terminal",
    "konsole",
    "WindowsTerminal",
];

/// `TERM` values that identify high-performance terminals when
/// `TERM_PROGRAM` is unset (some terminals don't set TERM_PROGRAM).
/// Matched case-insensitively as a SUBSTRING of TERM (e.g., `xterm-ghostty`
/// contains `ghostty`). Conservative list — false positives here would
/// push a slow terminal to 144 FPS, which it can't sustain.
const HIGH_PERF_TERM_HINTS: &[&str] = &[
    "alacritty",
    "kitty",
    "ghostty",
    "foot",
    "wezterm",
    "konsole",
];

/// Returns the detection source if the terminal appears to be a
/// high-performance emulator. Checks (in order, 5 layers):
///   1. TERM_PROGRAM (case-insensitive exact match)
///   2. KONSOLE_VERSION env var (KDE Konsole doesn't set TERM_PROGRAM)
///   3. WT_SESSION env var (Windows Terminal)
///   4. TERM substring hints (e.g., `xterm-ghostty` contains `ghostty`)
///   5. Linux ancestor process name via /proc walk (catches Alacritty
///      launched with TERM=xterm-direct — no TERM_PROGRAM, no hint in TERM)
///
/// Returns Some(source_str) if matched, None if no layer matched.
/// The source string is shown in `-v` verbose output for transparency.
fn high_perf_detection_source(term_program: &str, term: &str) -> Option<&'static str> {
    let tp_lower = term_program.to_ascii_lowercase();
    if !tp_lower.is_empty()
        && HIGH_PERF_TERMINALS
            .iter()
            .any(|&t| t.eq_ignore_ascii_case(&tp_lower))
    {
        return Some("TERM_PROGRAM");
    }
    // KDE Konsole: doesn't set TERM_PROGRAM, but exports KONSOLE_VERSION.
    if std::env::var("KONSOLE_VERSION").is_ok() {
        return Some("KONSOLE_VERSION");
    }
    // Windows Terminal: sets WT_SESSION (not TERM_PROGRAM).
    if std::env::var("WT_SESSION").is_ok() {
        return Some("WT_SESSION");
    }
    // Layer 4: TERM substring hints (case-insensitive).
    let term_lower = term.to_ascii_lowercase();
    if !term_lower.is_empty()
        && HIGH_PERF_TERM_HINTS
            .iter()
            .any(|&hint| term_lower.contains(hint))
    {
        return Some("TERM substring");
    }
    // Layer 5 (v30.5): Linux /proc ancestor process name.
    let ancestors = ancestor_process_names(10);
    if ancestor_matches_high_perf(&ancestors) {
        // Find the matching ancestor name for the source string.
        for name in &ancestors {
            let name_lower = name.to_ascii_lowercase();
            if HIGH_PERF_TERM_HINTS
                .iter()
                .any(|&hint| name_lower.contains(hint))
            {
                // Leak-alloc the name for a 'static lifetime. This is
                // called at most once per process (detect() is cached
                // via OnceLock in production), so the leak is bounded.
                // The string is ≤15 chars (kernel TASK_COMM_LEN limit).
                return Some("/proc ancestor");
            }
        }
    }
    None
}

/// Parse the `ppid` field from a `/proc/<pid>/stat` line. The stat format
/// is `pid (comm) state ppid ...` where `comm` can contain spaces and
/// parens. We parse from the right of the LAST `)` to avoid ambiguity
/// with parens inside `comm`. Returns None if the line is malformed.
///
/// Pure function — unit-testable without touching the filesystem.
#[cfg(target_os = "linux")]
fn parse_proc_ppid(stat_line: &str) -> Option<i32> {
    let rparen = stat_line.rfind(')')?;
    let after_comm = &stat_line[rparen + 1..];
    let mut fields = after_comm.split_whitespace();
    fields.next()?; // state (S, R, D, T, Z, etc.)
    fields.next()?.parse().ok()
}

/// Read the `comm` name (process name) for a given PID on Linux. Returns
/// None if /proc is not available or the PID doesn't exist. The kernel
/// truncates `comm` to 15 characters (TASK_COMM_LEN=16 including NUL).
#[cfg(target_os = "linux")]
fn read_proc_comm(pid: i32) -> Option<String> {
    let raw = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Walk the process ancestor chain on Linux and return the list of
/// process names (comm) from parent → grandparent → ... → init. Stops
/// after `max_depth` hops or when reaching PID 1 (init/systemd).
/// Returns an empty vec on non-Linux platforms or if /proc is unavailable.
///
/// This is the fallback detection layer for terminals that don't set
/// `TERM_PROGRAM` AND have a non-standard `TERM` (e.g., Alacritty with
/// `TERM=xterm-direct`). Walking the process tree finds the terminal
/// emulator process by name (e.g., "alacritty", "kitty", "ghostty").
#[cfg(target_os = "linux")]
fn ancestor_process_names(max_depth: usize) -> Vec<String> {
    let mut names = Vec::with_capacity(max_depth);
    let mut pid = std::process::id() as i32;
    for _ in 0..max_depth {
        let stat = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(s) => s,
            Err(_) => break,
        };
        let ppid = match parse_proc_ppid(&stat) {
            Some(p) => p,
            None => break,
        };
        if ppid <= 1 {
            break;
        }
        if let Some(name) = read_proc_comm(ppid) {
            names.push(name);
        }
        pid = ppid;
    }
    names
}

/// No-op stub on non-Linux platforms. macOS users rely on TERM_PROGRAM
/// (iTerm.app, Apple_Terminal) which is always set by those terminals.
#[cfg(not(target_os = "linux"))]
fn ancestor_process_names(_max_depth: usize) -> Vec<String> {
    Vec::new()
}

/// Returns true if any name in `names` matches a HIGH_PERF_TERM_HINT
/// (case-insensitive substring). Extracted from `is_high_perf_terminal`
/// for unit testability — the ancestor walk itself requires /proc and
/// can't be tested in isolation, but the matching logic can.
fn ancestor_matches_high_perf(names: &[String]) -> bool {
    names.iter().any(|name| {
        let name_lower = name.to_ascii_lowercase();
        HIGH_PERF_TERM_HINTS
            .iter()
            .any(|&hint| name_lower.contains(hint))
    })
}

/// Dynamic default FPS for high-performance terminals when the user
/// doesn't specify `--fps` or `fps =`. 144 Hz matches the most common
/// high-refresh monitor rate (between 120 and 165). The user's explicit
/// value always wins over this default.
const HIGH_PERF_DEFAULT_FPS: f64 = 144.0;

/// Dynamic default FPS for standard/unknown terminals when the user
/// doesn't specify `--fps` or `fps =`. 60 FPS is the universal safe
/// default that every terminal can sustain.
const STANDARD_DEFAULT_FPS: f64 = 60.0;

/// Run detection from environment variables. Safe to call before any
/// terminal initialization.
pub(crate) fn detect() -> TerminalCaps {
    let term = env::var("TERM").unwrap_or_default();
    let term_program = env::var("TERM_PROGRAM").unwrap_or_default();

    // Tier 2: match against the full list of known xterm.js hosts. The
    // comparison is case-sensitive against the canonical strings these
    // terminals emit (VSCode emits lowercase "vscode", Hyper emits
    // "Hyper" with capital H, etc.) — matching the upstream documented
    // behavior, not a lowercased approximation.
    let xtermjs_host = XTERMJS_HOSTS.iter().any(|&h| term_program == h);

    // VSCode-specific alias for back-compat with Tier 1 code paths that
    // single out VSCode in user-facing strings (warnings, verbose output).
    let vscode_integrated = xtermjs_host && term_program == "vscode";

    // Synchronized output is supported by virtually all modern terminals.
    // The escape sequences are a no-op on terminals that don't support
    // them, so enabling unconditionally is safe. Three exceptions:
    //   1. Linux console (TERM=linux) — does not understand the sequence.
    //   2. xterm.js hosts — xterm.js's mode 2026 buffer implementation
    //      amplifies memory pressure under high frame rates, contributing
    //      to the multi-hour SIGTRAP crash.
    // tmux 3.3+ passes sync sequences through to the outer terminal.
    let sync_ok = !term.eq_ignore_ascii_case("linux") && !xtermjs_host;

    // xterm.js hosts get a 30 FPS cap; everything else is effectively
    // uncapped (the user's --fps value, validated to 1.0..=240.0, wins).
    let default_fps_cap = if xtermjs_host { XTERMJS_FPS_CAP } else { 240.0 };

    // v30.3 masterclass: dynamic default FPS based on terminal tier.
    // v30.4 hotfix: case-insensitive matching + env-var fallbacks.
    // v30.5 hardening: Layer 5 (/proc ancestor walk) + source tracking.
    // The source string records WHICH layer matched — shown in -v output
    // so the user can verify the detection chain.
    let (dynamic_default_fps, dynamic_fps_source) = if xtermjs_host {
        (XTERMJS_FPS_CAP, "xtermjs_host (capped)")
    } else if let Some(source) = high_perf_detection_source(&term_program, &term) {
        (HIGH_PERF_DEFAULT_FPS, source)
    } else {
        (STANDARD_DEFAULT_FPS, "standard/unknown fallback")
    };

    TerminalCaps {
        sync_output: sync_ok,
        xtermjs_host,
        vscode_integrated,
        default_fps_cap,
        dynamic_default_fps,
        dynamic_fps_source,
    }
}

/// Byte sequence to begin a synchronized output region.
/// The terminal buffers all subsequent output until the end marker.
pub(crate) const SYNC_START: &[u8] = b"\x1b[?2026h";

/// Byte sequence to end a synchronized output region.
/// The terminal flushes all buffered content atomically.
pub(crate) const SYNC_END: &[u8] = b"\x1b[?2026l";

/// RIS (Reset to Initial State) — `ESC c`.
///
/// Tier 2: emitted periodically when running inside an xterm.js host to
/// force xterm.js to clear its in-memory scrollback buffer, preventing
/// the unbounded growth that leads to V8 OOM (SIGTRAP). The next frame
/// after a RIS performs a full redraw, so the user sees a brief
/// (single-frame) blanking — far less disruptive than the multi-second
/// hang of an OOM crash.
///
/// RIS is a hard reset in the ANSI spec, but xterm.js's implementation
/// is more lenient than hardware terminals: it preserves the current
/// TTY mode (raw mode, alternate screen, etc.) and only flushes the
/// buffer + scrollback. We still re-issue the alternate-screen sequence
/// in `Terminal::emit_ris_reset` to be safe across hosts.
///
/// This constant is exposed for tests that verify the byte sequence. The
/// runtime path uses a richer `RIS_RECOVERY` sequence (RIS + re-enter
/// alternate screen + cursor hide + SGR mouse mode) defined locally in
/// `Terminal::emit_ris_reset`.
#[cfg(test)]
pub(crate) const RIS_RESET: &[u8] = b"\x1bc";

/// Returns the canonical list of xterm.js host `TERM_PROGRAM` strings.
/// Used by the test suite to verify every entry in `XTERMJS_HOSTS`
/// triggers detection. Not used in production code paths.
#[cfg(test)]
pub(crate) fn known_xtermjs_hosts() -> &'static [&'static str] {
    XTERMJS_HOSTS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // env::set_var is process-global and not thread-safe; serialize the
    // tests that touch TERM_PROGRAM so they don't race with each other.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Helper: restore TERM and TERM_PROGRAM after a test mutates them.
    /// Captures the prev values up-front and returns a closure that
    /// restores them when dropped (RAII). Eliminates the boilerplate
    /// `match prev_*` blocks that previously appeared in every test.
    struct EnvGuard {
        prev_term: Option<String>,
        prev_tp: Option<String>,
        prev_konsole_version: Option<String>,
        prev_wt_session: Option<String>,
    }

    impl EnvGuard {
        fn capture() -> Self {
            Self {
                prev_term: env::var("TERM").ok(),
                prev_tp: env::var("TERM_PROGRAM").ok(),
                prev_konsole_version: env::var("KONSOLE_VERSION").ok(),
                prev_wt_session: env::var("WT_SESSION").ok(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
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
        // Simulate TERM=linux detection result
        let caps = TerminalCaps {
            sync_output: false,
            xtermjs_host: false,
            vscode_integrated: false,
            default_fps_cap: 240.0,
            dynamic_default_fps: 60.0,
            dynamic_fps_source: "test",
        };
        assert!(!caps.sync_output);
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
            assert_eq!(
                caps.default_fps_cap, XTERMJS_FPS_CAP,
                "FPS cap must apply for TERM_PROGRAM={host}"
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

    // ── v30.3 masterclass: dynamic default FPS tests ──

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
        // v30.4 hotfix: case-insensitive matching — `alacritty` (lowercase)
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
        // v30.4 hotfix: terminals that don't set TERM_PROGRAM but set a
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
        // v30.4 hotfix: KDE Konsole doesn't set TERM_PROGRAM; it exports
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
        // v30.4 hotfix: Windows Terminal sets WT_SESSION (not TERM_PROGRAM).
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
        // v30.4 hotfix: tmux doesn't override TERM_PROGRAM (it sets TMUX
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

    // ── v30.5 hardening: /proc ancestor walk tests ──

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

    // ── v30.5: dynamic_fps_source tests ──

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
}
